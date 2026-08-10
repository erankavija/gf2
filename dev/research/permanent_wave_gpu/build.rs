use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Watch the directory as well as every discovered source so adding a
    // candidate kernel triggers discovery without a build-script edit.
    println!("cargo:rerun-if-changed=hip");
    println!("cargo:rerun-if-env-changed=ROCM_PATH");

    if env::var_os("CARGO_FEATURE_HIP").is_none() {
        return;
    }

    let hip_root = Path::new("hip");
    let sources = collect_hip_sources(hip_root);
    assert!(
        !sources.is_empty(),
        "the HIP feature requires at least one source under {}",
        hip_root.display()
    );

    let rocm_path = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".to_owned());
    let hipcc = Path::new(&rocm_path).join("bin/hipcc");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let shared_mapping = hip_root.join("wave_ryser_mapping.h");
    assert!(
        shared_mapping.is_file(),
        "the HIP feature requires {}",
        shared_mapping.display()
    );
    println!("cargo:rerun-if-changed={}", shared_mapping.display());
    for source in sources {
        println!("cargo:rerun-if-changed={}", source.display());
        compile_hip_source(&hipcc, &out_dir, &source);
    }
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("f5_wave_equivalence.hip"),
        "f5_wave_equivalence",
        "PERMANENT_WAVE_GPU_F5_EQUIVALENCE_BIN",
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("f7_three_plane_equivalence.hip"),
        "f7_three_plane_equivalence",
        "PERMANENT_WAVE_GPU_F7_EQUIVALENCE_BIN",
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("wave_gf3_equivalence.hip"),
        "wave_gf3_equivalence",
        "PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN",
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("wave_gf7_equivalence.hip"),
        "wave_gf7_equivalence",
        "PERMANENT_WAVE_GPU_WAVE_GF7_EQUIVALENCE_BIN",
    );
}

fn compile_hip_source(hipcc: &Path, out_dir: &Path, source: &Path) {
    let object_name = source
        .strip_prefix("hip")
        .expect("discovered HIP source stays below hip/")
        .to_string_lossy()
        .replace(['/', '\\'], "_");
    let object = out_dir.join(format!("{object_name}.o"));
    let status = Command::new(hipcc)
        .args(["--offload-arch=gfx1030", "-O3", "-fPIC", "-c"])
        .arg(source)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap_or_else(|error| panic!("failed to invoke {}: {error}", hipcc.display()));
    assert!(
        status.success(),
        "{} failed while compiling {}",
        hipcc.display(),
        source.display()
    );
}

fn publish_hip_executable(
    hipcc: &Path,
    out_dir: &Path,
    source: &Path,
    executable_name: &str,
    environment_name: &str,
) {
    assert!(
        source.is_file(),
        "the HIP feature requires {}",
        source.display()
    );
    let executable = out_dir.join(executable_name);
    let status = Command::new(hipcc)
        .args(["--offload-arch=gfx1030", "-O3"])
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap_or_else(|error| panic!("failed to invoke {}: {error}", hipcc.display()));
    assert!(
        status.success(),
        "{} failed while linking {}",
        hipcc.display(),
        source.display()
    );
    println!(
        "cargo:rustc-env={environment_name}={}",
        executable.display()
    );
}

fn collect_hip_sources(directory: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_hip_sources_into(directory, &mut sources);
    sources.sort();
    sources
}

fn collect_hip_sources_into(directory: &Path, sources: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read HIP directory entry: {error}"))
            .path();
        if path.is_dir() {
            collect_hip_sources_into(&path, sources);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("hip") {
            sources.push(path);
        }
    }
}
