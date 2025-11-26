//! Tests for LDPC cache file I/O integration.

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use std::path::Path;
use tempfile::TempDir;

/// Helper: create a simple test LDPC code
fn simple_ldpc_code() -> LdpcCode {
    LdpcCode::dvb_t2_short(CodeRate::Rate1_2)
}

#[test]
#[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
fn test_cache_save_to_directory() {
    let temp_dir = TempDir::new().unwrap();
    let cache = EncodingCache::new();

    // Precompute one entry
    let code = simple_ldpc_code();
    let encoder = LdpcEncoder::with_cache(code, &cache);
    drop(encoder);

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
#[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
fn test_cache_load_from_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create and save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    let _encoder = LdpcEncoder::with_cache(code.clone(), &cache1);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load into new cache
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();

    // Verify loaded cache works
    let encoder2 = LdpcEncoder::with_cache(code, &cache2);
    assert_eq!(encoder2.k(), 7200);
}

#[test]
#[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
fn test_cache_load_is_fast() {
    let temp_dir = TempDir::new().unwrap();

    // Save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    let _encoder = LdpcEncoder::with_cache(code.clone(), &cache1);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load and measure time
    let start = std::time::Instant::now();
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let load_time = start.elapsed();

    // Create encoder from loaded cache (should be instant)
    let start = std::time::Instant::now();
    let _encoder = LdpcEncoder::with_cache(code, &cache2);
    let create_time = start.elapsed();

    println!("Load time: {:?}", load_time);
    println!("Create time: {:?}", create_time);

    // Should be much faster than 2-3 seconds of preprocessing
    // Load time includes deserializing ~30M edges, so 500ms is reasonable
    assert!(load_time.as_millis() < 500, "Load should be <500ms");
    assert!(create_time.as_micros() < 100, "Create should be <100μs");
}

#[test]
#[ignore = "Slow: requires all 12 DVB-T2 configs preprocessing (~15-30 seconds with SIMD)"]
fn test_precompute_and_save_dvb_t2() {
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
#[ignore = "Slow: requires all 12 DVB-T2 configs preprocessing (~15-30 seconds with SIMD)"]
fn test_load_dvb_t2_cache() {
    let temp_dir = TempDir::new().unwrap();

    // Precompute and save
    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    // Load cache
    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();

    // Verify all 12 configs work instantly
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
        let encoder = LdpcEncoder::with_cache(code, &cache);
        let duration = start.elapsed();

        println!("{:?} {:?}: {:?}", frame_size, rate, duration);
        assert!(duration.as_micros() < 100, "Should be instant from cache");
        assert!(encoder.k() > 0);
    }
}

#[test]
#[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
fn test_cache_roundtrip_encoding() {
    let temp_dir = TempDir::new().unwrap();

    // Save cache
    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    let encoder1 = LdpcEncoder::with_cache(code.clone(), &cache1);

    let message = gf2_core::BitVec::zeros(encoder1.k());
    let codeword1 = encoder1.encode(&message);

    cache1.save_to_directory(temp_dir.path()).unwrap();

    // Load cache and encode same message
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let encoder2 = LdpcEncoder::with_cache(code, &cache2);
    let codeword2 = encoder2.encode(&message);

    // Results should be identical
    assert_eq!(codeword1, codeword2, "Encoding should be deterministic");
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
