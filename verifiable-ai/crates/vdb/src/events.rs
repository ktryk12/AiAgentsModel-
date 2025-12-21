use crate::Hash32;
use serde::{Deserialize, Serialize};
use ed25519_dalek::{Signature, VerifyingKey, Verifier as _};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    Set,
    Delete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteEvent {
    pub operation: Operation,
    pub key: Vec<u8>,
    pub value_hash: Hash32,
    pub prev_event_hash: Hash32,
    pub state_root: Hash32,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchWriteEvent {
    pub batch_hash: Hash32,
    pub op_count: u32,
    pub prev_event_hash: Hash32,
    pub state_root: Hash32,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Single(WriteEvent),
    Batch(BatchWriteEvent),
}

impl Event {
    pub fn prev_event_hash(&self) -> Hash32 {
        match self {
            Event::Single(e) => e.prev_event_hash,
            Event::Batch(e) => e.prev_event_hash,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub event_hash: Hash32,
    pub event: Event,
    pub signature: Vec<u8>, // signature over event_bytes
}

pub struct EventLog {
    pub entries: Vec<LogEntry>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn append(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    pub fn latest_hash(&self) -> Hash32 {
        self.entries.last().map(|e| e.event_hash).unwrap_or([0u8; 32])
    }

    pub fn verify_chain_and_sigs(&self, vk: &VerifyingKey) -> bool {
        let mut prev = [0u8; 32];

        for e in &self.entries {
            // chain link
            if e.event.prev_event_hash() != prev {
                return false;
            }

            // hash correctness
            let event_bytes = match bincode::serialize(&e.event) {
                Ok(b) => b,
                Err(_) => return false,
            };
            let computed: Hash32 = blake3::hash(&event_bytes).into();
            if computed != e.event_hash {
                return false;
            }

            // signature correctness
            let sig_slice = e.signature.as_slice();
            let sig = match Signature::from_slice(sig_slice) {
                Ok(s) => s,
                Err(_) => return false,
            };
            if vk.verify(&event_bytes, &sig).is_err() {
                return false;
            }

            prev = e.event_hash;
        }

        true
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}
