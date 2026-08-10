//! Uniform $\mathbb{F}_q$ matrix sampling by exact rejection.
//!
//! # Why rejection
//!
//! This sampler consumes bytes, and `256 % 7 = 4`, so a bare `byte % 7` would
//! give the residues `0..4` probability `37/256` and the rest `36/256` — the
//! worst class off uniform by 1.56 %, and 2.78 % between the most and least
//! likely. Against the campaign's tightest target, a standard error of
//! $10^{-4}$ on a probability near $1/7$, which is 0.07 % of the same
//! reference, that is about 22 times the resolution being aimed at. Bytes at or
//! above [`accept_bound`] are therefore discarded and redrawn.
//!
//! An earlier revision of this note put the ratio at three orders of magnitude,
//! comparing a relative imbalance against an absolute standard error. Both
//! figures above are relative to $1/7$; the conclusion is unchanged.
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
//! seed[24..32] = purpose tag | stream index
//! ```
//!
//! The purpose tag occupies a fixed high field and the bounded stream index
//! occupies the remaining low field. Distinct `(root, q, n, purpose, index)`
//! tuples therefore have distinct seed addresses, and any single draw is
//! reproducible from its tuple alone. This does not claim the resulting
//! ChaCha20 output sequences are cryptographically disjoint.
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

/// Width of the high stream-word field reserved for measurement purposes.
pub const PURPOSE_TAG_BITS: u32 = 16;
/// Width of the low stream-word field reserved for per-purpose indices.
pub const STREAM_INDEX_BITS: u32 = u64::BITS - PURPOSE_TAG_BITS;
/// Number of purpose tags the seed-address encoding can represent.
pub const PURPOSE_TAG_CAPACITY: u64 = 1u64 << PURPOSE_TAG_BITS;
/// Number of stream indices available to each measurement purpose.
pub const STREAM_INDEX_CAPACITY: u64 = 1u64 << STREAM_INDEX_BITS;

/// Named consumers of the feasibility study's deterministic randomness.
///
/// Each member has one fixed tag in the high field of the stream seed word.
/// The exhaustive [`Self::tag`] match is deliberate: adding a purpose without
/// a tag is a compile error, while the purpose-set test checks the registered
/// tags are valid and distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MeasurementPurpose {
    /// Shared matrices for cross-backend agreement checks.
    Equivalence,
    /// One-matrix calibration used to size a grid cell's batch.
    GridProbe,
    /// Untimed per-cell warm-up repetitions.
    GridWarmup,
    /// Timed per-cell grid repetitions.
    GridTimed,
    /// Full-machine thermal warm-up before the grid.
    MachineWarmup,
    /// Long-running throughput checks.
    Sustained,
    /// Deterministic ordering of the grid specification list.
    Shuffle,
}

impl MeasurementPurpose {
    /// Complete, canonical purpose set.
    pub const ALL: [Self; 7] = [
        Self::Equivalence,
        Self::GridProbe,
        Self::GridWarmup,
        Self::GridTimed,
        Self::MachineWarmup,
        Self::Sustained,
        Self::Shuffle,
    ];

    /// Fixed tag encoded in the high field of the stream word.
    #[must_use]
    pub const fn tag(self) -> u64 {
        match self {
            Self::Equivalence => 1,
            Self::GridProbe => 2,
            Self::GridWarmup => 3,
            Self::GridTimed => 4,
            Self::MachineWarmup => 5,
            Self::Sustained => 6,
            Self::Shuffle => 7,
        }
    }

    /// Stable name recorded with a receipt's stream index.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Equivalence => "equivalence",
            Self::GridProbe => "grid_probe",
            Self::GridWarmup => "grid_warmup",
            Self::GridTimed => "grid_timed",
            Self::MachineWarmup => "machine_warmup",
            Self::Sustained => "sustained",
            Self::Shuffle => "shuffle",
        }
    }
}

/// Largest byte value that may be accepted for field order `q`, exclusive.
///
/// Bytes in `0..accept_bound(q)` cover every residue class an equal number of
/// times, so reducing an accepted byte modulo `q` is exactly uniform. Bytes at
/// or above the bound are discarded and redrawn.
///
/// | `q` | bound | rejection probability |
/// |-----|-------|-----------------------|
/// | 2   | 256   | 0 (no byte rejected)  |
/// | 3   | 255   | 1/256                 |
/// | 5   | 255   | 1/256                 |
/// | 7   | 252   | 4/256                 |
///
/// # Supported domain
///
/// `2 <= q <= 256`. The return type is `u16` rather than `u8` precisely so that
/// the divisors of 256 are representable: at `q = 2` the bound is 256, and
/// truncating that to a `u8` would yield 0, which would reject every byte and
/// make [`MatrixSampler::next_entry`] loop forever. The campaign only uses
/// `q in {3, 5, 7}`, but a silently non-terminating sampler is not an
/// acceptable failure mode for a public function.
///
/// # Panics
///
/// Panics if `q < 2` or `q > 256`. A one-element field has no uniform
/// distribution worth sampling, and above 256 a single byte cannot cover the
/// range — that would need a wider draw, which this sampler does not implement.
#[must_use]
pub const fn accept_bound(q: u64) -> u16 {
    assert!(
        2 <= q && q <= 256,
        "accept_bound: q must satisfy 2 <= q <= 256; this sampler draws one byte per entry"
    );
    (256 - 256 % q) as u16
}

