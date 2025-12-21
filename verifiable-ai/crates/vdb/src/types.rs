//! Core types for verifiable database

use serde::{Deserialize, Serialize};

/// 32-byte hash
pub type Hash32 = [u8; 32];

/// Receipt for a write operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteReceipt {
    pub key: Vec<u8>,
    pub value_hash: Hash32,
    pub state_root: Hash32,
    pub event_hash: Hash32,
    pub signature: Vec<u8>,
}

/// Result of a read operation with proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadResult {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
    pub value_hash: Hash32,
    pub state_root: Hash32,
    pub proof: MerkleProof256,
}

/// Sparse Merkle Tree proof (256-bit depth)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MerkleProof256 {
    /// Sibling hashes from leaf to root
    pub siblings: Vec<Hash32>,
}

impl MerkleProof256 {
    pub fn new() -> Self {
        Self {
            siblings: Vec::with_capacity(256),
        }
    }
}

impl Default for MerkleProof256 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub state_root: Hash32,
    pub latest_event_hash: Hash32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchReceipt {
    pub state_root: Hash32,
    pub latest_event_hash: Hash32,
    pub batch_hash: Hash32,
    pub signature: [u8; 64],
    pub op_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressedProof {
    pub depth: u16,                 // 256
    pub bitmap: Vec<u8>,            // 256 bits => 32 bytes
    pub siblings: Vec<Hash32>,      // only non-default siblings
}
