//! GPU hybrid: the same DVB-T2 chain on CPU+GPU vs CPU-only, byte-identical.
//!
//! Builds the production DVB-T2 BICM pipeline twice — once CPU-only
//! (`with_gpu(false)`) and once on the hybrid CPU+GPU path
//! (`with_gpu(true)`) — runs the same waterfall SNR point on each, and asserts
//! the design-doc §11 **CPU-vs-GPU** contract: the three columns
//! `fer` / `frames` / `errors` are byte-identical across the two paths.
//! `mean_iters` is EXCLUDED from that contract (RDNA2 hardware transcendentals
//! differ from CPU polynomial reductions by 1–3 ULPs, which can shift the BP
//! convergence iteration by ±1 near the threshold); it is logged, never
//! asserted. `ber` is excluded entirely (non-associative f32 reduction).
//!
//! The GPU arm runs the demap and LDPC BP on the device (max-log demap — the
//! GPU kernel is max-log only — and normalized-min-sum BP); BCH outer decode
//! stays on the CPU on both arms.
//!
//! This example is compiled only under `--features hip` (see the `[[example]]`
//! entry in `Cargo.toml`). It **skips gracefully** when no usable GPU is
//! present (`gf2_kernels_hip::host::device_mem_info().is_err()`), printing a
//! skip notice and exiting 0 — the same guard the GPU test suites use.
//!
//! Runtime: a few seconds on a gfx1030 (200 frames at the waterfall point).
//!
//! Run with: `cargo run -p gf2-sim --example gpu_hybrid --features hip --release`

#[cfg(not(feature = "hip"))]
fn main() {
    // The Cargo `required-features = ["hip"]` gate means this body is never
    // compiled into a runnable example without `hip`; this stub only exists so
    // the file type-checks on a default build that happens to reference it.
    eprintln!("gpu_hybrid requires --features hip");
}

#[cfg(feature = "hip")]
fn main() {
    use std::num::NonZeroUsize;

    use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    use gf2_coding::modem::DemapMethod;
    use gf2_coding::CodeRate;

    use gf2_kernels_hip::host::device_mem_info;
    use gf2_sim::executor::SnrPointResult;
    use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    use gf2_sim::Pipeline;

    // Waterfall point for NMS(0.75) max-log at this seed (matches the
    // tests/gpu_byte_identity.rs r1/2 16-QAM calibration): non-vacuous, a mix
    // of errored and clean frames.
    const SEED: u64 = 0x14F5_9C2D_0012_0010;
    const ES_N0_DB: f32 = 6.4;
    const FRAMES: u64 = 200;

    if device_mem_info().is_err() {
        eprintln!("skipping gpu_hybrid: no usable GPU (device_mem_info failed)");
        return;
    }

    let run_arm = |gpu: bool| -> SnrPointResult {
        let mut pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(
                DecoderAlgorithm::NormalizedMinSum(0.75),
                true,
            ))
            .demap(DemapMethod::MaxLog)
            .channel(Channel::awgn(ES_N0_DB))
            .seed(SEED)
            .parallelism(NonZeroUsize::new(4).expect("4 is non-zero"))
            .with_gpu(gpu)
            .build()
            .expect("r1/2 16-QAM Normal is an in-scope MODCOD");
        pipeline.config_mut().esn0_db_points = vec![f64::from(ES_N0_DB)];
        pipeline.config_mut().max_frames = FRAMES;
        pipeline
            .run()
            .expect("the DVB-T2 sweep runs end-to-end")
            .per_point[0]
    };

    let cpu = run_arm(false);
    let hybrid = run_arm(true);

    println!("DVB-T2 r1/2 16-QAM Normal @ {ES_N0_DB} dB, seed {SEED:#018x}, {FRAMES} frames");
    println!("path        frames  errors  FER          mean_iters");
    for (label, p) in [("CPU", &cpu), ("CPU+GPU", &hybrid)] {
        println!(
            "{label:<11} {:<7} {:<7} {:<12.9} {:<10.6}",
            p.frames, p.errors, p.fer, p.mean_iters
        );
    }

    // Non-vacuity: the verdict boundary §11 is about must be exercised.
    assert!(
        cpu.errors > 0 && cpu.errors < cpu.frames,
        "VACUOUS sweep: {} errored of {} frames (need 0 < errors < frames)",
        cpu.errors,
        cpu.frames,
    );

    // The §11 CPU-vs-GPU three-column contract.
    assert_eq!(cpu.frames, hybrid.frames, "frames byte-identical");
    assert_eq!(
        cpu.errors, hybrid.errors,
        "errors (frame errors) byte-identical"
    );
    assert_eq!(
        cpu.fer.to_bits(),
        hybrid.fer.to_bits(),
        "fer byte-identical"
    );

    // mean_iters is LOGGED, never asserted (§11 CPU-vs-GPU exclusion).
    println!(
        "\nmean_iters (LOGGED, NOT asserted — §11 exclusion): CPU {:.6}, CPU+GPU {:.6}, diff {:+.6}",
        cpu.mean_iters,
        hybrid.mean_iters,
        hybrid.mean_iters - cpu.mean_iters,
    );
    println!("byte-identity: PASS (fer/frames/errors identical CPU vs CPU+GPU)");
}
