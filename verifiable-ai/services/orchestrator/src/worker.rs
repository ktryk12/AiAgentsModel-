use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerEvent {
    #[serde(rename="start")]
    Start {
        repo_id: String,
        revision: Option<String>,
    },
    #[serde(rename="progress")]
    Progress {
        phase: String,
        detail: Option<String>,
    },
    #[serde(rename="done")]
    Done {
        repo_id: String,
        revision: Option<String>,
        snapshot_dir: String,
        files: Vec<WorkerFile>,
    },
    #[serde(rename="error")]
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct WorkerFile {
    pub rel_path: String,
    pub size: u64,
}
