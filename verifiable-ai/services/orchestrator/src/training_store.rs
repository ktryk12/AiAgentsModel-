use uuid::Uuid;
use crate::types_training::{DatasetRecord, KEY_DATASET_INDEX};

pub fn dataset_key(id: Uuid) -> Vec<u8> {
    format!("dataset:{id}").into_bytes()
}

pub fn add_to_dataset_index<S: vdb::Storage>(
    vdb: &mut vdb::VerifiableKV<S>,
    key: &[u8],
) -> anyhow::Result<()> {
    let cur = vdb.get(KEY_DATASET_INDEX)?;
    let mut keys: Vec<Vec<u8>> = match cur.value {
        Some(bytes) => serde_json::from_slice(&bytes)?,
        None => vec![],
    };

    if !keys.iter().any(|k| k.as_slice() == key) {
        keys.push(key.to_vec());
        let bytes = serde_json::to_vec(&keys)?;
        vdb.set(KEY_DATASET_INDEX, &bytes)?;
    }
    Ok(())
}

pub fn list_datasets<S: vdb::Storage>(
    vdb: &mut vdb::VerifiableKV<S>,
) -> anyhow::Result<Vec<DatasetRecord>> {
    let idx = vdb.get(KEY_DATASET_INDEX)?;
    let keys: Vec<Vec<u8>> = match idx.value {
        Some(bytes) => serde_json::from_slice(&bytes)?,
        None => vec![],
    };

    let mut out = Vec::new();
    for k in keys {
        let r = vdb.get(&k)?;
        if let Some(bytes) = r.value {
            if let Ok(rec) = serde_json::from_slice::<DatasetRecord>(&bytes) {
                out.push(rec);
            }
        }
    }
    Ok(out)
}
