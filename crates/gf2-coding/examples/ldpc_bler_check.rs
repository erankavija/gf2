//! Quick BLER check at 3 dB for (256,121) — paper says ~0.001 for BP
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};

fn main() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    println!("Code: n={}, k={}", rm_code.n(), rm_code.k());

    // NMS decoder (alpha=0.75)
    let mut decoder_nms = Nr5gRateMatchedDecoder::new(rm_code.clone());
    // BP decoder (alpha=1.0)
    let mut decoder_bp = Nr5gRateMatchedDecoder::with_scale(rm_code.clone(), 1.0);

    let channel = BpskAwgnChannel;
    let config = SimulationConfig {
        eb_n0_range_db: vec![3.0],
        min_errors: 50,
        max_frames: 50_000,
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };

    let nms = SimulationRunner::run_coded_iterative(&rm_code, &mut decoder_nms, &channel, &config);
    println!(
        "NMS @3dB: BLER={:.4e}, BER={:.4e}, frames={}, errors={}",
        nms.points[0].bler,
        nms.points[0].ber,
        nms.points[0].num_frames,
        nms.points[0].num_frame_errors
    );

    let bp = SimulationRunner::run_coded_iterative(&rm_code, &mut decoder_bp, &channel, &config);
    println!(
        "BP  @3dB: BLER={:.4e}, BER={:.4e}, frames={}, errors={}",
        bp.points[0].bler, bp.points[0].ber, bp.points[0].num_frames, bp.points[0].num_frame_errors
    );

    println!("\nPaper reference: BLER_BP ≈ 0.001, BLER_NMS ≈ 0.005");
}
