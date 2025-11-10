//! BitSlice views over a BitVec's underlying storage.
//!
//! Immutable and mutable views that reference a window of bits within a
//! `&[u64]`/`&mut [u64]` backing store. Views carry a bit offset and a bit
//! length and preserve the tail-masking invariant when converted back to
//! `BitVec`.

/// Immutable view of a bit slice.
#[derive(Copy, Clone)]
pub struct BitSlice<'a> {
    pub(crate) words: &'a [u64],
    pub(crate) offset: usize,   // bit offset from the start of `words`
    pub(crate) len_bits: usize, // number of live bits in the view
}

impl<'a> BitSlice<'a> {
    /// Returns the number of bits in this slice.
    #[inline]
    pub fn len(&self) -> usize {
        self.len_bits
    }

    /// Returns true if the slice is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len_bits == 0
    }

    /// Returns the bit at relative index `i` within the slice.
    /// Panics if out of bounds.
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
    /// Returns the number of bits in this slice.
    #[inline]
    pub fn len(&self) -> usize {
        self.len_bits
    }

    /// Returns true if the slice is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len_bits == 0
    }

    /// Reads the bit at relative index `i` within the slice.
    pub fn get(&self, i: usize) -> bool {
        assert!(i < self.len_bits, "BitSlice index out of bounds");
        let abs = self.offset + i;
        let w = abs >> 6;
        let b = abs & 63;
        ((self.words[w] >> b) & 1) != 0
    }

    /// Sets the bit at relative index `i`.
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
