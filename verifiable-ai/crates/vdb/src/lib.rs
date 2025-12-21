//! Verifiable Key-Value Database
//! 
//! Provides cryptographically verifiable storage with Sparse Merkle Tree proofs.

mod smt;
mod storage;
mod types;
mod events;
mod crypto;
mod nodestore;
mod history;

pub use types::{Hash32, WriteReceipt, ReadResult, MerkleProof256, Checkpoint, BatchReceipt, CompressedProof};
pub use storage::{Storage, InMemoryStorage, FileBackedStorage};
pub use nodestore::{NodeStore, InMemoryNodeStore, NodeId};
pub use history::{StateHistory, RootPoint};

use smt::SparseMerkleTree;
use events::EventLog;
use ed25519_dalek::{SigningKey, VerifyingKey, Signer as _};
use thiserror::Error;
use rand_core::OsRng;
// use events::{LogEntry, Event, BatchWriteEvent}; // Removed to avoid potential conflicts/unused

#[derive(Debug, Error)]
pub enum VdbError {
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Invalid proof")]
    InvalidProof,
}

pub type Result<T> = std::result::Result<T, VdbError>;

/// Verifiable Key-Value Database
pub struct VerifiableKV<S: Storage, N: NodeStore = InMemoryNodeStore> {
    storage: S,
    smt: SparseMerkleTree<N>,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    event_log: EventLog,
    history: StateHistory,
}

impl<S: Storage> VerifiableKV<S, InMemoryNodeStore> {
    /// Create a new VerifiableKV instance with default in-memory node store
    pub fn new(storage: S) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        
        let node_store = InMemoryNodeStore::new();
        let smt = SparseMerkleTree::new(node_store);
        
        Self {
            storage,
            smt,
            signing_key,
            verifying_key,
            event_log: EventLog::new(),
            history: StateHistory::new(100), // Default 100 history points
        }
    }
}

impl<S: Storage, N: NodeStore> VerifiableKV<S, N> {
    /// Create with a specific node store and signing key (for testing/recovery)
    pub fn with_store_and_key(storage: S, node_store: N, signing_key: SigningKey) -> Self {
        let verifying_key = signing_key.verifying_key();
        
        Self {
            storage,
            smt: SparseMerkleTree::new(node_store),
            signing_key,
            verifying_key,
            event_log: EventLog::new(),
            history: StateHistory::new(100),
        }
    }
    
    /// Set a key-value pair
    pub fn set(&mut self, key: &[u8], value: &[u8]) -> Result<WriteReceipt> {
        let key_hash = crypto::hash_key(key);
        let value_hash = crypto::hash_value(value);
        
        // Store value
        self.storage.set(key, value);
        
        // Update SMT
        self.smt.update(key_hash, value_hash);
        let new_root = self.smt.root();
        
        // Event timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create and log event
        let event = events::WriteEvent {
            operation: events::Operation::Set,
            key: key.to_vec(),
            value_hash,
            prev_event_hash: self.event_log.latest_hash(),
            state_root: new_root,
            timestamp,
        };
        
        let wrapper = events::Event::Single(event.clone());

        let event_bytes = bincode::serialize(&wrapper)
            .map_err(|e| VdbError::Serialization(e.to_string()))?;
        let event_hash: Hash32 = blake3::hash(&event_bytes).into();
        
        // Sign event
        let signature = self.signing_key.sign(&event_bytes);
        
        // Append to log
        let signature_vec = signature.to_bytes().to_vec();
        
        self.event_log.append(events::LogEntry {
            event_hash,
            event: wrapper,
            signature: signature_vec.clone()
        });
        
        self.history.record(RootPoint {
            event_hash,
            state_root: new_root,
            timestamp,
        });

        Ok(WriteReceipt {
            key: key.to_vec(),
            value_hash,
            state_root: new_root,
            event_hash,
            signature: signature_vec,
        })
    }
    
