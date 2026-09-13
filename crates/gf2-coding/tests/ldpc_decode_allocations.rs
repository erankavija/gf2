//! Heap-allocation census of LDPC decoding (jit:07ca8585, REQ-07).
//!
//! This test binary installs a counting global allocator that forwards every
//! request to the system allocator and records it only while a flag is set, so
//! a count covers exactly the section between the two stores. The counts are
//! exact and deterministic for a fixed decoder and input; they are not a
//! timing, and this file measures nothing that depends on the host's speed.
//!
//! Three phases are counted separately, because they answer different
//! questions:
//!
//! - **Construction** builds the code's edge layout and every message array,
//!   and allocates. The test records that it does rather than fixing a figure.
//! - **Workspace growth** is the first decode into a caller's codeword buffer,
//!   which grows that buffer to the codeword length once.
//! - **Steady state** is every later decode through the same decoder and the
//!   same prepared buffer at a fixed configuration. It allocates nothing:
//!   no allocation, no reallocation and no deallocation.
//!
//! The counting flag and the counters are thread-local, so a section counts
//! only what the thread running it requests and stays exact while other tests
//! run in parallel in the same process.

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_core::BitVec;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Whether this thread is inside a counted section.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static REALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Whether the calling thread is counting. False during thread-local teardown,
/// when the flag is no longer reachable.
fn counting() -> bool {
    COUNTING.try_with(Cell::get).unwrap_or(false)
}

fn bump(counter: &'static std::thread::LocalKey<Cell<u64>>, by: u64) {
    let _ = counter.try_with(|cell| cell.set(cell.get() + by));
}

/// The system allocator with per-thread request counters that record only while
/// that thread is inside a counted section.
struct Counting;

// SAFETY: every method forwards to `System` with the caller's arguments
// unchanged; the counters are thread-local `Cell`s with a `const` initializer
// and allocate nothing themselves.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            bump(&ALLOCATIONS, 1);
            bump(&BYTES, layout.size() as u64);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if counting() {
            bump(&ALLOCATIONS, 1);
            bump(&BYTES, layout.size() as u64);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if counting() {
            bump(&DEALLOCATIONS, 1);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            bump(&REALLOCATIONS, 1);
            bump(&BYTES, new_size as u64);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// What one counted section requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counts {
    allocations: u64,
    reallocations: u64,
    deallocations: u64,
    bytes: u64,
}

impl Counts {
    fn is_zero(self) -> bool {
        self.allocations == 0 && self.reallocations == 0 && self.deallocations == 0
    }
}

/// Runs `body` with this thread's counters recording, and returns its value
/// beside them.
fn count<T>(body: impl FnOnce() -> T) -> (T, Counts) {
    for counter in [&ALLOCATIONS, &REALLOCATIONS, &DEALLOCATIONS, &BYTES] {
        counter.with(|cell| cell.set(0));
    }
    COUNTING.with(|flag| flag.set(true));
    let value = body();
    COUNTING.with(|flag| flag.set(false));
    let counts = Counts {
        allocations: ALLOCATIONS.with(Cell::get),
        reallocations: REALLOCATIONS.with(Cell::get),
        deallocations: DEALLOCATIONS.with(Cell::get),
        bytes: BYTES.with(Cell::get),
    };
    (value, counts)
}

/// Channel LLRs with both signs and a spread of magnitudes.
fn channel_llrs(n: usize, seed: u64) -> Vec<Llr> {
    // A small integer recurrence, so the inputs are fixed by the seed and the
    // generator itself allocates nothing during a counted section.
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let magnitude = 0.05 + (state >> 40) as f32 / 1e5;
            let sign = if state & (1 << 33) == 0 { 1.0 } else { -1.0 };
            Llr::new(sign * magnitude)
        })
        .collect()
}

const ALGORITHMS: [DecoderAlgorithm; 4] = [
    DecoderAlgorithm::MinSum,
    DecoderAlgorithm::NormalizedMinSum(0.75),
    DecoderAlgorithm::OffsetMinSum(0.5),
    DecoderAlgorithm::SumProduct,
];

