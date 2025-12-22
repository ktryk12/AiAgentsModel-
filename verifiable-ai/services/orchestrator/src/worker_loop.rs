use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Row, Transaction};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::state::SharedState;

const POLL_EVERY: Duration = Duration::from_secs(5);
const MAX_CONCURRENT: usize = 2;
const LEASE_SECS: i64 = 30;
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);
const MAX_ATTEMPTS: i32 = 5;
const SCAN_LIMIT: i64 = 10;

fn worker_id() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "orchestrator".to_string())
}

pub async fn run_worker_loop(state: SharedState) {
    let wid = worker_id();
    info!(worker_id=%wid, "worker_loop: started (fair scheduling)");

    let mut running: usize = 0;

    loop {
        // Reaper: fail jobs with too many attempts
        if let Ok(n) = reap_max_attempts(&state.pg_pool).await {
            if n > 0 { warn!("reaper: failed {n} jobs due to max attempts"); }
        }

        // Naive concurrency check
        if running >= MAX_CONCURRENT {
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        match claim_one_job_fair(&state.pg_pool, &wid).await {
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
                sleep(Duration::from_millis(1000)).await;
            }
            Err(e) => {
                warn!("worker_loop: claim failed: {e:?}");
                sleep(POLL_EVERY).await;
            }
        }
    }
}

// Runtime-checked query
async fn reap_max_attempts(pool: &PgPool) -> Result<u64> {
    let res = sqlx::query(
        r#"
        UPDATE jobs
        SET status='failed',
            error='Max attempts reached',
            lease_owner=NULL,
            lease_until=NULL,
            updated_at=NOW()
        WHERE status IN ('pending','running')
          AND attempts >= $1
        "#
    )
    .bind(MAX_ATTEMPTS)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
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

fn extract_dataset_id(payload: &serde_json::Value) -> Option<String> {
    payload.get("dataset_id").and_then(|v| v.as_str()).map(|s| s.to_string())
}

// Runtime-checked query
async fn try_acquire_dataset_lock(
    tx: &mut Transaction<'_, Postgres>,
    dataset_id: &str,
    job_id: uuid::Uuid,
    lease_secs: i64,
) -> Result<bool> {
    let row = sqlx::query(
        r#"
        INSERT INTO dataset_locks (dataset_id, job_id, lease_until)
        VALUES ($1, $2, NOW() + ($3 * INTERVAL '1 second'))
        ON CONFLICT (dataset_id) DO UPDATE
          SET job_id = EXCLUDED.job_id,
              lease_until = EXCLUDED.lease_until
        WHERE dataset_locks.lease_until < NOW()
        RETURNING dataset_id
        "#
    )
    .bind(dataset_id)
    .bind(job_id)
    .bind(lease_secs)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.is_some())
}

// --- Fair Scheduling Logic ---

