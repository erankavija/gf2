//! BCJR (forward-backward) SISO decoder for linear block codes
//! (`@/citation/Bahl1974`) on the syndrome trellis (`@/citation/Wolf1978`,
//! `@/citation/McEliece1996`), in the log-MAP formulation.

use crate::grand::SisoResult;
use crate::llr::Llr;
use gf2_core::BitMatrix;

/// BCJR SISO decoder on the syndrome trellis of a parity-check matrix H: the
/// state at boundary `i` is the partial syndrome of bits `0..i`, and bit `i`
/// set XORs column `i` of H into it.
#[derive(Debug, Clone)]
pub struct BcjrDecoder {
    /// Column bitmasks of H: h_cols[i] is the i-th column as a u32.
    h_cols: Vec<u32>,
    /// Number of trellis states: 2^m where m = n-k.
    num_states: usize,
    n: usize,
    k: usize,
}

impl BcjrDecoder {
    /// Creates the decoder for the `m × n` parity-check matrix `h`, with `k = n - m`.
    ///
    /// # Panics
    ///
    /// Panics if `h` has more than 20 rows.
    pub fn new(h: &BitMatrix) -> Self {
        let m = h.rows();
        let n = h.cols();
        assert!(
            m <= 20,
            "Parity-check matrix has {} rows; BCJR trellis with 2^{} states is infeasible",
            m,
            m
        );
        let k = n - m;
        let num_states = 1usize << m;

        let h_cols = h.cols_as_u32_masks();

        Self {
            h_cols,
            num_states,
            n,
            k,
        }
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length (n - number of parity rows).
    pub fn k(&self) -> usize {
        self.k
    }

    /// Computes APP and extrinsic (APP minus input) LLRs; a positive LLR favours
    /// bit 0. `list_bler_prediction` and `query_count` of the result are zero.
    ///
    /// # Panics
    ///
    /// Panics if `combined_llrs.len() != n`.
    ///
    /// # Complexity
    ///
    /// O(n * 2^(n-k)) time and memory.
    pub fn decode_siso(&self, combined_llrs: &[Llr]) -> SisoResult {
        let n = self.n;
        assert_eq!(
            combined_llrs.len(),
            n,
            "Expected {} LLRs, got {}",
            n,
            combined_llrs.len()
        );

        let llr_vals: Vec<f32> = combined_llrs.iter().map(|l| l.value()).collect();

        let log_alpha = self.forward_pass(&llr_vals);

        let log_beta = self.backward_pass(&llr_vals);

        let mut app_llrs = Vec::with_capacity(n);
        let mut extrinsic_llrs = Vec::with_capacity(n);

        for i in 0..n {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            let mut log_p0 = f32::NEG_INFINITY;
            let mut log_p1 = f32::NEG_INFINITY;

            for s in 0..self.num_states {
                let a = log_alpha[i][s];
                if a == f32::NEG_INFINITY {
                    continue;
                }

                // c_i = 0: state s -> s
                log_p0 = max_star(log_p0, a + log_gamma_0 + log_beta[i + 1][s]);

                // c_i = 1: state s -> s ^ h_col
                log_p1 = max_star(log_p1, a + log_gamma_1 + log_beta[i + 1][s ^ h_col]);
            }

            let l_app = log_p0 - log_p1;
            let l_ext = l_app - llr_vals[i];
            app_llrs.push(Llr::new(l_app));
            extrinsic_llrs.push(Llr::new(l_ext));
        }

        SisoResult {
            app_llrs,
            extrinsic_llrs,
            list_bler_prediction: 0.0,
            query_count: 0,
        }
    }

    /// Returns the (n+1) x num_states log-alpha metrics.
    fn forward_pass(&self, llr_vals: &[f32]) -> Vec<Vec<f32>> {
        let n = self.n;
        let ns = self.num_states;

        let mut log_alpha = vec![vec![f32::NEG_INFINITY; ns]; n + 1];
        log_alpha[0][0] = 0.0;

        for i in 0..n {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            for s in 0..ns {
                let a = log_alpha[i][s];
                if a == f32::NEG_INFINITY {
                    continue;
                }

                // c_i = 0: state s -> s
                log_alpha[i + 1][s] = max_star(log_alpha[i + 1][s], a + log_gamma_0);

                // c_i = 1: state s -> s ^ h_col
                let s1 = s ^ h_col;
                log_alpha[i + 1][s1] = max_star(log_alpha[i + 1][s1], a + log_gamma_1);
            }

            normalize_log_probs(&mut log_alpha[i + 1]);
        }

        log_alpha
    }

    /// Returns the (n+1) x num_states log-beta metrics.
    fn backward_pass(&self, llr_vals: &[f32]) -> Vec<Vec<f32>> {
        let n = self.n;
        let ns = self.num_states;

        let mut log_beta = vec![vec![f32::NEG_INFINITY; ns]; n + 1];
        log_beta[n][0] = 0.0;

        for i in (0..n).rev() {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            for s in 0..ns {
                // c_i = 0: state s -> s (forward), so backward: beta[i][s] += beta[i+1][s] * gamma_0
                let b_same = log_beta[i + 1][s];
                if b_same != f32::NEG_INFINITY {
                    log_beta[i][s] = max_star(log_beta[i][s], b_same + log_gamma_0);
                }

                // c_i = 1: state s -> s^h_col (forward), so backward: beta[i][s] += beta[i+1][s^h_col] * gamma_1
                let s1 = s ^ h_col;
                let b_xor = log_beta[i + 1][s1];
                if b_xor != f32::NEG_INFINITY {
                    log_beta[i][s] = max_star(log_beta[i][s], b_xor + log_gamma_1);
                }
            }

            normalize_log_probs(&mut log_beta[i]);
        }

        log_beta
    }
}

/// Jacobian logarithm max*(a, b) = max(a, b) + ln(1 + exp(-|a - b|)), which
/// equals ln(e^a + e^b); the correction term is omitted for |a - b| > 8.
#[inline]
fn max_star(a: f32, b: f32) -> f32 {
    if a == f32::NEG_INFINITY {
        return b;
    }
    if b == f32::NEG_INFINITY {
        return a;
    }
    let max_val = a.max(b);
    let diff = (a - b).abs();
    if diff > 8.0 {
        max_val
    } else {
        max_val + (-diff).exp().ln_1p()
    }
}

/// Subtracts the maximum, so the largest entry is 0.0; `NEG_INFINITY` entries stay.
fn normalize_log_probs(buf: &mut [f32]) {
    let max_val = buf.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if max_val == f32::NEG_INFINITY {
        return;
    }
    for v in buf.iter_mut() {
        if *v != f32::NEG_INFINITY {
            *v -= max_val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drm::DrmCode;
    use crate::traits::BlockEncoder;
    use gf2_core::BitVec;

    fn hamming74_h() -> BitMatrix {
        gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1
        ]
    }

    /// All length-n words with zero syndrome, found by testing each of the 2^n words.
    fn enumerate_codewords(h: &BitMatrix) -> Vec<BitVec> {
        let n = h.cols();
        let mut codewords = Vec::new();
        for pattern in 0..(1u64 << n) {
            let mut word = BitVec::with_capacity(n);
            for j in 0..n {
                word.push_bit((pattern >> j) & 1 == 1);
            }
            let mut valid = true;
            for row in 0..h.rows() {
                let mut sum = false;
                for col in 0..n {
                    if h.get(row, col) && word.get(col) {
                        sum = !sum;
                    }
                }
                if sum {
                    valid = false;
                    break;
                }
            }
            if valid {
                codewords.push(word);
            }
        }
        codewords
    }

    /// APP LLRs by enumeration over all codewords.
    fn exhaustive_app_llrs(h: &BitMatrix, combined_llrs: &[f32]) -> Vec<f32> {
        let n = h.cols();
        let codewords = enumerate_codewords(h);

        // Up to a constant, log P(y_i | c_i=0) = L_i/2 and log P(y_i | c_i=1) = -L_i/2.
        let log_probs: Vec<f64> = codewords
            .iter()
            .map(|cw| {
                combined_llrs
                    .iter()
                    .enumerate()
                    .map(|(j, &l)| {
                        let l = l as f64;
                        if cw.get(j) {
                            -l / 2.0
                        } else {
                            l / 2.0
                        }
                    })
                    .sum()
            })
            .collect();

        (0..n)
            .map(|i| {
                let mut log_sum_0 = f64::NEG_INFINITY;
                let mut log_sum_1 = f64::NEG_INFINITY;
                for (j, cw) in codewords.iter().enumerate() {
                    if cw.get(i) {
                        log_sum_1 = log_sum_exp_f64(log_sum_1, log_probs[j]);
                    } else {
                        log_sum_0 = log_sum_exp_f64(log_sum_0, log_probs[j]);
                    }
                }
                (log_sum_0 - log_sum_1) as f32
            })
            .collect()
    }

    fn log_sum_exp_f64(a: f64, b: f64) -> f64 {
        if a == f64::NEG_INFINITY {
            return b;
        }
        if b == f64::NEG_INFINITY {
            return a;
        }
        let max_val = a.max(b);
        max_val + ((a - max_val).exp() + (b - max_val).exp()).ln()
    }

    #[test]
    fn test_noiseless_all_zeros() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        let input: Vec<Llr> = vec![Llr::new(10.0); 32];
        let result = decoder.decode_siso(&input);

        assert_eq!(result.app_llrs.len(), 32);
        for (i, l) in result.app_llrs.iter().enumerate() {
            assert!(
                l.value() > 0.0,
                "APP LLR at bit {} should be positive (favoring 0), got {}",
                i,
                l.value()
            );
        }
        for l in &result.app_llrs {
            assert!(!l.hard_decision(), "Hard decision should be 0 (false)");
        }
    }

    #[test]
    fn test_noiseless_known_codeword() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        let mut msg = BitVec::with_capacity(21);
        for i in 0..21 {
            msg.push_bit(i % 3 == 0);
        }
        let cw = code.encode(&msg);

        let input: Vec<Llr> = (0..32)
            .map(|j| {
                if cw.get(j) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let result = decoder.decode_siso(&input);

        for (j, app) in result.app_llrs.iter().enumerate() {
            let hard = app.hard_decision();
            assert_eq!(
                hard,
                cw.get(j),
                "Hard decision mismatch at bit {}: got {}, expected {}",
                j,
                hard,
                cw.get(j)
            );
        }
    }

    #[test]
    fn test_hamming74_vs_exhaustive() {
        let h = hamming74_h();
        let decoder = BcjrDecoder::new(&h);

        let test_vectors: Vec<Vec<f32>> = vec![
            vec![2.0, -1.5, 3.0, 0.5, -2.0, 1.0, -0.5],
            vec![-3.0, 2.0, 1.0, -1.0, 0.5, -2.5, 3.0],
            vec![1.5, 1.5, -2.0, 2.5, -1.0, 0.8, -1.5],
            vec![4.0, -3.0, 2.0, -1.0, 3.0, -2.0, 1.0],
            vec![-0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5],
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
            vec![-5.0, -5.0, -5.0, -5.0, -5.0, -5.0, -5.0],
            vec![0.1, -0.1, 0.2, -0.3, 0.4, -0.5, 0.6],
        ];

        for (idx, llr_vals) in test_vectors.iter().enumerate() {
            let input: Vec<Llr> = llr_vals.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);
            let expected = exhaustive_app_llrs(&h, llr_vals);

            for (j, (app, &exp)) in result.app_llrs.iter().zip(expected.iter()).enumerate() {
                let diff = (app.value() - exp).abs();
                assert!(
                    diff < 0.15,
                    "Vector {}, bit {}: BCJR={:.4}, exhaustive={:.4}, diff={:.4}",
                    idx,
                    j,
                    app.value(),
                    exp,
                    diff
                );
            }
        }
    }

    #[test]
    fn test_extrinsic_correct_sign_and_identity() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        let input: Vec<Llr> = vec![Llr::new(10.0); 32];
        let result = decoder.decode_siso(&input);

        for (i, l) in result.extrinsic_llrs.iter().enumerate() {
            assert!(
                l.value() > 0.0,
                "Extrinsic LLR at bit {} should be positive for all-zero cw, got {}",
                i,
                l.value()
            );
            assert!(
                l.value().is_finite(),
                "Extrinsic LLR at bit {} should be finite, got {}",
                i,
                l.value()
            );
        }

        for (i, ((app, ext), inp)) in result
            .app_llrs
            .iter()
            .zip(result.extrinsic_llrs.iter())
            .zip(input.iter())
            .enumerate()
        {
            let expected_ext = app.value() - inp.value();
            let actual_ext = ext.value();
            assert!(
                (expected_ext - actual_ext).abs() < 1e-4,
                "Extrinsic identity mismatch at bit {}: {} vs {}",
                i,
                actual_ext,
                expected_ext
            );
        }
    }

