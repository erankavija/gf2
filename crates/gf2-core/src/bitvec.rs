//! Core BitVec type for bit string manipulation.

use std::cell::RefCell;
use std::fmt;

/// Rank/Select index for efficient bit queries.
///
/// Uses a two-level index structure:
/// - Superblocks: One entry per 512 bits (8 words), stores cumulative popcount
/// - Blocks: One entry per 64 bits (1 word), stores popcount within superblock
///
/// This enables O(1) rank and O(log n) select queries.
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
///
/// ## Examples
///
/// ```
/// use gf2_core::BitVec;
///
/// let mut bv = BitVec::new();
/// bv.push_bit(true);
/// assert_eq!(bv.len(), 1);
/// assert_eq!(bv.get(0), true);
/// ```
#[derive(Debug, Clone)]
pub struct BitVec {
    data: Vec<u64>,
    len_bits: usize,
    /// Lazy rank/select index (built on first query)
    rank_select_index: RefCell<Option<RankSelectIndex>>,
}

impl PartialEq for BitVec {
    fn eq(&self, other: &Self) -> bool {
        self.len_bits == other.len_bits && self.data == other.data
    }
}

impl Eq for BitVec {}

impl BitVec {
    /// Creates an empty `BitVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::new();
    /// assert_eq!(bv.len(), 0);
    /// assert!(bv.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            len_bits: 0,
            rank_select_index: RefCell::new(None),
        }
    }

    /// Creates a `BitVec` with at least the specified bit capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::with_capacity(100);
    /// assert_eq!(bv.len(), 0);
    /// ```
    pub fn with_capacity(bits: usize) -> Self {
        let words = bits.div_ceil(64);
        Self {
            data: Vec::with_capacity(words),
            len_bits: 0,
            rank_select_index: RefCell::new(None),
        }
    }

    /// Creates a `BitVec` with `len` bits, all initialized to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::zeros(10);
    /// assert_eq!(bv.len(), 10);
    /// assert_eq!(bv.count_ones(), 0);
    /// ```
    pub fn zeros(len: usize) -> Self {
        let num_words = len.div_ceil(64);
        Self {
            data: vec![0u64; num_words],
            len_bits: len,
            rank_select_index: RefCell::new(None),
        }
    }

    /// Creates a `BitVec` with `len` bits, all initialized to one.
    ///
    /// Padding bits beyond `len` in the last word are set to zero,
    /// maintaining the tail masking invariant.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::ones(10);
    /// assert_eq!(bv.len(), 10);
    /// assert_eq!(bv.count_ones(), 10);
    /// ```
    pub fn ones(len: usize) -> Self {
        if len == 0 {
            return Self {
                data: Vec::new(),
                len_bits: 0,
                rank_select_index: RefCell::new(None),
            };
        }

        let num_words = len.div_ceil(64);
        let mut data = vec![u64::MAX; num_words];

        // Mask padding bits in the last word to maintain tail masking invariant
        let used_bits = len % 64;
        if used_bits != 0 {
            let mask = (1u64 << used_bits) - 1;
            data[num_words - 1] = mask;
        }

        Self {
            data,
            len_bits: len,
            rank_select_index: RefCell::new(None),
        }
    }

    /// Returns the number of bits in the `BitVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// assert_eq!(bv.len(), 0);
    /// bv.push_bit(true);
    /// assert_eq!(bv.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len_bits
    }

    /// Returns `true` if the `BitVec` contains no bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// assert!(bv.is_empty());
    /// bv.push_bit(false);
    /// assert!(!bv.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len_bits == 0
    }

    /// Appends a bit to the end of the `BitVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// assert_eq!(bv.len(), 2);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// assert_eq!(bv.pop_bit(), Some(true));
    /// assert_eq!(bv.pop_bit(), None);
    /// ```
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

        // Remove word if it was the last bit in the word
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// assert_eq!(bv.get(0), true);
    /// assert_eq!(bv.get(1), false);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(false);
    /// bv.set(0, true);
    /// assert_eq!(bv.get(0), true);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut a = BitVec::from_bytes_le(&[0b11110000]);
    /// let b = BitVec::from_bytes_le(&[0b11001100]);
    /// a.bit_and_into(&b);
    /// assert_eq!(a.to_bytes_le(), vec![0b11000000]);
    /// ```
    pub fn bit_and_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            (fns.and_fn)(&mut self.data, &other.data);
            return;
        }
        for (a, b) in self.data.iter_mut().zip(other.data.iter()) {
            *a &= *b;
        }
    }

    /// Performs bitwise OR with `other` and stores the result in `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut a = BitVec::from_bytes_le(&[0b11110000]);
    /// let b = BitVec::from_bytes_le(&[0b00001111]);
    /// a.bit_or_into(&b);
    /// assert_eq!(a.to_bytes_le(), vec![0b11111111]);
    /// ```
    pub fn bit_or_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            (fns.or_fn)(&mut self.data, &other.data);
            return;
        }
        for (a, b) in self.data.iter_mut().zip(other.data.iter()) {
            *a |= *b;
        }
    }

    /// Performs bitwise XOR with `other` and stores the result in `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut a = BitVec::from_bytes_le(&[0b11110000]);
    /// let b = BitVec::from_bytes_le(&[0b11001100]);
    /// a.bit_xor_into(&b);
    /// assert_eq!(a.to_bytes_le(), vec![0b00111100]);
    /// ```
    pub fn bit_xor_into(&mut self, other: &BitVec) {
        assert_eq!(self.len_bits, other.len_bits, "BitVec lengths must match");
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            (fns.xor_fn)(&mut self.data, &other.data);
            return;
        }
        for (a, b) in self.data.iter_mut().zip(other.data.iter()) {
            *a ^= *b;
        }
    }

    /// Performs bitwise NOT on all bits in `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::from_bytes_le(&[0b11110000]);
    /// bv.not_into();
    /// assert_eq!(bv.to_bytes_le(), vec![0b00001111]);
    /// ```
    pub fn not_into(&mut self) {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            (fns.not_fn)(&mut self.data);
            self.mask_tail();
            return;
        }
        for word in self.data.iter_mut() {
            *word = !*word;
        }
        self.mask_tail();
    }

    /// Shifts all bits left by `k` positions. Bits shifted out are lost; zeros fill from the right.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::from_bytes_le(&[0b00001111]);
    /// bv.shift_left(2);
    /// assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
    /// ```
    pub fn shift_left(&mut self, k: usize) {
        if k == 0 || self.len_bits == 0 {
            return;
        }

        if k >= self.len_bits {
            // Zero all data but preserve length
            for word in self.data.iter_mut() {
                *word = 0;
            }
            return;
        }

        let word_shift = k / 64;
        let bit_shift = k % 64;

        if bit_shift == 0 {
            // Word-aligned shift - can use SIMD
            #[cfg(feature = "simd")]
            if let Some(fns) = crate::simd::maybe_simd() {
                (fns.shift_left_words_fn)(&mut self.data, word_shift);
                self.mask_tail();
                return;
            }

            // Scalar fallback
            for i in (word_shift..self.data.len()).rev() {
                self.data[i] = self.data[i - word_shift];
            }
            for i in 0..word_shift.min(self.data.len()) {
                self.data[i] = 0;
            }
        } else {
            // Non-aligned shift
            let inv_shift = 64 - bit_shift;
            for i in (word_shift + 1..self.data.len()).rev() {
                self.data[i] = (self.data[i - word_shift] << bit_shift)
                    | (self.data[i - word_shift - 1] >> inv_shift);
            }
            if word_shift < self.data.len() {
                self.data[word_shift] = self.data[0] << bit_shift;
            }
            for i in 0..word_shift.min(self.data.len()) {
                self.data[i] = 0;
            }
        }

        self.mask_tail();
    }

    /// Shifts all bits right by `k` positions. Bits shifted out are lost; zeros fill from the left.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::from_bytes_le(&[0b11110000]);
    /// bv.shift_right(2);
    /// assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
    /// ```
    pub fn shift_right(&mut self, k: usize) {
        if k == 0 || self.len_bits == 0 {
            return;
        }

        if k >= self.len_bits {
            // Zero all data but preserve length
            for word in self.data.iter_mut() {
                *word = 0;
            }
            return;
        }

        let word_shift = k / 64;
        let bit_shift = k % 64;

        if bit_shift == 0 {
            // Word-aligned shift - can use SIMD
            #[cfg(feature = "simd")]
            if let Some(fns) = crate::simd::maybe_simd() {
                (fns.shift_right_words_fn)(&mut self.data, word_shift);
                self.mask_tail();
                return;
            }

            // Scalar fallback
            for i in 0..(self.data.len() - word_shift) {
                self.data[i] = self.data[i + word_shift];
            }
            for i in (self.data.len() - word_shift)..self.data.len() {
                self.data[i] = 0;
            }
        } else {
            // Non-aligned shift
            let inv_shift = 64 - bit_shift;
            for i in 0..(self.data.len() - word_shift - 1) {
                self.data[i] = (self.data[i + word_shift] >> bit_shift)
                    | (self.data[i + word_shift + 1] << inv_shift);
            }
            if word_shift < self.data.len() {
                let len = self.data.len();
                let last_val = self.data[len - 1] >> bit_shift;
                self.data[len - word_shift - 1] = last_val;
            }
            for i in (self.data.len() - word_shift)..self.data.len() {
                self.data[i] = 0;
            }
        }

        self.mask_tail();
    }

    /// Returns the number of set bits (population count).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// assert_eq!(bv.count_ones(), 2);
    /// ```
    pub fn count_ones(&self) -> usize {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return (fns.popcnt_fn)(&self.data) as usize;
        }
        self.data.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Builds the rank/select index if not already built.
    fn ensure_index(&self) {
        let mut index = self.rank_select_index.borrow_mut();
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

        // One superblock per 8 words (512 bits)
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
    /// This operation runs in O(1) time after the index is built (on first call).
    /// The index is built lazily and cached for subsequent queries.
    ///
    /// # Arguments
    ///
    /// * `idx` - The bit position (0-indexed, inclusive)
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::from_bytes_le(&[0b00101101]); // bits: 10110100
    /// assert_eq!(bv.rank(0), 1); // bit 0 is set
    /// assert_eq!(bv.rank(2), 2); // bits 0, 2 are set
    /// assert_eq!(bv.rank(5), 4); // bits 0, 2, 3, 5 are set
    /// ```
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
        let index = self.rank_select_index.borrow();
        let index = index.as_ref().unwrap();

        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        let sb_idx = word_idx / 8;

        // Count from superblock + block + bits within word
        let mut count = index.superblocks[sb_idx];
        count += index.blocks[word_idx] as usize;

        // Count bits in the current word up to bit_idx (inclusive)
        // Special case: if bit_idx == 63, we want all 64 bits
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
    /// # Arguments
    ///
    /// * `k` - The rank of the set bit to find (0-indexed)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::from_bytes_le(&[0b00101101]); // bits: 10110100
    /// assert_eq!(bv.select(0), Some(0)); // 1st set bit at position 0
    /// assert_eq!(bv.select(1), Some(2)); // 2nd set bit at position 2
    /// assert_eq!(bv.select(3), Some(5)); // 4th set bit at position 5
    /// assert_eq!(bv.select(4), None);     // only 4 set bits total
    /// ```
    ///
    /// # Complexity
    ///
    /// O(log n) using binary search over the rank index, then O(1) for word-level selection.
    pub fn select(&self, k: usize) -> Option<usize> {
        if k >= self.count_ones() {
            return None;
        }

        self.ensure_index();
        let index = self.rank_select_index.borrow();
        let index = index.as_ref().unwrap();

        let target = k + 1; // We want the position where rank equals k+1

        // Binary search to find the superblock containing the target
        // superblocks[i] stores the cumulative count BEFORE superblock i
        // If we get an exact match at i, the target bit is in superblock i-1
        let sb_idx = match index.superblocks.binary_search(&target) {
            Ok(i) => i.saturating_sub(1),
            Err(i) => i.saturating_sub(1),
        };

        if sb_idx >= index.superblocks.len() {
            return None;
        }

        // How many ones we've seen before this superblock
        let sb_base = index.superblocks[sb_idx];

        // Linear search within superblock to find the word
        let start_word = sb_idx * 8;
        let end_word = ((sb_idx + 1) * 8).min(self.data.len());

        for word_idx in start_word..end_word {
            // Total count up to (but not including) this word
            let count_before_word = sb_base + index.blocks[word_idx] as usize;
            // Total count up to and including this word
            let count_after_word = count_before_word + self.data[word_idx].count_ones() as usize;

            if target <= count_before_word {
                // Target is in a previous word, shouldn't happen with correct logic
                continue;
            }

            if target > count_after_word {
                // Target is beyond this word, continue
                continue;
            }

            // The target bit is in this word
            // We need the (target - count_before_word)-th set bit in this word
            let in_word_target = target - count_before_word;

            // Find the in_word_target-th set bit within self.data[word_idx]
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(false);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// assert_eq!(bv.find_first_set(), Some(2));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// assert_eq!(bv.find_last_set(), Some(2));
    /// ```
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
    /// This method can benefit from SIMD acceleration when the `simd` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::from_bytes_le(&[0b0001_0000]); // bit 4 set
    /// assert_eq!(bv.find_first_one(), Some(4));
    ///
    /// let empty = BitVec::new();
    /// assert_eq!(empty.find_first_one(), None);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of words, but typically much faster due to early exit.
    /// SIMD implementations can process multiple words in parallel.
    pub fn find_first_one(&self) -> Option<usize> {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return (fns.find_first_one_fn)(&self.data).filter(|&pos| pos < self.len_bits);
        }

        // Scalar fallback
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
    /// This method can benefit from SIMD acceleration when the `simd` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::from_bytes_le(&[0b1110_1111]); // bit 4 clear
    /// assert_eq!(bv.find_first_zero(), Some(4));
    ///
    /// let all_ones = BitVec::from_bytes_le(&[0xFF]);
    /// assert_eq!(all_ones.find_first_zero(), None);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of words, but typically much faster due to early exit.
    /// SIMD implementations can process multiple words in parallel.
    pub fn find_first_zero(&self) -> Option<usize> {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return (fns.find_first_zero_fn)(&self.data).filter(|&pos| pos < self.len_bits);
        }

        // Scalar fallback
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
    /// The length is set to `bytes.len() * 8`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let bv = BitVec::from_bytes_le(&[0b10101010, 0b11110000]);
    /// assert_eq!(bv.len(), 16);
    /// assert_eq!(bv.get(1), true); // second bit of first byte
    /// ```
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
            rank_select_index: RefCell::new(None),
        }
    }

    /// Converts the `BitVec` to a byte vector in little-endian order.
    ///
    /// The returned vector has `(self.len() + 7) / 8` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// assert_eq!(bv.to_bytes_le(), vec![0b10101010]);
    /// ```
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

        // Mask the last byte if needed
        if self.len_bits % 8 != 0 {
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
    ///
    /// # Arguments
    ///
    /// * `len_bits` - The number of bits in the resulting vector
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::BitVec;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut rng = StdRng::seed_from_u64(42);
    /// let bv = BitVec::random(100, &mut rng);
    /// assert_eq!(bv.len(), 100);
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of words (`len_bits / 64`).
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
            rank_select_index: RefCell::new(None),
        };
        bv.mask_tail();
        bv
    }

    /// Creates a `BitVec` with random bits using a seeded RNG.
    ///
    /// This provides deterministic random generation - the same seed
    /// will always produce the same bit vector.
    ///
    /// # Arguments
    ///
    /// * `len_bits` - The number of bits in the resulting vector
    /// * `seed` - Seed value for the random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::BitVec;
    ///
    /// let bv1 = BitVec::random_seeded(100, 0x1234);
    /// let bv2 = BitVec::random_seeded(100, 0x1234);
    /// assert_eq!(bv1, bv2); // Same seed produces same bits
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of words (`len_bits / 64`).
    #[cfg(feature = "rand")]
    pub fn random_seeded(len_bits: usize, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(seed);
        Self::random(len_bits, &mut rng)
    }

    /// Creates a `BitVec` with random bits where each bit is set with probability `p`.
    ///
    /// For `p = 0.5`, prefer [`BitVec::random`] which is optimized for the uniform case.
    ///
    /// # Arguments
    ///
    /// * `len_bits` - The number of bits in the resulting vector
    /// * `p` - Probability in [0.0, 1.0] that each bit is set to 1
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in the range [0.0, 1.0].
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::BitVec;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut rng = StdRng::seed_from_u64(42);
    /// // Create a sparse bit vector (~10% ones)
    /// let bv = BitVec::random_with_probability(1000, 0.1, &mut rng);
    /// assert_eq!(bv.len(), 1000);
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n = `len_bits`. Note that this is slower than [`BitVec::random`]
    /// for the default p=0.5 case.
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

        // Fast paths for extreme probabilities
        if p == 0.0 {
            return Self {
                data: vec![0u64; len_bits.div_ceil(64)],
                len_bits,
                rank_select_index: RefCell::new(None),
            };
        }
        if p == 1.0 {
            let mut bv = Self {
                data: vec![u64::MAX; len_bits.div_ceil(64)],
                len_bits,
                rank_select_index: RefCell::new(None),
            };
            bv.mask_tail();
            return bv;
        }

        // For p=0.5, use optimized word-level generation
        if (p - 0.5).abs() < 1e-10 {
            return Self::random(len_bits, rng);
        }

        // General case: generate bits individually
        let mut bv = Self::with_capacity(len_bits);
        for _ in 0..len_bits {
            bv.push_bit(rng.gen_bool(p));
        }
        bv
    }

    /// Fills this `BitVec` with random bits using the provided RNG.
    ///
    /// The length of the bit vector remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::BitVec;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut bv = BitVec::from_bytes_le(&[0x00; 10]);
    /// let mut rng = StdRng::seed_from_u64(42);
    /// bv.fill_random(&mut rng);
    /// // bv now contains random bits
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of words.
    #[cfg(feature = "rand")]
    pub fn fill_random<R: rand::Rng>(&mut self, rng: &mut R) {
        if !self.data.is_empty() {
            rng.fill(&mut self.data[..]);
            self.mask_tail();
        }
    }

    /// Clears all bits, setting the length to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::from_bytes_le(&[0xFF, 0xFF]);
    /// bv.clear();
    /// assert_eq!(bv.len(), 0);
    /// assert!(bv.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.data.clear();
        self.len_bits = 0;
    }

    /// Resizes the `BitVec` to `new_len_bits`, filling with `fill_bit`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.resize(5, true);
    /// assert_eq!(bv.len(), 5);
    /// assert_eq!(bv.count_ones(), 5);
    /// ```
    pub fn resize(&mut self, new_len_bits: usize, fill_bit: bool) {
        if new_len_bits == self.len_bits {
            return;
        }

        if new_len_bits < self.len_bits {
            // Shrinking
            self.len_bits = new_len_bits;
            let new_num_words = new_len_bits.div_ceil(64);
            self.data.truncate(new_num_words);
            self.mask_tail();
        } else {
            // Growing
            let old_len = self.len_bits;
            let new_num_words = new_len_bits.div_ceil(64);
            self.data
                .resize(new_num_words, if fill_bit { u64::MAX } else { 0 });
            self.len_bits = new_len_bits;

            if fill_bit {
                // Set bits from old_len to new_len_bits
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
}

impl Default for BitVec {
    fn default() -> Self {
        Self::new()
    }
}

impl std::hash::Hash for BitVec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the length
        self.len_bits.hash(state);

        // Hash all complete words
        let complete_words = self.len_bits / 64;
        for i in 0..complete_words {
            self.data[i].hash(state);
        }

        // Hash the remaining bits in the last word (if any)
        let remaining_bits = self.len_bits % 64;
        if remaining_bits > 0 {
            // We know padding bits are always zero due to tail masking invariant
            self.data[complete_words].hash(state);
        }
    }
}

