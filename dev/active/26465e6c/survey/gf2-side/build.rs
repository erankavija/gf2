//! Compiles the vendored external kernels into one static archive.
//!
//! Each translation unit uses its upstream's recommended flags: libpopcnt's
//! README asks only for `-O3` and dispatches on CPUID at run time, and Mula's
//! Makefile builds its AVX2 target with `FLAGS_AVX2`. The exact commands and
//! compiler versions are written to `$OUT_DIR/external-build.json`, which the
//! arm binary embeds and prints under `--build-identity`.

use std::env;
use std::path::PathBuf;
use std::process::Command;

struct Unit {
    name: &'static str,
    compiler: &'static str,
    source: &'static str,
    flags: &'static [&'static str],
    include: &'static str,
}

const UNITS: &[Unit] = &[
    Unit {
        name: "libpopcnt_kernels",
        compiler: "gcc",
        source: "csrc/libpopcnt_kernels.c",
        // libpopcnt README "How to compile": `cc -O3 program.c`.
        flags: &["-O3"],
        include: "../vendor/libpopcnt",
    },
    Unit {
        name: "mula_kernels",
        compiler: "g++",
        source: "csrc/mula_kernels.cpp",
        // sse-popcount Makefile: FLAGS, FLAGS_INTEL and FLAGS_AVX2.
        flags: &[
            "-std=c++17",
            "-O2",
            "-Wall",
            "-pedantic",
            "-Wextra",
            "-Wfatal-errors",
            "-mpopcnt",
            "-fabi-version=6",
            "-mavx2",
            "-DHAVE_AVX2_INSTRUCTIONS",
        ],
        include: "../vendor/mula",
    },
];

fn run(command: &mut Command) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("cannot run {command:?}: {error}"));
    assert!(
        output.status.success(),
        "{command:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let mut objects = Vec::new();
    let mut records = Vec::new();
    for unit in UNITS {
        let source = manifest.join(unit.source);
        let include = manifest.join(unit.include);
        let object = out.join(format!("{}.o", unit.name));
        println!("cargo:rerun-if-changed={}", source.display());
        for entry in std::fs::read_dir(&include).expect("vendored include directory") {
            println!(
                "cargo:rerun-if-changed={}",
                entry.expect("entry").path().display()
            );
        }
        let mut args: Vec<String> = unit.flags.iter().map(|flag| (*flag).to_owned()).collect();
        args.extend([
            "-I".to_owned(),
            unit.include.to_owned(),
            "-c".to_owned(),
            unit.source.to_owned(),
            "-o".to_owned(),
        ]);
        run(Command::new(unit.compiler)
            .current_dir(&manifest)
            .args(&args)
            .arg(&object));
        let version = first_line(&run(Command::new(unit.compiler).arg("--version")));
        // The object path lies under the build-specific OUT_DIR, so the record
        // names it symbolically and the embedded record stays build-invariant.
        args.push(format!("$OUT_DIR/{}.o", unit.name));
        let argv = args
            .iter()
            .map(|arg| json_string(arg))
            .collect::<Vec<_>>()
            .join(", ");
        records.push(format!(
            "{{\"unit\": {}, \"compiler\": {}, \"compiler_version\": {}, \"arguments\": [{argv}]}}",
            json_string(unit.name),
            json_string(unit.compiler),
            json_string(&version),
        ));
        objects.push(object);
    }
    let archive = out.join("libsurvey_external.a");
    let _ = std::fs::remove_file(&archive);
    run(Command::new("ar").arg("crs").arg(&archive).args(&objects));
    std::fs::write(
        out.join("external-build.json"),
        format!("{{\"units\": [{}]}}\n", records.join(", ")),
    )
    .expect("write external build record");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=survey_external");
    println!("cargo:rerun-if-changed=build.rs");
}
