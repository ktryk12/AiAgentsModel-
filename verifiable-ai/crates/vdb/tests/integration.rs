use vdb::{VerifiableKV, InMemoryStorage, NodeStore};
use rand::Rng;

#[test]
fn test_basic_set_get() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let key = b"test_key";
    let value = b"test_value";
    
    let receipt = db.set(key, value).unwrap();
    let result = db.get(key).unwrap();
    
    assert_eq!(result.value, Some(value.to_vec()));
    assert_eq!(result.state_root, receipt.state_root);
}

#[test]
fn test_proof_verification() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let key = b"verify_key";
    let value = b"verify_value";
    
    db.set(key, value).unwrap();
    let result = db.get(key).unwrap();
    
    // Verify correct proof
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &result.proof,
        key,
        Some(value),
        result.state_root,
    ));
}

#[test]
fn test_tamper_detection() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let key = b"tamper_key";
    let value = b"original_value";
    
    db.set(key, value).unwrap();
    let result = db.get(key).unwrap();
    
    // Tamper with value
    let tampered = b"tampered_value";
    
    // Verification should fail
    assert!(!VerifiableKV::<InMemoryStorage>::verify_proof(
        &result.proof,
        key,
        Some(tampered),
        result.state_root,
    ));
}

#[test]
fn test_proof_of_absence() {
    let storage = InMemoryStorage::new();
    let db = VerifiableKV::new(storage);
    
    let key = b"nonexistent_key";
    let result = db.get(key).unwrap();
    
    assert_eq!(result.value, None);
    
    // Verify proof of non-inclusion
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &result.proof,
        key,
        None,
        result.state_root,
    ));
}

#[test]
fn test_delete() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let key = b"delete_key";
    let value = b"delete_value";
    
    db.set(key, value).unwrap();
    db.delete(key).unwrap();
    
    let result = db.get(key).unwrap();
    assert_eq!(result.value, None);
}

#[test]
fn test_state_root_changes() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let root1 = db.state_root();
    
    db.set(b"key1", b"value1").unwrap();
    let root2 = db.state_root();
    
    db.set(b"key2", b"value2").unwrap();
    let root3 = db.state_root();
    
    // All roots should be different
    assert_ne!(root1, root2);
    assert_ne!(root2, root3);
    assert_ne!(root1, root3);
}

#[test]
fn test_randomized_operations() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    let mut rng = rand::thread_rng();
    
    for i in 0..1000 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", rng.gen::<u64>());
        
        db.set(key.as_bytes(), value.as_bytes()).unwrap();
        let result = db.get(key.as_bytes()).unwrap();
        
        assert_eq!(result.value, Some(value.as_bytes().to_vec()));
        
        // Verify proof
        assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
            &result.proof,
            key.as_bytes(),
            Some(value.as_bytes()),
            result.state_root,
        ));
    }
}

#[test]
fn test_concurrent_proofs() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    // Set multiple values
    db.set(b"key1", b"value1").unwrap();
    db.set(b"key2", b"value2").unwrap();
    db.set(b"key3", b"value3").unwrap();
    
    let final_root = db.state_root();
    
    // All proofs should verify against final root
    let result1 = db.get(b"key1").unwrap();
    let result2 = db.get(b"key2").unwrap();
    let result3 = db.get(b"key3").unwrap();
    
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &result1.proof,
        b"key1",
        Some(b"value1"),
        final_root,
    ));
    
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &result2.proof,
        b"key2",
        Some(b"value2"),
        final_root,
    ));
    
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &result3.proof,
        b"key3",
        Some(b"value3"),
        final_root,
    ));
}

#[test]
fn test_event_log_verification() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);

    db.set(b"k1", b"v1").unwrap();
    db.set(b"k2", b"v2").unwrap();
    db.delete(b"k1").unwrap();

    let vk = db.verifying_key();
    assert!(db.verify_event_log(&vk));
}

#[test]
fn test_event_log_tamper_signature_fails() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);

    db.set(b"k1", b"v1").unwrap();
    let vk = db.verifying_key();

    // Tamper signature
    db.tamper_last_signature_for_test();

    assert!(!db.verify_event_log(&vk));
}

// Phase 1.2 Tests

#[test]
fn test_batch_set() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    let ops = vec![
        (b"batch_k1".as_slice(), b"batch_v1".as_slice()),
        (b"batch_k2".as_slice(), b"batch_v2".as_slice()),
        (b"batch_k3".as_slice(), b"batch_v3".as_slice()),
    ];
    
    let receipt = db.batch_set(&ops).unwrap();
    
    assert_eq!(receipt.op_count, 3);
    
    // Verify each key
    for (k, v) in ops {
        let result = db.get(k).unwrap();
        assert_eq!(result.value, Some(v.to_vec()));
        assert_eq!(result.state_root, receipt.state_root);
        
        assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
            &result.proof,
            k,
            Some(v),
            receipt.state_root
        ));
    }
}

#[test]
fn test_root_history_by_event() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);

    let r1 = db.set(b"a", b"1").unwrap();
    let p1 = db.get(b"a").unwrap(); // proof against current root at that time

    let _r2 = db.set(b"a", b"2").unwrap();

    // verify p1 against root at event r1.event_hash
    let root1 = db.history_root_by_event_for_test(r1.event_hash).unwrap();
    assert!(VerifiableKV::<InMemoryStorage>::verify_proof(
        &p1.proof,
        b"a",
        Some(b"1"),
        root1,
    ));

    // should fail against newer root (since value changed)
    assert!(!VerifiableKV::<InMemoryStorage>::verify_proof(
        &p1.proof,
        b"a",
        Some(b"1"),
        db.state_root(),
    ));
}

#[test]
fn test_proof_compression_roundtrip() {
    let storage = InMemoryStorage::new();
    let mut db = VerifiableKV::new(storage);
    
    // Create sparse data
    db.set(b"comp_k1", b"v").unwrap();
    let result = db.get(b"comp_k1").unwrap();
    
    let compressed = db.compress_proof(&result.proof);
    
    // Should be significantly smaller than 256 * 32
    // Minimal bitmap (32 bytes) + at least 1-2 siblings
    assert!(compressed.siblings.len() < 256);
    
    let decompressed = db.decompress_proof(&compressed).unwrap();
    assert_eq!(decompressed, result.proof);
}
