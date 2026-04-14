//! Deterministic linear-congruential pseudo-random generator used across
//! workspace test and benchmark code as the single source of truth for
//! the "cheap deterministic LCG" pattern that would otherwise be
//! duplicated in every downstream crate.
//!
//! This is test-support infrastructure; it is not part of `gf2-core`'s
//! public mathematical API and carries no stability guarantees. The
//! module is gated behind `#[doc(hidden)]` so it does not appear in the
//! public rustdoc.
//!
//! The implementation uses Numerical Recipes' `MMIX` constants so
//! every downstream crate that seeds the same state observes the same
//! pseudo-random stream — parity tests between e.g. the CPU scalar,
//! AVX2, and HIP backends stay reproducible.

/// Deterministic LCG state.
///
/// See the module documentation for the rationale for living in
/// `gf2-core`.
#[doc(hidden)]
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Constructs a new LCG seeded with `seed`.
    ///
    /// # Arguments
    ///
    /// * `seed` — 64-bit seed. Any value is valid; distinct seeds
    ///   produce independent streams.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::test_rng::Lcg;
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
    /// use gf2_core::test_rng::Lcg;
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
    /// use gf2_core::test_rng::Lcg;
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
    /// use gf2_core::test_rng::Lcg;
    ///
    /// let mut rng = Lcg::new(7);
    /// assert!(rng.next_unit_f32().abs() <= 1.0);
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
    /// use gf2_core::test_rng::Lcg;
    ///
    /// let mut rng = Lcg::new(11);
    /// assert!(rng.next_unit_f64().abs() <= 1.0);
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
    /// * `lo` — Lower inclusive bound.
    /// * `hi` — Upper exclusive bound; caller must ensure `hi > lo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::test_rng::Lcg;
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
    /// * `lo` — Lower inclusive bound.
    /// * `hi` — Upper exclusive bound; caller must ensure `hi > lo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::test_rng::Lcg;
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

    /// Returns a pseudo-uniform `usize` in `[0, n)`.
    ///
    /// # Arguments
    ///
    /// * `n` — Exclusive upper bound; caller must ensure `n > 0`.
    ///
    /// # Panics
    ///
    /// Panics (via integer `%`) if `n == 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::test_rng::Lcg;
    ///
    /// let mut rng = Lcg::new(19);
    /// assert!(rng.next_bounded_usize(8) < 8);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn next_bounded_usize(&mut self, n: usize) -> usize {
        (self.next_u64() as usize) % n
    }
}
