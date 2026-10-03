//! Shared gating helpers for gf2-coding integration tests.
//!
//! Two kinds of test cannot run on a shared CI runner, and both used to fail
//! the nightly slow tier by panicking instead of standing down:
//!
//! * **Host-local data.** ETSI DVB-T2 test-vector streams and the precomputed
//!   DVB-T2 RREF generator cache are large artefacts that live on the dev host
//!   and are not in the repository. Tests needing them resolve the path through
//!   [`dvb_vectors_dir`] / [`dvb_t2_generator_cache_dir`] and skip when it is
//!   absent.
//! * **Benchmarks in test clothing.** Wall-clock assertions and multi-minute
//!   RREF preprocessing measure the machine, so they only mean something on a
//!   quiesced host. They are gated behind [`GF2_BENCH_ENV`] and skip elsewhere.
//!
//! A skipped test prints `SKIP <name>: <reason>` to stderr and passes.

#![allow(dead_code)] // each test binary uses only the helpers it needs

use std::path::PathBuf;

/// Opt-in environment variable for benchmark-grade tests (wall-clock
/// assertions, multi-minute preprocessing). Set to any value except `0`.
pub const GF2_BENCH_ENV: &str = "GF2_BENCH";

/// Whether benchmark-grade tests are enabled on this host.
pub fn bench_enabled() -> bool {
    matches!(std::env::var(GF2_BENCH_ENV), Ok(v) if v != "0")
}

/// Prints a skip notice to stderr. Call immediately before returning early.
pub fn skip(test: &str, reason: &str) {
    eprintln!("SKIP {test}: {reason}");
}

/// Skips the calling test unless [`GF2_BENCH_ENV`] is set.
///
/// ```ignore
/// skip_unless_bench!("test_foo", "wall-clock assertion");
/// ```
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

/// The DVB-T2 reference-stream tree, or `None` when it is not a directory.
pub fn dvb_vectors_dir() -> Option<PathBuf> {
    let path = gf2_coding::test_support::dvb_vectors_path();
    path.is_dir().then_some(path)
}

/// Directory holding the precomputed DVB-T2 RREF generator cache
/// (`crates/gf2-coding/data/ldpc/dvb_t2`), or `None` when it has not been
/// generated on this host.
///
/// The cache is ~640 MB of `.gf2` files produced by
/// `EncodingCache::precompute_and_save_dvb_t2`; it is deliberately not
/// committed.
pub fn dvb_t2_generator_cache_dir() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    path.is_dir().then_some(path)
}
