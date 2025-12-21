mod types;
mod worker;
mod state;

use axum::{routing::{get, post}, Json, Router, extract::{Path, State}, http::StatusCode};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use types::*;
use worker::*;
use state::*;

use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command};

use vdb::{InMemoryStorage, VerifiableKV};
use modelops::{ModelFile, ModelRecord, ModelStatus, manifest_hash, put_model};

#[tokio::main]
async fn main() {
    let app_state = AppState::new();

    let app = Router::new()
        .route("/models/download", post(download_model))
        .route("/models/jobs/:id", get(get_job))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = "0.0.0.0:8080";
    println!("orchestrator listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn download_model(
    State(st): State<SharedState>,
    Json(req): Json<DownloadRequest>,
) -> Result<(StatusCode, Json<JobCreated>), (StatusCode, Json<JobStatus>)> {
    let job_id = Uuid::new_v4();

    let mut job = JobStatus {
        job_id,
        phase: JobPhase::Pending,
        message: Some("queued".into()),
        repo_id: Some(req.repo_id.clone()),
        revision: req.revision.clone(),
        snapshot_dir: None,
        manifest_hash_hex: None,
    };
    st.set_job(job.clone()).await;

    // Spawn task
    tokio::spawn(run_worker_job(st.clone(), job_id, req));

    Ok((StatusCode::ACCEPTED, Json(JobCreated { job_id })))
}

async fn get_job(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<JobStatus>, StatusCode> {
    let id = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    st.get_job(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn run_worker_job(st: SharedState, job_id: Uuid, req: DownloadRequest) {
    // Update job → Downloading
    st.update_job(job_id, |j| {
        j.phase = JobPhase::Downloading;
        j.message = Some("starting worker".into());
    }).await;

    let allow_patterns = req.allow_patterns.as_ref().map(|v| v.join(","));
    let ignore_patterns = req.ignore_patterns.as_ref().map(|v| v.join(","));

    let mut cmd = Command::new("python");
    cmd.arg("workers/hf_downloader.py")
        .arg("--repo_id").arg(&req.repo_id);

    if let Some(rev) = &req.revision {
        cmd.arg("--revision").arg(rev);
    }
    if let Some(ap) = allow_patterns {
        cmd.arg("--allow_patterns").arg(ap);
    }
    if let Some(ip) = ignore_patterns {
        cmd.arg("--ignore_patterns").arg(ip);
    }

    // Token via env (ikke logge!)
    if let Some(token) = &req.hf_token {
        cmd.env("HF_TOKEN", token);
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            st.update_job(job_id, |j| {
                j.phase = JobPhase::Failed;
                j.message = Some(format!("failed to spawn worker: {e}"));
            }).await;
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    // Saml output fra worker (done event)
    let mut done_snapshot: Option<(String, Option<String>, String, Vec<ModelFile>)> = None;

    while let Ok(Some(line)) = reader.next_line().await {
        let evt: Result<WorkerEvent, _> = serde_json::from_str(&line);
        match evt {
            Ok(WorkerEvent::Start { repo_id, revision }) => {
                st.update_job(job_id, |j| {
                    j.message = Some(format!("worker started: {repo_id}"));
                    j.repo_id = Some(repo_id);
                    j.revision = revision;
                }).await;
            }
            Ok(WorkerEvent::Progress { phase, detail }) => {
                st.update_job(job_id, |j| {
                    j.message = Some(format!("progress: {phase} {}", detail.unwrap_or_default()));
                }).await;
            }
            Ok(WorkerEvent::Done { repo_id, revision, snapshot_dir, files }) => {
                let mf: Vec<ModelFile> = files.into_iter().map(|f| ModelFile {
                    rel_path: f.rel_path,
                    size: f.size,
                }).collect();

                done_snapshot = Some((repo_id, revision, snapshot_dir, mf));
                break;
            }
            Ok(WorkerEvent::Error { message }) => {
                st.update_job(job_id, |j| {
                    j.phase = JobPhase::Failed;
                    j.message = Some(message);
                }).await;
                return;
            }
            Err(_) => {
                // ignorér garbage lines men du kan logge i debug
            }
        }
    }

    // Wait exit
    let _ = child.wait().await;

    let Some((repo_id, revision, snapshot_dir, files)) = done_snapshot else {
        st.update_job(job_id, |j| {
            j.phase = JobPhase::Failed;
            j.message = Some("worker ended without done event".into());
        }).await;
        return;
    };

    // Verify phase: compute manifest hash
    st.update_job(job_id, |j| {
        j.phase = JobPhase::Verifying;
        j.snapshot_dir = Some(snapshot_dir.clone());
        j.message = Some("computing manifest hash".into());
    }).await;

    let mh = manifest_hash(files.clone());
    let mh_hex = hex::encode(mh);

    // Write VDB phase
    st.update_job(job_id, |j| {
        j.phase = JobPhase::WritingVdb;
        j.manifest_hash_hex = Some(mh_hex.clone());
        j.message = Some("writing ModelRecord to VDB".into());
    }).await;

    // V1: bruger InMemoryStorage. Senere: RocksDB backend.
    let storage = InMemoryStorage::new();
    let mut vdb = VerifiableKV::new(storage);

    let downloaded_at = now_secs();
    let rec = ModelRecord {
        repo_id: repo_id.clone(),
        revision: revision.clone(),
        snapshot_dir: snapshot_dir.clone(),
        files,
        manifest_hash: mh,
        downloaded_at,
        status: ModelStatus::Ready,
        error: None,
    };

    if let Err(e) = put_model(&mut vdb, &rec) {
        st.update_job(job_id, |j| {
            j.phase = JobPhase::Failed;
            j.message = Some(format!("VDB write failed: {e}"));
        }).await;
        return;
    }

    st.update_job(job_id, |j| {
        j.phase = JobPhase::Ready;
        j.message = Some("ready".into());
    }).await;
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
