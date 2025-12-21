use axum::{extract::State, Json};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ChatReq {
    pub prompt: String,
}

pub async fn chat_complete(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<ChatReq>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // 1) Require Ready status
    let active = {
        let rt = state.runtime.lock().await;
        match &rt.status {
            crate::runtime::RuntimeStatus::Ready { active, .. } => active.clone(),
            _ => return Err((axum::http::StatusCode::SERVICE_UNAVAILABLE, "Runtime not ready".to_string())),
        }
    };

    // 2) Execute
    let out = {
        let rt = state.runtime.lock().await;
        rt.provider.complete(&req.prompt).await
            .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "result": out,
        "model_used": {
            "repo_id": active.repo_id,
            "revision": active.revision,
            "manifest_hash": active.manifest_hash,
        }
    })))
}
