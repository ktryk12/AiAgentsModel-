use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::types::{JobPhase, JobStatus};

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub jobs: RwLock<HashMap<Uuid, JobStatus>>,
}

impl AppState {
    pub fn new() -> SharedState {
        Arc::new(Self { jobs: RwLock::new(HashMap::new()) })
    }

    pub async fn set_job(&self, job: JobStatus) {
        self.jobs.write().await.insert(job.job_id, job);
    }

    pub async fn update_job<F: FnOnce(&mut JobStatus)>(&self, id: Uuid, f: F) {
        if let Some(j) = self.jobs.write().await.get_mut(&id) {
            f(j);
        }
    }

    pub async fn get_job(&self, id: Uuid) -> Option<JobStatus> {
        self.jobs.read().await.get(&id).cloned()
    }
}
