use crate::{ModelFile, Hash32};

/// Deterministic manifest hash:
/// - sort by rel_path (bytewise)
/// - hash bytes: "<rel_path>\n<size>\n" for each file
pub fn manifest_hash(mut files: Vec<ModelFile>) -> Hash32 {
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let mut hasher = blake3::Hasher::new();
    for f in files {
        hasher.update(f.rel_path.as_bytes());
        hasher.update(b"\n");
        hasher.update(f.size.to_string().as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_hash_deterministic() {
        let files1 = vec![
            ModelFile{ rel_path:"b.bin".into(), size: 2 },
            ModelFile{ rel_path:"a.bin".into(), size: 1 },
        ];
        let files2 = vec![
            ModelFile{ rel_path:"a.bin".into(), size: 1 },
            ModelFile{ rel_path:"b.bin".into(), size: 2 },
        ];
        assert_eq!(manifest_hash(files1), manifest_hash(files2));
    }

    #[test]
    fn test_manifest_hash_changes_on_size() {
        let files1 = vec![ModelFile{ rel_path:"a.bin".into(), size: 1 }];
        let files2 = vec![ModelFile{ rel_path:"a.bin".into(), size: 2 }];
        assert_ne!(manifest_hash(files1), manifest_hash(files2));
    }
}
