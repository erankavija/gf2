//! Example: Gray-coded 16-QAM preset over AWGN via the modem framework.
//!
//! This example demonstrates the **preset workflow**:
//!
//! 1. Build a validated [`ModemSpec`] via [`ModemSpec::gray_square_qam`].
//! 2. Ask the spec for its preferred mapper and soft demapper via the
//!    shared-API factories [`ModemSpec::preferred_mapper`] and
//!    [`ModemSpec::preferred_soft_demapper`]. Both will dispatch to the
//!    optimized Gray-QAM fast-path backend for a preset of this shape.
//! 3. Run a small ad-hoc AWGN loop: bits -> symbols -> noisy symbols ->
//!    LLRs -> hard decisions -> BER.
//!
//! The harness is intentionally tiny so the example runs in well under
//! one second; the point is to show end-to-end ergonomics, not to produce
//! a publication-quality curve.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --example modem_gray_qam_preset --release
//! ```
//!
//! [`ModemSpec`]: gf2_coding::modem::ModemSpec
//! [`ModemSpec::gray_square_qam`]: gf2_coding::modem::ModemSpec::gray_square_qam
//! [`ModemSpec::preferred_mapper`]: gf2_coding::modem::ModemSpec::preferred_mapper
//! [`ModemSpec::preferred_soft_demapper`]: gf2_coding::modem::ModemSpec::preferred_soft_demapper

use std::process::ExitCode;

use gf2_coding::channel::AwgnChannel;
use gf2_coding::llr::Llr;
use gf2_coding::modem::{DemapInput, DemapMethod, ModemSpec};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn run() -> Result<(), String> {
    // ---- 1. Build the 16-QAM preset -------------------------------------
    // `gray_square_qam(16)` returns a validated 4-bit-per-symbol spec with
    // the DVB-T2 EN 302 755 bit-to-cell mapping and unit average symbol
    // energy. Construction itself panics on an invalid order, so any spec
    // handed to downstream code is already consistent.
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let m = spec.bits_per_symbol() as usize;
    println!(
        "Built {}-QAM spec: {} bits/symbol, {} constellation points",
        spec.num_symbols(),
        m,
        spec.num_symbols(),
    );

    // ---- 2. Fetch the preferred backends --------------------------------
    // `preferred_mapper` / `preferred_soft_demapper` inspect the spec and
    // route to the fast-path Gray-QAM backends when the geometry matches.
    // For a preset this always succeeds; for custom specs that don't match
    // the Gray layout they fall back to the reference path transparently.
    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();

    // ---- 3. Generate a deterministic batch of random bits ---------------
    // Seed the RNG so the example's output is reproducible across runs.
    let mut rng = StdRng::seed_from_u64(0xC0FFEE);
    let num_symbols = 4_096;
    let num_bits = num_symbols * m;
    let tx_bits: Vec<bool> = (0..num_bits).map(|_| rng.gen()).collect();

    // ---- 4. Map bits to I/Q symbols -------------------------------------
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&tx_bits, &mut tx_i, &mut tx_q);

    // ---- 5. Apply AWGN --------------------------------------------------
    // We pick a fixed Eb/N0 of 10 dB. `AwgnChannel::from_eb_n0_db` bakes
    // in BPSK `m = 1`, which is fine for the noise *generator* — the
    // framework's demapper re-derives the correct variance below. What we
    // need here is just a Gaussian source of the right variance per axis.
    //
    // For 16-QAM at Eb/N0 = 10 dB, rate = 1, the per-axis noise variance
    // is `sigma^2 = 1 / (2 * m * rate * 10^(Eb_N0_dB / 10))`.
    let eb_n0_db: f64 = 10.0;
    let sigma_sq = 1.0 / (2.0 * m as f64 * 1.0 * 10.0_f64.powf(eb_n0_db / 10.0));
    let channel = AwgnChannel::from_variance(sigma_sq);

    // Apply independent Gaussian noise on I and Q (the usual 2-D AWGN
    // convention for QAM).
    let mut rx_i = tx_i.clone();
    let mut rx_q = tx_q.clone();
    for s in rx_i.iter_mut() {
        *s = channel.transmit(*s as f64, &mut rng) as f32;
    }
    for s in rx_q.iter_mut() {
        *s = channel.transmit(*s as f64, &mut rng) as f32;
    }

    // ---- 6. Demap to per-bit LLRs ---------------------------------------
    // The framework demapper takes `N0 = 2 * sigma^2` per the module-level
    // noise convention.
    let n0 = 2.0 * sigma_sq as f32;
    let noise_var = vec![n0; num_symbols];
    let input = DemapInput::<f32> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &noise_var,
        method: DemapMethod::MaxLog,
    };
    let mut llrs = vec![Llr::new(0.0); num_bits];
    demapper.demap_llrs(input, &mut llrs);

    // ---- 7. Hard-decode the LLRs and compute BER ------------------------
    // Framework convention: positive LLR => bit 0, negative => bit 1.
    let errors: usize = tx_bits
        .iter()
        .zip(llrs.iter())
        .filter(|(b, l)| (**b) != (l.value() < 0.0))
        .count();
    let ber = errors as f64 / num_bits as f64;

    println!(
        "Eb/N0 = {:>4.1} dB  N0 = {:.4}  symbols = {}  bit errors = {}  BER = {:.3e}",
        eb_n0_db, n0, num_symbols, errors, ber,
    );

    // Sanity: at 10 dB Eb/N0 uncoded 16-QAM (max-log) should be well
    // below 5 %, modulo the small batch. Flag suspicious outputs.
    if ber > 0.1 {
        return Err(format!(
            "BER {ber:.3e} is suspiciously high at Eb/N0 = {eb_n0_db} dB; expected < 0.1",
        ));
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("modem_gray_qam_preset failed: {msg}");
            ExitCode::FAILURE
        }
    }
}
