//! Seeded fixtures with the addendum's exact alignments and cache states.
//!
//! One canonical `SplitMix64` stream per cell, started at that cell's workload
//! seed, fills every bank in order: for each bank, each item's matrix words in
//! canonical row-major order and then its vector words. A partial final word is
//! masked immediately after generation, so a `tail1` fixture carries canonical
//! zero tail padding before any operation runs.

use crate::cells::{Cache, MatvecShape, STREAMING_BANK_BYTES};
use gf2_core::{BitMatrix, BitVec};
use std::alloc::{alloc_zeroed, dealloc, Layout as AllocLayout};
use std::ptr::NonNull;
use tuning_campaign_support::abtest::SplitMix64;

/// Alignment every kernel fixture slab base satisfies.
pub const SLAB_ALIGN: usize = 64;

/// A 64-byte-aligned, zero-initialized `u64` allocation.
///
/// Allocation is explicit because the addendum fixes the isolated operands'
/// addresses modulo 64, which a `Vec<u64>` (aligned to 8) cannot promise.
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

/// Items per bank for a cache state and the bytes one item's fixture holds.
///
/// `item_bytes` is the fixture bytes the bank holds for one item, so a
/// streaming bank rounds up to the smallest integral number of complete tuples
/// that reaches [`STREAMING_BANK_BYTES`], as the addendum's `streaming` policy
/// requires (§ Cache, warmup, and sampling). A warm or cold cell rotates
/// nothing, so its one bank holds one item.
pub fn items_per_bank(cache: Cache, item_bytes: usize) -> usize {
    match cache {
        Cache::Warm | Cache::Cold => 1,
        // The guard keeps the division defined for the empty logical shapes the
        // oracle builds warm; no streaming cell declares one.
        Cache::Streaming => STREAMING_BANK_BYTES.div_ceil(item_bytes.max(1)),
    }
}

fn fill(mixer: &mut SplitMix64, words: &mut [u64]) {
    for word in words.iter_mut() {
        *word = mixer.next_u64();
    }
}

/// One aligned row-and-vector pair of an isolated fused-parity bank.
struct KernelItem {
    row: AlignedSlab,
    vector: AlignedSlab,
}

/// The working set of one isolated fused-parity cell.
///
/// Each item holds one `words`-word row and one `words`-word vector, each in
/// its own 64-byte-aligned slab, so both operands begin at address `0 (mod 64)`
/// as the addendum's primary product requires.
pub struct KernelBanks {
    banks: Vec<Vec<KernelItem>>,
    items: usize,
    words: usize,
}

impl KernelBanks {
    /// Fixture bytes one item holds: the two `words`-word slabs it allocates.
    pub const fn item_bytes(words: usize) -> usize {
        2 * words * 8
    }

    /// Builds the working set of one cell.
    pub fn build(words: usize, cache: Cache, seed: u64) -> Self {
        let items = items_per_bank(cache, Self::item_bytes(words));
        let mut mixer = SplitMix64::new(seed);
        let banks = (0..cache.banks())
            .map(|_| {
                (0..items)
                    .map(|_| {
                        let mut row = AlignedSlab::zeroed(words);
                        let mut vector = AlignedSlab::zeroed(words);
                        fill(&mut mixer, row.as_mut_slice());
                        fill(&mut mixer, vector.as_mut_slice());
                        KernelItem { row, vector }
                    })
                    .collect()
            })
            .collect();
        Self { banks, items, words }
    }

    /// Fixture banks in this working set.
    pub fn banks(&self) -> usize {
        self.banks.len()
    }

    /// Items in every bank.
    pub fn items(&self) -> usize {
        self.items
    }

    /// Fixture bytes one bank holds.
    pub fn bank_bytes(&self) -> usize {
        self.items * Self::item_bytes(self.words)
    }

    /// Resident bytes across every bank.
    pub fn working_set_bytes(&self) -> usize {
        self.banks() * self.bank_bytes()
    }

    /// Reads every initialized byte once, outside timing, without executing
    /// the measured operation. Returns an observation the caller keeps alive.
    pub fn touch(&self) -> u64 {
        let mut sink = 0_u64;
        for bank in &self.banks {
            for item in bank {
                for word in item.row.as_slice().iter().chain(item.vector.as_slice()) {
                    sink ^= *word;
                }
            }
        }
        sink
    }

