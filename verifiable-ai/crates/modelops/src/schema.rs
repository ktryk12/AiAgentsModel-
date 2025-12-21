use serde::{Deserialize, Serialize};

pub type Hash32 = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelFile {
    pub rel_path: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModelStatus {
    Ready,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelRecord {
    pub repo_id: String,
    pub revision: Option<String>,         // tag/branch/commit (input)
    pub snapshot_dir: String,             // local resolved path
    pub files: Vec<ModelFile>,
    pub manifest_hash: Hash32,            // blake3 over sorted metadata
    pub downloaded_at: u64,
    pub status: ModelStatus,
    pub error: Option<String>,
}

/// Deterministic VDB key: model:<repo_id>@<revision_or_latest>
/// Uses urlencoding to handle special characters in repo_id and revision.
pub fn model_key(repo_id: &str, revision: Option<&str>) -> Vec<u8> {
    let r = urlencoding::encode(repo_id);
    let v = urlencoding::encode(revision.unwrap_or("latest"));
    format!("model:{}@{}", r, v).into_bytes()
}
