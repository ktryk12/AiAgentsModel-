use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, time::sleep};
use tracing::{error, info, warn};

use crate::state::SharedState;

const POLL_EVERY: Duration = Duration::from_secs(5);
const MAX_CONCURRENT: usize = 2; // tune later

pub async fn run_worker_loop(state: SharedState) {
    info!("worker_loop: started");

    let mut running: usize = 0;

    loop {
        // naive concurrency guard (good enough for one orchestrator instance)
        // If you plan multiple orchestrators, the DB claim logic is the real guard.
        if running >= MAX_CONCURRENT {
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        match claim_one_job(&state.pg_pool).await {
            Ok(Some(job)) => {
                running += 1;
                let st = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = execute_job(st, job).await {
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

#[derive(Debug, Clone)]
struct ClaimedJob {
    id: uuid::Uuid,
    kind: String,
    payload: JsonValue,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct JobRow {
    id: uuid::Uuid,
    kind: String,
    payload: JsonValue,
}

async fn claim_one_job(pool: &PgPool) -> Result<Option<ClaimedJob>> {
    let mut tx: Transaction<Postgres> = pool.begin().await?;

    // Pick one pending job and lock it so others skip it.
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, payload
        FROM jobs
        WHERE status = 'pending'
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

    // Mark running inside the same transaction.
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'running', updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(r.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(ClaimedJob {
        id: r.id,
        kind: r.kind,
        payload: r.payload,
    }))
}

async fn execute_job(state: SharedState, job: ClaimedJob) -> Result<()> {
    info!(job_id=%job.id, kind=%job.kind, "worker: starting job");

    append_event(&state.pg_pool, job.id, serde_json::json!({
        "type": "start",
        "source": "orchestrator",
        "message": "Starting worker process",
        "kind": job.kind
    }))
    .await?;

    // Decide which python script to run
    let (script, args) = match job.kind.as_str() {
        "hf_download" => {
            // payload: { "repo_id": "...", "revision": "..."? }
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

    // Capture stderr too (as progress/error events)
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
        r#"UPDATE jobs SET status='done', updated_at=NOW() WHERE id=$1"#
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    append_event(pool, job_id, serde_json::json!({"type":"done","source":"orchestrator"})).await?;
    Ok(())
}

async fn fail_job(pool: &PgPool, job_id: uuid::Uuid, msg: String) -> Result<()> {
    sqlx::query(
        r#"UPDATE jobs SET status='failed', error=$2, updated_at=NOW() WHERE id=$1"#
    )
    .bind(job_id)
    .bind(&msg)
    .execute(pool)
    .await?;
    append_event(pool, job_id, serde_json::json!({"type":"error","source":"orchestrator","message":msg})).await?;
    Ok(())
}
