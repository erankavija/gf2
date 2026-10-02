//! Bit-matrix transpose kernels.
//!
//! This module owns the 64×64 bit-block transpose: the register-tiled scalar
//! kernel that needs no processor feature, the AVX2 lanes isolated in
//! `crate::x86::transpose`, and the one dispatch that publishes a lane to
//! callers. Every lane answers the same contract, so a caller picks a lane and
//! gets the same output words; only the instruction mix differs.
//!
//! # The block contract
//!
//! A 64-row bit-block is 64 contiguous `u64` words under little-endian bit
//! indexing: bit `c` of `input[r]` is the matrix entry $(r, c)$. A transpose
//! writes 64 words with bit `r` of `output[c]` equal to that entry, which is
//! $\mathrm{output}\[c\]\[r\] = \mathrm{input}\[r\]\[c\]$. The input and output
//! buffers must not overlap. The buffers carry no alignment requirement: every
//! lane reads and writes them through unaligned vector accesses.
//!
//! # Lanes
//!
//! [`TransposeLane`] names every implementation of that contract and
//! [`lane`] maps a name to the safe function pointer, or to `None` where the
//! host lacks the processor feature the lane needs. [`detect`] resolves the
//! production lane through [`PRODUCTION_PREFERENCE`], the measured order, and
//! is what `gf2_core::BitMatrix::transpose` and
//! [`crate::bch_encode`]'s bit-slicing reach. There is no second dispatch and
//! no private copy of a lane: a caller that wants a specific implementation
//! names it through [`TransposeLane`].
//!
//! | Lane | Mechanism |
//! |---|---|
//! | [`TransposeLane::Scalar`] | six mask-shift-XOR stages over general-purpose words (Hacker's Delight ch. 7-3) |
//! | [`TransposeLane::Avx2BitTwiddle`] | the four wide stages in YMM registers over a stack copy of the block, the two narrow stages in words |
//! | [`TransposeLane::Avx2Ymm6`] | all six stages in YMM registers, the first writing the caller's output directly, so the block is never copied to a stack scratch |
//! | [`TransposeLane::Avx2Pshufb`] | 8×8 byte tiles through a `vpshufb` bit-reversal lookup |
//! | [`TransposeLane::Avx2Movemask`] | a byte transpose through the SSE interleave ladder, then `vpmovmskb` bit-plane extraction |
//!
//! The AVX2 lanes need the `avx2` processor feature, which [`lane`] tests at
//! run time. The whole module is reachable from `gf2-core` only when that
//! crate's `simd` cargo feature is on; `simd` is not one of its defaults.
//!
//! # PPC-spiral context
//!
//! Issue `1c1c4242` (kernel B1 in
//! `dev/archive/babcf05e-gf2-core-ppc-spiral/plans/gf2_core_ppc_spiral.md`)
//! drives the PPC spiral for `gf2_core::BitMatrix::transpose`: **V0** is the
//! criterion baseline of `crates/gf2-core/benches/matrix_transpose.rs`, **V4**
//! is [`TransposeLane::Scalar`], **V3a** and **V3b** are
//! [`TransposeLane::Avx2Pshufb`] and [`TransposeLane::Avx2BitTwiddle`], and
//! **V7** is the cache-tiling outer loop `gf2-core` drives. Issue `1d4fd63d`
//! adds [`TransposeLane::Avx2Ymm6`] and [`TransposeLane::Avx2Movemask`] and
//! the lane family that measures all of them against each other.

/// Safe 64×64 bit-block transpose function pointer.
pub type Transpose64x64Fn = fn(&[u64; 64], &mut [u64; 64]);

/// One implementation of the 64×64 bit-block transpose contract.
///
/// [`lane`] turns a variant into the callable kernel, and [`name`](Self::name)
/// into the tag receipts and benchmark records report. The variants are the
/// candidates the `1d4fd63d` lane family measures; [`detect`] publishes the
/// one [`PRODUCTION_PREFERENCE`] names first among those the host supports.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TransposeLane {
    /// Six mask-shift-XOR stages over general-purpose words. Available on
    /// every host and every target architecture.
    Scalar,
    /// The four wide stages in YMM registers over a stack copy of the block;
    /// the two narrow stages run in words.
    Avx2BitTwiddle,
    /// All six stages in YMM registers, with no stack copy of the block.
    Avx2Ymm6,
    /// 8×8 byte tiles through a `vpshufb` bit-reversal lookup.
    Avx2Pshufb,
    /// A byte transpose followed by `vpmovmskb` bit-plane extraction.
    Avx2Movemask,
}

