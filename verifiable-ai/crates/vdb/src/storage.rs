//! Storage trait and implementations

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::{fs, io::{self, Write}, path::PathBuf, time::Duration};
use std::fs::File;
use serde::{Serialize, Deserialize};

pub trait Storage: Send + Sync + Clone {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn set(&mut self, key: &[u8], value: &[u8]);
    fn delete(&mut self, key: &[u8]);
}

/// In-memory storage (for testing and demos)
#[derive(Clone, Default)]
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for InMemoryStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.read().unwrap().get(key).cloned()
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.data.write().unwrap().insert(key.to_vec(), value.to_vec());
    }

    fn delete(&mut self, key: &[u8]) {
        self.data.write().unwrap().remove(key);
    }
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    items: Vec<(Vec<u8>, Vec<u8>)>, // Simple list for JSON serialization
}

#[derive(Clone)]
pub struct FileBackedStorage {
    path: PathBuf,
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl FileBackedStorage {
    pub fn new(path: PathBuf) -> Result<Self, io::Error> {
        let data = if path.exists() {
            let bytes = fs::read(&path)?;
            let persisted: Persisted = serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            persisted.items.into_iter().collect()
        } else {
            HashMap::new()
        };

        Ok(Self {
            path,
            data: Arc::new(RwLock::new(data)),
        })
    }

    fn flush_atomic_bytes(&self, bytes: &[u8]) -> Result<(), io::Error> {
        let tmp = self.path.with_extension("tmp");

        // 1) write temp + fsync
        {
            let mut f = File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?; // CRITICAL durability
        }

        // 2) replace target
        #[cfg(unix)]
        {
            fs::rename(&tmp, &self.path)?;
            return Ok(());
        }

        #[cfg(windows)]
        {
            // Windows: remove + retry (locks happen)
            for attempt in 0..6 {
                if self.path.exists() {
                    match fs::remove_file(&self.path) {
                        Ok(_) => break,
                        Err(e) if attempt == 5 => return Err(e),
                        Err(_) => std::thread::sleep(Duration::from_millis(15 * (attempt + 1))),
                    }
                } else {
                    break;
                }
            }
            fs::rename(&tmp, &self.path)?;
            return Ok(());
        }
    }

    fn flush_atomic(&self) -> Result<(), io::Error> {
        let items: Vec<(Vec<u8>, Vec<u8>)> = {
            let data = self.data.read().unwrap();
            data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        let json = serde_json::to_vec(&Persisted { items })
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.flush_atomic_bytes(&json)
    }
}

impl Storage for FileBackedStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.read().unwrap().get(key).cloned()
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.data.write().unwrap().insert(key.to_vec(), value.to_vec());
        let _ = self.flush_atomic(); // In a simple MVP, immediate flush is fine
    }

    fn delete(&mut self, key: &[u8]) {
        self.data.write().unwrap().remove(key);
        let _ = self.flush_atomic();
    }
}
