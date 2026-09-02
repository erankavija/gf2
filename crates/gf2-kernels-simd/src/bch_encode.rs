//! Bit-sliced batch-encoding kernels for binary BCH codes.
//!
//! A systematic BCH encode reduces $x^r m(x)$ modulo the generator $g$ with a
//! shift register of $r$ binary coefficients, one message coefficient per
//! step. One frame's recurrence is serial, so the parallelism a batch offers
//! is across frames: this module holds the register **bit-sliced**, one word
//! per register coefficient with bit $l$ carrying lane $l$'s value, and
//! advances [`BITSLICE_LANES`] frames per step.
//!
//! The step is then pure bitwise work. With $s_j$ the slice of register
//! coefficient $j$, $\mu_j$ the all-ones-or-zero broadcast of the generator's
//! coefficient of $x^j$, and $m$ the slice of the message coefficient
//! entering the register,
//!
//! $$ f = s_{r-1} \oplus m, \qquad s'_j = s_{j-1} \oplus (\mu_j \wedge f), $$
//!
//! with $s_{-1}$ read as zero. Every lane evaluates its own $f$ in its own
//! bit of the same word, so the whole lane group advances in $r$ word
//! operations whatever the batch length is.
//!
//! # Surface
//!
//! [`BchEncodeFns`] bundles the two primitives a lane group needs, and its
//! methods are the driver a caller runs:
//!
//! - [`absorb_block`](BchEncodeFns::absorb_block) takes one 64-degree window
//!   of each lane's message, bit-slices it through a 64×64 bit-block
//!   transpose, and advances the register over those degrees;
//! - [`unpack_parity`](BchEncodeFns::unpack_parity) reads the reduced slices
//!   back through the same transpose as one packed parity word run per lane.
//!
//! The transpose is [`crate::transpose`]'s primitive rather than a second
//! copy of it. [`bitslice_scratch_words`] and [`bitslice_split`] fix the
//! buffer geometry both methods read, so a caller sizes one buffer from the
//! redundancy alone and never from the batch length. Every input is a packed
//! `u64` word or a plain size.
//!
//! [`detect`] publishes the AVX2 bundle and [`scalar`] the portable one. The
//! two compute the same words, so a caller that runs the scalar bundle on a
//! host that would admit the AVX2 one gets identical output; only the
//! instruction count differs.
//!
//! # Required processor features
//!
//! [`detect`] is one predicate over the complete feature set both kernels of
//! its bundle need. That set is `avx2` alone: the reduction emits
//! `vpbroadcastq`, `vpand`, `vpxor`, and unaligned 256-bit loads and stores,
//! and the AVX2 transpose lane emits `vpand`, `vpsllq`, `vpsrlq`, and
//! `vpxor`. Neither kernel multiplies, so neither needs `pclmulqdq`; neither
//! extracts or deposits bit fields, so neither needs `bmi2`. The `# Safety`
//! section of every kernel under `src/x86/` that this module publishes names
//! that same feature, so the contract the predicate establishes and the
//! contract the kernel demands are the same sentence.
//!
//! # Generated code of the scalar path
//!
//! The survey behind this module left one question open: whether a scalar
//! per-frame encoder auto-vectorizes, which would explain why an eight-lane
//! interleaved encoder gains 4.4× at generator degree 32 but only 1.6–2.0× at
//! the DVB-T2 degrees 168 and 192. The committed artefact
//! `src/x86/asm/bch_encode.asm.txt` answers it for this repository's kernels.
//! [`bitslice_reduce_scalar`] compiles to a `mov`/`and`/`xor`/`mov` chain over
//! general-purpose registers, one register coefficient per iteration, with no
//! `v`-prefixed instruction and no unrolling: a recurrence whose next word
//! reads the word one index below does not auto-vectorize, and the packed
//! per-frame recurrence the coding crate registers as `poly-remainder-scalar`
//! carries the same dependence through its shift-and-feedback loop. The AVX2
//! kernel emits one `vmovq`/`vpbroadcastq` pair per step over a
//! `vpand`/`vpxor`/`vmovdqu` inner loop advancing four register coefficients
//! at a time. So an interleaved kernel here competes against genuinely scalar
//! code, and the decay of its advantage at large $\deg g$ is not an
//! auto-vectorizing competitor: it is that the reduction's word count grows
//! with $\deg g$ while the per-frame codeword write it is amortized against
//! does not.
//!
//! # Complexity
//!
//! One lane group of [`BITSLICE_LANES`] frames costs $r$ word operations per
//! message degree in [`bitslice_reduce_scalar`] and $\lceil r/4 \rceil$
//! 256-bit operations in the AVX2 kernel, so $O(k r / 64)$ and
//! $O(k r / 256)$ word-equivalents per frame respectively, against the
//! $O(k \lceil r/64 \rceil)$ of a packed per-frame recurrence. A batch below
//! the lane width pays a whole lane group, which is what makes the family's
//! admission a batch-length question.
//!
//! # Examples
//!
//! Two frames of a one-coefficient and a two-coefficient message under
//! $g = x^3 + x + 1$, encoded in one lane group.
//!
//! ```
//! use gf2_kernels_simd::bch_encode::{self, BITSLICE_LANES};
//!
//! // The generator's low coefficients are 1 + x, packed into one word; the
//! // monic leading coefficient never enters the recurrence.
//! let redundancy = 3;
//! let low = [0b011u64];
//! let mut scratch = vec![0u64; bch_encode::bitslice_scratch_words(redundancy)];
//! let mut parts = bch_encode::bitslice_split(&mut scratch, redundancy);
//! bch_encode::bitslice_masks(&low, parts.masks);
//!
//! let fns = bch_encode::detect().unwrap_or_else(bch_encode::scalar);
//!
//! // Lane 0 encodes m(x) = 1 and lane 1 encodes m(x) = x, so the group runs
//! // one block of two message degrees; bit j of a lane's window carries that
//! // lane's coefficient of x^j.
//! let mut windows = [0u64; BITSLICE_LANES];
//! windows[0] = 0b01;
//! windows[1] = 0b10;
//! fns.absorb_block(parts.register, parts.masks, &windows, 2);
//! fns.unpack_parity(parts.register, redundancy, parts.parity);
//!
//! // x^3 ≡ x + 1 and x^4 ≡ x^2 + x modulo the generator.
//! assert_eq!(parts.parity[0], 0b011);
//! assert_eq!(parts.parity[1], 0b110);
//! ```

