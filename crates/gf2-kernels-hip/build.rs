use std::env;
use std::path::PathBuf;

fn main() {
    let rocm_path = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".to_string());
    let hipcc = format!("{rocm_path}/bin/hipcc");

    // Compile HIP kernels to a single static library. Both kernels share
    // the same hipcc flags; adding more kernels is a one-liner here.
    cc::Build::new()
        .compiler(&hipcc)
        .file("hip/bcjr_kernel.hip")
        .file("hip/gray_qam_demapper.hip")
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
    println!("cargo:rerun-if-env-changed=ROCM_PATH");

    // Export the ROCm path for runtime library resolution
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-env=GF2_ROCM_LIB_PATH={lib_path}");
    let _ = out_dir; // suppress unused warning
}