    /// Delete a key
    pub fn delete(&mut self, key: &[u8]) -> Result<WriteReceipt> {
        let key_hash = crypto::hash_key(key);
        let empty_hash = crypto::empty_value_hash(); // Empty value
        
        // Remove from storage
        self.storage.delete(key);
        
        // Update SMT (set to empty)
        self.smt.update(key_hash, empty_hash);
        let new_root = self.smt.root();
        
        // Event timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create and log event
        let event = events::WriteEvent {
            operation: events::Operation::Delete,
            key: key.to_vec(),
            value_hash: empty_hash,
            prev_event_hash: self.event_log.latest_hash(),
            state_root: new_root,
            timestamp,
        };
        
        let wrapper = events::Event::Single(event.clone());

        let event_bytes = bincode::serialize(&wrapper)
            .map_err(|e| VdbError::Serialization(e.to_string()))?;
        let event_hash: Hash32 = blake3::hash(&event_bytes).into();
        
        // Sign event
        let signature = self.signing_key.sign(&event_bytes);
        
        // Append to log
        let signature_vec = signature.to_bytes().to_vec();
        
        self.event_log.append(events::LogEntry {
            event_hash,
            event: wrapper,
            signature: signature_vec.clone()
        });
        
        self.history.record(RootPoint {
            event_hash,
            state_root: new_root,
            timestamp,
        });
        
        Ok(WriteReceipt {
            key: key.to_vec(),
            value_hash: empty_hash,
            state_root: new_root,
            event_hash,
            signature: signature_vec,
        })
    }
    
    /// Batch set operations
    pub fn batch_set(&mut self, ops: &[(&[u8], &[u8])]) -> Result<BatchReceipt> {
        // 1. Prepare ops
        let mut prepared_ops: Vec<([u8; 32], [u8; 32])> = Vec::with_capacity(ops.len());
        
        // We iterate and apply immediately, but we need sorted keys if we were doing a pure batch SMT update.
        // For simple batching, we can reuse logic but we must be deterministic.
        // Phase 1.2 spec says "sort for determinism".
        
        // Let's store full details to operate
        struct Op<'a> {
            key: &'a [u8],
            value: &'a [u8],
            key_hash: Hash32,
            value_hash: Hash32,
        }
        
        let mut full_ops: Vec<Op> = ops.iter().map(|(k, v)| {
            Op {
                key: *k,
                value: *v,
                key_hash: crypto::hash_key(k),
                value_hash: crypto::hash_value(v),
            }
        }).collect();
        
        // Sort by key_hash
        full_ops.sort_by(|a, b| a.key_hash.cmp(&b.key_hash));
        
        // Commitment hasher
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(b"batch");
        
        for op in &full_ops {
            // Store value
            self.storage.set(op.key, op.value);
            
            // Update SMT
            self.smt.update(op.key_hash, op.value_hash);
            
            // Build op hash for commitment: H("set" || key_hash || value_hash)
            let mut op_hasher = blake3::Hasher::new();
            op_hasher.update(b"set");
            op_hasher.update(&op.key_hash);
            op_hasher.update(&op.value_hash);
            let op_hash = op_hasher.finalize();
            commitment_data.extend_from_slice(op_hash.as_bytes());
        }
        
        let batch_hash: Hash32 = blake3::hash(&commitment_data).into();
        let new_root = self.smt.root();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create Batch Event
        let event = events::BatchWriteEvent {
            batch_hash,
            op_count: full_ops.len() as u32,
            prev_event_hash: self.event_log.latest_hash(),
            state_root: new_root,
            timestamp,
        };
        
        let wrapper = events::Event::Batch(event);
        
        let event_bytes = bincode::serialize(&wrapper)
            .map_err(|e| VdbError::Serialization(e.to_string()))?;
        let event_hash: Hash32 = blake3::hash(&event_bytes).into();
        
        let signature = self.signing_key.sign(&event_bytes);
        let signature_bytes = signature.to_bytes();
        
        self.event_log.append(events::LogEntry {
            event_hash,
            event: wrapper,
            signature: signature_bytes.to_vec(),
        });
        
        self.history.record(RootPoint {
            event_hash,
            state_root: new_root,
            timestamp,
        });
        
