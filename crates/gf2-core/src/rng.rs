//! Seed-reproducible 64-bit linear-congruential generator with the MMIX
//! constants of `@/citation/Knuth1997`, for simulation, test and benchmark
//! data. It is unsuitable for cryptographic use.

/// LCG state.
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Starts the stream at state `seed`.
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Advances the state one step and returns the raw 64-bit output.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    /// Advances the state and returns the top 32 bits of the new state.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Returns a pseudo-uniform `f32` in the closed interval `[-1.0, 1.0]`.
    #[inline]
    pub fn next_unit_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Returns a pseudo-uniform `f64` in the closed interval `[-1.0, 1.0]`.
    #[inline]
    pub fn next_unit_f64(&mut self) -> f64 {
        (self.next_u32() as f64 / u32::MAX as f64) * 2.0 - 1.0
    }

    /// Returns a pseudo-uniform `f32` in the closed interval `[lo, hi]`.
    #[inline]
    pub fn next_positive_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next_u32() as f32 / u32::MAX as f32) * (hi - lo)
    }

    /// Returns a pseudo-uniform `f64` in the closed interval `[lo, hi]`.
    #[inline]
    pub fn next_positive_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next_u32() as f64 / u32::MAX as f64) * (hi - lo)
    }

    /// Returns a pseudo-uniform `usize` in `[0, n)`.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`.
    #[inline]
    pub fn next_bounded_usize(&mut self, n: usize) -> usize {
        (self.next_u64() as usize) % n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lcg_derived_methods_stay_in_range() {
        let mut rng = Lcg::new(42);

        let _v = rng.next_u32();

        let uf32 = rng.next_unit_f32();
        assert!((-1.0_f32..=1.0_f32).contains(&uf32));

        let uf64 = rng.next_unit_f64();
        assert!((-1.0_f64..=1.0_f64).contains(&uf64));

        let pf32 = rng.next_positive_f32(0.1, 0.9);
        assert!((0.1_f32..=0.9_f32).contains(&pf32));

        let pf64 = rng.next_positive_f64(0.1, 0.9);
        assert!((0.1_f64..=0.9_f64).contains(&pf64));

        let bu = rng.next_bounded_usize(7);
        assert!(bu < 7);
    }
}
