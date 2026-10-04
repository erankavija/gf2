//! Chase-Pyndiah soft-input soft-output (SISO) decoder for turbo product codes
//! (`@/citation/Chase1972`, `@/citation/Pyndiah1998`).

use crate::llr::{Llr, ReliabilityPermutation};
use gf2_core::{BitMatrix, BitVec};

use super::{ProductCode, ProductComponent, TurboDecoderResult};

/// Configuration for the Chase-Pyndiah turbo product code decoder.
#[derive(Debug, Clone)]
pub struct ChasePyndiahConfig {
    /// Maximum number of row-column iteration pairs.
    pub max_iterations: usize,

    /// Number of least reliable positions to flip in the Chase search.
    ///
    /// The search generates 2^p candidate codewords per component decode.
    pub p: usize,

    /// Per-half-iteration extrinsic scaling schedule.
    ///
    /// `alpha[h]` scales the extrinsic LLRs at half-iteration `h`.
    /// Half-iteration 0 is the first row step, 1 is the first column step,
    /// 2 is the second row step, etc.
    ///
    /// If the schedule is shorter than the total number of half-iterations,
    /// its last entry repeats.
    pub alpha: Vec<f32>,

    /// Per-half-iteration extrinsic bound schedule.
    ///
    /// The extrinsic at half-iteration `h` is clamped to
    /// `beta[h] * mean_j |L_j|`.  A schedule shorter than the total number of
    /// half-iterations repeats its last entry.
    pub beta: Vec<f32>,
}

impl Default for ChasePyndiahConfig {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            p: 4,
            alpha: vec![0.0, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0],
            beta: vec![0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.2, 1.2],
        }
    }
}

/// Chase-Pyndiah turbo product code decoder.
///
/// The turbo loop alternates between row-wise and column-wise SISO decodes,
/// exchanging extrinsic information under the per-half-iteration alpha/beta
/// schedules.
pub struct ChasePyndiahDecoder<C: ProductComponent> {
    component: C,
    config: ChasePyndiahConfig,
    /// Component codeword length.
    n: usize,
    /// Component message length.
    k: usize,
    /// `h_cols[j]` is a bitmask where bit `r` is set if H[r][j] == 1.
    h_cols: Vec<u32>,
    product_code: ProductCode<C>,
}

impl<C: ProductComponent + Clone> ChasePyndiahDecoder<C> {
    /// Creates a Chase-Pyndiah decoder for the given component code.
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix has more than 32 rows (i.e., n - k > 32),
    /// since syndrome bitmasks are stored as `u32`.
    pub fn new(component: C, config: ChasePyndiahConfig) -> Self {
        let n = component.comp_n();
        let k = component.comp_k();
        let h = component.comp_parity_check();
        let n_checks = h.rows();
        assert!(
            n_checks <= 32,
            "Chase-Pyndiah requires n-k <= 32, got {n_checks}"
        );

        let h_cols: Vec<u32> = (0..n)
            .map(|j| {
                let mut mask = 0u32;
                for r in 0..n_checks {
                    if h.get(r, j) {
                        mask |= 1u32 << r;
                    }
                }
                mask
            })
            .collect();

        let product_code = ProductCode::new(component.clone());
        Self {
            component,
            config,
            n,
            k,
            h_cols,
            product_code,
        }
    }

