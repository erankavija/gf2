//! IRA staircase encoder tests for every supported DVB-T2 (frame, rate):
//! `H · c = 0`, systematic layout, and bit identity with the RREF encoder.
//! The first two determine the systematic codeword of a full-rank code
//! uniquely, so they imply the third.

mod common;

use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

const ALL_RATES: [CodeRate; 6] = [
    CodeRate::Rate1_2,
    CodeRate::Rate3_5,
    CodeRate::Rate2_3,
    CodeRate::Rate3_4,
    CodeRate::Rate4_5,
    CodeRate::Rate5_6,
];

/// Deterministic pseudo-random message seeded by `(seed, length)`.
fn make_message(seed: u8, length: usize) -> BitVec {
    let mut bv = BitVec::with_capacity(length);
    let mut state = seed as u32;
    for _ in 0..length {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        bv.push_bit((state >> 17) & 1 == 1);
    }
    bv
}

#[test]
fn test_ira_encoder_selected_for_all_short_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_short(rate);
        let enc = LdpcEncoder::new(code);
        assert!(
            enc.is_ira(),
            "DVB-T2 Short {rate:?} must use IRA encoder, not RREF"
        );
    }
}

#[test]
fn test_ira_encoder_selected_for_all_normal_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_normal(rate);
        let enc = LdpcEncoder::new(code);
        assert!(
            enc.is_ira(),
            "DVB-T2 Normal {rate:?} must use IRA encoder, not RREF"
        );
    }
}

#[test]
fn test_ira_short_syndrome_zero_all_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_short(rate);
        let enc = LdpcEncoder::new(code.clone());

        for seed in 0u8..5 {
            let msg = make_message(seed, code.k());
            let cw = enc.encode(&msg);

            assert_eq!(cw.len(), code.n(), "{rate:?} seed={seed}: codeword length");

            for i in 0..code.k() {
                assert_eq!(
                    cw.get(i),
                    msg.get(i),
                    "{rate:?} seed={seed}: systematic bit {i}"
                );
            }

            let syn = code.syndrome(&cw);
            assert_eq!(
                syn.count_ones(),
                0,
                "{rate:?} seed={seed}: syndrome must be zero"
            );
        }
    }
}

#[test]
fn test_ira_normal_syndrome_zero_all_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_normal(rate);
        let enc = LdpcEncoder::new(code.clone());

        for seed in 0u8..3 {
            let msg = make_message(seed, code.k());
            let cw = enc.encode(&msg);

            assert_eq!(cw.len(), code.n(), "{rate:?} seed={seed}: codeword length");

            for i in 0..code.k() {
                assert_eq!(
                    cw.get(i),
                    msg.get(i),
                    "{rate:?} seed={seed}: systematic bit {i}"
                );
            }

            let syn = code.syndrome(&cw);
            assert_eq!(
                syn.count_ones(),
                0,
                "{rate:?} seed={seed}: syndrome must be zero"
            );
        }
    }
}

#[test]
fn test_ira_short_rate_1_2_encode_decode_roundtrip() {
    use gf2_coding::ldpc::LdpcDecoder;
    use gf2_coding::llr::Llr;
    use gf2_coding::traits::IterativeSoftDecoder;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let enc = LdpcEncoder::new(code.clone());
    let mut dec = LdpcDecoder::new(code.clone());

    let msg = make_message(42, code.k());
    let cw = enc.encode(&msg);

    let llrs: Vec<Llr> = (0..cw.len())
        .map(|i| {
            if cw.get(i) {
                Llr::new(-10.0)
            } else {
                Llr::new(10.0)
            }
        })
        .collect();

    let result = dec.decode_iterative(&llrs, 50);
    assert!(result.converged, "BP must converge on error-free Short 1/2");
    assert!(
        result.syndrome_check_passed,
        "Syndrome must pass after decoding"
    );

    let mut recovered = BitVec::with_capacity(code.k());
    for i in 0..code.k() {
        recovered.push_bit(result.decoded_bits.get(i));
    }
    assert_eq!(
        recovered, msg,
        "Roundtrip must recover the original message"
    );
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate (load-sensitive at 5 s fast-tier budget)"]
fn test_ira_vs_rref_short_rate_1_2() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let ira_enc = LdpcEncoder::new(code.clone());

    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix())
        .expect("RREF preprocessing failed");

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());

        let ira_cw = ira_enc.encode(&msg);
        let ru_cw = ru.encode(&msg);

        assert_eq!(
            ira_cw, ru_cw,
            "Short Rate1/2 seed={seed}: IRA and RREF outputs must be bit-identical"
        );
    }
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate"]
fn test_ira_vs_rref_short_rate_3_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_5);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Short Rate3/5 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate"]
fn test_ira_vs_rref_short_rate_2_3() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate2_3);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Short Rate2/3 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate"]
fn test_ira_vs_rref_short_rate_3_4() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_4);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Short Rate3/4 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate"]
fn test_ira_vs_rref_short_rate_4_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate4_5);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Short Rate4/5 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate"]
fn test_ira_vs_rref_short_rate_5_6() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate5_6);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..20 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Short Rate5/6 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 1/2 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_1_2() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_1_2",
        "RREF preprocessing of the Normal DVB-T2 Rate 1/2 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate1/2 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 3/5 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_3_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_3_5",
        "RREF preprocessing of the Normal DVB-T2 Rate 3/5 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate3/5 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 2/3 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_2_3() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_2_3",
        "RREF preprocessing of the Normal DVB-T2 Rate 2/3 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate2_3);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate2/3 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 3/4 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_3_4() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_3_4",
        "RREF preprocessing of the Normal DVB-T2 Rate 3/4 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_4);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate3/4 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 4/5 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_4_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_4_5",
        "RREF preprocessing of the Normal DVB-T2 Rate 4/5 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate4_5);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate4/5 seed={seed}: IRA vs RREF mismatch"
        );
    }
}

#[test]
#[ignore = "bench: RREF preprocessing for Normal DVB-T2 Rate 5/6 takes several minutes; run on a quiesced host"]
fn test_ira_vs_rref_normal_rate_5_6() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    skip_unless_bench!(
        "test_ira_vs_rref_normal_rate_5_6",
        "RREF preprocessing of the Normal DVB-T2 Rate 5/6 parity matrix takes several minutes"
    );

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate5_6);
    let ira_enc = LdpcEncoder::new(code.clone());
    let ru = RuEncodingMatrices::preprocess(code.parity_check_matrix()).unwrap();

    for seed in 0u8..5 {
        let msg = make_message(seed, code.k());
        assert_eq!(
            ira_enc.encode(&msg),
            ru.encode(&msg),
            "Normal Rate5/6 seed={seed}: IRA vs RREF mismatch"
        );
    }
}
