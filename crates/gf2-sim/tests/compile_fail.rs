//! Compile-fail guards for the typestate preset builders: DVB-T2
//! (criterion-2 of `81d05bab`) and 5G NR (criterion-2 of `e478daa8`).
//!
//! Verifies that the typestate markers make an out-of-order builder call a
//! genuine **compile** error: `.decoder()` cannot be invoked before `.modcod()`
//! because `decoder()` is only defined on `Builder<NeedsDecoder>`, whereas
//! `Pipeline::dvb_t2()` returns a `Builder<NeedsModcod>`; likewise
//! `.lifting_size()` cannot precede `.base_graph()` on the 5G NR builder.
//!
//! The expectation `.stderr` files are checked in alongside the failing cases.
//! `trybuild` compares the *rendered* rustc diagnostic byte-for-byte, so the
//! snapshots are specific to the rustc version they were generated against.
//! Running this on a floating toolchain breaks the build on every rustc bump
//! (observed 2026-06-30: CI's `@stable` moved past 1.95.0 and re-rendered the
//! E0599 note). To keep the guard meaningful without that churn it is gated
//! behind `RUN_TRYBUILD` and exercised in CI only under the pinned MSRV
//! toolchain the snapshots target (see the `Lean`-adjacent compile-fail step in
//! `.github/workflows/ci.yml`).
//!
//! Run locally and regenerate snapshots under the pinned toolchain:
//!   RUN_TRYBUILD=1 cargo +1.95.0 test -p gf2-sim --release --test compile_fail
//!   TRYBUILD=overwrite RUN_TRYBUILD=1 cargo +1.95.0 test -p gf2-sim --release \
//!     --test compile_fail

#[test]
fn typestate_rejects_out_of_order_calls() {
    // Skip on toolchains the `.stderr` snapshots were not generated against so a
    // rustc diagnostic-format change cannot fail the default `cargo test` /
    // nextest battery (which CI runs on floating `@stable`). CI re-enables this
    // via RUN_TRYBUILD=1 under the pinned MSRV toolchain.
    if std::env::var_os("RUN_TRYBUILD").is_none() {
        eprintln!(
            "skipping typestate_rejects_out_of_order_calls: rustc-version-specific; \
             set RUN_TRYBUILD=1 and run under the pinned MSRV toolchain"
        );
        return;
    }
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
