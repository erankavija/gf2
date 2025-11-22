//! Primitive polynomial generation for GF(2^m).
//!
//! This module provides algorithms to generate primitive polynomials over GF(2),
//! which are essential for constructing binary extension fields GF(2^m).
//!
//! # Generation Strategies
//!
//! Different strategies are optimal for different field sizes:
//! - **Exhaustive**: Test all monic polynomials (m ≤ 16)
//! - **Trinomial**: Search for x^m + x^k + 1 forms (hardware-efficient)
//! - **Pentanomial**: Fallback when trinomials don't exist
//! - **Parallel**: Multi-core exhaustive search using rayon
//!
//! # Examples
//!
//! ```
//! use gf2_core::gf2m::generation::{PrimitiveGenerator, GenerationStrategy};
//!
//! // Find first primitive polynomial for GF(2^8)
//! let gen = PrimitiveGenerator::new(8);
//! let poly = gen.find_first().expect("primitive polynomial exists");
//! println!("Found: {:#b}", poly);
//!
//! // Find all primitives for small fields
//! let gen = PrimitiveGenerator::new(5)
//!     .with_strategy(GenerationStrategy::Exhaustive);
//! let all = gen.find_all();
//! println!("Found {} primitive polynomials", all.len());
//! ```

use super::Gf2mField;

/// Strategy for generating primitive polynomials.
#[derive(Debug, Clone, Copy)]
pub enum GenerationStrategy {
    /// Exhaustive search through all monic polynomials
    Exhaustive,
    /// Search for trinomials x^m + x^k + 1
    Trinomial,
    /// Search for pentanomials x^m + x^a + x^b + x^c + 1
    Pentanomial,
    /// Parallel exhaustive search with rayon (not yet implemented)
    ParallelExhaustive {
        /// Number of threads to use for parallel search
        threads: usize,
    },
}

/// Generator for primitive polynomials of a given degree.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::generation::PrimitiveGenerator;
///
/// let gen = PrimitiveGenerator::new(4);
/// let poly = gen.find_first().unwrap();
/// assert_eq!(poly, 0b10011); // x^4 + x + 1
/// ```
pub struct PrimitiveGenerator {
    degree: usize,
    strategy: GenerationStrategy,
}

impl PrimitiveGenerator {
    /// Create a new generator for polynomials of degree m.
    ///
    /// # Panics
    ///
    /// Panics if m == 0 or m > 64.
    pub fn new(degree: usize) -> Self {
        assert!(degree > 0, "Degree must be positive");
        assert!(degree <= 64, "Degree > 64 not supported");

        Self {
            degree,
            strategy: GenerationStrategy::Exhaustive,
        }
    }

