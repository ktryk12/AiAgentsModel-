use claims::{Claim, ClaimKind, EvidenceRef};
use vdb::{InMemoryStorage, InMemoryNodeStore, VerifiableKV};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn main() {
    println!("=== Claim Demo Phase 2.1: Scalable Knowledge ===\n");

    let storage = InMemoryStorage::new();
    let db = VerifiableKV::new(storage);
    let mut store = claims::ClaimStore::new(db);

    // 1. Batch Ingestion
    println!("--- Batch Ingestion (1000 claims) ---");
    let mut batch = Vec::new();
    let issuer = [7u8; 32];
    
    for i in 0..1000 {
        batch.push(Claim::new(
            ClaimKind::Fact,
            format!("Fact #{} about the world.", i),
            vec![],
            vec![],
            vec![],
            0.9,
            issuer,
            now(),
        ));
    }
    
    // Remember the middle one for testing
    let target_claim = batch[500].clone();
    println!("Target Claim ID: {}", hex::encode(target_claim.id));

    let receipt = store.put_claims_batch(&batch).unwrap();
    println!("Batch committed!");
    println!("  State Root: {}", hex::encode(receipt.state_root));
    println!("  Batch Hash: {}", hex::encode(receipt.batch_hash));
    println!("  Op Count:   {}", receipt.op_count);

    // 2. Retrieval with Compressed Proof
    println!("\n--- Compressed Retrieval ---");
    let (fetched_opt, root, compressed_proof) = store.get_claim_with_compressed_proof(target_claim.id).unwrap();
    
    let fetched = fetched_opt.expect("Claim should exist");
    println!("Fetched: \"{}\"", fetched.statement);
    println!("Proof depth: {}", compressed_proof.depth);
    println!("Proof siblings: {} (vs 256 uncompressed)", compressed_proof.siblings.len());
    println!("Proof bitmap size: {} bytes", compressed_proof.bitmap.len());

    // 3. Client-side Verification
    println!("\n--- Verification ---");
    let ok = store.verify_claim_read_compressed(
        target_claim.id,
        Some(&fetched),
        root,
        &compressed_proof
    ).unwrap();
    
    println!("Verification: {}", if ok { "OK ✅" } else { "FAIL ❌" });

    // 4. Tamper Resistance
    println!("\n--- Tamper Resistance ---");
    let mut mut_claim = fetched.clone();
    mut_claim.statement = "Lies and fabrication!".to_string();
    
    let ok_tamper = store.verify_claim_read_compressed(
        target_claim.id,
        Some(&mut_claim), // client tries to verify modified claim against valid proof
        root,
        &compressed_proof
    ).unwrap();
    
    println!("Tampered Verification: {}", if ok_tamper { "OK (BAD!) ❌" } else { "FAIL (GOOD) ✅" });
}
