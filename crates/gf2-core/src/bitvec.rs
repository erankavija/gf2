//! Core BitVec type for bit string manipulation.

use std::fmt;
use std::sync::Mutex;

/// Reverses the lowest `num_bits` bits of `x`.
#[inline]
fn reverse_bits(mut x: usize, num_bits: usize) -> usize {
    let mut result = 0;
    for _ in 0..num_bits {
        result = (result << 1) | (x & 1);
        x >>= 1;
    }
    result
}

/// Two-level popcount index behind [`BitVec::rank`] and [`BitVec::select`].
#[derive(Debug, Clone)]
struct RankSelectIndex {
    /// Cumulative popcount at each superblock (512-bit boundary)
    superblocks: Vec<usize>,
    /// Popcount within superblock for each block (64-bit boundary)
    blocks: Vec<u16>,
}

/// An owning, growable bit string backed by `Vec<u64>`.
///
/// ## Invariants
///
/// 1. `data` stores bits in little-endian order within each word.
/// 2. Bit `i` is stored at `data[i >> 6] & (1u64 << (i & 63))`.
/// 3. Padding bits beyond `len_bits` in the last word are always zero.
/// 4. `data.len() * 64 >= len_bits`, with exactly enough words allocated.
#[derive(Debug)]
pub struct BitVec {
    data: Vec<u64>,
    len_bits: usize,
    /// Lazy rank/select index (built on first query, thread-safe)
    rank_select_index: Mutex<Option<RankSelectIndex>>,
}

impl Clone for BitVec {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            len_bits: self.len_bits,
            // Don't clone the index - let it be built lazily in the clone
            rank_select_index: Mutex::new(None),
        }
    }
}

impl PartialEq for BitVec {
    fn eq(&self, other: &Self) -> bool {
        self.len_bits == other.len_bits && self.data == other.data
    }
}

impl Eq for BitVec {}

