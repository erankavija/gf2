//! Batch-encoding kernels for binary BCH codes: one dispatch bundle carries two
//! independent reductions of $x^r m(x)$ modulo the generator $g$. The
//! carry-less-multiply fold reduces one frame 64 message coefficients at a time,
//! either into the remainder block by block ([`BchFoldBlocksFn`]) or in chunks
//! of independent products into an unreduced accumulator
//! ([`BchFoldChunksFn`]), and the bit-sliced reduction advances
//! [`BITSLICE_LANES`] frames per step.
//!
//! # Bit-sliced register
//!
//! The bit-sliced reduction holds the shift register of $r$ binary coefficients
//! one word per coefficient, bit $l$ carrying lane $l$'s value. With $s_j$ the
//! slice of register coefficient $j$, $\mu_j$ the all-ones-or-zero broadcast of
//! the generator's coefficient of $x^j$, $m$ the slice of the entering message
//! coefficient and $s_{-1}$ read as zero, a step computes
//! $f = s_{r-1} \oplus m$ and $s'_j = s_{j-1} \oplus (\mu_j \wedge f)$.
//!
//! # Required processor features
//!
//! [`detect`] publishes the accelerated bundle only when
//! `avx2 && pclmulqdq && sse4.1 && ssse3` holds. The predicate is the union over every
//! kernel of the bundle, because a caller holding the bundle may call any
//! entry: the bit-sliced reduction and the AVX2 transpose lane need `avx2`,
//! the fold needs `pclmulqdq` and `sse4.1`, and the chunk fold `ssse3` beside
//! them. [`scalar`] computes the same
//! words with no processor feature.
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

/// Carry-less-multiply fold of a run of 64-degree message blocks into one
/// frame's packed remainder.
///
/// The arguments are `(register, low, redundancy, barrett, blocks)`:
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
/// - `blocks` are consecutive blocks of the frame, the highest degrees first
///   and 64 degrees apart, each carrying 64 message coefficients, bit $j$
///   holding the coefficient of the block's degree $j$. A block shorter than
///   64 degrees is zero above its last, which is exact rather than
///   approximate: the recurrence multiplies by $x^{64}$ per block whatever
///   the block holds, so a zero-padded top block contributes nothing while
///   the register is still zero.
///
/// The caller passes a frame's blocks in one call or in consecutive runs and
/// reads the remainder out of `register`. A run keeps the register in
/// machine registers for the register widths [`FOLD_UNROLLED_WORDS`] bounds.
pub type BchFoldBlocksFn = fn(&mut [u64], &[u64], usize, u64, &[u64]);

/// Register words up to which a fold run keeps the remainder in machine
/// registers, unrolled by width; a wider remainder folds one block at a time
/// through memory. Four words cover every generator of degree up to 256.
pub const FOLD_UNROLLED_WORDS: usize = 4;

/// Independent carry-less-multiply fold of whole chunks of 64-degree message
/// blocks into an unreduced accumulator.
///
/// The arguments are `(state, constants, blocks)`:
///
/// - `state` holds the accumulator $X = \sum_i X_i x^{64 i}$ in $L$ words,
///   where $L$ is its length, [`fold_accumulators`] by default. A frame's
///   fold starts from zero; the accumulator is congruent modulo the generator
///   to the polynomial of the blocks consumed so far, and
///   [`fold_accumulator_finish`] reduces it.
/// - `constants` is [`fold_chunk_constants`] of the generator for $L$
///   accumulators: $L$ runs of $W$ packed words, run $i$ holding
///   $x^{64(L+i)} \bmod g$, with $W$ the remainder's word count, from one up
///   to [`FOLD_UNROLLED_WORDS`].
/// - `blocks` are consecutive blocks of the frame, the highest degrees first
///   and 64 degrees apart, in whole chunks of $L$. A caller whose block count
///   is not a multiple of $L$ passes zero blocks above the frame's top block,
///   which leave the accumulator unchanged.
///
/// One chunk $C$ computes $X' = C \oplus \sum_i X_i \cdot k_i$. The $L W$
/// products of a chunk depend only on the accumulator before it, so they
/// issue together, and the chain from one chunk to the next is one multiply
/// deep: $L$ at [`fold_accumulators`] keeps the multiplier's pipeline full.
/// Each product has degree below $r + 64 \le 64 L$, so $L > W$ is what lets
/// the accumulator hold it unreduced.
pub type BchFoldChunksFn = fn(&mut [u64], &[u64], &[u64]);

