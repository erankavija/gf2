//! Batch-encoding kernels for binary BCH codes.
//!
//! Two independent reductions of $x^r m(x)$ modulo the generator $g$ live
//! here, and one dispatch bundle carries both. The bit-sliced one advances
//! [`BITSLICE_LANES`] frames of a batch per step and is described first; the
//! carry-less-multiply fold reduces one frame 64 message coefficients at a
//! time and is described under [`BchFoldBlockFn`] and [`fold_block_scalar`].
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
//! [`BchEncodeFns`] bundles the primitives both reductions need, and its
//! entries are the driver a caller runs:
//!
//! - [`absorb_block`](BchEncodeFns::absorb_block) takes one 64-degree window
//!   of each lane's message, bit-slices it through a 64×64 bit-block
//!   transpose, and advances the bit-sliced register over those degrees;
//! - [`unpack_parity`](BchEncodeFns::unpack_parity) reads the reduced slices
//!   back through the same transpose as one packed parity word run per lane;
//! - [`fold_block`](BchEncodeFns::fold_block) advances one frame's packed
//!   remainder over one 64-degree block through carry-less multiplication.
//!
//! The transpose is [`crate::transpose`]'s primitive rather than a second
//! copy of it, and the fold's carry-less multiply is
//! [`crate::x86::clmul`]'s PCLMULQDQ primitive or
//! [`crate::clmul_u64_scalar`], never a third one.
//! [`bitslice_scratch_words`] and [`bitslice_split`] fix the buffer geometry
//! the bit-sliced methods read, and [`fold_barrett_constant`] derives the one
//! constant the fold reads, so a caller sizes every buffer from the
//! redundancy alone and never from the batch length. Every input is a packed
//! `u64` word or a plain size.
//!
//! [`detect`] publishes the accelerated bundle and [`scalar`] the portable
//! one. The two compute the same words, so a caller that runs the scalar
//! bundle on a host that would admit the accelerated one gets identical
//! output; only the instruction count differs.
//!
//! # Required processor features
//!
//! [`detect`] is one combined predicate over the complete feature set the
//! bundle's kernels use, and it publishes the accelerated bundle only when
//! every one of them is present. That set is
//! `avx2 && pclmulqdq && sse4.1`:
//!
//! - the bit-sliced reduction emits `vpbroadcastq`, `vpand`, `vpxor`, and
//!   unaligned 256-bit loads and stores, and the AVX2 transpose lane emits
//!   `vpand`, `vpsllq`, `vpsrlq`, and `vpxor`, so both need `avx2`;
//! - the fold emits `pclmulqdq` for the carry-less product and `pextrq` for
//!   reading its halves back, so it needs `pclmulqdq` and `sse4.1`, the same
//!   pair [`crate::gf2m`] detects for the primitive it shares.
//!
//! No kernel here extracts or deposits bit fields, so none needs `bmi2`. The
//! predicate is the union rather than a per-kernel test because the bundle is
//! published as a whole: a caller holding it may call any entry, so any
//! feature any entry uses is a precondition of holding it at all. The
//! `# Safety` section of every kernel under `src/x86/` that this module
//! publishes names exactly the features that kernel needs, and the predicate
//! is their union, so no kernel is ever reached without its own contract
//! established.
//!
//! # Generated code
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
//! The same artefact carries the fold kernels, and they answer the matching
//! question for the carry-less path. `fold_block_pclmul` shows two
//! `pclmulqdq` instructions, one for the Barrett quotient and one in the
//! product loop, each a `movq` in and a `pextrq` or `movq` out, with no call
//! through a function pointer: the shared step and the primitive both inline
//! into it. [`fold_block_scalar`] shows the same step around
//! [`crate::clmul_u64_scalar`]'s `bsf`/`shld`/`shl`/`cmovne`/`xor` loop, one
//! iteration per set bit of the multiplier. So the portable arm costs about
//! as many iterations per multiply as the generator has coefficients, where
//! the accelerated arm costs one instruction, and it is a fallback rather
//! than a competitor.
//!
//! # Complexity
//!
//! One lane group of [`BITSLICE_LANES`] frames costs $r$ word operations per
//! message degree in [`bitslice_reduce_scalar`] and $\lceil r/4 \rceil$
//! 256-bit operations in the AVX2 kernel, so $O(k r / 64)$ and
//! $O(k r / 256)$ word-equivalents per frame respectively, against the
//! $O(k \lceil r/64 \rceil)$ of a packed per-frame recurrence. A batch below
//! the lane width pays a whole lane group, which is what makes that
//! reduction's admission a batch-length question.
//!
//! The fold costs $1 + \lceil r/64 \rceil$ carry-less multiplies and
//! $O(\lceil r/64 \rceil)$ word operations per 64 message coefficients of
//! one frame, so $O(k (1 + \lceil r/64 \rceil) / 64)$ multiplies per
//! message, whatever the batch length is.
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

