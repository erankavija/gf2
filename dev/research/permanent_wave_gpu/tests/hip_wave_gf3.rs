#![cfg(feature = "hip")]

use std::process::Command;

use permanent_wave_gpu::MeasurementPath;

/// The prebuilt executable owns the actual device checks, so normal test runs
/// do not select a ROCm device merely by compiling the optional HIP feature.
/// The equivalent release evidence invokes this executable directly rather
/// than opting the fast test tier into ignored tests.
#[test]
#[ignore = "device: requires a ROCm gfx1030 device"]
fn device_wave_gf3_matches_independent_structural_references() {
    MeasurementPath::WaveGf3
        .dispatch()
        .expect("the existing registry entry must reach the landed F_3 candidate");

    let status = Command::new(env!("PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN"))
        .status()
        .expect("the HIP build must provide the F_3 wave equivalence executable");
    assert!(
        status.success(),
        "F_3 wave device equivalence failed: {status}"
    );
}
