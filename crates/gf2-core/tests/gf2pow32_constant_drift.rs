//! Drift-check between the Rust SSOT for the GF(2^32) Conway polynomial
//! (`PrimitivePolynomialDatabase::standard(32)`) and the C++ header
//! (`benchmarks/reference/gf2pow32_constants.h`).
//!
//! The C++ harnesses (`ntl_bench.cpp`, `ntl_gf2pow32_smoke.cpp`, and any
//! future m=32 lane) all read the constant from that header. This test
//! parses the header at test time and asserts the constant equals the
//! Rust SSOT — so a drift in either direction fails CI before the
//! mismatch can reach a benchmark or smoke run.
//!
//! As of jit:b13799ac R2, the C++ side no longer carries a scalar
//! GF(2^32) reference multiplier (the smoke is now a direct
//! gf2-core ↔ NTL byte-equality oracle via the ground-truth file
//! emitted by `gf2pow32_smoke_emit_expected`), so this drift check
//! covers the only remaining cross-language SSOT for m=32: the
//! polynomial bits themselves.
//!
//! The Rust scalar reference `gf2pow32_matmul.rs::ref_gf2pow32_mul` is
//! retained as a Rust-internal gf2-core ↔ scalar witness; it does not
//! participate in this drift check (its SSOT is the in-file
//! `CONWAY_LOW32` constant, derived from the same database value).
//!
//! Issue: jit:b13799ac (SSOT extraction follow-on after R2 review).

use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

/// Parse `constexpr uint64_t kGf2coreConwayM32 = 0x...ULL;` (or any C/C++
/// integer-literal style) out of the header text and return its value.
///
/// Strict parser: looks for the line that defines `kGf2coreConwayM32`, then
/// extracts the hex literal between `=` and `;`. Hex digit grouping with `'`
/// (C++14 single-quote separators) and trailing `ULL` / `u64` suffixes are
/// tolerated.
fn parse_header_constant(header: &str, name: &str) -> u64 {
    let line = header
        .lines()
        .find(|l| l.contains(&format!("{name} ")) && l.contains('='))
        .unwrap_or_else(|| panic!("could not locate `{name}` definition in header"));
    let rhs = line
        .split('=')
        .nth(1)
        .unwrap_or_else(|| panic!("`{name}` line has no `=`: {line}"));
    let body = rhs.split(';').next().unwrap().trim();
    // Strip C++ literal hygiene: leading `0x`, single-quote digit separators,
    // and the unsigned/long-long suffix forms.
    let mut cleaned = body
        .trim()
        .trim_end_matches(['U', 'L', 'u', 'l'])
        .to_string();
    cleaned.retain(|c| c != '\'');
    let hex = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or_else(|| panic!("`{name}` value is not a hex literal: {body}"));
    u64::from_str_radix(hex, 16)
        .unwrap_or_else(|e| panic!("`{name}` hex parse failed for `{hex}`: {e}"))
}

#[test]
fn cpp_header_conway_m32_matches_rust_ssot() {
    // Locate the header relative to CARGO_MANIFEST_DIR (the gf2-core crate
    // root). The benchmarks/ directory sits two levels up from the crate.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let header_path = std::path::Path::new(manifest_dir)
        .join("..")
        .join("..")
        .join("benchmarks")
        .join("reference")
        .join("gf2pow32_constants.h");
    let header = std::fs::read_to_string(&header_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", header_path.display()));

    let cpp_value = parse_header_constant(&header, "kGf2coreConwayM32");
    let rust_value =
        PrimitivePolynomialDatabase::standard(32).expect("Rust SSOT for m=32 must exist");

    assert_eq!(
        cpp_value,
        rust_value,
        "GF(2^32) Conway polynomial drift: \
         C++ `{}::kGf2coreConwayM32` = {:#x}, \
         Rust `PrimitivePolynomialDatabase::standard(32)` = {:#x}",
        header_path.display(),
        cpp_value,
        rust_value
    );
}

#[test]
fn parse_header_constant_smoke() {
    // Sanity-check the header parser against handwritten samples covering
    // the literal-style variants the C++ source actually uses.
    let cases: &[(&str, &str, u64)] = &[
        (
            "constexpr uint64_t kFoo = 0x1'0000'8299ULL;",
            "kFoo",
            0x1_0000_8299,
        ),
        (
            "static constexpr uint64_t kFoo = 0x1'0000'8299ULL;",
            "kFoo",
            0x1_0000_8299,
        ),
        ("constexpr uint64_t kFoo = 0x10000ull;", "kFoo", 0x10000),
        ("constexpr uint64_t kFoo = 0xDEADBEEF;", "kFoo", 0xDEADBEEF),
    ];
    for (input, name, expected) in cases {
        assert_eq!(
            parse_header_constant(input, name),
            *expected,
            "input: {input}"
        );
    }
}
