use axum::{Json, extract::{State, Path}, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::state::SharedState;
use crate::events::append_event;

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub kind: String,
    pub payload: serde_json::Value,
    pub queue: Option<String>,
}

#[derive(Serialize)]
pub struct JobCreatedResponse {
    pub job_id: Uuid,
    pub status: String,
    pub queue: String,
}

fn infer_queue(kind: &str, provided: Option<&str>) -> String {
    if let Some(q) = provided {
        return q.to_string();
    }
    match kind {
        "lora_train" => "train".to_string(),
        "hf_download" => "download".to_string(),
        _ => "default".to_string(),
    }
}

pub async fn perform_create_job(
    State(state): State<SharedState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobCreatedResponse>), (StatusCode, String)> {
    let job_id = Uuid::new_v4();
    let queue = infer_queue(&req.kind, req.queue.as_deref());

    sqlx::query(
        r#"
        INSERT INTO jobs (id, kind, queue, status, payload, priority)
        VALUES ($1, $2, $3, 'pending', $4, 0)
        "#
    )
    .bind(job_id)
    .bind(&req.kind)
    .bind(&queue)
    .bind(&req.payload)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create job: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(JobCreatedResponse {
            job_id,
            status: "pending".to_string(),
            queue,
        }),
    ))
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    kind: String,
    status: String,
    payload: serde_json::Value,
    error: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    queue: String,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    cancel_requested: bool,
    paused: bool,
}

