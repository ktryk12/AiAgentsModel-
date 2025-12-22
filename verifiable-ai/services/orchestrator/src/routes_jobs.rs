use axum::{Json, extract::{State, Path}, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::state::SharedState;

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
}

pub async fn get_job(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, status, payload, error, created_at, updated_at, queue
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
            "updated_at": r.updated_at
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

    // Hardcoded for single-instance orchestrator
    let workers_active = 1_i64;
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
