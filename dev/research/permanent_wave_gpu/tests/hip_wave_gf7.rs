#![cfg(feature = "hip")]

use std::process::Command;

use permanent_wave_gpu::MeasurementPath;

/// Full F_7 equality at orders 16, 20, and 24 belongs to the direct evidence
/// driver. This ignored smoke route must never make ordinary HIP-feature test
/// compilation select a device.
#[test]
#[ignore = "device: requires a ROCm gfx1030 device"]
fn device_f7_wave_paths_match_their_structural_references() {
    for path in [
        MeasurementPath::F7LookupTableControl,
        MeasurementPath::F7ThreePlanePermanent,
    ] {
        path.device_batch_kernel()
            .expect("each registered F_7 permanent candidate must own a batch kernel");
        let status = Command::new(env!("PERMANENT_WAVE_GPU_WAVE_GF7_EQUIVALENCE_BIN"))
            .args(["--path", path.name()])
            .status()
            .expect("the HIP build must provide the F_7 wave equivalence executable");
        assert!(
            status.success(),
            "{} device structural equivalence failed: {status}",
            path.name()
        );
    }
}
