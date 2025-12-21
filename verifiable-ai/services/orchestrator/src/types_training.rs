use serde::{Serialize, Deserialize};
use uuid::Uuid;

pub type Hash32 = [u8; 32];

pub const KEY_DATASET_INDEX: &[u8] = b"dataset:index";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetRecord {
    pub id: Uuid,
    pub name: String,
    pub file_path: String,      // data/datasets/<uuid>.jsonl
    #[serde(with = "hex", rename = "dataset_hash_hex")]
    pub dataset_hash: Hash32,   // BLAKE3(file_bytes)
    pub examples: u64,
    pub validated: bool,
    pub quality: QualityReport,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityReport {
    pub score: u8,                 // 0..100
    pub warnings: Vec<String>,     // soft issues
    pub hard_errors: Vec<String>,  // should usually be empty if validated=true
    pub duplicate_rate: f32,       // 0.0..1.0
    pub avg_prompt_len: u32,
    pub avg_completion_len: u32,
    pub too_short_count: u64,
}