pub async fn get_job(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, status, payload, error, created_at, updated_at, queue, finished_at, cancel_requested, paused
        FROM jobs
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(r) = row {
        Ok(Json(serde_json::json!({
            "id": r.id,
            "kind": r.kind,
            "status": r.status,
            "queue": r.queue,
            "payload": r.payload,
            "error": r.error,
            "created_at": r.created_at,
            "updated_at": r.updated_at,
            "finished_at": r.finished_at,
            "cancel_requested": r.cancel_requested,
            "paused": r.paused,
        })))
    } else {
        Err((StatusCode::NOT_FOUND, "Job not found".to_string()))
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobListRow {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub attempts: i32,
    pub dataset_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub queue: String,
}

pub async fn get_jobs(
    State(state): State<SharedState>,
) -> Result<Json<Vec<JobListRow>>, (StatusCode, String)> {
    let rows: Vec<JobListRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            kind,
            status,
            attempts,
            payload->>'dataset_id' as dataset_id,
            created_at,
            updated_at,
            queue
        FROM jobs
        ORDER BY created_at DESC
        LIMIT 50
        "#
    )
    .fetch_all(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

#[derive(Serialize)]
pub struct LifecycleResponse {
    pub status: String,
    pub message: String,
}

// Phase 10: Cancel Job
pub async fn cancel_job(
    State(state): State<SharedState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<LifecycleResponse>, (StatusCode, String)> {
    // 1. Try canceling Pending job (Immediate effect)
    let res = sqlx::query(
        r#"
        UPDATE jobs
        SET status='cancelled',
            cancel_requested=TRUE,
            finished_at=NOW(),
            lease_owner=NULL,
            lease_until=NULL,
            updated_at=NOW()
        WHERE id=$1 AND status='pending'
        "#
    )
    .bind(job_id)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() > 0 {
        // Also release any locks (though pending jobs shouldn't have locks usually, but good practice)
        let _ = sqlx::query("DELETE FROM dataset_locks WHERE job_id=$1")
            .bind(job_id)
            .execute(&state.pg_pool).await;

        let _ = append_event(&state.pg_pool, job_id, serde_json::json!({
            "type": "cancelled",
            "source": "api",
            "message": "Pending job cancelled via API"
        })).await;

        return Ok(Json(LifecycleResponse {
            status: "cancelled".to_string(),
            message: "Job was pending and is now cancelled".to_string(),
        }));
    }

    // 2. Try requesting cancel for Running job
    let res = sqlx::query(
        r#"
        UPDATE jobs
        SET cancel_requested=TRUE,
            updated_at=NOW()
        WHERE id=$1 AND status='running'
        "#
    )
    .bind(job_id)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() > 0 {
        return Ok(Json(LifecycleResponse {
            status: "cancel_requested".to_string(),
            message: "Cancel requested for running job".to_string(),
        }));
    }

    Ok(Json(LifecycleResponse {
        status: "noop".to_string(),
        message: "Job is already terminal or not found".to_string(),
    }))
}

// Phase 10: Retry Job
pub async fn retry_job(
    State(state): State<SharedState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<LifecycleResponse>, (StatusCode, String)> {
    let res = sqlx::query(
        r#"
        UPDATE jobs
        SET status='pending',
            cancel_requested=FALSE,
            paused=FALSE,
            error=NULL,
            lease_owner=NULL,
            lease_until=NULL,
            finished_at=NULL,
            updated_at=NOW()
        WHERE id=$1 AND status IN ('failed', 'cancelled')
        "#
    )
    .bind(job_id)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() > 0 {
        // Log retry event (best effort)
        let _ = append_event(&state.pg_pool, job_id, serde_json::json!({
            "type": "retried",
            "source": "api",
            "message": "Job retried via API"
        })).await;

        return Ok(Json(LifecycleResponse {
            status: "pending".to_string(),
            message: "Job has been re-queued".to_string(),
        }));
    }

    Err((StatusCode::BAD_REQUEST, "Job must be failed or cancelled to retry".to_string()))
}

// Phase 10: Pause Job
pub async fn pause_job(
    State(state): State<SharedState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<LifecycleResponse>, (StatusCode, String)> {
    let res = sqlx::query(
        r#"UPDATE jobs SET paused=TRUE, updated_at=NOW() WHERE id=$1 AND status='running'"#
    )
    .bind(job_id)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() > 0 {
        let _ = append_event(&state.pg_pool, job_id, serde_json::json!({"type": "paused", "source": "api"})).await;
        Ok(Json(LifecycleResponse { status: "paused".to_string(), message: "Job paused".to_string() }))
    } else {
        Err((StatusCode::BAD_REQUEST, "Job is not running".to_string()))
    }
}

// Phase 10: Resume Job
pub async fn resume_job(
    State(state): State<SharedState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<LifecycleResponse>, (StatusCode, String)> {
    let res = sqlx::query(
        r#"UPDATE jobs SET paused=FALSE, updated_at=NOW() WHERE id=$1 AND status='running'"#
    )
    .bind(job_id)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() > 0 {
        let _ = append_event(&state.pg_pool, job_id, serde_json::json!({"type": "resumed", "source": "api"})).await;
        Ok(Json(LifecycleResponse { status: "running".to_string(), message: "Job resumed".to_string() }))
    } else {
        Err((StatusCode::BAD_REQUEST, "Job is not running".to_string()))
    }
}

#[derive(Serialize)]
pub struct QueueMetrics {
    running: i64,
    pending: i64,
    cap: i64,
}

#[derive(Serialize)]
pub struct SchedulerMetrics {
    running: i64,
    pending: i64,
    locked_datasets: i64,
    workers_active: i64,
    capacity_pct: i64,
    queues: std::collections::HashMap<String, QueueMetrics>,
}

pub async fn get_scheduler_metrics(
    State(state): State<SharedState>,
) -> Result<Json<SchedulerMetrics>, (StatusCode, String)> {
    let running_total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM jobs WHERE status='running'"#
    )
    .fetch_one(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pending_total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM jobs WHERE status='pending'"#
    )
    .fetch_one(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let locked_datasets: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM dataset_locks WHERE lease_until > NOW()"#
    )
    .fetch_one(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Active workers (heartbeat in last 30s) - Phase 9
    let workers_active: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM workers WHERE last_heartbeat > NOW() - INTERVAL '30 seconds'"#
    )
    .fetch_one(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Hardcoded max concurrent (must match Orchestrator logic)
    let max_concurrent = 2_i64; 

    let capacity_pct = if max_concurrent <= 0 {
        0
    } else {
        (running_total * 100 / max_concurrent).min(100)
    };

    // Per-queue metrics
    let running_by_queue_rows: Vec<(String, Option<i64>)> = sqlx::query_as(
        r#"SELECT queue, COUNT(*)::bigint FROM jobs WHERE status='running' GROUP BY queue"#
    )
    .fetch_all(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pending_by_queue_rows: Vec<(String, Option<i64>)> = sqlx::query_as(
        r#"SELECT queue, COUNT(*)::bigint FROM jobs WHERE status='pending' GROUP BY queue"#
    )
    .fetch_all(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Default caps
    let mut queues = std::collections::HashMap::new();
    let default_caps = std::collections::HashMap::from([
        ("train", 1),
        ("download", 1),
        ("default", 1),
    ]);

    // Populate known queues from db + defaults
    let mut all_queues = std::collections::HashSet::new();
    for q in default_caps.keys() { all_queues.insert(q.to_string()); }
    for (q, _) in &running_by_queue_rows { all_queues.insert(q.clone()); }
    for (q, _) in &pending_by_queue_rows { all_queues.insert(q.clone()); }

    for q in all_queues {
        let run = running_by_queue_rows.iter().find(|(name, _)| name == &q).and_then(|(_, c)| *c).unwrap_or(0);
        let pen = pending_by_queue_rows.iter().find(|(name, _)| name == &q).and_then(|(_, c)| *c).unwrap_or(0);
        let cap = default_caps.get(q.as_str()).copied().unwrap_or(1);
        queues.insert(q, QueueMetrics { running: run, pending: pen, cap });
    }

    Ok(Json(SchedulerMetrics {
        running: running_total,
        pending: pending_total,
        locked_datasets,
        workers_active,
        capacity_pct,
        queues,
    }))
}
