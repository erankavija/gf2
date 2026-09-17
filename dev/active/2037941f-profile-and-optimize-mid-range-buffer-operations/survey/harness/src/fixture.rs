//! Seeded fixtures with the addendum's exact address layouts and cache states.
//!
//! One canonical `SplitMix64` stream per cell, started at that cell's workload
//! seed, fills every bank in order: for each bank, each item's source words
//! then its destination words, in canonical row-major order. A partial final
//! word is masked immediately after generation.
//!
//! A bank is one 64-byte-aligned slab per buffer role. Items inside a slab are
//! strided to a whole multiple of 64 bytes, so every item view begins at the
//! layout's byte offset from a 64-byte-aligned base.

use crate::cells::{Cache, Layout, RowShape, STREAMING_BANK_BYTES};
use gf2_core::BitMatrix;
use std::alloc::{alloc_zeroed, dealloc, Layout as AllocLayout};
use std::ptr::NonNull;
use tuning_campaign_support::abtest::SplitMix64;

/// Alignment every fixture slab base satisfies.
pub const SLAB_ALIGN: usize = 64;

/// A 64-byte-aligned, zero-initialized `u64` allocation.
///
/// Allocation is explicit because the addendum fixes view addresses modulo 64,
/// which a `Vec<u64>` (aligned to 8) cannot promise.
pub struct AlignedSlab {
    ptr: NonNull<u64>,
    words: usize,
}

// SAFETY: the slab owns its allocation exclusively and holds no interior
// mutability or thread-affine resource, so moving it across threads is sound.
unsafe impl Send for AlignedSlab {}

impl AlignedSlab {
    /// Allocates `words` zeroed `u64` on a 64-byte boundary.
    ///
    /// # Panics
    ///
    /// Panics when `words` is zero or the allocation fails.
    pub fn zeroed(words: usize) -> Self {
        assert!(words > 0, "a fixture slab holds at least one word");
        let layout = AllocLayout::from_size_align(words * 8, SLAB_ALIGN)
            .expect("fixture slab layout is valid");
        // SAFETY: `layout` has non-zero size, so `alloc_zeroed` is callable and
        // returns either null or a block of `words * 8` zeroed bytes aligned to
        // `SLAB_ALIGN`, which is exactly the storage this type owns.
        let raw = unsafe { alloc_zeroed(layout) }.cast::<u64>();
        let ptr = NonNull::new(raw).expect("fixture slab allocation succeeds");
        Self { ptr, words }
    }

    /// The slab's words.
    pub fn as_slice(&self) -> &[u64] {
        // SAFETY: the allocation holds `self.words` initialized `u64` and lives
        // as long as `self`, and no `&mut` to it exists while this borrow does.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.words) }
    }

    /// The slab's words, mutably.
    pub fn as_mut_slice(&mut self) -> &mut [u64] {
        // SAFETY: as `as_slice`, and the `&mut self` borrow excludes any other
        // reference to the same allocation.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.words) }
    }

    /// Base address of the allocation.
    pub fn base_addr(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
}

impl Drop for AlignedSlab {
    fn drop(&mut self) {
        let layout = AllocLayout::from_size_align(self.words * 8, SLAB_ALIGN)
            .expect("fixture slab layout is valid");
        // SAFETY: the pointer came from `alloc_zeroed` with this exact layout
        // and has not been freed, because only `Drop` frees it.
        unsafe { dealloc(self.ptr.as_ptr().cast(), layout) }
    }
}

/// One buffer role of one fixture bank.
pub struct BufferBank {
    slab: AlignedSlab,
    offset_words: usize,
    stride_words: usize,
    items: usize,
    words: usize,
}

impl BufferBank {
    /// Builds a bank of `items` views of `words` words at `layout`.
    pub fn new(words: usize, items: usize, layout: Layout) -> Self {
        assert!(words > 0 && items > 0);
        let offset_bytes = layout.offset_bytes();
        assert_eq!(offset_bytes % 8, 0, "a view offset is a whole word");
        let offset_words = offset_bytes / 8;
        let stride_words = stride_words(words);
        let slab = AlignedSlab::zeroed(offset_words + stride_words * items);
        Self {
            slab,
            offset_words,
            stride_words,
            items,
            words,
        }
    }