/// Carry-less-multiply fold of one 64-degree message block into one frame's
/// packed remainder.
///
/// The arguments are `(register, low, redundancy, barrett, block)`:
///
/// - `register` holds the running remainder $R$ packed little-endian in
///   `redundancy.div_ceil(64)` words, clear above degree $r - 1$. The caller
///   zeroes it before a frame's first block and passes it back unchanged
///   between blocks.
/// - `low` holds the generator's low $r$ coefficients in the same packing and
///   the same number of words; the monic leading coefficient never enters.
/// - `redundancy` is $r$, the generator's degree.
/// - `barrett` is [`fold_barrett_constant`] of the same generator, the low 64
///   coefficients of $\lfloor x^{r+64} / g \rfloor$.
/// - `block` carries 64 message coefficients, bit $j$ holding the coefficient
///   of the block's degree $j$. A block shorter than 64 degrees is zero above
///   its last, which is exact rather than approximate: the recurrence
///   multiplies by $x^{64}$ per block whatever the block holds, so a
///   zero-padded top block contributes nothing while the register is still
///   zero.
///
/// The caller runs the blocks of one frame from the highest degrees down, 64
/// degrees apart, and reads the remainder out of `register`.
pub type BchFoldBlockFn = fn(&mut [u64], &[u64], usize, u64, u64);

