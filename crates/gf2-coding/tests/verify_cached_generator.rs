//! Property test for cached generator matrix
//!
//! Key property: H × G^T = 0 for systematic generator G = [I_k | P]
//!
//! This verifies that the cached parity matrix P is mathematically consistent
//! with the parity-check matrix H.

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::path::PathBuf;

fn load_cache() -> EncodingCache {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    EncodingCache::from_directory(&cache_dir).expect("Cache must exist")
}

#[test]
#[ignore]
fn test_cached_parity_matrix_property() {
    let cache = load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let k = code.k();
    let m = code.m();
    let n = code.n();

    println!("\n=== Verifying Cached Parity Matrix Property ===");
    println!("Code: n={}, k={}, m={}", n, k, m);
    println!();
    println!("Property: H × G^T = 0 where G = [I_k | P]");
    println!("This verifies: H × [I_k^T; P^T] = [H_A; H_B] × [I_k^T; P^T] = H_A + H_B × P^T = 0");
    println!();

    // Get the cached parity matrix P (k × m)
    // We need to access the cache's parity matrix
    // Since we can't access it directly, we'll verify by encoding

    println!("Strategy: Verify H × c^T = 0 for unit vectors");
    println!("For each standard basis vector e_i, encode to get c_i = [e_i | p_i]");
    println!("Then check H × c_i^T = 0");
    println!();

    // We'll test a sample of basis vectors due to computational cost
    let test_indices = vec![0, 1, 2, 10, 100, 1000, 5000, 10000, 20000, 30000, 38879];

    println!("Testing {} standard basis vectors...", test_indices.len());

    let encoder = gf2_coding::ldpc::LdpcEncoder::with_cache(code.clone(), &cache);
    let mut all_pass = true;
    let mut failed_indices = Vec::new();

    for &i in &test_indices {
        // Create unit vector e_i (all zeros except position i)
        let mut message = BitVec::zeros(k);
        message.set(i, true);

        // Encode to get codeword c = [e_i | p_i]
        let codeword = encoder.encode(&message);

        // Verify H × c^T = 0
        let syndrome = code.syndrome(&codeword);
        let syndrome_weight = syndrome.count_ones();

        if syndrome_weight != 0 {
            all_pass = false;
            failed_indices.push(i);
            println!(
                "  e_{}: FAIL - syndrome weight = {}/{}",
                i, syndrome_weight, m
            );
        } else {
            print!("  e_{}: ✓", i);
            if i == test_indices[test_indices.len() - 1] {
                println!();
            } else {
                print!(" ");
            }
        }
    }

    println!();

    if all_pass {
        println!("✅ SUCCESS: All tested basis vectors satisfy H × G^T = 0");
        println!("   Cached parity matrix is mathematically consistent with H");
    } else {
        println!(
            "❌ FAILURE: {} / {} basis vectors failed",
            failed_indices.len(),
            test_indices.len()
        );
        println!("   Failed indices: {:?}", failed_indices);
        println!("   This indicates the cached parity matrix P is INCORRECT");
        panic!("Cached parity matrix does not satisfy H × G^T = 0");
    }
}

#[test]
#[ignore]
fn test_problematic_parity_bits_property() {
    let cache = load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let k = code.k();
    let _m = code.m();

    println!("\n=== Testing Property for Problematic Parity Positions ===");
    println!();

    // The problematic parity positions we identified
    let problem_parities = vec![
        17249, 17273, 23076, 23077, 23078, 23079, 23080, 23081, 23082, 23083, 23084, 23093,
    ];

    println!("Problematic parity bit positions: {:?}", problem_parities);
    println!();

    // For each parity bit p_j, we want to find which info bits contribute to it
    // In the cached parity matrix P (k × m), column j tells us which info bits contribute to p_j

    // We can test this by checking if H × [e_i | 0]^T gives us information about
    // which parity bits should be set for info bit i

    let encoder = gf2_coding::ldpc::LdpcEncoder::with_cache(code.clone(), &cache);

    println!("For each problematic parity bit, testing its relationship with info bits...");
    println!();

    // Test with a few info bits to see which ones affect these parity bits
    for &parity_idx in &problem_parities[..3] {
        // Test first 3 to save time
        println!("Parity bit p{}:", parity_idx);

        let mut contributors = Vec::new();

        // Test first 1000 info bits
        for i in 0..1000 {
            let mut message = BitVec::zeros(k);
            message.set(i, true);

            let codeword = encoder.encode(&message);
            let parity_value = codeword.get(k + parity_idx);

            if parity_value {
                contributors.push(i);
            }
        }

        println!(
            "  First 1000 info bits: {} contributors",
            contributors.len()
        );
        if contributors.len() <= 20 {
            println!("  Contributors: {:?}", contributors);
        } else {
            println!("  First 10: {:?}", &contributors[..10]);
        }

        // Now verify using H matrix
        // H[parity_idx, :] tells us which variable nodes connect to check parity_idx
        // Due to dual-diagonal structure, check parity_idx connects to:
        //   - Variable k+parity_idx (diagonal)
        //   - Variable k+parity_idx-1 (sub-diagonal, if parity_idx > 0)
        //   - Plus connections from information part (A matrix)

        println!();
    }
}

#[test]
#[ignore]
fn test_full_generator_orthogonality() {
    let cache = load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let k = code.k();
    let m = code.m();

    println!("\n=== Full Generator Matrix Orthogonality Test ===");
    println!("Testing H × G^T = 0 for ALL {} standard basis vectors", k);
    println!("This will take a while...");
    println!();

    let encoder = gf2_coding::ldpc::LdpcEncoder::with_cache(code.clone(), &cache);

    let mut fail_count = 0;
    let mut fail_indices = Vec::new();

    for i in 0..k {
        let mut message = BitVec::zeros(k);
        message.set(i, true);

        let codeword = encoder.encode(&message);
        let syndrome = code.syndrome(&codeword);
        let syndrome_weight = syndrome.count_ones();

        if syndrome_weight != 0 {
            fail_count += 1;
            fail_indices.push(i);

            if fail_count <= 10 {
                println!(
                    "  Info bit {}: syndrome weight = {}/{}",
                    i, syndrome_weight, m
                );
            }
        }

        if (i + 1) % 5000 == 0 {
            println!(
                "  Tested {} / {} info bits... ({} failures so far)",
                i + 1,
                k,
                fail_count
            );
        }
    }

    println!();
    println!("=== Results ===");
    println!("Total failures: {} / {}", fail_count, k);
    println!(
        "Success rate: {:.4}%",
        100.0 * (k - fail_count) as f64 / k as f64
    );

    if fail_count > 0 {
        println!();
        println!("Failed info bit indices:");
        if fail_indices.len() <= 50 {
            println!("{:?}", fail_indices);
        } else {
            println!("First 50: {:?}", &fail_indices[..50]);
            println!("... and {} more", fail_indices.len() - 50);
        }

        panic!("Cached parity matrix has {} errors", fail_count);
    } else {
        println!("✅ PERFECT: All basis vectors satisfy H × G^T = 0");
    }
}
