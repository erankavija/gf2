//! `init_permanent_gf7_from_slices` in a process with no visible HIP device:
//! the safe entry point needs no caller-established runtime state and reports
//! the missing device as a HIP status code.

#![cfg(feature = "hip")]

use gf2_algebra::packed::packed7::{ADD_LUT, MUL_LUT, SUB_LUT};
use gf2_kernels_hip::permanent::init_permanent_gf7_from_slices;
use std::process::Command;

/// Marks the re-executed child, which runs with every HIP device hidden.
const CHILD_ROLE: &str = "GF2_F7_INIT_NO_DEVICE_CHILD";

#[test]
fn test_init_permanent_gf7_from_slices_without_device_returns_status() {
    if std::env::var_os(CHILD_ROLE).is_some() {
        let rc = init_permanent_gf7_from_slices(&ADD_LUT, &SUB_LUT, &MUL_LUT);
        assert_ne!(rc, 0, "an upload with no visible device must fail");
        return;
    }

    // The device mask is read once at runtime initialisation, so the
    // no-device case needs its own process.
    let status = Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            "test_init_permanent_gf7_from_slices_without_device_returns_status",
        ])
        .env(CHILD_ROLE, "1")
        .env("HIP_VISIBLE_DEVICES", "-1")
        .status()
        .expect("spawn the no-device child");
    assert!(
        status.success(),
        "the no-device child must exit normally with a non-zero HIP status, got {status}"
    );
}
