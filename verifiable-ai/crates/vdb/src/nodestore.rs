use crate::Hash32;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct NodeId {
    /// 0 = leaf, 256 = root
    pub height: u16,
    /// Prefix key representation (see smt.rs helpers)
    pub key: Hash32,
}

pub trait NodeStore: Send + Sync {
    fn get(&self, id: &NodeId) -> Option<Hash32>;
    fn insert(&mut self, id: NodeId, hash: Hash32);
    fn remove(&mut self, id: &NodeId);
}

/// Simple in-memory store
#[derive(Default, Clone)]
pub struct InMemoryNodeStore {
    nodes: HashMap<NodeId, Hash32>,
}

impl InMemoryNodeStore {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    /// Only for tests / debugging
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

impl NodeStore for InMemoryNodeStore {
    fn get(&self, id: &NodeId) -> Option<Hash32> {
        self.nodes.get(id).copied()
    }

    fn insert(&mut self, id: NodeId, hash: Hash32) {
        self.nodes.insert(id, hash);
    }

    fn remove(&mut self, id: &NodeId) {
        self.nodes.remove(id);
    }
}
