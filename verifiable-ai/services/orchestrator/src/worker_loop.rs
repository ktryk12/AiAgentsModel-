use std::time::Duration;
use std::collections::HashMap;
use std::os::unix::process::ExitStatusExt;

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Row, Transaction};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use libc;

use crate::state::SharedState;
use crate::events::append_event;

const POLL_EVERY: Duration = Duration::from_secs(5);
const MAX_TOTAL: usize = 2; // Total concurrent jobs across all queues
const LEASE_SECS: i64 = 30;
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);
const MAX_ATTEMPTS: i32 = 5;
const SCAN_LIMIT: i64 = 10;
const AGING_EVERY: Duration = Duration::from_secs(60);
const CONTROL_POLL: Duration = Duration::from_secs(1);
const TERM_GRACE: Duration = Duration::from_secs(5);

fn worker_id() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "orchestrator".to_string())
}

// Hardcoded Quotas
fn quota(queue: &str) -> usize {
    match queue {
        "train" => 1,
        "download" => 1,
        "default" => 1,
        _ => 1,
    }
}

// Stable hash for advisory locks
fn queue_lock_key(queue: &str) -> i64 {
    let mut h = blake3::Hasher::new();
    h.update(queue.as_bytes());
    let bytes = h.finalize();
    // take first 8 bytes as i64 (little endian)
    let b: [u8; 8] = bytes.as_bytes()[0..8].try_into().unwrap_or([0u8; 8]);
    i64::from_le_bytes(b)
}

pub async fn run_worker_loop(state: SharedState) {
    let wid = worker_id();
    info!(worker_id=%wid, "worker_loop: started (strict quotas + registry + lifecycle)");

    // Register worker and start heartbeat
    if let Err(e) = register_worker(&state.pg_pool, &wid, &wid).await {
        error!("failed to register worker: {e:?}");
    }
    let pool_hb = state.pg_pool.clone();
    let wid_hb = wid.clone();
    tokio::spawn(async move {
        run_worker_heartbeat(pool_hb, wid_hb).await;
    });

    loop {
        // Reaper: fail jobs with too many attempts
        if let Ok(n) = reap_max_attempts(&state.pg_pool).await {
            if n > 0 { warn!("reaper: failed {n} jobs due to max attempts"); }
        }

        match claim_one_job_fair_quota_strict(&state.pg_pool, &wid).await {
            Ok(Some(job)) => {
                // Success claim
                let st = state.clone();
                let wid2 = wid.clone();
                tokio::spawn(async move {
                    if let Err(e) = execute_job(st, job, wid2).await {
                        error!("job execute error: {e:?}");
                    }
                });
            }
            Ok(None) => {
                // No job claimed
                sleep(Duration::from_millis(1000)).await;
            }
            Err(e) => {
                warn!("worker_loop: claim failed: {e:?}");
                sleep(POLL_EVERY).await;
            }
        }
    }
}

// Phase 10: Helper to kill child process gracefully
async fn terminate_then_kill(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        // Send SIGTERM using libc
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
    // Wait for grace period
    if tokio::time::timeout(TERM_GRACE, child.wait()).await.is_err() {
        warn!("child did not exit after grace period, sending SIGKILL");
        let _ = child.kill().await;
    }
}

