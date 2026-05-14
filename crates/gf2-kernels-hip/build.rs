use std::env;
use std::path::PathBuf;

fn main() {
    let rocm_path = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".to_string());
    let hipcc = format!("{rocm_path}/bin/hipcc");

    // Compile HIP kernels to a single static library. Both kernels share
    // the same hipcc flags; adding more kernels is a one-liner here.
    let mut build = cc::Build::new();
    build
        .compiler(&hipcc)
        .file("hip/bcjr_kernel.hip")
        .file("hip/gray_qam_demapper.hip");

    // Compile per-prime permanent kernels when the `hip` feature is enabled.
    // Non-`hip` builds (i.e., the crate built without `--features hip`) skip
    // these files entirely so the build remains host-agnostic for the
    // baseline kernel set.
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

    // Link against HIP runtime
    let lib_path = format!("{rocm_path}/lib");
    println!("cargo:rustc-link-search=native={lib_path}");
    println!("cargo:rustc-link-lib=dylib=amdhip64");

    // Rerun if kernel source changes
    println!("cargo:rerun-if-changed=hip/bcjr_kernel.hip");
    println!("cargo:rerun-if-changed=hip/gray_qam_demapper.hip");
    if hip_feature {
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal3.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal5.hip");
        println!("cargo:rerun-if-changed=hip/permanent/permanent_bipedal7.hip");
    }
    println!("cargo:rerun-if-env-changed=ROCM_PATH");

    // Export the ROCm path for runtime library resolution
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-env=GF2_ROCM_LIB_PATH={lib_path}");
    let _ = out_dir; // suppress unused warning
}
