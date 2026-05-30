//! End-to-end DVB-T2 BICM chain integration tests.
//!
//! Verifies the documented forward + inverse composition:
//!
//! ```text
//! BBFRAME → BCH encode → LDPC encode → bit interleave → QAM map
//!                                                             ↓
//!                                                           AWGN (skipped: noiseless)
//!                                                             ↓
//!         BCH decode ← LDPC decode ← bit deinterleave ← QAM demap
//! ```
//!
//! # Noiseless test strategy
//!
//! The full BICM roundtrip tests (both slow- and fast-tier) route through the
//! actual QAM mapper and soft demapper using a very small noise variance
//! (1e-6), which produces high-confidence LLRs equivalent to noiseless
//! reception.  The chain exercised is:
//!
//! ```text
//! BBFRAME -> BCH+LDPC encode -> interleave
//!        -> mapper.map_bits -> (noiseless receive, noise_var = 1e-6)
//!        -> demapper.demap_llrs -> deinterleave_llrs -> decode_soft -> BBFRAME
//! ```
//!
//! The fast-tier LLR-path tests (`test_interleaver_llr_path_identity_*`) use
//! `noiseless_llrs_from_bitvec` for structural verification of the interleaver
//! permutation only -- no LDPC encoding or QAM involved.
//!
//! # Tier
//!
//! The 3 x 2 Normal-frame roundtrip tests are marked
//! `#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]`.
//! With the IRA staircase encoder the first `concat.encode` is no longer the
//! bottleneck, but the full chain on Normal frames (n=64800), running BCH
//! and LDPC encode, interleave, QAM map, AWGN, soft demap, deinterleave,
//! and BP decode end-to-end, still exceeds the 5 s per-test fast-tier
//! budget on release builds under contention. A single Short x Rate 1/2 x
//! QPSK fast-tier roundtrip test provides immediate full-chain coverage
//! without hitting the slow threshold.
//!
//! # In-scope configurations
//!
//! Normal frame × {Rate 1/2, 2/3, 3/4} × {16-QAM, 64-QAM} = 6 combinations.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::llr::Llr;
use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Modulation order for a [`DvbT2Modulation`] variant.
fn qam_order(modulation: DvbT2Modulation) -> usize {
    match modulation {
        DvbT2Modulation::Qpsk => 4,
        DvbT2Modulation::Qam16 => 16,
        DvbT2Modulation::Qam64 => 64,
    }
}

/// Build a deterministic pseudo-random BBFRAME of `k_bch` bits using an LCG.
///
/// Seed: `seed` (MMIX Knuth LCG). Used so tests are deterministic and bit
/// patterns vary across configurations.
fn random_bbframe(k_bch: usize, seed: u64) -> BitVec {
    let mut state = seed;
    let mut bv = BitVec::with_capacity(k_bch);
    for _ in 0..k_bch {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        bv.push_bit((state >> 63) != 0);
    }
    bv
}

/// Convert a [`BitVec`] to a flat `Vec<bool>` (for the mapper).
fn bitvec_to_bools(bv: &BitVec) -> Vec<bool> {
    (0..bv.len()).map(|i| bv.get(i)).collect()
}

