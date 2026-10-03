use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Kept in sync with `GfxTarget::ALL` in `src/host/arch.rs`. gfx1030 (the first
/// entry) is the only target whose blob compilation is mandatory; the rest are
/// best-effort.
const GFX_TARGETS: &[&str] = &[
    "gfx1030", "gfx1100", "gfx1200", "gfx90a", "gfx940", "gfx942",
];

fn main() {
    let rocm_path = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".to_string());
    let hipcc = format!("{rocm_path}/bin/hipcc");

    // The static library is compiled for gfx1030 only; the `hip` feature adds
    // the BCH syndrome and permanent kernels.
    let mut build = cc::Build::new();
    build
        .compiler(&hipcc)
        .file("hip/host_runtime.hip")
        .file("hip/bcjr_kernel.hip")
        .file("hip/gray_qam_demapper.hip")
        .file("hip/chacha20_awgn.hip")
        .file("hip/ldpc_bp.hip");

    let hip_feature = env::var("CARGO_FEATURE_HIP").is_ok();
    if hip_feature {
        build
            .file("hip/bch_syndrome.hip")
            .file("hip/permanent/permanent_bipedal3.hip")
            .file("hip/permanent/permanent_bipedal5.hip")
            .file("hip/permanent/permanent_bipedal7.hip")
            .file("hip/permanent/gray_update_micro.hip");
    }

    build
        .flag("--offload-arch=gfx1030")
        .flag("-fPIC")
        .flag("-O3")
        .cpp(true)
        .compile("gf2_kernels_hip");

    // gfx1030 blobs must compile. A hipcc failure for any other arch is
    // reported through `cargo:warning` and that arch is skipped.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let kernels_root = manifest_dir.join("kernels");
    let compiled = compile_arch_blobs(&hipcc, &kernels_root);

    // `GfxTarget::has_compiled_blob` reads this manifest instead of scanning
    // the gitignored `kernels/` output directory, where residue of another
    // build could report support this build lacks. Comma-separated `as_str()`
    // names; empty when none compiled.
    println!(
        "cargo:rustc-env=GF2_HIP_COMPILED_ARCHS={}",
        compiled.join(",")
    );

    let lib_path = format!("{rocm_path}/lib");
    println!("cargo:rustc-link-search=native={lib_path}");
    println!("cargo:rustc-link-lib=dylib=amdhip64");

    println!("cargo:rerun-if-changed=hip/host_runtime.hip");
    println!("cargo:rerun-if-changed=hip/bcjr_kernel.hip");
    println!("cargo:rerun-if-changed=hip/gray_qam_demapper.hip");
    println!("cargo:rerun-if-changed=hip/chacha20_awgn.hip");
    println!("cargo:rerun-if-changed=hip/ldpc_bp.hip");
    if hip_feature {
        println!("cargo:rerun-if-changed=hip/bch_syndrome.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal3.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal5.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal7.hip");
        println!("cargo:rerun-if-changed=hip/permanent/gray_update_micro.hip");
        println!("cargo:rerun-if-changed=hip/permanent/horizontal_product_micro.hip");
    }
    // `kernels/` is not watched: `compile_arch_blobs` writes its `.co` output
    // there, so watching the directory would invalidate every build. Each
    // `.cpp` source in it is watched individually by `compile_arch_blobs`.
    println!("cargo:rerun-if-env-changed=ROCM_PATH");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-env=GF2_ROCM_LIB_PATH={lib_path}");
    let _ = out_dir; // suppress unused warning
}

/// Returns the targets for which every source compiled in this build, in
/// `as_str()` form. A gfx1030 failure panics.
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

        ensure_probe_source(&target_dir, mandatory);

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

        if all_ok {
            compiled.push((*target).to_string());
        }
    }
    compiled
}

/// Writes a no-op probe source when the arch directory holds no `*.cpp`
/// source, so each target compiles at least one blob.
fn ensure_probe_source(target_dir: &Path, mandatory: bool) {
    let has_cpp = collect_cpp_sources(target_dir).iter().any(|_| true);
    if has_cpp {
        return;
    }
    let probe = target_dir.join("probe.cpp");
    if probe.exists() {
        return;
    }
    // An empty HIP device kernel compiles to a valid `.co` on every gfx target
    // without host-side dependencies.
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
