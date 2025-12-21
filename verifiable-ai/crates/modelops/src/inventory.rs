use crate::ModelRecord;
use vdb::{Storage, VerifiableKV};
use serde::{Serialize, Deserialize};

const KEY_INDEX: &[u8] = b"model:index";

fn load_index<S: Storage>(vdb: &mut VerifiableKV<S>) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    Ok(match vdb.get(KEY_INDEX)?.value {
        Some(bytes) => serde_json::from_slice(&bytes)?,
        None => vec![],
    })
}

fn save_index<S: Storage>(vdb: &mut VerifiableKV<S>, keys: &[Vec<u8>]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(keys)?;
    vdb.set(KEY_INDEX, &bytes)?;
    Ok(())
}

pub fn add_model_to_index<S: Storage>(
    vdb: &mut VerifiableKV<S>,
    key: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // Expected to be called under orchestrator write-lock => no lost updates.
    let mut keys = load_index(vdb)?;
    if !keys.iter().any(|k| k.as_slice() == key) {
        keys.push(key.to_vec());
        save_index(vdb, &keys)?;
    }
    Ok(())
}

pub fn list_models<S: Storage>(
    vdb: &mut VerifiableKV<S>,
    repair: bool,
) -> Result<Vec<ModelRecord>, Box<dyn std::error::Error>> {
    let keys = load_index(vdb)?;
    let mut out = Vec::new();
    let mut valid_keys = Vec::new();

    for k in keys {
        let Some(bytes) = vdb.get(&k)?.value else { continue };
        let Ok(rec) = serde_json::from_slice::<ModelRecord>(&bytes) else { continue };

        // lazy validation: snapshot_dir must exist
        if std::path::Path::new(&rec.snapshot_dir).exists() {
            valid_keys.push(k.clone());
            out.push(rec);
        }
    }

    if repair {
        save_index(vdb, &valid_keys)?;
    }

    Ok(out)
}