/// Synthesize noiseless LLRs from a [`BitVec`]: bit 0 → +magnitude, bit 1 → −magnitude.
///
/// These LLRs are in the same order as the bits in `bv`. When `bv` is the
/// *interleaved* FECFRAME, the result is what a noiseless QAM demapper would
/// produce (in interleaved output order), ready for `deinterleave_llrs`.
fn noiseless_llrs_from_bitvec(bv: &BitVec, magnitude: f32) -> Vec<Llr> {
    (0..bv.len())
        .map(|i| {
            if bv.get(i) {
                Llr::new(-magnitude)
            } else {
                Llr::new(magnitude)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Fast-tier: QAM map/demap path smoke tests (no LDPC encode/decode)
// ---------------------------------------------------------------------------

/// Verify the QAM mapper → noiseless LLR path is consistent with the
/// bit-interleaver output order for 16-QAM Normal × Rate 1/2.
///
/// This test does NOT call `DvbT2Concat::encode`. It exercises:
///   bits → interleave → QAM map → noiseless demap → deinterleave LLRs
/// on a small fixed pattern to confirm the mapper/demapper wiring is correct
/// independently of the slow LDPC encoder.
#[test]
fn test_qam_mapper_demap_path_16qam_normal_rate1_2() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits(); // 64800

    // Build a pattern FECFRAME (all zeros — known good for structural check).
    let fecframe = BitVec::zeros(n);

    // Interleave the FECFRAME bits.
    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), n);

    // Build a 16-QAM spec and map the interleaved bits to symbols.
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let m = spec.bits_per_symbol() as usize; // 4
    let num_symbols = n / m;
    assert_eq!(n % m, 0, "frame_bits must be a multiple of bits_per_symbol");

    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();

    let interleaved_bits: Vec<bool> = bitvec_to_bools(&interleaved);
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

    // Noiseless demap: very small noise_var so LLR magnitudes are large.
    let noise_var = vec![1e-6_f32; num_symbols];
    let mut out_llrs = vec![Llr::new(0.0); n];
    let input = DemapInput {
        rx_i: &tx_i,
        rx_q: &tx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &noise_var,
        method: DemapMethod::MaxLog,
    };
    demapper.demap_llrs(input, &mut out_llrs);

    // The interleaved LLRs are in interleaved-bit order.
    // Deinterleave them back to FECFRAME order.
    let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
    assert_eq!(fecframe_llrs.len(), n);

    // All bits in fecframe are 0, so all LLRs must be positive.
    for (i, &llr) in fecframe_llrs.iter().enumerate() {
        assert!(
            llr.value() > 0.0,
            "fecframe_llrs[{}] = {} should be positive for all-zero FECFRAME",
            i,
            llr.value()
        );
    }
}

/// Same smoke test for 64-QAM Normal × Rate 1/2.
#[test]
fn test_qam_mapper_demap_path_64qam_normal_rate1_2() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits(); // 64800

    let fecframe = BitVec::zeros(n);
    let interleaved = interleaver.interleave(&fecframe);

    let spec = ModemSpec::<f32>::gray_square_qam(64);
    let m = spec.bits_per_symbol() as usize; // 6
    let num_symbols = n / m;
    assert_eq!(n % m, 0);

    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();

    let interleaved_bits = bitvec_to_bools(&interleaved);
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

    let noise_var = vec![1e-6_f32; num_symbols];
    let mut out_llrs = vec![Llr::new(0.0); n];
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

    let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
    for (i, &llr) in fecframe_llrs.iter().enumerate() {
        assert!(
            llr.value() > 0.0,
            "fecframe_llrs[{}] = {} should be positive for all-zero FECFRAME",
            i,
            llr.value()
        );
    }
}

// ---------------------------------------------------------------------------
// Interleaver-path roundtrip without QAM: verify the noiseless LLR synthesis
// strategy works correctly by checking that `deinterleave_llrs` applied to
// noiseless LLRs in interleaved order recovers LLRs in FECFRAME order.
// This is a fast-tier structural test.
// ---------------------------------------------------------------------------

/// Verify that noiseless LLR synthesis → `deinterleave_llrs` → compare with
/// original FECFRAME bits sign is an identity for a known pseudo-random
/// FECFRAME. No LDPC encoding/decoding; tests the LLR path only.
#[test]
fn test_interleaver_llr_path_identity_16qam_normal() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits();

    // Pseudo-random FECFRAME.
    let mut state: u64 = 0xFEED_FACE_DEAD_BEEF;
    let mut fecframe = BitVec::with_capacity(n);
    for _ in 0..n {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        fecframe.push_bit((state >> 63) != 0);
    }

    // Forward interleave.
    let interleaved = interleaver.interleave(&fecframe);

    // Synthesize noiseless LLRs in interleaved order.
    let interleaved_llrs = noiseless_llrs_from_bitvec(&interleaved, 10.0);

    // Deinterleave the LLRs back to FECFRAME order.
    let fecframe_llrs = interleaver.deinterleave_llrs(&interleaved_llrs);

    // Each LLR sign must agree with the corresponding FECFRAME bit.
    for (i, &llr) in fecframe_llrs.iter().enumerate() {
        let bit = fecframe.get(i);
        let llr_positive = llr.value() > 0.0;
        assert_eq!(
            !bit,
            llr_positive,
            "sign mismatch at FECFRAME position {}: bit={}, llr={}",
            i,
            bit,
            llr.value()
        );
    }
}

