//! Storage trait and implementations

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait Storage: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>>;
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
    fn delete(&mut self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
}

/// In-memory storage (for testing and demos)
#[derive(Clone)]
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for InMemoryStorage {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }
    
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }
    
    fn delete(&mut self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }
}
