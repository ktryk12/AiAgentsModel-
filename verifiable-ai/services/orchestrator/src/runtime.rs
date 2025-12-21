
#[derive(Debug, Clone)]
pub enum RuntimeStatus {
    Empty,
    Loading { started_at: u64 },
    Ready { active: modelops::ActiveModel, loaded_at: u64 },
    Failed { error: String, failed_at: u64 },
}

pub struct ModelRuntimeManager {
    pub status: RuntimeStatus,
    pub pending_reload: bool,
    pub provider: Box<dyn crate::provider::LLMProvider>,
}

impl ModelRuntimeManager {
    pub fn new(provider: Box<dyn crate::provider::LLMProvider>) -> Self {
        Self {
            status: RuntimeStatus::Empty,
            pending_reload: true, // load at startup
            provider,
        }
    }

    pub fn mark_reload_needed(&mut self) {
        self.pending_reload = true;
    }

    pub fn now() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }
}
