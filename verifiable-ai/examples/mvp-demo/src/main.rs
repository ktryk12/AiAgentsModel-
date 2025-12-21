//! MVP Demo: 30-second shock
//! 
//! Shows:
//! 1. Lab writes test result
//! 2. Patient fetches with proof
//! 3. Evil server tampers
//! 4. Verification catches tampering
//! 5. Proof of absence works

use vdb::{VerifiableKV, InMemoryStorage};

fn main() {
    println!("╔════════════════════════════════════════════════╗");
    println!("║  Verifiable AI - MVP Demo                     ║");
    println!("║  Cryptographic Proof of Data Integrity        ║");
    println!("╚════════════════════════════════════════════════╝\n");
    
    // 1. Lab writes test result
    println!("📝 Step 1: Lab writes test result");
    println!("   ─────────────────────────────────");
    
    let storage = InMemoryStorage::new();
    let mut lab_db = VerifiableKV::new(storage);
    
    let patient_id = b"patient:alice";
    let test_result = b"glucose: 95 mg/dL";
    
    let receipt = lab_db.set(patient_id, test_result).unwrap();
    
    println!("   Patient ID: {}", String::from_utf8_lossy(patient_id));
    println!("   Test Result: {}", String::from_utf8_lossy(test_result));
    println!("   State Root: {}", hex::encode(receipt.state_root));
    println!("   ✓ Result stored and signed\n");
    
    // 2. Patient fetches result
    println!("🔍 Step 2: Patient fetches result from server");
    println!("   ──────────────────────────────────────────");
    
    let result = lab_db.get(patient_id).unwrap();
    
    println!("   Received Value: {}", 
        String::from_utf8_lossy(result.value.as_ref().unwrap()));
    println!("   Proof Size: {} sibling hashes", result.proof.siblings.len());
    println!("   ✓ Received data with cryptographic proof\n");
    
    // 3. Evil server tampers with data
    println!("😈 Step 3: Evil server tampers with data");
    println!("   ───────────────────────────────────────");
    
    let tampered_value = b"glucose: 150 mg/dL";  // Changed!
    
    println!("   Original: {}", String::from_utf8_lossy(test_result));
    println!("   Tampered: {}", String::from_utf8_lossy(tampered_value));
    println!("   ⚠️  Server attempting to deceive patient\n");
    
    // 4. Patient verifies - TAMPERING DETECTED
    println!("✓ Step 4: Patient verifies data integrity");
    println!("   ────────────────────────────────────────");
    
    let is_valid_tampered = VerifiableKV::<InMemoryStorage>::verify_proof(
        &result.proof,
        patient_id,
        Some(tampered_value),
        receipt.state_root,
    );
    
    if is_valid_tampered {
        println!("   ✓ Data is valid");
    } else {
        println!("   ✗ TAMPERING DETECTED!");
        println!("   ✗ Cryptographic proof verification FAILED");
        println!("   ✗ Data has been modified after signing");
    }
    println!();
    
    // 5. Verify original data still works
    println!("🔐 Step 5: Verify original data");
    println!("   ─────────────────────────────");
    
    let is_valid_original = VerifiableKV::<InMemoryStorage>::verify_proof(
        &result.proof,
        patient_id,
        Some(test_result),
        receipt.state_root,
    );
    
    if is_valid_original {
        println!("   ✓ Original data verified successfully");
        println!("   ✓ Cryptographic integrity intact");
    }
    println!();
    
    // 6. Bonus: Proof of absence
    println!("🔍 Bonus: Proof of non-existence");
    println!("   ─────────────────────────────────");
    
    let nonexistent_key = b"patient:bob";
    let absence_result = lab_db.get(nonexistent_key).unwrap();
    
    println!("   Querying: {}", String::from_utf8_lossy(nonexistent_key));
    println!("   Result: {:?}", absence_result.value);
    
    let absence_verified = VerifiableKV::<InMemoryStorage>::verify_proof(
        &absence_result.proof,
        nonexistent_key,
        None,
        lab_db.state_root(),
    );
    
    if absence_verified {
        println!("   ✓ Proof of non-existence verified");
        println!("   ✓ Can cryptographically prove data doesn't exist");
    }
    println!();
    
    // Summary
    println!("╔════════════════════════════════════════════════╗");
    println!("║  Summary                                       ║");
    println!("╠════════════════════════════════════════════════╣");
    println!("║  ✓ Cryptographic proofs detect tampering      ║");
    println!("║  ✓ Works for both presence and absence        ║");
    println!("║  ✓ No trusted third party needed              ║");
    println!("║  ✓ Mathematics guarantees integrity           ║");
    println!("╚════════════════════════════════════════════════╝");
}
