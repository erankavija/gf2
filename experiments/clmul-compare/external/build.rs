//! Links gf2x from the install prefix `GF2_GF2X_PREFIX` names, with that
//! prefix's library directory on the run path.

fn main() {
    println!("cargo:rerun-if-env-changed=GF2_GF2X_PREFIX");
    let prefix = std::env::var("GF2_GF2X_PREFIX")
        .expect("GF2_GF2X_PREFIX must name a gf2x install prefix");
    println!("cargo:rustc-link-search=native={prefix}/lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{prefix}/lib");
    println!("cargo:rustc-link-lib=gf2x");
}
