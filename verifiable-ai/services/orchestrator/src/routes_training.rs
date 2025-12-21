use axum::{extract::State, Json};
use axum::http::StatusCode;
use axum::extract::Multipart;
use uuid::Uuid;

use crate::state::AppState;
use crate::types_training::{DatasetRecord, QualityReport};
use crate::dataset_validator::validate_jsonl_and_hash;
use crate::training_store::{dataset_key, add_to_dataset_index, list_datasets};

fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

use serde_json::json;

pub async fn post_dataset(
    State(state): State<crate::state::SharedState>,
    mut mp: Multipart,
) -> Result<(StatusCode, Json<DatasetRecord>), (StatusCode, Json<serde_json::Value>)> {
    let mut name: Option<String> = None;
    let mut file_bytes: Option<bytes::Bytes> = None;

    while let Some(field) = mp.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))? {
        match field.name() {
            Some("name") => name = Some(field.text().await.map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?),
            Some("file") => file_bytes = Some(field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?),
            _ => {}
        }
    }

    let name = name.unwrap_or_else(|| "dataset".to_string());
    let bytes = file_bytes.ok_or((StatusCode::BAD_REQUEST, Json(json!({"error": "Missing file"}))))?;

    let id = Uuid::new_v4();
    let tmp_path = format!("data/datasets/tmp_{id}.jsonl");
    let final_path = format!("data/datasets/{id}.jsonl");

    tokio::fs::create_dir_all("data/datasets").await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    tokio::fs::write(&tmp_path, &bytes).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // blocking validate + hash
    let tmp_path_clone = tmp_path.clone();
    let stats = tokio::task::spawn_blocking(move || {
        validate_jsonl_and_hash(std::path::Path::new(&tmp_path_clone))
    }).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
      .map_err(|errs| (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
          "error": "dataset_validation_failed",
          "errors": errs
      }))))?;

    tokio::fs::rename(&tmp_path, &final_path).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let rec = DatasetRecord {
        id,
        name,
        file_path: final_path,
        dataset_hash: stats.dataset_hash,
        examples: stats.examples,
        validated: true,
        quality: stats.quality,
        created_at: now(),
    };

    // write to VDB (spawn_blocking because FileBackedStorage is blocking IO)
    let rec_bytes = serde_json::to_vec(&rec).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    let key = dataset_key(id);

    crate::vdb_exec::with_vdb_blocking(state.vdb.clone(), move |vdb| {
        vdb.set(&key, &rec_bytes).map_err(|e| e.to_string())?;
        add_to_dataset_index(vdb, &key).map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    }).await.expect("vdb task panicked")
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    Ok((StatusCode::CREATED, Json(rec)))
}

pub async fn get_datasets(
    State(state): State<crate::state::SharedState>
) -> Result<Json<Vec<DatasetRecord>>, (StatusCode, Json<serde_json::Value>)> {
    let datasets = crate::vdb_exec::with_vdb_blocking(state.vdb.clone(), move |vdb| {
        list_datasets(vdb).map_err(|e| e.to_string())
    }).await.expect("vdb task panicked")
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    Ok(Json(datasets))
}
