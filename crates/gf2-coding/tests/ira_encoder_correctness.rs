//! Correctness tests for the IRA staircase accumulator encoder.
//!
//! Verifies three properties for all supported DVB-T2 configurations:
//!
//! 1. Syndrome zero: H · c = 0 for all produced codewords (fast tier; all
//!    12 (frame, rate) configs).
//! 2. Systematic: codeword[0..k] = info bits (fast tier; all 12 configs,
//!    asserted alongside the syndrome check).
//! 3. Bit-identity vs the RREF encoder. Together with (1) and (2) this
//!    establishes the [hard] bit-identity criterion directly. Marked
//!    `#[ignore = "slow:"]` for every config because RREF preprocessing
//!    is ~2-3 s per Short rate (load-sensitive at the 5 s per-test fast-
//!    tier hard kill) and minutes per Normal rate. Properties (1)+(2)
//!    algebraically imply bit-identity (uniqueness of the systematic
//!    codeword for a full-rank LDPC code), so the fast tier still has
//!    automated coverage of the criterion via the syndrome+systematic
//!    tests below.

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

// ---------------------------------------------------------------------------
// Short-frame syndrome tests (fast — IRA encoder, no RREF)
// ---------------------------------------------------------------------------

/// IRA encoder uses the fast path for all Short-frame DVB-T2 configs.
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

/// IRA encoder uses the fast path for all Normal-frame DVB-T2 configs.
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

/// H·c = 0 for 5 random messages per Short-frame rate.
#[test]
fn test_ira_short_syndrome_zero_all_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_short(rate);
        let enc = LdpcEncoder::new(code.clone());

        for seed in 0u8..5 {
            let msg = make_message(seed, code.k());
            let cw = enc.encode(&msg);

            assert_eq!(cw.len(), code.n(), "{rate:?} seed={seed}: codeword length");

            // Systematic property: first k bits = message
            for i in 0..code.k() {
                assert_eq!(
                    cw.get(i),
                    msg.get(i),
                    "{rate:?} seed={seed}: systematic bit {i}"
                );
            }

            // H·c = 0
            let syn = code.syndrome(&cw);
            assert_eq!(
                syn.count_ones(),
                0,
                "{rate:?} seed={seed}: syndrome must be zero"
            );
        }
    }
}

/// H·c = 0 and systematic property for 3 random messages per Normal-frame
/// rate (fast tier). Together with the Short-frame version above this gives
/// fast-tier automated coverage of the [hard] bit-identity criterion via
/// the algebraic equivalence; the direct RREF comparisons for Normal frames
/// (slow tier) appear later in this file.
#[test]
fn test_ira_normal_syndrome_zero_all_rates() {
    for rate in ALL_RATES {
        let code = LdpcCode::dvb_t2_normal(rate);
        let enc = LdpcEncoder::new(code.clone());

        for seed in 0u8..3 {
            let msg = make_message(seed, code.k());
            let cw = enc.encode(&msg);

            assert_eq!(cw.len(), code.n(), "{rate:?} seed={seed}: codeword length");

            // Systematic property
            for i in 0..code.k() {
                assert_eq!(
                    cw.get(i),
                    msg.get(i),
                    "{rate:?} seed={seed}: systematic bit {i}"
                );
            }

            // H·c = 0
            let syn = code.syndrome(&cw);
            assert_eq!(
                syn.count_ones(),
                0,
                "{rate:?} seed={seed}: syndrome must be zero"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Encode→decode roundtrip (Short frame, high-SNR, fast BP convergence)
// ---------------------------------------------------------------------------

/// Short Rate 1/2: encode with IRA then decode with BP (zero-error channel).
///
/// Verifies that BP decoding recovers the original message.
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

    // Perfect channel: convert bits to high-confidence LLRs.
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

    // Recovered message (first k bits of decoded codeword)
    let mut recovered = BitVec::with_capacity(code.k());
    for i in 0..code.k() {
        recovered.push_bit(result.decoded_bits.get(i));
    }
    assert_eq!(
        recovered, msg,
        "Roundtrip must recover the original message"
    );
}

// ---------------------------------------------------------------------------
// Bit-identity vs RREF — slow tier only. All twelve (Short and Normal) configs
// are covered below; each test is `#[ignore]` because RREF preprocessing is
// load-sensitive against the 5 s fast-tier hard kill (Short ~2-3 s, Normal
// several minutes). The fast-tier syndrome + systematic tests above
// algebraically imply this bit-identity.
// ---------------------------------------------------------------------------

/// Short Rate 1/2: IRA output must be bit-identical to RREF output.
///
/// Marked slow because RREF preprocessing of a Short DVB-T2 code takes
/// ~2-3 s on an idle host and substantially more under CPU contention,
/// which makes a direct RREF comparison unreliable inside the fast tier's
/// 5 s per-test hard kill. The [hard] bit-identity criterion is covered in
/// the fast tier algebraically by [`test_ira_short_syndrome_zero_all_rates`]
/// and [`test_ira_normal_syndrome_zero_all_rates`] above: those tests
/// verify both `H · c^T = 0` and `c[0..k] == message`, which together
/// uniquely identify the systematic codeword and therefore imply
/// bit-identity with any other correct systematic encoder (RREF included).
#[test]
#[ignore = "slow: RREF preprocessing for Short DVB-T2 takes ~2-3 s per rate (load-sensitive at 5 s fast-tier budget)"]
fn test_ira_vs_rref_short_rate_1_2() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let ira_enc = LdpcEncoder::new(code.clone());

    // Build the RREF encoder explicitly for comparison.
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

/// Short Rate 3/5: IRA output must be bit-identical to RREF output.
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

/// Short Rate 2/3: IRA output must be bit-identical to RREF output.
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

/// Short Rate 3/4: IRA output must be bit-identical to RREF output.
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

/// Short Rate 4/5: IRA output must be bit-identical to RREF output.
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

/// Short Rate 5/6: IRA output must be bit-identical to RREF output.
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

/// Normal Rate 1/2: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 1/2 takes several minutes"]
fn test_ira_vs_rref_normal_rate_1_2() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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

/// Normal Rate 3/5: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 3/5 takes several minutes"]
fn test_ira_vs_rref_normal_rate_3_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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

/// Normal Rate 2/3: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 2/3 takes several minutes"]
fn test_ira_vs_rref_normal_rate_2_3() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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

/// Normal Rate 3/4: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 3/4 takes several minutes"]
fn test_ira_vs_rref_normal_rate_3_4() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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

/// Normal Rate 4/5: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 4/5 takes several minutes"]
fn test_ira_vs_rref_normal_rate_4_5() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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

/// Normal Rate 5/6: IRA vs RREF bit identity (RREF takes several minutes).
#[test]
#[ignore = "slow: RREF preprocessing for Normal DVB-T2 Rate 5/6 takes several minutes"]
fn test_ira_vs_rref_normal_rate_5_6() {
    use gf2_coding::ldpc::encoding::RuEncodingMatrices;

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
