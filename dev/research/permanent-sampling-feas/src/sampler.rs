//! Uniform $\mathbb{F}_q$ matrix sampling by exact rejection.
//!
//! # Why rejection
//!
//! This sampler consumes bytes, and `256 % 7 = 4`, so a bare `byte % 7` would
//! over-represent the residues `0..4` by a factor `37/36` — a 2.8 % bias, three
//! orders of magnitude larger than the $10^{-3}$–$10^{-4}$ effect the campaign
//! is trying to resolve. Bytes at or above [`accept_bound`] are therefore
//! discarded and redrawn.
//!
//! # In-tree alternative and why it is not used
//!
//! `gf2_algebra::testutil::random_matrix` draws `Lcg::next_u64() % P`. Its
//! *modulo* bias is negligible — reducing a full 64-bit word costs at most
//! $q \cdot 2^{-64}$ — but the generator is not. `gf2_core::rng::Lcg` is the
//! MMIX linear congruential generator modulo $2^{64}$, documented in its own
//! module as simulation-grade and explicitly not a substitute for
//! `rand`/`rand_chacha`. Consecutive LCG outputs lie on a coarse lattice, and a
//! matrix here is $n^2$ *consecutive* draws, so the entries of a single sampled
//! matrix carry deterministic linear structure. The statistic under study is
//! itself algebraic, so that structure cannot be assumed harmless. The LCG is
//! fit for cross-check fixtures and unfit for a published statistic; see the
//! feasibility study's gap G1.
//!
//! # RNG and stream derivation
//!
//! The generator is ChaCha20 as implemented by `rand_chacha` 0.9
//! (`ChaCha20Rng`), seeded from a 32-byte block assembled as four
//! little-endian `u64` words:
//!
//! ```text
//! seed[ 0.. 8] = root      (campaign root seed)
//! seed[ 8..16] = q         (field order)
//! seed[16..24] = n         (matrix dimension)
//! seed[24..32] = stream    (per-cell / per-shard stream index)
//! ```
//!
//! Distinct `(root, q, n, stream)` tuples therefore address disjoint,
//! independently addressable ChaCha20 streams, and any single draw is
//! reproducible from the tuple alone.
//!
//! # Matrix layout mapping
//!
//! Entries are drawn in row-major order: draw `k` becomes `A[k / n][k % n]`.
//! The resulting `Vec<Fp<Q>>` is handed to `from_row_major(&data, n, n)`, so
//! the packed column-major storage used by the kernels is produced by the
//! matrix constructor, not by the sampler.

use gf2_core::gfp::Fp;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

/// Largest byte value that may be accepted for field order `q`, exclusive.
///
/// Bytes in `0..accept_bound(q)` cover every residue class an equal number of
/// times, so reducing an accepted byte modulo `q` is exactly uniform. Bytes at
/// or above the bound are discarded and redrawn.
///
/// | `q` | bound | rejection probability |
/// |-----|-------|-----------------------|
/// | 3   | 255   | 1/256                 |
/// | 5   | 255   | 1/256                 |
/// | 7   | 252   | 4/256                 |
#[must_use]
pub const fn accept_bound(q: u64) -> u8 {
    (256 - 256 % q) as u8
}

/// Assemble the 32-byte ChaCha20 seed for one `(root, q, n, stream)` tuple.
#[must_use]
pub fn derive_seed(root: u64, q: u64, n: usize, stream: u64) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed[0..8].copy_from_slice(&root.to_le_bytes());
    seed[8..16].copy_from_slice(&q.to_le_bytes());
    seed[16..24].copy_from_slice(&(n as u64).to_le_bytes());
    seed[24..32].copy_from_slice(&stream.to_le_bytes());
    seed
}

/// A domain-separated uniform sampler over $\mathbb{F}_q^{n \times n}$.
pub struct MatrixSampler {
    rng: ChaCha20Rng,
    buf: Vec<u8>,
    /// Read cursor into `buf`; `buf[cursor..]` is unconsumed randomness.
    cursor: usize,
}

/// Bytes drawn from ChaCha20 per refill. One ChaCha20 block is 64 bytes; a
/// 4 KiB refill amortises the block function over 64 blocks and keeps the
/// buffer inside L1.
const REFILL_BYTES: usize = 4096;

