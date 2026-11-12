//! LDPC encoding-cache file I/O. `LdpcEncoder::with_cache` consults the cache
//! only on the RREF fallback path, which no DVB-T2 code takes, so these tests
//! fill it through [`EncodingCache::get_or_compute`].

mod common;

use gf2_coding::ldpc::encoding::{CacheKey, EncodingCache, RuEncodingMatrices};
use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
use gf2_coding::test_support::{ldpc_batch_code, ldpc_dvb_t2_codes};
use gf2_coding::CodeRate;
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::test_scratch::scratch;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Seeded random messages each round-trip test encodes.
const MESSAGES: u64 = 8;

/// Upper bound on the binary `BitMatrix` header and JSON metadata that precede
/// the parity part's row words.
const PARITY_HEADER_BOUND: usize = 1024;

fn key_of(h: &SpBitMatrixDual) -> CacheKey {
    CacheKey::from_params(h.cols(), h.cols() - h.rows(), h)
}

/// Asserts that `dir` holds exactly the [`CacheKey::entry_files`] of `keys`,
/// with each parity file sized by its k × (n − k) row words and each column
/// file by its n decimal column indices.
fn assert_entry_files(dir: &Path, cache: &EncodingCache, keys: &[CacheKey]) {
    let expected: BTreeSet<PathBuf> = keys
        .iter()
        .flat_map(|key| {
            let files = key.entry_files(dir);
            [files.parity, files.columns]
        })
        .collect();
    let written: BTreeSet<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(written, expected, "cache directory layout");

    let len = |path: &Path| std::fs::metadata(path).unwrap().len() as usize;
    for key in keys {
        let matrices = cache.get(key).expect("saved entry");
        let files = key.entry_files(dir);
        let parity = matrices.parity_part();
        let words = parity.rows() * parity.cols().div_ceil(64) * 8;
        let parity_len = len(&files.parity);
        assert!(
            (words..=words + PARITY_HEADER_BOUND).contains(&parity_len),
            "{}: {parity_len} bytes for {words} bytes of row words",
            files.parity.display()
        );
        let n = matrices.n();
        let digits = (n - 1).to_string().len();
        let columns_len = len(&files.columns);
        assert!(
            (2 * n..=n * (digits + 1)).contains(&columns_len),
            "{}: {columns_len} bytes for {n} column indices",
            files.columns.display()
        );
    }
}

/// Saves `h`'s entry, checks the directory layout, reloads it, and asserts
/// that the reloaded entry encodes [`MESSAGES`] seeded random messages like a
/// fresh build. Returns whether the reduction selects message columns other
/// than `0..k`.
fn assert_reload_encodes_like_a_fresh_build(name: &str, h: &SpBitMatrixDual) -> bool {
    let temp_dir = scratch("gf2-ldpc-cache-io");
    let key = key_of(h);
    let cache = EncodingCache::new();
    cache.get_or_compute(key.clone(), h).expect("preprocess");
    cache.save_to_directory(temp_dir.path()).unwrap();
    assert_entry_files(temp_dir.path(), &cache, std::slice::from_ref(&key));

    let reloaded = EncodingCache::from_directory(temp_dir.path())
        .unwrap()
        .get(&key)
        .expect("entry must round-trip");
    let fresh = RuEncodingMatrices::preprocess(h).expect("preprocess");
    for seed in 0..MESSAGES {
        let message = BitVec::random(fresh.k(), &mut StdRng::seed_from_u64(seed));
        assert_eq!(
            reloaded.encode(&message),
            fresh.encode(&message),
            "{name}: reloaded encoder differs, seed {seed}"
        );
    }
    fresh.systematic_cols() != (0..fresh.k()).collect::<Vec<_>>()
}

/// A hand-built code with an all-zero last column, which no pivot can take.
fn hand_built_code() -> SpBitMatrixDual {
    SpBitMatrixDual::from_coo(2, 5, &[(0, 0), (0, 2), (1, 1), (1, 2), (1, 3)])
}

