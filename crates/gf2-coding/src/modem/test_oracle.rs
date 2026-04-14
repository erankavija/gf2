//! Shared brute-force log-MAP oracle for modem demapper cross-checks.
//!
//! Kept as a `#[doc(hidden)] pub` module (re-exported from
//! [`super::mod`](super)) so both internal unit tests and out-of-crate
//! integration tests can compute LLRs from a post-normalization
//! `(points, labels)` snapshot through a single implementation.
//!
//! This is test-support infrastructure; it is not part of the public
//! modem API and carries no stability guarantees.

use super::bit_pack::bit_at_msb_first;

/// Deterministic linear-congruential pseudo-random generator used by
/// modem demapper / mapper tests.
///
/// SSOT for the "cheap deterministic LCG" pattern repeated across the
/// modem test suites and HIP integration tests. Seeded with the same
/// Numerical-Recipes constants so test vectors are reproducible across
/// builds and platforms.
///
/// This is test-support infrastructure; carries no stability guarantees.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::test_oracle::Lcg;
///
/// let mut rng = Lcg::new(0xDEAD_BEEF);
/// let a = rng.next_u32();
/// let b = rng.next_u32();
/// assert_ne!(a, b);
/// ```
#[doc(hidden)]
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Constructs a new LCG seeded with `seed`.
    ///
    /// # Arguments
    ///
    /// * `seed` - 64-bit seed. Any value is valid; distinct seeds produce
    ///   independent streams.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let _ = Lcg::new(0);
    /// let _ = Lcg::new(u64::MAX);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Advances the state one step and returns the raw 64-bit output.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(1);
    /// let _ = rng.next_u64();
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    /// Advances the state and returns the top 32 bits of the new state.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(42);
    /// let _ = rng.next_u32();
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Returns a pseudo-uniform `f32` in `(-1, 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(7);
    /// let v = rng.next_unit_f32();
    /// assert!(v.abs() <= 1.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_unit_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Returns a pseudo-uniform `f64` in `(-1, 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(11);
    /// let v = rng.next_unit_f64();
    /// assert!(v.abs() <= 1.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_unit_f64(&mut self) -> f64 {
        (self.next_u32() as f64 / u32::MAX as f64) * 2.0 - 1.0
    }

    /// Returns a pseudo-uniform `f32` in `[lo, hi)`.
    ///
    /// # Arguments
    ///
    /// * `lo` - Lower inclusive bound.
    /// * `hi` - Upper exclusive bound; caller must ensure `hi > lo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(13);
    /// let v = rng.next_positive_f32(0.05, 2.0);
    /// assert!(v >= 0.05 && v <= 2.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_positive_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next_u32() as f32 / u32::MAX as f32) * (hi - lo)
    }

    /// Returns a pseudo-uniform `f64` in `[lo, hi)`.
    ///
    /// # Arguments
    ///
    /// * `lo` - Lower inclusive bound.
    /// * `hi` - Upper exclusive bound; caller must ensure `hi > lo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::test_oracle::Lcg;
    ///
    /// let mut rng = Lcg::new(17);
    /// let v = rng.next_positive_f64(0.05, 2.0);
    /// assert!(v >= 0.05 && v <= 2.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_positive_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next_u32() as f64 / u32::MAX as f64) * (hi - lo)
    }
}

/// Brute-force exact log-MAP LLR for a single received sample, bit
/// position, and total complex noise variance `N0 = 2 sigma^2`.
///
/// Computes
/// `log(sum_{j ∈ S0} exp(-d_j/N0)) - log(sum_{j ∈ S1} exp(-d_j/N0))`
/// with a numerical-stability min-shift. Operates directly on flat
/// `Vec<(f64, f64)>` / `Vec<u16>` snapshots of a post-normalization
/// `ModemSpec` so callers do not depend on `ModemSpec<S>` generics for
/// their oracle math.
///
/// Positive return value means `bit == 0` is more likely.
///
/// # Arguments
///
/// * `points` - Constellation points as `(I, Q)` pairs, post-normalization.
/// * `labels` - Per-point MSB-first labels (length = `points.len()`).
/// * `bits_per_symbol` - Label width in bits.
/// * `y_i`, `y_q` - Received sample.
/// * `h_i`, `h_q` - Complex channel gain (`(1.0, 0.0)` for pure AWGN).
/// * `n0` - Total complex noise variance (`2 sigma^2` convention).
/// * `b` - Bit position (MSB-first, `b = 0` is the MSB).
///
/// # Complexity
///
/// O(`points.len()`).
#[allow(clippy::too_many_arguments)]
pub fn brute_force_log_map_llr(
    points: &[(f64, f64)],
    labels: &[u16],
    bits_per_symbol: u8,
    y_i: f64,
    y_q: f64,
    h_i: f64,
    h_q: f64,
    n0: f64,
    b: u8,
) -> f64 {
    let dists: Vec<f64> = points
        .iter()
        .map(|&(pi, pq)| {
            let ei = y_i - (h_i * pi - h_q * pq);
            let eq = y_q - (h_i * pq + h_q * pi);
            (ei * ei + eq * eq) / n0
        })
        .collect();
    let d_min = dists.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut sum0 = 0.0_f64;
    let mut sum1 = 0.0_f64;
    for (j, &d) in dists.iter().enumerate() {
        let bit = bit_at_msb_first(labels[j], b, bits_per_symbol);
        let e = (d_min - d).exp();
        if bit == 0 {
            sum0 += e;
        } else {
            sum1 += e;
        }
    }
    sum0.ln() - sum1.ln()
}