impl MatrixSampler {
    /// Open the stream addressed by `(root, q, n, stream)`.
    #[must_use]
    pub fn new(root: u64, q: u64, n: usize, stream: u64) -> Self {
        Self {
            rng: ChaCha20Rng::from_seed(derive_seed(root, q, n, stream)),
            buf: vec![0u8; REFILL_BYTES],
            cursor: REFILL_BYTES,
        }
    }

    /// Next raw byte from the stream, before any rejection test.
    ///
    /// Exposed so callers that need uniform integers other than field elements
    /// — the randomised cell ordering in [`crate::protocol::shuffle`] — draw
    /// from the same audited generator instead of a second RNG.
    pub fn next_raw_byte(&mut self) -> u8 {
        if self.cursor == self.buf.len() {
            self.rng.fill_bytes(&mut self.buf);
            self.cursor = 0;
        }
        let b = self.buf[self.cursor];
        self.cursor += 1;
        b
    }

    /// Next entry, uniform on `0..Q`, by rejection.
    pub fn next_entry<const Q: u64>(&mut self) -> Fp<Q> {
        let bound = accept_bound(Q);
        loop {
            let b = self.next_raw_byte();
            if b < bound {
                return Fp::<Q>::new(u64::from(b) % Q);
            }
        }
    }

    /// Draw one `n × n` matrix in row-major order.
    pub fn next_matrix<const Q: u64>(&mut self, n: usize) -> Vec<Fp<Q>> {
        (0..n * n).map(|_| self.next_entry::<Q>()).collect()
    }

    /// Fill `out` with one `n × n` matrix in row-major order, reusing the
    /// caller's allocation. `out` is cleared first.
    pub fn fill_matrix<const Q: u64>(&mut self, n: usize, out: &mut Vec<Fp<Q>>) {
        out.clear();
        out.reserve(n * n);
        for _ in 0..n * n {
            let e = self.next_entry::<Q>();
            out.push(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_bounds_are_multiples_of_q() {
        for q in [3u64, 5, 7] {
            let bound = u64::from(accept_bound(q));
            assert_eq!(bound % q, 0, "accept bound for q={q} must be a multiple");
            assert!(bound > 256 - q, "accept bound for q={q} discards too much");
        }
    }

    #[test]
    fn streams_are_reproducible_and_domain_separated() {
        let a = MatrixSampler::new(7, 3, 4, 0).next_matrix::<3>(4);
        let b = MatrixSampler::new(7, 3, 4, 0).next_matrix::<3>(4);
        assert_eq!(a, b, "same tuple must reproduce the same draw");

        let other_stream = MatrixSampler::new(7, 3, 4, 1).next_matrix::<3>(4);
        assert_ne!(a, other_stream, "stream index must separate draws");

        let other_n = MatrixSampler::new(7, 3, 5, 0).next_matrix::<3>(4);
        assert_ne!(a, other_n, "n must separate draws");
    }

    /// The entry distribution must be flat to well inside the campaign's
    /// target resolution. With 3 x 10^5 draws the standard error on each
    /// class frequency is below 10^-3 for every supported `q`.
    #[test]
    fn entries_are_uniform_within_sampling_error() {
        fn check<const Q: u64>() {
            let draws = 300_000usize;
            let mut sampler = MatrixSampler::new(0xFEED, Q, 1, 0);
            let mut counts = vec![0usize; Q as usize];
            for _ in 0..draws {
                counts[sampler.next_entry::<Q>().value() as usize] += 1;
            }
            let expected = draws as f64 / Q as f64;
            // Six standard errors of a Binomial(draws, 1/Q) count.
            let tol = 6.0 * (expected * (1.0 - 1.0 / Q as f64)).sqrt();
            for (v, &c) in counts.iter().enumerate() {
                let dev = (c as f64 - expected).abs();
                assert!(
                    dev < tol,
                    "q={Q} value={v} count={c} deviates by {dev} > {tol}"
                );
            }
        }
        check::<3>();
        check::<5>();
        check::<7>();
    }
}