    /// The `(row, vector)` operands of one item.
    pub fn operands(&self, bank: usize, item: usize) -> (&[u64], &[u64]) {
        let item = &self.banks[bank][item];
        (item.row.as_slice(), item.vector.as_slice())
    }

    /// Observed `(row, vector)` operand addresses modulo 64.
    pub fn addresses_mod_64(&self, bank: usize, item: usize) -> (usize, usize) {
        let item = &self.banks[bank][item];
        (item.row.base_addr() % SLAB_ALIGN, item.vector.base_addr() % SLAB_ALIGN)
    }
}

/// One matrix-and-vector tuple of an allocated or comparator bank.
pub struct MatvecItem {
    /// Public matrix under measurement.
    pub matrix: BitMatrix,
    /// Public vector under measurement.
    pub vector: BitVec,
}

/// The working set of one allocated or comparator cell.
pub struct MatvecBanks {
    banks: Vec<Vec<MatvecItem>>,
    items: usize,
    rows: usize,
    columns: usize,
    words: usize,
}

impl MatvecBanks {
    /// Fixture bytes one item holds: the matrix's `rows` row slabs of `words`
    /// words each, which is the stride `BitMatrix::zeros` derives, and the
    /// vector's `words` words.
    pub const fn item_bytes(rows: usize, words: usize) -> usize {
        (rows + 1) * words * 8
    }

    /// Builds the working set of one cell from its shape and cache state.
    pub fn build(rows: usize, columns: usize, cache: Cache, seed: u64) -> Self {
        let words = columns.div_ceil(64);
        let items = items_per_bank(cache, Self::item_bytes(rows, words));
        let mut mixer = SplitMix64::new(seed);
        let tail_bits = columns % 64;
        let mask = |word: u64| {
            if tail_bits == 0 {
                word
            } else {
                word & ((1_u64 << tail_bits) - 1)
            }
        };
        let banks = (0..cache.banks())
            .map(|_| {
                (0..items)
                    .map(|_| {
                        let mut matrix = BitMatrix::zeros(rows, columns);
                        for row in 0..rows {
                            let target = matrix.row_words_mut(row);
                            fill(&mut mixer, target);
                            if let Some(last) = target.last_mut() {
                                // Masked immediately after generation: a logical
                                // tail never carries a set padding bit.
                                *last = mask(*last);
                            }
                        }
                        let mut vector_words = vec![0_u64; words];
                        fill(&mut mixer, &mut vector_words);
                        if let Some(last) = vector_words.last_mut() {
                            *last = mask(*last);
                        }
                        let vector = BitVec::from_words(vector_words, columns);
                        MatvecItem { matrix, vector }
                    })
                    .collect()
            })
            .collect();
        Self { banks, items, rows, columns, words }
    }

    /// Builds the working set of one allocated cell.
    pub fn allocated(words: usize, shape: MatvecShape, cache: Cache, seed: u64) -> Self {
        Self::build(crate::cells::MATVEC_ROWS, shape.columns(words), cache, seed)
    }

    /// Fixture banks in this working set.
    pub fn banks(&self) -> usize {
        self.banks.len()
    }

    /// Items in every bank.
    pub fn items(&self) -> usize {
        self.items
    }

    /// Fixture bytes one bank holds.
    pub fn bank_bytes(&self) -> usize {
        self.items * Self::item_bytes(self.rows, self.words)
    }

    /// Logical rows of every matrix.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Logical columns of every matrix and vector.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// Resident bytes across every bank.
    pub fn working_set_bytes(&self) -> usize {
        self.banks() * self.bank_bytes()
    }

    /// Reads every initialized byte once, outside timing.
    pub fn touch(&self) -> u64 {
        let mut sink = 0_u64;
        for bank in &self.banks {
            for item in bank {
                for row in 0..item.matrix.rows() {
                    for word in item.matrix.row_words(row) {
                        sink ^= *word;
                    }
                }
                for word in item.vector.words() {
                    sink ^= *word;
                }
            }
        }
        sink
    }

    /// One tuple of the working set.
    pub fn item(&self, bank: usize, item: usize) -> &MatvecItem {
        &self.banks[bank][item]
    }

    /// Observed allocation base of one matrix's row storage modulo 64.
    pub fn base_mod_64(&self, bank: usize, item: usize) -> usize {
        let item = &self.banks[bank][item];
        if self.rows == 0 || self.words == 0 {
            return 0;
        }
        item.matrix.row_words(0).as_ptr() as usize % SLAB_ALIGN
    }
}