async fn execute_job(state: SharedState, job: ClaimedJob, wid: String) -> Result<()> {
    info!(job_id=%job.id, kind=%job.kind, queue=%job.queue, priority=%job.priority, "worker: starting job");

    append_event(&state.pg_pool, job.id, serde_json::json!({
        "type": "start",
        "source": "orchestrator",
        "message": "Starting worker process",
        "kind": job.kind,
        "queue": job.queue,
        "attempts": job.attempts,
        "priority": job.priority
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
    
    // Heartbeat setup
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
    let child_id = child.id().unwrap_or(0); // keep ID for logging

    // Capture stdout/stderr
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

    // Phase 10: Control Loop
    let mut exit_status = None;
    
    loop {
        // Poll for flags
        let flags_row = sqlx::query(
            r#"SELECT cancel_requested, paused FROM jobs WHERE id=$1"#
        )
        .bind(job.id)
        .fetch_optional(&state.pg_pool)
        .await
        .context("failed to poll job flags")?;

        if let Some(r) = flags_row {
            let cancel_req: bool = r.get("cancel_requested");
            let paused: bool = r.get("paused");

            if cancel_req {
                 info!(job_id=%job.id, "worker: cancel needed, terminating child");
                 terminate_then_kill(&mut child).await;
                 
                 // Mark cancelled in DB
                 let _ = append_event(&state.pg_pool, job.id, serde_json::json!({
                     "type": "cancelled",
                     "source": "orchestrator",
                     "message": "Job cancelled by user"
                 })).await;
                 
                 sqlx::query(
                     r#"
                     UPDATE jobs
                     SET status='cancelled',
                         finished_at=NOW(),
                         lease_owner=NULL,
                         lease_until=NULL,
                         updated_at=NOW()
                     WHERE id=$1 AND status='running' AND lease_owner=$2
                     "#
                 )
                 .bind(job.id)
                 .bind(&wid)
                 .execute(&state.pg_pool)
                 .await?;

                 // Release locks
                 if let Some(ds) = extract_dataset_id(&job.payload) {
                    let _ = release_dataset_lock(&state.pg_pool, &ds, job.id).await;
                 }
                 
                 cancel.cancel(); // stop heartbeat
                 return Ok(());
            }

            if paused {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        // Check if child exited
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                tokio::time::sleep(CONTROL_POLL).await;
            }
            Err(e) => {
                error!("error waiting for child: {e:?}");
                break;
            }
        }
    }
    
    // If we're here, child exited naturally (or we errored waiting)
    let status = exit_status.unwrap_or_else(|| std::process::ExitStatus::from_raw(0)); // fallback
    
    cancel.cancel(); // stop heartbeat
    
    if let Some(ds) = extract_dataset_id(&job.payload) {
        let _ = release_dataset_lock(&state.pg_pool, &ds, job.id).await;
    }

    if status.success() {
        finish_job(&state.pg_pool, job.id).await?;
        info!(job_id=%job.id, "worker: job done");
    } else {
        // If it wasn't a manual cancel (handled above), it's a failure
        // Unless it was a kill signal we sent? No, we return early on cancel.
        fail_job(&state.pg_pool, job.id, format!("worker exit status: {status}")).await?;
        warn!(job_id=%job.id, "worker: job failed");
    }

    Ok(())
}

pub async fn run_aging_task(state: SharedState) {
    info!("aging_task: started");
    loop {
        sleep(AGING_EVERY).await;

        let res = sqlx::query(
            r#"
            UPDATE jobs
            SET priority = LEAST(priority + 1, 1000),
                updated_at = NOW()
            WHERE status = 'pending'
            "#
        )
        .execute(&state.pg_pool)
        .await;

         match res {
            Ok(r) => {
                let n = r.rows_affected();
                if n > 0 {
                    info!("aging: boosted priority for {n} pending jobs");
                }
            }
            Err(e) => warn!("aging: failed: {e:?}"),
        }
    }
}

async fn register_worker(pool: &PgPool, id: &str, hostname: &str) -> Result<()> {
    // Runtime-checked
    sqlx::query(
        r#"
        INSERT INTO workers (id, hostname, last_heartbeat, started_at)
        VALUES ($1, $2, NOW(), NOW())
        ON CONFLICT (id) DO UPDATE
          SET hostname = EXCLUDED.hostname,
              last_heartbeat = NOW()
        "#
    )
    .bind(id)
    .bind(hostname)
    .execute(pool)
    .await?;
    info!("worker registered: {id}");
    Ok(())
}

async fn run_worker_heartbeat(pool: PgPool, id: String) {
    loop {
        sleep(HEARTBEAT_EVERY).await;
        let res = sqlx::query(r#"UPDATE workers SET last_heartbeat = NOW() WHERE id = $1"#)
            .bind(&id)
            .execute(&pool)
            .await;
        if let Err(e) = res {
            warn!("worker heartbeat failed: {e:?}");
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
    priority: i32,
    queue: String,
}

#[derive(Debug, Clone)]
struct ClaimedJob {
    id: uuid::Uuid,
    kind: String,
    payload: JsonValue,
    attempts: i32,
    priority: i32,
    queue: String,
}

#[derive(Debug, Clone)]
struct Usage {
    total_running: usize,
    running_by_queue: HashMap<String, usize>,
}

async fn get_usage(pool: &PgPool) -> Result<Usage> {
    let rows = sqlx::query(
        r#"
        SELECT queue, COUNT(*)::bigint AS running
        FROM jobs
        WHERE status='running'
        GROUP BY queue
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut running_by_queue = HashMap::new();
    let mut total_running = 0usize;

    for r in rows {
        let q: String = r.get("queue");
        let n: i64 = r.get("running");
        let count = n as usize;
        total_running += count;
        running_by_queue.insert(q, count);
    }

    Ok(Usage { total_running, running_by_queue })
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

// --- Fair Scheduling + Quota Logic (Strict) ---

async fn fetch_candidates(pool: &PgPool) -> Result<Vec<JobRow>> {
    let rows: Vec<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, payload, attempts, priority, queue
        FROM jobs
        WHERE
          attempts < $1
          AND (
            status = 'pending'
            OR (status = 'running' AND (lease_until IS NULL OR lease_until < NOW()))
          )
        ORDER BY priority DESC, created_at ASC
        LIMIT $2
        "#
    )
    .bind(MAX_ATTEMPTS)
    .bind(SCAN_LIMIT)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn try_claim_candidate_strict(
    pool: &PgPool,
    job: &JobRow,
    worker_id: &str,
) -> Result<Option<ClaimedJob>> {
    let mut tx = pool.begin().await?;

    // 1. Acquire Logical Lock on Job
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

    // 2. Strict Quota Check via Advisory Lock + Count
    let lock_key = queue_lock_key(&job.queue);
    
    // Explicitly cast lock key to i64 (postgres bigint)
    sqlx::query(r#"SELECT pg_advisory_xact_lock($1)"#)
        .bind(lock_key)
        .execute(&mut *tx)
        .await?;

    let cap = quota(&job.queue) as i64;
    let running_in_queue: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM jobs WHERE queue = $1 AND status='running'"#
    )
    .bind(&job.queue)
    .fetch_one(&mut *tx)
    .await?;

    if running_in_queue >= cap {
         tx.rollback().await?;
         return Ok(None);
    }

    // 3. Dataset lock (if applicable)
    if let Some(dataset_id) = extract_dataset_id(&job.payload) {
        let ok = try_acquire_dataset_lock(&mut tx, &dataset_id, job.id, LEASE_SECS).await?;
        if !ok {
            tx.rollback().await?;
            return Ok(None);
        }
    }

    // 4. Claim job
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
        priority: job.priority,
        queue: job.queue.clone(),
    }))
}

async fn claim_one_job_fair_quota_strict(pool: &PgPool, wid: &str) -> Result<Option<ClaimedJob>> {
    let usage = get_usage(pool).await?;

    // Check MAX TOTAL concurrency
    if usage.total_running >= MAX_TOTAL {
        return Ok(None);
    }

    let candidates = fetch_candidates(pool).await?;

    for job in candidates {
        // Pre-check (Optimistic)
        let q = job.queue.as_str();
        let cap = quota(q);
        let used = usage.running_by_queue.get(q).copied().unwrap_or(0);

        if used >= cap {
            continue; // optimistic skip
        }

        // Strict Claim Attempt
        if let Some(claimed) = try_claim_candidate_strict(pool, &job, wid).await? {
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
