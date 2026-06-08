use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The multi-arch gfx target list (design doc §6). Kept in sync with
/// `GfxTarget::ALL` in `src/host/arch.rs`. gfx1030 (the first entry) is the
/// only target whose blob compilation is mandatory; the rest are best-effort.
const GFX_TARGETS: &[&str] = &[
    "gfx1030", "gfx1100", "gfx1200", "gfx90a", "gfx940", "gfx942",
];

fn main() {
    let rocm_path = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".to_string());
    let hipcc = format!("{rocm_path}/bin/hipcc");

    // --- Static library: host-runtime FFI + device kernels (gfx1030) --------
    //
    // The host-runtime wrappers (`host_runtime.hip`) and the BCJR / Gray-QAM
    // kernels are compiled into one static lib linked into the Rust crate.
    // These are unconditional (the crate is excluded from the default
    // workspace on non-ROCm hosts). The `hip` feature adds the permanent
    // kernels on top.
    let mut build = cc::Build::new();
    build
        .compiler(&hipcc)
        .file("hip/host_runtime.hip")
        .file("hip/bcjr_kernel.hip")
        .file("hip/gray_qam_demapper.hip")
        .file("hip/chacha20_awgn.hip");

    let hip_feature = env::var("CARGO_FEATURE_HIP").is_ok();
    if hip_feature {
        build
            .file("hip/permanent/permanent_bipedal3.hip")
            .file("hip/permanent/permanent_bipedal5.hip")
            .file("hip/permanent/permanent_bipedal7.hip");
    }

    build
        .flag("--offload-arch=gfx1030")
        .flag("-fPIC")
        .flag("-O3")
        .cpp(true)
        .compile("gf2_kernels_hip");

    // --- Per-arch kernel blobs (design doc §6) ------------------------------
    //
    // For each gfx target, compile every `kernels/<target>/*.cpp` source into a
    // `kernels/<target>/<name>.co` blob via `hipcc --offload-arch=<target>`.
    // The kernel sources are owned by the next wave (f6004add / a930be7f /
    // d3f1616a); until they land, each `kernels/<target>/` directory holds only
    // a `probe.cpp` no-op so the multi-arch compile path is exercised and a
    // gfx1030 blob exists for `GfxTarget::load_blob` to find.
    //
    // gfx1030 MUST compile (it is the CI target). Other archs are best-effort:
    // a hipcc failure for them (e.g. missing device libs) is logged via
    // `cargo:warning` and skipped — it does NOT fail the build.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let kernels_root = manifest_dir.join("kernels");
    let compiled = compile_arch_blobs(&hipcc, &kernels_root);

    // Record which arches THIS build actually produced a usable blob for, so
    // `GfxTarget::has_compiled_blob` consults a build-accurate manifest rather
    // than scanning the gitignored `kernels/` output dir (where STALE residue
    // from a prior build could wrongly report support). Comma-separated
    // `as_str()` names; an empty string when none compiled.
    println!(
        "cargo:rustc-env=GF2_HIP_COMPILED_ARCHS={}",
        compiled.join(",")
    );

    // --- Link + rerun triggers ---------------------------------------------
    let lib_path = format!("{rocm_path}/lib");
    println!("cargo:rustc-link-search=native={lib_path}");
    println!("cargo:rustc-link-lib=dylib=amdhip64");

    println!("cargo:rerun-if-changed=hip/host_runtime.hip");
    println!("cargo:rerun-if-changed=hip/bcjr_kernel.hip");
    println!("cargo:rerun-if-changed=hip/gray_qam_demapper.hip");
    println!("cargo:rerun-if-changed=hip/chacha20_awgn.hip");
    if hip_feature {
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal3.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal5.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal7.hip");
    }
    // NOTE: do NOT `rerun-if-changed=kernels`. `compile_arch_blobs` WRITES the
    // generated `<name>.co` blobs (and any best-effort probe) into
    // `kernels/<target>/`, so watching that directory makes this script
    // self-invalidating — every build mutates a watched path and forces the
    // next build to recompile the arch blobs. Real kernel sources live under
    // `hip/` (watched above); Phase B kernel owners adding `.cpp` sources under
    // `kernels/<target>/` must emit `rerun-if-changed` for those specific
    // SOURCE files only, never the output directory.
    println!("cargo:rerun-if-env-changed=ROCM_PATH");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-env=GF2_ROCM_LIB_PATH={lib_path}");
    let _ = out_dir; // suppress unused warning
}

