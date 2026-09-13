//! Links the byte-field C shim and the pinned external libraries.
//!
//! `stage-externals.sh` places the verified library prefix at
//! `$GF2_SURVEY_EXT/prefix` and the survey Makefile builds
//! `libbytefieldext.a` into `$GF2_SURVEY_BUILD`. Both are required: this
//! build script neither downloads nor compiles anything, so a missing
//! artifact fails here with the command that produces it rather than at
//! link time.

use std::path::{Path, PathBuf};

fn main() {
    let staging = required_dir(
        "GF2_SURVEY_EXT",
        "dev/active/6c6b09b1/survey/stage-externals.sh",
    );
    let shim_dir = required_dir("GF2_SURVEY_BUILD", "make -C dev/active/6c6b09b1/survey");
    let shim = shim_dir.join("libbytefieldext.a");
    require(&shim, "make -C dev/active/6c6b09b1/survey");
    let lib = staging.join("prefix").join("lib");
    require(&lib, "dev/active/6c6b09b1/survey/stage-externals.sh");

    println!("cargo:rustc-link-search=native={}", shim_dir.display());
    println!("cargo:rustc-link-lib=static=bytefieldext");
    println!("cargo:rustc-link-search=native={}", lib.display());
    for library in ["isal", "gf_complete", "m4rie", "m4ri"] {
        println!("cargo:rustc-link-lib=dylib={library}");
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    println!("cargo:rerun-if-changed={}", shim.display());
    println!("cargo:rerun-if-env-changed=GF2_SURVEY_EXT");
    println!("cargo:rerun-if-env-changed=GF2_SURVEY_BUILD");
}

fn required_dir(variable: &str, producer: &str) -> PathBuf {
    match std::env::var(variable) {
        Ok(value) => PathBuf::from(value),
        Err(_) => panic!("{variable} must name a directory; run {producer} first"),
    }
}

fn require(path: &Path, producer: &str) {
    if !path.exists() {
        panic!("{} is missing; run {producer} first", path.display());
    }
}
