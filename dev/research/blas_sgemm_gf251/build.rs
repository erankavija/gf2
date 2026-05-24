//! Build script: link the system OpenBLAS via the dynamic loader.
//!
//! OpenBLAS 0.3.x is BSD-3 licensed and the canonical
//! permissively-licensed single-threaded sgemm provider on
//! Arch / Debian / Ubuntu hosts. We do NOT vendor or build OpenBLAS
//! from source — that would be the `openblas-src` crate's job — we
//! just request that rustc link `-lopenblas`.
//!
//! If the host has no OpenBLAS install at `/usr/lib`, this build
//! script still emits the link directive and the linker reports a
//! clear "cannot find -lopenblas" error. The harness is not part of
//! the default `cargo build --workspace --all-features` (it is
//! excluded by the `[workspace]` table in `Cargo.toml` of this
//! prototype), so a missing OpenBLAS does not break the main build.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=dylib=openblas");
}
