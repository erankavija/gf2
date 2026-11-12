//! Compiles the comparator shims against the host-local source trees that
//! `GF2_SRSRAN_ROOT`, `GF2_AFF3CT_ROOT` and `GF2_BCHLIB_ROOT` name, all
//! `-O3 -march=native`, and, when `GF2_M4RI_ROOT` names an M4RI install
//! prefix, the M4RI shim, enabling `cfg(gf2_m4ri)`.
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

/// AFF3CT's include directories, below its tree.
const AFF3CT_INCLUDES: [&str; 10] = [
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

/// bchlib's const-parameter builds, which `src/main.rs` declares under the
/// same prefixes: the DVB-T2 rate-1/2 outer codes, short then normal frame
/// (`@/citation/Etsi2015` Tables 6a and 6b), and the cutover contract's one
/// byte-aligned primitive row, $m = 7$, $t = 10$.
const BCHLIB_CONST: [(u32, u32); 3] = [(14, 12), (16, 12), (7, 10)];

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

    // bchlib's runtime-parameter build, then one CONFIG_BCH_CONST_PARAMS
    // build per (m, t) of BCHLIB_CONST, each under its own symbol prefix.
    let bchlib = root("GF2_BCHLIB_ROOT");
    let configurations = std::iter::once(("gf2_bchlib".to_owned(), None)).chain(
        BCHLIB_CONST
            .iter()
            .map(|&(m, t)| (format!("gf2_bchlib_m{m}t{t}"), Some((m, t)))),
    );
    for (prefix, constant) in configurations {
        let mut build = cc::Build::new();
        build
            .file("cpp/bchlib.c")
            .include(bchlib.join("src"))
            .flag("-std=c99")
            .flag("-O3")
            .flag("-march=native")
            .flag("-w")
            .define("NDEBUG", None)
            .define("GF2_BCH_PREFIX", prefix.as_str());
        if let Some((m, t)) = constant {
            build
                .define("CONFIG_BCH_CONST_PARAMS", None)
                .define("CONFIG_BCH_CONST_M", m.to_string().as_str())
                .define("CONFIG_BCH_CONST_T", t.to_string().as_str());
        }
        build.compile(&prefix);
    }

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
    for include in AFF3CT_INCLUDES {
        build.include(aff3ct.join(include));
    }
    build.compile("aff3ct_nr");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/aff3ct_bch.cpp")
        .flag("-std=gnu++17")
        .flag("-O3")
        .flag("-march=native")
        .flag("-w")
        .define("NDEBUG", None);
    for definition in AFF3CT_DEFINITIONS {
        build.define(definition, None);
    }
    for include in AFF3CT_INCLUDES {
        build.include(aff3ct.join(include));
    }
    build.compile("aff3ct_bch");
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

    println!("cargo::rustc-check-cfg=cfg(gf2_m4ri)");
    println!("cargo:rerun-if-env-changed=GF2_M4RI_ROOT");
    if let Ok(prefix) = env::var("GF2_M4RI_ROOT") {
        let prefix = PathBuf::from(prefix);
        cc::Build::new()
            .file("cpp/m4ri_bch.c")
            .include(prefix.join("include"))
            .flag("-std=c99")
            .flag("-O3")
            .flag("-march=native")
            .flag("-w")
            .define("NDEBUG", None)
            .compile("m4ri_bch");
        let lib = prefix.join("lib");
        println!("cargo:rustc-link-search=native={}", lib.display());
        println!("cargo:rustc-link-lib=m4ri");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
        println!("cargo:rustc-cfg=gf2_m4ri");
    }
}
