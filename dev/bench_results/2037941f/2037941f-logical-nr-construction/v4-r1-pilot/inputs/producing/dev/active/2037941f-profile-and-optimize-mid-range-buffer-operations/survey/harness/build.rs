//! Compiles the pinned ISA-L scalar XOR reference for the `isal` feature.
//!
//! The revision, the three source digests and the compiler flags are the ones
//! `isal-comparator.md` qualifies. The digests are verified before compilation,
//! so a different checkout fails the build instead of producing an arm whose
//! external identity is unknown.

use std::env;
use std::path::PathBuf;

/// ISA-L revision qualified by `isal-comparator.md` (tag `v2.32.1`).
const ISAL_REVISION: &str = "7c3479e0a9dac17f448603ec1ad64c7c625f530c";
/// Pinned content digests, as `(repository-relative path, sha256)`.
const ISAL_PINS: [(&str, &str); 3] = [
    (
        "raid/raid_base.c",
        "4fb636b16cebebadfb52236871421f1a143d3d7e488e7bc9b23b2fc25bd04aeb",
    ),
    (
        "include/raid.h",
        "5e51c4abcd86ade41426cef509c3eeb06b0c0b3f9ef28080ddedb6af02a5cd03",
    ),
    (
        "LICENSE",
        "bc8fd4a3d031e65e05e9c9e2add2c3f336ce527fa85c1e31031c808b58216217",
    ),
];

fn main() {
    println!("cargo:rerun-if-env-changed=GF2_ISAL_SOURCE");
    if env::var_os("CARGO_FEATURE_ISAL").is_none() {
        return;
    }
    let source = PathBuf::from(env::var_os("GF2_ISAL_SOURCE").unwrap_or_else(|| {
        panic!(
            "GF2_ISAL_SOURCE is unset; point it at the pinned ISA-L {ISAL_REVISION} checkout \
             qualified by isal-comparator.md"
        )
    }));
    for (relative, want) in ISAL_PINS {
        let path = source.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let got = hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes));
        assert_eq!(
            got,
            want,
            "{} does not match the digest isal-comparator.md pins",
            path.display()
        );
    }
    cc::Build::new()
        .file(source.join("raid/raid_base.c"))
        .include(source.join("include"))
        .flag("-std=c11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-Werror")
        .opt_level(3)
        .compile("isal_raid_base");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
