//! Block Rician fading channel and seeded random bit interleaver for coded QPSK.

use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::ops::{Add, Mul};

/// Complex number for the fading channel math (`y = h·x + n`); symbol
/// mapping and demapping live in [`crate::modem`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Complex {
    /// Creates a new complex number.
    pub fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }

    /// Returns the complex conjugate.
    pub fn conj(self) -> Self {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }

    /// Returns the squared absolute value |z|^2.
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Returns the absolute value |z|.
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Scales the complex number by a real scalar.
    pub fn scale(self, s: f64) -> Self {
        Complex {
            re: self.re * s,
            im: self.im * s,
        }
    }
}

impl Mul for Complex {
    type Output = Complex;

    fn mul(self, other: Complex) -> Complex {
        Complex {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

impl Add for Complex {
    type Output = Complex;

    fn add(self, other: Complex) -> Complex {
        Complex {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

/// Block Rician fading configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RicianConfig {
    /// Rician K-factor (ratio of LOS to scatter power).
    pub k_factor: f64,
    /// Coherence block size in QPSK symbols.
    pub coherence_block: usize,
    /// Number of coherence blocks per frame (taps).
    pub taps: usize,
}

impl RicianConfig {
    /// `@/citation/Yuan2025` channel K = 5, N_c = 128, t = 4: a 1024-bit frame.
    pub fn fig8() -> Self {
        RicianConfig {
            k_factor: 5.0,
            coherence_block: 128,
            taps: 4,
        }
    }

    /// `@/citation/Yuan2025` channel K = 8, N_c = 256, t = 2: a 1024-bit frame.
    pub fn fig9() -> Self {
        RicianConfig {
            k_factor: 8.0,
            coherence_block: 256,
            taps: 2,
        }
    }

    /// `@/citation/Yuan2025` channel K = 6, N_c = 256, t = 8: a 4096-bit frame.
    pub fn fig10() -> Self {
        RicianConfig {
            k_factor: 6.0,
            coherence_block: 256,
            taps: 8,
        }
    }

    /// Returns the total number of bits per frame: `N = 2·t·N_c`.
    pub fn frame_bits(&self) -> usize {
        2 * self.taps * self.coherence_block
    }

    /// Returns the number of QPSK symbols per frame.
    pub fn frame_symbols(&self) -> usize {
        self.taps * self.coherence_block
    }
}

/// Block Rician fading channel: the gain is constant within a coherence
/// block and i.i.d. across blocks.
///
/// ```text
/// H_Ri = sqrt(K/(K+1)) + sqrt(1/(K+1)) · (X + jY)
/// ```
/// where `X, Y ~ N(0, 0.5)` i.i.d., so `E[|H_Ri|²] = 1`.
pub struct RicianChannel {
    config: RicianConfig,
    /// sqrt(K / (K+1))
    los_amplitude: f64,
    /// sqrt(1 / (K+1))
    scatter_scale: f64,
    scatter_dist: Normal<f64>,
}

impl RicianChannel {
    /// Creates a Rician fading channel.
    ///
    /// # Panics
    ///
    /// Panics unless `config.k_factor >= 0.0`, `config.coherence_block > 0`
    /// and `config.taps > 0`.
    pub fn new(config: RicianConfig) -> Self {
        assert!(
            config.k_factor >= 0.0,
            "Rician K-factor must be non-negative, got {}",
            config.k_factor
        );
        assert!(
            config.coherence_block > 0,
            "Coherence block size N_c must be positive, got {}",
            config.coherence_block
        );
        assert!(
            config.taps > 0,
            "Number of taps t must be positive, got {}",
            config.taps
        );

        let k = config.k_factor;
        let los_amplitude = (k / (k + 1.0)).sqrt();
        let scatter_scale = (1.0 / (k + 1.0)).sqrt();
        let sigma = (0.5_f64).sqrt();
        let scatter_dist = Normal::new(0.0, sigma).expect("Failed to create normal distribution");
        RicianChannel {
            config,
            los_amplitude,
            scatter_scale,
            scatter_dist,
        }
    }

    /// Returns the channel configuration.
    pub fn config(&self) -> &RicianConfig {
        &self.config
    }

    /// Samples one coefficient `H_Ri`.
    pub fn sample_coefficient<R: Rng>(&self, rng: &mut R) -> Complex {
        let x = self.scatter_dist.sample(rng);
        let y = self.scatter_dist.sample(rng);
        Complex::new(
            self.los_amplitude + self.scatter_scale * x,
            self.scatter_scale * y,
        )
    }

    /// Samples the per-symbol gains of one frame: `frame_symbols()` entries,
    /// each run of `coherence_block` consecutive entries sharing one
    /// coefficient.
    pub fn generate_frame_gains<R: Rng>(&self, rng: &mut R) -> Vec<Complex> {
        let mut gains = Vec::with_capacity(self.config.frame_symbols());
        for _ in 0..self.config.taps {
            let h = self.sample_coefficient(rng);
            for _ in 0..self.config.coherence_block {
                gains.push(h);
            }
        }
        gains
    }

    /// Applies `y_k = h_k · x_k + n_k` with `n_k ~ CN(0, σ²)`,
    /// `σ² = sigma_squared` (real and imaginary parts each `N(0, σ²/2)`).
    ///
    /// # Panics
    ///
    /// Panics if `symbols` and `channel_gains` have different lengths,
    /// or if `sigma_squared` is not positive and finite.
    pub fn transmit<R: Rng>(
        &self,
        symbols: &[Complex],
        channel_gains: &[Complex],
        sigma_squared: f64,
        rng: &mut R,
    ) -> Vec<Complex> {
        assert_eq!(
            symbols.len(),
            channel_gains.len(),
            "symbols and channel_gains must have equal length"
        );
        assert!(
            sigma_squared > 0.0 && sigma_squared.is_finite(),
            "sigma_squared must be positive and finite, got {sigma_squared}"
        );
        let noise_std = (sigma_squared / 2.0).sqrt();
        let noise_dist = Normal::new(0.0, noise_std).expect("Failed to create noise distribution");

        symbols
            .iter()
            .zip(channel_gains.iter())
            .map(|(&x, &h)| {
                let hx = h * x;
                let n_re = noise_dist.sample(rng);
                let n_im = noise_dist.sample(rng);
                Complex::new(hx.re + n_re, hx.im + n_im)
            })
            .collect()
    }
}

/// Random bit interleaver and de-interleaver; the permutation is a
/// function of the block length and seed.
pub struct BitInterleaver {
    /// Forward permutation: `perm[i]` is where bit `i` goes.
    perm: Vec<usize>,
    /// Inverse permutation: `inv_perm[j]` is where bit `j` came from.
    inv_perm: Vec<usize>,
}

impl BitInterleaver {
    /// Creates the interleaver for `length` bits from `seed`.
    ///
    /// # Panics
    ///
    /// Panics if `length == 0`.
    pub fn new(length: usize, seed: u64) -> Self {
        assert!(length > 0, "Interleaver length must be positive");
        let perm = generate_permutation(length, seed);
        let mut inv_perm = vec![0usize; length];
        for (i, &p) in perm.iter().enumerate() {
            inv_perm[p] = i;
        }
        BitInterleaver { perm, inv_perm }
    }

    /// Block length in bits.
    pub fn len(&self) -> usize {
        self.perm.len()
    }

    /// Always `false`: [`BitInterleaver::new`] rejects length zero.
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// Permutes `bits`: output position `perm[i]` holds input bit `i`.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.len()`.
    pub fn interleave(&self, bits: &[bool]) -> Vec<bool> {
        assert_eq!(
            bits.len(),
            self.perm.len(),
            "Input length {} does not match interleaver length {}",
            bits.len(),
            self.perm.len()
        );
        let mut out = vec![false; bits.len()];
        for (i, &bit) in bits.iter().enumerate() {
            out[self.perm[i]] = bit;
        }
        out
    }

    /// Inverse of [`BitInterleaver::interleave`].
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.len()`.
    pub fn deinterleave(&self, bits: &[bool]) -> Vec<bool> {
        assert_eq!(
            bits.len(),
            self.inv_perm.len(),
            "Input length {} does not match interleaver length {}",
            bits.len(),
            self.inv_perm.len()
        );
        let mut out = vec![false; bits.len()];
        for (j, &bit) in bits.iter().enumerate() {
            out[self.inv_perm[j]] = bit;
        }
        out
    }

    /// Applies the permutation of [`BitInterleaver::interleave`] to LLRs.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.len()`.
    pub fn interleave_llrs(&self, llrs: &[crate::llr::Llr]) -> Vec<crate::llr::Llr> {
        assert_eq!(
            llrs.len(),
            self.perm.len(),
            "Input length {} does not match interleaver length {}",
            llrs.len(),
            self.perm.len()
        );
        let mut out = vec![crate::llr::Llr::new(0.0); llrs.len()];
        for (i, &llr) in llrs.iter().enumerate() {
            out[self.perm[i]] = llr;
        }
        out
    }

    /// Inverse of [`BitInterleaver::interleave_llrs`].
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.len()`.
    pub fn deinterleave_llrs(&self, llrs: &[crate::llr::Llr]) -> Vec<crate::llr::Llr> {
        assert_eq!(
            llrs.len(),
            self.inv_perm.len(),
            "Input length {} does not match interleaver length {}",
            llrs.len(),
            self.inv_perm.len()
        );
        let mut out = vec![crate::llr::Llr::new(0.0); llrs.len()];
        for (j, &llr) in llrs.iter().enumerate() {
            out[self.inv_perm[j]] = llr;
        }
        out
    }
}

/// Fisher-Yates shuffle of `[0, length)`, deterministic in `seed`.
fn generate_permutation(length: usize, seed: u64) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..length).collect();
    let mut rng = gf2_core::rng::Lcg::new(seed.wrapping_add(1));
    for i in (1..length).rev() {
        let j = (rng.next_u64() >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }
    perm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rician_config_fig8_params() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.k_factor, 5.0);
        assert_eq!(cfg.coherence_block, 128);
        assert_eq!(cfg.taps, 4);
    }

    #[test]
    fn test_rician_config_fig8_frame_bits() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.frame_bits(), 1024);
    }

    #[test]
    fn test_rician_config_fig8_frame_symbols() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.frame_symbols(), 512);
    }

    #[test]
    fn test_rician_config_fig9_params() {
        let cfg = RicianConfig::fig9();
        assert_eq!(cfg.k_factor, 8.0);
        assert_eq!(cfg.coherence_block, 256);
        assert_eq!(cfg.taps, 2);
    }

    #[test]
    fn test_rician_config_fig9_frame_bits() {
        let cfg = RicianConfig::fig9();
        assert_eq!(cfg.frame_bits(), 1024);
    }

    #[test]
    fn test_rician_config_fig10_params() {
        let cfg = RicianConfig::fig10();
        assert_eq!(cfg.k_factor, 6.0);
        assert_eq!(cfg.coherence_block, 256);
        assert_eq!(cfg.taps, 8);
    }

    #[test]
    fn test_rician_config_fig10_frame_bits() {
        let cfg = RicianConfig::fig10();
        assert_eq!(cfg.frame_bits(), 4096);
    }

    #[test]
    #[should_panic(expected = "K-factor must be non-negative")]
    fn test_rician_channel_negative_k_panics() {
        let cfg = RicianConfig {
            k_factor: -1.0,
            coherence_block: 128,
            taps: 4,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    #[should_panic(expected = "N_c must be positive")]
    fn test_rician_channel_zero_coherence_block_panics() {
        let cfg = RicianConfig {
            k_factor: 5.0,
            coherence_block: 0,
            taps: 4,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    #[should_panic(expected = "t must be positive")]
    fn test_rician_channel_zero_taps_panics() {
        let cfg = RicianConfig {
            k_factor: 5.0,
            coherence_block: 128,
            taps: 0,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    fn test_rician_channel_k_factor_zero_is_rayleigh() {
        let cfg = RicianConfig {
            k_factor: 0.0,
            coherence_block: 64,
            taps: 1,
        };
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let h = channel.sample_coefficient(&mut rng);
        assert!(h.re.is_finite() && h.im.is_finite());
    }

    #[test]
    fn test_rician_channel_coefficient_is_finite() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let h = channel.sample_coefficient(&mut rng);
            assert!(h.re.is_finite());
            assert!(h.im.is_finite());
        }
    }

    #[test]
    fn test_rician_channel_mean_power_near_one() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_power: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).norm_sq())
            .sum::<f64>()
            / n as f64;
        assert!(
            (mean_power - 1.0).abs() < 0.05,
            "Expected E[|H|^2] ≈ 1, got {mean_power:.4}"
        );
    }

    #[test]
    fn test_rician_channel_mean_real_part() {
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let expected_mean_re = (cfg.k_factor / (cfg.k_factor + 1.0)).sqrt();
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_re: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).re)
            .sum::<f64>()
            / n as f64;
        assert!(
            (mean_re - expected_mean_re).abs() < 0.05,
            "Expected E[Re(H)] ≈ {expected_mean_re:.4}, got {mean_re:.4}"
        );
    }

    #[test]
    fn test_rician_channel_mean_imag_near_zero() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_im: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).im)
            .sum::<f64>()
            / n as f64;
        assert!(
            mean_im.abs() < 0.05,
            "Expected E[Im(H)] ≈ 0, got {mean_im:.4}"
        );
    }

    #[test]
    fn test_rician_channel_generate_frame_gains_length() {
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let gains = channel.generate_frame_gains(&mut rng);
        assert_eq!(gains.len(), cfg.frame_symbols());
    }

    #[test]
    fn test_rician_channel_block_fading_constant_within_block() {
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let gains = channel.generate_frame_gains(&mut rng);
        for tap in 0..cfg.taps {
            let start = tap * cfg.coherence_block;
            let block = &gains[start..start + cfg.coherence_block];
            let first = block[0];
            for g in block {
                assert_eq!(g.re, first.re);
                assert_eq!(g.im, first.im);
            }
        }
    }

    #[test]
    fn test_rician_channel_transmit_length() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let symbols: Vec<Complex> = (0..8).map(|_| Complex::new(1.0, 0.0)).collect();
        let gains = vec![Complex::new(1.0, 0.0); 8];
        let received = channel.transmit(&symbols, &gains, 0.1, &mut rng);
        assert_eq!(received.len(), 8);
    }

    #[test]
    fn test_rician_channel_transmit_is_finite() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let symbols: Vec<Complex> = (0..16).map(|_| Complex::new(1.0, -1.0)).collect();
        let gains = vec![Complex::new(0.8, 0.3); 16];
        let received = channel.transmit(&symbols, &gains, 0.5, &mut rng);
        for r in &received {
            assert!(r.re.is_finite() && r.im.is_finite());
        }
    }

    #[test]
    fn test_rician_channel_all_configs() {
        let mut rng = rand::thread_rng();
        for cfg in [
            RicianConfig::fig8(),
            RicianConfig::fig9(),
            RicianConfig::fig10(),
        ] {
            let channel = RicianChannel::new(cfg);
            let gains = channel.generate_frame_gains(&mut rng);
            assert_eq!(gains.len(), cfg.frame_symbols());
        }
    }

    #[test]
    fn test_interleaver_len() {
        let il = BitInterleaver::new(128, 42);
        assert_eq!(il.len(), 128);
    }

    #[test]
    fn test_interleaver_is_permutation() {
        let n = 64;
        let il = BitInterleaver::new(n, 99);
        let mut seen = vec![false; n];
        for &p in &il.perm {
            assert!(!seen[p], "Duplicate index {p} in permutation");
            seen[p] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_interleaver_inverse_is_valid() {
        let n = 64;
        let il = BitInterleaver::new(n, 7);
        for i in 0..n {
            assert_eq!(il.inv_perm[il.perm[i]], i);
        }
    }

    #[test]
    fn test_interleave_deinterleave_roundtrip() {
        let n = 64;
        let il = BitInterleaver::new(n, 12345);
        let bits: Vec<bool> = (0..n).map(|i| i % 3 == 0).collect();
        let interleaved = il.interleave(&bits);
        let recovered = il.deinterleave(&interleaved);
        assert_eq!(recovered, bits);
    }

    #[test]
    fn test_interleave_changes_order() {
        let n = 64;
        let il = BitInterleaver::new(n, 42);
        let bits: Vec<bool> = (0..n).map(|i| i % 2 == 0).collect();
        let interleaved = il.interleave(&bits);
        assert_ne!(interleaved, bits, "Interleaving should change the order");
    }

    #[test]
    fn test_interleave_llrs_roundtrip() {
        let n = 64;
        let il = BitInterleaver::new(n, 55);
        let llrs: Vec<crate::llr::Llr> = (0..n)
            .map(|i| crate::llr::Llr::new(i as f32 - 32.0))
            .collect();
        let interleaved = il.interleave_llrs(&llrs);
        let recovered = il.deinterleave_llrs(&interleaved);
        for (a, b) in llrs.iter().zip(recovered.iter()) {
            assert!((a.value() - b.value()).abs() < 1e-6);
        }
    }

    #[test]
    fn test_interleaver_deterministic_same_seed() {
        let a = BitInterleaver::new(32, 999);
        let b = BitInterleaver::new(32, 999);
        assert_eq!(a.perm, b.perm);
    }

    #[test]
    fn test_interleaver_different_seeds_differ() {
        let a = BitInterleaver::new(32, 1);
        let b = BitInterleaver::new(32, 2);
        assert_ne!(a.perm, b.perm);
    }

    #[test]
    #[should_panic(expected = "Input length")]
    fn test_interleave_wrong_length_panics() {
        let il = BitInterleaver::new(16, 0);
        let bits = vec![false; 8];
        il.interleave(&bits);
    }

    #[test]
    #[should_panic(expected = "must be positive")]
    fn test_interleaver_zero_length_panics() {
        let _ = BitInterleaver::new(0, 0);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn interleave_deinterleave_roundtrip_prop(
            n in 1usize..128usize,
            seed: u64,
            bits in prop::collection::vec(any::<bool>(), 0..128)
        ) {
            let bits: Vec<bool> = bits.into_iter().take(n).chain(std::iter::repeat(false)).take(n).collect();
            let il = BitInterleaver::new(n, seed);
            let interleaved = il.interleave(&bits);
            let recovered = il.deinterleave(&interleaved);
            prop_assert_eq!(recovered, bits);
        }

        #[test]
        fn rician_channel_power_near_one(k_factor in 0.1f64..20.0f64) {
            let cfg = RicianConfig { k_factor, coherence_block: 1, taps: 1 };
            let channel = RicianChannel::new(cfg);
            let mut rng = rand::thread_rng();
            let n = 10_000;
            let mean_power: f64 = (0..n)
                .map(|_| channel.sample_coefficient(&mut rng).norm_sq())
                .sum::<f64>() / n as f64;
            prop_assert!((mean_power - 1.0).abs() < 0.1,
                "E[|H|^2] = {mean_power:.4}, expected ~1.0");
        }
    }
}

use crate::simulation::ChannelModel;

/// QPSK over a block Rician fading channel as a [`ChannelModel`]: bits →
/// [`BitInterleaver`] → QPSK map → fading + AWGN → exact log-MAP demap with
/// the per-symbol complex gain → de-interleave.
///
/// `N0` is [`crate::modem::awgn_link::unit_energy_n0_from_eb_n0_db`] at two
/// bits per symbol.
///
/// # Panics
///
/// [`ChannelModel::transmit_and_demodulate`] panics if the codeword length is
/// zero, odd or above `config.frame_bits()`, if `code_rate` is outside
/// `(0, 1]`, or if [`RicianChannel::new`] rejects the configuration.
pub struct QpskRicianChannelModel {
    config: RicianConfig,
}

impl QpskRicianChannelModel {
    /// Creates the channel model without validating `config`.
    pub fn new(config: RicianConfig) -> Self {
        Self { config }
    }
}

impl ChannelModel for QpskRicianChannelModel {
    fn batch_alignment(&self) -> usize {
        // QPSK: two bits per symbol.
        2
    }

    fn demap_method(&self) -> crate::modem::DemapMethod {
        // Matches the `DemapInput::method` of `transmit_and_demodulate`.
        crate::modem::DemapMethod::ExactLogMap
    }

    fn transmit_and_demodulate<R: rand::Rng>(
        &self,
        codeword: &gf2_core::BitVec,
        eb_n0_db: f64,
        code_rate: f64,
        rng: &mut R,
    ) -> Vec<crate::llr::Llr> {
        use crate::modem::{
            BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, GrayQamMapper, ModemSpec,
            ReferenceSoftDemapper,
        };
        use rand_distr::{Distribution, Normal};

        let n = codeword.len();
        assert!(
            n.is_multiple_of(2),
            "QPSK requires even codeword length, got {n}"
        );
        assert!(
            n <= self.config.frame_bits(),
            "codeword length {n} exceeds frame capacity {} for this Rician config",
            self.config.frame_bits()
        );

        // `sigma_squared` is `N0`: each noise axis has variance `N0 / 2`
        // and the demapper takes `noise_var = N0`.
        use crate::modem::awgn_link::unit_energy_n0_from_eb_n0_db;
        const M_BITS_PER_SYMBOL: usize = 2; // QPSK
        let sigma_squared = unit_energy_n0_from_eb_n0_db(M_BITS_PER_SYMBOL, code_rate, eb_n0_db);
        let noise_dist = Normal::new(0.0, (sigma_squared / 2.0).sqrt())
            .expect("Failed to create noise distribution");

        let bit_vec: Vec<bool> = (0..n).map(|i| codeword.get(i)).collect();

        let interleaver = BitInterleaver::new(n, 0xFADE);
        let interleaved = interleaver.interleave(&bit_vec);

        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let num_symbols = n / 2;
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        mapper.map_bits(&interleaved, &mut tx_i, &mut tx_q);

        let channel = RicianChannel::new(self.config);
        let mut gains = channel.generate_frame_gains(rng);
        gains.truncate(num_symbols);

        // The demapper takes the raw complex gain: no `conj(h)` pre-rotation.
        let mut rx_i = vec![0.0_f32; num_symbols];
        let mut rx_q = vec![0.0_f32; num_symbols];
        let mut gain_i = vec![0.0_f32; num_symbols];
        let mut gain_q = vec![0.0_f32; num_symbols];
        for k in 0..num_symbols {
            let h = gains[k];
            let xi = tx_i[k] as f64;
            let xq = tx_q[k] as f64;
            let noise_re: f64 = noise_dist.sample(rng);
            let noise_im: f64 = noise_dist.sample(rng);
            rx_i[k] = (h.re * xi - h.im * xq + noise_re) as f32;
            rx_q[k] = (h.re * xq + h.im * xi + noise_im) as f32;
            gain_i[k] = h.re as f32;
            gain_q[k] = h.im as f32;
        }

        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let noise_var = vec![sigma_squared as f32; num_symbols];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: Some(&gain_i),
            gain_q: Some(&gain_q),
            noise_var: &noise_var,
            method: DemapMethod::ExactLogMap,
        };
        let mut llrs = vec![crate::llr::Llr::new(0.0); n];
        demapper.demap_llrs(input, &mut llrs);

        interleaver.deinterleave_llrs(&llrs)
    }
}

#[cfg(test)]
mod channel_model_tests {
    use super::*;
    use crate::grand::{OrbGrand, OrbGrandConfig};
    use crate::simulation::{SimulationConfig, SimulationRunner};
    use crate::test_support::generic_ebch_16_11;
    use crate::traits::block::ParityCheckMatrixAccess;

    #[test]
    fn test_qpsk_rician_channel_model_preconditions() {
        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        assert_eq!(channel.config.frame_bits(), 1024);
    }

    #[test]
    #[should_panic(expected = "even codeword length")]
    fn test_qpsk_rician_rejects_odd_length() {
        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        let bits = gf2_core::BitVec::zeros(7);
        let mut rng = rand::thread_rng();
        channel.transmit_and_demodulate(&bits, 6.0, 0.5, &mut rng);
    }

    #[test]
    fn test_qpsk_rician_through_simulation_runner() {
        let ebch = generic_ebch_16_11();
        let h = ebch
            .parity_check_matrix()
            .expect("extended BCH parity matrix");
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let channel = QpskRicianChannelModel::new(RicianConfig::fig9());

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.max_frames = 20;
        config.min_errors = 1;

        let results = SimulationRunner::run_coded(&ebch, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        assert!(results.points[0].ber < 0.5, "BER too high at 10 dB");
    }
}

#[cfg(test)]
mod modem_framework_calibration_tests {
    use super::*;
    use crate::test_support::generic_ebch_16_11;
    use crate::traits::block::ParityCheckMatrixAccess;

    #[test]
    fn test_qpsk_rician_shared_n0_calibration() {
        use crate::modem::awgn_link::{
            unit_energy_n0_from_eb_n0_db, unit_energy_sigma_sq_from_eb_n0_db,
        };
        use crate::simulation::ChannelModel;
        use gf2_core::BitVec;
        use rand::{rngs::StdRng, SeedableRng};

        // `N0 = 1 / (m * rate * 10^(Eb/N0_dB/10))`.
        for (m, rate, eb_n0_db, expected_n0) in [
            (2usize, 1.0_f64, 0.0_f64, 0.5_f64),
            (2, 1.0, 10.0, 0.05),
            (2, 0.5, 0.0, 1.0),
            (2, 0.5, 10.0, 0.1),
            (4, 1.0, 0.0, 0.25),
            (4, 1.0, 10.0, 0.025),
        ] {
            let n0 = unit_energy_n0_from_eb_n0_db(m, rate, eb_n0_db);
            let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db);
            assert!(
                (n0 - expected_n0).abs() < 1e-12,
                "n0(m={m}, rate={rate}, eb_n0_db={eb_n0_db}) = {n0}, expected {expected_n0}"
            );
            assert!(
                (n0 - 2.0 * sigma_sq).abs() < 1e-12,
                "N0 must equal 2·sigma^2 by construction"
            );
        }

        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        let n_bits = 64;
        let mut codeword = BitVec::zeros(n_bits);
        for i in 0..n_bits {
            if (i * 17 + 11) & 1 == 0 {
                codeword.set(i, true);
            }
        }
        let mut rng = StdRng::seed_from_u64(0xDEADBEEFCAFE0010);
        let llrs = channel.transmit_and_demodulate(&codeword, 10.0, 1.0, &mut rng);
        assert_eq!(llrs.len(), n_bits);
        for llr in &llrs {
            assert!(
                llr.value().is_finite(),
                "non-finite LLR through fading pipeline: {}",
                llr.value()
            );
        }
        let any_decisive = llrs.iter().any(|l| l.value().abs() > 1.0);
        assert!(
            any_decisive,
            "no decisive LLRs at 10 dB Eb/N0 — calibration likely broken"
        );
    }

    #[test]
    fn test_interleaver_still_composes() {
        use crate::grand::{OrbGrand, OrbGrandConfig};
        use crate::simulation::{SimulationConfig, SimulationRunner};

        let ebch = generic_ebch_16_11();
        let h = ebch
            .parity_check_matrix()
            .expect("extended BCH parity matrix");
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());
        let channel = QpskRicianChannelModel::new(RicianConfig::fig9());

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![12.0];
        config.max_frames = 30;
        config.min_errors = 1;

        let results = SimulationRunner::run_coded(&ebch, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        assert!(
            results.points[0].ber < 0.5,
            "interleaver+modem composition broken: BER {} too high",
            results.points[0].ber
        );
    }
}
