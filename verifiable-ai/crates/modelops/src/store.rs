use crate::{ModelRecord, model_key};
use thiserror::Error;
use vdb::{Storage, VerifiableKV};

#[derive(Debug, Error)]
pub enum ModelOpsError {
    #[error("VDB error: {0}")]
    Vdb(String),
    #[error("Serialization error: {0}")]
    Ser(String),
}

pub type Result<T> = std::result::Result<T, ModelOpsError>;

pub fn put_model<S: Storage>(vdb: &mut VerifiableKV<S>, rec: &ModelRecord) -> Result<()> {
    let key = model_key(&rec.repo_id, rec.revision.as_deref());
    let val = bincode::serialize(rec).map_err(|e| ModelOpsError::Ser(e.to_string()))?;
    vdb.set(&key, &val).map_err(|e| ModelOpsError::Vdb(e.to_string()))?;
    Ok(())
}

pub fn get_model<S: Storage>(vdb: &VerifiableKV<S>, repo_id: &str, revision: Option<&str>) -> Result<Option<ModelRecord>> {
    let key = model_key(repo_id, revision);
    let read = vdb.get(&key).map_err(|e| ModelOpsError::Vdb(e.to_string()))?;
    if let Some(bytes) = read.value {
        let rec: ModelRecord = bincode::deserialize(&bytes).map_err(|e| ModelOpsError::Ser(e.to_string()))?;
        Ok(Some(rec))
    } else {
        Ok(None)
    }
}