/// The accumulator counts [`BchFoldChunksFn`] accepts at most.
pub const FOLD_MAX_ACCUMULATORS: usize = 16;

/// Carry-less multiplies the scalar fold arm keeps in flight: its multiply is
/// a dependent loop, which gains nothing from more independent ones than one.
pub const FOLD_SCALAR_IN_FLIGHT: usize = 1;

/// Carry-less multiplies the PCLMULQDQ arm needs in flight to cover the
/// instruction's latency: its latency in cycles times its issue rate per
/// cycle, at the deepest of the x86 cores uops.info tabulates (Skylake: 7
/// cycles at one per cycle; Ice Lake 6 at one).
pub const FOLD_PCLMUL_IN_FLIGHT: usize = 7;

/// The accumulator count [`BchFoldChunksFn`] defaults to at `redundancy`, for
/// a multiplier that needs `in_flight` independent products to stay busy.
///
/// It is the larger of $W + 1$, the fewest that hold a product unreduced, and
/// $\lceil \text{in\_flight} / W \rceil$, the fewest whose $L W$ products per
/// chunk fill the multiplier's pipeline, with $W$ =
/// `redundancy.div_ceil(64)`. `in_flight` is the bundle's
/// [`BchEncodeFns::fold_in_flight`].
///
/// # Panics
///
/// Panics when `redundancy` is zero or the count exceeds
/// [`FOLD_MAX_ACCUMULATORS`].
#[must_use]
pub fn fold_accumulators(redundancy: usize, in_flight: usize) -> usize {
    assert!(
        redundancy > 0,
        "a fold needs a generator of positive degree"
    );
    let words = redundancy.div_ceil(64);
    let count = (words + 1).max(in_flight.div_ceil(words));
    assert!(
        count <= FOLD_MAX_ACCUMULATORS,
        "{count} accumulators exceed {FOLD_MAX_ACCUMULATORS}"
    );
    count
}

/// Writes the constants [`BchFoldChunksFn`] multiplies by for `accumulators`
/// accumulators: run $i$ of `constants` gets $x^{64(L+i)} \bmod g$ in
/// `redundancy.div_ceil(64)` packed words, clear above degree $r - 1$.
///
/// `low` holds the generator's low $r$ coefficients packed little-endian.
///
/// # Panics
///
/// Panics when `redundancy` is zero or above `64 * FOLD_UNROLLED_WORDS`,
/// when `accumulators` is not above the word count or above
/// [`FOLD_MAX_ACCUMULATORS`], or when `low` and `constants` are not the
/// word count and `accumulators` times it.
///
/// # Complexity
///
/// $O(\lceil r/64 \rceil)$ word operations per degree up to $x^{64(2L-1)}$,
/// paid once per generator.
pub fn fold_chunk_constants(
    low: &[u64],
    redundancy: usize,
    accumulators: usize,
    constants: &mut [u64],
) {
    let words = redundancy.div_ceil(64);
    assert!(
        (1..=FOLD_UNROLLED_WORDS).contains(&words),
        "a chunk fold runs over 1 to {FOLD_UNROLLED_WORDS} remainder words, not {words}"
    );
    assert!(
        accumulators > words && accumulators <= FOLD_MAX_ACCUMULATORS,
        "{accumulators} accumulators cannot hold a product of {words} words"
    );
    assert_eq!(
        (low.len(), constants.len()),
        (words, accumulators * words),
        "the constants are {accumulators} runs of {words} words"
    );
    let top = redundancy - 1;
    let tail = packed_tail_mask(redundancy);
    let mut running = [0u64; FOLD_UNROLLED_WORDS];
    let running = &mut running[..words];
    // x^r is the generator's low coefficients; each step multiplies by x.
    running.copy_from_slice(low);
    let mut degree = redundancy;
    for (index, constant) in constants.chunks_exact_mut(words).enumerate() {
        let target = 64 * (accumulators + index);
        while degree < target {
            let leaving = (running[top / 64] >> (top % 64)) & 1;
            for word in (1..words).rev() {
                running[word] = (running[word] << 1) | (running[word - 1] >> 63);
            }
            running[0] <<= 1;
            running[words - 1] &= tail;
            if leaving == 1 {
                for (slot, coefficient) in running.iter_mut().zip(low) {
                    *slot ^= *coefficient;
                }
            }
            degree += 1;
        }
        constant.copy_from_slice(running);
    }
}

