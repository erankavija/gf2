//! Immutable and mutable views of a window of bits within a `u64` word store.

/// Immutable view of a bit slice.
#[derive(Copy, Clone)]
pub struct BitSlice<'a> {
    pub(crate) words: &'a [u64],
    pub(crate) offset: usize, // bit offset from the start of `words`
    pub(crate) len_bits: usize,
}

impl<'a> BitSlice<'a> {
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

    /// Bit at index `i` relative to the slice start.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn get(&self, i: usize) -> bool {
        assert!(i < self.len_bits, "BitSlice index out of bounds");
        let abs = self.offset + i;
        let w = abs >> 6;
        let b = abs & 63;
        ((self.words[w] >> b) & 1) != 0
    }
}

/// Mutable view of a bit slice.
pub struct BitSliceMut<'a> {
    pub(crate) words: &'a mut [u64],
    pub(crate) offset: usize,
    pub(crate) len_bits: usize,
}

impl<'a> BitSliceMut<'a> {
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

    /// Bit at index `i` relative to the slice start.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn get(&self, i: usize) -> bool {
        assert!(i < self.len_bits, "BitSlice index out of bounds");
        let abs = self.offset + i;
        let w = abs >> 6;
        let b = abs & 63;
        ((self.words[w] >> b) & 1) != 0
    }

    /// Writes `bit` at index `i` relative to the slice start.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn set(&mut self, i: usize, bit: bool) {
        assert!(i < self.len_bits, "BitSlice index out of bounds");
        let abs = self.offset + i;
        let w = abs >> 6;
        let b = abs & 63;
        let mask = 1u64 << b;
        if bit {
            self.words[w] |= mask;
        } else {
            self.words[w] &= !mask;
        }
    }
}
