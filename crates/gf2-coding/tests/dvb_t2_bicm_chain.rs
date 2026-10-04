//! DVB-T2 BICM chain round trips: BCH and LDPC encode, bit interleave, QAM
//! map, soft demap at noise variance 1e-6, deinterleave, soft decode.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::llr::Llr;
use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn qam_order(modulation: DvbT2Modulation) -> usize {
    match modulation {
        DvbT2Modulation::Qpsk => 4,
        DvbT2Modulation::Qam16 => 16,
        DvbT2Modulation::Qam64 => 64,
    }
}

/// Pseudo-random BBFRAME from the MMIX LCG multiplier (`@/citation/Knuth1997`).
fn random_bbframe(k_bch: usize, seed: u64) -> BitVec {
    let mut state = seed;
    let mut bv = BitVec::with_capacity(k_bch);
    for _ in 0..k_bch {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        bv.push_bit((state >> 63) != 0);
    }
    bv
}

fn bitvec_to_bools(bv: &BitVec) -> Vec<bool> {
    (0..bv.len()).map(|i| bv.get(i)).collect()
}

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

#[test]
fn test_qam_mapper_demap_path_16qam_normal_rate1_2() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits(); // 64800

    let fecframe = BitVec::zeros(n);

    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), n);

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

    let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
    assert_eq!(fecframe_llrs.len(), n);

    for (i, &llr) in fecframe_llrs.iter().enumerate() {
        assert!(
            llr.value() > 0.0,
            "fecframe_llrs[{}] = {} should be positive for all-zero FECFRAME",
            i,
            llr.value()
        );
    }
}

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

#[test]
fn test_interleaver_llr_path_identity_16qam_normal() {
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let n = interleaver.frame_bits();

    let mut state: u64 = 0xFEED_FACE_DEAD_BEEF;
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

fn run_full_bicm_roundtrip_fs(
    frame_size: FrameSize,
    code_rate: CodeRate,
    modulation: DvbT2Modulation,
    seed: u64,
) {
    let concat = DvbT2Concat::new(frame_size, code_rate).expect("unsupported configuration");

    let modcod = DvbT2Modcod::new(frame_size, code_rate, modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);

    assert_eq!(
        interleaver.frame_bits(),
        concat.n_ldpc(),
        "interleaver.frame_bits() ({}) must equal concat.n_ldpc() ({}) for rate={:?} mod={:?}",
        interleaver.frame_bits(),
        concat.n_ldpc(),
        code_rate,
        modulation
    );

    let bbframe_in = random_bbframe(concat.k_bch(), seed);

    let fecframe = concat.encode(&bbframe_in);
    assert_eq!(fecframe.len(), concat.n_ldpc());

    let interleaved = interleaver.interleave(&fecframe);
    assert_eq!(interleaved.len(), concat.n_ldpc());

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

    let fecframe_llrs = interleaver.deinterleave_llrs(&interleaved_llrs);
    assert_eq!(fecframe_llrs.len(), concat.n_ldpc());

    let bbframe_out = concat
        .decode_soft(&fecframe_llrs)
        .expect("LDPC decode failed");

    assert_eq!(
        bbframe_out, bbframe_in,
        "BICM roundtrip mismatch for {:?} x {:?} x {:?}",
        frame_size, code_rate, modulation
    );
}

fn run_full_bicm_roundtrip(code_rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    run_full_bicm_roundtrip_fs(FrameSize::Normal, code_rate, modulation, seed);
}

#[test]
fn test_bicm_roundtrip_short_rate1_2_qpsk() {
    run_full_bicm_roundtrip_fs(
        FrameSize::Short,
        CodeRate::Rate1_2,
        DvbT2Modulation::Qpsk,
        0xA1B2_C3D4_E5F6_0718,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate1_2_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        0x1111_2222_3333_4444,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate1_2_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam64,
        0x5555_6666_7777_8888,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate2_3_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam16,
        0x9999_AAAA_BBBB_CCCC,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate2_3_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam64,
        0xDDDD_EEEE_FFFF_0000,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate3_4_16qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam16,
        0x0101_0202_0303_0404,
    );
}

#[test]
#[ignore = "slow: Normal-frame BICM roundtrip exceeds 5 s fast-tier budget"]
fn test_bicm_roundtrip_normal_rate3_4_64qam() {
    run_full_bicm_roundtrip(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam64,
        0x0505_0606_0707_0808,
    );
}

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

#[test]
fn test_qam_order_helper() {
    assert_eq!(qam_order(DvbT2Modulation::Qpsk), 4);
    assert_eq!(qam_order(DvbT2Modulation::Qam16), 16);
    assert_eq!(qam_order(DvbT2Modulation::Qam64), 64);
}