#[test]
fn test_reload_encoding_hand_built_code() {
    assert!(
        assert_reload_encodes_like_a_fresh_build("hand-built", &hand_built_code()),
        "the hand-built code selects message columns outside 0..k"
    );
}

#[test]
fn test_reload_encoding_quasi_cyclic_code() {
    assert_reload_encodes_like_a_fresh_build(
        "quasi-cyclic (96, 48)",
        ldpc_batch_code().parity_check_matrix(),
    );
}

#[test]
fn test_reload_encoding_nr_5g_codes() {
    for (base_graph, z) in [(1, 2), (2, 2), (2, 3)] {
        let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, z));
        assert_reload_encodes_like_a_fresh_build(
            &format!("NR BG{base_graph} Z={z}"),
            code.parity_check_matrix(),
        );
    }
}

/// The DVB-T2 short frame at the highest rate, the family member with the
/// fewest parity checks and so the cheapest row reduction.
#[test]
fn test_reload_encoding_dvb_t2_code() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate5_6);
    assert_reload_encodes_like_a_fresh_build("DVB-T2 short 5/6", code.parity_check_matrix());
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
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let h = code.parity_check_matrix();
    cache1.get_or_compute(key_of(h), h).expect("preprocess");
    cache1.save_to_directory(temp_dir.path()).unwrap();

    let start = std::time::Instant::now();
    let cache2 = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let load_time = start.elapsed();

    let start = std::time::Instant::now();
    let matrices = cache2.get(&key_of(h));
    let lookup_time = start.elapsed();
    assert!(matrices.is_some(), "entry must be present after load");

    println!("Load time: {:?}", load_time);
    println!("Lookup time: {:?}", lookup_time);

    assert!(load_time.as_millis() < 500, "Load should be <500ms");
    assert!(lookup_time.as_micros() < 100, "Lookup should be <100μs");
}

#[test]
#[ignore = "bench: RREF preprocessing of every DVB-T2 code; run on a quiesced host"]
fn test_precompute_and_save_dvb_t2() {
    skip_unless_bench!(
        "test_precompute_and_save_dvb_t2",
        "RREF-preprocesses every DVB-T2 code"
    );

    let temp_dir = scratch("gf2-ldpc-cache-io");

    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();
    let keys: Vec<_> = ldpc_dvb_t2_codes()
        .map(|(_, _, code)| key_of(code.parity_check_matrix()))
        .collect();
    assert_entry_files(temp_dir.path(), &cache, &keys);
}

#[test]
#[ignore = "bench: RREF preprocessing of every DVB-T2 code; run on a quiesced host"]
fn test_load_dvb_t2_cache() {
    skip_unless_bench!(
        "test_load_dvb_t2_cache",
        "RREF-preprocesses every DVB-T2 code"
    );

    let temp_dir = scratch("gf2-ldpc-cache-io");

    EncodingCache::precompute_and_save_dvb_t2(temp_dir.path()).unwrap();

    let cache = EncodingCache::from_directory(temp_dir.path()).unwrap();

    for (frame_size, rate, code) in ldpc_dvb_t2_codes() {
        let start = std::time::Instant::now();
        let matrices = cache.get(&key_of(code.parity_check_matrix()));
        let duration = start.elapsed();

        println!("{:?} {:?}: {:?}", frame_size, rate, duration);
        assert!(duration.as_micros() < 100, "Should be instant from cache");
        assert_eq!(matrices.expect("config must be cached").k(), code.k());
    }
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

#[test]
fn test_entry_without_column_selection_is_not_loaded() {
    let temp_dir = scratch("gf2-ldpc-cache-io");
    let h = hand_built_code();
    let key = key_of(&h);
    let cache = EncodingCache::new();
    cache.get_or_compute(key.clone(), &h).expect("preprocess");
    cache.save_to_directory(temp_dir.path()).unwrap();
    std::fs::remove_file(key.entry_files(temp_dir.path()).columns).unwrap();
    let reloaded = EncodingCache::from_directory(temp_dir.path()).unwrap();
    assert!(
        reloaded.get(&key).is_none(),
        "an entry loaded under assumed columns"
    );
}
