//! Compiles the M4RI comparator shim and links the qualified external build.
//!
//! The installed library's digest and the compiler that produced it are read
//! from the committed record of the selected build level and the install's own
//! build record rather than typed here, so the arm links the one build those
//! records qualify and a different install fails the build instead of
//! producing an arm whose external identity is unknown. The same digest
//! reaches the arm as `GF2_M4RI_PINNED_SHA256`, which it checks against the
//! object the loader actually gives it.
//!
//! `GF2_M4RI_LEVEL` selects the level, `host` when unset. The shim holds the
//! charged packing and unpacking loops and M4RI's inline accessors, so it is
//! compiled for the host only at the `host` level, whose library is.

use std::env;
use std::path::{Path, PathBuf};

/// Record of each build level, relative to the manifest, read for the pinned
/// digest.
const LEVEL_RECORDS: [(&str, &str); 2] = [
    ("host", "../../m4ri-probe-record.txt"),
    ("portable", "../../m4ri-portable-build-record.txt"),
];
/// Environment variable naming the build level.
const LEVEL_VAR: &str = "GF2_M4RI_LEVEL";
/// The level whose library and shim are compiled with [`HOST_FLAG`].
const HOST_LEVEL: &str = "host";
/// The host-targeting compiler option.
const HOST_FLAG: &str = "-march=native";
/// Option prefixes that select an instruction set or tune for a processor.
const TARGETING_PREFIXES: [&str; 3] = ["-march=", "-mtune=", "-mcpu="];
/// Line prefix of the install's own record carrying the library's flags.
const BUILD_FLAGS_KEY: &str = "build_flags=";
/// Comparator shim compiled at the linked library's own targeting level.
const SHIM_SOURCE: &str = "../m4ri_matvec_arm.c";
/// Line prefix carrying the installed library's content identity.
const LIBRARY_DIGEST_KEY: &str = "installed libm4ri.so sha256:";
/// Line prefix of the install's own record naming the compiler that built it.
const COMPILER_KEY: &str = "compiler_command=";
/// Environment variable naming the qualified install prefix.
const PREFIX_VAR: &str = "GF2_M4RI_PREFIX";

fn main() {
    println!("cargo:rerun-if-env-changed={PREFIX_VAR}");
    println!("cargo:rerun-if-env-changed={LEVEL_VAR}");
    if env::var_os("CARGO_FEATURE_M4RI").is_none() {
        return;
    }
    let level = env::var(LEVEL_VAR).unwrap_or_else(|_| HOST_LEVEL.to_owned());
    let (_, level_record) = LEVEL_RECORDS
        .into_iter()
        .find(|(id, _)| *id == level)
        .unwrap_or_else(|| panic!("{LEVEL_VAR}={level} names no comparator build level"));
    println!("cargo:rustc-env={LEVEL_VAR}={level}");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("a manifest directory"));
    let record = manifest.join(level_record);
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

    // The arm links M4RI dynamically, so it verifies at run time that the
    // object the loader gave it is this one. The pin travels from the record
    // to the executable here rather than being typed into the arm.
    println!("cargo:rustc-env=GF2_M4RI_PINNED_SHA256={want}");

    // The install's own record states how its library was compiled; a level
    // that disagrees with it would label the arm with the wrong targeting.
    let host = level == HOST_LEVEL;
    let library_flags = install_field(&prefix, BUILD_FLAGS_KEY);
    assert_eq!(
        library_flags.split_whitespace().any(|flag| flag == HOST_FLAG),
        host,
        "{} was built with {library_flags:?}, which is not the {level} level",
        library.display()
    );

    let shim = manifest.join(SHIM_SOURCE);
    println!("cargo:rerun-if-changed={}", shim.display());
    let mut build = cc::Build::new();
    build
        .compiler(install_field(&prefix, COMPILER_KEY))
        .file(&shim)
        .include(prefix.join("include"))
        .flag("-std=c11")
        .flag("-O3");
    if host {
        build.flag(HOST_FLAG);
    }
    build.flag("-Wall").flag("-Wextra").flag("-Werror").opt_level(3);

    // The resolved command includes what the `cc` crate adds from its own
    // defaults and from an ambient `CFLAGS`, so the arm reports the shim's
    // real compile line and a portable shim cannot pick up a targeting option.
    let tool = build.get_compiler();
    let command: Vec<String> = std::iter::once(tool.path().display().to_string())
        .chain(tool.args().iter().map(|argument| argument.to_string_lossy().into_owned()))
        .collect();
    let targeted: Vec<&String> = command
        .iter()
        .filter(|argument| TARGETING_PREFIXES.iter().any(|prefix| argument.starts_with(prefix)))
        .collect();
    assert_eq!(
        targeted.is_empty(),
        !host,
        "the {level} shim would compile with targeting options {targeted:?}"
    );
    println!("cargo:rustc-env=GF2_M4RI_SHIM_COMMAND={}", command.join(" "));
    build.compile("gf2_m4ri_matvec_arm");

    let lib_dir = prefix.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=m4ri");
}

fn pinned_digest(record: &Path) -> String {
    field(record, LIBRARY_DIGEST_KEY)
}

/// One field of the install's own build record.
fn install_field(prefix: &Path, key: &str) -> String {
    let record = prefix.join("gf2-m4ri-build-record.txt");
    println!("cargo:rerun-if-changed={}", record.display());
    field(&record, key)
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
