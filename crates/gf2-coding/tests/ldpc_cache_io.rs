//! LDPC encoding-cache file I/O. `LdpcEncoder::with_cache` consults the cache
//! only on the RREF fallback path, which no DVB-T2 code takes, so these tests
//! fill it through [`EncodingCache::get_or_compute`].

mod common;

use gf2_coding::ldpc::encoding::{CacheKey, EncodingCache};
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use gf2_core::test_scratch::scratch;
use std::path::Path;
use std::sync::Arc;

fn simple_ldpc_code() -> LdpcCode {
    LdpcCode::dvb_t2_short(CodeRate::Rate1_2)
}

fn key_of(code: &LdpcCode) -> CacheKey {
    CacheKey::from_params(code.n(), code.k(), code.parity_check_matrix())
}

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
    let temp_dir = scratch("gf2-ldpc-cache-io");
    let cache = EncodingCache::new();

    let code = simple_ldpc_code();
    populate(&cache, &code);
    assert_eq!(cache.stats().entries, 1, "Cache should hold one entry");

    cache.save_to_directory(temp_dir.path()).unwrap();

    let mut files: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .map(|e| e.unwrap().path().extension().unwrap().to_owned())
        .collect();
    files.sort();

    assert_eq!(files, ["cols", "gf2"], "one parity file and its columns");
}

#[test]
#[ignore = "slow: DVB-T2 Short RREF preprocessing (~2-3 s)"]
fn test_cache_load_from_directory() {
    let temp_dir = scratch("gf2-ldpc-cache-io");

    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    populate(&cache1, &code);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();

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

    let temp_dir = scratch("gf2-ldpc-cache-io");

    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    populate(&cache1, &code);
    cache1.save_to_directory(temp_dir.path()).unwrap();

    let start = std::time::Instant::now();
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let load_time = start.elapsed();

    let start = std::time::Instant::now();
    let matrices = cache2.get(&key_of(&code));
    let lookup_time = start.elapsed();
    assert!(matrices.is_some(), "entry must be present after load");

    println!("Load time: {:?}", load_time);
    println!("Lookup time: {:?}", lookup_time);

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

    let temp_dir = scratch("gf2-ldpc-cache-io");

    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    let files: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .map(|e| e.unwrap())
        .collect();

    assert_eq!(files.len(), 12, "Should create 12 files for DVB-T2 configs");

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

    let temp_dir = scratch("gf2-ldpc-cache-io");

    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();

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
    let temp_dir = scratch("gf2-ldpc-cache-io");

    let cache1 = EncodingCache::new();
    let code = simple_ldpc_code();
    let matrices1 = populate(&cache1, &code);

    let message = gf2_core::BitVec::zeros(matrices1.k());
    let codeword1 = matrices1.encode(&message);

    cache1.save_to_directory(temp_dir.path()).unwrap();

    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let matrices2 = cache2.get(&key_of(&code)).expect("entry must round-trip");
    let codeword2 = matrices2.encode(&message);

    assert_eq!(codeword1, codeword2, "Encoding should survive save/load");
}

#[test]
fn test_empty_directory_loads_empty_cache() {
    let temp_dir = scratch("gf2-ldpc-cache-io");

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

/// Codes whose systematic reduction selects message columns other than
/// `0..k`: a hand-built code with an all-zero last column, which no pivot can
/// take, and small 5G NR lifts.
fn codes_with_permuted_columns() -> Vec<(String, gf2_core::sparse::SpBitMatrixDual)> {
    let hand_built = gf2_core::sparse::SpBitMatrixDual::from_coo(
        2,
        5,
        &[(0, 0), (0, 2), (1, 1), (1, 2), (1, 3)],
    );
    let mut codes = vec![("hand-built".to_owned(), hand_built)];
    for (base_graph, z) in [(1, 2), (2, 2), (2, 3)] {
        let code =
            LdpcCode::from_quasi_cyclic(&gf2_coding::ldpc::QuasiCyclicLdpc::nr_5g(base_graph, z));
        codes.push((
            format!("NR BG{base_graph} Z={z}"),
            code.parity_check_matrix().clone(),
        ));
    }
    codes
}

#[test]
fn reloaded_entries_encode_like_the_uncached_matrices() {
    let mut permuted = 0;
    for (name, h) in codes_with_permuted_columns() {
        let temp_dir = scratch("gf2-ldpc-cache-io");
        let key = CacheKey::from_params(h.cols(), h.cols() - h.rows(), &h);
        let cache = EncodingCache::new();
        let uncached = cache.get_or_compute(key.clone(), &h).expect("preprocess");
        permuted +=
            usize::from(uncached.systematic_cols() != (0..uncached.k()).collect::<Vec<_>>());
        cache.save_to_directory(temp_dir.path()).unwrap();
        let reloaded = EncodingCache::from_directory(temp_dir.path())
            .unwrap()
            .get(&key)
            .expect("entry must round-trip");
        for seed in 0..8u64 {
            let mut message = gf2_core::BitVec::zeros(uncached.k());
            for bit in 0..uncached.k() {
                let mixed = (seed + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (bit % 61);
                message.set(bit, mixed & 1 == 1);
            }
            assert_eq!(
                reloaded.encode(&message),
                uncached.encode(&message),
                "{name}: reloaded encoder differs, seed {seed}"
            );
        }
    }
    assert!(
        permuted > 0,
        "no code exercised message columns outside 0..k"
    );
}

#[test]
fn an_entry_without_its_column_selection_is_not_loaded() {
    let temp_dir = scratch("gf2-ldpc-cache-io");
    let (_, h) = codes_with_permuted_columns().remove(0);
    let key = CacheKey::from_params(h.cols(), h.cols() - h.rows(), &h);
    let cache = EncodingCache::new();
    cache.get_or_compute(key.clone(), &h).expect("preprocess");
    cache.save_to_directory(temp_dir.path()).unwrap();
    for entry in std::fs::read_dir(temp_dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("gf2") {
            std::fs::remove_file(path).unwrap();
        }
    }
    let reloaded = EncodingCache::from_directory(temp_dir.path()).unwrap();
    assert!(
        reloaded.get(&key).is_none(),
        "an entry loaded under assumed columns"
    );
}
