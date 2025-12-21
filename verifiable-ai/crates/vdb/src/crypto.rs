//! Domain-separated cryptographic operations

use crate::Hash32;

const DOMAIN_LEAF: u8 = 0x00;
const DOMAIN_INTERNAL: u8 = 0x01;

/// Hash a key to 32 bytes (used ONLY for path bits)
pub fn hash_key(key: &[u8]) -> Hash32 {
    blake3::hash(key).into()
}

/// Hash a value to 32 bytes (payload to leaf)
pub fn hash_value(value: &[u8]) -> Hash32 {
    blake3::hash(value).into()
}

/// Leaf hash depends ONLY on value_hash (NOT key_hash)
/// leaf = H(0x00 || value_hash)
pub fn hash_leaf(value_hash: Hash32) -> Hash32 {
    let mut data = [0u8; 1 + 32];
    data[0] = DOMAIN_LEAF;
    data[1..].copy_from_slice(&value_hash);
    blake3::hash(&data).into()
}

/// Internal node hash
/// node = H(0x01 || left || right)
pub fn hash_internal(left: Hash32, right: Hash32) -> Hash32 {
    let mut data = [0u8; 1 + 32 + 32];
    data[0] = DOMAIN_INTERNAL;
    data[1..33].copy_from_slice(&left);
    data[33..].copy_from_slice(&right);
    blake3::hash(&data).into()
}

/// Canonical "empty value hash"
pub fn empty_value_hash() -> Hash32 {
    [0u8; 32]
}

/// Canonical "empty leaf hash" (proves non-existence)
pub fn empty_leaf_hash() -> Hash32 {
    hash_leaf(empty_value_hash())
}