    /// Items in the bank.
    pub fn items(&self) -> usize {
        self.items
    }

    /// Bytes the bank occupies.
    pub fn bytes(&self) -> usize {
        self.stride_words * self.items * 8
    }

    fn range(&self, item: usize) -> std::ops::Range<usize> {
        let start = self.offset_words + item * self.stride_words;
        start..start + self.words
    }

    /// One item view.
    pub fn item(&self, item: usize) -> &[u64] {
        &self.slab.as_slice()[self.range(item)]
    }

    /// One item view, mutably.
    pub fn item_mut(&mut self, item: usize) -> &mut [u64] {
        let range = self.range(item);
        &mut self.slab.as_mut_slice()[range]
    }

    /// Address of one item view.
    pub fn item_addr(&self, item: usize) -> usize {
        self.slab.base_addr() + self.range(item).start * 8
    }

    /// Base address of the bank's slab.
    pub fn base_addr(&self) -> usize {
        self.slab.base_addr()
    }
}

/// Item stride that keeps every item start on a 64-byte boundary.
pub fn stride_words(words: usize) -> usize {
    words.div_ceil(8) * 8
}

/// Items per bank for a cache state and per-item byte cost.
pub fn items_per_bank(cache: Cache, item_bytes: usize) -> usize {
    match cache {
        Cache::Warm | Cache::Cold => 1,
        Cache::Streaming => STREAMING_BANK_BYTES.div_ceil(item_bytes).max(1),
    }
}

fn fill(mixer: &mut SplitMix64, words: &mut [u64]) {
    for word in words.iter_mut() {
        *word = mixer.next_u64();
    }
}

/// Source and destination banks of an isolated-XOR or ISA-L-peer cell.
pub struct XorBanks {
    src: Vec<BufferBank>,
    dst: Vec<BufferBank>,
    items: usize,
    words: usize,
    layout: Layout,
}

impl XorBanks {
    /// Builds the working set of one cell.
    ///
    /// Every bank holds two buffer roles. The isolated-XOR question reads them
    /// as destination and source; the ISA-L question reads them as source zero
    /// and source one, because that comparison arranges a fresh destination
    /// inside every timed call.
    pub fn build(words: usize, layout: Layout, cache: Cache, seed: u64) -> Self {
        let item_bytes = 2 * stride_words(words) * 8;
        let items = items_per_bank(cache, item_bytes);
        let banks = cache.banks();
        let mut mixer = SplitMix64::new(seed);
        let mut src = Vec::with_capacity(banks);
        let mut dst = Vec::with_capacity(banks);
        for _ in 0..banks {
            let mut source = BufferBank::new(words, items, layout);
            let mut destination = BufferBank::new(words, items, layout);
            for item in 0..items {
                fill(&mut mixer, source.item_mut(item));
                fill(&mut mixer, destination.item_mut(item));
            }
            src.push(source);
            dst.push(destination);
        }
        Self {
            src,
            dst,
            items,
            words,
            layout,
        }
    }

    /// Items in every bank.
    pub fn items(&self) -> usize {
        self.items
    }

    /// Words one logical operation consumes.
    pub fn words(&self) -> usize {
        self.words
    }

    /// Declared layout.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Resident bytes across every bank and role.
    pub fn working_set_bytes(&self) -> usize {
        self.src.iter().map(BufferBank::bytes).sum::<usize>()
            + self.dst.iter().map(BufferBank::bytes).sum::<usize>()
    }

    /// Reads every initialized byte once, outside timing, without executing
    /// the measured operation. Returns an observation the caller keeps alive.
    pub fn touch(&self) -> u64 {
        let mut sink = 0_u64;
        for bank in self.src.iter().chain(self.dst.iter()) {
            for item in 0..bank.items() {
                for word in bank.item(item) {
                    sink ^= *word;
                }
            }
        }
        sink
    }

    /// The `(destination, source)` views of one item.
    pub fn pair(&mut self, bank: usize, item: usize) -> (&mut [u64], &[u64]) {
        let Self { src, dst, .. } = self;
        (dst[bank].item_mut(item), src[bank].item(item))
    }