    /// Decodes a received product codeword from `n^2` row-major channel LLRs,
    /// where a positive LLR favours bit 0.
    ///
    /// Decoding stops early when the hard-decision matrix forms a valid
    /// product codeword.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != n^2`.
    /// Panics if any LLR has a NaN magnitude and at least one iteration runs.
    /// Panics if `alpha` or `beta` is empty and at least one iteration runs.
    ///
    /// # Complexity
    ///
    /// O(I × n² × 2^p) for I iteration pairs, plus the component re-encodes
    /// and one product-codeword check per half-iteration.
    pub fn decode(&self, channel_llrs: &[Llr]) -> TurboDecoderResult {
        let n = self.n;
        let n_sq = n * n;
        assert_eq!(
            channel_llrs.len(),
            n_sq,
            "Channel LLR length {} must equal n^2 = {}",
            channel_llrs.len(),
            n_sq
        );

        let l_ch: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| channel_llrs[i * n + j].value()).collect())
            .collect();

        let mut l_a: Vec<Vec<f32>> = vec![vec![0.0; n]; n];

        let mut half_iter: usize = 0;

        for iteration in 0..self.config.max_iterations {
            let alpha_h = self.config.alpha[half_iter.min(self.config.alpha.len() - 1)];
            let beta_h = self.config.beta[half_iter.min(self.config.beta.len() - 1)];

            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for i in 0..n {
                let input: Vec<f32> = (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect();
                let w = self.chase_pyndiah_siso(&input, beta_h);
                for j in 0..n {
                    l_e[i][j] = w[j] - input[j];
                }
            }

            let l_total_row: Vec<Vec<f32>> = (0..n)
                .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j] + l_e[i][j]).collect())
                .collect();
            if self.check_early_termination(&l_total_row) {
                let decoded = self.extract_decoded_message(&l_total_row);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries: 0,
                    queries_per_bit: 0.0,
                };
            }

            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha_h * l_e[i][j];
                }
            }

            half_iter += 1;

            let alpha_h = self.config.alpha[half_iter.min(self.config.alpha.len() - 1)];
            let beta_h = self.config.beta[half_iter.min(self.config.beta.len() - 1)];

            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for j in 0..n {
                let input: Vec<f32> = (0..n).map(|i| l_ch[i][j] + l_a[i][j]).collect();
                let w = self.chase_pyndiah_siso(&input, beta_h);
                for i in 0..n {
                    l_e[i][j] = w[i] - input[i];
                }
            }

            let l_total_col: Vec<Vec<f32>> = (0..n)
                .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j] + l_e[i][j]).collect())
                .collect();
            if self.check_early_termination(&l_total_col) {
                let decoded = self.extract_decoded_message(&l_total_col);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries: 0,
                    queries_per_bit: 0.0,
                };
            }

            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha_h * l_e[i][j];
                }
            }

            half_iter += 1;
        }

        let final_llrs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect())
            .collect();
        let decoded = self.extract_decoded_message(&final_llrs);

        TurboDecoderResult {
            decoded_bits: decoded,
            iterations: self.config.max_iterations,
            converged: false,
            total_queries: 0,
            queries_per_bit: 0.0,
        }
    }

    /// Performs one Chase-Pyndiah SISO decode on a single row or column and
    /// returns the soft output `W` for the combined channel + a-priori LLRs
    /// `input`.
    ///
    /// # Complexity
    ///
    /// O(2^p * n) plus the component re-encodes.
    fn chase_pyndiah_siso(&self, input: &[f32], beta: f32) -> Vec<f32> {
        let n = self.n;
        let k = self.k;
        let p = self.config.p.min(n);

        let hard: Vec<bool> = input
            .iter()
            .copied()
            .map(Llr::new)
            .map(Llr::hard_decision)
            .collect();
        let reliability: Vec<f32> = input.iter().copied().map(f32::abs).collect();

        let least_reliable: Vec<usize> = ReliabilityPermutation::from_magnitudes(&reliability)
            .ascending()
            .iter()
            .take(p)
            .copied()
            .collect();

        let mean_reliability: f32 = reliability.iter().copied().sum::<f32>() / n as f32;

        let num_patterns = 1usize << p;
        let mut codewords: Vec<Vec<bool>> = Vec::with_capacity(num_patterns);
        let mut correlations: Vec<f32> = Vec::with_capacity(num_patterns);

        for pattern in 0..num_patterns {
            let mut candidate: Vec<bool> = hard.clone();
            for (bit_idx, &pos) in least_reliable.iter().enumerate() {
                if pattern & (1 << bit_idx) != 0 {
                    candidate[pos] = !candidate[pos];
                }
            }

            let syndrome = self.compute_syndrome(&candidate);
            let codeword = if syndrome == 0 {
                Some(candidate)
            } else {
                // Try single-error correction: find H column matching syndrome
                let corrected = (0..n).find(|&j| self.h_cols[j] == syndrome).map(|j| {
                    let mut c = candidate.clone();
                    c[j] = !c[j];
                    c
                });
                if corrected.is_some() {
                    corrected
                } else {
                    // Re-encode from systematic bits as fallback
                    let mut msg = BitVec::with_capacity(k);
                    for &bit in candidate.iter().take(k) {
                        msg.push_bit(bit);
                    }
                    let encoded = self.component.encode(&msg);
                    Some((0..n).map(|j| encoded.get(j)).collect())
                }
            };

            if let Some(cw) = codeword {
                // M = sum_j L_j * (1 - 2*bit_j)
                let corr: f32 = (0..n)
                    .map(|j| {
                        let bipolar = if cw[j] { -1.0f32 } else { 1.0 };
                        input[j] * bipolar
                    })
                    .sum();

                codewords.push(cw);
                correlations.push(corr);
            }
        }

        if codewords.is_empty() {
            return input.to_vec();
        }

        let (ml_idx, &ml_corr) = correlations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let ml_codeword = &codewords[ml_idx];

        // The raw W_j = c_d_j * (M_ML - M_comp_j) / 2 is unbounded in the LLR
        // domain when the competitor is far from ML, so the extrinsic
        // (W - input) is bounded by the beta schedule times the mean
        // reliability.
        let ext_bound = beta * mean_reliability;

        let mut w = vec![0.0f32; n];
        for i in 0..n {
            let ml_bipolar = if ml_codeword[i] { -1.0f32 } else { 1.0 };

            let mut best_comp_corr: Option<f32> = None;
            for (idx, corr) in correlations.iter().enumerate() {
                if idx == ml_idx {
                    continue;
                }
                if codewords[idx][i] != ml_codeword[i] {
                    best_comp_corr = Some(match best_comp_corr {
                        None => *corr,
                        Some(prev) => prev.max(*corr),
                    });
                }
            }

            let raw_w = match best_comp_corr {
                Some(comp_corr) => ml_bipolar * (ml_corr - comp_corr) / 2.0,
                // No competitor: zero extrinsic.
                None => input[i],
            };

            let ext = (raw_w - input[i]).clamp(-ext_bound, ext_bound);
            w[i] = input[i] + ext;
        }

        w
    }

    /// Returns the syndrome of `candidate` as a bitmask: the XOR of
    /// `h_cols[j]` over its set bits `j`.
    fn compute_syndrome(&self, candidate: &[bool]) -> u32 {
        let mut syndrome = 0u32;
        for (j, &bit) in candidate.iter().enumerate() {
            if bit {
                syndrome ^= self.h_cols[j];
            }
        }
        syndrome
    }

    /// Checks if the hard decision on the given LLR matrix forms a valid product codeword.
    fn check_early_termination(&self, llr_matrix: &[Vec<f32>]) -> bool {
        let n = self.n;
        let mut matrix = BitMatrix::zeros(n, n);
        for (i, row) in llr_matrix.iter().enumerate().take(n) {
            for (j, &val) in row.iter().enumerate().take(n) {
                if Llr::new(val).hard_decision() {
                    matrix.set(i, j, true);
                }
            }
        }
        self.product_code.is_valid_codeword(&matrix)
    }

    /// Extracts k^2 decoded message bits from the hard decision on an LLR matrix.
    ///
    /// For a systematic code, the message bits are in the top-left k x k submatrix.
    fn extract_decoded_message(&self, llr_matrix: &[Vec<f32>]) -> BitVec {
        let k = self.k;
        let mut msg = BitVec::with_capacity(k * k);
        for row in llr_matrix.iter().take(k) {
            for &val in row.iter().take(k) {
                msg.push_bit(Llr::new(val).hard_decision());
            }
        }
        msg
    }

    /// Returns a reference to the decoder configuration.
    pub fn config(&self) -> &ChasePyndiahConfig {
        &self.config
    }

    /// Returns a reference to the component code.
    pub fn component(&self) -> &C {
        &self.component
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::ExtendedBchComponent;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_config_default() {
        let config = ChasePyndiahConfig::default();
        assert_eq!(config.max_iterations, 8);
        assert_eq!(config.p, 4);
        assert_eq!(config.alpha.len(), 8);
        assert_eq!(config.beta.len(), 8);
        assert!((config.alpha[0] - 0.0).abs() < 1e-10);
        assert!((config.alpha[1] - 0.2).abs() < 1e-10);
        assert!((config.alpha[7] - 1.0).abs() < 1e-10);
        assert!((config.beta[0] - 0.2).abs() < 1e-10);
        assert!((config.beta[7] - 1.2).abs() < 1e-10);
    }

    #[test]
    #[should_panic(expected = "reliability magnitude cannot be NaN")]
    fn test_siso_uses_canonical_reliability_input_contract() {
        let component = ExtendedBchComponent::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());

        let mut input = vec![1.0; 16];
        input[0] = f32::NAN;
        let _ = decoder.chase_pyndiah_siso(&input, 0.5);
    }

    #[test]
    fn test_extract_message_uses_canonical_hard_decision_for_signed_zero_and_infinity() {
        let component = ExtendedBchComponent::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());

        let mut matrix = vec![vec![f32::INFINITY; 16]; 16];
        matrix[0][0] = -0.0;
        matrix[0][1] = f32::NEG_INFINITY;

        let decoded = decoder.extract_decoded_message(&matrix);
        assert!(!decoded.get(0), "signed zero must map to bit zero");
        assert!(decoded.get(1), "negative infinity must map to bit one");
    }

    #[test]
    fn test_syndrome_check_valid_codeword() {
        let component = ExtendedBchComponent::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(component.clone(), ChasePyndiahConfig::default());

        let mut msg = BitVec::with_capacity(11);
        for i in 0..11 {
            msg.push_bit(i % 3 == 0);
        }
        let codeword = component.encode(&msg);
        let candidate: Vec<bool> = (0..16).map(|j| codeword.get(j)).collect();

        let syndrome = decoder.compute_syndrome(&candidate);
        assert_eq!(
            syndrome, 0,
            "Valid codeword must have zero syndrome, got {syndrome:#010b}"
        );

        let mut corrupted = candidate.clone();
        corrupted[0] = !corrupted[0];
        let syndrome = decoder.compute_syndrome(&corrupted);
        assert_ne!(syndrome, 0, "Corrupted codeword must have nonzero syndrome");
    }

    #[test]
    fn test_chase_search_noiseless() {
        let component = ExtendedBchComponent::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(
            component.clone(),
            ChasePyndiahConfig {
                p: 3,
                ..ChasePyndiahConfig::default()
            },
        );

        let input: Vec<f32> = vec![10.0; 16];
        let w = decoder.chase_pyndiah_siso(&input, 0.5);

        for (i, &wi) in w.iter().enumerate() {
            assert!(
                wi > 0.0,
                "Soft output W[{i}] = {wi} should be positive for all-zeros input"
            );
        }
    }

    #[test]
    fn test_decode_all_zeros_high_snr() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = ChasePyndiahConfig {
            max_iterations: 4,
            p: 3,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(8.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged, "Should converge at high SNR");
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all-zeros"
        );
    }

    #[test]
    fn test_syndrome_check_valid_drm_codeword() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let decoder = ChasePyndiahDecoder::new(component.clone(), ChasePyndiahConfig::default());

        for seed in 0..10 {
            let mut msg = BitVec::with_capacity(21);
            for i in 0..21 {
                msg.push_bit((i + seed) % 3 == 0);
            }
            let cw = component.encode(&msg);
            let candidate: Vec<bool> = (0..32).map(|j| cw.get(j)).collect();
            let syndrome = decoder.compute_syndrome(&candidate);
            assert_eq!(
                syndrome, 0,
                "dRM codeword (seed={seed}) must have zero syndrome, got {syndrome:#010b}"
            );
        }
    }

    #[test]
    fn test_drm_chase_siso_component_decode() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let decoder = ChasePyndiahDecoder::new(
            component.clone(),
            ChasePyndiahConfig {
                p: 4,
                ..ChasePyndiahConfig::default()
            },
        );

        let input: Vec<f32> = vec![10.0; 32];
        let w = decoder.chase_pyndiah_siso(&input, 0.5);

        for (i, &wi) in w.iter().enumerate() {
            assert!(
                wi > 0.0,
                "dRM W[{i}] = {wi} should be positive for all-zeros input"
            );
        }

        let mut msg = BitVec::with_capacity(21);
        for i in 0..21 {
            msg.push_bit(i % 2 == 0);
        }
        let cw = component.encode(&msg);

        let input: Vec<f32> = (0..32)
            .map(|j| if cw.get(j) { -10.0 } else { 10.0 })
            .collect();
        let w = decoder.chase_pyndiah_siso(&input, 0.5);

        for (j, &wi) in w.iter().enumerate() {
            let expected_sign = if cw.get(j) { "negative" } else { "positive" };
            let correct = if cw.get(j) { wi < 0.0 } else { wi > 0.0 };
            assert!(
                correct,
                "dRM W[{j}] = {wi} should be {expected_sign} for bit={}",
                cw.get(j)
            );
        }
    }

    #[test]
    fn test_drm_turbo_first_iteration() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let config = ChasePyndiahConfig {
            max_iterations: 1,
            p: 4,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component, config);

        let n = 32;
        let llrs: Vec<Llr> = vec![Llr::new(4.0); n * n];

        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "Should converge at LLR=4.0 for all-zeros (got {} iters, {} errors)",
            result.iterations,
            result.decoded_bits.count_ones()
        );
    }

    #[test]
    fn test_drm_turbo_moderate_noise() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let product = ProductCode::new(component.clone());
        let n = 32;
        let config = ChasePyndiahConfig {
            max_iterations: 8,
            p: 4,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component.clone(), config);

        let mut llrs = vec![Llr::new(3.0); n * n];

        let error_positions = [5, 37, 100, 200, 350, 500, 700, 850];
        for &pos in &error_positions {
            if pos < n * n {
                llrs[pos] = Llr::new(-1.0);
            }
        }

        let result = decoder.decode(&llrs);

        let bit_errors = result.decoded_bits.count_ones();
        eprintln!(
            "dRM CP turbo with {} scattered errors: converged={}, iters={}, bit_errors={}/{}",
            error_positions.len(),
            result.converged,
            result.iterations,
            bit_errors,
            product.k()
        );

        assert!(
            bit_errors < 50,
            "Too many bit errors after turbo decode: {bit_errors}"
        );
    }

    #[test]
    fn test_drm_turbo_nonzero_message() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let product = ProductCode::new(component.clone());
        let n = 32;
        let k = 21;
        let config = ChasePyndiahConfig {
            max_iterations: 8,
            p: 4,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component.clone(), config);

        let mut msg = BitVec::with_capacity(k * k);
        for i in 0..(k * k) {
            msg.push_bit(i % 3 == 0);
        }
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..n * n)
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-8.0)
                } else {
                    Llr::new(8.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);

        let bit_errors = (0..k * k)
            .filter(|&i| result.decoded_bits.get(i) != msg.get(i))
            .count();

        assert_eq!(
            bit_errors, 0,
            "dRM CP turbo with non-trivial message at high SNR: {bit_errors} bit errors \
             (converged={}, iters={})",
            result.converged, result.iterations
        );
    }

    #[test]
    #[ignore = "slow: Chase-Pyndiah turbo decode with AWGN on dRM(32,21) product"]
    fn test_drm_turbo_with_awgn() {
        use crate::drm::DrmCode;
        use crate::simulation::{BpskAwgnChannel, ChannelModel};
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let component = DrmCode::drm_32_21();
        let product = ProductCode::new(component.clone());
        let n = 32;
        let k = 21;
        let config = ChasePyndiahConfig {
            max_iterations: 8,
            p: 4,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component.clone(), config);
        let channel = BpskAwgnChannel;
        let rate = (k * k) as f64 / (n * n) as f64;

        let mut rng = StdRng::seed_from_u64(42);
        let mut total_errors = 0;
        let num_frames = 5;

        for frame in 0..num_frames {
            let msg = BitVec::random(k * k, &mut rng);
            let cw = product.encode(&msg);
            let llrs = channel.transmit_and_demodulate(&cw, 3.0, rate, &mut rng);

            let result = decoder.decode(&llrs);
            let bit_errors = (0..k * k)
                .filter(|&i| result.decoded_bits.get(i) != msg.get(i))
                .count();

            eprintln!(
                "Frame {frame}: converged={}, iters={}, bit_errors={}/{} (BER={:.4})",
                result.converged,
                result.iterations,
                bit_errors,
                k * k,
                bit_errors as f64 / (k * k) as f64
            );
            total_errors += bit_errors;
        }

        let avg_ber = total_errors as f64 / (num_frames * k * k) as f64;
        eprintln!("Average BER over {num_frames} frames: {avg_ber:.4}");

        assert!(
            avg_ber < 0.10,
            "Average BER {avg_ber:.4} is too high — decoder is degrading the signal"
        );
    }

    #[test]
    fn test_drm_chase_extrinsic_sign() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let decoder = ChasePyndiahDecoder::new(
            component.clone(),
            ChasePyndiahConfig {
                p: 4,
                ..ChasePyndiahConfig::default()
            },
        );

        let input: Vec<f32> = vec![2.0; 32];
        let w = decoder.chase_pyndiah_siso(&input, 0.5);

        let mut negative_ext_count = 0;
        for (j, &wi) in w.iter().enumerate() {
            let ext = wi - input[j];
            if ext < -0.1 {
                negative_ext_count += 1;
                eprintln!(
                    "WARN: bit {j}: input={:.2}, W={:.2}, ext={:.2}",
                    input[j], wi, ext
                );
            }
        }

        assert!(
            negative_ext_count <= 8,
            "Too many negative extrinsics ({negative_ext_count}/32) for correct codeword"
        );
    }
}
