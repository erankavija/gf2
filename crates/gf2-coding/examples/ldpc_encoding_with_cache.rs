//! LDPC Encoding with optional caching.
//!
//! Demonstrates both the simple (no cache) and cached encoder creation paths.

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::time::Instant;

fn main() {
    println!("=== LDPC Encoding: With and Without Cache ===\n");

    // Example 1: Without cache (simplest)
    println!("1. Without cache (simple, always works):");
    println!("   Creating encoder for DVB-T2 short rate 1/2...");

    let start = Instant::now();
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder_no_cache = LdpcEncoder::new(code.clone());
    let time_no_cache = start.elapsed();

    println!(
        "   Time: {:.2}s (preprocessing H matrix)",
        time_no_cache.as_secs_f64()
    );
    println!(
        "   Parameters: n={}, k={}\n",
        encoder_no_cache.n(),
        encoder_no_cache.k()
    );

    // Example 2: With cache (first call)
    println!("2. With cache (first call - preprocesses and caches):");

    let cache = EncodingCache::new();
    println!("   Creating encoder with cache...");

    let start = Instant::now();
    let _encoder_cached_1 = LdpcEncoder::with_cache(code.clone(), &cache);
    let time_first = start.elapsed();

    println!(
        "   Time: {:.2}s (similar to no-cache)",
        time_first.as_secs_f64()
    );
    println!("   Cache entries: {}\n", cache.stats().entries);

    // Example 3: With cache (second call - instant!)
    println!("3. With cache (second call - cache hit):");

    let start = Instant::now();
    let _encoder_cached_2 = LdpcEncoder::with_cache(code.clone(), &cache);
    let time_second = start.elapsed();

    println!(
        "   Time: {:.6}s (<1μs - instant!)",
        time_second.as_secs_f64()
    );
    println!(
        "   Speedup: {:.0}×\n",
        time_first.as_secs_f64() / time_second.as_secs_f64()
    );

    // Example 4: Encode some data with all encoders
    println!("4. Encoding performance (all encoders work the same):");

    let message = BitVec::zeros(encoder_no_cache.k());

    let start = Instant::now();
    let codeword = encoder_no_cache.encode(&message);
    let encode_time = start.elapsed();

    println!(
        "   Encoded {} bits → {} bits",
        message.len(),
        codeword.len()
    );
    println!(
        "   Encoding time: {:.3}ms (same for all)\n",
        encode_time.as_secs_f64() * 1000.0
    );

    // Example 5: Precomputing all configs
    println!("5. Precomputing all DVB-T2 configurations:");
    println!("   (This is recommended for production applications)");

    let cache_precomp = EncodingCache::new();

    let start = Instant::now();
    cache_precomp.precompute_dvb_t2();
    let precomp_time = start.elapsed();

    println!(
        "   Precomputation time: {:.1}s (one-time cost)",
        precomp_time.as_secs_f64()
    );
    println!("   Cached entries: {}", cache_precomp.stats().entries);
    println!("   Memory: ~200 MB\n");

    // Now all encoder creation is instant
    println!("   Creating encoders for different rates (all instant):");

    let rates = [CodeRate::Rate1_2, CodeRate::Rate3_5, CodeRate::Rate2_3];
    for rate in &rates {
        let start = Instant::now();
        let _enc = LdpcEncoder::with_cache(LdpcCode::dvb_t2_short(*rate), &cache_precomp);
        let time = start.elapsed();
        println!("      Rate {:?}: {:.6}s", rate, time.as_secs_f64());
    }

    println!("\n=== Summary ===");
    println!("✓ Without cache: Simple, always works");
    println!("✓ With cache: Optional performance boost");
    println!("✓ You control cache lifetime and scope");
    println!("✓ No global state required");
}