/// Compiles the per-arch kernel blobs. gfx1030 is mandatory; others best-effort.
///
/// Returns the list of target names (`as_str()` form) for which at least one
/// `.co` blob compiled successfully THIS build. gfx1030 is always present
/// (mandatory; a failure panics); best-effort arches appear only when hipcc
/// succeeded for every one of their sources. The caller emits this set as the
/// `GF2_HIP_COMPILED_ARCHS` env manifest consulted by `has_compiled_blob`.
fn compile_arch_blobs(hipcc: &str, kernels_root: &Path) -> Vec<String> {
    let mut compiled: Vec<String> = Vec::new();
    for (idx, target) in GFX_TARGETS.iter().enumerate() {
        let mandatory = idx == 0; // gfx1030 is the first entry
        let target_dir = kernels_root.join(target);

        if let Err(e) = fs::create_dir_all(&target_dir) {
            if mandatory {
                panic!("failed to create kernels dir {target_dir:?}: {e}");
            }
            println!("cargo:warning=skip {target}: cannot create {target_dir:?}: {e}");
            continue;
        }

        // Ensure at least the probe source exists so the path is exercised even
        // before real kernel sources land next wave.
        ensure_probe_source(&target_dir, mandatory);

        // Gather every `*.cpp` source in this arch's directory.
        let sources = collect_cpp_sources(&target_dir);
        if sources.is_empty() {
            // Nothing to compile (probe write failed on a best-effort arch).
            continue;
        }

        let mut all_ok = true;
        for src in &sources {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("kernel");
            let out = target_dir.join(format!("{stem}.co"));
            println!("cargo:rerun-if-changed={}", src.display());

            let status = Command::new(hipcc)
                .arg(format!("--offload-arch={target}"))
                .arg("--genco") // emit a code-object (.co) blob
                .arg("-O3")
                .arg("-fPIC")
                .arg(src)
                .arg("-o")
                .arg(&out)
                .status();

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    let msg = format!(
                        "hipcc --offload-arch={target} on {} exited with {s}",
                        src.display()
                    );
                    if mandatory {
                        panic!("{msg}");
                    }
                    println!("cargo:warning=skip {target}: {msg}");
                    all_ok = false;
                    break; // skip remaining sources for this best-effort arch
                }
                Err(e) => {
                    let msg = format!("failed to invoke {hipcc} for {target}: {e}");
                    if mandatory {
                        panic!("{msg}");
                    }
                    println!("cargo:warning=skip {target}: {msg}");
                    all_ok = false;
                    break;
                }
            }
        }

        // Record this target as supported only when EVERY source compiled.
        // gfx1030 always reaches here (any failure above panicked).
        if all_ok {
            compiled.push((*target).to_string());
        }
    }
    compiled
}

/// Writes a minimal no-op probe source if the arch directory has no `*.cpp`
/// sources yet. Real kernels (next wave) drop their `.cpp` here and the probe
/// can be removed at that point.
fn ensure_probe_source(target_dir: &Path, mandatory: bool) {
    let has_cpp = collect_cpp_sources(target_dir).iter().any(|_| true);
    if has_cpp {
        return;
    }
    let probe = target_dir.join("probe.cpp");
    if probe.exists() {
        return;
    }
    // A trivial empty HIP device kernel — compiles to a valid `.co` on every
    // gfx target without pulling in any host-side dependencies. Documents the
    // expected source location for the next wave.
    let body = "// Auto-generated build probe for gf2-kernels-hip multi-arch dispatch.\n\
                // Real kernels (f6004add / a930be7f / d3f1616a) land their *.cpp here\n\
                // next wave; this no-op keeps the per-arch .co compile path green.\n\
                #include <hip/hip_runtime.h>\n\
                __global__ void gf2_hip_probe(int* out) { if (out) *out = 0; }\n";
    if let Err(e) = fs::write(&probe, body) {
        if mandatory {
            panic!("failed to write probe source {probe:?}: {e}");
        }
        println!("cargo:warning=could not write probe {probe:?}: {e}");
    }
}

/// Collects all `*.cpp` source paths directly under `dir` (non-recursive).
fn collect_cpp_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("cpp") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}