impl TransposeLane {
    /// Every lane of the family, in the order the survey reports them.
    pub const ALL: [TransposeLane; 5] = [
        TransposeLane::Scalar,
        TransposeLane::Avx2BitTwiddle,
        TransposeLane::Avx2Ymm6,
        TransposeLane::Avx2Pshufb,
        TransposeLane::Avx2Movemask,
    ];

    /// The tag receipts, benchmark records and [`TransposeFns::name`] carry.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TransposeLane::Scalar => "scalar-bit-twiddle",
            TransposeLane::Avx2BitTwiddle => "avx2-bit-twiddle",
            TransposeLane::Avx2Ymm6 => "avx2-ymm6",
            TransposeLane::Avx2Pshufb => "avx2-pshufb",
            TransposeLane::Avx2Movemask => "avx2-movemask",
        }
    }

    /// The lane [`name`](Self::name) spells, or `None` for an unknown tag.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lane| lane.name() == name)
    }
}

/// The order [`detect`] resolves the production lane in.
///
/// The first entry whose [`lane`] is available on the host wins.
/// [`TransposeLane::Scalar`] is last and always available, so the walk always
/// ends in a usable kernel. The order above it is unchanged by the evidence
/// so far: the `transpose-lane-selection` family of issue `1d4fd63d` measured
/// every AVX2 candidate against this entry, its confirmation receipt qualified
/// no candidate for production selection under the frozen rule, and
/// `dev/active/1d4fd63d/findings.md` (§ Adoption) states that rule and names
/// the receipt. Issue `63bad95d` owns the calibration of this selector.
pub const PRODUCTION_PREFERENCE: [TransposeLane; 2] =
    [TransposeLane::Avx2BitTwiddle, TransposeLane::Scalar];

/// Bundle of dispatched bit-matrix-transpose kernels.
///
/// Currently exposes a single 64×64 block primitive. Callers tile
/// arbitrary `rows × cols` matrices on top of this primitive.
#[derive(Copy, Clone)]
pub struct TransposeFns {
    /// 64×64 bit-block transpose, reading from 64 input words and writing 64
    /// output words. The input and output buffers must not overlap.
    pub transpose_64x64: Transpose64x64Fn,
    /// The lane [`transpose_64x64`](Self::transpose_64x64) implements.
    pub lane: TransposeLane,
    /// Human-readable tag of the chosen lane, equal to
    /// [`TransposeLane::name`] of [`lane`](Self::lane).
    pub name: &'static str,
}

/// Returns the kernel of one named lane, or `None` when this host or target
/// cannot run it.
///
/// This is the only way to reach a specific implementation: the AVX2 lanes are
/// `unsafe` kernels whose whole safety contract is the `avx2` processor
/// feature, and the pointer this function returns is a safe wrapper published
/// only after `is_x86_feature_detected!("avx2")` holds.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::transpose::{lane, TransposeLane};
///
/// // The scalar lane needs no processor feature.
/// let scalar = lane(TransposeLane::Scalar).expect("the scalar lane is always available");
/// let mut input = [0u64; 64];
/// input[0] = 0xFF; // row 0 has eight set bits, in columns 0..8
/// let mut output = [0u64; 64];
/// scalar(&input, &mut output);
/// for word in &output[..8] {
///     assert_eq!(word & 1, 1);
/// }
/// ```
#[must_use]
pub fn lane(lane: TransposeLane) -> Option<Transpose64x64Fn> {
    match lane {
        TransposeLane::Scalar => Some(transpose_64x64_scalar_safe),
        #[allow(unused_variables)]
        other => {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                use std::arch::is_x86_feature_detected;
                if !is_x86_feature_detected!("avx2") {
                    return None;
                }
                return Some(match other {
                    TransposeLane::Scalar => unreachable!("handled above"),
                    TransposeLane::Avx2BitTwiddle => transpose_64x64_avx2_safe,
                    TransposeLane::Avx2Ymm6 => transpose_64x64_avx2_ymm6_safe,
                    TransposeLane::Avx2Pshufb => transpose_64x64_avx2_pshufb_safe,
                    TransposeLane::Avx2Movemask => transpose_64x64_avx2_movemask_safe,
                });
            }
            #[allow(unreachable_code)]
            None
        }
    }
}

