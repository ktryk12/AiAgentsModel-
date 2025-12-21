use crate::{Hash32, MerkleProof256};
use crate::crypto;
use crate::nodestore::{NodeId, NodeStore};

const DEPTH: usize = 256;

pub struct SparseMerkleTree<N: NodeStore> {
    root: Hash32,
    default_hashes: Vec<Hash32>,
    store: N,
}

impl<N: NodeStore> SparseMerkleTree<N> {
    pub fn new(store: N) -> Self {
        let default_hashes = compute_default_hashes();
        let root = default_hashes[DEPTH];
        Self { root, default_hashes, store }
    }

    pub fn root(&self) -> Hash32 {
        self.root
    }

    pub fn default_hashes(&self) -> &[Hash32] {
        &self.default_hashes
    }

    pub fn update(&mut self, key_hash: Hash32, value_hash: Hash32) {
        let mut current = crypto::hash_leaf(value_hash);

        // leaf
        let leaf_id = NodeId { height: 0, key: key_hash };
        let default_leaf = self.default_hashes[0];

        if current == default_leaf {
            self.store.remove(&leaf_id);
        } else {
            self.store.insert(leaf_id, current);
        }

        // internal nodes bottom-up
        for h in 0..DEPTH {
            let is_right = bit_at_lsb(&key_hash, h);
            let sibling_key = flip_bit_lsb(key_hash, h);
            let sibling_id = NodeId {
                height: h as u16,
                key: prefix_key(sibling_key, h),
            };
            let sibling_hash = self.get_node_or_default(&sibling_id);

            let parent_id = NodeId {
                height: (h + 1) as u16,
                key: prefix_key(key_hash, h + 1),
            };

            let parent_hash = if is_right {
                crypto::hash_internal(sibling_hash, current)
            } else {
                crypto::hash_internal(current, sibling_hash)
            };

            let default_parent = self.default_hashes[h + 1];
            if parent_hash == default_parent {
                self.store.remove(&parent_id);
            } else {
                self.store.insert(parent_id, parent_hash);
            }

            current = parent_hash;
        }

        self.root = current;
    }

    pub fn prove(&self, key_hash: Hash32) -> MerkleProof256 {
        let mut proof = MerkleProof256::new();
        for h in 0..DEPTH {
            let sibling_key = flip_bit_lsb(key_hash, h);
            let sibling_id = NodeId {
                height: h as u16,
                key: prefix_key(sibling_key, h),
            };
            proof.siblings.push(self.get_node_or_default(&sibling_id));
        }
        proof
    }

    pub fn verify_proof(
        proof: &MerkleProof256,
        key_hash: Hash32,
        value_hash: Hash32,
        state_root: Hash32,
    ) -> bool {
        if proof.siblings.len() != DEPTH {
            return false;
        }
        let mut current = crypto::hash_leaf(value_hash);
        for h in 0..DEPTH {
            let is_right = bit_at_lsb(&key_hash, h);
            let sibling = proof.siblings[h];
            current = if is_right {
                crypto::hash_internal(sibling, current)
            } else {
                crypto::hash_internal(current, sibling)
            };
        }
        current == state_root
    }

    fn get_node_or_default(&self, id: &NodeId) -> Hash32 {
        self.store.get(id).unwrap_or(self.default_hashes[id.height as usize])
    }
}

// --- helpers (same as before) ---

fn compute_default_hashes() -> Vec<Hash32> {
    let mut defaults = Vec::with_capacity(DEPTH + 1);
    defaults.push(crypto::empty_leaf_hash());
    for h in 0..DEPTH {
        let prev = defaults[h];
        defaults.push(crypto::hash_internal(prev, prev));
    }
    defaults
}

fn bit_at_lsb(key: &Hash32, h: usize) -> bool {
    let byte_index = 31 - (h / 8);
    let bit_index = h % 8;
    ((key[byte_index] >> bit_index) & 1) == 1
}

fn flip_bit_lsb(mut key: Hash32, h: usize) -> Hash32 {
    let byte_index = 31 - (h / 8);
    let bit_index = h % 8;
    key[byte_index] ^= 1 << bit_index;
    key
}

fn prefix_key(mut key: Hash32, h: usize) -> Hash32 {
    let full_bytes = h / 8;
    for i in 0..full_bytes {
        key[31 - i] = 0;
    }
    let rem_bits = h % 8;
    if rem_bits != 0 {
        let idx = 31 - full_bytes;
        let mask = 0xFFu8 << rem_bits;
        key[idx] &= mask;
    }
    key
}
