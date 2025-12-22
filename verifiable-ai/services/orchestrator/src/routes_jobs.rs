use axum::{Json, extract::{State, Path}, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Serialize)]
pub struct JobCreatedResponse {
    pub job_id: Uuid,
    pub status: String,
}

pub async fn perform_create_job(
    State(state): State<SharedState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobCreatedResponse>), (StatusCode, String)> {
    let job_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO jobs (id, kind, status, payload)
        VALUES ($1, $2, 'pending', $3)
        "#
    )
    .bind(job_id)
    .bind(req.kind)
    .bind(req.payload)
    .execute(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create job: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(JobCreatedResponse {
            job_id,
            status: "pending".to_string(),
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
}

pub async fn get_job(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, status, payload, error, created_at, updated_at
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
            updated_at
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
