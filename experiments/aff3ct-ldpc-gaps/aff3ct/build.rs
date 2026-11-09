mod make_rules;

use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
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

/// First word `command` prints.
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

fn build_aff3ct() {
    println!("cargo:rerun-if-env-changed=GF2_AFF3CT_ROOT");
    let aff3ct = PathBuf::from(
        env::var("GF2_AFF3CT_ROOT")
            .expect("GF2_AFF3CT_ROOT must name the staged AFF3CT tree for the AFF3CT arm"),
    );
    assert!(
        aff3ct.join("build/lib/libaff3ct-4.7.0.a").is_file(),
        "GF2_AFF3CT_ROOT must contain build/lib/libaff3ct-4.7.0.a"
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/throughput_shim.cpp")
        .flag("-std=gnu++17")
        .flag("-O3")
        .flag("-march=native")
        .flag("-g1")
        .flag("-MD")
        .flag("-frecord-gcc-switches")
        .define("NDEBUG", None)
        .include("cpp");
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
    rerun_if_compiler_input_changed();

    let library_dir = aff3ct.join("build/lib");
    let library = library_dir.join(format!("lib{AFF3CT_LIBRARY}.a"));
    println!(
        "cargo:rustc-env=LDPC_THROUGHPUT_AFF3CT_LIBRARY_SHA256={}",
        first_word(
            Command::new("sha256sum")
                .stdin(File::open(&library).expect("the AFF3CT static library"))
        )
    );
    println!(
        "cargo:rustc-env=LDPC_THROUGHPUT_AFF3CT_REVISION={}",
        first_word(
            Command::new("git")
                .arg("-C")
                .arg(&aff3ct)
                .args(["rev-parse", "HEAD"])
        )
    );
    println!("cargo:rerun-if-changed={}", library.display());
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        aff3ct.join("build/lib/streampu/lib/cpptrace/lib").display()
    );
    for library in [AFF3CT_LIBRARY, "cpptrace", "dwarf", "z", "zstd", "dl"] {
        println!("cargo:rustc-link-lib={library}");
    }
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
    build_aff3ct();
}
