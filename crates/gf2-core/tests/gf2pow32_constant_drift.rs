//! Drift check between the GF(2^32) Conway polynomial (`@/citation/Lubeck2024`)
//! in `PrimitivePolynomialDatabase::standard(32)` and the C++ header
//! `benchmarks/reference/gf2pow32_constants.h`, parsed at test time.

use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

/// Extracts the hex literal that defines `name` in the header text; `'` digit
/// separators and trailing `U`/`L` suffixes are tolerated.
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
