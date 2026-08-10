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
    println!("cargo:rerun-if-env-changed=PERMANENT_WAVE_GPU_OFFLOAD_ARCHES");

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
    let offload_arches = offload_arches();
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
        compile_hip_source(&hipcc, &out_dir, &source, &offload_arches);
    }
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("f5_wave_equivalence.hip"),
        "f5_wave_equivalence",
        "PERMANENT_WAVE_GPU_F5_EQUIVALENCE_BIN",
        &offload_arches,
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("f7_three_plane_equivalence.hip"),
        "f7_three_plane_equivalence",
        "PERMANENT_WAVE_GPU_F7_EQUIVALENCE_BIN",
        &offload_arches,
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("wave_gf3_equivalence.hip"),
        "wave_gf3_equivalence",
        "PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN",
        &offload_arches,
    );
    publish_hip_executable(
        &hipcc,
        &out_dir,
        &hip_root.join("wave_gf7_equivalence.hip"),
        "wave_gf7_equivalence",
        "PERMANENT_WAVE_GPU_WAVE_GF7_EQUIVALENCE_BIN",
        &offload_arches,
    );
}

fn offload_arches() -> Vec<String> {
    let raw =
        env::var("PERMANENT_WAVE_GPU_OFFLOAD_ARCHES").unwrap_or_else(|_| "gfx1030".to_owned());
    let arches = raw
        .split(',')
        .map(str::trim)
        .filter(|arch| !arch.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(
        !arches.is_empty(),
        "PERMANENT_WAVE_GPU_OFFLOAD_ARCHES must contain at least one architecture"
    );
    assert!(
        arches.iter().all(|arch| {
            arch.starts_with("gfx")
                && arch.len() > 3
                && arch
                    .bytes()
                    .skip(3)
                    .all(|byte| byte.is_ascii_alphanumeric())
        }),
        "PERMANENT_WAVE_GPU_OFFLOAD_ARCHES must be comma-separated gfx targets"
    );
    arches
}

fn hipcc_args(arches: &[String], tail: &[&str]) -> Vec<String> {
    arches
        .iter()
        .map(|arch| format!("--offload-arch={arch}"))
        .chain(tail.iter().map(|argument| (*argument).to_owned()))
        .collect()
}

fn report_hipcc(hipcc: &Path, args: &[String]) {
    let mut command = hipcc.display().to_string();
    for argument in args {
        command.push(' ');
        command.push_str(argument);
    }
    println!("cargo:warning=permanent-wave-gpu invoking {command}");
}

fn compile_hip_source(hipcc: &Path, out_dir: &Path, source: &Path, arches: &[String]) {
    let object_name = source
        .strip_prefix("hip")
        .expect("discovered HIP source stays below hip/")
        .to_string_lossy()
        .replace(['/', '\\'], "_");
    let object = out_dir.join(format!("{object_name}.o"));
    let mut args = hipcc_args(arches, &["-O3", "-fPIC", "-c"]);
    args.push(source.to_string_lossy().into_owned());
    args.push("-o".to_owned());
    args.push(object.display().to_string());
    report_hipcc(hipcc, &args);
    let status = Command::new(hipcc)
        .args(&args)
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
    arches: &[String],
) {
    assert!(
        source.is_file(),
        "the HIP feature requires {}",
        source.display()
    );
    let executable = out_dir.join(executable_name);
    let mut args = hipcc_args(arches, &["-O3"]);
    args.push(source.to_string_lossy().into_owned());
    args.push("-o".to_owned());
    args.push(executable.display().to_string());
    report_hipcc(hipcc, &args);
    let status = Command::new(hipcc)
        .args(&args)
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
