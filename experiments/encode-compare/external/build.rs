//! Compiles the comparator shims against the host-local source trees that
//! `GF2_SRSRAN_ROOT`, `GF2_AFF3CT_ROOT` and `GF2_BCHLIB_ROOT` name, all
//! `-O3 -march=native`.
//! srsRAN's LDPC translation units are compiled directly, which needs none
//! of its other layers; AFF3CT links its static library and must see the
//! definitions that library was built with.

use std::env;
use std::path::PathBuf;

const AFF3CT_DEFINITIONS: [&str; 6] = [
    "AFF3CT_EXT_STRINGS",
    "AFF3CT_MULTI_PREC",
    "AFF3CT_POLAR_BIT_PACKING",
    "MIPP_ENABLE_BACKTRACE",
    "SPU_COLORS",
    "SPU_STACKTRACE",
];

const SRSRAN_SOURCES: [&str; 7] = [
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_generic.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_graph_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_luts_impl.cpp",
    "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp",
    "lib/srsvec/bit.cpp",
];

fn root(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    PathBuf::from(
        env::var(variable).unwrap_or_else(|_| panic!("{variable} must name the staged tree")),
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp");

    let srsran = root("GF2_SRSRAN_ROOT");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/srsran_nr.cpp")
        .flag("-std=c++17")
        .flag("-O3")
        .flag("-march=native")
        .flag("-w")
        .define("NDEBUG", None)
        .include(srsran.join("include"))
        .include(srsran.join("lib"))
        .include(srsran.join("external/fmt/include"));
    for source in SRSRAN_SOURCES {
        build.file(srsran.join(source));
    }
    build.compile("srsran_nr");

    let bchlib = root("GF2_BCHLIB_ROOT");
    cc::Build::new()
        .file("cpp/bchlib.c")
        .file(bchlib.join("src/bch.c"))
        .include(bchlib.join("src"))
        .flag("-std=c99")
        .flag("-O3")
        .flag("-march=native")
        .flag("-w")
        .define("NDEBUG", None)
        .compile("bchlib");

    let aff3ct = root("GF2_AFF3CT_ROOT");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/aff3ct_nr.cpp")
        .flag("-std=gnu++17")
        .flag("-O3")
        .flag("-march=native")
        .flag("-w")
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
    build.compile("aff3ct_nr");
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
}