/// Frames one bit-sliced lane group carries.
///
/// One word of a slice holds one bit per lane, so this is the width of the
/// register word the kernels run over and the block width of the transpose
/// that fills them.
pub const BITSLICE_LANES: usize = 64;

/// Bit-sliced shift-register reduction over one lane group.
///
/// The arguments are `(register, masks, slices)`:
///
/// - `register` holds `masks.len() + 1` words. `register[0]` is a pad the
///   recurrence reads as the coefficient below degree zero and never writes,
///   so it stays zero for the whole reduction; `register[1 + j]` is the slice
///   of register coefficient `j`, its bit `l` carrying lane `l`'s value.
///   The caller zeroes the whole buffer before the first block of a batch and
///   passes it back unchanged between the blocks of one batch.
/// - `masks` holds one word per register coefficient: `masks[j]` is
///   [`u64::MAX`] when the generator's coefficient of $x^j$ is one and zero
///   otherwise. Its length is the redundancy $r$.
/// - `slices` holds one word per message degree of the block, ascending:
///   bit `l` of `slices[d]` is lane `l`'s coefficient of the block's degree
///   `d`. The kernel consumes them from the last index to the first, which is
///   the highest message degree first.
///
/// A lane the caller is not filling carries zero in every slice and leaves
/// zeros in its bit of the register, so a partial final lane group costs a
/// full group and writes the same bits a full one would.
pub type BchBitsliceReduceFn = fn(&mut [u64], &[u64], &[u64]);

