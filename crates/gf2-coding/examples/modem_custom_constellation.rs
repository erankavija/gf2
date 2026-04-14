//! Example: non-Gray 8-PSK custom constellation via [`ModemSpecBuilder`].
//!
//! This example demonstrates the **custom-constellation workflow**:
//!
//! 1. Lay out an 8-point unit-circle 8-PSK constellation.
//! 2. Apply a *deliberately non-Gray* bit labelling so the framework's
//!    Gray-QAM detector (which drives `preferred_*` fast-path dispatch)
//!    classifies it as a generic spec.
//! 3. Build a validated [`ModemSpec`] through [`ModemSpecBuilder`]. The
//!    builder normalizes the constellation to unit average symbol energy
//!    and funnels every invariant through the spec's sealed validator.
//! 4. Round-trip a handful of symbols through the [`ReferenceMapper`] and
//!    [`ReferenceSoftDemapper`], which handle arbitrary constellations.
//!    At essentially zero noise the demapper's hard-decision output must
//!    match the transmitted bits exactly.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --example modem_custom_constellation --release
//! ```
//!
//! [`ModemSpec`]: gf2_coding::modem::ModemSpec
//! [`ModemSpecBuilder`]: gf2_coding::modem::ModemSpecBuilder
//! [`ReferenceMapper`]: gf2_coding::modem::ReferenceMapper
//! [`ReferenceSoftDemapper`]: gf2_coding::modem::ReferenceSoftDemapper

use std::process::ExitCode;

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, LabelWord, ModemSpecBuilder,
    ReferenceMapper, ReferenceSoftDemapper, SymbolPoint,
};

/// Unpacks a `u16` label into MSB-first bits of width `m`.
fn unpack_label(bits: u16, m: u8) -> Vec<bool> {
    (0..m).rev().map(|i| ((bits >> i) & 1) == 1).collect()
}

fn run() -> Result<(), String> {
    // ---- 1. 8-PSK geometry on the unit circle ---------------------------
    //
    // `theta_k = k * pi / 4`, so points walk counter-clockwise around the
    // unit circle starting on the positive I axis. Before builder-side
    // normalization every point has energy 1; after normalization the
    // same holds (they already average to 1). The scale factor will come
    // out to 1.0.
    let m: u8 = 3;
    let num_symbols = 1usize << m;
    let points: Vec<SymbolPoint<f32>> = (0..num_symbols)
        .map(|k| {
            let theta = (k as f32) * core::f32::consts::PI / 4.0;
            SymbolPoint::new(theta.cos(), theta.sin())
        })
        .collect();

    // ---- 2. Deliberately non-Gray bit labelling -------------------------
    //
    // Any permutation of `0..8` is a valid bijection and will pass the
    // builder's validator; this particular permutation changes two or
    // three bits between several adjacent constellation points, which is
    // the opposite of a Gray code. The framework will accept it, compute
    // the correct (exact log-MAP) LLRs, and route through the reference
    // soft-demapper because the Gray-QAM preset detector will (rightly)
    // refuse to claim it.
    let labels_perm: [u16; 8] = [0b011, 0b001, 0b110, 0b100, 0b000, 0b111, 0b010, 0b101];
    let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, m)).collect();

    // ---- 3. Build the spec ----------------------------------------------
    //
    // The builder defaults to unit-average-symbol-energy normalization.
    // It also fills in conservative `BitChannelSemantics::Opaque(k)`
    // defaults for every bit position — appropriate for a research
    // constellation with no closed-form per-bit analysis.
    let spec = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(m)
        .points(points)
        .labels(labels)
        .build();

    println!(
        "Built custom 8-PSK spec: {} symbols, bits/symbol = {}, unit-energy scale = {:.4}",
        spec.num_symbols(),
        spec.bits_per_symbol(),
        spec.normalization_scale(),
    );
    println!(
        "Spec recognized as Gray-QAM preset? {}",
        spec.is_gray_square_qam_preset(),
    );

    // ---- 4. Round-trip each of the 8 labels through the reference path --
    //
    // We drive the reference mapper directly (rather than going through
    // `preferred_mapper`) to make clear that the reference path works for
    // any spec — custom or preset. We exercise every constellation label
    // once.
    let mapper = ReferenceMapper::new(spec.clone());
    let demapper = ReferenceSoftDemapper::new(spec.clone());

    // Flatten "0, 1, ..., 7" into MSB-first-within-symbol bits.
    let tx_bits: Vec<bool> = (0..num_symbols as u16)
        .flat_map(|k| unpack_label(k, m))
        .collect();

    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&tx_bits, &mut tx_i, &mut tx_q);

    // Near-noiseless channel: `N0 = 1e-8`. The reference demapper will
    // essentially pick out the nearest constellation point for each
    // received symbol.
    let n0 = 1.0e-8_f32;
    let noise_var = vec![n0; num_symbols];
    let input = DemapInput::<f32> {
        rx_i: &tx_i,
        rx_q: &tx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &noise_var,
        method: DemapMethod::ExactLogMap,
    };

    let mut llrs = vec![Llr::new(0.0); tx_bits.len()];
    demapper.demap_llrs(input, &mut llrs);

    // ---- 5. Sanity checks and readable output ---------------------------
    //
    // At essentially zero noise the hard decisions must reproduce the
    // transmitted bits exactly. We print the first two round-trips in
    // full to make the mapping visually obvious.
    for k in 0..2 {
        let label_bits = &tx_bits[k * m as usize..(k + 1) * m as usize];
        let label_llrs = &llrs[k * m as usize..(k + 1) * m as usize];
        let label_decoded: Vec<bool> = label_llrs.iter().map(|l| l.value() < 0.0).collect();
        println!(
            "k={k}: tx={:?}  symbol=({:+.3},{:+.3})  rx_bits={:?}  llrs={:?}",
            label_bits,
            tx_i[k],
            tx_q[k],
            label_decoded,
            label_llrs
                .iter()
                .map(|l| format!("{:+.2}", l.value()))
                .collect::<Vec<_>>(),
        );
    }

    let errors: usize = tx_bits
        .iter()
        .zip(llrs.iter())
        .filter(|(b, l)| **b != (l.value() < 0.0))
        .count();
    if errors != 0 {
        return Err(format!(
            "round-trip at negligible noise produced {errors} bit errors; reference demapper disagrees with mapper",
        ));
    }
    println!(
        "All {} round-trip bits recovered exactly at N0 = {:.0e}.",
        tx_bits.len(),
        n0,
    );

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("modem_custom_constellation failed: {msg}");
            ExitCode::FAILURE
        }
    }
}
