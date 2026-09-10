use std::env;
use std::path::PathBuf;

fn main() {
    let include_dir = env::var_os("GF2_XDSOPL_INCLUDE").unwrap_or_else(|| {
        panic!("GF2_XDSOPL_INCLUDE is unset; point it at the fetched xdsopl-ldpc checkout containing interleaver.hh")
    });
    let include_dir = PathBuf::from(include_dir);
    let header = include_dir.join("interleaver.hh");
    assert!(
        header.is_file(),
        "GF2_XDSOPL_INCLUDE does not contain interleaver.hh: {}",
        include_dir.display()
    );

    // The external arm's C++ architecture level is declared by the caller so
    // one build flavour compiles the whole arm at one level: `native` for the
    // tuned flavour, `x86-64` for the conservative-portable control.
    let cxx_arch = env::var("GF2_XDSOPL_CXX_ARCH").unwrap_or_else(|_| "native".to_owned());

    println!("cargo:rerun-if-env-changed=GF2_XDSOPL_INCLUDE");
    println!("cargo:rerun-if-env-changed=GF2_XDSOPL_CXX_ARCH");
    println!("cargo:rerun-if-changed=../xdsopl-shim/xdsopl_shim.cpp");
    println!("cargo:rerun-if-changed={}", header.display());

    cc::Build::new()
        .cpp(true)
        .file("../xdsopl-shim/xdsopl_shim.cpp")
        .include(&include_dir)
        .flag("-O3")
        .flag(format!("-march={cxx_arch}"))
        .flag("-std=c++17")
        .compile("xdsopl_shim");
}
