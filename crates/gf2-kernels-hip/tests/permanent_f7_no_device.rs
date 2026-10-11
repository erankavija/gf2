//! F_7 permanent entry points in a process with no visible HIP device: the
//! init entry point needs no caller-established runtime state and reports the
//! missing device as a HIP status code, and the LUT checksum launch reports it
//! the same way.

#![cfg(feature = "hip")]

use gf2_algebra::packed::packed7::{ADD_LUT, MUL_LUT, SUB_LUT};
use gf2_kernels_hip::permanent::{compute_lut_checksum_gpu, init_permanent_gf7_from_slices};
use std::process::Command;

/// Marks the re-executed child, which runs with every HIP device hidden.
const CHILD_ROLE: &str = "GF2_F7_NO_DEVICE_CHILD";

/// Runs `test_name` again in a child process with every HIP device hidden and
/// returns whether the body ran (`true`, in the child) or the child exited
/// normally (`false`, in the parent, after asserting success). The device mask
/// is read once at runtime initialisation, so each no-device case needs its own
/// process.
fn in_no_device_child(test_name: &str) -> bool {
    if std::env::var_os(CHILD_ROLE).is_some() {
        return true;
    }
    let status = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", test_name])
        .env(CHILD_ROLE, "1")
        .env("HIP_VISIBLE_DEVICES", "-1")
        .status()
        .expect("spawn the no-device child");
    assert!(
        status.success(),
        "the no-device child must exit normally with a non-zero HIP status, got {status}"
    );
    false
}

#[test]
fn test_init_permanent_gf7_from_slices_without_device_returns_status() {
    if in_no_device_child("test_init_permanent_gf7_from_slices_without_device_returns_status") {
        let rc = init_permanent_gf7_from_slices(&ADD_LUT, &SUB_LUT, &MUL_LUT);
        assert_ne!(rc, 0, "an upload with no visible device must fail");
    }
}

#[test]
fn test_compute_lut_checksum_gpu_without_device_returns_status() {
    if in_no_device_child("test_compute_lut_checksum_gpu_without_device_returns_status") {
        // SAFETY: with every device hidden the launch fails before the kernel
        // runs, and only the kernel dereferences the output pointer, so null
        // satisfies the contract's no-visible-device case.
        let rc = unsafe { compute_lut_checksum_gpu(std::ptr::null_mut()) };
        assert_ne!(rc, 0, "a launch with no visible device must fail");
    }
}