/// Bundle of dispatched bit-sliced BCH batch-encoding kernels.
#[derive(Copy, Clone, Debug)]
pub struct BchEncodeFns {
    /// Transposes a 64×64 bit block, both to bit-slice a lane group's message
    /// words and to read the reduced slices back as per-frame parity words.
    pub transpose_lane_block: crate::transpose::Transpose64x64Fn,
    /// Advances the bit-sliced shift register over one block of message
    /// degrees.
    pub bitslice_reduce: BchBitsliceReduceFn,
    /// Human-readable tag of the chosen bundle, `"avx2-bitslice"` or
    /// `"scalar-bitslice"`.
    pub name: &'static str,
}

impl BchEncodeFns {
    /// Advances `register` over one block of `degrees` message degrees.
    ///
    /// `windows[l]` carries lane `l`'s coefficients of the block, bit `j`
    /// holding the coefficient of the block's degree `j`; bits at and above
    /// `degrees`, and the windows of lanes the caller is not filling, are
    /// ignored. The block is bit-sliced through
    /// [`transpose_lane_block`](Self::transpose_lane_block) and reduced
    /// through [`bitslice_reduce`](Self::bitslice_reduce), so the highest
    /// degree of the block enters the register first. A caller runs the
    /// blocks of one lane group from the highest degrees down.
    ///
    /// `register` and `masks` are the [`BchBitsliceReduceFn`] buffers, which
    /// [`bitslice_split`] carves out of one scratch allocation.
    ///
    /// # Panics
    ///
    /// Panics when `degrees` exceeds [`BITSLICE_LANES`], and through the
    /// reduction kernel when `register` does not hold exactly one word more
    /// than `masks`.
    ///
    /// # Complexity
    ///
    /// One 64×64 bit-block transpose plus `degrees * masks.len()` word
    /// operations, for [`BITSLICE_LANES`] frames at once.
    pub fn absorb_block(
        &self,
        register: &mut [u64],
        masks: &[u64],
        windows: &[u64; BITSLICE_LANES],
        degrees: usize,
    ) {
        assert!(
            degrees <= BITSLICE_LANES,
            "a block carries at most {BITSLICE_LANES} message degrees, not {degrees}"
        );
        let mut slices = [0u64; BITSLICE_LANES];
        (self.transpose_lane_block)(windows, &mut slices);
        (self.bitslice_reduce)(register, masks, &slices[..degrees]);
    }

    /// Reads the reduced `register` back as one packed parity word run per
    /// lane.
    ///
    /// `parity[l * w + i]` is lane `l`'s parity word `i`, bit `b` carrying
    /// the remainder's coefficient of $x^{64 i + b}$, where `w` is
    /// `redundancy.div_ceil(64)`. Coefficients above $r - 1$ are written
    /// clear, so every lane's run carries the zero tail padding a packed
    /// buffer maintains.
    ///
    /// # Panics
    ///
    /// Panics when `register` does not hold `redundancy + 1` words or
    /// `parity` does not hold [`BITSLICE_LANES`] runs of
    /// `redundancy.div_ceil(64)` words.
    ///
    /// # Complexity
    ///
    /// One 64×64 bit-block transpose per parity word, so
    /// $\lceil r/64 \rceil$ of them for all [`BITSLICE_LANES`] lanes
    /// together.
    pub fn unpack_parity(&self, register: &[u64], redundancy: usize, parity: &mut [u64]) {
        let words = redundancy.div_ceil(64);
        assert_eq!(
            register.len(),
            redundancy + 1,
            "a bit-sliced register holds one pad word below its {redundancy} coefficient slices"
        );
        assert_eq!(
            parity.len(),
            BITSLICE_LANES * words,
            "parity holds {BITSLICE_LANES} runs of {words} words"
        );

        let mut block = [0u64; BITSLICE_LANES];
        let mut lanes = [0u64; BITSLICE_LANES];
        for word in 0..words {
            let start = word * BITSLICE_LANES;
            let count = (redundancy - start).min(BITSLICE_LANES);
            block[..count].copy_from_slice(&register[1 + start..1 + start + count]);
            block[count..].fill(0);
            (self.transpose_lane_block)(&block, &mut lanes);
            for (lane, slot) in lanes.iter().enumerate() {
                parity[lane * words + word] = *slot;
            }
        }
    }
}

