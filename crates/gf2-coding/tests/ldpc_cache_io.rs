//! Tests for LDPC cache file I/O integration.
//!
//! The cache stores RREF ([`RuEncodingMatrices`]) preprocessing results. Note
//! that `LdpcEncoder::with_cache` only consults the cache on the RREF *fallback*
//! path: every DVB-T2 code takes the IRA fast path and never populates it. These
//! tests therefore drive the cache through [`EncodingCache::get_or_compute`],
//! which is the API that actually fills it.

mod common;

use gf2_coding::ldpc::encoding::{CacheKey, EncodingCache};
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper: create a simple test LDPC code
fn simple_ldpc_code() -> LdpcCode {
    LdpcCode::dvb_t2_short(CodeRate::Rate1_2)
}

/// Cache key for a code.
fn key_of(code: &LdpcCode) -> CacheKey {
    CacheKey::from_params(code.n(), code.k(), code.parity_check_matrix())
}

/// Populate `cache` with the RREF matrices for `code` (~2-3 s for a Short rate).
fn populate(
    cache: &EncodingCache,
    code: &LdpcCode,
) -> Arc<gf2_coding::ldpc::encoding::RuEncodingMatrices> {
    cache
        .get_or_compute(key_of(code), code.parity_check_matrix())
        .expect("RREF preprocessing failed")
}

#[test]
#[ignore = "slow: DVB-T2 Short RREF preprocessing (~2-3 s)"]
fn test_cache_save_to_directory() {
    let temp_dir = TempDir::new().unwrap();
    let cache = EncodingCache::new();

    // Precompute one entry
    let code = simple_ldpc_code();
    populate(&cache, &code);
    assert_eq!(cache.stats().entries, 1, "Cache should hold one entry");

    // Save cache to directory
    cache.save_to_directory(temp_dir.path()).unwrap();

    // Verify file was created
    let files: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();

    assert_eq!(files.len(), 1, "Should create one file");
    assert!(files[0].to_str().unwrap().ends_with(".gf2"));
}

#[test]
#[ignore = "slow: DVB-T2 Short RREF preprocessing (~2-3 s)"]
fn test_cache_load_from_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create and save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    populate(&cache1, &code);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load into new cache
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();

    // Verify the loaded cache holds the same entry, reachable without recompute
    let loaded = cache2.get(&key_of(&code)).expect("entry must round-trip");
    assert_eq!(loaded.k(), 7200);
}

#[test]
#[ignore = "bench: wall-clock assertions on cache load/lookup; run on a quiesced host"]
fn test_cache_load_is_fast() {
    skip_unless_bench!(
        "test_cache_load_is_fast",
        "asserts cache load <500 ms and lookup <100 μs, which measures the host"
    );

    let temp_dir = TempDir::new().unwrap();

    // Save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    populate(&cache1, &code);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load and measure time
    let start = std::time::Instant::now();
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let load_time = start.elapsed();

    // Look up from the loaded cache (should be instant — no preprocessing)
    let start = std::time::Instant::now();
    let matrices = cache2.get(&key_of(&code));
    let lookup_time = start.elapsed();
    assert!(matrices.is_some(), "entry must be present after load");

    println!("Load time: {:?}", load_time);
    println!("Lookup time: {:?}", lookup_time);

    // Should be much faster than 2-3 seconds of preprocessing
    // Load time includes deserializing ~30M edges, so 500ms is reasonable
    assert!(load_time.as_millis() < 500, "Load should be <500ms");
    assert!(lookup_time.as_micros() < 100, "Lookup should be <100μs");
}

#[test]
#[ignore = "bench: RREF preprocessing of all 12 DVB-T2 configs (~13 min); run on a quiesced host"]
fn test_precompute_and_save_dvb_t2() {
    skip_unless_bench!(
        "test_precompute_and_save_dvb_t2",
        "RREF-preprocesses all 12 DVB-T2 configs (~13 min, ~800 MB peak)"
    );

    let temp_dir = TempDir::new().unwrap();

    // Precompute and save all DVB-T2 configs (slow, but one-time)
    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    // Verify all 12 files were created
    let files: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .map(|e| e.unwrap())
        .collect();

    assert_eq!(files.len(), 12, "Should create 12 files for DVB-T2 configs");

    // Check file sizes are reasonable (~800 KB each)
    for entry in files {
        let metadata = entry.metadata().unwrap();
        let size_kb = metadata.len() / 1024;
        assert!(
            size_kb > 100 && size_kb < 5000,
            "File size should be 100KB-5MB, got {}KB",
            size_kb
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing of all 12 DVB-T2 configs (~13 min); run on a quiesced host"]
fn test_load_dvb_t2_cache() {
    skip_unless_bench!(
        "test_load_dvb_t2_cache",
        "RREF-preprocesses all 12 DVB-T2 configs (~13 min, ~800 MB peak)"
    );

    let temp_dir = TempDir::new().unwrap();

    // Precompute and save
    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    // Load cache
    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();

    // Verify all 12 configs are present and reachable without recompute
    let configs = [
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate1_2,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate3_5,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate2_3,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate3_4,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate4_5,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            CodeRate::Rate5_6,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate1_2,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate3_5,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate2_3,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate3_4,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate4_5,
        ),
        (
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
            CodeRate::Rate5_6,
        ),
    ];

    for (frame_size, rate) in &configs {
        let code = match frame_size {
            gf2_coding::ldpc::dvb_t2::FrameSize::Short => LdpcCode::dvb_t2_short(*rate),
            gf2_coding::ldpc::dvb_t2::FrameSize::Normal => LdpcCode::dvb_t2_normal(*rate),
        };

        let start = std::time::Instant::now();
        let matrices = cache.get(&key_of(&code));
        let duration = start.elapsed();

        println!("{:?} {:?}: {:?}", frame_size, rate, duration);
        assert!(duration.as_micros() < 100, "Should be instant from cache");
        assert!(matrices.expect("config must be cached").k() > 0);
    }
}

#[test]
#[ignore = "slow: DVB-T2 Short RREF preprocessing (~2-3 s)"]
fn test_cache_roundtrip_encoding() {
    let temp_dir = TempDir::new().unwrap();

    // Save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    let matrices1 = populate(&cache1, &code);

    let message = gf2_core::BitVec::zeros(matrices1.k());
    let codeword1 = matrices1.encode(&message);

    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load cache and encode the same message from the deserialized matrices
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let matrices2 = cache2.get(&key_of(&code)).expect("entry must round-trip");
    let codeword2 = matrices2.encode(&message);

    // Results should be identical
    assert_eq!(codeword1, codeword2, "Encoding should survive save/load");
}

#[test]
fn test_empty_directory_loads_empty_cache() {
    let temp_dir = TempDir::new().unwrap();

    // Load from empty directory
    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();

    let stats = cache.stats();
    assert_eq!(
        stats.entries, 0,
        "Empty directory should create empty cache"
    );
}

#[test]
fn test_nonexistent_directory_error() {
    let result = EncodingCache::from_directory(Path::new("/nonexistent/path"));
    assert!(result.is_err(), "Should error on nonexistent directory");
}
