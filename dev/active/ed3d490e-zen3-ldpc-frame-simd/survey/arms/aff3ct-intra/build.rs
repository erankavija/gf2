//! Compiles the AFF3CT translation unit of the sum-product INTRA arm.
//!
//! The flags and definitions are those of the `3be770d5` build script: the
//! pinned static library's class layouts are gated on the definitions.
//! `-frecord-gcc-switches` leaves the compiler's options in the executable,
//! and `-MD` the files it read, which cargo's dep-info then names.

mod make_rules;

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
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

/// Directory of the path dependency `name`, read from this package's
/// manifest, which alone counts the directory levels to the harness crates.
fn dependency_dir(name: &str) -> PathBuf {
    let manifest = fs::read_to_string("Cargo.toml").expect("the package manifest");
    let prefix = format!("{name} = {{ path = \"");
    let path = manifest
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("the manifest declares no path dependency {name}"));
    PathBuf::from(path)
}

fn revision(aff3ct: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(aff3ct)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git reads the AFF3CT revision");
    assert!(output.status.success(), "GF2_AFF3CT_ROOT is no git tree");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn build_aff3ct() {
    println!("cargo:rerun-if-env-changed=GF2_AFF3CT_ROOT");
    let aff3ct = PathBuf::from(
        env::var("GF2_AFF3CT_ROOT").expect("GF2_AFF3CT_ROOT must name the staged AFF3CT tree"),
    );
    let throughput_shim = dependency_dir("ldpc-throughput-harness").join("cpp");
    let survey_shim = dependency_dir("ldpc-survey-harness").join("cpp");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/intra_shim.cpp")
        .flag("-std=gnu++11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-g1")
        .flag("-MD")
        .flag("-frecord-gcc-switches")
        .define("NDEBUG", None)
        .include(&throughput_shim)
        .include(&survey_shim);
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
    build.compile("aff3ct_intra_shim");
    rerun_if_compiler_input_changed();

    let library_dir = aff3ct.join("build/lib");
    let library = library_dir.join(format!("lib{AFF3CT_LIBRARY}.a"));
    let digest = Sha256::digest(fs::read(&library).expect("the AFF3CT static library"));
    let digest: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    println!("cargo:rustc-env=LDPC_ARMS_AFF3CT_LIBRARY_SHA256={digest}");
    println!(
        "cargo:rustc-env=LDPC_ARMS_AFF3CT_REVISION={}",
        revision(&aff3ct)
    );
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        aff3ct.join("build/lib/streampu/lib/cpptrace/lib").display()
    );
    for name in [AFF3CT_LIBRARY, "cpptrace", "dwarf", "z", "zstd", "dl"] {
        println!("cargo:rustc-link-lib={name}");
    }
    println!("cargo:rerun-if-changed={}", library.display());
}

/// Names to cargo each prerequisite of the make rules `-MD` writes beside the
/// objects.
fn rerun_if_compiler_input_changed() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let mut files = 0;
    for entry in fs::read_dir(&out_dir).expect("the build script output directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|extension| extension == "d") {
            let rules = fs::read_to_string(&path).expect("a compiler dependency file");
            for input in make_rules::prerequisites(&rules) {
                println!("cargo:rerun-if-changed={input}");
            }
            files += 1;
        }
    }
    assert!(files > 0, "the compiler wrote no dependency file");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=make_rules.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    build_aff3ct();
}
