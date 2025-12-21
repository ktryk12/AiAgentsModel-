use crate::{Claim, ClaimId, SimpleIndex};
use vdb::{Storage, NodeStore, VerifiableKV, WriteReceipt, ReadResult, Hash32, BatchReceipt, CompressedProof, MerkleProof256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClaimsError {
    #[error("VDB error: {0}")]
    Vdb(String),

    #[error("Serialization error: {0}")]
    Ser(String),
}

pub type Result<T> = std::result::Result<T, ClaimsError>;

pub struct ClaimStore<S: Storage, N: NodeStore> {
    vdb: VerifiableKV<S, N>,
    index: SimpleIndex,
}

impl<S: Storage, N: NodeStore> ClaimStore<S, N> {
    pub fn new(vdb: VerifiableKV<S, N>) -> Self {
        Self { vdb, index: SimpleIndex::new() }
    }

    pub fn vdb(&self) -> &VerifiableKV<S, N> {
        &self.vdb
    }

    pub fn vdb_mut(&mut self) -> &mut VerifiableKV<S, N> {
        &mut self.vdb
    }

    fn key_for(id: ClaimId) -> Vec<u8> {
        // claim:<hex>
        let mut key = b"claim:".to_vec();
        key.extend_from_slice(hex::encode(id).as_bytes());
        key
    }

    pub fn put_claim(&mut self, claim: &Claim) -> Result<WriteReceipt> {
        let key = Self::key_for(claim.id);
        let value = bincode::serialize(claim).map_err(|e| ClaimsError::Ser(e.to_string()))?;

        let receipt = self.vdb
            .set(&key, &value)
            .map_err(|e| ClaimsError::Vdb(e.to_string()))?;

        self.index.add(claim.id, &claim.statement);

        Ok(receipt)
    }

    pub fn get_claim_with_proof(&self, id: ClaimId) -> Result<(Option<Claim>, ReadResult)> {
        let key = Self::key_for(id);

        let read = self.vdb
            .get(&key)
            .map_err(|e| ClaimsError::Vdb(e.to_string()))?;

        let claim = if let Some(bytes) = &read.value {
            Some(bincode::deserialize::<Claim>(bytes).map_err(|e| ClaimsError::Ser(e.to_string()))?)
        } else {
            None
        };

        Ok((claim, read))
    }

    pub fn verify_claim_read(read: &ReadResult, claim_id: ClaimId, claim: Option<&Claim>) -> bool {
        let key = Self::key_for(claim_id);

        let value_bytes_opt: Option<Vec<u8>> = claim.map(|c| bincode::serialize(c).ok()).flatten();
        let value_slice_opt = value_bytes_opt.as_ref().map(|v| v.as_slice());

        // Verify proof against the read.state_root.
        // Note: proof ties to (key, value_hash) in your VDB design.
        vdb::VerifiableKV::<S, N>::verify_proof(
            &read.proof,
            &key,
            value_slice_opt,
            read.state_root,
        )
    }

    pub fn search(&self, query: &str) -> Vec<ClaimId> {
        self.index.search(query)
    }

    pub fn checkpoint(&self) -> (Hash32, Hash32) {
        let cp = self.vdb.checkpoint();
        (cp.state_root, cp.latest_event_hash)
    }

    pub fn put_claims_batch(&mut self, claims: &[Claim]) -> Result<BatchReceipt> {
        if claims.is_empty() {
            return Err(ClaimsError::Ser("empty batch".into()));
        }

        // Prepare owned buffers so we can take stable slices
        let mut keys: Vec<Vec<u8>> = Vec::with_capacity(claims.len());
        let mut vals: Vec<Vec<u8>> = Vec::with_capacity(claims.len());

        for c in claims {
            let key = Self::key_for(c.id);
            let val = bincode::serialize(c).map_err(|e| ClaimsError::Ser(e.to_string()))?;
            keys.push(key);
            vals.push(val);
        }

        // Build slice view
        let ops: Vec<(&[u8], &[u8])> = keys
            .iter()
            .zip(vals.iter())
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        // Single batch commit
        let receipt = self.vdb
            .batch_set(&ops)
            .map_err(|e| ClaimsError::Vdb(e.to_string()))?;

        // Update index after commit (or before; depends on your preferences)
        for c in claims {
            self.index.add(c.id, &c.statement);
        }

        Ok(receipt)
    }

    pub fn get_claim_with_compressed_proof(&self, id: ClaimId) -> Result<(Option<Claim>, Hash32, CompressedProof)> {
        let (claim_opt, read) = self.get_claim_with_proof(id)?;

        // Compress proof for transport
        let cp = self.vdb.compress_proof(&read.proof);

        Ok((claim_opt, read.state_root, cp))
    }

    pub fn verify_claim_read_compressed(
        &self,
        claim_id: ClaimId,
        claim: Option<&Claim>,
        state_root: Hash32,
        proof: &CompressedProof,
    ) -> Result<bool> {
        let full: MerkleProof256 = self.vdb
            .decompress_proof(proof)
            .map_err(|e| ClaimsError::Vdb(e.to_string()))?;

        // Recreate value bytes (must match stored bytes)
        let key = Self::key_for(claim_id);
        let value_bytes_opt = claim.and_then(|c| bincode::serialize(c).ok());

        Ok(VerifiableKV::<S, N>::verify_proof(
            &full,
            &key,
            value_bytes_opt.as_ref().map(|v| v.as_slice()),
            state_root,
        ))
    }
}
