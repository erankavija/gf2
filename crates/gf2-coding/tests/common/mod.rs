//! Gating helpers for gf2-coding integration tests that need host-local data
//! ([`dvb_vectors_dir`], [`dvb_t2_generator_cache_dir`]) or a quiesced
//! benchmark host ([`GF2_BENCH_ENV`]). A skipped test prints
//! `SKIP <name>: <reason>` to stderr and passes.

#![allow(dead_code)] // each test binary uses only the helpers it needs

use std::path::PathBuf;

/// Opt-in environment variable for benchmark-grade tests (wall-clock
/// assertions, RREF preprocessing). Set to any value except `0`.
pub const GF2_BENCH_ENV: &str = "GF2_BENCH";

pub fn bench_enabled() -> bool {
    matches!(std::env::var(GF2_BENCH_ENV), Ok(v) if v != "0")
}

/// Emits the skip notice only; the calling test returns by itself.
pub fn skip(test: &str, reason: &str) {
    eprintln!("SKIP {test}: {reason}");
}

/// Returns from the calling test unless [`bench_enabled`].
#[macro_export]
macro_rules! skip_unless_bench {
    ($test:expr, $reason:expr) => {
        if !$crate::common::bench_enabled() {
            $crate::common::skip(
                $test,
                &format!(
                    "{} — set {}=1 on a quiesced host to run it",
                    $reason,
                    $crate::common::GF2_BENCH_ENV
                ),
            );
            return;
        }
    };
}

pub fn dvb_vectors_dir() -> Option<PathBuf> {
    let path = gf2_coding::test_support::dvb_vectors_path();
    path.is_dir().then_some(path)
}

/// The uncommitted DVB-T2 RREF generator cache directory written by
/// `EncodingCache::precompute_and_save_dvb_t2`, or `None` when it is absent.
pub fn dvb_t2_generator_cache_dir() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    path.is_dir().then_some(path)
}
