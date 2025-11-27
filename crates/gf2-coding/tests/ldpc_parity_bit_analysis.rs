//! LDPC Parity Bit Analysis
//!
//! Detailed analysis of parity bit generation to identify where divergence occurs.
//! Checks each parity bit individually against test vectors to pinpoint the exact
//! location of the encoding error.

mod test_vectors;

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use std::path::PathBuf;
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

fn try_load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

fn create_encoder(code: LdpcCode, cache: Option<&EncodingCache>) -> LdpcEncoder {
    match cache {
        Some(c) => LdpcEncoder::with_cache(code, c),
        None => {
            eprintln!("Creating encoder without cache (this may take time)...");
            LdpcEncoder::new(code)
        }
    }
}

#[test]
#[ignore]
fn analyze_parity_bits_one_by_one() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let cache = try_load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = create_encoder(code.clone(), cache.as_ref());

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    // Analyze first block only for detailed output
    let input_block = &tp05.frame(0)[0];
    let expected_output = &tp06.frame(0)[0];
    let encoded = encoder.encode(&input_block.data);

    let k = code.k();
    let n = code.n();
    let m = n - k; // Number of parity bits

    println!("\n=== DVB-T2 LDPC Parity Bit Analysis ===");
    println!("Code: n={}, k={}, m={}", n, k, m);
    println!("Analyzing Block 1 of Frame 1\n");

    // Check systematic bits
    let mut systematic_matches = 0;
    for i in 0..k {
        if encoded.get(i) == expected_output.data.get(i) {
            systematic_matches += 1;
        }
    }
    println!("Systematic bits (0..{}): {}/{} match", k, systematic_matches, k);
    
    if systematic_matches != k {
        println!("WARNING: Systematic bits don't match! This shouldn't happen.");
        println!("First 10 mismatches:");
        let mut count = 0;
        for i in 0..k {
            if encoded.get(i) != expected_output.data.get(i) && count < 10 {
                println!("  Bit {}: got {}, expected {}", 
                    i, 
                    encoded.get(i) as u8, 
                    expected_output.data.get(i) as u8
                );
                count += 1;
            }
        }
    }

    println!("\n=== Parity Bits Analysis ===");
    println!("Position range: {}..{}\n", k, n);

    // Track statistics
    let mut correct_count = 0;
    let mut incorrect_count = 0;
    let mut first_error_idx: Option<usize> = None;
    let mut last_correct_idx: Option<usize> = None;
    let mut error_runs: Vec<(usize, usize)> = Vec::new(); // (start, length)
    let mut current_error_start: Option<usize> = None;

    // Check each parity bit
    for p in 0..m {
        let bit_idx = k + p;
        let got = encoded.get(bit_idx);
        let expected = expected_output.data.get(bit_idx);
        let matches = got == expected;

        if matches {
            correct_count += 1;
            last_correct_idx = Some(p);
            
            // End of error run?
            if let Some(start) = current_error_start {
                error_runs.push((start, p - start));
                current_error_start = None;
            }
        } else {
            incorrect_count += 1;
            
            if first_error_idx.is_none() {
                first_error_idx = Some(p);
            }
            
            // Start of error run?
            if current_error_start.is_none() {
                current_error_start = Some(p);
            }
        }
    }

    // Close final error run if needed
    if let Some(start) = current_error_start {
        error_runs.push((start, m - start));
    }

    println!("Parity bits: {}/{} correct ({:.1}%)", 
        correct_count, m, 100.0 * correct_count as f64 / m as f64);
    println!("             {}/{} incorrect ({:.1}%)", 
        incorrect_count, m, 100.0 * incorrect_count as f64 / m as f64);

    if let Some(idx) = first_error_idx {
        println!("\nFirst error at parity bit p{} (absolute position {})", idx, k + idx);
    } else {
        println!("\nNo errors found!");
        return;
    }

    if let Some(idx) = last_correct_idx {
        println!("Last correct bit at parity bit p{} (absolute position {})", idx, k + idx);
    }

    // Show first 20 parity bits with status
    println!("\n=== First 20 Parity Bits ===");
    for p in 0..20.min(m) {
        let bit_idx = k + p;
        let got = encoded.get(bit_idx);
        let expected = expected_output.data.get(bit_idx);
        let status = if got == expected { "✓" } else { "✗" };
        
        println!("p{:2} (pos {:5}): got {}, expected {} {}", 
            p, bit_idx, got as u8, expected as u8, status);
    }

    // Show parity bits around first error
    if let Some(first_err) = first_error_idx {
        println!("\n=== Around First Error (p{}) ===", first_err);
        let start = first_err.saturating_sub(5);
        let end = (first_err + 15).min(m);
        
        for p in start..end {
            let bit_idx = k + p;
            let got = encoded.get(bit_idx);
            let expected = expected_output.data.get(bit_idx);
            let status = if got == expected { "✓" } else { "✗" };
            let marker = if p == first_err { " <-- FIRST ERROR" } else { "" };
            
            println!("p{:5} (pos {:5}): got {}, expected {} {}{}", 
                p, bit_idx, got as u8, expected as u8, status, marker);
        }
    }

    // Analyze error runs
    if !error_runs.is_empty() {
        println!("\n=== Error Run Analysis ===");
        println!("Found {} error runs:", error_runs.len());
        
        for (i, (start, length)) in error_runs.iter().enumerate().take(10) {
            println!("  Run {}: p{}..p{} ({} bits)", 
                i + 1, start, start + length - 1, length);
        }
        
        if error_runs.len() > 10 {
            println!("  ... and {} more runs", error_runs.len() - 10);
        }
    }

    // Check if there's a pattern (e.g., every other bit, every 72nd bit, etc.)
    println!("\n=== Pattern Analysis ===");
    
    // Check if errors are at regular intervals
    if incorrect_count > 2 {
        let error_positions: Vec<usize> = (0..m)
            .filter(|&p| encoded.get(k + p) != expected_output.data.get(k + p))
            .collect();
        
        // Check first few intervals
        if error_positions.len() >= 3 {
            let intervals: Vec<usize> = error_positions.windows(2)
                .map(|w| w[1] - w[0])
                .take(10)
                .collect();
            
            println!("First 10 error position intervals: {:?}", intervals);
            
            // Check if intervals are constant
            let first_interval = intervals[0];
            let all_same = intervals.iter().all(|&x| x == first_interval);
            
            if all_same {
                println!("PATTERN DETECTED: Errors at regular intervals of {} bits", first_interval);
            } else {
                // Check for alternating pattern
                let interval_set: std::collections::HashSet<_> = intervals.iter().collect();
                if interval_set.len() <= 3 {
                    println!("Possible pattern with {} distinct intervals: {:?}", 
                        interval_set.len(), interval_set);
                }
            }
        }
    }

    // Check modulo 360 pattern (DVB-T2 expansion factor Z=360)
    println!("\n=== DVB-T2 Parameter Analysis ===");
    let z = 360; // DVB-T2 normal frame expansion factor
    println!("Expansion factor Z = {}", z);
    
    if let Some(first_err) = first_error_idx {
        println!("First error position modulo Z: {} mod {} = {}", 
            first_err, z, first_err % z);
        
        // Check if all errors are in the same equivalence class mod Z
        let error_positions: Vec<usize> = (0..m)
            .filter(|&p| encoded.get(k + p) != expected_output.data.get(k + p))
            .collect();
        
        let mod_classes: std::collections::HashSet<_> = 
            error_positions.iter().map(|&p| p % z).collect();
        
        println!("Errors occur in {} different mod-{} classes", mod_classes.len(), z);
        if mod_classes.len() <= 10 {
            let mut classes: Vec<_> = mod_classes.iter().cloned().collect();
            classes.sort();
            println!("Classes: {:?}", classes);
        }
    }
}
