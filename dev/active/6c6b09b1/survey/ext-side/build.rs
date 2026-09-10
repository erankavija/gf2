//! Links the byte-field C shim and the pinned external libraries.
//!
//! `fetch-build.sh` builds the libraries into `<staging>/prefix` and the
//! survey Makefile builds `libbytefieldext.a` beside this crate. Both paths
//! are required: this build script neither downloads nor compiles anything,
//! so a missing artifact fails here with the command that produces it rather
//! than at link time.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let survey = manifest.parent().expect("survey directory").to_path_buf();
    let staging = match std::env::var("GF2_SURVEY_EXT") {
        Ok(value) => PathBuf::from(value),
        Err(_) => panic!(
            "GF2_SURVEY_EXT must name the external staging directory; \
             run dev/active/6c6b09b1/survey/fetch-build.sh first"
        ),
    };
    let shim = survey.join("libbytefieldext.a");
    if !shim.exists() {
        panic!(
            "{} is missing; run `make -C {}` first",
            shim.display(),
            survey.display()
        );
    }
    let prefix = staging.join("prefix");
    require(&prefix.join("lib"));

    println!("cargo:rustc-link-search=native={}", survey.display());
    println!("cargo:rustc-link-lib=static=bytefieldext");
    println!("cargo:rustc-link-search=native={}", prefix.join("lib").display());
    for library in ["isal", "gf_complete", "m4rie", "m4ri"] {
        println!("cargo:rustc-link-lib=dylib={library}");
    }
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,{}",
        prefix.join("lib").display()
    );
    println!("cargo:rerun-if-changed={}", shim.display());
    println!("cargo:rerun-if-env-changed=GF2_SURVEY_EXT");
}

fn require(path: &Path) {
    if !path.exists() {
        panic!(
            "{} is missing; run dev/active/6c6b09b1/survey/fetch-build.sh first",
            path.display()
        );
    }
}