        Ok(BatchReceipt {
            state_root: new_root,
            latest_event_hash: event_hash,
            batch_hash,
            signature: signature_bytes.to_vec(),
            op_count: full_ops.len() as u32,
        })
    }
    
    /// Get a value with proof
    pub fn get(&self, key: &[u8]) -> Result<ReadResult> {
        let key_hash = crypto::hash_key(key);
        
        // Try to fetch from storage
        let value = self.storage.get(key);
        
        let proof = self.smt.prove(key_hash);
        
        let value_hash = if let Some(ref v) = value {
            crypto::hash_value(v)
        } else {
            crypto::empty_value_hash()
        };
        
        Ok(ReadResult {
            key: key.to_vec(),
            value,
            value_hash,
            state_root: self.smt.root(),
            proof,
        })
    }
    
    /// Get current state root
    pub fn state_root(&self) -> Hash32 {
        self.smt.root()
    }
    
    /// Get verifying key for signature verification
    pub fn verifying_key(&self) -> VerifyingKey {
        self.verifying_key
    }
    
    /// Get checkpoint
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            state_root: self.state_root(),
            latest_event_hash: self.event_log.latest_hash(),
        }
    }
    
    /// Verify proof helper logic (calls SMT static method)
    pub fn verify_proof(
        proof: &MerkleProof256,
        key: &[u8],
        value: Option<&[u8]>,
        state_root: Hash32,
    ) -> bool {
        let key_hash = crypto::hash_key(key);
        let value_hash = if let Some(v) = value {
            crypto::hash_value(v)
        } else {
            crypto::empty_value_hash() // Empty/non-existent
        };
        
        SparseMerkleTree::<N>::verify_proof(proof, key_hash, value_hash, state_root)
    }
    
    // Verification helper for tests/clients
    pub fn verify_event_log(&self, vk: &VerifyingKey) -> bool {
        self.event_log.verify_chain_and_sigs(vk)
    }

    pub fn tamper_last_signature_for_test(&mut self) {
        if let Some(last) = self.event_log.entries.last_mut() {
            if !last.signature.is_empty() {
                last.signature[0] ^= 0x01;
            }
        }
    }

    #[doc(hidden)]
    pub fn history_root_by_event_for_test(&self, event_hash: Hash32) -> Option<Hash32> {
        self.history.root_by_event(event_hash)
    }
    
    // ---------------- Proof Compression Helpers ---------------- //
    
    pub fn compress_proof(&self, proof: &MerkleProof256) -> CompressedProof {
        let mut bitmap = vec![0u8; 32];
        let mut siblings = Vec::new();
        let defaults = self.smt.default_hashes();
        
        for (i, sibling) in proof.siblings.iter().enumerate() {
            if *sibling != defaults[i] {
                // Set bit i
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bitmap[byte_idx] |= 1 << bit_idx;
                
                siblings.push(*sibling);
            }
        }
        
        CompressedProof {
            depth: 256,
            bitmap,
            siblings,
        }
    }
    
    pub fn decompress_proof(&self, compressed: &CompressedProof) -> Result<MerkleProof256> {
        if compressed.depth != 256 {
            return Err(VdbError::InvalidProof);
        }
        
        let defaults = self.smt.default_hashes();
        let mut full_siblings = Vec::with_capacity(256);
        let mut sib_iter = compressed.siblings.iter();
        
        for i in 0..256 {
             let byte_idx = i / 8;
             let bit_idx = i % 8;
             let is_present = (compressed.bitmap[byte_idx] >> bit_idx) & 1 == 1;
             
             if is_present {
                 if let Some(s) = sib_iter.next() {
                     full_siblings.push(*s);
                 } else {
                     return Err(VdbError::InvalidProof);
                 }
             } else {
                 full_siblings.push(defaults[i]);
             }
        }
        
        if sib_iter.next().is_some() {
            return Err(VdbError::InvalidProof); // Too many siblings
        }
        
        Ok(MerkleProof256 { siblings: full_siblings })
    }
}
