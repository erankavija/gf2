//! Primitive polynomial generation for GF(2^m).

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
    /// Exhaustive search on a dedicated rayon pool with the `parallel`
    /// feature, sequential without it. The results equal those of
    /// [`Self::Exhaustive`] for every `threads`.
    ParallelExhaustive {
        /// Worker count of the pool; 0 takes rayon's default.
        threads: usize,
    },
}

/// Generator for primitive polynomials of a given degree.
pub struct PrimitiveGenerator {
    degree: usize,
    strategy: GenerationStrategy,
    /// Prime factors of 2^degree - 1, shared by every candidate check.
    order_prime_factors: Vec<u64>,
}

impl PrimitiveGenerator {
    /// Largest supported degree: a polynomial of degree m is a `u64` with its
    /// leading coefficient at bit m.
    pub const MAX_DEGREE: usize = (u64::BITS - 1) as usize;

    /// Create a new generator for polynomials of degree m.
    ///
    /// # Panics
    ///
    /// Panics if m == 0 or m > [`Self::MAX_DEGREE`].
    pub fn new(degree: usize) -> Self {
        assert!(degree > 0, "Degree must be positive");
        assert!(
            degree <= Self::MAX_DEGREE,
            "Degree must be at most {}",
            Self::MAX_DEGREE
        );

        Self {
            degree,
            strategy: GenerationStrategy::Exhaustive,
            order_prime_factors: Gf2mField::order_prime_factors(degree),
        }
    }