/// Encode a purpose tag and stream index into the fourth seed word.
///
/// The proof supplied by this encoding is about **seed addresses**: distinct
/// valid `(purpose, index)` pairs have distinct fourth seed words. It does not
/// claim that the ChaCha20 output streams are disjoint as sequences.
#[must_use]
fn stream_address_from_tag(tag: u64, index: u64) -> u64 {
    assert!(
        tag < PURPOSE_TAG_CAPACITY,
        "purpose tag {tag} exceeds the {PURPOSE_TAG_BITS}-bit seed-address field"
    );
    assert!(
        index < STREAM_INDEX_CAPACITY,
        "stream index {index} exceeds the {STREAM_INDEX_BITS}-bit seed-address field"
    );
    (tag << STREAM_INDEX_BITS) | index
}

/// Encode one named measurement purpose and its bounded stream index.
#[must_use]
pub fn stream_address(purpose: MeasurementPurpose, index: u64) -> u64 {
    stream_address_from_tag(purpose.tag(), index)
}

/// Assemble the 32-byte ChaCha20 seed for one `(root, q, n, purpose, index)`
/// tuple.
#[must_use]
pub fn derive_seed(
    root: u64,
    q: u64,
    n: usize,
    purpose: MeasurementPurpose,
    index: u64,
) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed[0..8].copy_from_slice(&root.to_le_bytes());
    seed[8..16].copy_from_slice(&q.to_le_bytes());
    seed[16..24].copy_from_slice(&(n as u64).to_le_bytes());
    seed[24..32].copy_from_slice(&stream_address(purpose, index).to_le_bytes());
    seed
}

/// A domain-separated uniform sampler over $\mathbb{F}_q^{n \times n}$.
pub struct MatrixSampler {
    rng: ChaCha20Rng,
    buf: Vec<u8>,
    /// Read cursor into `buf`; `buf[cursor..]` is unconsumed randomness.
    cursor: usize,
    /// The `q` this stream was addressed with. Retained only to catch a `Q`
    /// that disagrees with it; see [`MatrixSampler::new`].
    stream_q: u64,
}

/// Bytes drawn from ChaCha20 per refill. One ChaCha20 block is 64 bytes; a
/// 4 KiB refill amortises the block function over 64 blocks and keeps the
/// buffer inside L1.
const REFILL_BYTES: usize = 4096;

