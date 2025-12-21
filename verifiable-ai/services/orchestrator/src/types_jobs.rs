use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobPhase {
    Pending,
    LoadingBase,
    LoadingDataset,
    Training,
    Saving,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub epoch: u32,
    pub step: u32,
    pub loss: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub adapter_dir: String,
    pub adapter_manifest_hash_hex: String, // 64 hex (BLAKE3)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraParams {
    pub r: u32,
    pub alpha: u32,
    pub epochs: u32,
    pub lr: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: Uuid,
    pub phase: JobPhase,

    pub dataset_id: Uuid,
    pub base_model_repo: String,
    pub base_model_revision: String,
    pub name: String,

    pub lora: LoraParams,

    pub progress: Option<ProgressSnapshot>,
    pub started_at: Option<u64>,
    pub updated_at: u64,

    pub result: Option<JobResult>,
    pub error: Option<String>,

    // Audit trail / governance
    pub quality_score: Option<u32>,
    pub quality_warnings: Vec<String>,
    pub quality_policy_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TrainingEvent {
    #[serde(rename = "start")]
    Start { ts: Option<u64> },

    #[serde(rename = "loading_base")]
    LoadingBase,

    #[serde(rename = "loading_dataset")]
    LoadingDataset,

    #[serde(rename = "progress")]
    Progress { epoch: u32, step: u32, loss: f32 },

    #[serde(rename = "saving")]
    Saving,

    #[serde(rename = "done")]
    Done { adapter_dir: String, adapter_manifest_hash_hex: String },

    #[serde(rename = "error")]
    Error { message: String },
}

impl JobRecord {
    pub fn apply_event(&mut self, ev: &TrainingEvent, now_ts: u64) {
        self.updated_at = now_ts;

        match ev {
            TrainingEvent::Start { ts } => {
                // Idempotent start
                if matches!(self.phase, JobPhase::Pending | JobPhase::Failed) {
                     self.phase = JobPhase::Pending; 
                }
                if self.started_at.is_none() {
                    self.started_at = ts.or(Some(now_ts));
                }
            }
            TrainingEvent::LoadingBase => self.phase = JobPhase::LoadingBase,
            TrainingEvent::LoadingDataset => self.phase = JobPhase::LoadingDataset,

            TrainingEvent::Progress { epoch, step, loss } => {
                self.phase = JobPhase::Training;
                self.progress = Some(ProgressSnapshot { epoch: *epoch, step: *step, loss: *loss });
            }

            TrainingEvent::Saving => self.phase = JobPhase::Saving,

            TrainingEvent::Done { adapter_dir, adapter_manifest_hash_hex } => {
                self.phase = JobPhase::Ready;
                self.result = Some(JobResult {
                    adapter_dir: adapter_dir.clone(),
                    adapter_manifest_hash_hex: adapter_manifest_hash_hex.clone(),
                });
                self.error = None;
            }

            TrainingEvent::Error { message } => {
                self.phase = JobPhase::Failed;
                self.error = Some(message.clone());
            }
        }
    }
}

// ---- VDB Keys (helpers) ----
pub fn key_job(job_id: Uuid) -> String {
    format!("training:job:{job_id}")
}

pub fn key_job_index() -> &'static str {
    "training:job:index"
}

pub fn key_dataset_active(dataset_hash_hex: &str) -> String {
    format!("training:dataset_active:{}", dataset_hash_hex)
}

pub fn key_model_lora(lora_id: Uuid) -> String {
    format!("model:lora:{lora_id}")
}