    /// Set the generation strategy.
    pub fn with_strategy(mut self, strategy: GenerationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Find the first primitive polynomial of degree m.
    ///
    /// Returns `None` when the strategy's search space holds no primitive polynomial.
    ///
    /// # Panics
    ///
    /// Panics if the `ParallelExhaustive` pool cannot be built.
    pub fn find_first(&self) -> Option<u64> {
        match self.strategy {
            GenerationStrategy::Exhaustive | GenerationStrategy::ParallelExhaustive { .. } => {
                self.exhaustive_first(|candidate| self.is_primitive_poly(candidate))
            }
            GenerationStrategy::Trinomial => self.trinomial_search(),
            GenerationStrategy::Pentanomial => self.pentanomial_search(),
        }
    }

    /// Find all primitive polynomials of degree m, in ascending order.
    ///
    /// Visits each of the `2^m` monic degree-`m` candidates.
    ///
    /// # Panics
    ///
    /// Panics if strategy is not `Exhaustive` or `ParallelExhaustive`, or if
    /// the `ParallelExhaustive` pool cannot be built.
    pub fn find_all(&self) -> Vec<u64> {
        match self.strategy {
            GenerationStrategy::Exhaustive | GenerationStrategy::ParallelExhaustive { .. } => {
                self.exhaustive_all(|candidate| self.is_primitive_poly(candidate))
            }
            _ => panic!("find_all() only supported with exhaustive strategies"),
        }
    }

    /// Above degree 1, even weight means x + 1 divides the candidate.
    fn passes_parity_sieve(&self, candidate: u64) -> bool {
        self.degree == 1 || candidate.count_ones() % 2 == 1
    }

    /// First monic degree-m candidate, in ascending order, that `accept` takes.
    fn exhaustive_first(&self, accept: impl Fn(u64) -> bool + Sync) -> Option<u64> {
        let high_bit = 1u64 << self.degree;
        let keep = |candidate: &u64| self.passes_parity_sieve(*candidate) && accept(*candidate);

        #[cfg(feature = "parallel")]
        if let GenerationStrategy::ParallelExhaustive { threads } = self.strategy {
            use rayon::prelude::*;
            return crate::compute::field::run_in_dedicated_parallel_pool(threads, || {
                (0..high_bit)
                    .into_par_iter()
                    .map(|lower_bits| high_bit | lower_bits)
                    .find_first(keep)
            });
        }

        (0..high_bit)
            .map(|lower_bits| high_bit | lower_bits)
            .find(keep)
    }

    /// Every monic degree-m candidate that `accept` takes, in ascending order.
    fn exhaustive_all(&self, accept: impl Fn(u64) -> bool + Sync) -> Vec<u64> {
        let high_bit = 1u64 << self.degree;
        let keep = |candidate: &u64| self.passes_parity_sieve(*candidate) && accept(*candidate);

        #[cfg(feature = "parallel")]
        if let GenerationStrategy::ParallelExhaustive { threads } = self.strategy {
            use rayon::prelude::*;
            return crate::compute::field::run_in_dedicated_parallel_pool(threads, || {
                (0..high_bit)
                    .into_par_iter()
                    .map(|lower_bits| high_bit | lower_bits)
                    .filter(keep)
                    .collect()
            });
        }

        (0..high_bit)
            .map(|lower_bits| high_bit | lower_bits)
            .filter(keep)
            .collect()
    }

    fn trinomial_search(&self) -> Option<u64> {
        let m = self.degree;
        let high_bit = 1u64 << m;
        let constant = 1u64;

        for k in 1..m {
            // x^m + x^k + 1 is a polynomial in x^gcd(m, k), so not primitive when
            // gcd(m, k) > 1, and is reducible when 8 | m (`@/citation/Swan1962`).
            if gcd(m, k) > 1 {
                continue;
            }
            if m.is_multiple_of(8) && k % 2 == 1 {
                continue;
            }

            let candidate = high_bit | (1u64 << k) | constant;

            if self.is_primitive_poly(candidate) {
                return Some(candidate);
            }
        }

        None
    }

    fn pentanomial_search(&self) -> Option<u64> {
        let m = self.degree;
        let high_bit = 1u64 << m;
        let constant = 1u64;

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

    fn is_primitive_poly(&self, poly: u64) -> bool {
        Gf2mField::new(self.degree, poly).verify_primitive_with_factors(&self.order_prime_factors)
    }
}

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

    // Primitive-polynomial counts per degree: `@/citation/Oeis2026` (A011260).

    #[test]
    fn test_generate_m2_first() {
        let gen = PrimitiveGenerator::new(2);
        let poly = gen.find_first();
        assert_eq!(poly, Some(0b111));
    }

    #[test]
    fn test_generate_m3_first() {
        let gen = PrimitiveGenerator::new(3);
        let poly = gen.find_first().unwrap();
        assert!(poly == 0b1011 || poly == 0b1101);
    }

    #[test]
    fn test_generate_m4_first() {
        let gen = PrimitiveGenerator::new(4);
        let poly = gen.find_first().unwrap();
        let field = Gf2mField::new(4, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_generate_m5_first() {
        let gen = PrimitiveGenerator::new(5);
        let poly = gen.find_first().unwrap();
        let field = Gf2mField::new(5, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_generate_m2_all() {
        let gen = PrimitiveGenerator::new(2).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], 0b111);
    }

    #[test]
    fn test_generate_m3_all() {
        let gen = PrimitiveGenerator::new(3).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&0b1011));
        assert!(all.contains(&0b1101));
    }

    #[test]
    fn test_generate_m4_all() {
        let gen = PrimitiveGenerator::new(4).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&0b10011));
        assert!(all.contains(&0b11001));
    }

    #[test]
    fn test_generate_m5_all() {
        let gen = PrimitiveGenerator::new(5).with_strategy(GenerationStrategy::Exhaustive);
        let all = gen.find_all();
        assert_eq!(all.len(), 6);

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
        let gen = PrimitiveGenerator::new(3).with_strategy(GenerationStrategy::Trinomial);
        let poly = gen.find_first().unwrap();
        assert_eq!(poly.count_ones(), 3);
        let field = Gf2mField::new(3, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_trinomial_search_m5() {
        let gen = PrimitiveGenerator::new(5).with_strategy(GenerationStrategy::Trinomial);
        let poly = gen.find_first().unwrap();
        assert_eq!(poly.count_ones(), 3);
        let field = Gf2mField::new(5, poly);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_generate_m1() {
        let gen = PrimitiveGenerator::new(1);
        assert_eq!(gen.find_first(), Some(0b11));
        assert_eq!(gen.find_all(), vec![0b11]);
    }

    #[test]
    #[should_panic(expected = "Degree must be at most 63")]
    fn test_degree_64_is_rejected() {
        let _ = PrimitiveGenerator::new(64);
    }

    #[test]
    fn test_sparse_search_at_largest_degree() {
        let m = PrimitiveGenerator::MAX_DEGREE;
        for strategy in [
            GenerationStrategy::Trinomial,
            GenerationStrategy::Pentanomial,
        ] {
            let poly = PrimitiveGenerator::new(m)
                .with_strategy(strategy)
                .find_first()
                .unwrap();
            assert_eq!(poly >> m, 1, "{strategy:?}: {poly:#x} has degree {m}");
            assert!(Gf2mField::new(m, poly).verify_primitive());
        }
    }

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 5), 1);
        assert_eq!(gcd(21, 14), 7);
        assert_eq!(gcd(5, 0), 5);
    }

    #[test]
    fn test_parallel_exhaustive_matches_exhaustive_for_every_thread_count() {
        for m in [1usize, 2, 5, 9] {
            let sequential =
                PrimitiveGenerator::new(m).with_strategy(GenerationStrategy::Exhaustive);
            for threads in [0usize, 1, 2, 3, 7] {
                let parallel = PrimitiveGenerator::new(m)
                    .with_strategy(GenerationStrategy::ParallelExhaustive { threads });
                assert_eq!(
                    parallel.find_all(),
                    sequential.find_all(),
                    "m={m} threads={threads}"
                );
                assert_eq!(
                    parallel.find_first(),
                    sequential.find_first(),
                    "m={m} threads={threads}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn test_parallel_exhaustive_searches_on_a_pool_of_its_thread_count() {
        use std::collections::BTreeSet;
        use std::sync::Mutex;

        for threads in [1usize, 2, 3] {
            let gen = PrimitiveGenerator::new(8)
                .with_strategy(GenerationStrategy::ParallelExhaustive { threads });
            let widths = Mutex::new(BTreeSet::new());
            let reject_recording_width = |_candidate: u64| {
                widths.lock().unwrap().insert(rayon::current_num_threads());
                false
            };
            assert!(gen.exhaustive_all(reject_recording_width).is_empty());
            assert_eq!(gen.exhaustive_first(reject_recording_width), None);
            assert_eq!(widths.into_inner().unwrap(), BTreeSet::from([threads]));
        }
    }
}