    /// Set the generation strategy.
    pub fn with_strategy(mut self, strategy: GenerationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Find the first primitive polynomial of degree m.
    ///
    /// Returns `None` if no primitive polynomial exists (which shouldn't happen for valid m).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::generation::PrimitiveGenerator;
    ///
    /// let gen = PrimitiveGenerator::new(2);
    /// assert_eq!(gen.find_first(), Some(0b111)); // x^2 + x + 1
    /// ```
    pub fn find_first(&self) -> Option<u64> {
        match self.strategy {
            GenerationStrategy::Exhaustive | GenerationStrategy::ParallelExhaustive { .. } => {
                self.exhaustive_search_first()
            }
            GenerationStrategy::Trinomial => self.trinomial_search(),
            GenerationStrategy::Pentanomial => self.pentanomial_search(),
        }
    }

    /// Find all primitive polynomials of degree m.
    ///
    /// This is only practical for small m (≤ 16) with exhaustive search.
    ///
    /// # Panics
    ///
    /// Panics if strategy is not `Exhaustive` or `ParallelExhaustive`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::generation::{PrimitiveGenerator, GenerationStrategy};
    ///
    /// let gen = PrimitiveGenerator::new(3)
    ///     .with_strategy(GenerationStrategy::Exhaustive);
    /// let all = gen.find_all();
    /// assert_eq!(all.len(), 2); // x^3+x+1 and x^3+x^2+1
    /// ```
    pub fn find_all(&self) -> Vec<u64> {
        match self.strategy {
            GenerationStrategy::Exhaustive | GenerationStrategy::ParallelExhaustive { .. } => {
                self.exhaustive_search_all()
            }
            _ => panic!("find_all() only supported with exhaustive strategies"),
        }
    }

    /// Exhaustive search for first primitive polynomial.
    fn exhaustive_search_first(&self) -> Option<u64> {
        #[cfg(feature = "parallel")]
        if matches!(self.strategy, GenerationStrategy::ParallelExhaustive { .. }) {
            return self.parallel_search_first();
        }

        let m = self.degree;
        let high_bit = 1u64 << m;

        // Lower bits can be anything from 0 to 2^m - 1
        for lower_bits in 0..(1u64 << m) {
            let candidate = high_bit | lower_bits;

            // Skip even-weight polynomials
            // Primitive polynomials must have odd weight
            let weight = candidate.count_ones();
            if weight % 2 == 0 {
                continue;
            }

            if self.is_primitive_poly(candidate) {
                return Some(candidate);
            }
        }

        None
    }

    /// Exhaustive search for all primitive polynomials.
    fn exhaustive_search_all(&self) -> Vec<u64> {
        #[cfg(feature = "parallel")]
        if matches!(self.strategy, GenerationStrategy::ParallelExhaustive { .. }) {
            return self.parallel_search_all();
        }

        let m = self.degree;
        let mut primitives = Vec::new();
        let high_bit = 1u64 << m;

        for lower_bits in 0..(1u64 << m) {
            let candidate = high_bit | lower_bits;

            // Skip even-weight polynomials
            let weight = candidate.count_ones();
            if weight % 2 == 0 {
                continue;
            }

            if self.is_primitive_poly(candidate) {
                primitives.push(candidate);
            }
        }

        primitives
    }

    /// Search for primitive trinomials x^m + x^k + 1.
    fn trinomial_search(&self) -> Option<u64> {
        let m = self.degree;
        let high_bit = 1u64 << m;
        let constant = 1u64;

        // Try all possible middle terms x^k where 1 ≤ k < m
        for k in 1..m {
            // Swan's theorem: trinomial x^m + x^k + 1 is reducible if:
            // - gcd(m, k) > 1, OR
            // - m ≡ 0 (mod 8) and k is odd
            if gcd(m, k) > 1 {
                continue;
            }
            if m % 8 == 0 && k % 2 == 1 {
                continue;
            }

            let candidate = high_bit | (1u64 << k) | constant;

            if self.is_primitive_poly(candidate) {
                return Some(candidate);
            }
        }

        None
    }

    /// Search for primitive pentanomials x^m + x^a + x^b + x^c + 1.
    fn pentanomial_search(&self) -> Option<u64> {
        let m = self.degree;
        let high_bit = 1u64 << m;
        let constant = 1u64;

        // Try all combinations where m > a > b > c > 0
        for a in (1..m).rev() {
            for b in (1..a).rev() {
                for c in 1..b {
                    let candidate = high_bit | (1u64 << a) | (1u64 << b) | (1u64 << c) | constant;

                    if self.is_primitive_poly(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }

        None
    }

    /// Test if a polynomial is primitive using our existing verification.
    fn is_primitive_poly(&self, poly: u64) -> bool {
        // Create a field with this polynomial without database warnings
        // (we're testing many polynomials, most won't be in database)
        let field = Gf2mField::new_unchecked(self.degree, poly);
        field.verify_primitive()
    }

    #[cfg(feature = "parallel")]
    fn parallel_search_first(&self) -> Option<u64> {
        use rayon::prelude::*;

        let m = self.degree;
        let high_bit = 1u64 << m;

        (0..(1u64 << m))
            .into_par_iter()
            .map(|lower_bits| high_bit | lower_bits)
            .filter(|candidate| candidate.count_ones() % 2 == 1)
            .find_first(|&candidate| self.is_primitive_poly(candidate))
    }

    #[cfg(feature = "parallel")]
    fn parallel_search_all(&self) -> Vec<u64> {
        use rayon::prelude::*;

        let m = self.degree;
        let high_bit = 1u64 << m;

        let mut primitives: Vec<u64> = (0..(1u64 << m))
            .into_par_iter()
            .map(|lower_bits| high_bit | lower_bits)
            .filter(|candidate| candidate.count_ones() % 2 == 1)
            .filter(|&candidate| self.is_primitive_poly(candidate))
            .collect();

        primitives.sort_unstable();
        primitives
    }
}

/// Compute GCD using Euclidean algorithm.
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known primitive polynomials from OEIS A011260 and authoritative sources
    // Used to validate our generation algorithms

    #[test]
    fn test_generate_m2_first() {
        // x^2 + x + 1 is the only primitive polynomial for m=2
        let gen = PrimitiveGenerator::new(2);
        let poly = gen.find_first();
        assert_eq!(poly, Some(0b111));
    }

    #[test]
    fn test_generate_m3_first() {
        // x^3 + x + 1 is primitive (0b1011)
        let gen = PrimitiveGenerator::new(3);
        let poly = gen.find_first().unwrap();
        // Could be x^3+x+1 (0b1011) or x^3+x^2+1 (0b1101)
        assert!(poly == 0b1011 || poly == 0b1101);
    }

    #[test]
    fn test_generate_m4_first() {
        // x^4 + x + 1 is primitive (0b10011)
        let gen = PrimitiveGenerator::new(4);
        let poly = gen.find_first().unwrap();
        // Verify it's actually primitive
        let field = Gf2mField::new(4, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_generate_m5_first() {
        // x^5 + x^2 + 1 is primitive (0b100101)
        let gen = PrimitiveGenerator::new(5);
        let poly = gen.find_first().unwrap();
        let field = Gf2mField::new(5, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_generate_m2_all() {
        // m=2 has exactly 1 primitive polynomial
        let gen = PrimitiveGenerator::new(2).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], 0b111); // x^2 + x + 1
    }

    #[test]
    fn test_generate_m3_all() {
        // m=3 has exactly 2 primitive polynomials
        let gen = PrimitiveGenerator::new(3).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&0b1011)); // x^3 + x + 1
        assert!(all.contains(&0b1101)); // x^3 + x^2 + 1
    }

    #[test]
    fn test_generate_m4_all() {
        // m=4 has exactly 2 primitive polynomials
        let gen = PrimitiveGenerator::new(4).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&0b10011)); // x^4 + x + 1
        assert!(all.contains(&0b11001)); // x^4 + x^3 + 1
    }

    #[test]
    fn test_generate_m5_all() {
        // m=5 has exactly 6 primitive polynomials (OEIS A011260)
        let gen = PrimitiveGenerator::new(5).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 6);

        // Verify all are actually primitive
        for &poly in &all {
            let field = Gf2mField::new(5, poly);
            assert!(
                field.verify_primitive(),
                "polynomial {:#b} should be primitive",
                poly
            );
        }
    }

