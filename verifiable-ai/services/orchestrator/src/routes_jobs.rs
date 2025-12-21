use axum::{extract::{State, Path}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;

use crate::state::SharedState;
use crate::types_jobs::{JobRecord, JobPhase, LoraParams, key_job, key_job_index, key_dataset_active};
use crate::types_training::DatasetRecord;
use crate::orchestrator_job::spawn_training_job;
use crate::vdb_exec::with_vdb_blocking;

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub dataset_id: Uuid,
    pub base_model_repo: String,
    pub base_model_revision: String,
    pub name: String,
    pub lora: LoraParams,
}

pub async fn perform_create_job(
    State(state): State<SharedState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobRecord>), (StatusCode, Json<serde_json::Value>)> {
    let job_id = Uuid::new_v4();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

    // 1. Validate dataset & Check Lock & Persist Job (Atomic-ish)
    let job = with_vdb_blocking(state.vdb.clone(), move |db| {
        // A. Check dataset
        let ds_key = crate::training_store::dataset_key(req.dataset_id);
        let ds_bytes = db.get(&ds_key).map_err(|e| e.to_string())?.value
            .ok_or(format!("dataset {} not found", req.dataset_id))?;
        let ds: DatasetRecord = serde_json::from_slice(&ds_bytes).map_err(|_| "invalid dataset record".to_string())?;

        if !ds.validated {
             return Err("dataset not validated".into());
        }

        // B. Check Lock
        let hash_hex = hex::encode(ds.dataset_hash);
        let lock_key = key_dataset_active(&hash_hex);
        if db.get(lock_key.as_bytes()).map_err(|e| e.to_string())?.value.is_some() {
            return Err("conflict: dataset is currently in use by another job".into());
        }

        // C. Create Record
        let job = JobRecord {
            job_id,
            phase: JobPhase::Pending,
            dataset_id: req.dataset_id,
            base_model_repo: req.base_model_repo,
            base_model_revision: req.base_model_revision,
            name: req.name,
            lora: req.lora,
            progress: None,
            started_at: None,
            updated_at: now,
            result: None,
            error: None,
            quality_score: None,
            quality_warnings: vec![],
            quality_policy_version: 1,
        };

        // D. Write Job + Lock + Index
        let job_bytes = serde_json::to_vec(&job).map_err(|e| e.to_string())?;
        db.set(key_job(job_id).as_bytes(), &job_bytes).map_err(|e| e.to_string())?;
        
        db.set(lock_key.as_bytes(), job_id.as_bytes()).map_err(|e| e.to_string())?;
        
        // Add to index
        let idx_key = key_job_index();
        let mut keys: Vec<String> = if let Some(b) = db.get(idx_key.as_bytes()).map_err(|e| e.to_string())?.value {
            serde_json::from_slice(&b).unwrap_or_default()
        } else {
            Vec::new()
        };
        keys.push(key_job(job_id));
        let idx_bytes = serde_json::to_vec(&keys).map_err(|e| e.to_string())?;
        db.set(idx_key.as_bytes(), &idx_bytes).map_err(|e| e.to_string())?;

        Ok::<JobRecord, String>(job)
    }).await.expect("vdb task panicked")
    .map_err(|e| {
        if e.contains("conflict") {
            (StatusCode::CONFLICT, Json(json!({"error": e})))
        } else if e.contains("not found") {
            (StatusCode::NOT_FOUND, Json(json!({"error": e})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
        }
    })?;

    // 2. Spawn Background Task
    let vdb = state.vdb.clone();
    let logs_dir = std::path::PathBuf::from("data/logs"); 
    let adapters_dir = std::path::PathBuf::from("data/adapters");
    
    tokio::spawn(async move {
        if let Err(e) = spawn_training_job(vdb, job_id, logs_dir, adapters_dir).await {
            eprintln!("Job {} failed to spawn: {}", job_id, e);
        }
    });

    Ok((StatusCode::ACCEPTED, Json(job)))
}

pub async fn get_job(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobRecord>, (StatusCode, Json<serde_json::Value>)> {
    let job = with_vdb_blocking(state.vdb.clone(), move |db| {
        let key = key_job(id);
        let bytes = db.get(key.as_bytes()).map_err(|e| e.to_string())?.value.ok_or("job not found")?;
        serde_json::from_slice::<JobRecord>(&bytes).map_err(|e| e.to_string())
    }).await.expect("vdb panicked")
    .map_err(|e| {
         if e == "job not found" {
             (StatusCode::NOT_FOUND, Json(json!({"error": e})))
         } else {
             (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
         }
    })?;

    Ok(Json(job))
}
