//! End-to-end DVB-T2 BICM chain for Normal × 1/2 × 16-QAM: `DvbT2Concat`
//! (BCH + LDPC) encoding, `DvbT2BitInterleaver`, `DvbT2CellMapper` cell
//! demultiplexing and mapping, then soft demapping of the transmitted cells at
//! noise variance 1e-6, LLR deinterleaving and `DvbT2Concat::decode_soft`.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::{DvbT2CellMapper, FrameSize};
use gf2_coding::llr::Llr;
use gf2_coding::modem::{DemapInput, DemapMethod};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn main() {
    let frame_size = FrameSize::Normal;
    let code_rate = CodeRate::Rate1_2;
    let modulation = DvbT2Modulation::Qam16;

    println!("DVB-T2 BICM chain — Normal × Rate 1/2 × 16-QAM");
    println!("------------------------------------------------");

    let concat = DvbT2Concat::new(frame_size, code_rate).expect("unsupported configuration");
    println!(
        "Codec:       k_bch={} k_ldpc={} n_ldpc={}",
        concat.k_bch(),
        concat.k_ldpc(),
        concat.n_ldpc()
    );

    let modcod = DvbT2Modcod::new(frame_size, code_rate, modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    println!(
        "Interleaver: Nc={} Nr={} frame_bits={}",
        interleaver.num_columns(),
        interleaver.num_rows(),
        interleaver.frame_bits()
    );

    let cell_mapper = DvbT2CellMapper::new(modcod);
    println!(
        "Modem:       16-QAM, {} bits/cell, {} cells/FECFRAME",
        cell_mapper.bits_per_cell(),
        cell_mapper.num_cells()
    );
    println!();

    let mut state: u64 = 0xDEAD_BEEF_CAFE_1234;
    let mut bbframe_in = BitVec::with_capacity(concat.k_bch());
    for _ in 0..concat.k_bch() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        bbframe_in.push_bit((state >> 63) != 0);
    }
    println!(
        "BBFRAME: {} bits (first byte = {:08b})",
        bbframe_in.len(),
        {
            let mut byte = 0u8;
            for bit in 0..8 {
                if bbframe_in.get(bit) {
                    byte |= 1 << (7 - bit);
                }
            }
            byte
        }
    );

    println!("Encoding...");
    let fecframe = concat.encode(&bbframe_in);
    assert_eq!(fecframe.len(), concat.n_ldpc());
    println!("FECFRAME: {} bits", fecframe.len());

    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), concat.n_ldpc());
    println!("Interleaved FECFRAME: {} bits", interleaved.len());

    let num_cells = cell_mapper.num_cells();
    let mut tx_i = vec![0.0_f32; num_cells];
    let mut tx_q = vec![0.0_f32; num_cells];
    cell_mapper.map(&interleaved, &mut tx_i, &mut tx_q);
    println!("Cell mapped: {} cells (16-QAM)", num_cells);

    println!("Receive: noiseless (noise_var = 1e-6, equivalent to Eb/N0 >> 30 dB)");

    // out_llrs[j] is the LLR of bit-interleaver output bit j.
    let noise_var = vec![1e-6_f32; num_cells];
    let mut out_llrs = vec![Llr::new(0.0); concat.n_ldpc()];
    cell_mapper.demap_llrs(
        DemapInput {
            rx_i: &tx_i,
            rx_q: &tx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &noise_var,
            method: DemapMethod::MaxLog,
        },
        &mut out_llrs,
    );
    println!("Demapped: {} LLRs (interleaved order)", out_llrs.len());

    let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
    assert_eq!(fecframe_llrs.len(), concat.n_ldpc());
    println!(
        "Deinterleaved LLRs: {} (FECFRAME order)",
        fecframe_llrs.len()
    );

    let bbframe_out = concat
        .decode_soft(&fecframe_llrs)
        .expect("LDPC decode failed");
    assert_eq!(bbframe_out.len(), concat.k_bch());
    println!("Decoded BBFRAME: {} bits", bbframe_out.len());

    let equal = bbframe_out == bbframe_in;
    if equal {
        println!();
        println!("PASS: roundtrip identity for Normal x 1/2 x 16-QAM");
    } else {
        let mismatches: usize = (0..bbframe_in.len())
            .filter(|&i| bbframe_in.get(i) != bbframe_out.get(i))
            .count();
        eprintln!(
            "FAIL: {} bit errors in recovered BBFRAME (out of {})",
            mismatches,
            bbframe_in.len()
        );
        std::process::exit(1);
    }
}
