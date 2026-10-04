//! Uncoded BER of the Gray-coded 16-QAM preset over AWGN through the spec's preferred mapper and
//! soft demapper.

use std::process::ExitCode;

use gf2_coding::channel::AwgnChannel;
use gf2_coding::llr::Llr;
use gf2_coding::modem::awgn_link::{
    unit_energy_n0_from_eb_n0_db, unit_energy_sigma_sq_from_eb_n0_db,
};
use gf2_coding::modem::{DemapInput, DemapMethod, ModemSpec};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn run() -> Result<(), String> {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let m = spec.bits_per_symbol() as usize;
    println!(
        "Built {}-QAM spec: {} bits/symbol, {} constellation points",
        spec.num_symbols(),
        m,
        spec.num_symbols(),
    );

    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();

    let mut rng = StdRng::seed_from_u64(0xC0FFEE);
    let num_symbols = 4_096;
    let num_bits = num_symbols * m;
    let tx_bits: Vec<bool> = (0..num_bits).map(|_| rng.gen()).collect();

    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&tx_bits, &mut tx_i, &mut tx_q);

    let eb_n0_db: f64 = 10.0;
    let rate = 1.0_f64; // uncoded
    let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db);
    let channel = AwgnChannel::from_variance(sigma_sq);

    let mut rx_i = tx_i.clone();
    let mut rx_q = tx_q.clone();
    for s in rx_i.iter_mut() {
        *s = channel.transmit(*s as f64, &mut rng) as f32;
    }
    for s in rx_q.iter_mut() {
        *s = channel.transmit(*s as f64, &mut rng) as f32;
    }

    let n0 = unit_energy_n0_from_eb_n0_db(m, rate, eb_n0_db) as f32;
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
