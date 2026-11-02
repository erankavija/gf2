//! Stamps the flags and compiler of this build into the crate, because an
//! executable cannot observe them at run time and `rustc` on `PATH` at run time
//! need not be the compiler that built it.

use std::collections::BTreeMap;
use std::env::var;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[path = "src/build_profile.rs"]
mod build_profile;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/build_profile.rs");
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
    let profile = named_profile(Path::new(&out_dir)).unwrap_or(declared_profile);
    stamp("PROFILE", &profile);
    let manifest_dir = var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let environment: BTreeMap<_, _> = std::env::vars()
        .filter(|(name, _)| name.starts_with(build_profile::ENVIRONMENT_PREFIX))
        .collect();
    let settings = build_profile::resolve(
        &profile,
        &configuration_layers(Path::new(&manifest_dir)),
        &environment,
    )
    .unwrap_or_else(|reason| panic!("cannot stamp build flags: {reason}"));
    for name in &settings.consulted_environment {
        println!("cargo:rerun-if-env-changed={name}");
    }
    stamp("LTO", &settings.lto);
    stamp(
        "CODEGEN_UNITS",
        &settings
            .codegen_units
            .map(|units| units.to_string())
            .unwrap_or_default(),
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

/// Cargo's files holding profiles, from lowest to highest precedence: the
/// workspace manifest, the Cargo home configuration, then the configuration of
/// each directory from the filesystem root down to this package.
fn configuration_layers(package: &Path) -> Vec<toml::Table> {
    let read = |path: &Path| -> Option<toml::Table> {
        let text = fs::read_to_string(path).ok()?;
        println!("cargo:rerun-if-changed={}", path.display());
        Some(
            text.parse()
                .unwrap_or_else(|error| panic!("{} is invalid: {error}", path.display())),
        )
    };
    let manifest = package.ancestors().find_map(|directory| {
        read(&directory.join("Cargo.toml")).filter(|manifest| manifest.contains_key("workspace"))
    });
    let home = var("CARGO_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| var("HOME").ok().map(|home| Path::new(&home).join(".cargo")));
    let mut directories: Vec<_> = package
        .ancestors()
        .map(|path| path.join(".cargo"))
        .collect();
    directories.extend(home);
    directories.reverse();
    let configurations = directories.iter().filter_map(|directory| {
        // Cargo prefers the extensionless name when both exist.
        read(&directory.join("config")).or_else(|| read(&directory.join("config.toml")))
    });
    manifest.into_iter().chain(configurations).collect()
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
