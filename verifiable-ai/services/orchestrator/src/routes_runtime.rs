use axum::{extract::State, Json};

pub async fn get_runtime(State(state): State<crate::state::SharedState>) -> Json<serde_json::Value> {
    let rt = state.runtime.lock().await;
    let info = rt.provider.info();

    let (phase, active, error) = match &rt.status {
        crate::runtime::RuntimeStatus::Empty => ("Empty", None, None),
        crate::runtime::RuntimeStatus::Loading { .. } => ("Loading", None, None),
        crate::runtime::RuntimeStatus::Ready { active, .. } => (
            "Ready",
            Some(serde_json::json!({
                "repo_id": active.repo_id,
                "revision": active.revision,
                "manifest_hash": active.manifest_hash,
                "snapshot_dir": active.snapshot_dir,
            })),
            None
        ),
        crate::runtime::RuntimeStatus::Failed { error, .. } => ("Failed", None, Some(error.clone())),
    };

    Json(serde_json::json!({
        "provider": info,
        "phase": phase,
        "active": active,
        "error": error
    }))
}