impl BitVec {
    /// Creates an empty `BitVec`.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            len_bits: 0,
            rank_select_index: Mutex::new(None),
        }
    }

    /// Creates a `BitVec` with at least the specified bit capacity.
    pub fn with_capacity(bits: usize) -> Self {
        let words = bits.div_ceil(64);
        Self {
            data: Vec::with_capacity(words),
            len_bits: 0,
            rank_select_index: Mutex::new(None),
        }
    }

    /// Creates a `BitVec` with `len` bits, all initialized to zero.
    pub fn zeros(len: usize) -> Self {
        let num_words = len.div_ceil(64);
        Self {
            data: vec![0u64; num_words],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        }
    }

    /// Creates a `BitVec` with `len` bits, all initialized to one.
    pub fn ones(len: usize) -> Self {
        if len == 0 {
            return Self {
                data: Vec::new(),
                len_bits: 0,
                rank_select_index: Mutex::new(None),
            };
        }

        let num_words = len.div_ceil(64);
        let mut data = vec![u64::MAX; num_words];

        let used_bits = len % 64;
        if used_bits != 0 {
            let mask = (1u64 << used_bits) - 1;
            data[num_words - 1] = mask;
        }

        Self {
            data,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        }
    }

    /// Creates a `BitVec` from raw word data with specified bit length.
    ///
    /// Words beyond `len_bits.div_ceil(64)` are discarded and bits at or
    /// above `len_bits` in the last kept word are cleared.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() * 64 < len_bits`.
    pub fn from_words(mut data: Vec<u64>, len_bits: usize) -> Self {
        let required_words = len_bits.div_ceil(64);
        assert!(
            data.len() >= required_words,
            "Insufficient words: need {}, got {}",
            required_words,
            data.len()
        );
        data.truncate(required_words);

        let mut bv = Self {
            data,
            len_bits,
            rank_select_index: Mutex::new(None),
        };
        bv.mask_tail();
        bv
    }

    /// Underlying word storage.
    #[inline]
    pub fn words(&self) -> &[u64] {
        &self.data
    }

    /// Length in bits.
    #[inline]
    pub fn len(&self) -> usize {
        self.len_bits
    }

    /// Whether the length is zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len_bits == 0
    }

    /// Appends a bit to the end of the `BitVec`.
    pub fn push_bit(&mut self, bit: bool) {
        let word_idx = self.len_bits / 64;
        let bit_idx = self.len_bits % 64;

        if bit_idx == 0 {
            self.data.push(0);
        }

        if bit {
            self.data[word_idx] |= 1u64 << bit_idx;
        }

        self.len_bits += 1;
    }

    /// Removes and returns the last bit, or `None` if empty.
    pub fn pop_bit(&mut self) -> Option<bool> {
        if self.len_bits == 0 {
            return None;
        }

        self.len_bits -= 1;
        let word_idx = self.len_bits / 64;
        let bit_idx = self.len_bits % 64;

        let bit = (self.data[word_idx] >> bit_idx) & 1 == 1;

        // Clear the bit to maintain invariant
        self.data[word_idx] &= !(1u64 << bit_idx);

        if bit_idx == 0 && !self.data.is_empty() {
            self.data.pop();
        }

        Some(bit)
    }

    /// Returns the value of the bit at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    #[inline]
    pub fn get(&self, idx: usize) -> bool {
        assert!(idx < self.len_bits, "index out of bounds");
        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        (self.data[word_idx] >> bit_idx) & 1 == 1
    }

    /// Sets the bit at the given index to the specified value.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    pub fn set(&mut self, idx: usize, bit: bool) {
        assert!(idx < self.len_bits, "index out of bounds");
        let word_idx = idx / 64;
        let bit_idx = idx % 64;

        if bit {
            self.data[word_idx] |= 1u64 << bit_idx;
        } else {
            self.data[word_idx] &= !(1u64 << bit_idx);
        }
    }

    /// Performs bitwise AND with `other` and stores the result in `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn bit_and_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        crate::kernels::ops::and_inplace(&mut self.data, &other.data);
    }

    /// Performs bitwise OR with `other` and stores the result in `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn bit_or_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        crate::kernels::ops::or_inplace(&mut self.data, &other.data);
    }

    /// Performs bitwise XOR with `other` and stores the result in `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn bit_xor_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        crate::kernels::ops::xor_inplace(&mut self.data, &other.data);
    }

    /// Performs bitwise NOT on all bits in `self`.
    pub fn not_into(&mut self) {
        crate::kernels::ops::not_inplace(&mut self.data);
        self.mask_tail();
    }

    /// Shifts all bits left by `k` positions. Bits shifted out are lost; zeros fill from the right.
    ///
    /// # Routes
    ///
    /// A `k` that is a whole number of words consults the logical kernel bundle,
    /// which needs this crate's non-default `simd` cargo feature and the `avx2`
    /// processor feature. Any other `k` goes to
    /// [`crate::residual_shift`], whose kernel route needs the same cargo
    /// feature and the `bmi2` processor feature and whose fallback is the
    /// portable funnel.
    pub fn shift_left(&mut self, k: usize) {
        if k == 0 || self.len_bits == 0 {
            return;
        }

        if k >= self.len_bits {
            for word in self.data.iter_mut() {
                *word = 0;
            }
            return;
        }

        let word_shift = k / 64;
        let bit_shift = k % 64;

        if bit_shift == 0 {
            #[cfg(feature = "simd")]
            if let Some(fns) = crate::simd::maybe_simd() {
                (fns.shift_left_words_fn)(&mut self.data, word_shift);
                self.mask_tail();
                return;
            }
            for i in (word_shift..self.data.len()).rev() {
                self.data[i] = self.data[i - word_shift];
            }
            for i in 0..word_shift.min(self.data.len()) {
                self.data[i] = 0;
            }
        } else {
            crate::residual_shift::shift_left(&mut self.data, word_shift, bit_shift as u32);
        }

        self.mask_tail();
    }

    /// Shifts all bits right by `k` positions. Bits shifted out are lost; zeros fill from the left.
    ///
    /// # Routes
    ///
    /// The same two branches as [`BitVec::shift_left`], under the same cargo
    /// feature and processor features.
    pub fn shift_right(&mut self, k: usize) {
        if k == 0 || self.len_bits == 0 {
            return;
        }

        if k >= self.len_bits {
            for word in self.data.iter_mut() {
                *word = 0;
            }
            return;
        }

        let word_shift = k / 64;
        let bit_shift = k % 64;

        if bit_shift == 0 {
            #[cfg(feature = "simd")]
            if let Some(fns) = crate::simd::maybe_simd() {
                (fns.shift_right_words_fn)(&mut self.data, word_shift);
                self.mask_tail();
                return;
            }
            for i in 0..(self.data.len() - word_shift) {
                self.data[i] = self.data[i + word_shift];
            }
            for i in (self.data.len() - word_shift)..self.data.len() {
                self.data[i] = 0;
            }
        } else {
            crate::residual_shift::shift_right(&mut self.data, word_shift, bit_shift as u32);
        }

        self.mask_tail();
    }

    /// Returns the number of set bits (population count).
    pub fn count_ones(&self) -> usize {
        crate::kernels::ops::popcount(&self.data) as usize
    }

    /// Returns `true` if the number of set bits is odd.
    pub fn parity(&self) -> bool {
        self.data
            .iter()
            .map(|&w| (w.count_ones() & 1) != 0)
            .fold(false, |acc, p| acc ^ p)
    }

    /// Builds the rank/select index if not already built.
    fn ensure_index(&self) {
        let mut index = self.rank_select_index.lock().unwrap();
        if index.is_some() {
            return;
        }

        let num_words = self.data.len();
        if num_words == 0 {
            *index = Some(RankSelectIndex {
                superblocks: Vec::new(),
                blocks: Vec::new(),
            });
            return;
        }

        let num_superblocks = num_words.div_ceil(8);
        let mut superblocks = Vec::with_capacity(num_superblocks);
        let mut blocks = Vec::with_capacity(num_words);

        let mut cumulative = 0usize;

        for sb_idx in 0..num_superblocks {
            superblocks.push(cumulative);
            let mut sb_count = 0u16;

            let start_word = sb_idx * 8;
            let end_word = ((sb_idx + 1) * 8).min(num_words);

            for w_idx in start_word..end_word {
                blocks.push(sb_count);
                let word_count = self.data[w_idx].count_ones() as u16;
                sb_count += word_count;
            }

            cumulative += sb_count as usize;
        }

        *index = Some(RankSelectIndex {
            superblocks,
            blocks,
        });
    }

    /// Returns the number of set bits in the range `[0..=idx]`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    ///
    /// # Complexity
    ///
    /// O(1) after index is built. Index building is O(n) and happens lazily on first query.
    pub fn rank(&self, idx: usize) -> usize {
        assert!(idx < self.len_bits, "index out of bounds");

        if self.len_bits == 0 {
            return 0;
        }

        self.ensure_index();
        let index = self.rank_select_index.lock().unwrap();
        let index = index.as_ref().unwrap();

        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        let sb_idx = word_idx / 8;

        let mut count = index.superblocks[sb_idx];
        count += index.blocks[word_idx] as usize;

        // `1 << 64` overflows, so bit 63 takes the full mask.
        let mask = if bit_idx == 63 {
            u64::MAX
        } else {
            (1u64 << (bit_idx + 1)) - 1
        };
        count += (self.data[word_idx] & mask).count_ones() as usize;

        count
    }

    /// Returns the position of the k-th set bit (0-indexed).
    ///
    /// Returns `None` if there are fewer than `k + 1` set bits in the vector.
    ///
    /// # Complexity
    ///
    /// O(n / 64) for the population count that bounds `k`, plus a binary
    /// search over the rank index.
    pub fn select(&self, k: usize) -> Option<usize> {
        if k >= self.count_ones() {
            return None;
        }

        self.ensure_index();
        let index = self.rank_select_index.lock().unwrap();
        let index = index.as_ref().unwrap();

        let target = k + 1;

        // `superblocks[i]` is the cumulative count before superblock `i`, so an
        // exact match at `i` places the target bit in superblock `i - 1`.
        let sb_idx = match index.superblocks.binary_search(&target) {
            Ok(i) => i.saturating_sub(1),
            Err(i) => i.saturating_sub(1),
        };

        if sb_idx >= index.superblocks.len() {
            return None;
        }
        let sb_base = index.superblocks[sb_idx];
        let start_word = sb_idx * 8;
        let end_word = ((sb_idx + 1) * 8).min(self.data.len());

        for word_idx in start_word..end_word {
            let count_before_word = sb_base + index.blocks[word_idx] as usize;
            let count_after_word = count_before_word + self.data[word_idx].count_ones() as usize;

            if target <= count_before_word {
                continue;
            }

            if target > count_after_word {
                continue;
            }
            let in_word_target = target - count_before_word;
            let mut word = self.data[word_idx];
            let mut count = 0;

            for bit_pos in 0..64 {
                if word & 1 == 1 {
                    count += 1;
                    if count == in_word_target {
                        let idx = word_idx * 64 + bit_pos;
                        return if idx < self.len_bits { Some(idx) } else { None };
                    }
                }
                word >>= 1;
            }
        }

        None
    }

    /// Returns the index of the first set bit, or `None` if all bits are zero.
    pub fn find_first_set(&self) -> Option<usize> {
        for (i, &word) in self.data.iter().enumerate() {
            if word != 0 {
                let bit_in_word = word.trailing_zeros() as usize;
                let idx = i * 64 + bit_in_word;
                if idx < self.len_bits {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Returns the index of the last set bit, or `None` if all bits are zero.
    pub fn find_last_set(&self) -> Option<usize> {
        for (i, &word) in self.data.iter().enumerate().rev() {
            if word != 0 {
                let leading = word.leading_zeros() as usize;
                let bit_in_word = 63 - leading;
                let idx = i * 64 + bit_in_word;
                if idx < self.len_bits {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Finds the index of the first set bit (1).
    ///
    /// Returns `None` if the bit vector is empty or contains only zeros.
    pub fn find_first_one(&self) -> Option<usize> {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return (fns.find_first_one_fn)(&self.data).filter(|&pos| pos < self.len_bits);
        }
        for (i, &word) in self.data.iter().enumerate() {
            if word != 0 {
                let bit_in_word = word.trailing_zeros() as usize;
                let pos = i * 64 + bit_in_word;
                if pos < self.len_bits {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Finds the index of the first clear bit (0).
    ///
    /// Returns `None` if the bit vector is empty or contains only ones.
    pub fn find_first_zero(&self) -> Option<usize> {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return (fns.find_first_zero_fn)(&self.data).filter(|&pos| pos < self.len_bits);
        }
        for (i, &word) in self.data.iter().enumerate() {
            if word != !0u64 {
                let bit_in_word = (!word).trailing_zeros() as usize;
                let pos = i * 64 + bit_in_word;
                if pos < self.len_bits {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Creates a `BitVec` from a byte slice in little-endian order.
    ///
    /// The length is `bytes.len() * 8`; bit `8 * i + j` is bit `j` of `bytes[i]`.
    pub fn from_bytes_le(bytes: &[u8]) -> Self {
        let len_bits = bytes.len() * 8;
        let num_words = len_bits.div_ceil(64);
        let mut data = vec![0u64; num_words];

        for (i, &byte) in bytes.iter().enumerate() {
            let word_idx = i / 8;
            let byte_in_word = i % 8;
            data[word_idx] |= (byte as u64) << (byte_in_word * 8);
        }

        Self {
            data,
            len_bits,
            rank_select_index: Mutex::new(None),
        }
    }

    /// Converts the `BitVec` to a byte vector in little-endian order.
    ///
    /// The returned vector has `(self.len() + 7) / 8` bytes.
    pub fn to_bytes_le(&self) -> Vec<u8> {
        let num_bytes = self.len_bits.div_ceil(8);
        let mut bytes = vec![0u8; num_bytes];

        for (i, byte) in bytes.iter_mut().enumerate() {
            let word_idx = i / 8;
            let byte_in_word = i % 8;
            if word_idx < self.data.len() {
                *byte = (self.data[word_idx] >> (byte_in_word * 8)) as u8;
            }
        }
        if !self.len_bits.is_multiple_of(8) {
            let last_byte_bits = self.len_bits % 8;
            let mask = (1u8 << last_byte_bits) - 1;
            if let Some(last) = bytes.last_mut() {
                *last &= mask;
            }
        }

        bytes
    }

    /// Returns an immutable `BitSlice` view for the given inclusive-exclusive range.
    ///
    /// Panics if the range is out of bounds.
    pub fn bit_slice<R: std::ops::RangeBounds<usize>>(&self, range: R) -> crate::BitSlice<'_> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(&s) => s,
            std::ops::Bound::Excluded(&s) => s + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(&e) => e + 1,
            std::ops::Bound::Excluded(&e) => e,
            std::ops::Bound::Unbounded => self.len_bits,
        };
        assert!(
            end >= start && end <= self.len_bits,
            "BitSlice range out of bounds"
        );
        crate::BitSlice {
            words: &self.data,
            offset: start,
            len_bits: end - start,
        }
    }

    /// Returns a mutable `BitSliceMut` view for the specified range.
    /// Panics if out of bounds.
    pub fn bit_slice_mut<R: std::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> crate::BitSliceMut<'_> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(&s) => s,
            std::ops::Bound::Excluded(&s) => s + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(&e) => e + 1,
            std::ops::Bound::Excluded(&e) => e,
            std::ops::Bound::Unbounded => self.len_bits,
        };
        assert!(
            end >= start && end <= self.len_bits,
            "BitSlice range out of bounds"
        );
        crate::BitSliceMut {
            words: &mut self.data,
            offset: start,
            len_bits: end - start,
        }
    }

    /// Creates a new `BitVec` by copying bits from a `BitSlice` view.
    pub fn from_bitslice(slice: crate::BitSlice) -> Self {
        if slice.len_bits == 0 {
            return Self::new();
        }
        let mut out = BitVec::with_capacity(slice.len_bits);
        for i in 0..slice.len_bits {
            out.push_bit(slice.get(i));
        }
        out
    }

    /// Creates a `BitVec` with random bits using the provided RNG.
    ///
    /// Each bit has probability 0.5 of being set. For custom probabilities,
    /// use [`BitVec::random_with_probability`].
    #[cfg(feature = "rand")]
    pub fn random<R: rand::Rng>(len_bits: usize, rng: &mut R) -> Self {
        if len_bits == 0 {
            return Self::new();
        }

        let num_words = len_bits.div_ceil(64);
        let mut data = vec![0u64; num_words];
        rng.fill(&mut data[..]);

        let mut bv = Self {
            data,
            len_bits,
            rank_select_index: Mutex::new(None),
        };
        bv.mask_tail();
        bv
    }

    /// Creates a `BitVec` with random bits from an RNG seeded with `seed`; equal
    /// seeds give equal vectors (`test_bitvec_random_seeded_deterministic`).
    #[cfg(feature = "rand")]
    pub fn random_seeded(len_bits: usize, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(seed);
        Self::random(len_bits, &mut rng)
    }

    /// Creates a `BitVec` with random bits where each bit is set with probability `p`.
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in the range [0.0, 1.0].
    #[cfg(feature = "rand")]
    pub fn random_with_probability<R: rand::Rng>(len_bits: usize, p: f64, rng: &mut R) -> Self {
        assert!(
            (0.0..=1.0).contains(&p),
            "Probability must be in range [0.0, 1.0], got {}",
            p
        );

        if len_bits == 0 {
            return Self::new();
        }

        if p == 0.0 {
            return Self {
                data: vec![0u64; len_bits.div_ceil(64)],
                len_bits,
                rank_select_index: Mutex::new(None),
            };
        }
        if p == 1.0 {
            let mut bv = Self {
                data: vec![u64::MAX; len_bits.div_ceil(64)],
                len_bits,
                rank_select_index: Mutex::new(None),
            };
            bv.mask_tail();
            return bv;
        }
        if (p - 0.5).abs() < 1e-10 {
            return Self::random(len_bits, rng);
        }
        let mut bv = Self::with_capacity(len_bits);
        for _ in 0..len_bits {
            bv.push_bit(rng.gen_bool(p));
        }
        bv
    }

    /// Fills this `BitVec` with random bits using the provided RNG.
    ///
    /// The length of the bit vector remains unchanged.
    #[cfg(feature = "rand")]
    pub fn fill_random<R: rand::Rng>(&mut self, rng: &mut R) {
        if !self.data.is_empty() {
            rng.fill(&mut self.data[..]);
            self.mask_tail();
        }
    }

    /// Clears all bits, setting the length to zero.
    pub fn clear(&mut self) {
        self.data.clear();
        self.len_bits = 0;
    }

    /// Resizes the `BitVec` to `new_len_bits`, filling with `fill_bit`.
    pub fn resize(&mut self, new_len_bits: usize, fill_bit: bool) {
        if new_len_bits == self.len_bits {
            return;
        }

        if new_len_bits < self.len_bits {
            self.len_bits = new_len_bits;
            let new_num_words = new_len_bits.div_ceil(64);
            self.data.truncate(new_num_words);
            self.mask_tail();
        } else {
            let old_len = self.len_bits;
            let new_num_words = new_len_bits.div_ceil(64);
            self.data
                .resize(new_num_words, if fill_bit { u64::MAX } else { 0 });
            self.len_bits = new_len_bits;

            if fill_bit {
                for i in old_len..new_len_bits {
                    self.set(i, true);
                }
            }

            self.mask_tail();
        }
    }

    /// Masks out padding bits in the last word to maintain the invariant.
    #[inline]
    fn mask_tail(&mut self) {
        if self.len_bits == 0 {
            return;
        }
        let bits_in_last_word = self.len_bits % 64;
        if bits_in_last_word != 0 {
            if let Some(last) = self.data.last_mut() {
                let mask = (1u64 << bits_in_last_word) - 1;
                *last &= mask;
            }
        }
    }

    /// Creates a bit-reversed copy of the first `n_bits`.
    ///
    /// Bit-reversal permutation reorders bits such that bit at position `i`
    /// moves to position with binary representation reversed.
    ///
    /// # Panics
    ///
    /// Panics if `n_bits` is not a power of 2 or if `n_bits > self.len()`.
    pub fn bit_reversed(&self, n_bits: usize) -> BitVec {
        assert!(
            n_bits.is_power_of_two() || n_bits == 0,
            "n_bits must be a power of 2"
        );
        assert!(n_bits <= self.len_bits, "n_bits exceeds BitVec length");

        let mut result = self.clone();
        result.bit_reverse_into(n_bits);
        result
    }

    /// Bit-reverses the first `n_bits` in place.
    ///
    /// # Panics
    ///
    /// Panics if `n_bits` is not a power of 2 or if `n_bits > self.len()`.
    pub fn bit_reverse_into(&mut self, n_bits: usize) {
        assert!(
            n_bits.is_power_of_two() || n_bits == 0,
            "n_bits must be a power of 2"
        );
        assert!(n_bits <= self.len_bits, "n_bits exceeds BitVec length");

        if n_bits <= 1 {
            return;
        }

        let num_bits_to_reverse = n_bits.trailing_zeros() as usize;

        for i in 0..n_bits {
            let j = reverse_bits(i, num_bits_to_reverse);
            // Only swap if i < j to avoid double-swapping
            if i < j {
                let bit_i = self.get(i);
                let bit_j = self.get(j);
                self.set(i, bit_j);
                self.set(j, bit_i);
            }
        }
    }

    /// Applies polar transform G_N = [1 0; 1 1]^⊗log2(n) to first `n` bits.
    ///
    /// # Panics
    ///
    /// Panics if `n` is not a power of 2 or if `n > self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n log n) where n is the transform size.
    pub fn polar_transform(&self, n: usize) -> BitVec {
        assert!(n.is_power_of_two(), "n must be a power of 2");
        assert!(n <= self.len_bits, "n exceeds BitVec length");

        let mut result = self.clone();
        result.polar_transform_into(n);
        result
    }

    /// Applies polar transform in place.
    ///
    /// # Panics
    ///
    /// Panics if `n` is not a power of 2 or if `n > self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n log n) where n is the transform size.
    pub fn polar_transform_into(&mut self, n: usize) {
        assert!(n.is_power_of_two(), "n must be a power of 2");
        assert!(n <= self.len_bits, "n exceeds BitVec length");

        if n <= 1 {
            return;
        }

        // Butterfly: (a, b) -> (a, a XOR b)
        let mut stride = 1;
        while stride < n {
            let mut i = 0;
            while i < n {
                for j in 0..stride {
                    let pos_low = i + j;
                    let pos_high = i + j + stride;

                    let bit_low = self.get(pos_low);
                    let bit_high = self.get(pos_high);
                    self.set(pos_high, bit_low ^ bit_high);
                }
                i += 2 * stride;
            }
            stride *= 2;
        }
    }

    /// Applies inverse polar transform to first `n` bits.
    ///
    /// # Panics
    ///
    /// Panics if `n` is not a power of 2 or if `n > self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n log n) where n is the transform size.
    pub fn polar_transform_inverse(&self, n: usize) -> BitVec {
        assert!(n.is_power_of_two(), "n must be a power of 2");
        assert!(n <= self.len_bits, "n exceeds BitVec length");

        let mut result = self.clone();
        result.polar_transform_inverse_into(n);
        result
    }

    /// Applies inverse polar transform in place.
    ///
    /// # Panics
    ///
    /// Panics if `n` is not a power of 2 or if `n > self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n log n) where n is the transform size.
    pub fn polar_transform_inverse_into(&mut self, n: usize) {
        assert!(n.is_power_of_two(), "n must be a power of 2");
        assert!(n <= self.len_bits, "n exceeds BitVec length");

        if n <= 1 {
            return;
        }

        // The polar transform is an involution.
        self.polar_transform_into(n);
    }
}

impl Default for BitVec {
    fn default() -> Self {
        Self::new()
    }
}

impl std::hash::Hash for BitVec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.len_bits.hash(state);
        let complete_words = self.len_bits / 64;
        for i in 0..complete_words {
            self.data[i].hash(state);
        }
        let remaining_bits = self.len_bits % 64;
        if remaining_bits > 0 {
            // We know padding bits are always zero due to tail masking invariant
            self.data[complete_words].hash(state);
        }
    }
}

impl fmt::Display for BitVec {
    /// Formats the BitVec in nalgebra-like style as a row vector.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[ ")?;
        for i in 0..self.len_bits {
            if self.get(i) {
                write!(f, "1")?;
            } else {
                write!(f, "0")?;
            }
            if i < self.len_bits - 1 {
                write!(f, " ")?;
            }
        }
        write!(f, " ]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let bv = BitVec::new();
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
    }

    #[test]
    fn test_with_capacity() {
        let bv = BitVec::with_capacity(100);
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
    }

    #[test]
    fn test_zeros() {
        let bv = BitVec::zeros(10);
        assert_eq!(bv.len(), 10);
        assert_eq!(bv.count_ones(), 0);
        for i in 0..10 {
            assert!(!bv.get(i));
        }
    }

    #[test]
    fn test_zeros_empty() {
        let bv = BitVec::zeros(0);
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
    }

    #[test]
    fn test_zeros_word_boundary() {
        let bv = BitVec::zeros(64);
        assert_eq!(bv.len(), 64);
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_zeros_cross_word() {
        let bv = BitVec::zeros(130);
        assert_eq!(bv.len(), 130);
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_ones() {
        let bv = BitVec::ones(10);
        assert_eq!(bv.len(), 10);
        assert_eq!(bv.count_ones(), 10);
        for i in 0..10 {
            assert!(bv.get(i));
        }
    }

    #[test]
    fn test_ones_empty() {
        let bv = BitVec::ones(0);
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
    }

    #[test]
    fn test_ones_word_boundary() {
        let bv = BitVec::ones(64);
        assert_eq!(bv.len(), 64);
        assert_eq!(bv.count_ones(), 64);
        for i in 0..64 {
            assert!(bv.get(i));
        }
    }

    #[test]
    fn test_ones_cross_word() {
        let bv = BitVec::ones(130);
        assert_eq!(bv.len(), 130);
        assert_eq!(bv.count_ones(), 130);
        for i in 0..130 {
            assert!(bv.get(i));
        }
    }

    #[test]
    fn test_ones_partial_word() {
        let bv = BitVec::ones(65);
        assert_eq!(bv.len(), 65);
        assert_eq!(bv.count_ones(), 65);
        assert_eq!(bv.data[1], 0x1);
    }

    #[test]
    fn test_ones_tail_masking() {
        for len in [1, 7, 63, 65, 127, 129] {
            let bv = BitVec::ones(len);
            assert_eq!(bv.len(), len);
            assert_eq!(bv.count_ones(), len);
            if len % 64 != 0 {
                let last_word = bv.data.last().unwrap();
                let used_bits = len % 64;
                let mask = (1u64 << used_bits) - 1;
                assert_eq!(*last_word, mask);
            }
        }
    }

    fn assert_padding_zero(bv: &BitVec) {
        let len = bv.len();
        let used_words = len.div_ceil(64);
        let words = bv.words();
        if !len.is_multiple_of(64) {
            assert_eq!(
                words[used_words - 1] >> (len % 64),
                0,
                "dirty tail padding at len={len}"
            );
        }
        assert!(
            words[used_words..].iter().all(|&w| w == 0),
            "dirty padding word beyond len={len}"
        );
    }

    #[test]
    fn test_from_words_masks_dirty_tail() {
        for len in [0_usize, 1, 63, 64, 65] {
            let bv = BitVec::from_words(vec![u64::MAX; len.div_ceil(64)], len);
            assert_eq!(bv.len(), len);
            assert_padding_zero(&bv);
            assert_eq!(bv.count_ones(), len, "popcount at len={len}");
            assert_eq!(bv, BitVec::ones(len), "equality at len={len}");
        }
    }

    #[test]
    fn test_from_words_drops_excess_words() {
        for len in [0_usize, 1, 63, 64, 65] {
            let bv = BitVec::from_words(vec![u64::MAX; len.div_ceil(64) + 2], len);
            assert_eq!(bv.len(), len);
            assert_padding_zero(&bv);
            assert_eq!(bv.count_ones(), len, "popcount at len={len}");
            assert_eq!(bv, BitVec::ones(len), "equality at len={len}");
        }
    }

    #[test]
    fn test_push_pop_single_bit() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        assert_eq!(bv.len(), 1);
        assert!(bv.get(0));
        assert_eq!(bv.pop_bit(), Some(true));
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_push_pop_multiple_bits() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.push_bit(false);
        bv.push_bit(true);
        assert_eq!(bv.len(), 3);
        assert_eq!(bv.pop_bit(), Some(true));
        assert_eq!(bv.pop_bit(), Some(false));
        assert_eq!(bv.pop_bit(), Some(true));
        assert_eq!(bv.pop_bit(), None);
    }

    #[test]
    fn test_get_set() {
        let mut bv = BitVec::new();
        bv.push_bit(false);
        bv.push_bit(true);
        bv.push_bit(false);

        assert!(!bv.get(0));
        assert!(bv.get(1));
        assert!(!bv.get(2));

        bv.set(0, true);
        bv.set(1, false);

        assert!(bv.get(0));
        assert!(!bv.get(1));
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_get_out_of_bounds() {
        let bv = BitVec::new();
        bv.get(0);
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_set_out_of_bounds() {
        let mut bv = BitVec::new();
        bv.set(0, true);
    }

    #[test]
    fn test_bit_and_into() {
        let mut a = BitVec::from_bytes_le(&[0b11110000]);
        let b = BitVec::from_bytes_le(&[0b11001100]);
        a.bit_and_into(&b);
        assert_eq!(a.to_bytes_le(), vec![0b11000000]);
    }

    #[test]
    fn test_bit_or_into() {
        let mut a = BitVec::from_bytes_le(&[0b11110000]);
        let b = BitVec::from_bytes_le(&[0b00001111]);
        a.bit_or_into(&b);
        assert_eq!(a.to_bytes_le(), vec![0b11111111]);
    }

    #[test]
    fn test_bit_xor_into() {
        let mut a = BitVec::from_bytes_le(&[0b11110000]);
        let b = BitVec::from_bytes_le(&[0b11001100]);
        a.bit_xor_into(&b);
        assert_eq!(a.to_bytes_le(), vec![0b00111100]);
    }

    #[test]
    fn test_not_into() {
        let mut bv = BitVec::from_bytes_le(&[0b11110000]);
        bv.not_into();
        assert_eq!(bv.to_bytes_le(), vec![0b00001111]);
    }

    #[test]
    fn test_shift_left() {
        let mut bv = BitVec::from_bytes_le(&[0b00001111]);
        bv.shift_left(2);
        assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
    }

    #[test]
    fn test_shift_right() {
        let mut bv = BitVec::from_bytes_le(&[0b11110000]);
        bv.shift_right(2);
        assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
    }

    #[test]
    fn test_count_ones() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.push_bit(false);
        bv.push_bit(true);
        bv.push_bit(true);
        assert_eq!(bv.count_ones(), 3);
    }

    #[test]
    fn test_find_first_set() {
        let mut bv = BitVec::new();
        bv.push_bit(false);
        bv.push_bit(false);
        bv.push_bit(true);
        bv.push_bit(false);
        assert_eq!(bv.find_first_set(), Some(2));
    }

    #[test]
    fn test_find_first_set_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.find_first_set(), None);
    }

    #[test]
    fn test_find_first_set_all_zeros() {
        let bv = BitVec::from_bytes_le(&[0, 0, 0]);
        assert_eq!(bv.find_first_set(), None);
    }

    #[test]
    fn test_find_last_set() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.push_bit(false);
        bv.push_bit(true);
        assert_eq!(bv.find_last_set(), Some(2));
    }

    #[test]
    fn test_find_last_set_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.find_last_set(), None);
    }

    #[test]
    fn test_from_bytes_le_to_bytes_le_roundtrip() {
        let bytes = vec![0xAA, 0x55, 0xFF, 0x00];
        let bv = BitVec::from_bytes_le(&bytes);
        assert_eq!(bv.len(), 32);
        assert_eq!(bv.to_bytes_le(), bytes);
    }

    #[test]
    fn test_clear() {
        let mut bv = BitVec::from_bytes_le(&[0xFF, 0xFF]);
        assert_eq!(bv.len(), 16);
        bv.clear();
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
    }

    #[test]
    fn test_resize_grow_with_zeros() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.resize(5, false);
        assert_eq!(bv.len(), 5);
        assert!(bv.get(0));
        assert!(!bv.get(1));
        assert!(!bv.get(4));
    }

    #[test]
    fn test_resize_grow_with_ones() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.resize(5, true);
        assert_eq!(bv.len(), 5);
        assert_eq!(bv.count_ones(), 5);
    }

    #[test]
    fn test_resize_shrink() {
        let mut bv = BitVec::from_bytes_le(&[0xFF]);
        bv.resize(4, false);
        assert_eq!(bv.len(), 4);
        assert_eq!(bv.count_ones(), 4);
    }

    #[test]
    fn test_boundary_63_bits() {
        let mut bv = BitVec::with_capacity(63);
        for _ in 0..63 {
            bv.push_bit(true);
        }
        assert_eq!(bv.len(), 63);
        assert_eq!(bv.count_ones(), 63);
    }

    #[test]
    fn test_boundary_64_bits() {
        let mut bv = BitVec::with_capacity(64);
        for _ in 0..64 {
            bv.push_bit(true);
        }
        assert_eq!(bv.len(), 64);
        assert_eq!(bv.count_ones(), 64);
    }

    #[test]
    fn test_boundary_65_bits() {
        let mut bv = BitVec::with_capacity(65);
        for _ in 0..65 {
            bv.push_bit(true);
        }
        assert_eq!(bv.len(), 65);
        assert_eq!(bv.count_ones(), 65);
    }

    #[test]
    fn test_shift_left_word_boundary() {
        let mut bv = BitVec::with_capacity(128);
        for i in 0..128 {
            bv.push_bit(i < 64);
        }
        bv.shift_left(64);
        assert_eq!(bv.count_ones(), 64);
        assert_eq!(bv.find_first_set(), Some(64));
    }

    #[test]
    fn test_shift_right_word_boundary() {
        let mut bv = BitVec::with_capacity(128);
        for i in 0..128 {
            bv.push_bit(i >= 64);
        }
        bv.shift_right(64);
        assert_eq!(bv.count_ones(), 64);
        assert_eq!(bv.find_last_set(), Some(63));
    }

    #[test]
    fn test_shift_left_beyond_length() {
        let mut bv = BitVec::from_bytes_le(&[0xFF]);
        let orig_len = bv.len();
        bv.shift_left(100);
        assert_eq!(bv.len(), orig_len);
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_shift_left_zero_is_noop() {
        let mut bv = BitVec::from_bytes_le(&[0b10101010]);
        let before = bv.to_bytes_le();
        bv.shift_left(0);
        assert_eq!(bv.to_bytes_le(), before);
    }

    #[test]
    fn test_shift_right_zero_is_noop() {
        let mut bv = BitVec::from_bytes_le(&[0b11001100]);
        let before = bv.to_bytes_le();
        bv.shift_right(0);
        assert_eq!(bv.to_bytes_le(), before);
    }

    #[test]
    fn test_shift_right_beyond_length() {
        let mut bv = BitVec::from_bytes_le(&[0xFF]);
        let orig_len = bv.len();
        bv.shift_right(100);
        assert_eq!(bv.len(), orig_len);
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_non_byte_aligned_length() {
        let mut bv = BitVec::new();
        for i in 0..10 {
            bv.push_bit(i % 2 == 0);
        }
        assert_eq!(bv.len(), 10);
        let bytes = bv.to_bytes_le();
        assert_eq!(bytes.len(), 2);

        let bv2 = BitVec::from_bytes_le(&bytes);
        assert_eq!(bv2.len(), 16);
    }

    #[test]
    fn test_mask_tail_invariant() {
        let mut bv = BitVec::new();
        for _ in 0..10 {
            bv.push_bit(true);
        }
        bv.not_into();
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_bit_slice_basic() {
        let bv = BitVec::from_bytes_le(&[0b1010_1100]);
        let s = bv.bit_slice(2..6);
        assert_eq!(s.len(), 4);
        assert!(s.get(0));
        assert!(s.get(3));
        let round = BitVec::from_bitslice(s);
        assert_eq!(round.len(), 4);
        assert!(round.get(0));
    }

    #[test]
    fn test_bit_slice_boundaries() {
        let mut bv = BitVec::with_capacity(65);
        for i in 0..65 {
            bv.push_bit(i % 3 == 0);
        }
        let s1 = bv.bit_slice(0..63);
        let s2 = bv.bit_slice(0..64);
        let s3 = bv.bit_slice(1..65);
        assert_eq!(s1.len(), 63);
        assert_eq!(s2.len(), 64);
        assert_eq!(s3.len(), 64);
        assert!(s2.get(0));
        assert_eq!(s3.get(0), bv.get(1));
    }

    #[test]
    fn test_parity_empty() {
        let bv = BitVec::new();
        assert!(!bv.parity());
    }

    #[test]
    fn test_parity_single_bit() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        assert!(bv.parity());
    }

    #[test]
    fn test_parity_two_bits() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.push_bit(true);
        assert!(!bv.parity());
    }

    #[test]
    fn test_parity_three_bits() {
        let mut bv = BitVec::new();
        bv.push_bit(true);
        bv.push_bit(true);
        bv.push_bit(true);
        assert!(bv.parity());
    }

    #[test]
    fn test_parity_from_bytes() {
        let bv = BitVec::from_bytes_le(&[0xFF]);
        assert!(!bv.parity());

        let bv = BitVec::from_bytes_le(&[0x7F]);
        assert!(bv.parity());

        let bv = BitVec::from_bytes_le(&[0x00]);
        assert!(!bv.parity());
    }

    #[test]
    fn test_parity_xor_property() {
        let bv_all_ones = BitVec::from_bytes_le(&[0xFF]);
        let bv_all_zeros = BitVec::from_bytes_le(&[0x00]);
        let bv_single = BitVec::from_bytes_le(&[0x01]);

        assert!(!bv_all_ones.parity());
        assert!(!bv_all_zeros.parity());
        assert!(bv_single.parity());
    }

    #[test]
    fn test_parity_matches_count_ones() {
        for byte in 0u8..=255 {
            let bv = BitVec::from_bytes_le(&[byte]);
            assert_eq!(
                bv.parity(),
                (bv.count_ones() % 2) == 1,
                "parity mismatch for byte 0x{:02x}",
                byte
            );
        }
    }

    #[test]
    fn test_find_first_one_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.find_first_one(), None);
    }

    #[test]
    fn test_find_first_one_all_zeros() {
        let bv = BitVec::from_bytes_le(&[0x00, 0x00, 0x00]);
        assert_eq!(bv.find_first_one(), None);
    }

    #[test]
    fn test_find_first_one_first_bit() {
        let bv = BitVec::from_bytes_le(&[0x01]);
        assert_eq!(bv.find_first_one(), Some(0));
    }

    #[test]
    fn test_find_first_one_last_bit_of_word() {
        let mut bv = BitVec::new();
        for _ in 0..63 {
            bv.push_bit(false);
        }
        bv.push_bit(true);
        assert_eq!(bv.find_first_one(), Some(63));
    }

    #[test]
    fn test_find_first_one_second_word() {
        let mut bv = BitVec::new();
        for _ in 0..64 {
            bv.push_bit(false);
        }
        bv.push_bit(true);
        assert_eq!(bv.find_first_one(), Some(64));
    }

    #[test]
    fn test_find_first_one_middle_bit() {
        let bv = BitVec::from_bytes_le(&[0b0001_0000]);
        assert_eq!(bv.find_first_one(), Some(4));
    }

    #[test]
    fn test_find_first_one_multiple_bits() {
        let bv = BitVec::from_bytes_le(&[0b1111_1000]);
        assert_eq!(bv.find_first_one(), Some(3));
    }

    #[test]
    fn test_find_first_one_respects_length() {
        let bv = BitVec::from_bytes_le(&[0xFF]);
        assert_eq!(bv.len(), 8);
        assert_eq!(bv.find_first_one(), Some(0));
    }

    #[test]
    fn test_find_first_zero_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.find_first_zero(), None);
    }

    #[test]
    fn test_find_first_zero_all_ones() {
        let bv = BitVec::from_bytes_le(&[0xFF, 0xFF, 0xFF]);
        assert_eq!(bv.find_first_zero(), None);
    }

    #[test]
    fn test_find_first_zero_first_bit() {
        let bv = BitVec::from_bytes_le(&[0xFE]);
        assert_eq!(bv.find_first_zero(), Some(0));
    }

    #[test]
    fn test_find_first_zero_middle_bit() {
        let bv = BitVec::from_bytes_le(&[0b1110_1111]);
        assert_eq!(bv.find_first_zero(), Some(4));
    }

    #[test]
    fn test_find_first_zero_second_word() {
        let mut bv = BitVec::new();
        for _ in 0..64 {
            bv.push_bit(true);
        }
        bv.push_bit(false);
        assert_eq!(bv.find_first_zero(), Some(64));
    }

    #[test]
    fn test_find_first_zero_respects_length() {
        let bv = BitVec::from_bytes_le(&[0x00]);
        assert_eq!(bv.len(), 8);
        assert_eq!(bv.find_first_zero(), Some(0));
    }

    #[test]
    fn test_rank_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_rank_all_zeros() {
        let bv = BitVec::zeros(100);
        for i in 0..100 {
            assert_eq!(bv.rank(i), 0);
        }
    }

    #[test]
    fn test_rank_all_ones() {
        let bv = BitVec::ones(100);
        for i in 0..100 {
            assert_eq!(bv.rank(i), i + 1);
        }
    }

    #[test]
    fn test_rank_single_bit() {
        let mut bv = BitVec::zeros(10);
        bv.set(5, true);

        assert_eq!(bv.rank(0), 0);
        assert_eq!(bv.rank(4), 0);
        assert_eq!(bv.rank(5), 1);
        assert_eq!(bv.rank(6), 1);
        assert_eq!(bv.rank(9), 1);
    }

    #[test]
    fn test_rank_multiple_bits() {
        let bv = BitVec::from_bytes_le(&[0b00101101]);

        assert_eq!(bv.rank(0), 1);
        assert_eq!(bv.rank(1), 1);
        assert_eq!(bv.rank(2), 2);
        assert_eq!(bv.rank(3), 3);
        assert_eq!(bv.rank(4), 3);
        assert_eq!(bv.rank(5), 4);
        assert_eq!(bv.rank(6), 4);
        assert_eq!(bv.rank(7), 4);
    }

    #[test]
    fn test_rank_word_boundary() {
        let mut bv = BitVec::zeros(128);
        bv.set(0, true);
        bv.set(63, true);
        bv.set(64, true);
        bv.set(127, true);

        assert_eq!(bv.rank(0), 1);
        assert_eq!(bv.rank(63), 2);
        assert_eq!(bv.rank(64), 3);
        assert_eq!(bv.rank(127), 4);
    }

    #[test]
    fn test_rank_large() {
        let bytes: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        let mut expected = 0;
        for i in 0..bv.len() {
            if bv.get(i) {
                expected += 1;
            }
            assert_eq!(bv.rank(i), expected);
        }
    }

    #[test]
    #[allow(clippy::manual_div_ceil)]
    fn test_rank_alternating_pattern() {
        let bv = BitVec::from_bytes_le(&[0b10101010]);

        for i in 0..8 {
            let expected = (i + 1) / 2;
            assert_eq!(bv.rank(i), expected);
        }
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_rank_out_of_bounds() {
        let bv = BitVec::zeros(10);
        let _ = bv.rank(10);
    }

    #[test]
    fn test_select_empty() {
        let bv = BitVec::new();
        assert_eq!(bv.select(0), None);
        assert_eq!(bv.select(1), None);
    }

    #[test]
    fn test_select_all_zeros() {
        let bv = BitVec::zeros(100);
        assert_eq!(bv.select(0), None);
        assert_eq!(bv.select(1), None);
    }

    #[test]
    fn test_select_all_ones() {
        let bv = BitVec::ones(100);
        for i in 0..100 {
            assert_eq!(bv.select(i), Some(i));
        }
        assert_eq!(bv.select(100), None);
    }

    #[test]
    fn test_select_single_bit() {
        let mut bv = BitVec::zeros(10);
        bv.set(5, true);

        assert_eq!(bv.select(0), Some(5));
        assert_eq!(bv.select(1), None);
    }

    #[test]
    fn test_select_multiple_bits() {
        let bv = BitVec::from_bytes_le(&[0b00101101]);

        assert_eq!(bv.select(0), Some(0));
        assert_eq!(bv.select(1), Some(2));
        assert_eq!(bv.select(2), Some(3));
        assert_eq!(bv.select(3), Some(5));
        assert_eq!(bv.select(4), None);
    }

    #[test]
    fn test_select_word_boundary() {
        let mut bv = BitVec::zeros(128);
        bv.set(0, true);
        bv.set(63, true);
        bv.set(64, true);
        bv.set(127, true);

        assert_eq!(bv.select(0), Some(0));
        assert_eq!(bv.select(1), Some(63));
        assert_eq!(bv.select(2), Some(64));
        assert_eq!(bv.select(3), Some(127));
        assert_eq!(bv.select(4), None);
    }

    #[test]
    fn test_select_large() {
        let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        let mut positions = Vec::new();
        for i in 0..bv.len() {
            if bv.get(i) {
                positions.push(i);
            }
        }
        for (k, &pos) in positions.iter().enumerate() {
            assert_eq!(bv.select(k), Some(pos));
        }
        assert_eq!(bv.select(positions.len()), None);
    }

    #[test]
    fn test_select_alternating_pattern() {
        let bv = BitVec::from_bytes_le(&[0b10101010]);

        assert_eq!(bv.select(0), Some(1));
        assert_eq!(bv.select(1), Some(3));
        assert_eq!(bv.select(2), Some(5));
        assert_eq!(bv.select(3), Some(7));
        assert_eq!(bv.select(4), None);
    }

    #[test]
    fn test_rank_select_invariant() {
        let bytes: Vec<u8> = (0..128).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(i) = bv.select(k) {
                assert_eq!(bv.rank(i), k + 1);
            }
        }
    }

    #[test]
    fn test_select_rank_roundtrip() {
        let bytes: Vec<u8> = (0..128).map(|i| (i * 17) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);
        let positions: Vec<usize> = (0..bv.len()).filter(|&i| bv.get(i)).collect();
        for &pos in &positions {
            let rank = bv.rank(pos);
            assert_eq!(bv.select(rank - 1), Some(pos));
        }
    }

    #[test]
    fn test_rank_is_cumulative() {
        let bv = BitVec::from_bytes_le(&[0b11010010, 0b00101101]);

        for i in 0..bv.len() - 1 {
            assert!(bv.rank(i) <= bv.rank(i + 1));
            assert!(bv.rank(i + 1) - bv.rank(i) <= 1);
        }
    }
}

