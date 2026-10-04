//! Runs the DVB-T2 BICM pipeline CPU-only (`with_gpu(false)`) and on the hybrid
//! CPU+GPU path (`with_gpu(true)`) at one waterfall SNR point and asserts that
//! `fer` / `frames` / `errors` are byte-identical; `mean_iters` is logged only.
//! Exits 0 with a notice when no usable GPU is present.
//!
//! Run with: `cargo run -p gf2-sim --example gpu_hybrid --features hip --release`

#[cfg(not(feature = "hip"))]
fn main() {
    // Lets the file type-check without `hip`; `required-features` keeps the
    // example from being built without it.
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

    // Waterfall point for NMS(0.75) max-log at this seed: a mix of errored and
    // clean frames.
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

    assert!(
        cpu.errors > 0 && cpu.errors < cpu.frames,
        "VACUOUS sweep: {} errored of {} frames (need 0 < errors < frames)",
        cpu.errors,
        cpu.frames,
    );

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

    println!(
        "\nmean_iters (LOGGED, NOT asserted — §11 exclusion): CPU {:.6}, CPU+GPU {:.6}, diff {:+.6}",
        cpu.mean_iters,
        hybrid.mean_iters,
        hybrid.mean_iters - cpu.mean_iters,
    );
    println!("byte-identity: PASS (fer/frames/errors identical CPU vs CPU+GPU)");
}
