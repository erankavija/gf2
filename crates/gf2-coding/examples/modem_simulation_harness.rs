//! Uncoded BER sweeps through `SimulationRunner`: QPSK over AWGN via `ModemChannelAdapter` and
//! QPSK over Rician block fading via `QpskRicianChannelModel`.

use std::process::ExitCode;

use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
use gf2_coding::modem::{DemapMethod, ModemChannelAdapter, ModemSpec};
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn run() -> Result<(), String> {
    let config = SimulationConfig {
        eb_n0_range_db: vec![2.0, 6.0, 10.0],
        min_errors: 1,
        max_frames: 4_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0xA5A5_A5A5),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };

    let qpsk_spec = ModemSpec::<f32>::gray_square_qam(4);
    let qpsk_mapper = qpsk_spec.clone().preferred_mapper();
    let qpsk_demap = qpsk_spec.preferred_soft_demapper();
    let awgn_channel = ModemChannelAdapter::new(qpsk_mapper, qpsk_demap, DemapMethod::MaxLog);

    let mut rng = StdRng::seed_from_u64(0xDEAD_BEEF);
    let awgn_results =
        SimulationRunner::run_uncoded_ber_with_channel(&awgn_channel, &config, &mut rng);

    println!("=== QPSK over AWGN via ModemChannelAdapter ===");
    println!("Eb/N0 (dB)     bits     errors      BER");
    for r in &awgn_results {
        println!(
            "  {:>5.1}     {:>7}    {:>5}    {:.3e}",
            r.eb_n0_db, r.num_bits, r.num_bit_errors, r.ber,
        );
    }

    let rician_channel = QpskRicianChannelModel::new(RicianConfig::fig8());
    let mut rng = StdRng::seed_from_u64(0xFACE_CAFE);
    let rician_results =
        SimulationRunner::run_uncoded_ber_with_channel(&rician_channel, &config, &mut rng);

    println!();
    println!("=== QPSK over Rician fading (fig8: K=5, N_c=128, t=4) ===");
    println!("Eb/N0 (dB)     bits     errors      BER");
    for r in &rician_results {
        println!(
            "  {:>5.1}     {:>7}    {:>5}    {:.3e}",
            r.eb_n0_db, r.num_bits, r.num_bit_errors, r.ber,
        );
    }

    if awgn_results.len() != config.eb_n0_range_db.len() {
        return Err(format!(
            "AWGN sweep produced {} results, expected {}",
            awgn_results.len(),
            config.eb_n0_range_db.len(),
        ));
    }
    if rician_results.len() != config.eb_n0_range_db.len() {
        return Err(format!(
            "Rician sweep produced {} results, expected {}",
            rician_results.len(),
            config.eb_n0_range_db.len(),
        ));
    }

    let top_awgn = awgn_results.last().ok_or("missing AWGN top point")?;
    let top_rician = rician_results.last().ok_or("missing Rician top point")?;
    println!();
    println!(
        "At Eb/N0 = {:.1} dB: AWGN BER = {:.3e}, Rician BER = {:.3e}",
        top_awgn.eb_n0_db, top_awgn.ber, top_rician.ber,
    );

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("modem_simulation_harness failed: {msg}");
            ExitCode::FAILURE
        }
    }
}