impl fmt::Display for BitVec {
    /// Formats the BitVec in nalgebra-like style as a row vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitVec;
    ///
    /// let mut bv = BitVec::new();
    /// bv.push_bit(true);
    /// bv.push_bit(false);
    /// bv.push_bit(true);
    /// bv.push_bit(true);
    /// println!("{}", bv);  // Displays: [ 1 0 1 1 ]
    /// ```
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
        // Check that padding bits are zero (tail masking invariant)
        assert_eq!(bv.data[1], 0x1); // Only bit 0 of second word should be set
    }

    #[test]
    fn test_ones_tail_masking() {
        // Test that padding bits are properly masked for various lengths
        for len in [1, 7, 63, 65, 127, 129] {
            let bv = BitVec::ones(len);
            assert_eq!(bv.len(), len);
            assert_eq!(bv.count_ones(), len);

            // Verify tail masking: padding bits should be zero
            if len % 64 != 0 {
                let last_word = bv.data.last().unwrap();
                let used_bits = len % 64;
                let mask = (1u64 << used_bits) - 1;
                assert_eq!(*last_word, mask);
            }
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

    // Edge case tests: boundary conditions
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
        assert_eq!(bv.len(), orig_len); // Length should be preserved
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_shift_right_beyond_length() {
        let mut bv = BitVec::from_bytes_le(&[0xFF]);
        let orig_len = bv.len();
        bv.shift_right(100);
        assert_eq!(bv.len(), orig_len); // Length should be preserved
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
        // Note: from_bytes_le creates a bitvec with len = bytes.len() * 8
        assert_eq!(bv2.len(), 16);
    }

    #[test]
    fn test_mask_tail_invariant() {
        let mut bv = BitVec::new();
        for _ in 0..10 {
            bv.push_bit(true);
        }
        bv.not_into();
        // Verify that bits beyond len_bits are zero
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn test_bit_slice_basic() {
        let bv = BitVec::from_bytes_le(&[0b1010_1100]); // 8 bits
        let s = bv.bit_slice(2..6);
        assert_eq!(s.len(), 4);
        assert!(s.get(0)); // original bit 2
        assert!(s.get(3)); // original bit 5
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
        // Spot check a few bits
        assert!(s2.get(0));
        assert_eq!(s3.get(0), bv.get(1));
    }

    // Scan operation tests
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
        let bv = BitVec::from_bytes_le(&[0x01]); // bit 0 set
        assert_eq!(bv.find_first_one(), Some(0));
    }

    #[test]
    fn test_find_first_one_last_bit_of_word() {
        let mut bv = BitVec::new();
        for _ in 0..63 {
            bv.push_bit(false);
        }
        bv.push_bit(true); // bit 63 set
        assert_eq!(bv.find_first_one(), Some(63));
    }

    #[test]
    fn test_find_first_one_second_word() {
        let mut bv = BitVec::new();
        for _ in 0..64 {
            bv.push_bit(false);
        }
        bv.push_bit(true); // bit 64 set
        assert_eq!(bv.find_first_one(), Some(64));
    }

    #[test]
    fn test_find_first_one_middle_bit() {
        let bv = BitVec::from_bytes_le(&[0b0001_0000]); // bit 4 set
        assert_eq!(bv.find_first_one(), Some(4));
    }

    #[test]
    fn test_find_first_one_multiple_bits() {
        let bv = BitVec::from_bytes_le(&[0b1111_1000]); // bits 3-7 set
        assert_eq!(bv.find_first_one(), Some(3));
    }

    #[test]
    fn test_find_first_one_respects_length() {
        let bv = BitVec::from_bytes_le(&[0xFF]);
        // BitVec length is 8 bits
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
        let bv = BitVec::from_bytes_le(&[0xFE]); // bit 0 clear
        assert_eq!(bv.find_first_zero(), Some(0));
    }

    #[test]
    fn test_find_first_zero_middle_bit() {
        let bv = BitVec::from_bytes_le(&[0b1110_1111]); // bit 4 clear
        assert_eq!(bv.find_first_zero(), Some(4));
    }

    #[test]
    fn test_find_first_zero_second_word() {
        let mut bv = BitVec::new();
        for _ in 0..64 {
            bv.push_bit(true);
        }
        bv.push_bit(false); // bit 64 clear
        assert_eq!(bv.find_first_zero(), Some(64));
    }

    #[test]
    fn test_find_first_zero_respects_length() {
        let bv = BitVec::from_bytes_le(&[0x00]); // 8 bits, all zero
        assert_eq!(bv.len(), 8);
        assert_eq!(bv.find_first_zero(), Some(0));
    }

    // ========== Rank Tests ==========

    #[test]
    fn test_rank_empty() {
        let bv = BitVec::new();
        // Empty bitvec has no valid indices
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
        // Pattern: 10110100
        let bv = BitVec::from_bytes_le(&[0b00101101]);

        assert_eq!(bv.rank(0), 1); // bit 0 is set
        assert_eq!(bv.rank(1), 1); // bit 1 is clear
        assert_eq!(bv.rank(2), 2); // bit 2 is set
        assert_eq!(bv.rank(3), 3); // bit 3 is set
        assert_eq!(bv.rank(4), 3); // bit 4 is clear
        assert_eq!(bv.rank(5), 4); // bit 5 is set
        assert_eq!(bv.rank(6), 4); // bit 6 is clear
        assert_eq!(bv.rank(7), 4); // bit 7 is clear
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

        // Verify rank is cumulative count
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
        // Pattern: 01010101
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

    // ========== Select Tests ==========

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
        // Pattern: 10110100
        let bv = BitVec::from_bytes_le(&[0b00101101]);

        assert_eq!(bv.select(0), Some(0)); // 1st one at position 0
        assert_eq!(bv.select(1), Some(2)); // 2nd one at position 2
        assert_eq!(bv.select(2), Some(3)); // 3rd one at position 3
        assert_eq!(bv.select(3), Some(5)); // 4th one at position 5
        assert_eq!(bv.select(4), None); // no 5th one
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

        // Build expected positions of set bits
        let mut positions = Vec::new();
        for i in 0..bv.len() {
            if bv.get(i) {
                positions.push(i);
            }
        }

        // Verify select returns correct positions
        for (k, &pos) in positions.iter().enumerate() {
            assert_eq!(bv.select(k), Some(pos));
        }
        assert_eq!(bv.select(positions.len()), None);
    }

    #[test]
    fn test_select_alternating_pattern() {
        // Pattern: 01010101
        let bv = BitVec::from_bytes_le(&[0b10101010]);

        assert_eq!(bv.select(0), Some(1));
        assert_eq!(bv.select(1), Some(3));
        assert_eq!(bv.select(2), Some(5));
        assert_eq!(bv.select(3), Some(7));
        assert_eq!(bv.select(4), None);
    }

    // ========== Rank-Select Invariant Tests ==========

    #[test]
    fn test_rank_select_invariant() {
        let bytes: Vec<u8> = (0..128).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        // For all k, if select(k) = i, then rank(i) = k + 1
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

        // Build all set bit positions
        let positions: Vec<usize> = (0..bv.len()).filter(|&i| bv.get(i)).collect();

        // For each set bit position, select(rank(i) - 1) should equal i
        for &pos in &positions {
            let rank = bv.rank(pos);
            assert_eq!(bv.select(rank - 1), Some(pos));
        }
    }

    #[test]
    fn test_rank_is_cumulative() {
        let bv = BitVec::from_bytes_le(&[0b11010010, 0b00101101]);

        // Rank should be monotonically increasing
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
        // Regression test for select bug at superblock boundaries
        // Create a bitvec where select(191) should return position 509
        // This tests the case where the target equals a superblock boundary

        let mut bv = BitVec::zeros(1024);

        // Set the first 192 bits (positions 0..192)
        // But position 509 should be the 192nd set bit (0-indexed: select(191))
        // So we set bits 0..191 and bit 509

        // Actually, let's create the exact scenario:
        // Set bits at positions creating the boundary condition
        // We want 191 ones before position 509, and position 509 to be set

        // Set bits 0 through 190 (191 bits)
        for i in 0..191 {
            bv.set(i, true);
        }

        // Skip some bits, then set position 509
        bv.set(509, true);

        // Now we have:
        // - 191 set bits at positions 0..191
        // - 1 set bit at position 509
        // Total: 192 set bits

        // Verify rank
        assert_eq!(bv.rank(509), 192, "rank(509) should be 192");
        assert_eq!(bv.rank(508), 191, "rank(508) should be 191");

        // Test select
        assert_eq!(bv.select(190), Some(190), "select(190) should return 190");
        assert_eq!(bv.select(191), Some(509), "select(191) should return 509");
        assert_eq!(bv.select(192), None, "select(192) should return None");

        // Additional edge case: set bit 518 to be the 193rd set bit
        bv.set(518, true);
        assert_eq!(bv.select(192), Some(518), "select(192) should return 518");
    }

    #[test]
    fn test_select_exact_superblock_match() {
        // Test when target exactly equals a superblock cumulative count
        let mut bv = BitVec::zeros(1024);

        // Set exactly 512 bits in the first superblock (words 0-7)
        for i in 0..512 {
            bv.set(i, true);
        }

        // select(511) should return 511 (the last bit of first superblock)
        assert_eq!(bv.select(511), Some(511));

        // Add one more bit at position 600
        bv.set(600, true);

        // select(512) should return 600 (first bit in second superblock)
        assert_eq!(bv.select(512), Some(600));
    }
}
