use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::state::SharedState;

const POLL_EVERY: Duration = Duration::from_secs(5);
const MAX_CONCURRENT: usize = 2;
const LEASE_SECS: i64 = 30;
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

fn worker_id() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "orchestrator".to_string())
}

pub async fn run_worker_loop(state: SharedState) {
    let wid = worker_id();
    info!(worker_id=%wid, "worker_loop: started");

    let mut running: usize = 0;

    loop {
        // Naive concurrency check (only for local pool)
        if running >= MAX_CONCURRENT {
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        match claim_one_job(&state.pg_pool, &wid).await {
            Ok(Some(job)) => {
                running += 1;
                let st = state.clone();
                let wid2 = wid.clone();
                tokio::spawn(async move {
                    if let Err(e) = execute_job(st, job, wid2).await {
                        error!("job execute error: {e:?}");
                    }
                });
            }
            Ok(None) => {
                sleep(POLL_EVERY).await;
            }
            Err(e) => {
                warn!("worker_loop: claim failed: {e:?}");
                sleep(POLL_EVERY).await;
            }
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct JobRow {
    id: uuid::Uuid,
    kind: String,
    payload: JsonValue,
    attempts: i32,
}

#[derive(Debug, Clone)]
struct ClaimedJob {
    id: uuid::Uuid,
    kind: String,
    payload: JsonValue,
    attempts: i32,
}

async fn claim_one_job(pool: &PgPool, wid: &str) -> Result<Option<ClaimedJob>> {
    let mut tx: Transaction<Postgres> = pool.begin().await?;

    // 1) Find one job that is:
    // - pending
    // - OR running but lease expired (or missing lease_until for safety)
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, payload, attempts
        FROM jobs
        WHERE
          status = 'pending'
          OR (
              status = 'running'
              AND (lease_until IS NULL OR lease_until < NOW())
          )
        ORDER BY created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(r) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    // 2) Claim it: set running + lease owner/until + bump attempts
    sqlx::query(
        r#"
        UPDATE jobs
        SET
          status = 'running',
          lease_owner = $2,
          lease_until = NOW() + ($3 * INTERVAL '1 second'),
          attempts = attempts + 1,
          updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(r.id)
    .bind(wid)
    .bind(LEASE_SECS)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(ClaimedJob {
        id: r.id,
        kind: r.kind,
        payload: r.payload,
        attempts: r.attempts + 1,
    }))
}

async fn heartbeat(pool: &PgPool, job_id: uuid::Uuid, wid: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET lease_until = NOW() + ($3 * INTERVAL '1 second'),
            updated_at = NOW()
        WHERE id = $1
          AND status = 'running'
          AND lease_owner = $2
        "#
    )
    .bind(job_id)
    .bind(wid)
    .bind(LEASE_SECS)
    .execute(pool)
    .await?;
    Ok(())
}

async fn execute_job(state: SharedState, job: ClaimedJob, wid: String) -> Result<()> {
    info!(job_id=%job.id, kind=%job.kind, attempts=%job.attempts, "worker: starting job");

    append_event(&state.pg_pool, job.id, serde_json::json!({
        "type": "start",
        "source": "orchestrator",
        "message": "Starting worker process",
        "kind": job.kind,
        "attempts": job.attempts
    }))
    .await?;

    // Decide which python script to run
    let (script, args) = match job.kind.as_str() {
        "hf_download" => {
            let repo_id = job.payload.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
            let revision = job.payload.get("revision").and_then(|v| v.as_str());
            let mut a = vec![repo_id.to_string()];
            if let Some(r) = revision { a.push(r.to_string()); }
            ("/app/workers/hf_downloader.py", a)
        }
        "lora_train" => {
            ("/app/workers/lora_trainer.py", vec![job.id.to_string()])
        }
        other => {
            fail_job(&state.pg_pool, job.id, format!("unknown job kind: {other}")).await?;
            return Ok(());
        }
    };
    
    // Setup Heartbeat
    let cancel = CancellationToken::new();
    let cancel_hb = cancel.clone();
    let pool = state.pg_pool.clone();
    let job_id = job.id;
    let wid_hb = wid.clone();
    
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel_hb.cancelled() => break,
                _ = tokio::time::sleep(HEARTBEAT_EVERY) => {
                    if let Err(e) = heartbeat(&pool, job_id, &wid_hb).await {
                        tracing::warn!(job_id=%job_id, "heartbeat failed: {:?}", e);
                    }
                }
            }
        }
    });

    // ... subprocess logic ...
    let mut cmd = Command::new("python3");
    cmd.arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("failed to spawn python worker")?;

    // Stream stdout NDJSON -> job_events
    if let Some(stdout) = child.stdout.take() {
        let pool = state.pg_pool.clone();
        let job_id = job.id;
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(json) = serde_json::from_str::<JsonValue>(&line) {
                    let _ = append_event(&pool, job_id, json).await;
                } else {
                    let _ = append_event(&pool, job_id, serde_json::json!({
                        "type":"progress",
                        "source":"orchestrator",
                        "message":"non-json stdout line",
                        "line": line
                    })).await;
                }
            }
        });
    }

    // Capture stderr too
    if let Some(stderr) = child.stderr.take() {
        let pool = state.pg_pool.clone();
        let job_id = job.id;
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = append_event(&pool, job_id, serde_json::json!({
                    "type":"progress",
                    "source":"stderr",
                    "line": line
                })).await;
            }
        });
    }

    let status = child.wait().await?;
    
    // Stop heartbeat
    cancel.cancel();
    
    if status.success() {
        finish_job(&state.pg_pool, job.id).await?;
        info!(job_id=%job.id, "worker: job done");
    } else {
        fail_job(&state.pg_pool, job.id, format!("worker exit status: {status}")).await?;
        warn!(job_id=%job.id, "worker: job failed");
    }

    Ok(())
}

async fn append_event(pool: &PgPool, job_id: uuid::Uuid, event: JsonValue) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO job_events (job_id, event) VALUES ($1, $2)"#
    )
    .bind(job_id)
    .bind(event)
    .execute(pool)
    .await?;
    Ok(())
}

async fn finish_job(pool: &PgPool, job_id: uuid::Uuid) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status='done',
            lease_owner=NULL,
            lease_until=NULL,
            updated_at=NOW(),
            error=NULL
        WHERE id=$1
        "#
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    append_event(pool, job_id, serde_json::json!({"type":"done","source":"orchestrator"})).await?;
    Ok(())
}

async fn fail_job(pool: &PgPool, job_id: uuid::Uuid, msg: String) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status='failed',
            lease_owner=NULL,
            lease_until=NULL,
            updated_at=NOW(),
            error=$2
        WHERE id=$1
        "#
    )
    .bind(job_id)
    .bind(&msg)
    .execute(pool)
    .await?;
    append_event(pool, job_id, serde_json::json!({"type":"error","source":"orchestrator","message":msg})).await?;
    Ok(())
}
