//! Compiles the AFF3CT de-rate-matching shim when the `aff3ct` feature is on.
//!
//! The shim is one C++ translation unit with a C ABI. It must see the
//! preprocessor definitions and language standard the pinned AFF3CT static
//! library was built with: the library's class layouts are gated on them.
//! `c077a88b`'s build identity records that library and its flags; the list
//! below mirrors its harness `build.rs`.

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
    let aff3ct = PathBuf::from(env::var("GF2_AFF3CT_ROOT").unwrap_or_else(|_| {
        panic!("GF2_AFF3CT_ROOT must name the pinned AFF3CT tree with its build/lib static library")
    }));
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/aff3ct_depuncture_shim.cpp")
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
    ] {
        build.include(aff3ct.join(include));
    }
    build.compile("aff3ct_depuncture_shim");
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
    println!("cargo:rerun-if-changed=cpp/aff3ct_depuncture_shim.cpp");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_AFF3CT").is_some() {
        build_aff3ct();
    }
}
