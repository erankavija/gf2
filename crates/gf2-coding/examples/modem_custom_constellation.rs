//! Non-Gray 8-PSK constellation built with `ModemSpecBuilder` and round-tripped through the
//! reference mapper and soft demapper.

use std::process::ExitCode;

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    unpack_label_msb_first, BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, LabelWord,
    ModemSpecBuilder, ReferenceMapper, ReferenceSoftDemapper, SymbolPoint,
};

fn run() -> Result<(), String> {
    let m: u8 = 3;
    let num_symbols = 1usize << m;
    let points: Vec<SymbolPoint<f32>> = (0..num_symbols)
        .map(|k| {
            let theta = (k as f32) * core::f32::consts::PI / 4.0;
            SymbolPoint::new(theta.cos(), theta.sin())
        })
        .collect();

    // Non-Gray labelling: several adjacent points differ in two or three bits.
    let labels_perm: [u16; 8] = [0b011, 0b001, 0b110, 0b100, 0b000, 0b111, 0b010, 0b101];
    let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, m)).collect();

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

    let mapper = ReferenceMapper::new(spec.clone());
    let demapper = ReferenceSoftDemapper::new(spec.clone());

    let tx_bits: Vec<bool> = (0..num_symbols as u16)
        .flat_map(|k| unpack_label_msb_first(k, m))
        .collect();

    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&tx_bits, &mut tx_i, &mut tx_q);

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