    /// The two source views of one item, for the ISA-L comparison.
    pub fn sources(&self, bank: usize, item: usize) -> (&[u64], &[u64]) {
        (self.src[bank].item(item), self.dst[bank].item(item))
    }

    /// Observed address of one item's source and destination modulo 64.
    pub fn addresses_mod_64(&self, bank: usize, item: usize) -> (usize, usize) {
        (
            self.src[bank].item_addr(item) % SLAB_ALIGN,
            self.dst[bank].item_addr(item) % SLAB_ALIGN,
        )
    }

    /// Observed base addresses of one bank's slabs modulo 64.
    pub fn bases_mod_64(&self, bank: usize) -> (usize, usize) {
        (
            self.src[bank].base_addr() % SLAB_ALIGN,
            self.dst[bank].base_addr() % SLAB_ALIGN,
        )
    }
}

/// Matrix banks of a public row-XOR cell.
pub struct RowBanks {
    banks: Vec<Vec<BitMatrix>>,
    items: usize,
    words: usize,
    shape: RowShape,
}

impl RowBanks {
    /// Builds the working set of one row-XOR cell.
    pub fn build(words: usize, shape: RowShape, cache: Cache, seed: u64) -> Self {
        let columns = shape.columns(words);
        let item_bytes = crate::cells::ROW_MATRIX_ROWS * words * 8;
        let items = items_per_bank(cache, item_bytes);
        let mut mixer = SplitMix64::new(seed);
        let tail_bits = columns % 64;
        let banks = (0..cache.banks())
            .map(|_| {
                (0..items)
                    .map(|_| {
                        let mut matrix = BitMatrix::zeros(crate::cells::ROW_MATRIX_ROWS, columns);
                        for row in 0..crate::cells::ROW_MATRIX_ROWS {
                            let target = matrix.row_words_mut(row);
                            for word in target.iter_mut() {
                                *word = mixer.next_u64();
                            }
                            if tail_bits != 0 {
                                // The partial final word is masked immediately
                                // after generation: canonical zero tail padding.
                                let last = target.len() - 1;
                                target[last] &= (1_u64 << tail_bits) - 1;
                            }
                        }
                        matrix
                    })
                    .collect()
            })
            .collect();
        Self {
            banks,
            items,
            words,
            shape,
        }
    }

    /// Items in every bank.
    pub fn items(&self) -> usize {
        self.items
    }

    /// Declared shape.
    pub fn shape(&self) -> RowShape {
        self.shape
    }

    /// Resident bytes across every bank.
    pub fn working_set_bytes(&self) -> usize {
        self.banks.len() * self.items * crate::cells::ROW_MATRIX_ROWS * self.words * 8
    }

    /// Reads every initialized byte once, outside timing.
    pub fn touch(&self) -> u64 {
        let mut sink = 0_u64;
        for bank in &self.banks {
            for matrix in bank {
                for row in 0..matrix.rows() {
                    for word in matrix.row_words(row) {
                        sink ^= *word;
                    }
                }
            }
        }
        sink
    }

    /// One matrix of the working set.
    pub fn matrix_mut(&mut self, bank: usize, item: usize) -> &mut BitMatrix {
        &mut self.banks[bank][item]
    }

    /// One matrix of the working set.
    pub fn matrix(&self, bank: usize, item: usize) -> &BitMatrix {
        &self.banks[bank][item]
    }

    /// Observed allocation base of one matrix's row storage modulo 64.
    pub fn base_mod_64(&self, bank: usize, item: usize) -> usize {
        self.banks[bank][item].row_words(0).as_ptr() as usize % SLAB_ALIGN
    }

    /// Observed `(dst, src)` addresses modulo 64 for every frozen row pair.
    pub fn pair_addresses_mod_64(&self, bank: usize, item: usize) -> Vec<(usize, usize)> {
        let matrix = &self.banks[bank][item];
        crate::cells::ROW_PAIRS
            .iter()
            .map(|(dst, src)| {
                (
                    matrix.row_words(*dst).as_ptr() as usize % SLAB_ALIGN,
                    matrix.row_words(*src).as_ptr() as usize % SLAB_ALIGN,
                )
            })
            .collect()
    }
}
