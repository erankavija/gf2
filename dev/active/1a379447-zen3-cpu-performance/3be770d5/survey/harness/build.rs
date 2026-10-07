//! Compiles the AFF3CT translation unit of the throughput harness.
//!
//! `cpp/throughput_shim.cpp` includes the `c077a88b` shim unchanged and adds a
//! clone entry, so the flags and definitions below are that shim's: the pinned
//! static library's class layouts are gated on the definitions, and a
//! translation unit that omits them corrupts its heap on the first decoder
//! construction. `-g1` adds line tables for profiler attribution; it does not
//! change generated code.

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

/// The `c077a88b` shim this unit includes.
const SURVEY_SHIM_DIR: &str = "../../../c077a88b/survey/harness/cpp";

fn build_aff3ct() {
    println!("cargo:rerun-if-env-changed=GF2_AFF3CT_ROOT");
    let aff3ct = PathBuf::from(
        env::var("GF2_AFF3CT_ROOT")
            .expect("GF2_AFF3CT_ROOT must name the staged AFF3CT tree for the aff3ct feature"),
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/throughput_shim.cpp")
        .flag("-std=gnu++11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-g1")
        .define("NDEBUG", None)
        .include(SURVEY_SHIM_DIR);
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
    build.compile("aff3ct_throughput_shim");
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
    println!("cargo:rerun-if-changed=cpp/throughput_shim.cpp");
    println!("cargo:rerun-if-changed={SURVEY_SHIM_DIR}/aff3ct_shim.cpp");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_AFF3CT").is_some() {
        build_aff3ct();
    }
}
