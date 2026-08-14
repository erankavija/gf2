#![cfg(feature = "hip")]

use std::process::Command;

use permanent_wave_gpu::MeasurementPath;

/// The prebuilt executable owns device selection and checks both F_5 paths.
/// Release evidence invokes it directly; the normal fast tier never opts into
/// this device-gated test.
#[test]
#[ignore = "device: requires a ROCm gfx1030 device"]
fn device_f5_wave_paths_match_independent_structural_references() {
    MeasurementPath::F5ByteControl
        .device_batch_kernel()
        .expect("the existing byte-control registry entry must reach the landed candidate");
    MeasurementPath::F5ThreePlane
        .device_batch_kernel()
        .expect("the existing three-plane registry entry must reach the landed candidate");

    let status = Command::new(env!("PERMANENT_WAVE_GPU_F5_EQUIVALENCE_BIN"))
        .status()
        .expect("the HIP build must provide the F_5 wave equivalence executable");
    assert!(
        status.success(),
        "F_5 wave device equivalence failed: {status}"
    );
}
