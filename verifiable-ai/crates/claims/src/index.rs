use std::collections::{HashMap, HashSet};
use crate::ClaimId;

#[derive(Default)]
pub struct SimpleIndex {
    map: HashMap<String, HashSet<ClaimId>>,
}

impl SimpleIndex {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn add(&mut self, id: ClaimId, text: &str) {
        for tok in tokenize(text) {
            self.map.entry(tok).or_default().insert(id);
        }
    }

    pub fn search(&self, query: &str) -> Vec<ClaimId> {
        let mut sets: Vec<&HashSet<ClaimId>> = Vec::new();
        for tok in tokenize(query) {
            if let Some(s) = self.map.get(&tok) {
                sets.push(s);
            }
        }
        if sets.is_empty() {
            return vec![];
        }
        // AND search: intersection
        let mut out: HashSet<ClaimId> = sets[0].clone();
        for s in sets.iter().skip(1) {
            out = out.intersection(s).copied().collect();
        }
        out.into_iter().collect()
    }
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_string())
        .collect()
}
