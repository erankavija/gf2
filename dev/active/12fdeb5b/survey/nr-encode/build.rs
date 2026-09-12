//! Compiles the external encoder shims the enabled features select.
//!
//! Each shim is one C++ translation unit with a C ABI. The AFF3CT shim links
//! the pinned static library and must see the preprocessor definitions and
//! language standard that library was built with, because its class layouts
//! are gated on them; `dev/active/c077a88b`'s build identity records them and
//! the list below mirrors `eda07788`'s harness. The srsRAN shim compiles the
//! LDPC translation units from source, because srsRAN's own CMake
//! configuration stops on a host without MbedTLS.

use std::env;
use std::path::{Path, PathBuf};

/// Preprocessor definitions the pinned AFF3CT static library was built with.
const AFF3CT_DEFINITIONS: [&str; 6] = [
    "AFF3CT_EXT_STRINGS",
    "AFF3CT_MULTI_PREC",
    "AFF3CT_POLAR_BIT_PACKING",
    "MIPP_ENABLE_BACKTRACE",
    "SPU_COLORS",
    "SPU_STACKTRACE",
];

/// The srsRAN translation units the LDPC encoder and rate matcher need.
///
/// `srsran-build.sh` digests exactly this list. The AVX2 encoder backend is
/// compiled for every x86-64 host; the shim selects it at run time only when
/// the CPU reports AVX2, which is the choice srsRAN's own
/// `ldpc_encoder_factory_sw("auto")` makes.
const SRSRAN_SOURCES: [&str; 7] = [
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_generic.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_graph_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_luts_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp",
    "lib/srsvec/bit.cpp",
];

fn external_root(variable: &str) -> PathBuf {
    PathBuf::from(
        env::var(variable)
            .unwrap_or_else(|_| panic!("{variable} must name the pinned source tree")),
    )
}

fn build_aff3ct() {
    println!("cargo:rerun-if-env-changed=GF2_AFF3CT_ROOT");
    let aff3ct = external_root("GF2_AFF3CT_ROOT");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/aff3ct_encode_shim.cpp")
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
    build.compile("aff3ct_encode_shim");
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
    println!("cargo:rerun-if-changed=cpp/aff3ct_encode_shim.cpp");
}

fn build_srsran() {
    println!("cargo:rerun-if-env-changed=GF2_SRSRAN_ROOT");
    let srsran = external_root("GF2_SRSRAN_ROOT");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/srsran_encode_shim.cpp")
        .flag("-std=c++17")
        .flag("-O3")
        .flag("-march=native")
        .define("NDEBUG", None)
        .include(srsran.join("include"))
        .include(srsran.join("lib"))
        .include(srsran.join("external/fmt/include"));
    for source in SRSRAN_SOURCES {
        let path: &Path = source.as_ref();
        build.file(srsran.join(path));
    }
    build.compile("srsran_encode_shim");
    println!("cargo:rerun-if-changed=cpp/srsran_encode_shim.cpp");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_AFF3CT").is_some() {
        build_aff3ct();
    }
    if env::var_os("CARGO_FEATURE_SRSRAN").is_some() {
        build_srsran();
    }
}
