use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = std::env::var_os("RUSTC").expect("Cargo sets RUSTC for build scripts");
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("the configured Rust compiler is executable");
    assert!(output.status.success(), "`rustc --version` succeeds");
    let version = String::from_utf8(output.stdout).expect("rustc version is UTF-8");
    println!("cargo:rustc-env=GF2_BUILD_RUSTC_VERSION={}", version.trim());
}