/// Detect and return the production bit-matrix-transpose kernels.
///
/// The walk is [`PRODUCTION_PREFERENCE`], so this returns `None` only where
/// even [`TransposeLane::Scalar`] is unavailable, which no supported target
/// is. Callers may equally use `lane(TransposeLane::Scalar)` directly, which
/// needs no processor feature.
///
/// # Examples
///
/// ```
/// if let Some(fns) = gf2_kernels_simd::transpose::detect() {
///     let mut input = [0u64; 64];
///     input[0] = 0xFF; // first row has 8 set bits in cols 0..8
///     let mut output = [0u64; 64];
///     (fns.transpose_64x64)(&input, &mut output);
///     // After transpose, the first 8 output rows have bit 0 set.
///     for i in 0..8 {
///         assert_eq!(output[i] & 1, 1);
///     }
///     assert_eq!(fns.name, fns.lane.name());
/// }
/// ```
#[must_use]
pub fn detect() -> Option<TransposeFns> {
    PRODUCTION_PREFERENCE.into_iter().find_map(|candidate| {
        lane(candidate).map(|transpose_64x64| TransposeFns {
            transpose_64x64,
            lane: candidate,
            name: candidate.name(),
        })
    })
}

// ---------------------------------------------------------------------------
// Safe function-pointer wrappers
// ---------------------------------------------------------------------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: `lane` publishes this pointer only when
    // `is_x86_feature_detected!("avx2")` holds, which is the whole safety
    // condition of the kernel.
    unsafe { crate::x86::transpose::transpose_64x64_avx2(input, output) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_ymm6_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: as above; `lane` established the `avx2` feature.
    unsafe { crate::x86::transpose::transpose_64x64_avx2_ymm6(input, output) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_pshufb_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: as above; `lane` established the `avx2` feature.
    unsafe { crate::x86::transpose::transpose_64x64_avx2_pshufb(input, output) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_movemask_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: as above; `lane` established the `avx2` feature.
    unsafe { crate::x86::transpose::transpose_64x64_avx2_movemask(input, output) }
}

fn transpose_64x64_scalar_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    transpose_64x64_scalar(input, output)
}

// ---------------------------------------------------------------------------
// Public scalar reference (V4): Hacker's Delight ch. 7-3
// ---------------------------------------------------------------------------

/// In-place 64×64 bit-block transpose using the recursive
/// bit-interleave / mask-and-shift pattern (Hacker's Delight ch. 7-3).
///
/// `input[r]` is interpreted as the `r`-th row, with bit `c` carrying
/// the matrix entry `(r, c)`. After the call, `output[c]` is the
/// transposed row, with bit `r` carrying `(r, c)`.
///
/// This is the V4 register-tiled scalar reference: 6 stages of
/// mask-and-XOR-swap, halving the swap distance each stage. The
/// algorithm runs in O(N log N) bit operations on N×N tiles —
/// dramatically better than the O(N²) naive double-loop.
///
/// # Algorithm sketch
///
/// The 64×64 bit matrix is viewed as a recursive partition into 32×32
/// quadrants, then 16×16, etc. At each stage the kernel swaps the
/// off-diagonal sub-quadrants of each pair using a mask-shift-XOR
/// idiom that interchanges bit columns at distance `j` with bit rows
/// at distance `j` simultaneously across all pairs.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::transpose::transpose_64x64_scalar;
/// // Identity matrix maps to itself under transpose.
/// let mut input = [0u64; 64];
/// for i in 0..64 {
///     input[i] = 1u64 << i;
/// }
/// let mut output = [0u64; 64];
/// transpose_64x64_scalar(&input, &mut output);
/// assert_eq!(input, output);
/// ```
///
/// # Complexity
///
/// O(64 · log₂ 64) = O(384) word operations. Bench numbers under
/// `dev/scripts/ppc-baselines.json` entry `B1`.
pub fn transpose_64x64_scalar(input: &[u64; 64], output: &mut [u64; 64]) {
    // Copy input into a scratch buffer; we mutate it in place.
    let mut buf: [u64; 64] = *input;

    // Stage masks. Each mask is a 64-bit pattern selecting alternating
    // groups of 2^k columns. The swap distance is the same as the
    // group width.
    //
    //   k=5  width=32, mask = 0x00000000_FFFFFFFF
    //   k=4  width=16, mask = 0x0000FFFF_0000FFFF
    //   k=3  width=8,  mask = 0x00FF00FF_00FF00FF
    //   k=2  width=4,  mask = 0x0F0F0F0F_0F0F0F0F
    //   k=1  width=2,  mask = 0x33333333_33333333
    //   k=0  width=1,  mask = 0x55555555_55555555
    const MASKS: [u64; 6] = [
        0x0000_0000_FFFF_FFFF,
        0x0000_FFFF_0000_FFFF,
        0x00FF_00FF_00FF_00FF,
        0x0F0F_0F0F_0F0F_0F0F,
        0x3333_3333_3333_3333,
        0x5555_5555_5555_5555,
    ];

    // For each stage, walk pairs of rows separated by `1 << k` and
    // swap the lower / upper halves selected by the stage mask.
    let mut k = 5usize;
    loop {
        let j = 1usize << k;
        let m = MASKS[5 - k];
        // For each block of size 2j, swap rows [i..i+j) with rows
        // [i+j..i+2j) using the mask-shift-XOR idiom.
        let mut i = 0usize;
        while i < 64 {
            let mut r = i;
            while r < i + j {
                let a = buf[r];
                let b = buf[r + j];
                // t = ((a >> j) ^ b) & m
                // a' = a ^ (t << j)
                // b' = b ^ t
                let t = ((a >> j) ^ b) & m;
                buf[r] = a ^ (t << j);
                buf[r + j] = b ^ t;
                r += 1;
            }
            i += 2 * j;
        }
        if k == 0 {
            break;
        }
        k -= 1;
    }

    *output = buf;
}

// ---------------------------------------------------------------------------
// The shared block contract
// ---------------------------------------------------------------------------

/// Contracts every lane of the family answers, run against one kernel.
///
/// The module's own tests run [`contract::assert_block_contract`] over
/// [`TransposeLane::ALL`], and `crates/gf2-core/tests/transpose_lane_contract.rs`
/// runs the matrix-level contract over the same list, so a lane added to the
/// enum is exercised by both without a second copy of the cases.
#[cfg(any(test, feature = "test-support"))]
pub mod contract {
    use super::Transpose64x64Fn;

    /// Deterministic block generator, so a failure names a reproducible case.
    ///
    /// SplitMix64 [Steele2014], the same mixer the campaign harnesses use.
    pub fn block(seed: u64) -> [u64; 64] {
        let mut state = seed;
        let mut out = [0u64; 64];
        for word in out.iter_mut() {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *word = z ^ (z >> 31);
        }
        out
    }

    /// The naive definition: bit `r` of `out[c]` is bit `c` of `input[r]`.
    pub fn naive(input: &[u64; 64]) -> [u64; 64] {
        let mut out = [0u64; 64];
        for (r, &row) in input.iter().enumerate() {
            for (c, slot) in out.iter_mut().enumerate() {
                *slot |= ((row >> c) & 1) << r;
            }
        }
        out
    }

    /// Asserts the whole block contract for one lane.
    ///
    /// The cases are the canonical relation against the naive definition on
    /// pseudorandom blocks, the zero and all-ones blocks, every single-bit
    /// block at the four corners, the involution
    /// $(A^{\mathsf T})^{\mathsf T} = A$, and the same relation with both
    /// buffers held at an odd word offset inside a larger allocation, which
    /// is storage no vector load may assume aligned.
    ///
    /// # Panics
    ///
    /// Panics naming `label` and the failing case when the kernel breaks any
    /// of them.
    pub fn assert_block_contract(label: &str, kernel: Transpose64x64Fn) {
        let mut out = [0u64; 64];

        kernel(&[0u64; 64], &mut out);
        assert_eq!(out, [0u64; 64], "{label}: zero block");
        kernel(&[!0u64; 64], &mut out);
        assert_eq!(out, [!0u64; 64], "{label}: all-ones block");

        for (row, col) in [(0usize, 0usize), (0, 63), (63, 0), (63, 63)] {
            let mut input = [0u64; 64];
            input[row] = 1u64 << col;
            let mut expected = [0u64; 64];
            expected[col] = 1u64 << row;
            kernel(&input, &mut out);
            assert_eq!(out, expected, "{label}: single bit at ({row}, {col})");
        }

        for seed in 0..16u64 {
            let input = block(0x1d4f_d63d_0000_0000 ^ seed);
            kernel(&input, &mut out);
            assert_eq!(out, naive(&input), "{label}: naive relation, seed {seed}");
            let mut back = [0u64; 64];
            kernel(&out, &mut back);
            assert_eq!(back, input, "{label}: involution, seed {seed}");
        }

        // Unaligned storage: both buffers start one word into a larger
        // allocation, so neither is 32-byte aligned however the allocator
        // placed the backing memory.
        let mut storage = vec![0u64; 1 + 64 + 1 + 64];
        let input = block(0x1d4f_d63d_0000_00ffu64);
        storage[1..65].copy_from_slice(&input);
        let (head, tail) = storage.split_at_mut(66);
        let unaligned_in: &[u64; 64] = head[1..65].try_into().expect("64 words");
        let unaligned_out: &mut [u64; 64] = (&mut tail[..64]).try_into().expect("64 words");
        kernel(unaligned_in, unaligned_out);
        assert_eq!(
            *unaligned_out,
            naive(&input),
            "{label}: naive relation at an odd word offset"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::contract::{assert_block_contract, block, naive};
    use super::*;

    /// Every available lane, with the label a failure reports.
    fn available_lanes() -> Vec<(&'static str, Transpose64x64Fn)> {
        TransposeLane::ALL
            .into_iter()
            .filter_map(|candidate| lane(candidate).map(|kernel| (candidate.name(), kernel)))
            .collect()
    }

    #[test]
    fn every_available_lane_answers_the_block_contract() {
        let lanes = available_lanes();
        assert!(
            lanes
                .iter()
                .any(|(name, _)| *name == TransposeLane::Scalar.name()),
            "the scalar lane is available on every host"
        );
        for (name, kernel) in lanes {
            assert_block_contract(name, kernel);
        }
    }

    #[test]
    fn every_available_lane_agrees_with_the_scalar_reference() {
        let scalar = lane(TransposeLane::Scalar).expect("always available");
        for (name, kernel) in available_lanes() {
            for seed in 0..32u64 {
                let input = block(seed);
                let (mut expected, mut actual) = ([0u64; 64], [0u64; 64]);
                scalar(&input, &mut expected);
                kernel(&input, &mut actual);
                assert_eq!(expected, actual, "lane {name} diverged at seed {seed}");
            }
        }
    }

    #[test]
    fn detect_publishes_a_lane_of_the_family() {
        let fns = detect().expect("the scalar lane makes detection total");
        assert_eq!(fns.name, fns.lane.name());
        assert!(PRODUCTION_PREFERENCE.contains(&fns.lane));
        assert_eq!(
            TransposeLane::from_name(fns.name),
            Some(fns.lane),
            "the reported tag names the lane it came from"
        );
        assert_block_contract(fns.name, fns.transpose_64x64);
    }

    #[test]
    fn lane_names_round_trip_and_are_distinct() {
        let mut seen = std::collections::BTreeSet::new();
        for candidate in TransposeLane::ALL {
            assert!(seen.insert(candidate.name()), "duplicate lane tag");
            assert_eq!(TransposeLane::from_name(candidate.name()), Some(candidate));
        }
        assert_eq!(TransposeLane::from_name("not-a-lane"), None);
    }

    #[test]
    fn the_naive_reference_matches_the_scalar_kernel() {
        // The contract helper's own oracle, checked against the published
        // scalar kernel so a broken oracle cannot silently pass every lane.
        for seed in 0..8u64 {
            let input = block(seed ^ 0xAAAA_AAAA);
            let mut expected = [0u64; 64];
            transpose_64x64_scalar(&input, &mut expected);
            assert_eq!(naive(&input), expected);
        }
    }
}
