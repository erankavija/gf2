//! Seeded word buffers placed at a declared offset from a vector boundary.

use tuning_campaign_support::abtest::SplitMix64;

/// Bytes in one AVX2 vector; the alignment of every fixture's base.
pub const VECTOR_BYTES: usize = 32;
/// Largest supported word offset: one to three words break vector alignment.
pub const MAX_WORD_OFFSET: usize = 3;
const WORD_BYTES: usize = size_of::<u64>();
/// Words of seeded data past the window, so each offset reads real data.
const SLACK_WORDS: usize = MAX_WORD_OFFSET + 1;

/// Bit pattern of a fixture's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// SplitMix64 output seeded by the workload seed.
    Random,
    /// Every bit clear.
    AllZero,
    /// Every bit set.
    AllOne,
}

impl Pattern {
    /// Parses the plan's spelling.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "random" => Ok(Self::Random),
            "all_zero" => Ok(Self::AllZero),
            "all_one" => Ok(Self::AllOne),
            other => Err(format!("unknown pattern {other:?}")),
        }
    }
}

/// A window of `len` words whose first word sits `word_offset` words past a
/// 32-byte boundary.
///
/// The seeded region is `len + 4` words starting at the boundary, so offsets
/// 0 to 3 all select real data and offset 0 is AVX2-vector aligned. The global
/// allocator only guarantees `u64` alignment, so the storage over-allocates one
/// vector and starts the region at the first 32-byte aligned word.
pub struct Fixture {
    storage: Vec<u64>,
    start: usize,
    len: usize,
}

impl Fixture {
    /// Builds the window, rejecting an offset outside `0..=3`.
    pub fn new(
        len: usize,
        seed: u64,
        pattern: Pattern,
        word_offset: usize,
    ) -> Result<Self, String> {
        let mut fixture = Self::zeroed(len, word_offset)?;
        let region = &mut fixture.storage[fixture.start - word_offset..][..len + SLACK_WORDS];
        match pattern {
            Pattern::Random => {
                let mut generator = SplitMix64::new(seed);
                region.fill_with(|| generator.next_u64());
            }
            // Explicit stores give a large all-zero window its own physical
            // pages instead of the kernel's shared zero page.
            Pattern::AllZero => region.fill(0),
            Pattern::AllOne => region.fill(u64::MAX),
        }
        Ok(fixture)
    }

    /// Copies `words` into a window at `word_offset`; the slack stays zero.
    pub fn from_slice(words: &[u64], word_offset: usize) -> Result<Self, String> {
        let mut fixture = Self::zeroed(words.len(), word_offset)?;
        fixture.storage[fixture.start..fixture.start + words.len()].copy_from_slice(words);
        Ok(fixture)
    }

    fn zeroed(len: usize, word_offset: usize) -> Result<Self, String> {
        if word_offset > MAX_WORD_OFFSET {
            return Err(format!(
                "word_offset {word_offset} is outside 0..={MAX_WORD_OFFSET}"
            ));
        }
        let region = len
            .checked_add(SLACK_WORDS)
            .ok_or_else(|| "fixture length overflows".to_owned())?;
        let storage = vec![0_u64; region + VECTOR_BYTES / WORD_BYTES];
        let pad = storage.as_ptr().align_offset(VECTOR_BYTES);
        debug_assert!(pad < VECTOR_BYTES / WORD_BYTES);
        Ok(Self {
            storage,
            start: pad + word_offset,
            len,
        })
    }

    /// The window the arms read.
    pub fn words(&self) -> &[u64] {
        &self.storage[self.start..self.start + self.len]
    }

    /// Bytes between the window start and the previous 32-byte boundary.
    pub fn misalignment_bytes(&self) -> usize {
        self.words().as_ptr() as usize % VECTOR_BYTES
    }
}
