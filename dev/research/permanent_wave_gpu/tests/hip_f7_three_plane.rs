#![cfg(feature = "hip")]

use std::process::Command;

/// The prebuilt HIP executable exhaustively checks device add/sub pairs and
/// three-lane zero-mask/C6 products. Host corpus tests cover permanents.
#[test]
fn device_f7_three_plane_arithmetic_is_exact() {
    let status = Command::new(env!("PERMANENT_WAVE_GPU_F7_EQUIVALENCE_BIN"))
        .status()
        .expect("the HIP build must provide the F_7 equivalence executable");
    assert!(
        status.success(),
        "device arithmetic equivalence failed: {status}"
    );
}
