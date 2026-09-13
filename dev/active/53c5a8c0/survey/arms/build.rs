//! Points the gf2x-linked binaries at the pinned build variant selected by
//! `GF2X_PREFIX` (jit:53c5a8c0).
//!
//! Only the search path and the rpath of the binaries that bind gf2x are
//! emitted here; the `-lgf2x` request lives in the gf2x backend's own `#[link]`
//! attribute, so the gf2-only arms link nothing external and keep independent
//! executable identities. `--disable-new-dtags` makes the rpath take precedence
//! over `LD_LIBRARY_PATH`, and the backend re-checks at run time that the
//! loaded object really is the pinned build rather than a system copy.

fn main() {
    println!("cargo:rerun-if-env-changed=GF2X_PREFIX");
    println!("cargo:rerun-if-env-changed=GF2X_CFLAGS");
    let prefix = std::env::var("GF2X_PREFIX")
        .expect("GF2X_PREFIX must name a prefix produced by survey/fetch-build.sh");
    println!("cargo:rustc-link-search=native={prefix}/lib");
    for binary in ["gf2x-poly-arm", "crossover-validate"] {
        println!("cargo:rustc-link-arg-bin={binary}=-Wl,-rpath,{prefix}/lib");
        println!("cargo:rustc-link-arg-bin={binary}=-Wl,--disable-new-dtags");
    }
    println!("cargo:rustc-env=GF2X_PREFIX_USED={prefix}");
    println!(
        "cargo:rustc-env=GF2X_CFLAGS_USED={}",
        std::env::var("GF2X_CFLAGS").unwrap_or_else(|_| "unset".to_owned())
    );
}