async fn fetch_candidates(pool: &PgPool) -> Result<Vec<JobRow>> {
    let rows: Vec<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, payload, attempts
        FROM jobs
        WHERE
          attempts < $1
          AND (
            status = 'pending'
            OR (status = 'running' AND (lease_until IS NULL OR lease_until < NOW()))
          )
        ORDER BY created_at ASC
        LIMIT $2
        "#
    )
    .bind(MAX_ATTEMPTS)
    .bind(SCAN_LIMIT)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn try_claim_candidate(
    pool: &PgPool,
    job: &JobRow,
    worker_id: &str,
) -> Result<Option<ClaimedJob>> {
    let mut tx = pool.begin().await?;

    // Re-lock this specific job (skip if taken in between)
    // NOTE: We don't need 'attempts < $2' or status checks strictly if we trust FOR UPDATE SKIP LOCKED will fail or return nothing if it's gone.
    // But let's be safe and check conditions again.
    let locked: Option<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM jobs
        WHERE id = $1
          AND attempts < $2
          AND (
            status = 'pending'
            OR (status = 'running' AND (lease_until IS NULL OR lease_until < NOW()))
          )
        FOR UPDATE SKIP LOCKED
        "#
    )
    .bind(job.id)
    .bind(MAX_ATTEMPTS)
    .fetch_optional(&mut *tx)
    .await?;

    if locked.is_none() {
        tx.rollback().await?;
        return Ok(None);
    }

    // Dataset lock (if applicable)
    if let Some(dataset_id) = extract_dataset_id(&job.payload) {
        let ok = try_acquire_dataset_lock(&mut tx, &dataset_id, job.id, LEASE_SECS).await?;
        if !ok {
            tx.rollback().await?;
            return Ok(None);
        }
    }

    // Claim job
    sqlx::query(
        r#"
        UPDATE jobs
        SET status='running',
            lease_owner=$2,
            lease_until=NOW() + ($3 * INTERVAL '1 second'),
            attempts=attempts + 1,
            updated_at=NOW()
        WHERE id=$1
        "#
    )
    .bind(job.id)
    .bind(worker_id)
    .bind(LEASE_SECS)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(ClaimedJob {
        id: job.id,
        kind: job.kind.clone(),
        payload: job.payload.clone(),
        attempts: job.attempts + 1,
    }))
}

async fn claim_one_job_fair(pool: &PgPool, wid: &str) -> Result<Option<ClaimedJob>> {
    let candidates = fetch_candidates(pool).await?;

    for job in candidates {
        if let Some(claimed) = try_claim_candidate(pool, &job, wid).await? {
            return Ok(Some(claimed));
        }
    }

    Ok(None)
}

// -----------------------------

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

async fn heartbeat_dataset_lock(pool: &PgPool, dataset_id: &str, job_id: uuid::Uuid) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE dataset_locks
        SET lease_until = NOW() + ($3 * INTERVAL '1 second')
        WHERE dataset_id = $1 AND job_id = $2
        "#
    )
    .bind(dataset_id)
    .bind(job_id)
    .bind(LEASE_SECS)
    .execute(pool)
    .await?;
    Ok(())
}

async fn release_dataset_lock(pool: &PgPool, dataset_id: &str, job_id: uuid::Uuid) -> Result<()> {
    sqlx::query(
        r#"DELETE FROM dataset_locks WHERE dataset_id=$1 AND job_id=$2"#
    )
    .bind(dataset_id)
    .bind(job_id)
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
            if let Some(ds_id) = extract_dataset_id(&job.payload) {
                let _ = release_dataset_lock(&state.pg_pool, &ds_id, job.id).await;
            }
            fail_job(&state.pg_pool, job.id, format!("unknown job kind: {other}")).await?;
            return Ok(());
        }
    };
    
    let cancel = CancellationToken::new();
    let cancel_hb = cancel.clone();
    let pool = state.pg_pool.clone();
    let job_id = job.id;
    let wid_hb = wid.clone();
    let dataset_id = extract_dataset_id(&job.payload);
    
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel_hb.cancelled() => break,
                _ = tokio::time::sleep(HEARTBEAT_EVERY) => {
                    if let Err(e) = heartbeat(&pool, job_id, &wid_hb).await {
                        tracing::warn!(job_id=%job_id, "heartbeat failed: {:?}", e);
                    }
                    if let Some(ds) = &dataset_id {
                        if let Err(e) = heartbeat_dataset_lock(&pool, ds, job_id).await {
                            tracing::warn!(job_id=%job_id, dataset=%ds, "dataset heartbeat failed: {:?}", e);
                        }
                    }
                }
            }
        }
    });

    let mut cmd = Command::new("python3");
    cmd.arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("failed to spawn python worker")?;

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
                        "line": line
                    })).await;
                }
            }
        });
    }

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
    
    cancel.cancel();
    
    if let Some(ds) = extract_dataset_id(&job.payload) {
        let _ = release_dataset_lock(&state.pg_pool, &ds, job.id).await;
    }

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
    // Runtime-checked
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
    // Runtime-checked
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
    // Runtime-checked
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
