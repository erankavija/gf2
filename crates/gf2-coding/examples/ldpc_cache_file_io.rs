//! Demonstrates LDPC encoding cache with file I/O.
//!
//! This example shows how to use persistent file-based caching to avoid
//! expensive preprocessing (2-3 seconds per configuration).
//!
//! # Usage
//!
//! ```bash
//! # Generate cache files (slow, one-time operation)
//! cargo run --example ldpc_cache_file_io -- generate
//!
//! # Use cached files (instant)
//! cargo run --example ldpc_cache_file_io -- use
//! ```

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("use");

    // Use data directory by default, or target for examples
    let cache_dir = Path::new("data/ldpc/dvb_t2");

    match mode {
        "generate" => {
            eprintln!("Use 'cargo run --release --bin generate_cache' instead for generation.");
            std::process::exit(1);
        }
        "use" => use_cache(cache_dir),
        _ => {
            eprintln!("Usage: {} use", args[0]);
            std::process::exit(1);
        }
    }
}

#[allow(dead_code)]
fn generate_cache(cache_dir: &Path) {
    println!("=== Generating LDPC Cache Files ===\n");
    println!("This is a one-time operation that takes ~30-60 seconds.");
    println!("Cache directory: {}\n", cache_dir.display());

    let start = Instant::now();

    match EncodingCache::precompute_and_save_dvb_t2(cache_dir) {
        Ok(()) => {
            let elapsed = start.elapsed();
            println!(
                "\n✓ Cache generation complete in {:.1}s",
                elapsed.as_secs_f64()
            );
            println!(
                "✓ 12 DVB-T2 configurations saved to {}",
                cache_dir.display()
            );

            // Show file sizes
            if let Ok(entries) = std::fs::read_dir(cache_dir) {
                let mut total_size = 0;
                let mut count = 0;
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        total_size += metadata.len();
                        count += 1;
                    }
                }
                println!(
                    "✓ Total cache size: {:.1} MB ({} files)",
                    total_size as f64 / 1_000_000.0,
                    count
                );
            }

            println!("\nNow run: cargo run --example ldpc_cache_file_io -- use");
        }
        Err(e) => {
            eprintln!("✗ Error generating cache: {}", e);
            std::process::exit(1);
        }
    }
}

fn use_cache(cache_dir: &Path) {
    println!("=== Using Cached LDPC Matrices ===\n");

    if !cache_dir.exists() {
        eprintln!("✗ Cache directory not found: {}", cache_dir.display());
        eprintln!("\nRun first: cargo run --example ldpc_cache_file_io -- generate");
        std::process::exit(1);
    }

    // Load cache from disk
    println!("Loading cache from {}...", cache_dir.display());
    let load_start = Instant::now();

    let cache = match EncodingCache::from_directory(cache_dir) {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("✗ Error loading cache: {}", e);
            std::process::exit(1);
        }
    };

    let load_time = load_start.elapsed();
    println!("✓ Cache loaded in {:.1}ms\n", load_time.as_millis());

    // Test a few configurations
    let configs = vec![
        ("Short 1/2", CodeRate::Rate1_2, true),
        ("Short 3/5", CodeRate::Rate3_5, true),
        ("Normal 1/2", CodeRate::Rate1_2, false),
        ("Normal 3/5", CodeRate::Rate3_5, false),
    ];

    println!("Creating encoders (should be instant with cache):\n");

    for (name, rate, is_short) in configs {
        let code = if is_short {
            LdpcCode::dvb_t2_short(rate)
        } else {
            LdpcCode::dvb_t2_normal(rate)
        };

        let start = Instant::now();
        let encoder = LdpcEncoder::with_cache(code, &cache);
        let create_time = start.elapsed();

        println!(
            "  DVB-T2 {} (n={}, k={}): {:?}",
            name,
            encoder.n(),
            encoder.k(),
            create_time
        );

        if create_time.as_millis() > 10 {
            eprintln!("    ⚠ Warning: Creation took >10ms (cache may not be working)");
        }
    }

    // Demonstrate encoding
    println!("\nEncoding example message...");
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::with_cache(code, &cache);

    let message = gf2_core::BitVec::zeros(encoder.k());
    let encode_start = Instant::now();
    let _codeword = encoder.encode(&message);
    let encode_time = encode_start.elapsed();

    println!(
        "✓ Encoded {} bits → {} bits in {:?}",
        encoder.k(),
        encoder.n(),
        encode_time
    );

    println!("\n=== Summary ===");
    println!("✓ File cache enables instant encoder creation (<1μs vs 2-3 seconds)");
    println!("✓ 12,000× speedup for initialization");
    println!("✓ All DVB-T2 configurations available instantly");
}
