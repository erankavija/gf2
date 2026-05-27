//! DVB-T2 BICM chain — documented canonical composition example.
//!
//! Demonstrates the end-to-end forward and inverse composition of the three
//! DVB-T2 BICM component pieces for the Normal × 1/2 × 16-QAM configuration:
//!
//! ```text
//! BBFRAME → BCH encode → LDPC encode → bit interleave → QAM map
//!                                                             ↓
//!                                                  (noiseless receive)
//!                                                             ↓
//!         BCH decode ← LDPC decode ← bit deinterleave ← QAM demap
//! ```
//!
//! # Component pieces
//!
//! | Component             | Type                    | Location                              |
//! |-----------------------|-------------------------|---------------------------------------|
//! | BCH + LDPC concat     | `DvbT2Concat`           | `ldpc::dvb_t2::concat`                |
//! | Bit interleaver       | `DvbT2BitInterleaver`   | `ldpc::dvb_t2::bit_interleaver`       |
//! | Gray-QAM mapper       | `ModemSpec::preferred_mapper`     | `modem`               |
//! | Gray-QAM soft demapper| `ModemSpec::preferred_soft_demapper` | `modem`            |
//!
//! # Payload flow
//!
//! **Forward (transmit) path:**
//!
//! 1. `DvbT2Concat::new(frame_size, code_rate)` — construct the BCH+LDPC codec.
//! 2. `DvbT2BitInterleaver::new(modcod)` — construct the column-row bit interleaver.
//! 3. `concat.encode(&bbframe)` → `fecframe: BitVec` (n_ldpc bits).
//! 4. `interleaver.interleave(&fecframe)` → `interleaved: BitVec` (n_ldpc bits, reordered).
//! 5. Convert `interleaved` to a flat `&[bool]` slice; call
//!    `mapper.map_bits(&bits, &mut tx_i, &mut tx_q)` → I/Q symbol arrays.
//!
//! **Inverse (receive) path:**
//!
//! 6. Receive I/Q (noiseless here); call `demapper.demap_llrs(input, &mut out_llrs)`.
//!    `out_llrs` are in **interleaved** order (one LLR per coded bit, symbol-major).
//! 7. `interleaver.deinterleave_llrs(&out_llrs)` → `fecframe_llrs: Vec<Llr>`
//!    (LLRs in original FECFRAME bit order).
//! 8. `concat.decode_soft(&fecframe_llrs)` → recovered `bbframe: BitVec`.
//!
//! # LLR sign convention
//!
//! Positive LLR → bit 0 more likely; negative LLR → bit 1 more likely.
//! This is the convention used throughout `gf2-coding` (see [`gf2_coding::llr::Llr`]).
//! Both the demapper output and `concat.decode_soft` input follow this convention.
//!
//! # Noiseless receive strategy
//!
//! This example synthesizes noiseless LLRs by passing the actual transmitted
//! symbols through the soft demapper with a very small noise variance (1e-6),
//! which produces large-magnitude LLRs. An AWGN channel can be substituted by
//! adding Gaussian noise to `tx_i`/`tx_q` before demapping.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --example dvb_t2_bicm_chain --release
//! ```

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::llr::Llr;
use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn main() {
    // ------------------------------------------------------------------
    // Configuration: Normal × Rate 1/2 × 16-QAM
    // ------------------------------------------------------------------
    let frame_size = FrameSize::Normal;
    let code_rate = CodeRate::Rate1_2;
    let modulation = DvbT2Modulation::Qam16;
    let qam_order: usize = 16; // matches DvbT2Modulation::Qam16

    println!("DVB-T2 BICM chain — Normal × Rate 1/2 × 16-QAM");
    println!("------------------------------------------------");

    // ------------------------------------------------------------------
    // Step 0: Construct the three component pieces
    // ------------------------------------------------------------------

    // BCH + LDPC concatenated codec.
    // DvbT2Concat::new is O(nnz) for decoder graph; LDPC encoder is lazy.
    let concat = DvbT2Concat::new(frame_size, code_rate).expect("unsupported configuration");
    println!(
        "Codec:       k_bch={} k_ldpc={} n_ldpc={}",
        concat.k_bch(),
        concat.k_ldpc(),
        concat.n_ldpc()
    );

    // Column-row bit interleaver for this MODCOD.
    let modcod = DvbT2Modcod::new(frame_size, code_rate, modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    println!(
        "Interleaver: Nc={} Nr={} frame_bits={}",
        interleaver.num_columns(),
        interleaver.num_rows(),
        interleaver.frame_bits()
    );

    // Gray-QAM mapper and soft demapper (shared-API factories; routes to
    // the optimized fast-path backend for the Gray-square-QAM preset).
    let spec = ModemSpec::<f32>::gray_square_qam(qam_order);
    let m = spec.bits_per_symbol() as usize; // 4 for 16-QAM
    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();
    println!(
        "Modem:       {}-QAM, {} bits/symbol, {} symbols/FECFRAME",
        qam_order,
        m,
        concat.n_ldpc() / m
    );
    println!();

    // ------------------------------------------------------------------
    // Step 1: Build a deterministic BBFRAME (pseudo-random payload)
    // ------------------------------------------------------------------
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

    // ------------------------------------------------------------------
    // Step 2: Forward path — BCH + LDPC encode
    // ------------------------------------------------------------------
    // NOTE: The first call to concat.encode triggers LdpcEncoder::new,
    // which preprocesses Richardson-Urbanke encoding matrices (O(n²/64)).
    // For Normal frames this takes 2-10 s. Subsequent calls are O(nnz).
    println!("Encoding... (first call initialises LDPC encoder, may take a few seconds)");
    let fecframe = concat.encode(&bbframe_in);
    assert_eq!(fecframe.len(), concat.n_ldpc());
    println!("FECFRAME: {} bits", fecframe.len());

    // ------------------------------------------------------------------
    // Step 3: Bit interleave
    // ------------------------------------------------------------------
    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), concat.n_ldpc());
    println!("Interleaved FECFRAME: {} bits", interleaved.len());

    // ------------------------------------------------------------------
    // Step 4: QAM map — interleaved bits → I/Q symbols
    // ------------------------------------------------------------------
    let num_symbols = concat.n_ldpc() / m;
    let interleaved_bits: Vec<bool> = (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);
    println!("QAM mapped: {} symbols (16-QAM)", num_symbols);

    // ------------------------------------------------------------------
    // Step 5: Noiseless receive (no AWGN added)
    // ------------------------------------------------------------------
    // In a real system, AWGN would be added to tx_i / tx_q here.
    // For this example we pass the transmitted symbols directly to the
    // demapper with a tiny noise_var (1e-6), producing high-confidence LLRs.
    println!("Receive: noiseless (noise_var = 1e-6, equivalent to Eb/N0 >> 30 dB)");

    // ------------------------------------------------------------------
    // Step 6: QAM soft demap — I/Q → interleaved LLRs
    // ------------------------------------------------------------------
    // out_llrs are in interleaved order: out_llrs[s * m + k] is the LLR
    // for bit k of symbol s (MSB-first, k=0 is MSB).
    let noise_var = vec![1e-6_f32; num_symbols];
    let mut out_llrs = vec![Llr::new(0.0); concat.n_ldpc()];
    demapper.demap_llrs(
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

    // ------------------------------------------------------------------
    // Step 7: Bit deinterleave LLRs → FECFRAME order
    // ------------------------------------------------------------------
    // deinterleave_llrs applies the inverse permutation:
    // fecframe_llrs[i] is the LLR for the bit at position i in the
    // original (pre-interleaved) FECFRAME.
    let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
    assert_eq!(fecframe_llrs.len(), concat.n_ldpc());
    println!(
        "Deinterleaved LLRs: {} (FECFRAME order)",
        fecframe_llrs.len()
    );

    // ------------------------------------------------------------------
    // Step 8: LDPC + BCH decode → recovered BBFRAME
    // ------------------------------------------------------------------
    let bbframe_out = concat
        .decode_soft(&fecframe_llrs)
        .expect("LDPC decode failed");
    assert_eq!(bbframe_out.len(), concat.k_bch());
    println!("Decoded BBFRAME: {} bits", bbframe_out.len());

    // ------------------------------------------------------------------
    // Verify bit-exact recovery
    // ------------------------------------------------------------------
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
