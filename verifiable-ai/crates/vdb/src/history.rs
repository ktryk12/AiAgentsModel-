use crate::Hash32;

#[derive(Clone, Debug)]
pub struct RootPoint {
    pub event_hash: Hash32,
    pub state_root: Hash32,
    pub timestamp: u64,
}

pub struct StateHistory {
    points: Vec<RootPoint>,
    max: usize,
}

impl StateHistory {
    pub fn new(max: usize) -> Self {
        Self { points: Vec::new(), max }
    }

    pub fn record(&mut self, p: RootPoint) {
        self.points.push(p);
        if self.points.len() > self.max {
            let overflow = self.points.len() - self.max;
            self.points.drain(0..overflow);
        }
    }

    pub fn latest_root(&self) -> Option<Hash32> {
        self.points.last().map(|p| p.state_root)
    }

    pub fn root_by_event(&self, event_hash: Hash32) -> Option<Hash32> {
        self.points.iter().rev().find(|p| p.event_hash == event_hash).map(|p| p.state_root)
    }

    pub fn root_at_or_before(&self, timestamp: u64) -> Option<Hash32> {
        self.points.iter().rev().find(|p| p.timestamp <= timestamp).map(|p| p.state_root)
    }
}