fn codes() -> Vec<(&'static str, LdpcCode)> {
    vec![
        (
            "dvb-t2-short-r12",
            LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2),
        ),
        (
            "nr-bg1-z8",
            LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 8)),
        ),
    ]
}

/// REQ-07: repeated decoding with a prepared workspace and a fixed supported
/// configuration performs no heap allocation at all.
#[test]
fn steady_state_decoding_allocates_nothing() {
    for (label, code) in codes() {
        let llrs = channel_llrs(code.n(), 17);
        for algorithm in ALGORITHMS {
            for early_termination in [true, false] {
                let config = DecoderConfig::new(algorithm, early_termination);
                let mut decoder = LdpcDecoder::with_config(code.clone(), config);
                let mut codeword = BitVec::with_capacity(code.n());

                // Prepare the workspace: one decode grows the caller's buffer.
                decoder.decode_codeword_into(&llrs, 8, &mut codeword);

                let context =
                    format!("{label}, {algorithm:?}, early_termination={early_termination}");
                for repeat in 0..3 {
                    let (_, counts) =
                        count(|| decoder.decode_codeword_into(&llrs, 8, &mut codeword));
                    assert!(
                        counts.is_zero(),
                        "steady-state decode {repeat} of {context} requested {counts:?}"
                    );
                }
            }
        }
    }
}

/// Construction and the first prepared decode are measured separately, and both
/// allocate. The test records the fact, not a figure: the sizes follow from the
/// code's dimensions and would be a recorded constant with no independent
/// meaning.
#[test]
fn construction_and_workspace_growth_are_counted_separately() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    let llrs = channel_llrs(code.n(), 23);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);

    let code_for_build = code.clone();
    let (mut decoder, construction) =
        count(move || LdpcDecoder::with_config(code_for_build, config));
    assert!(
        construction.allocations > 0,
        "constructing a decoder allocates its layout and message arrays"
    );

    // An empty buffer: the first decode grows it to the codeword length.
    let mut codeword = BitVec::new();
    let (_, growth) = count(|| decoder.decode_codeword_into(&llrs, 6, &mut codeword));
    assert!(
        growth.allocations + growth.reallocations > 0,
        "the first decode into an empty buffer grows it"
    );
    assert_eq!(codeword.len(), code.n());

    // With the buffer prepared, the next decode allocates nothing.
    let (_, steady) = count(|| decoder.decode_codeword_into(&llrs, 6, &mut codeword));
    assert!(steady.is_zero(), "prepared decode requested {steady:?}");
}

/// A decoder that has been reset returns to the same steady state.
#[test]
fn decoding_after_reset_allocates_nothing() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 8));
    let llrs = channel_llrs(code.n(), 29);
    let config = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
    let mut decoder = LdpcDecoder::with_config(code.clone(), config);
    let mut codeword = BitVec::with_capacity(code.n());
    decoder.decode_codeword_into(&llrs, 8, &mut codeword);

    let (_, counts) = count(|| {
        decoder.reset();
        decoder.decode_codeword_into(&llrs, 8, &mut codeword);
    });
    assert!(counts.is_zero(), "decode after reset requested {counts:?}");
}

/// The owning entry points allocate the vectors they return, which is why the
/// steady-state claim is made about the buffer-writing one.
#[test]
fn owning_entry_points_allocate_what_they_return() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    let llrs = channel_llrs(code.n(), 31);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let mut decoder = LdpcDecoder::with_config(code.clone(), config);
    let mut codeword = BitVec::with_capacity(code.n());
    decoder.decode_codeword_into(&llrs, 6, &mut codeword);

    let (result, counts) = count(|| decoder.decode_to_codeword(&llrs, 6));
    assert_eq!(result.decoded_bits.len(), code.n());
    assert!(
        counts.allocations > 0,
        "decode_to_codeword returns an owned codeword"
    );

    // The first `decode_iterative` also resolves the systematic columns, so a
    // later call is the one that shows the per-call cost.
    decoder.decode_iterative(&llrs, 6);
    let (message, counts) = count(|| decoder.decode_iterative(&llrs, 6));
    assert_eq!(message.decoded_bits.len(), code.k());
    assert!(
        counts.allocations > 0,
        "decode_iterative returns an owned message"
    );
}
