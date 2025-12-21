use vdb::{Storage, VerifiableKV};
use serde::{Serialize, Deserialize};

const KEY_ACTIVE: &[u8] = b"model:active";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveModel {
    pub repo_id: String,
    pub revision: String,
    pub manifest_hash: String,
    pub snapshot_dir: String,
    pub activated_at: u64,
    pub last_used_at: u64,
}

pub fn set_active_model<S: Storage>(
    vdb: &mut VerifiableKV<S>,
    repo_id: &str,
    revision: Option<&str>,
) -> Result<ActiveModel, Box<dyn std::error::Error>> {
    let key = crate::model_key(repo_id, revision);
    let rec_bytes = vdb.get(&key)?.value.ok_or("model not found")?;
    let rec: crate::ModelRecord = bincode::deserialize(&rec_bytes)?;

    let ts = now();
    let active = ActiveModel {
        repo_id: repo_id.to_string(),
        revision: revision.unwrap_or("").to_string(),
        manifest_hash: hex::encode(rec.manifest_hash),
        snapshot_dir: rec.snapshot_dir.clone(),
        activated_at: ts,
        last_used_at: ts,
    };

    vdb.set(KEY_ACTIVE, &serde_json::to_vec(&active)?)?;
    Ok(active)
}

pub fn get_active_model<S: Storage>(
    vdb: &mut VerifiableKV<S>,
) -> Result<Option<ActiveModel>, Box<dyn std::error::Error>> {
    Ok(match vdb.get(KEY_ACTIVE)?.value {
        Some(bytes) => Some(serde_json::from_slice(&bytes)?),
        None => None,
    })
}

pub fn record_model_use<S: Storage>(
    vdb: &mut VerifiableKV<S>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(mut a) = get_active_model(vdb)? {
        a.last_used_at = now();
        vdb.set(KEY_ACTIVE, &serde_json::to_vec(&a)?)?;
    }
    Ok(())
}

fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