    #[test]
    fn test_trinomial_search_m3() {
        // x^3 + x + 1 is a primitive trinomial
        let gen = PrimitiveGenerator::new(3).with_strategy(GenerationStrategy::Trinomial);
        let poly = gen.find_first().unwrap();
        assert_eq!(poly.count_ones(), 3); // trinomial has 3 terms
        let field = Gf2mField::new(3, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_trinomial_search_m5() {
        // x^5 + x^2 + 1 is a primitive trinomial
        let gen = PrimitiveGenerator::new(5).with_strategy(GenerationStrategy::Trinomial);
        let poly = gen.find_first().unwrap();
        assert_eq!(poly.count_ones(), 3);
        let field = Gf2mField::new(5, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 5), 1);
        assert_eq!(gcd(21, 14), 7);
        assert_eq!(gcd(5, 0), 5);
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn test_parallel_search_m5() {
        let gen = PrimitiveGenerator::new(5)
            .with_strategy(GenerationStrategy::ParallelExhaustive { threads: 2 });
        let all = gen.find_all();
        assert_eq!(all.len(), 6);

        // Should match sequential results
        let sequential_gen =
            PrimitiveGenerator::new(5).with_strategy(GenerationStrategy::Exhaustive);
        let sequential_all = sequential_gen.find_all();
        assert_eq!(all, sequential_all);
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn test_parallel_find_first_m6() {
        let gen = PrimitiveGenerator::new(6)
            .with_strategy(GenerationStrategy::ParallelExhaustive { threads: 4 });
        let poly = gen.find_first().unwrap();

        // Verify it's actually primitive
        let field = Gf2mField::new_unchecked(6, poly);
        assert!(field.verify_primitive());
    }
}