/// The scratch buffers one lane group reduces over.
///
/// [`bitslice_split`] carves these out of a single allocation the caller
/// keeps, so a batch touches no allocator between lane groups.
#[derive(Debug)]
pub struct BchBitsliceScratch<'a> {
    /// One word per register coefficient, [`u64::MAX`] where the generator's
    /// coefficient of that degree is one. [`bitslice_masks`] fills it.
    pub masks: &'a mut [u64],
    /// The bit-sliced shift register, one pad word below `redundancy`
    /// coefficient slices, zeroed and so ready for a lane group's first
    /// block.
    pub register: &'a mut [u64],
    /// [`BITSLICE_LANES`] runs of `redundancy.div_ceil(64)` packed parity
    /// words, which [`BchEncodeFns::unpack_parity`] writes.
    pub parity: &'a mut [u64],
}

/// Words one lane group's scratch needs at `redundancy`.
///
/// The masks, the bit-sliced register, and the per-lane parity runs together,
/// so the buffer is a function of the redundancy and the fixed lane width and
/// never of the batch length.
#[must_use]
pub fn bitslice_scratch_words(redundancy: usize) -> usize {
    redundancy + (redundancy + 1) + BITSLICE_LANES * redundancy.div_ceil(64)
}

/// Splits `scratch` into the buffers one lane group reduces over.
///
/// The register comes back zeroed, which is the state a lane group's first
/// block reads; the masks and the parity runs are left as the caller last
/// wrote them.
///
/// # Panics
///
/// Panics when `scratch` does not hold exactly
/// [`bitslice_scratch_words(redundancy)`](bitslice_scratch_words) words.
pub fn bitslice_split(scratch: &mut [u64], redundancy: usize) -> BchBitsliceScratch<'_> {
    assert_eq!(
        scratch.len(),
        bitslice_scratch_words(redundancy),
        "a lane group's scratch holds its masks, register, and parity runs"
    );
    let (masks, rest) = scratch.split_at_mut(redundancy);
    let (register, parity) = rest.split_at_mut(redundancy + 1);
    register.fill(0);
    BchBitsliceScratch {
        masks,
        register,
        parity,
    }
}

/// Expands the generator's packed low coefficients into one broadcast word
/// per register coefficient.
///
/// `low` is the packed form the recurrence's caller keeps, bit `d` of word
/// `d / 64` carrying the generator's coefficient of $x^d$. `masks[d]` comes
/// back [`u64::MAX`] when that coefficient is one and zero otherwise, which
/// is what makes the reduction's feedback one `and` per coefficient rather
/// than a branch. Its length fixes the redundancy.
///
/// # Panics
///
/// Panics when `low` is shorter than the packed form of `masks.len()`
/// coefficients.
pub fn bitslice_masks(low: &[u64], masks: &mut [u64]) {
    assert!(
        low.len() >= masks.len().div_ceil(64),
        "{} packed words do not carry {} coefficients",
        low.len(),
        masks.len()
    );
    for (degree, mask) in masks.iter_mut().enumerate() {
        let set = (low[degree / 64] >> (degree % 64)) & 1 == 1;
        *mask = if set { u64::MAX } else { 0 };
    }
}

/// Returns the accelerated bundle when the host has every processor feature
/// its kernels need, and `None` otherwise.
///
/// The predicate is the module's [required feature set](self#required-processor-features):
/// one `avx2` detection covering both kernels of the bundle. A caller that
/// receives `None` runs [`scalar`], which needs no processor feature and
/// computes the same words.
#[must_use]
pub fn detect() -> Option<BchEncodeFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