/// Same LLR path identity test for 64-QAM Normal.
#[test]
fn test_interleaver_llr_path_identity_64qam_normal() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate2_3, DvbT2Modulation::Qam64);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits();

    let mut state: u64 = 0xCAFE_BABE_1234_5678;
    let mut fecframe = BitVec::with_capacity(n);
    for _ in 0..n {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        fecframe.push_bit((state >> 63) != 0);
    }

    let interleaved = interleaver.interleave(&fecframe);
    let interleaved_llrs = noiseless_llrs_from_bitvec(&interleaved, 10.0);
    let fecframe_llrs = interleaver.deinterleave_llrs(&interleaved_llrs);

    for (i, &llr) in fecframe_llrs.iter().enumerate() {
        let bit = fecframe.get(i);
        let llr_positive = llr.value() > 0.0;
        assert_eq!(
            !bit,
            llr_positive,
            "sign mismatch at FECFRAME position {}: bit={}, llr={}",
            i,
            bit,
            llr.value()
        );
    }
}

// ---------------------------------------------------------------------------
// Full end-to-end BICM roundtrip helper
//
// Used by both the fast-tier Short-frame test and the slow-tier Normal-frame
// tests.  Routes through the actual QAM mapper and soft demapper with
// noise_var = 1e-6 (effectively noiseless), matching the documented
// end-to-end chain from the example.
// ---------------------------------------------------------------------------

/// Run the full BICM chain for one (frame_size, rate, modulation) triple and
/// assert bit-exact BBFRAME recovery.
///
/// Chain:
///   BBFRAME -> concat.encode -> interleave
///          -> mapper.map_bits -> (noise_var = 1e-6)
///          -> demapper.demap_llrs -> deinterleave_llrs
///          -> concat.decode_soft -> BBFRAME
fn run_full_bicm_roundtrip_fs(
    frame_size: FrameSize,
    code_rate: CodeRate,
    modulation: DvbT2Modulation,
    seed: u64,
) {
    let concat = DvbT2Concat::new(frame_size, code_rate).expect("unsupported configuration");

    let modcod = DvbT2Modcod::new(frame_size, code_rate, modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);

    // Sanity: interleaver frame_bits must equal concat.n_ldpc().
    assert_eq!(
        interleaver.frame_bits(),
        concat.n_ldpc(),
        "interleaver.frame_bits() ({}) must equal concat.n_ldpc() ({}) for rate={:?} mod={:?}",
        interleaver.frame_bits(),
        concat.n_ldpc(),
        code_rate,
        modulation
    );

    // Step 1: build random BBFRAME.
    let bbframe_in = random_bbframe(concat.k_bch(), seed);

    // Step 2: BCH + LDPC encode -> FECFRAME.
    let fecframe = concat.encode(&bbframe_in);
    assert_eq!(fecframe.len(), concat.n_ldpc());

    // Step 3: bit interleave -> interleaved FECFRAME.
    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), concat.n_ldpc());

    // Step 4: QAM map -- interleaved bits -> I/Q symbols.
    let spec = ModemSpec::<f32>::gray_square_qam(qam_order(modulation));
    let m = spec.bits_per_symbol() as usize;
    let num_symbols = concat.n_ldpc() / m;
    assert_eq!(concat.n_ldpc() % m, 0);
    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();
    let interleaved_bits = bitvec_to_bools(&interleaved);
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

    // Step 5: QAM soft demap -- I/Q -> interleaved LLRs (noise_var = 1e-6).
    let noise_var = vec![1e-6_f32; num_symbols];
    let mut interleaved_llrs = vec![Llr::new(0.0); concat.n_ldpc()];
    demapper.demap_llrs(
        DemapInput {
            rx_i: &tx_i,
            rx_q: &tx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &noise_var,
            method: DemapMethod::MaxLog,
        },
        &mut interleaved_llrs,
    );

    // Step 6: deinterleave LLRs back to FECFRAME order.
    let fecframe_llrs = interleaver.deinterleave_llrs(&interleaved_llrs);
    assert_eq!(fecframe_llrs.len(), concat.n_ldpc());

    // Step 7: LDPC + BCH decode -> recovered BBFRAME.
    let bbframe_out = concat
        .decode_soft(&fecframe_llrs)
        .expect("LDPC decode failed");

    // Assert bit-exact recovery.
    assert_eq!(
        bbframe_out, bbframe_in,
        "BICM roundtrip mismatch for {:?} x {:?} x {:?}",
        frame_size, code_rate, modulation
    );
}

/// Convenience wrapper for Normal-frame roundtrip (used by slow-tier tests).
fn run_full_bicm_roundtrip(code_rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    run_full_bicm_roundtrip_fs(FrameSize::Normal, code_rate, modulation, seed);
}

