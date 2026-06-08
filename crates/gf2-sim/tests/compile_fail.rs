//! Compile-fail guard for the DVB-T2 typestate builder (criterion-2 of `81d05bab`).
//!
//! Verifies that the typestate markers make an out-of-order builder call a
//! genuine **compile** error: `.decoder()` cannot be invoked before `.modcod()`
//! because `decoder()` is only defined on `Builder<NeedsDecoder>`, whereas
//! `Pipeline::dvb_t2()` returns a `Builder<NeedsModcod>`.
//!
//! The expectation `.stderr` files are checked in alongside the failing cases.
//! `trybuild` stderr is rustc-version-sensitive, so the failing examples are
//! kept minimal (a single offending method call) to keep the rendered error
//! stable. Regenerate with `TRYBUILD=overwrite cargo test -p gf2-sim --release
//! --test compile_fail`.

#[test]
fn typestate_rejects_out_of_order_calls() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
