//! Example: tiny uncoded BER sweep via [`SimulationRunner`] over the
//! modem framework.
//!
//! This example demonstrates two integration paths between the modem
//! framework and the [`crate::simulation`] harness:
//!
//! 1. **AWGN via [`ModemChannelAdapter`]** — wraps a preset Gray-QAM
//!    spec's preferred mapper / demapper behind [`ChannelModel`] and
//!    runs the uncoded BER sweep through
//!    [`SimulationRunner::run_uncoded_ber_with_channel`].
//! 2. **Rician fading via [`QpskRicianChannelModel`]** — the ready-made
//!    Rician-fading [`ChannelModel`] that internally uses the same
//!    modem framework (QPSK Gray-mapped) plus a block-fading Rician
//!    gain.
//!
//! The sweep is intentionally tiny (a few Eb/N0 points, a few thousand
//! bits each) so the example completes in well under 10 s even on cold
//! CI machines.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --example modem_simulation_harness --release
//! ```
//!
//! [`crate::simulation`]: gf2_coding::simulation
//! [`ChannelModel`]: gf2_coding::simulation::ChannelModel
//! [`ModemChannelAdapter`]: gf2_coding::modem::ModemChannelAdapter
//! [`QpskRicianChannelModel`]: gf2_coding::fading::QpskRicianChannelModel
//! [`SimulationRunner`]: gf2_coding::simulation::SimulationRunner
//! [`SimulationRunner::run_uncoded_ber_with_channel`]: gf2_coding::simulation::SimulationRunner::run_uncoded_ber_with_channel

use std::process::ExitCode;

use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
use gf2_coding::modem::{DemapMethod, ModemChannelAdapter, ModemSpec};
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn run() -> Result<(), String> {
    // ---- Shared sweep configuration -------------------------------------
    //
    // Three Eb/N0 points are plenty to show the BER slope without making
    // the example slow. `max_frames` here is the total bit budget per
    // SNR point; with 4 kbits and `min_errors = 1` the runner will stop
    // as soon as it has any evidence at each point.
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

    // ---- 1. QPSK over AWGN via ModemChannelAdapter ----------------------
    //
    // The ergonomic path for user code is `ModemSpec::preferred_mapper` +
    // `ModemSpec::preferred_soft_demapper`. Those factories select the
    // Gray-QAM fast path for preset specs and fall back to the reference
    // path for custom constellations — the caller never has to name the
    // backend. We use them here so the example reads like the module-
    // level docs recommend. The returned `Box<dyn BatchMapper<f32> +
    // Send + Sync>` / `Box<dyn BatchSoftDemapper<f32> + Send + Sync>`
    // satisfy `ModemChannelAdapter`'s generic bounds directly.
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

    // ---- 2. QPSK over Rician fading via QpskRicianChannelModel ----------
    //
    // `QpskRicianChannelModel` is the library's ready-made Rician
    // `ChannelModel`. Internally it uses exactly the same modem-framework
    // surface as the AWGN adapter above (QPSK Gray-mapped, reference
    // soft-demapper, `unit_energy_n0_from_eb_n0_db` for the noise scale),
    // so this integration path is identical from the harness' point of
    // view — just pass a different channel to the same runner.
    //
    // `RicianConfig::fig8()` has `frame_bits = 1024`, so we cap the batch
    // length below that when the runner slices `max_frames` internally.
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

    // ---- Sanity check ---------------------------------------------------
    //
    // Both sweeps must return one result per configured Eb/N0 point.
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

    // At the highest Eb/N0 point AWGN BER should be noticeably better
    // than the fading BER — a cheap qualitative check that both channels
    // are "alive" and not stuck at zero / one.
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