/// Returns the bundle that needs no processor feature.
///
/// This is the fallback [`detect`] leaves a caller on, and the bundle a
/// differential check runs to compare the accelerated kernels against
/// portable ones on a host that has both.
#[must_use]
pub fn scalar() -> BchEncodeFns {
    BchEncodeFns {
        transpose_lane_block: crate::transpose::transpose_64x64_scalar,
        bitslice_reduce: bitslice_reduce_scalar,
        name: "scalar-bitslice",
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<BchEncodeFns> {
    use std::arch::is_x86_feature_detected;

    // One predicate over the complete feature set both kernels of the bundle
    // need; see the module-level section on required processor features for
    // why `avx2` is that whole set.
    if !is_x86_feature_detected!("avx2") {
        return None;
    }
    // The transpose lane is `crate::transpose`'s own dispatch rather than a
    // second wrapper around the same kernel, and it publishes its AVX2 lane
    // under exactly the feature this predicate just established.
    let transpose = crate::transpose::detect()?;
    Some(BchEncodeFns {
        transpose_lane_block: transpose.transpose_64x64,
        bitslice_reduce: bitslice_reduce_avx2_safe,
        name: "avx2-bitslice",
    })
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn bitslice_reduce_avx2_safe(register: &mut [u64], masks: &[u64], slices: &[u64]) {
    // SAFETY: `detect_x86` publishes this pointer only when
    // `is_x86_feature_detected!("avx2")` holds, which is the whole safety
    // condition of `x86::bch_encode::bitslice_reduce_avx2`; the kernel
    // decides the buffer geometry itself.
    unsafe { crate::x86::bch_encode::bitslice_reduce_avx2(register, masks, slices) }
}

/// Portable bit-sliced shift-register reduction.
///
/// This is the [`BchBitsliceReduceFn`] contract with no processor feature
/// requirement, and the kernel the accelerated one is checked against.
///
/// # Panics
///
/// Panics when `register` does not hold exactly one word more than `masks`,
/// which is the geometry the recurrence's pad word requires.
///
/// # Complexity
///
/// `masks.len()` word operations per entry of `slices`, advancing
/// [`BITSLICE_LANES`] frames at once.
pub fn bitslice_reduce_scalar(register: &mut [u64], masks: &[u64], slices: &[u64]) {
    let redundancy = masks.len();
    assert_eq!(
        register.len(),
        redundancy + 1,
        "a bit-sliced register holds one pad word below its {redundancy} coefficient slices"
    );
    if redundancy == 0 {
        return;
    }
    for &entering in slices.iter().rev() {
        let feedback = register[redundancy] ^ entering;
        for degree in (0..redundancy).rev() {
            register[degree + 1] = register[degree] ^ (masks[degree] & feedback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SplitMix64, the workspace's seeded generator for kernel-local fixtures.
    struct Seeded(u64);

    impl Seeded {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        /// `count` random bits, packed little-endian with a zero tail.
        fn bits(&mut self, count: usize) -> Vec<u64> {
            let mut words = vec![0u64; count.div_ceil(64)];
            for word in words.iter_mut() {
                *word = self.next();
            }
            if !count.is_multiple_of(64) {
                if let Some(last) = words.last_mut() {
                    *last &= (1u64 << (count % 64)) - 1;
                }
            }
            words
        }
    }

    fn bit(words: &[u64], index: usize) -> bool {
        words
            .get(index / 64)
            .is_some_and(|word| (word >> (index % 64)) & 1 == 1)
    }

    /// The reference: one frame at a time, one message coefficient per step,
    /// over a register of `redundancy` booleans.
    fn naive_parity(
        message: &[u64],
        dimension: usize,
        low: &[u64],
        redundancy: usize,
    ) -> Vec<bool> {
        let mut register = vec![false; redundancy];
        if redundancy == 0 {
            return register;
        }
        for degree in (0..dimension).rev() {
            let feedback = register[redundancy - 1] != bit(message, degree);
            for index in (1..redundancy).rev() {
                register[index] = register[index - 1] != (feedback && bit(low, index));
            }
            register[0] = feedback && bit(low, 0);
        }
        register
    }

    /// Reads the 64 message coefficients of degrees `offset..offset + 64`,
    /// bit `j` carrying the coefficient of $x^{offset + j}$; degrees past the
    /// buffer read as zero.
    fn read_window(message: &[u64], offset: usize) -> u64 {
        let word = offset / 64;
        let shift = offset % 64;
        let low = message.get(word).copied().unwrap_or(0) >> shift;
        let high = if shift == 0 {
            0
        } else {
            message.get(word + 1).copied().unwrap_or(0) << (64 - shift)
        };
        low | high
    }

    /// Drives `fns` over a whole batch through the module's own surface.
    fn bitslice_parity(
        fns: &BchEncodeFns,
        messages: &[Vec<u64>],
        dimension: usize,
        low: &[u64],
        redundancy: usize,
    ) -> Vec<Vec<bool>> {
        let words = redundancy.div_ceil(64);
        let mut parities = vec![vec![false; redundancy]; messages.len()];
        if redundancy == 0 {
            return parities;
        }
        let mut buffer = vec![0u64; bitslice_scratch_words(redundancy)];
        {
            let parts = bitslice_split(&mut buffer, redundancy);
            bitslice_masks(low, parts.masks);
        }

        for (group, frames) in messages.chunks(BITSLICE_LANES).enumerate() {
            let parts = bitslice_split(&mut buffer, redundancy);
            let mut degree = dimension;
            while degree > 0 {
                let count = ((degree - 1) % BITSLICE_LANES) + 1;
                degree -= count;
                let mut windows = [0u64; BITSLICE_LANES];
                for (window, message) in windows.iter_mut().zip(frames) {
                    *window = read_window(message, degree);
                }
                fns.absorb_block(parts.register, parts.masks, &windows, count);
            }
            fns.unpack_parity(parts.register, redundancy, parts.parity);
            for (lane, parity) in parities[group * BITSLICE_LANES..]
                .iter_mut()
                .take(frames.len())
                .enumerate()
            {
                let run = &parts.parity[lane * words..(lane + 1) * words];
                for (degree, slot) in parity.iter_mut().enumerate() {
                    *slot = bit(run, degree);
                }
            }
        }
        parities
    }

    const DIMENSIONS: &[usize] = &[0, 1, 63, 64, 65, 200];
    const REDUNDANCIES: &[usize] = &[1, 27, 45, 63, 64, 65, 192];
    const BATCHES: &[usize] = &[0, 1, 63, 64, 65, 255, 256, 257];

    fn check_bundle(fns: &BchEncodeFns) {
        let mut seeded = Seeded(0x2b69_68d3_0000_0001);
        for &dimension in DIMENSIONS {
            for &redundancy in REDUNDANCIES {
                let low = seeded.bits(redundancy);
                for &batch in BATCHES {
                    let messages: Vec<Vec<u64>> =
                        (0..batch).map(|_| seeded.bits(dimension)).collect();
                    let produced = bitslice_parity(fns, &messages, dimension, &low, redundancy);
                    for (frame, message) in messages.iter().enumerate() {
                        assert_eq!(
                            produced[frame],
                            naive_parity(message, dimension, &low, redundancy),
                            "{} diverged at k={dimension} r={redundancy} B={batch} frame {frame}",
                            fns.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_scalar_bundle_matches_the_per_frame_reference() {
        check_bundle(&scalar());
    }

    #[test]
    fn the_detected_bundle_matches_the_per_frame_reference() {
        let Some(fns) = detect() else {
            return;
        };
        check_bundle(&fns);
    }

    #[test]
    fn the_detected_bundle_matches_the_scalar_bundle_word_for_word() {
        let Some(fns) = detect() else {
            return;
        };
        let mut seeded = Seeded(0x2b69_68d3_0000_0002);
        for &redundancy in REDUNDANCIES {
            let masks: Vec<u64> = (0..redundancy)
                .map(|_| if seeded.next() & 1 == 1 { u64::MAX } else { 0 })
                .collect();
            for &degrees in &[0usize, 1, 7, 64, 200] {
                let slices = (0..degrees).map(|_| seeded.next()).collect::<Vec<u64>>();
                let mut expected = vec![0u64; redundancy + 1];
                let mut produced = vec![0u64; redundancy + 1];
                bitslice_reduce_scalar(&mut expected, &masks, &slices);
                (fns.bitslice_reduce)(&mut produced, &masks, &slices);
                assert_eq!(
                    produced, expected,
                    "{} diverged at r={redundancy} over {degrees} degrees",
                    fns.name
                );
            }
        }
    }

    #[test]
    fn masks_broadcast_the_packed_generator_coefficients() {
        let mut seeded = Seeded(0x2b69_68d3_0000_0003);
        for &redundancy in REDUNDANCIES {
            let low = seeded.bits(redundancy);
            let mut masks = vec![0u64; redundancy];
            bitslice_masks(&low, &mut masks);
            for (degree, &mask) in masks.iter().enumerate() {
                assert_eq!(mask, if bit(&low, degree) { u64::MAX } else { 0 });
            }
        }
    }

    #[test]
    fn a_lane_group_scratch_splits_into_its_three_buffers() {
        for &redundancy in REDUNDANCIES {
            let mut buffer = vec![u64::MAX; bitslice_scratch_words(redundancy)];
            let parts = bitslice_split(&mut buffer, redundancy);
            assert_eq!(parts.masks.len(), redundancy);
            assert_eq!(parts.register.len(), redundancy + 1);
            assert_eq!(parts.parity.len(), BITSLICE_LANES * redundancy.div_ceil(64));
            assert!(
                parts.register.iter().all(|&word| word == 0),
                "a split leaves the register ready for a lane group's first block"
            );
        }
    }

    #[test]
    fn unpacked_parity_keeps_its_tail_padding_clear() {
        // r = 65 leaves 63 padding coefficients in each lane's second word.
        let redundancy = 65;
        let fns = scalar();
        let mut register = vec![u64::MAX; redundancy + 1];
        register[0] = 0;
        let mut parity = vec![0u64; BITSLICE_LANES * 2];
        fns.unpack_parity(&register, redundancy, &mut parity);
        for lane in 0..BITSLICE_LANES {
            assert_eq!(parity[lane * 2], u64::MAX);
            assert_eq!(parity[lane * 2 + 1], 1, "only coefficient 64 survives");
        }
    }

    #[test]
    fn the_pad_word_below_degree_zero_stays_clear() {
        let masks = vec![u64::MAX; 5];
        let slices = vec![u64::MAX; 9];
        let mut register = vec![0u64; 6];
        bitslice_reduce_scalar(&mut register, &masks, &slices);
        assert_eq!(register[0], 0);
    }

    #[test]
    #[should_panic(expected = "one pad word")]
    fn a_register_without_its_pad_word_is_rejected() {
        let masks = vec![u64::MAX; 4];
        let mut register = vec![0u64; 4];
        bitslice_reduce_scalar(&mut register, &masks, &[1]);
    }

    #[test]
    #[should_panic(expected = "at most 64 message degrees")]
    fn a_block_wider_than_the_lane_width_is_rejected() {
        let mut register = vec![0u64; 2];
        scalar().absorb_block(&mut register, &[u64::MAX], &[0; BITSLICE_LANES], 65);
    }

    #[test]
    fn an_empty_register_reduces_to_nothing() {
        let mut register = [0u64; 1];
        bitslice_reduce_scalar(&mut register, &[], &[u64::MAX; 4]);
        assert_eq!(register, [0]);
    }
}
