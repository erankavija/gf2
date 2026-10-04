//! Compile-fail guards for intentionally absent or state-restricted APIs.
//! `trybuild` compares the rendered rustc diagnostic byte-for-byte, so the
//! `.stderr` snapshots are specific to the MSRV toolchain; the test runs only
//! with `RUN_TRYBUILD=1`, and `TRYBUILD=overwrite` regenerates the snapshots.

#[test]
fn typestate_rejects_out_of_order_calls() {
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