    #[test]
    fn test_forward_starts_and_ends_zero() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        let llr_vals: Vec<f32> = vec![10.0; 32];
        let log_alpha = decoder.forward_pass(&llr_vals);

        assert!(log_alpha[0][0].is_finite());
        for (s, &val) in log_alpha[0].iter().enumerate().skip(1) {
            assert_eq!(
                val,
                f32::NEG_INFINITY,
                "State {} at boundary 0 should be -inf",
                s
            );
        }

        let end = &log_alpha[32];
        let max_other = end[1..].iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            end[0] > max_other + 5.0,
            "State 0 at end ({}) should dominate others (max other: {})",
            end[0],
            max_other
        );
    }

    #[test]
    fn test_ebch_noiseless() {
        use crate::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
        use crate::traits::block::{
            BlockEncoder as CanonicalBlockEncoder, ParityCheckMatrixAccess,
        };
        use crate::transform::Extended;
        use gf2_core::field::extension::BinaryPrimeExt;
        use gf2_core::gf2m::Gf2mField;

        let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())
            .expect("a valid binary BCH extension");
        let base = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(3)
                .expect("a positive BCH designed distance"),
        })
        .expect("a valid binary BCH construction");
        let code = Extended::new(base).expect("an extended BCH code fits in memory");
        let h = code
            .parity_check_matrix()
            .expect("extended BCH parity matrix");
        let decoder = BcjrDecoder::new(&h);
        assert_eq!(decoder.n(), 16);
        assert_eq!(decoder.k(), 11);

        let mut msg = BitVec::with_capacity(11);
        for i in 0..11 {
            msg.push_bit(i % 2 == 0);
        }
        let cw = CanonicalBlockEncoder::encode(&code, &msg).expect("valid eBCH encode");

        let input: Vec<Llr> = (0..16)
            .map(|j| {
                if cw.get(j) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let result = decoder.decode_siso(&input);

        for (j, app) in result.app_llrs.iter().enumerate() {
            let hard = app.hard_decision();
            assert_eq!(
                hard,
                cw.get(j),
                "eBCH hard decision mismatch at bit {}: got {}, expected {}",
                j,
                hard,
                cw.get(j)
            );
        }
    }

    #[test]
    fn test_siso_result_metadata() {
        let h = hamming74_h();
        let decoder = BcjrDecoder::new(&h);
        let input: Vec<Llr> = vec![Llr::new(3.0); 7];
        let result = decoder.decode_siso(&input);

        assert_eq!(result.list_bler_prediction, 0.0);
        assert_eq!(result.query_count, 0);
        assert_eq!(result.app_llrs.len(), 7);
        assert_eq!(result.extrinsic_llrs.len(), 7);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_hamming74_bcjr_matches_exhaustive(
            llrs in proptest::collection::vec(-10.0f32..10.0f32, 7..=7)
        ) {
            let h = hamming74_h();
            let decoder = BcjrDecoder::new(&h);
            let input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);
            let expected = exhaustive_app_llrs(&h, &llrs);

            for (j, (app, &exp)) in result.app_llrs.iter().zip(expected.iter()).enumerate() {
                let diff = (app.value() - exp).abs();
                prop_assert!(
                    diff < 0.2,
                    "bit {}: BCJR={:.4}, exhaustive={:.4}, diff={:.4}",
                    j, app.value(), exp, diff
                );
            }
        }

        #[test]
        #[ignore = "slow: proptest BCJR decode of dRM(32,21) trellis"]
        fn prop_drm_noiseless_recovery(msg_bits in proptest::collection::vec(any::<bool>(), 21..=21)) {
            let code = DrmCode::drm_32_21();
            let decoder = BcjrDecoder::new(code.parity_check());

            let mut msg = BitVec::with_capacity(21);
            for &b in &msg_bits {
                msg.push_bit(b);
            }
            let cw = code.encode(&msg);

            let input: Vec<Llr> = (0..32)
                .map(|j| if cw.get(j) { Llr::new(-8.0) } else { Llr::new(8.0) })
                .collect();

            let result = decoder.decode_siso(&input);

            for (j, app) in result.app_llrs.iter().enumerate() {
                prop_assert_eq!(
                    app.hard_decision(), cw.get(j),
                    "bit {}: hard={}, expected={}",
                    j, app.hard_decision(), cw.get(j)
                );
            }
        }

        #[test]
        fn prop_extrinsic_identity(
            llrs in proptest::collection::vec(-5.0f32..5.0f32, 7..=7)
        ) {
            let h = hamming74_h();
            let decoder = BcjrDecoder::new(&h);
            let input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);

            for (j, ((app, ext), inp)) in result.app_llrs.iter()
                .zip(result.extrinsic_llrs.iter())
                .zip(input.iter())
                .enumerate()
            {
                let expected_ext = app.value() - inp.value();
                let diff = (ext.value() - expected_ext).abs();
                prop_assert!(
                    diff < 1e-4,
                    "bit {}: ext={:.4}, expected={:.4}, diff={:.6}",
                    j, ext.value(), expected_ext, diff
                );
            }
        }
    }

    #[test]
    fn test_bcjr_vs_sogrand_extrinsic_drm() {
        use crate::grand::{OneLineIntercept, OrbGrand, OrbGrandConfig, SoGrand};

        let code = DrmCode::drm_32_21();
        let bcjr = BcjrDecoder::new(code.parity_check());

        let h = code.parity_check().clone();
        let sogrand = SoGrand::new(OrbGrand::new(
            h,
            OrbGrandConfig {
                list_size: 4,
                max_queries: 50_000,
                even_code: code.is_even(),
                systematic: true,
                list_bler_stop_threshold: None,
                one_line_intercept: OneLineIntercept::Auto,
            },
        ));

        let input: Vec<Llr> = vec![Llr::new(3.0); 32];

        let bcjr_result = bcjr.decode_siso(&input);
        let sogrand_result = sogrand.decode_siso(&input);

        let bcjr_ext_mean: f32 = bcjr_result
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        let sogrand_ext_mean: f32 = sogrand_result
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;

        eprintln!("BCJR mean |ext|: {bcjr_ext_mean:.3}");
        eprintln!("SOGRAND mean |ext|: {sogrand_ext_mean:.3}");
        eprintln!(
            "SOGRAND P(C\\L): {:.6}",
            sogrand_result.list_bler_prediction
        );

        let mut sign_agree = 0;
        let mut sign_disagree = 0;
        for i in 0..32 {
            let b = bcjr_result.extrinsic_llrs[i].value();
            let s = sogrand_result.extrinsic_llrs[i].value();
            if (b > 0.0) == (s > 0.0) {
                sign_agree += 1;
            } else {
                sign_disagree += 1;
            }
        }
        eprintln!("Sign agreement: {sign_agree}/32, disagree: {sign_disagree}/32");

        for (i, l) in bcjr_result.extrinsic_llrs.iter().enumerate() {
            assert!(
                l.value() > -1.0,
                "BCJR ext[{i}] = {:.3} should not be strongly negative for correct codeword",
                l.value()
            );
        }

        eprintln!("\n--- With 2 bit errors ---");
        let mut noisy_input: Vec<Llr> = vec![Llr::new(3.0); 32];
        noisy_input[5] = Llr::new(-1.5);
        noisy_input[12] = Llr::new(-0.8);

        let bcjr_noisy = bcjr.decode_siso(&noisy_input);
        let sogrand_noisy = sogrand.decode_siso(&noisy_input);

        let bcjr_ext_noisy: f32 = bcjr_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        let sogrand_ext_noisy: f32 = sogrand_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        eprintln!("BCJR mean |ext|: {bcjr_ext_noisy:.3}");
        eprintln!("SOGRAND mean |ext|: {sogrand_ext_noisy:.3}");

        eprintln!(
            "BCJR ext[5]={:.3} ext[12]={:.3}",
            bcjr_noisy.extrinsic_llrs[5].value(),
            bcjr_noisy.extrinsic_llrs[12].value()
        );
        eprintln!(
            "SOGRAND ext[5]={:.3} ext[12]={:.3}",
            sogrand_noisy.extrinsic_llrs[5].value(),
            sogrand_noisy.extrinsic_llrs[12].value()
        );

        let bcjr_max = bcjr_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .fold(0.0f32, f32::max);
        let sogrand_max = sogrand_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .fold(0.0f32, f32::max);
        eprintln!("BCJR max |ext|: {bcjr_max:.3}");
        eprintln!("SOGRAND max |ext|: {sogrand_max:.3}");
    }

    #[test]
    #[ignore] // enumerates 2^26 codewords for eBCH(32,26)
    fn test_weight_distribution_comparison() {
        use crate::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
        use crate::traits::block::{
            BlockCode as CanonicalBlockCode,
            GeneratorMatrixAccess as CanonicalGeneratorMatrixAccess,
        };
        use crate::traits::GeneratorMatrixAccess;
        use crate::transform::Extended;
        use gf2_core::field::extension::BinaryPrimeExt;
        use gf2_core::gf2m::Gf2mField;

        fn build_ebch(
            degree: usize,
            primitive_polynomial: u64,
            designed_distance: u64,
        ) -> Extended<BinaryBchCode> {
            let extension =
                BinaryPrimeExt::new(Gf2mField::new(degree, primitive_polynomial).with_tables())
                    .expect("a valid binary BCH extension");
            let base = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
                extension,
                designed_distance: DesignedDistance::try_from(designed_distance)
                    .expect("a positive BCH designed distance"),
            })
            .expect("a valid binary BCH construction");
            Extended::new(base).expect("an extended BCH code fits in memory")
        }

        let drm = DrmCode::drm_32_21();
        let g_drm = drm.generator_matrix();
        let n_drm = drm.n();
        let k_drm = drm.k();

        let mut drm_weights = [0u64; 33];
        for msg_val in 0..(1u64 << k_drm) {
            let mut cw_word = 0u64;
            for col in 0..n_drm {
                let mut bit = false;
                for row in 0..k_drm {
                    if (msg_val >> row) & 1 == 1 && g_drm.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            drm_weights[w] += 1;
        }

        eprintln!("dRM(32,21) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A8={}, A12={}, A16={}",
            drm_weights[0], drm_weights[4], drm_weights[8], drm_weights[12], drm_weights[16]
        );

        let ebch = build_ebch(4, 0b10011, 3);
        let g_ebch = CanonicalGeneratorMatrixAccess::generator_matrix(&ebch)
            .expect("extended BCH generator matrix");
        let n_ebch = CanonicalBlockCode::n(&ebch);
        let k_ebch = CanonicalBlockCode::k(&ebch);

        let mut ebch_weights = [0u64; 17];
        for msg_val in 0..(1u64 << k_ebch) {
            let mut cw_word = 0u64;
            for col in 0..n_ebch {
                let mut bit = false;
                for row in 0..k_ebch {
                    if (msg_val >> row) & 1 == 1 && g_ebch.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            ebch_weights[w] += 1;
        }

        eprintln!("eBCH(16,11) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A6={}, A8={}, A10={}, A12={}, A16={}",
            ebch_weights[0],
            ebch_weights[4],
            ebch_weights[6],
            ebch_weights[8],
            ebch_weights[10],
            ebch_weights[12],
            ebch_weights[16]
        );

        let ebch32 = build_ebch(5, 0b100101, 3);
        let g_32 = CanonicalGeneratorMatrixAccess::generator_matrix(&ebch32)
            .expect("extended BCH generator matrix");
        let n_32 = CanonicalBlockCode::n(&ebch32);
        let k_32 = CanonicalBlockCode::k(&ebch32);

        let mut e32_weights = [0u64; 33];
        for msg_val in 0..(1u64 << k_32) {
            let mut cw_word = 0u64;
            for col in 0..n_32 {
                let mut bit = false;
                for row in 0..k_32 {
                    if (msg_val >> row) & 1 == 1 && g_32.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            e32_weights[w] += 1;
        }

        eprintln!("eBCH(32,26) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A6={}, A8={}, A12={}, A16={}",
            e32_weights[0],
            e32_weights[4],
            e32_weights[6],
            e32_weights[8],
            e32_weights[12],
            e32_weights[16]
        );

        let drm_a4_ratio = drm_weights[4] as f64 / (1u64 << k_drm) as f64;
        let ebch_a4_ratio = ebch_weights[4] as f64 / (1u64 << k_ebch) as f64;
        let e32_a4_ratio = e32_weights[4] as f64 / (1u64 << k_32) as f64;

        eprintln!("\nA4 density:");
        eprintln!(
            "  dRM(32,21):  A4={}, ratio={:.6e}",
            drm_weights[4], drm_a4_ratio
        );
        eprintln!(
            "  eBCH(16,11): A4={}, ratio={:.6e}",
            ebch_weights[4], ebch_a4_ratio
        );
        eprintln!(
            "  eBCH(32,26): A4={}, ratio={:.6e}",
            e32_weights[4], e32_a4_ratio
        );
    }
}
