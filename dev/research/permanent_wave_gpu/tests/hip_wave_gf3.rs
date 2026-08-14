#![cfg(feature = "hip")]

use std::process::Command;

use permanent_wave_gpu::MeasurementPath;

/// The prebuilt executable owns the actual device checks, so normal test runs
/// do not select a ROCm device merely by compiling the optional HIP feature.
/// The equivalent release evidence invokes this executable directly for both
/// registered F_3 fold choices rather than opting the fast test tier into
/// ignored tests.
#[test]
#[ignore = "device: requires a ROCm gfx1030 device"]
fn device_f3_folds_match_independent_structural_references() {
    for path in [MeasurementPath::WaveGf3, MeasurementPath::FoldGf3] {
        path.device_batch_kernel()
            .expect("the existing registry entry must reach the landed F_3 candidate");

        let status = Command::new(env!("PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN"))
            .args(["--fold", path.name()])
            .status()
            .expect("the HIP build must provide the F_3 wave equivalence executable");
        assert!(
            status.success(),
            "{} device equivalence failed: {status}",
            path.name()
        );
    }
}
