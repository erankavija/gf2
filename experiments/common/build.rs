//! Stamps the flags and compiler of this build into the crate, because an
//! executable cannot observe them at run time and `rustc` on `PATH` at run time
//! need not be the compiler that built it.

use std::env::var;
use std::path::{Component, Path};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let rustc = std::env::var_os("RUSTC").expect("Cargo sets RUSTC for build scripts");
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("the configured Rust compiler is executable");
    assert!(output.status.success(), "`rustc --version` succeeds");
    let version = String::from_utf8(output.stdout).expect("rustc version is UTF-8");
    stamp("TOOLCHAIN", version.trim());

    let declared_profile = var("PROFILE").expect("Cargo sets PROFILE for build scripts");
    let out_dir = var("OUT_DIR").expect("Cargo sets OUT_DIR for build scripts");
    stamp(
        "PROFILE",
        &named_profile(Path::new(&out_dir)).unwrap_or(declared_profile),
    );
    stamp(
        "OPT_LEVEL",
        &var("OPT_LEVEL").expect("Cargo sets OPT_LEVEL for build scripts"),
    );
    stamp(
        "TARGET",
        &var("TARGET").expect("Cargo sets TARGET for build scripts"),
    );
    stamp(
        "RUSTFLAGS",
        &var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default(),
    );
}

/// Cargo's `PROFILE` collapses a custom profile to `debug` or `release`, which
/// would equate `ci-test` with `release` although they differ in LTO. The
/// directory holding `build/` carries the profile's own name.
fn named_profile(out_dir: &Path) -> Option<String> {
    let components: Vec<_> = out_dir.components().collect();
    let build = components
        .iter()
        .rposition(|component| *component == Component::Normal("build".as_ref()))?;
    match components.get(build.checked_sub(1)?)? {
        Component::Normal(name) => name.to_str().map(str::to_owned),
        _ => None,
    }
}

fn stamp(name: &str, value: &str) {
    println!("cargo:rustc-env=EXPERIMENTS_BUILD_{name}={value}");
}
