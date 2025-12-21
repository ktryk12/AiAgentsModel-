use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub repo_id: String,
    pub revision: Option<String>,
    pub allow_patterns: Option<Vec<String>>,
    pub ignore_patterns: Option<Vec<String>>,
    pub hf_token: Option<String>, // UI sender; vi gemmer ikke permanent.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobCreated {
    pub job_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum JobPhase {
    Pending,
    Downloading,
    Verifying,
    WritingVdb,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: Uuid,
    pub phase: JobPhase,
    pub message: Option<String>,
    pub repo_id: Option<String>,
    pub revision: Option<String>,
    pub snapshot_dir: Option<String>,
    pub manifest_hash_hex: Option<String>,
}