/// Bundle of dispatched BCH batch-encoding kernels.
#[derive(Copy, Clone, Debug)]
pub struct BchEncodeFns {
    /// Transposes a 64×64 bit block, both to bit-slice a lane group's message
    /// words and to read the reduced slices back as per-frame parity words.
    pub transpose_lane_block: crate::transpose::Transpose64x64Fn,
    /// Advances the bit-sliced shift register over one block of message
    /// degrees.
    pub bitslice_reduce: BchBitsliceReduceFn,
    /// Folds one 64-degree message block into one frame's packed remainder.
    pub fold_block: BchFoldBlockFn,
    /// Human-readable tag of the chosen bundle, `"avx2-pclmul"` or
    /// `"scalar"`.
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
/// The predicate is the module's
/// [required feature set](self#required-processor-features): one combined
/// detection of `avx2`, `pclmulqdq` and `sse4.1`, the union over every kernel
/// the bundle carries. A caller that receives `None` runs [`scalar`], which
/// needs no processor feature and computes the same words.
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
        fold_block: fold_block_scalar,
        name: "scalar",
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<BchEncodeFns> {
    use std::arch::is_x86_feature_detected;

    // One combined predicate over the union of what every kernel of the
    // bundle uses; see the module-level section on required processor
    // features for which kernel contributes which feature.
    if !(is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("pclmulqdq")
        && is_x86_feature_detected!("sse4.1"))
    {
        return None;
    }
    // The transpose lane is `crate::transpose`'s own dispatch rather than a
    // second wrapper around the same kernel, and it publishes its AVX2 lane
    // under exactly the feature this predicate just established.
    let transpose = crate::transpose::detect()?;
    Some(BchEncodeFns {
        transpose_lane_block: transpose.transpose_64x64,
        bitslice_reduce: bitslice_reduce_avx2_safe,
        fold_block: fold_block_pclmul_safe,
        name: "avx2-pclmul",
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

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn fold_block_pclmul_safe(
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    block: u64,
) {
    // SAFETY: `detect_x86` publishes this pointer only when
    // `is_x86_feature_detected!("pclmulqdq")` and
    // `is_x86_feature_detected!("sse4.1")` both hold, which together are the
    // whole safety condition of `x86::bch_encode::fold_block_pclmul`; the
    // kernel decides the buffer geometry itself.
    unsafe {
        crate::x86::bch_encode::fold_block_pclmul(register, low, redundancy, barrett, block);
    }
}

/// Reads the 64 packed coefficients starting at `offset`, bit $j$ carrying
/// the coefficient of degree `offset + j`.
///
/// Degrees past the buffer read as zero, which is the tail padding a packed
/// buffer maintains.
fn packed_window(words: &[u64], offset: usize) -> u64 {
    let word = offset / 64;
    let shift = offset % 64;
    let low = words.get(word).copied().unwrap_or(0) >> shift;
    let high = if shift == 0 {
        0
    } else {
        words.get(word + 1).copied().unwrap_or(0) << (64 - shift)
    };
    low | high
}

/// The mask keeping a packed remainder of degree below `redundancy` clear
/// above its top coefficient.
fn packed_tail_mask(redundancy: usize) -> u64 {
    if redundancy.is_multiple_of(64) {
        u64::MAX
    } else {
        (1u64 << (redundancy % 64)) - 1
    }
}

/// The fold step of [`BchFoldBlockFn`], over the carry-less multiply `mul`.
///
/// Both arms of the bundle are this function: the accelerated one supplies
/// PCLMULQDQ and the portable one [`crate::clmul_u64_scalar`], so the
/// reduction has one definition and the arms differ only in their multiply.
///
/// With $R$ the remainder so far and $B$ the block entering at $x^r$, one step
/// reduces $T = R x^{64} \oplus B x^{r}$, whose degree is below $r + 64$. Its
/// quotient by $x^r$ is the single word $t = \lfloor T/x^r \rfloor$, which is
/// the 64 coefficients leaving the register exclusive-ored with the block.
/// Barrett gives the quotient by $g$ as
/// $q = t \oplus \mathrm{hi}_{64}(t \cdot \mu_{\text{low}})$, where
/// $\mu = x^{64} \oplus \mu_{\text{low}}$ is [`fold_barrett_constant`]'s
/// $\lfloor x^{r+64}/g \rfloor$: the leading $x^{64}$ contributes $t$ itself
/// and the rest is one carry-less product. Everything at or above $x^r$ then
/// cancels in $T \oplus q g$, so the new remainder is the low $r$
/// coefficients of $T \oplus q \cdot g_{\text{low}}$, one carry-less product
/// per word of `low`.
#[inline(always)]
pub(crate) fn fold_block_over<M>(
    mul: M,
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    block: u64,
) where
    M: Fn(u64, u64) -> u128,
{
    let words = redundancy.div_ceil(64);
    assert!(
        redundancy > 0,
        "a fold needs a generator of positive degree"
    );
    assert_eq!(
        (register.len(), low.len()),
        (words, words),
        "a fold runs over {words} packed words at redundancy {redundancy}"
    );

    // t: the 64 coefficients the register is about to carry past x^r, plus
    // the block entering there. Below 64 coefficients the register cannot
    // fill the word and its top coefficient lands at bit r - 1.
    let leaving = if redundancy >= 64 {
        packed_window(register, redundancy - 64)
    } else {
        register[0] << (64 - redundancy)
    };
    let quotient_input = leaving ^ block;

    // R x^64, keeping the coefficients below x^r: one whole-word shift.
    for index in (1..words).rev() {
        register[index] = register[index - 1];
    }
    register[0] = 0;

    let quotient = quotient_input ^ ((mul(quotient_input, barrett) >> 64) as u64);

    for index in 0..words {
        let product = mul(quotient, low[index]);
        register[index] ^= product as u64;
        if index + 1 < words {
            register[index + 1] ^= (product >> 64) as u64;
        }
    }
    register[words - 1] &= packed_tail_mask(redundancy);
}

/// Portable carry-less-multiply fold of one 64-degree message block.
///
/// This is the [`BchFoldBlockFn`] contract with no processor feature
/// requirement, over [`crate::clmul_u64_scalar`], and the kernel the
/// accelerated one is checked against.
///
/// # Panics
///
/// Panics when `register` and `low` are not both `redundancy.div_ceil(64)`
/// words, or when `redundancy` is zero.
///
/// # Complexity
///
/// $1 + \lceil r/64 \rceil$ carry-less multiplies, each
/// $O(\mathrm{popcount})$ of its multiplier here, plus
/// $O(\lceil r/64 \rceil)$ word operations, for 64 message coefficients of
/// one frame.
pub fn fold_block_scalar(
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    block: u64,
) {
    fold_block_over(
        crate::clmul_scalar::clmul_u64_scalar,
        register,
        low,
        redundancy,
        barrett,
        block,
    );
}

/// The Barrett constant [`BchFoldBlockFn`] reduces through: the low 64
/// coefficients of $\mu = \lfloor x^{r+64} / g \rfloor$.
///
/// $\mu$ has degree exactly 64, so its leading coefficient is one and is
/// implied rather than returned; the fold adds it back as the identity term
/// of its quotient.
///
/// `low` holds the generator's low $r$ coefficients packed little-endian in
/// $\lceil r/64 \rceil$ words. `scratch` is a buffer of the same length that
/// this function clobbers and the caller may reuse for anything afterwards.
///
/// The division runs the same shift-and-feedback recurrence a bit-serial
/// encoder does: $x^{r} \equiv g_{\text{low}}$ starts it, and each of the 64
/// steps multiplies the running remainder by $x$ while shifting the
/// coefficient that leaves it into the quotient.
///
/// # Panics
///
/// Panics when `redundancy` is zero, or when `low` or `scratch` is not
/// `redundancy.div_ceil(64)` words.
///
/// # Complexity
///
/// $O(\lceil r/64 \rceil)$ word operations per step over 64 steps, paid once
/// per generator rather than per message.
#[must_use]
pub fn fold_barrett_constant(low: &[u64], redundancy: usize, scratch: &mut [u64]) -> u64 {
    let words = redundancy.div_ceil(64);
    assert!(
        redundancy > 0,
        "a Barrett constant needs a generator of positive degree"
    );
    assert_eq!(
        (low.len(), scratch.len()),
        (words, words),
        "a Barrett constant runs over {words} packed words at redundancy {redundancy}"
    );

    let top = redundancy - 1;
    let tail = packed_tail_mask(redundancy);
    // The remainder of x^r is the generator's low coefficients, and its
    // quotient is one; 64 further steps carry both to x^{r+64}.
    scratch.copy_from_slice(low);
    let mut constant = 1u64;
    for _ in 0..64 {
        let leaving = (scratch[top / 64] >> (top % 64)) & 1;
        for index in (1..words).rev() {
            scratch[index] = (scratch[index] << 1) | (scratch[index - 1] >> 63);
        }
        scratch[0] <<= 1;
        scratch[words - 1] &= tail;
        if leaving == 1 {
            for (slot, coefficient) in scratch.iter_mut().zip(low) {
                *slot ^= *coefficient;
            }
        }
        // The leading one leaves this word on the last step, which is what
        // makes the result the low half of a degree-64 quotient.
        constant = (constant << 1) | leaving;
    }
    constant
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
                    *window = packed_window(message, degree);
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

    /// [`detect`] publishes the accelerated bundle exactly under the module's
    /// combined predicate, so dropping a feature from it fails here rather
    /// than reaching a kernel whose instructions the host lacks.
    #[test]
    fn the_accelerated_bundle_is_published_only_under_its_whole_feature_set() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let complete = {
            use std::arch::is_x86_feature_detected;
            is_x86_feature_detected!("avx2")
                && is_x86_feature_detected!("pclmulqdq")
                && is_x86_feature_detected!("sse4.1")
        };
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let complete = false;

        assert_eq!(
            detect().map(|fns| fns.name),
            complete.then_some("avx2-pclmul"),
            "the bundle is published exactly under `avx2 && pclmulqdq && sse4.1`"
        );
        assert_eq!(
            scalar().name,
            "scalar",
            "the portable bundle needs no processor feature"
        );
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

    /// Drives `fns`'s fold over one frame: 64-degree blocks from the highest
    /// degrees down, the top one zero-padded.
    fn fold_parity(
        fns: &BchEncodeFns,
        message: &[u64],
        dimension: usize,
        low: &[u64],
        redundancy: usize,
    ) -> Vec<bool> {
        let words = redundancy.div_ceil(64);
        let mut scratch = vec![0u64; words];
        let barrett = fold_barrett_constant(low, redundancy, &mut scratch);
        let mut register = vec![0u64; words];
        let mut degree = dimension;
        while degree > 0 {
            let count = ((degree - 1) % 64) + 1;
            degree -= count;
            let block = packed_window(message, degree) & mask_below(count);
            (fns.fold_block)(&mut register, low, redundancy, barrett, block);
        }
        (0..redundancy).map(|index| bit(&register, index)).collect()
    }

    fn mask_below(count: usize) -> u64 {
        if count == 64 {
            u64::MAX
        } else {
            (1u64 << count) - 1
        }
    }

    fn check_fold(fns: &BchEncodeFns) {
        let mut seeded = Seeded(0x2b69_68d3_0000_0004);
        for &dimension in DIMENSIONS {
            for &redundancy in REDUNDANCIES {
                let low = seeded.bits(redundancy);
                for &batch in BATCHES {
                    for _ in 0..batch {
                        let message = seeded.bits(dimension);
                        assert_eq!(
                            fold_parity(fns, &message, dimension, &low, redundancy),
                            naive_parity(&message, dimension, &low, redundancy),
                            "{} diverged at k={dimension} r={redundancy} B={batch}",
                            fns.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_scalar_fold_matches_the_per_frame_reference() {
        check_fold(&scalar());
    }

    #[test]
    fn the_detected_fold_matches_the_per_frame_reference() {
        let Some(fns) = detect() else {
            return;
        };
        check_fold(&fns);
    }

    #[test]
    fn the_detected_fold_matches_the_scalar_fold_word_for_word() {
        let Some(fns) = detect() else {
            return;
        };
        let mut seeded = Seeded(0x2b69_68d3_0000_0005);
        for &redundancy in REDUNDANCIES {
            let words = redundancy.div_ceil(64);
            let low = seeded.bits(redundancy);
            let mut scratch = vec![0u64; words];
            let barrett = fold_barrett_constant(&low, redundancy, &mut scratch);
            let mut expected = vec![0u64; words];
            let mut produced = vec![0u64; words];
            for _ in 0..16 {
                let block = seeded.next();
                fold_block_scalar(&mut expected, &low, redundancy, barrett, block);
                (fns.fold_block)(&mut produced, &low, redundancy, barrett, block);
                assert_eq!(
                    produced, expected,
                    "{} diverged at r={redundancy}",
                    fns.name
                );
            }
        }
    }

    #[test]
    fn the_barrett_constant_is_the_quotient_of_the_shifted_monomial() {
        // mu * g + rho = x^{r+64} with deg rho < r, checked by multiplying the
        // reported quotient back out one word at a time.
        let mut seeded = Seeded(0x2b69_68d3_0000_0006);
        for &redundancy in REDUNDANCIES {
            let words = redundancy.div_ceil(64);
            let low = seeded.bits(redundancy);
            let mut scratch = vec![0u64; words];
            let barrett = fold_barrett_constant(&low, redundancy, &mut scratch);

            // mu = x^64 + barrett, g = x^r + low. Accumulate mu * g over
            // enough words to hold degree r + 64, then check it equals
            // x^{r+64} above degree r - 1.
            let mut product = vec![0u64; words + 2];
            xor_shifted(&mut product, &[barrett], redundancy); // barrett * x^r
            set_bit(&mut product, redundancy + 64); // x^64 * x^r
            for (index, &word) in low.iter().enumerate() {
                let piece = crate::clmul_u64_scalar(barrett, word);
                xor_shifted(&mut product, &[piece as u64], 64 * index);
                xor_shifted(&mut product, &[(piece >> 64) as u64], 64 * (index + 1));
                // x^64 * low word
                xor_shifted(&mut product, &[word], 64 * index + 64);
            }
            for index in redundancy..(words + 2) * 64 {
                assert_eq!(
                    bit(&product, index),
                    index == redundancy + 64,
                    "r={redundancy}: mu * g differs from x^{{r+64}} at degree {index}"
                );
            }
        }
    }

    /// Exclusive-ors `addend`, read as packed coefficients, into `target` at
    /// degree `offset`.
    fn xor_shifted(target: &mut [u64], addend: &[u64], offset: usize) {
        for (index, &word) in addend.iter().enumerate() {
            for bit_index in 0..64 {
                if (word >> bit_index) & 1 == 1 {
                    set_bit(target, offset + 64 * index + bit_index);
                }
            }
        }
    }

    fn set_bit(target: &mut [u64], index: usize) {
        target[index / 64] ^= 1u64 << (index % 64);
    }

    #[test]
    #[should_panic(expected = "positive degree")]
    fn a_fold_without_a_generator_is_rejected() {
        fold_block_scalar(&mut [], &[], 0, 0, 1);
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
