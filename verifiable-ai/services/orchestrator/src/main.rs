mod config;
mod state;
mod routes_models;
mod vdb_exec;
pub mod provider;
mod provider_lmstudio;
pub mod runtime;
pub mod runtime_reload;
pub mod types_training;
pub mod types_jobs;
pub mod training_store;
pub mod dataset_validator;
pub mod orchestrator_job;
pub mod routes_training;
pub mod routes_jobs;
mod routes_runtime;
mod routes_chat;
mod worker_loop;
mod types;
mod worker;

use axum::{routing::{get, post}, Router, extract::{Path, State}, http::StatusCode};
use tower_http::cors::CorsLayer;
use uuid::Uuid;
use tokio::sync::Mutex;
use std::sync::Arc;

// use types::*;
// use worker::*;
use state::*;
use routes_models::*;
// use vdb_exec::with_vdb_blocking;

use std::path::PathBuf;

// use modelops::{ModelFile, ModelRecord, ModelStatus, manifest_hash, put_model, add_model_to_index};

use anyhow::{Context, Result};
// use aws_config::BehaviorVersion;
// use aws_sdk_s3::{config::Region, Client as S3Client};
use sqlx::PgPool;
use tracing::info;

use crate::config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = AppConfig::from_env()?;

    // --- Postgres ---
    let pg_pool = PgPool::connect(&cfg.database_url)
        .await
        .context("Failed to connect to Postgres")?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pg_pool)
        .await
        .context("Failed to run migrations")?;

    // --- S3 (MinIO) --- DEFERRED
    // let s3 = build_s3_client(&cfg).await?;

    // --- Startup health checks (fail fast) ---
    startup_checks(&cfg, &pg_pool).await?;
    
    let db_path = PathBuf::from("orchestrator_vdb.json");
    
    // Init provider & runtime
    let provider = crate::provider_lmstudio::LmStudioProvider::new("http://127.0.0.1:1234".to_string());
    let runtime = crate::runtime::ModelRuntimeManager::new(Box::new(provider));
    let runtime_arc = Arc::new(Mutex::new(runtime));

    let app_state = Arc::new(AppState::new(db_path, runtime_arc, cfg.clone(), pg_pool));

    // Spawn background reloader
    tokio::spawn(crate::runtime_reload::reload_from_vdb(app_state.clone()));
    
    // Spawn Worker Loop (Phase 3)
    let shared_for_worker = app_state.clone();
    tokio::spawn(async move {
        crate::worker_loop::run_worker_loop(shared_for_worker).await;
    });

    // RECOVERY: Scan jobs and fail non-terminal ones from previous run
    if let Err(e) = crate::orchestrator_job::recover_jobs(app_state.vdb.clone()).await {
        eprintln!("WARNING: Job recovery failed: {}", e);
    }

    let app = Router::new()
        //.route("/models/download", post(download_model)) // Removed: unified into training/jobs
        .route("/models/active", get(get_active))
        .route("/runtime", get(crate::routes_runtime::get_runtime))
        .route("/chat/completions", post(crate::routes_chat::chat_complete))
        .route("/training/datasets", post(crate::routes_training::post_dataset))
        .route("/training/datasets", get(crate::routes_training::get_datasets))
        .route("/training/jobs", post(crate::routes_jobs::perform_create_job))
        .route("/training/jobs/:id", get(crate::routes_jobs::get_job))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = &cfg.bind_addr;
    println!("orchestrator listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    
    Ok(())
}


async fn startup_checks(cfg: &AppConfig, pg_pool: &PgPool) -> Result<()> {
    check_comfyui(&cfg.comfyui_url).await?;
    info!("comfyui: ok (8188)");

    check_minio_http(&cfg.s3_endpoint).await?;
    info!("minio: ok (http health)");

    check_postgres(pg_pool).await?;
    info!("postgres: ok");

    Ok(())
}

async fn check_minio_http(base: &str) -> Result<()> {
    let url = format!("{}/minio/health/live", base.trim_end_matches('/'));
    let resp = reqwest::get(&url).await.context("MinIO health request failed")?;
    if resp.status() != StatusCode::OK {
        anyhow::bail!("MinIO unhealthy: HTTP {}", resp.status());
    }
    Ok(())
}

async fn check_comfyui(base: &str) -> Result<()> {
    // ComfyUI has multiple endpoints; "/system_stats" is common.
    let url = format!("{}/system_stats", base.trim_end_matches('/'));
    let resp = reqwest::get(&url).await.context("ComfyUI request failed")?;

    if resp.status() != StatusCode::OK {
        anyhow::bail!("ComfyUI unhealthy: HTTP {}", resp.status());
    }
    Ok(())
}

async fn check_postgres(pg_pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1")
        .execute(pg_pool)
        .await
        .context("Postgres ping failed")?;
    Ok(())
}
