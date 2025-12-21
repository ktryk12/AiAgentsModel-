use serde::{Deserialize, Serialize};
use vdb::Hash32;

pub type ClaimId = Hash32;
pub type AgentId = Hash32;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,                 // deterministic hash id
    pub kind: ClaimKind,
    pub statement: String,

    // Evidence as references (hash-addressed)
    pub evidence: Vec<EvidenceRef>,

    // Epistemics
    pub assumptions: Vec<String>,
    pub falsifiers: Vec<String>,
    pub confidence: f32,             // 0.0..1.0

    // Provenance
    pub issuer: AgentId,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClaimKind {
    Fact,
    Policy,
    Plan,
    Prediction { testable_after: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EvidenceRef {
    DocumentUrl { url: String, content_hash: Hash32 },
    PriorClaim { claim_id: ClaimId },
    EventHash { event_hash: Hash32 },
}

impl Claim {
    /// Create claim and compute deterministic id from canonical bytes (bincode)
    pub fn new(
        kind: ClaimKind,
        statement: impl Into<String>,
        evidence: Vec<EvidenceRef>,
        assumptions: Vec<String>,
        falsifiers: Vec<String>,
        confidence: f32,
        issuer: AgentId,
        timestamp: u64,
    ) -> Self {
        let mut c = Self {
            id: [0u8; 32],
            kind,
            statement: statement.into(),
            evidence,
            assumptions,
            falsifiers,
            confidence,
            issuer,
            timestamp,
        };

        c.id = c.compute_id();
        c
    }

    pub fn compute_id(&self) -> ClaimId {
        // IMPORTANT: id must not include itself in hash
        // so hash a copy with id=0
        let mut tmp = self.clone();
        tmp.id = [0u8; 32];
        let bytes = bincode::serialize(&tmp).expect("Claim serialization must work");
        blake3::hash(&bytes).into()
    }
}