// ---------------------------------------------------------------------------
// Fast-tier: full end-to-end BICM roundtrip — Short x Rate 1/2 x QPSK
//
// Short frame (n=16200) is ~16x cheaper than Normal (n=64800) on the
// BICM chain wall-clock, finishing well within the 5 s fast-tier limit.
// This test exercises the complete documented call sequence -- including
// QAM map/demap -- on a pseudo-random payload without hitting the slow tier.
// ---------------------------------------------------------------------------

/// Full BICM roundtrip via QAM map/demap: Short x Rate 1/2 x QPSK.
///
/// Exercises the complete chain:
///   BBFRAME -> BCH+LDPC encode -> interleave -> mapper.map_bits
///          -> demapper.demap_llrs (noise_var = 1e-6)
///          -> deinterleave_llrs -> decode_soft -> BBFRAME
///
/// Short frame keeps preprocessing within the 5 s fast-tier limit.
#[test]
fn test_bicm_roundtrip_short_rate1_2_qpsk() {
    run_full_bicm_roundtrip_fs(
        FrameSize::Short,
        CodeRate::Rate1_2,
        DvbT2Modulation::Qpsk,
        0xA1B2_C3D4_E5F6_0718,
    );
}

/// Full BICM roundtrip: Normal x Rate 1/2 x 16-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate1_2_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        0x1111_2222_3333_4444,
    );
}

/// Full BICM roundtrip: Normal × Rate 1/2 × 64-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate1_2_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam64,
        0x5555_6666_7777_8888,
    );
}

/// Full BICM roundtrip: Normal × Rate 2/3 × 16-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate2_3_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam16,
        0x9999_AAAA_BBBB_CCCC,
    );
}

/// Full BICM roundtrip: Normal × Rate 2/3 × 64-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate2_3_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam64,
        0xDDDD_EEEE_FFFF_0000,
    );
}

/// Full BICM roundtrip: Normal × Rate 3/4 × 16-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate3_4_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam16,
        0x0101_0202_0303_0404,
    );
}

/// Full BICM roundtrip: Normal × Rate 3/4 × 64-QAM.
#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate3_4_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam64,
        0x0505_0606_0707_0808,
    );
}

// ---------------------------------------------------------------------------
// Structural check: verify that frame_bits() == n_ldpc() for all in-scope
// (rate, modulation) pairs. This is a fast-tier sanity check that can
// catch parameter table mismatches without running the encoder.
// ---------------------------------------------------------------------------

/// Verify that `DvbT2BitInterleaver::frame_bits()` equals
/// `DvbT2Concat::n_ldpc()` for all 3 × 2 in-scope (rate, modulation) pairs.
///
/// This confirms the interleaver is parameterised for the correct FECFRAME
/// size and can accept the output of `DvbT2Concat::encode` directly.
#[test]
fn test_interleaver_frame_bits_matches_fecframe_size_all_in_scope() {
    let configs: &[(CodeRate, DvbT2Modulation)] = &[
        (CodeRate::Rate1_2, DvbT2Modulation::Qam16),
        (CodeRate::Rate1_2, DvbT2Modulation::Qam64),
        (CodeRate::Rate2_3, DvbT2Modulation::Qam16),
        (CodeRate::Rate2_3, DvbT2Modulation::Qam64),
        (CodeRate::Rate3_4, DvbT2Modulation::Qam16),
        (CodeRate::Rate3_4, DvbT2Modulation::Qam64),
    ];

    for &(rate, modulation) in configs {
        let codec = DvbT2Concat::new(FrameSize::Normal, rate).expect("construction failed");
        let modcod = DvbT2Modcod::new(FrameSize::Normal, rate, modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);

        assert_eq!(
            interleaver.frame_bits(),
            codec.n_ldpc(),
            "frame_bits mismatch for rate={:?} mod={:?}",
            rate,
            modulation
        );

        // Also verify n_ldpc is divisible by bits_per_cell for the mapper.
        let bpc = modulation.bits_per_cell();
        assert_eq!(
            codec.n_ldpc() % bpc,
            0,
            "n_ldpc ({}) not divisible by bits_per_cell ({}) for rate={:?} mod={:?}",
            codec.n_ldpc(),
            bpc,
            rate,
            modulation
        );
    }
}

// ---------------------------------------------------------------------------
// QAM order helper correctness
// ---------------------------------------------------------------------------

/// Verify `qam_order` helper returns the correct constellation order.
#[test]
fn test_qam_order_helper() {
    assert_eq!(qam_order(DvbT2Modulation::Qpsk), 4);
    assert_eq!(qam_order(DvbT2Modulation::Qam16), 16);
    assert_eq!(qam_order(DvbT2Modulation::Qam64), 64);
}
