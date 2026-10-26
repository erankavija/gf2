//! Compiles the AFF3CT shim and bchlib's `bch.c` into the external arms.
//!
//! The pinned AFF3CT static library's class layouts are gated on the
//! definitions below, so the shim is compiled with the library's own flags, as
//! the `4e732b56` survey Makefile compiles its harness; `bch.c` takes that
//! Makefile's flags too. The library digest and both source revisions reach
//! the executable as environment constants the arm reports.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Preprocessor definitions the pinned AFF3CT static library was built with.
const AFF3CT_DEFINITIONS: [&str; 6] = [
    "AFF3CT_EXT_STRINGS",
    "AFF3CT_MULTI_PREC",
    "AFF3CT_POLAR_BIT_PACKING",
    "MIPP_ENABLE_BACKTRACE",
    "SPU_COLORS",
    "SPU_STACKTRACE",
];

const AFF3CT_LIBRARY: &str = "aff3ct-4.7.0";

fn root(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    PathBuf::from(env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set")))
}

fn first_word(command: &mut Command) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(output.status.success(), "{command:?} failed");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_else(|| panic!("{command:?} printed nothing"))
        .to_owned()
}

fn revision(tree: &Path) -> String {
    first_word(
        Command::new("git")
            .arg("-C")
            .arg(tree)
            .args(["rev-parse", "HEAD"]),
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp/aff3ct_bch_shim.cpp");
    let aff3ct = root("GF2_AFF3CT_ROOT");
    let bchlib = root("GF2_BCHLIB_ROOT");

    let mut shim = cc::Build::new();
    // The pinned AFF3CT and MIPP headers warn under the default warning set.
    shim.cpp(true)
        .warnings(false)
        .file("cpp/aff3ct_bch_shim.cpp")
        .flag("-std=gnu++11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-frecord-gcc-switches")
        .define("NDEBUG", None);
    for definition in AFF3CT_DEFINITIONS {
        shim.define(definition, None);
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
        shim.include(aff3ct.join(include));
    }
    shim.compile("aff3ct_bch_shim");

    let source = bchlib.join("src/bch.c");
    println!("cargo:rerun-if-changed={}", source.display());
    cc::Build::new()
        .file(&source)
        .include(bchlib.join("src"))
        .flag("-std=c11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-frecord-gcc-switches")
        .compile("bchlib");

    let library_dir = aff3ct.join("build/lib");
    let library = library_dir.join(format!("lib{AFF3CT_LIBRARY}.a"));
    println!("cargo:rerun-if-changed={}", library.display());
    println!(
        "cargo:rustc-env=BCH_EXTERNAL_AFF3CT_LIBRARY_SHA256={}",
        first_word(Command::new("sha256sum").arg(&library))
    );
    println!(
        "cargo:rustc-env=BCH_EXTERNAL_AFF3CT_REVISION={}",
        revision(&aff3ct)
    );
    println!(
        "cargo:rustc-env=BCH_EXTERNAL_BCHLIB_REVISION={}",
        revision(&bchlib)
    );
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        aff3ct.join("build/lib/streampu/lib/cpptrace/lib").display()
    );
    for library in [AFF3CT_LIBRARY, "cpptrace", "dwarf", "z", "zstd", "dl"] {
        println!("cargo:rustc-link-lib={library}");
    }
}
