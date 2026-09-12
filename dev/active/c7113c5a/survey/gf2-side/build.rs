//! Points the gf2x arm at the pinned build variant selected by `GF2X_PREFIX`.
//!
//! Only the search path and the rpath of the three binaries that bind gf2x are
//! emitted here; the `-lgf2x` request lives in their own `#[link]` attribute, so
//! the gf2 arm links nothing external and keeps an independent executable
//! identity. `--disable-new-dtags` makes the rpath take precedence over
//! `LD_LIBRARY_PATH`, and `assert_pinned_library` re-checks at run time that the
//! loaded object really is the pinned build rather than a system copy.

fn main() {
    println!("cargo:rerun-if-env-changed=GF2X_PREFIX");
    println!("cargo:rerun-if-env-changed=GF2X_CFLAGS");
    let prefix = std::env::var("GF2X_PREFIX")
        .expect("GF2X_PREFIX must name a prefix produced by survey/fetch-build.sh");
    println!("cargo:rustc-link-search=native={prefix}/lib");
    for binary in ["gf2x-poly-arm", "poly-validate", "dot-reduction-probe"] {
        println!("cargo:rustc-link-arg-bin={binary}=-Wl,-rpath,{prefix}/lib");
        println!("cargo:rustc-link-arg-bin={binary}=-Wl,--disable-new-dtags");
    }
    println!("cargo:rustc-env=GF2X_PREFIX_USED={prefix}");
    println!(
        "cargo:rustc-env=GF2X_CFLAGS_USED={}",
        std::env::var("GF2X_CFLAGS").unwrap_or_else(|_| "unset".to_owned())
    );
}
