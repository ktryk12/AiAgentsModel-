use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;
use vdb::{VerifiableKV};
use vdb::FileBackedStorage;
// use aws_sdk_s3::Client as S3Client;
use sqlx::PgPool;

use crate::types::{JobPhase, JobStatus};
use crate::config::AppConfig;

pub type SharedState = Arc<AppState>;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<RwLock<HashMap<Uuid, JobStatus>>>,
    pub vdb: Arc<RwLock<VerifiableKV<FileBackedStorage>>>,
    pub runtime: Arc<Mutex<crate::runtime::ModelRuntimeManager>>,
    pub config: AppConfig,
    pub pg_pool: PgPool,
    // pub s3: S3Client,
}

impl AppState {
    pub fn new(
        path: std::path::PathBuf, 
        runtime: Arc<Mutex<crate::runtime::ModelRuntimeManager>>,
        config: AppConfig,
        pg_pool: PgPool,
    ) -> Self {
        let storage = FileBackedStorage::new(path).expect("Failed to init storage");
        let vdb = VerifiableKV::new(storage);
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            vdb: Arc::new(RwLock::new(vdb)),
            runtime,
            config,
            pg_pool,
            // s3,
        }
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
