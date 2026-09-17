//! Compiles the M4RI comparator shim and links the qualified external build.
//!
//! The installed library's digest and the compiler that produced it are read
//! from the committed qualification record and the install's own build record
//! rather than typed here, so the arm links the one build those records
//! qualify and a different install fails the build instead of producing an arm
//! whose external identity is unknown.

use std::env;
use std::path::{Path, PathBuf};

/// Qualification record, relative to the manifest, read for the pinned digest.
const PROBE_RECORD: &str = "../../m4ri-probe-record.txt";
/// Comparator shim compiled at the qualification's own flags.
const SHIM_SOURCE: &str = "../m4ri_matvec_arm.c";
/// Line prefix carrying the installed library's content identity.
const LIBRARY_DIGEST_KEY: &str = "installed libm4ri.so sha256:";
/// Line prefix of the install's own record naming the compiler that built it.
const COMPILER_KEY: &str = "compiler_command=";
/// Environment variable naming the qualified install prefix.
const PREFIX_VAR: &str = "GF2_M4RI_PREFIX";

fn main() {
    println!("cargo:rerun-if-env-changed={PREFIX_VAR}");
    if env::var_os("CARGO_FEATURE_M4RI").is_none() {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("a manifest directory"));
    let record = manifest.join(PROBE_RECORD);
    println!("cargo:rerun-if-changed={}", record.display());
    let want = pinned_digest(&record);

    let prefix = PathBuf::from(env::var_os(PREFIX_VAR).unwrap_or_else(|| {
        panic!("{PREFIX_VAR} is unset; point it at the install {} qualifies", record.display())
    }));
    let library = prefix.join("lib/libm4ri.so");
    println!("cargo:rerun-if-changed={}", library.display());
    let bytes = std::fs::read(&library)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", library.display()));
    let got = hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes));
    assert_eq!(
        got,
        want,
        "{} does not match the digest {} pins",
        library.display(),
        record.display()
    );

    let shim = manifest.join(SHIM_SOURCE);
    println!("cargo:rerun-if-changed={}", shim.display());
    cc::Build::new()
        .compiler(install_compiler(&prefix))
        .file(&shim)
        .include(prefix.join("include"))
        .flag("-std=c11")
        .flag("-O3")
        .flag("-march=native")
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-Werror")
        .opt_level(3)
        .compile("gf2_m4ri_matvec_arm");

    let lib_dir = prefix.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=m4ri");
}

fn pinned_digest(record: &Path) -> String {
    field(record, LIBRARY_DIGEST_KEY)
}

/// The compiler the install's own build record names.
fn install_compiler(prefix: &Path) -> String {
    let record = prefix.join("gf2-m4ri-build-record.txt");
    println!("cargo:rerun-if-changed={}", record.display());
    field(&record, COMPILER_KEY)
}

fn field(record: &Path, key: &str) -> String {
    let text = std::fs::read_to_string(record)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", record.display()));
    text.lines()
        .find_map(|line| line.trim().strip_prefix(key))
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|| panic!("{} carries no {key} line", record.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