impl MatrixSampler {
    /// Open the stream addressed by `(root, q, n, purpose, index)`.
    ///
    /// # The two `q`s
    ///
    /// This `q` is a **stream label**: it only feeds [`derive_seed`], and it is
    /// what gives the F_3, F_5 and F_7 streams of one campaign distinct seed
    /// addresses. The
    /// field order actually sampled is the const parameter `Q` on
    /// [`MatrixSampler::next_entry`] and [`MatrixSampler::next_matrix`].
    ///
    /// Nothing in the type system ties them together, so
    /// `MatrixSampler::new(root, 5, n, purpose, index).next_matrix::<7>(n)` is accepted and
    /// silently draws F_7 matrices from the stream reserved for F_5 — a
    /// reproducibility defect rather than a distributional one, since the draws
    /// are still uniform over F_7, but it collides with whatever else uses the
    /// F_5 stream. A `debug_assert` in `next_entry` catches the mismatch in
    /// debug and test builds; release builds do not pay for the check.
    ///
    /// `q` is deliberately unconstrained here: [`crate::protocol::shuffle`]
    /// opens a stream with a sentinel `q` far outside the samplable domain and
    /// only ever calls [`MatrixSampler::next_raw_byte`], which has no domain
    /// restriction.
    #[must_use]
    pub fn new(root: u64, q: u64, n: usize, purpose: MeasurementPurpose, index: u64) -> Self {
        Self {
            rng: ChaCha20Rng::from_seed(derive_seed(root, q, n, purpose, index)),
            buf: vec![0u8; REFILL_BYTES],
            cursor: REFILL_BYTES,
            stream_q: q,
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
    ///
    /// # Panics
    ///
    /// Panics if `Q` is outside [`accept_bound`]'s supported domain
    /// `2 <= Q <= 256`. In debug builds, also panics if `Q` disagrees with the
    /// `q` this sampler's stream was addressed with — see
    /// [`MatrixSampler::new`] for why those are two different values.
    ///
    /// # Termination
    ///
    /// Each iteration accepts with probability `accept_bound(Q) / 256`, which
    /// is at least `(256 - Q + 1) / 256 >= 1/256` on the supported domain, so
    /// the loop terminates with probability 1 and takes at most `256/(256-Q+1)`
    /// draws in expectation — under 1.02 draws for every `Q <= 7`.
    pub fn next_entry<const Q: u64>(&mut self) -> Fp<Q> {
        debug_assert_eq!(
            Q, self.stream_q,
            "sampling F_{Q} from the stream addressed as q={}: the draws are \
uniform but they collide with that stream's own reservation",
            self.stream_q
        );
        let bound = accept_bound(Q);
        loop {
            let b = self.next_raw_byte();
            if u16::from(b) < bound {
                return Fp::<Q>::new(u64::from(b) % Q);
            }
        }
    }

    /// Draw one `n × n` matrix in row-major order: draw `k` becomes
    /// `A[k / n][k % n]`.
    ///
    /// # Supported domain
    ///
    /// `2 <= Q <= 256`, and `Q` must equal the `q` this sampler was opened with
    /// — see [`MatrixSampler::new`] for why the two are separate and what goes
    /// wrong when they disagree. Both conditions are inherited from
    /// [`MatrixSampler::next_entry`], which draws every element.
    ///
    /// # Panics
    ///
    /// Panics if `Q` is outside the supported domain. In debug builds, also
    /// panics if `Q` disagrees with the sampler's stream `q`.
    ///
    /// # Termination
    ///
    /// Terminates with probability 1, drawing `n^2` entries at under 1.02 bytes
    /// each for every `Q <= 7`; see [`MatrixSampler::next_entry`] for the
    /// rejection bound this follows from.
    pub fn next_matrix<const Q: u64>(&mut self, n: usize) -> Vec<Fp<Q>> {
        (0..n * n).map(|_| self.next_entry::<Q>()).collect()
    }

    /// Fill `out` with one `n × n` matrix in row-major order, reusing the
    /// caller's allocation. `out` is cleared first.
    ///
    /// Same supported domain, panics, and termination as
    /// [`MatrixSampler::next_matrix`]; this differs only in reusing the
    /// caller's buffer.
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
    use std::collections::HashSet;

    #[test]
    fn purpose_and_index_address_the_seed_word() {
        let seed = derive_seed(
            0xB488_F02C_0000_0001,
            3,
            24,
            MeasurementPurpose::GridTimed,
            17,
        );
        assert_ne!(seed[24..32], 17u64.to_le_bytes());
    }

    #[test]
    fn purpose_set_has_one_in_capacity_tag_per_member() {
        let tags: HashSet<_> = MeasurementPurpose::ALL
            .into_iter()
            .map(MeasurementPurpose::tag)
            .collect();
        assert_eq!(tags.len(), MeasurementPurpose::ALL.len());
        assert!(tags.iter().all(|&tag| tag < PURPOSE_TAG_CAPACITY));
    }

    #[test]
    fn seed_addresses_are_injective_over_purposes_and_bounded_indices() {
        let mut seeds = HashSet::new();
        for purpose in MeasurementPurpose::ALL {
            for index in 0..1024 {
                assert!(seeds.insert(derive_seed(0xB488_F02C, 7, 24, purpose, index)));
            }
        }
        assert_eq!(seeds.len(), MeasurementPurpose::ALL.len() * 1024);
    }

    #[test]
    #[should_panic(expected = "purpose tag")]
    fn purpose_tag_at_capacity_is_rejected() {
        let _ = stream_address_from_tag(PURPOSE_TAG_CAPACITY, 0);
    }

    #[test]
    #[should_panic(expected = "stream index")]
    fn stream_index_at_capacity_is_rejected() {
        let _ = stream_address_from_tag(0, STREAM_INDEX_CAPACITY);
    }

    #[test]
    fn golden_seed_addresses_and_chacha_prefixes_are_stable() {
        struct Golden {
            root: u64,
            q: u64,
            n: usize,
            purpose: MeasurementPurpose,
            index: u64,
            seed: [u8; 32],
            prefix: [u8; 16],
        }

        let vectors = [
            Golden {
                root: 0xB488_F02C_0000_0001,
                q: 3,
                n: 24,
                purpose: MeasurementPurpose::GridTimed,
                index: 17,
                seed: [
                    1, 0, 0, 0, 44, 240, 136, 180, 3, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0,
                    17, 0, 0, 0, 0, 0, 4, 0,
                ],
                prefix: [
                    5, 140, 88, 65, 91, 196, 180, 169, 111, 127, 20, 24, 233, 97, 88, 43,
                ],
            },
            Golden {
                root: 0,
                q: 5,
                n: 16,
                purpose: MeasurementPurpose::GridWarmup,
                index: 0x1234,
                seed: [
                    0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 52,
                    18, 0, 0, 0, 0, 3, 0,
                ],
                prefix: [
                    231, 16, 108, 1, 163, 194, 49, 178, 13, 236, 225, 221, 249, 51, 179, 5,
                ],
            },
            Golden {
                root: u64::MAX,
                q: 7,
                n: 20,
                purpose: MeasurementPurpose::Sustained,
                index: STREAM_INDEX_CAPACITY - 1,
                seed: [
                    255, 255, 255, 255, 255, 255, 255, 255, 7, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0,
                    0, 0, 0, 255, 255, 255, 255, 255, 255, 6, 0,
                ],
                prefix: [
                    111, 3, 82, 253, 222, 57, 95, 238, 79, 3, 156, 133, 70, 209, 16, 111,
                ],
            },
        ];

        for vector in vectors {
            let seed = derive_seed(
                vector.root,
                vector.q,
                vector.n,
                vector.purpose,
                vector.index,
            );
            assert_eq!(
                seed,
                vector.seed,
                "seed vector for {}",
                vector.purpose.name()
            );
            let mut rng = ChaCha20Rng::from_seed(seed);
            let mut prefix = [0u8; 16];
            rng.fill_bytes(&mut prefix);
            assert_eq!(
                prefix,
                vector.prefix,
                "ChaCha20 vector for {}",
                vector.purpose.name()
            );
        }
    }

    #[test]
    fn accept_bounds_are_multiples_of_q() {
        for q in [3u64, 5, 7] {
            let bound = u64::from(accept_bound(q));
            assert_eq!(bound % q, 0, "accept bound for q={q} must be a multiple");
            assert!(bound > 256 - q, "accept bound for q={q} discards too much");
        }
    }

    /// Every divisor of 256 must accept the whole byte range. A `u8` return
    /// type would truncate 256 to 0 here and hang `next_entry` forever.
    #[test]
    fn divisors_of_256_accept_every_byte() {
        for q in [2u64, 4, 8, 16, 32, 64, 128, 256] {
            assert_eq!(accept_bound(q), 256, "q={q} must reject nothing");
        }
    }

    #[test]
    fn accept_bound_covers_the_whole_supported_domain() {
        for q in 2u64..=256 {
            let bound = u64::from(accept_bound(q));
            assert!(bound >= 1, "q={q} would reject every byte");
            assert_eq!(bound % q, 0, "q={q} bound is not a multiple of q");
        }
    }

    /// The stream-label guard must actually fire, or it is dead weight. Only
    /// compiled in debug builds, since `debug_assert` is a no-op in release and
    /// this crate's normal test invocation is `--release`.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "stream addressed as q=5")]
    fn drawing_a_field_from_another_fields_stream_is_caught() {
        let mut s = MatrixSampler::new(0xB488_F02C, 5, 4, MeasurementPurpose::GridTimed, 0);
        let _ = s.next_entry::<7>();
    }

    #[test]
    #[should_panic(expected = "2 <= q <= 256")]
    fn accept_bound_rejects_q_below_two() {
        let _ = accept_bound(1);
    }

    #[test]
    #[should_panic(expected = "2 <= q <= 256")]
    fn accept_bound_rejects_q_above_256() {
        let _ = accept_bound(257);
    }

    #[test]
    fn sampling_is_reproducible_for_one_seed_address() {
        let a = MatrixSampler::new(7, 3, 4, MeasurementPurpose::GridTimed, 0).next_matrix::<3>(4);
        let b = MatrixSampler::new(7, 3, 4, MeasurementPurpose::GridTimed, 0).next_matrix::<3>(4);
        assert_eq!(a, b, "same tuple must reproduce the same draw");

        let other_stream =
            MatrixSampler::new(7, 3, 4, MeasurementPurpose::GridTimed, 1).next_matrix::<3>(4);
        assert_ne!(
            a, other_stream,
            "stream index must select a different sample"
        );

        let other_n =
            MatrixSampler::new(7, 3, 5, MeasurementPurpose::GridTimed, 0).next_matrix::<3>(4);
        assert_ne!(a, other_n, "n must select a different sample");
    }

    /// The entry distribution must be flat to well inside the campaign's
    /// target resolution. With 3 x 10^5 draws the standard error on each
    /// class frequency is below 10^-3 for every supported `q`.
    #[test]
    fn entries_are_uniform_within_sampling_error() {
        fn check<const Q: u64>() {
            let draws = 300_000usize;
            let mut sampler = MatrixSampler::new(0xFEED, Q, 1, MeasurementPurpose::GridTimed, 0);
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