/// Reduces a [`BchFoldChunksFn`] accumulator into the remainder of $x^r X$
/// modulo the generator, through the bundle's `fold_blocks`: the accumulator's
/// words enter as blocks, highest first, into a zeroed `register`.
///
/// `register`, `low`, `redundancy` and `barrett` are the
/// [`BchFoldBlocksFn`] arguments.
///
/// # Panics
///
/// Panics as `fold_blocks` does.
pub fn fold_accumulator_finish(
    fns: &BchEncodeFns,
    state: &[u64],
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
) {
    register.fill(0);
    let mut blocks = [0u64; FOLD_MAX_ACCUMULATORS];
    let blocks = &mut blocks[..state.len()];
    for (block, word) in blocks.iter_mut().zip(state.iter().rev()) {
        *block = *word;
    }
    // Leading zero words of the accumulator leave a zero register unchanged.
    let start = blocks
        .iter()
        .position(|&block| block != 0)
        .unwrap_or(blocks.len());
    (fns.fold_blocks)(register, low, redundancy, barrett, &blocks[start..]);
}

/// Bundle of dispatched BCH batch-encoding kernels.
#[derive(Copy, Clone, Debug)]
pub struct BchEncodeFns {
    /// Transposes a 64×64 bit block, both to bit-slice a lane group's message
    /// words and to read the reduced slices back as per-frame parity words.
    pub transpose_lane_block: crate::transpose::Transpose64x64Fn,
    /// Advances the bit-sliced shift register over one block of message
    /// degrees.
    pub bitslice_reduce: BchBitsliceReduceFn,
    /// Folds a run of 64-degree message blocks into one frame's packed
    /// remainder.
    pub fold_blocks: BchFoldBlocksFn,
    /// Folds whole chunks of message blocks into an unreduced accumulator.
    pub fold_chunks: BchFoldChunksFn,
    /// Independent carry-less multiplies `fold_chunks` needs in flight to
    /// keep its multiplier busy, which [`fold_accumulators`] reads.
    pub fold_in_flight: usize,
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
/// [required feature set](self#required-processor-features). A caller that
/// receives `None` runs [`scalar`], which computes the same words.
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
        fold_blocks: fold_blocks_scalar,
        fold_chunks: fold_chunks_scalar,
        fold_in_flight: FOLD_SCALAR_IN_FLIGHT,
        name: "scalar",
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<BchEncodeFns> {
    use std::arch::is_x86_feature_detected;

    // The union of what every kernel of the bundle uses.
    if !(is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("pclmulqdq")
        && is_x86_feature_detected!("sse4.1")
        && is_x86_feature_detected!("ssse3"))
    {
        return None;
    }
    // `crate::transpose`'s own dispatch publishes its AVX2 lane under the
    // feature this predicate just established.
    let transpose = crate::transpose::detect()?;
    Some(BchEncodeFns {
        transpose_lane_block: transpose.transpose_64x64,
        bitslice_reduce: bitslice_reduce_avx2_safe,
        fold_blocks: fold_blocks_pclmul_safe,
        fold_chunks: fold_chunks_pclmul_safe,
        fold_in_flight: FOLD_PCLMUL_IN_FLIGHT,
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
fn fold_blocks_pclmul_safe(
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    blocks: &[u64],
) {
    // SAFETY: `detect_x86` publishes this pointer only when
    // `is_x86_feature_detected!("pclmulqdq")` and
    // `is_x86_feature_detected!("sse4.1")` both hold, which together are the
    // whole safety condition of `x86::bch_encode::fold_blocks_pclmul`; the
    // kernel decides the buffer geometry itself.
    unsafe {
        crate::x86::bch_encode::fold_blocks_pclmul(register, low, redundancy, barrett, blocks);
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn fold_chunks_pclmul_safe(state: &mut [u64], constants: &[u64], blocks: &[u64]) {
    // SAFETY: `detect_x86` publishes this pointer only when
    // `is_x86_feature_detected!("pclmulqdq")` and
    // `is_x86_feature_detected!("sse4.1")` and `is_x86_feature_detected!("ssse3")`
    // hold, which together are the whole safety condition of
    // `x86::bch_encode::fold_chunks_pclmul`; the
    // kernel decides the buffer geometry itself.
    unsafe { crate::x86::bch_encode::fold_chunks_pclmul(state, constants, blocks) }
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

/// The fold step of [`BchFoldBlocksFn`] for one block, over the carry-less
/// multiply `mul`.
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

/// [`BchFoldBlocksFn`] over the carry-less multiply `mul`: the run unrolled by
/// register width up to [`FOLD_UNROLLED_WORDS`], one [`fold_block_over`] step
/// per block beyond it.
#[inline(always)]
pub(crate) fn fold_blocks_over<M>(
    mul: M,
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    blocks: &[u64],
) where
    M: Fn(u64, u64) -> u128,
{
    match redundancy.div_ceil(64) {
        1 => fold_blocks_words::<M, 1>(mul, register, low, redundancy, barrett, blocks),
        2 => fold_blocks_words::<M, 2>(mul, register, low, redundancy, barrett, blocks),
        3 => fold_blocks_words::<M, 3>(mul, register, low, redundancy, barrett, blocks),
        4 => fold_blocks_words::<M, 4>(mul, register, low, redundancy, barrett, blocks),
        _ => {
            for &block in blocks {
                fold_block_over(&mul, register, low, redundancy, barrett, block);
            }
        }
    }
}

/// [`fold_block_over`] over a run of blocks for a remainder of `W` words,
/// held in a local array across the run.
#[inline(always)]
fn fold_blocks_words<M, const W: usize>(
    mul: M,
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    blocks: &[u64],
) where
    M: Fn(u64, u64) -> u128,
{
    assert!(
        redundancy > 0,
        "a fold needs a generator of positive degree"
    );
    assert_eq!(
        (register.len(), low.len()),
        (W, W),
        "a fold runs over {W} packed words at redundancy {redundancy}"
    );
    let low: [u64; W] = low.try_into().expect("checked above");
    let mut reg: [u64; W] = (&*register).try_into().expect("checked above");
    let tail = packed_tail_mask(redundancy);
    // r lies in (64 (W - 1), 64 W], so the 64 coefficients leaving the
    // register, from r - 64 up, sit in its top two words: constant indices
    // keep the array in machine registers.
    let shift = redundancy % 64;
    for &block in blocks {
        let leaving = if shift == 0 {
            reg[W - 1]
        } else if W == 1 {
            reg[0] << (64 - shift)
        } else {
            (reg[W.saturating_sub(2)] >> shift) | (reg[W - 1] << (64 - shift))
        };
        let quotient_input = leaving ^ block;
        let quotient = quotient_input ^ ((mul(quotient_input, barrett) >> 64) as u64);
        let products: [u128; W] = core::array::from_fn(|index| mul(quotient, low[index]));
        // R x^64 + q g_low, built whole rather than shifted in place: an
        // in-place shift compiles to an overlapping copy whose reload misses
        // store forwarding once per block.
        let previous = reg;
        reg = core::array::from_fn(|index| {
            let shifted = if index == 0 { 0 } else { previous[index - 1] };
            let carry = if index == 0 {
                0
            } else {
                (products[index - 1] >> 64) as u64
            };
            shifted ^ products[index] as u64 ^ carry
        });
        reg[W - 1] &= tail;
    }
    register.copy_from_slice(&reg);
}

/// Portable carry-less-multiply fold of a run of 64-degree message blocks.
///
/// This is the [`BchFoldBlocksFn`] contract with no processor feature
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
/// $O(\lceil r/64 \rceil)$ word operations, per block.
pub fn fold_blocks_scalar(
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    blocks: &[u64],
) {
    fold_blocks_over(
        crate::clmul_scalar::clmul_u64_scalar,
        register,
        low,
        redundancy,
        barrett,
        blocks,
    );
}

/// [`BchFoldChunksFn`] over the carry-less multiply `mul`, unrolled by
/// remainder width.
///
/// # Panics
///
/// Panics when the constants are not one run of 1 to
/// [`FOLD_UNROLLED_WORDS`] words per accumulator, and as
/// [`fold_chunks_words`] does.
#[inline(always)]
pub(crate) fn fold_chunks_over<M>(mul: M, state: &mut [u64], constants: &[u64], blocks: &[u64])
where
    M: Fn(u64, u64) -> u128,
{
    let accumulators = state.len();
    assert!(
        accumulators > 0 && constants.len().is_multiple_of(accumulators),
        "the constants are one run per accumulator"
    );
    match constants.len() / accumulators {
        1 => fold_chunks_words::<M, 1>(mul, state, constants, blocks),
        2 => fold_chunks_words::<M, 2>(mul, state, constants, blocks),
        3 => fold_chunks_words::<M, 3>(mul, state, constants, blocks),
        4 => fold_chunks_words::<M, 4>(mul, state, constants, blocks),
        words => panic!("a chunk fold runs over 1 to {FOLD_UNROLLED_WORDS} words, not {words}"),
    }
}

/// [`BchFoldChunksFn`] for constants of `W` words.
///
/// # Panics
///
/// Panics when the accumulator count is not above `W` or above
/// [`FOLD_MAX_ACCUMULATORS`], or when `blocks` is not whole chunks.
#[inline(always)]
fn fold_chunks_words<M, const W: usize>(
    mul: M,
    state: &mut [u64],
    constants: &[u64],
    blocks: &[u64],
) where
    M: Fn(u64, u64) -> u128,
{
    let accumulators = state.len();
    assert!(
        accumulators > W && accumulators <= FOLD_MAX_ACCUMULATORS,
        "{accumulators} accumulators cannot hold a product of {W} words"
    );
    assert!(
        blocks.len().is_multiple_of(accumulators),
        "{} blocks are not whole chunks of {accumulators}",
        blocks.len()
    );
    let mut x = [0u64; FOLD_MAX_ACCUMULATORS];
    x[..accumulators].copy_from_slice(state);
    for chunk in blocks.chunks_exact(accumulators) {
        let mut next = [0u64; FOLD_MAX_ACCUMULATORS];
        for (word, &block) in next[..accumulators].iter_mut().rev().zip(chunk) {
            *word = block;
        }
        for (index, constant) in constants.chunks_exact(W).enumerate() {
            let entering = x[index];
            for (word, &k) in constant.iter().enumerate() {
                let product = mul(entering, k);
                next[word] ^= product as u64;
                next[word + 1] ^= (product >> 64) as u64;
            }
        }
        x = next;
    }
    state.copy_from_slice(&x[..accumulators]);
}

/// Portable [`BchFoldChunksFn`] over [`crate::clmul_u64_scalar`], the kernel
/// the accelerated one is checked against.
///
/// # Panics
///
/// Panics when the constants are not one run of 1 to
/// [`FOLD_UNROLLED_WORDS`] words per accumulator, when the accumulator count
/// is not above that word count or above [`FOLD_MAX_ACCUMULATORS`], or when
/// `blocks` is not whole chunks.
///
/// # Complexity
///
/// $W$ carry-less multiplies per block.
pub fn fold_chunks_scalar(state: &mut [u64], constants: &[u64], blocks: &[u64]) {
    fold_chunks_over(
        crate::clmul_scalar::clmul_u64_scalar,
        state,
        constants,
        blocks,
    );
}

/// The Barrett constant [`BchFoldBlocksFn`] reduces through: the low 64
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

    /// SplitMix64 (`@/citation/Steele2014`).
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
        check_bundle_over(fns, DIMENSIONS, REDUNDANCIES, BATCHES);
    }

    fn check_bundle_over(
        fns: &BchEncodeFns,
        dimensions: &[usize],
        redundancies: &[usize],
        batches: &[usize],
    ) {
        let mut seeded = Seeded(0x2b69_68d3_0000_0001);
        for &dimension in dimensions {
            for &redundancy in redundancies {
                let low = seeded.bits(redundancy);
                for &batch in batches {
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

    /// The bundle with one named lane of [`crate::transpose`]'s family
    /// substituted for the transpose primitive, so the bit-slicing contract
    /// runs over every lane instead of only the dispatched one.
    fn bundle_with_lane(lane: crate::transpose::TransposeLane) -> Option<BchEncodeFns> {
        crate::transpose::lane(lane).map(|transpose_lane_block| BchEncodeFns {
            transpose_lane_block,
            name: lane.name(),
            ..scalar()
        })
    }

    /// Every lane answers the bit-slicing round trip, not just the block
    /// contract: a message batch slices through `absorb_block` and comes back
    /// through `unpack_parity` as the per-frame reference's parity.
    ///
    /// The grid is trimmed to the boundaries that exercise a partial lane
    /// group, a partial degree block and a partial parity word, because the
    /// full grid runs once per lane here.
    #[test]
    fn every_transpose_lane_bit_slices_and_unpacks_the_same_parity() {
        const LANE_DIMENSIONS: &[usize] = &[0, 1, 63, 64, 65];
        const LANE_REDUNDANCIES: &[usize] = &[1, 63, 64, 65];
        const LANE_BATCHES: &[usize] = &[0, 1, 63, 64, 65];
        let mut ran = 0usize;
        for lane in crate::transpose::TransposeLane::ALL {
            let Some(fns) = bundle_with_lane(lane) else {
                continue;
            };
            check_bundle_over(&fns, LANE_DIMENSIONS, LANE_REDUNDANCIES, LANE_BATCHES);
            ran += 1;
        }
        assert!(ran > 0, "the scalar lane is available on every host");
    }

    #[test]
    fn the_scalar_bundle_matches_the_per_frame_reference() {
        check_bundle(&scalar());
    }

    #[test]
    fn the_detected_bundle_matches_the_per_frame_reference() {
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
        check_bundle(&fns);
    }

    #[test]
    fn the_detected_bundle_matches_the_scalar_bundle_word_for_word() {
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
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
    /// degrees down, the top one zero-padded, in runs of `run` blocks.
    fn fold_parity(
        fns: &BchEncodeFns,
        message: &[u64],
        dimension: usize,
        low: &[u64],
        redundancy: usize,
        run: usize,
    ) -> Vec<bool> {
        let words = redundancy.div_ceil(64);
        let mut scratch = vec![0u64; words];
        let barrett = fold_barrett_constant(low, redundancy, &mut scratch);
        let mut register = vec![0u64; words];
        let mut blocks = Vec::new();
        let mut degree = dimension;
        while degree > 0 {
            let count = ((degree - 1) % 64) + 1;
            degree -= count;
            blocks.push(packed_window(message, degree) & mask_below(count));
        }
        for chunk in blocks.chunks(run) {
            (fns.fold_blocks)(&mut register, low, redundancy, barrett, chunk);
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
                    for frame in 0..batch {
                        let message = seeded.bits(dimension);
                        let run = 1 + frame % 3;
                        assert_eq!(
                            fold_parity(fns, &message, dimension, &low, redundancy, run),
                            naive_parity(&message, dimension, &low, redundancy),
                            "{} diverged at k={dimension} r={redundancy} B={batch} run={run}",
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
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
        check_fold(&fns);
    }

    #[test]
    fn the_detected_fold_matches_the_scalar_fold_word_for_word() {
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
        let mut seeded = Seeded(0x2b69_68d3_0000_0005);
        // 300 is past the unrolled widths, the one-block-at-a-time arm.
        for &redundancy in REDUNDANCIES.iter().chain(&[255, 256, 257, 300]) {
            let words = redundancy.div_ceil(64);
            let low = seeded.bits(redundancy);
            let mut scratch = vec![0u64; words];
            let barrett = fold_barrett_constant(&low, redundancy, &mut scratch);
            let mut expected = vec![0u64; words];
            let mut produced = vec![0u64; words];
            let blocks: Vec<u64> = (0..16).map(|_| seeded.next()).collect();
            for &block in &blocks {
                fold_blocks_scalar(&mut expected, &low, redundancy, barrett, &[block]);
            }
            (fns.fold_blocks)(&mut produced, &low, redundancy, barrett, &blocks);
            assert_eq!(
                produced, expected,
                "{} diverged at r={redundancy}",
                fns.name
            );
        }
    }

    /// Drives `fns`'s chunk fold over one frame with `accumulators`
    /// accumulators: blocks from the highest degrees down, the top one
    /// zero-padded, zero blocks above it to a whole chunk, in runs of `runs`
    /// chunks, then the accumulator's reduction.
    fn chunk_fold_parity(
        fns: &BchEncodeFns,
        message: &[u64],
        dimension: usize,
        low: &[u64],
        redundancy: usize,
        accumulators: usize,
        runs: usize,
    ) -> Vec<bool> {
        let words = redundancy.div_ceil(64);
        let mut scratch = vec![0u64; words];
        let barrett = fold_barrett_constant(low, redundancy, &mut scratch);
        let mut constants = vec![0u64; accumulators * words];
        fold_chunk_constants(low, redundancy, accumulators, &mut constants);
        let count = dimension.div_ceil(64);
        let mut blocks = vec![0u64; (accumulators - count % accumulators) % accumulators];
        let mut degree = dimension;
        while degree > 0 {
            let width = ((degree - 1) % 64) + 1;
            degree -= width;
            blocks.push(packed_window(message, degree) & mask_below(width));
        }
        let mut state = vec![0u64; accumulators];
        for run in blocks.chunks(runs * accumulators) {
            (fns.fold_chunks)(&mut state, &constants, run);
        }
        let mut register = vec![0u64; words];
        fold_accumulator_finish(fns, &state, &mut register, low, redundancy, barrett);
        (0..redundancy).map(|index| bit(&register, index)).collect()
    }

    /// Redundancies of the chunk fold, to its widest remainder.
    const CHUNK_REDUNDANCIES: &[usize] = &[1, 27, 63, 64, 65, 168, 192, 255, 256];

    fn check_chunk_fold(fns: &BchEncodeFns) {
        let mut seeded = Seeded(0x2b69_68d3_0000_0007);
        for &redundancy in CHUNK_REDUNDANCIES {
            let words = redundancy.div_ceil(64);
            let defaults = [
                fold_accumulators(redundancy, FOLD_SCALAR_IN_FLIGHT),
                fold_accumulators(redundancy, FOLD_PCLMUL_IN_FLIGHT),
            ];
            // The fewest that hold a product, the defaults, one past eight
            // (beyond the counts the accelerated arm unrolls) and the most.
            for accumulators in [
                words + 1,
                defaults[0],
                defaults[1],
                9,
                FOLD_MAX_ACCUMULATORS,
            ] {
                let low = seeded.bits(redundancy);
                // Dimensions 0, 1, 63, 64 and 65 put the top block at every
                // word boundary; 1000 spans several chunks at every count.
                for &dimension in &[0usize, 1, 63, 64, 65, 200, 1000] {
                    let message = seeded.bits(dimension);
                    let runs = 1 + dimension % 3;
                    assert_eq!(
                        chunk_fold_parity(
                            fns,
                            &message,
                            dimension,
                            &low,
                            redundancy,
                            accumulators,
                            runs
                        ),
                        naive_parity(&message, dimension, &low, redundancy),
                        "{} diverged at k={dimension} r={redundancy} L={accumulators}",
                        fns.name
                    );
                }
            }
        }
    }

    #[test]
    fn the_scalar_chunk_fold_matches_the_per_frame_reference() {
        check_chunk_fold(&scalar());
    }

    #[test]
    fn the_detected_chunk_fold_matches_the_per_frame_reference() {
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
        check_chunk_fold(&fns);
    }

    #[test]
    fn the_detected_chunk_fold_matches_the_scalar_chunk_fold_word_for_word() {
        let fns = crate::contract::require_detected("bch_encode::detect()", detect());
        let mut seeded = Seeded(0x2b69_68d3_0000_0008);
        for &redundancy in CHUNK_REDUNDANCIES {
            let words = redundancy.div_ceil(64);
            for accumulators in words + 1..=FOLD_MAX_ACCUMULATORS {
                let low = seeded.bits(redundancy);
                let mut constants = vec![0u64; accumulators * words];
                fold_chunk_constants(&low, redundancy, accumulators, &mut constants);
                let blocks: Vec<u64> = (0..3 * accumulators).map(|_| seeded.next()).collect();
                let start: Vec<u64> = (0..accumulators).map(|_| seeded.next()).collect();
                let mut expected = start.clone();
                let mut produced = start;
                fold_chunks_scalar(&mut expected, &constants, &blocks);
                (fns.fold_chunks)(&mut produced, &constants, &blocks);
                assert_eq!(
                    produced, expected,
                    "{} diverged at r={redundancy} L={accumulators}",
                    fns.name
                );
            }
        }
    }

    #[test]
    fn the_chunk_constants_are_the_shifted_monomials_modulo_the_generator() {
        let mut seeded = Seeded(0x2b69_68d3_0000_0009);
        for &redundancy in CHUNK_REDUNDANCIES {
            let words = redundancy.div_ceil(64);
            let accumulators = words + 2;
            let low = seeded.bits(redundancy);
            let mut constants = vec![0u64; accumulators * words];
            fold_chunk_constants(&low, redundancy, accumulators, &mut constants);
            for (index, run) in constants.chunks_exact(words).enumerate() {
                // x^d mod g is the remainder of a message that is the single
                // coefficient x^{d - r}.
                let degree = 64 * (accumulators + index) - redundancy;
                let mut message = vec![0u64; degree / 64 + 1];
                set_bit(&mut message, degree);
                let expected = naive_parity(&message, degree + 1, &low, redundancy);
                let produced: Vec<bool> = (0..redundancy)
                    .map(|bit_index| bit(run, bit_index))
                    .collect();
                assert_eq!(produced, expected, "r={redundancy} constant {index}");
                assert_eq!(run[words - 1] & !packed_tail_mask(redundancy), 0);
            }
        }
    }

    #[test]
    #[should_panic(expected = "whole chunks")]
    fn a_chunk_fold_rejects_a_partial_chunk() {
        let mut state = [0u64; 3];
        fold_chunks_scalar(&mut state, &[1, 2, 3], &[7; 4]);
    }

    #[test]
    #[should_panic(expected = "cannot hold a product")]
    fn a_chunk_fold_rejects_too_few_accumulators() {
        let mut state = [0u64; 2];
        fold_chunks_scalar(&mut state, &[1, 2, 3, 4], &[7; 2]);
    }

    #[test]
    fn the_default_accumulators_cover_the_product_and_the_pipeline() {
        for redundancy in [1usize, 64, 65, 168, 192, 256] {
            let words = redundancy.div_ceil(64);
            for in_flight in [FOLD_SCALAR_IN_FLIGHT, FOLD_PCLMUL_IN_FLIGHT] {
                let count = fold_accumulators(redundancy, in_flight);
                assert!(
                    count > words,
                    "r={redundancy}: a product needs {} words",
                    words + 1
                );
                assert!(
                    count * words >= in_flight,
                    "r={redundancy}: the pipeline needs {in_flight}"
                );
                assert!(count == words + 1 || (count - 1) * words < in_flight);
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
        fold_blocks_scalar(&mut [], &[], 0, 0, &[1]);
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