#[cfg(test)]
mod select_edge_cases {
    use super::*;

    #[test]
    fn test_select_at_superblock_boundary() {
        // 192 ones in the first superblock make the target of `select(191)`
        // equal to `superblocks[1]`.
        let mut bv = BitVec::zeros(1024);
        for i in 0..191 {
            bv.set(i, true);
        }
        bv.set(509, true);
        assert_eq!(bv.rank(509), 192, "rank(509) should be 192");
        assert_eq!(bv.rank(508), 191, "rank(508) should be 191");
        assert_eq!(bv.select(190), Some(190), "select(190) should return 190");
        assert_eq!(bv.select(191), Some(509), "select(191) should return 509");
        assert_eq!(bv.select(192), None, "select(192) should return None");

        bv.set(518, true);
        assert_eq!(bv.select(192), Some(518), "select(192) should return 518");
    }

    #[test]
    fn test_select_exact_superblock_match() {
        let mut bv = BitVec::zeros(1024);

        for i in 0..512 {
            bv.set(i, true);
        }
        assert_eq!(bv.select(511), Some(511));
        bv.set(600, true);
        assert_eq!(bv.select(512), Some(600));
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn mask_tail_zeros_padding() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let last_word: u64 = kani::any();

        let bits_in_last = len % 64;
        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            let masked = last_word & mask;
            assert!(masked >> bits_in_last == 0);
        }
    }

    #[kani::proof]
    fn mask_tail_preserves_valid_bits() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let last_word: u64 = kani::any();

        let bits_in_last = len % 64;
        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            let masked = last_word & mask;
            assert!((masked & mask) == (last_word & mask));
        }
    }

    #[kani::proof]
    fn ones_constructor_tail_clean() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let used_bits = len % 64;

        let last_word = if used_bits != 0 {
            (1u64 << used_bits) - 1
        } else {
            u64::MAX
        };

        if used_bits != 0 {
            assert!(last_word >> used_bits == 0);
            let expected_mask = (1u64 << used_bits) - 1;
            assert!(last_word == expected_mask);
        } else {
            assert!(last_word == u64::MAX);
        }
        assert!(num_words == (len + 63) / 64);
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn shift_left_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 64);
        let k: usize = kani::any();
        kani::assume(k <= 64);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        bv.shift_left(k);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    /// NOT flips the padding bits too, so `not_into` must mask the tail.
    #[kani::proof]
    #[kani::unwind(3)]
    fn not_into_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let mut data = vec![0u64; num_words];
        for w in data.iter_mut() {
            *w = kani::any();
        }

        let bits_in_last = len % 64;
        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            data[num_words - 1] &= mask;
        }

        let mut bv = BitVec {
            data,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        bv.not_into();
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    /// XOR of two clean tails is clean, so no explicit mask is needed.
    #[kani::proof]
    #[kani::unwind(3)]
    fn bit_xor_into_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let bits_in_last = len % 64;

        let mut data_a = vec![0u64; num_words];
        let mut data_b = vec![0u64; num_words];
        for w in data_a.iter_mut() {
            *w = kani::any();
        }
        for w in data_b.iter_mut() {
            *w = kani::any();
        }

        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            data_a[num_words - 1] &= mask;
            data_b[num_words - 1] &= mask;
        }

        let mut a = BitVec {
            data: data_a,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };
        let b = BitVec {
            data: data_b,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        a.bit_xor_into(&b);
        if bits_in_last != 0 {
            if let Some(&last) = a.data.last() {
                assert!(last >> bits_in_last == 0);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn shift_right_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 64);
        let k: usize = kani::any();
        kani::assume(k <= 128);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        bv.shift_right(k);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    /// AND of two clean tails is clean, so no explicit mask is needed.
    #[kani::proof]
    #[kani::unwind(3)]
    fn bit_and_into_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let bits_in_last = len % 64;

        let mut data_a = vec![0u64; num_words];
        let mut data_b = vec![0u64; num_words];
        for w in data_a.iter_mut() {
            *w = kani::any();
        }
        for w in data_b.iter_mut() {
            *w = kani::any();
        }

        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            data_a[num_words - 1] &= mask;
            data_b[num_words - 1] &= mask;
        }

        let mut a = BitVec {
            data: data_a,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };
        let b = BitVec {
            data: data_b,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        a.bit_and_into(&b);
        if bits_in_last != 0 {
            if let Some(&last) = a.data.last() {
                assert!(last >> bits_in_last == 0);
            }
        }
    }

    /// OR of two clean tails is clean, so no explicit mask is needed.
    #[kani::proof]
    #[kani::unwind(3)]
    fn bit_or_into_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let bits_in_last = len % 64;

        let mut data_a = vec![0u64; num_words];
        let mut data_b = vec![0u64; num_words];
        for w in data_a.iter_mut() {
            *w = kani::any();
        }
        for w in data_b.iter_mut() {
            *w = kani::any();
        }

        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            data_a[num_words - 1] &= mask;
            data_b[num_words - 1] &= mask;
        }

        let mut a = BitVec {
            data: data_a,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };
        let b = BitVec {
            data: data_b,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        a.bit_or_into(&b);
        if bits_in_last != 0 {
            if let Some(&last) = a.data.last() {
                assert!(last >> bits_in_last == 0);
            }
        }
    }

    #[kani::proof]
    fn set_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 64);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        let idx: usize = kani::any();
        kani::assume(idx < len);
        let bit: bool = kani::any();

        bv.set(idx, bit);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(3)]
    fn push_bit_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 127);

        let num_words = len.div_ceil(64);
        let bits_in_last = len % 64;

        let mut data = vec![0u64; num_words];
        for w in data.iter_mut() {
            *w = kani::any();
        }

        if bits_in_last != 0 {
            let mask = (1u64 << bits_in_last) - 1;
            data[num_words - 1] &= mask;
        }

        let mut bv = BitVec {
            data,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        let bit: bool = kani::any();
        bv.push_bit(bit);
        let new_bits_in_last = bv.len_bits % 64;
        if new_bits_in_last != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> new_bits_in_last == 0);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn resize_shrink_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 2 && len <= 64);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        let new_len: usize = kani::any();
        kani::assume(new_len >= 1 && new_len < len);

        bv.resize(new_len, false);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn resize_grow_zero_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 32);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        let new_len: usize = kani::any();
        kani::assume(new_len > len && new_len <= 64);

        bv.resize(new_len, false);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    /// Bounds are kept small to avoid OOM from the `set()` loop.
    #[kani::proof]
    #[kani::unwind(5)]
    fn resize_grow_fill_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 2);

        let word: u64 = kani::any();
        let bits_in_last = len % 64;
        let masked_word = if bits_in_last != 0 {
            word & ((1u64 << bits_in_last) - 1)
        } else {
            word
        };

        let mut bv = BitVec {
            data: vec![masked_word],
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        let new_len: usize = kani::any();
        kani::assume(new_len > len && new_len <= 4);

        bv.resize(new_len, true);
        let bits_after = bv.len_bits % 64;
        if bits_after != 0 {
            if let Some(&last) = bv.data.last() {
                assert!(last >> bits_after == 0);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(3)]
    fn clear_preserves_invariant() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 128);

        let num_words = len.div_ceil(64);
        let mut data = vec![0u64; num_words];
        for w in data.iter_mut() {
            *w = kani::any();
        }

        let mut bv = BitVec {
            data,
            len_bits: len,
            rank_select_index: Mutex::new(None),
        };

        bv.clear();

        assert!(bv.len_bits == 0);
        assert!(bv.data.is_empty());
    }
}
