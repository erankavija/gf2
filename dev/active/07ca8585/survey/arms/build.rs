//! Compiles the AFF3CT update-rule translation unit of the kernel arms.
//!
//! `cpp/update_rule_shim.cpp` includes no pinned shim: it instantiates
//! AFF3CT's own `tools::Update_rule_*` over AFF3CT's own check-node scan
//! order, which is what an isolated check-node comparison needs and what
//! neither pinned shim exposes. Its C entry points are named `a3u_*`, disjoint
//! from the pinned shims' `a3_*`, so a binary may link both.
//!
//! The flags and definitions are the pinned static library's: its class
//! layouts are gated on the definitions, and a translation unit that omits
//! them corrupts its heap on the first construction. `-g1` adds line tables
//! for profiler attribution; it does not change generated code.

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

fn build_aff3ct() {
    println!("cargo:rerun-if-env-changed=GF2_AFF3CT_ROOT");
    let aff3ct = PathBuf::from(
        env::var("GF2_AFF3CT_ROOT")
            .expect("GF2_AFF3CT_ROOT must name the staged AFF3CT tree for the aff3ct feature"),
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/update_rule_shim.cpp")
        .flag("-std=gnu++11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-g1")
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
    build.compile("aff3ct_update_rule_shim");
    println!("cargo:rerun-if-changed=cpp/update_rule_shim.cpp");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_AFF3CT").is_some() {
        build_aff3ct();
    }
}
