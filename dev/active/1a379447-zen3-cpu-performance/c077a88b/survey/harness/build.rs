//! Compiles the enabled external decoder shims of the survey harness.
//!
//! Each external adapter is one C++ translation unit that speaks a C ABI.
//! Its feature gates both the shim and the arm binary that consumes it, so
//! the harness builds on a host that stages none of the externals. The
//! AFF3CT definitions below are mandatory: the pinned static library's class
//! layouts are gated on them and a translation unit that omits them corrupts
//! its heap on the first decoder construction.

use std::env;
use std::path::PathBuf;

/// Preprocessor definitions the pinned AFF3CT static library was built with.
const AFF3CT_DEFINITIONS: [&str; 6] = [
    "AFF3CT_EXT_STRINGS",
    "AFF3CT_MULTI_PREC",
    "AFF3CT_POLAR_BIT_PACKING",
    "MIPP_ENABLE_BACKTRACE",
    "SPU_COLORS",
    "SPU_STACKTRACE",
];

fn root(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    PathBuf::from(env::var(variable).unwrap_or_else(|_| {
        panic!("{variable} must name the staged external tree for this feature")
    }))
}

fn build_aff3ct() {
    let aff3ct = root("GF2_AFF3CT_ROOT");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/aff3ct_shim.cpp")
        .flag("-std=gnu++11")
        .flag("-O3")
        .flag("-march=native")
        .define("NDEBUG", None);
    for definition in AFF3CT_DEFINITIONS {
        build.define(definition, None);
    }
    for include in [
        "include",
        "src",
        "lib/streampu/src",
        "lib/MIPP/include",
        "lib/streampu/include",
        "lib/streampu/lib/cli/src",
        "lib/streampu/lib/json/single_include",
        "lib/streampu/lib/rang/include",
        "lib/streampu/lib/cpptrace/include",
        "build/lib/streampu/include",
    ] {
        build.include(aff3ct.join(include));
    }
    build.compile("aff3ct_shim");
    println!(
        "cargo:rustc-link-search=native={}",
        aff3ct.join("build/lib").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        aff3ct.join("build/lib/streampu/lib/cpptrace/lib").display()
    );
    for library in ["aff3ct-4.7.0", "cpptrace", "dwarf", "z", "zstd", "dl"] {
        println!("cargo:rustc-link-lib={library}");
    }
    println!("cargo:rerun-if-changed=cpp/aff3ct_shim.cpp");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_AFF3CT").is_some() {
        build_aff3ct();
    }
}
