use axum::{extract::State, Json, http::StatusCode};
use crate::state::AppState;
use crate::SharedState;
use crate::vdb_exec::with_vdb_blocking;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ModelsResponse {
    pub models: Vec<modelops::ModelRecord>,
}

#[derive(Deserialize)]
pub struct UseModelReq {
    pub repo_id: String,
    pub revision: Option<String>,
}

#[derive(Serialize)]
pub struct UseModelResp {
    pub active: modelops::ActiveModel,
}

#[derive(Serialize)]
pub struct ActiveResp {
    pub active: Option<modelops::ActiveModel>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
}

impl From<Box<dyn std::error::Error>> for ApiError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        ApiError { error: e.to_string() }
    }
}

pub async fn get_models(State(st): State<SharedState>) -> Json<ModelsResponse> {
    let models = with_vdb_blocking(st.vdb.clone(), |vdb| {
        modelops::list_models(vdb, /*repair=*/ true).unwrap_or_default()
    }).await.expect("vdb task panicked");

    Json(ModelsResponse { models })
}

pub async fn post_use_model(
    State(st): State<SharedState>,
    Json(req): Json<UseModelReq>,
) -> Result<Json<UseModelResp>, (StatusCode, Json<ApiError>)> {
    let res = with_vdb_blocking(st.vdb.clone(), move |vdb| {
        modelops::set_active_model(vdb, &req.repo_id, req.revision.as_deref())
            .map_err(|e| e.to_string())
    }).await.expect("vdb task panicked");

    match res {
        Ok(active) => {
            // Trigger runtime reload
            {
                let mut rt = st.runtime.lock().await;
                rt.mark_reload_needed();
            }
            Ok(Json(UseModelResp { active }))
        },
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(ApiError { error: e.to_string() }))),
    }
}

pub async fn get_active(State(st): State<SharedState>) -> Json<ActiveResp> {
    let active = with_vdb_blocking(st.vdb.clone(), |vdb| {
        modelops::get_active_model(vdb).ok().flatten()
    }).await.expect("vdb task panicked");

    Json(ActiveResp { active })
}
