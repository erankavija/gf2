//! Per-call allocation counting for an arm that reports it.
//!
//! A global allocator is a process-wide choice, so this module supplies the
//! counter and leaves the `#[global_allocator]` declaration to the arm binary
//! that wants it: an arm that reports no allocation count links no counter and
//! keeps the system allocator.
//!
//! [`ARMED`] gates recording, so the counters cost one relaxed load per
//! allocation while they are disarmed. A timed window runs disarmed, which is
//! why an arm probes its allocation count outside the window.
//!
//! `unsafe` appears here for the [`GlobalAlloc`] implementation alone.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Whether allocations are currently being counted.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Allocating calls observed since the counters were last reset.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Bytes requested by those calls.
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Allocating calls made under [`allocations_during`], and the bytes they
/// requested.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocationCount {
    /// Allocating calls, counting each `realloc` once.
    pub calls: usize,
    /// Bytes those calls requested.
    pub bytes: usize,
}

/// Forwards every request to the system allocator, counting it while armed.
pub struct CountingAllocator;

fn record(bytes: usize) {
    if ARMED.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards its arguments unchanged to the system
// allocator, which satisfies the `GlobalAlloc` contract, and the counters
// touch no allocator of their own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

/// Runs `body` with the counters armed and reports what it allocated.
///
/// The count is this thread's plus any other thread's that allocates while the
/// section runs, so an arm probes a single-threaded call.
pub fn allocations_during(body: impl FnOnce()) -> AllocationCount {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    body();
    ARMED.store(false, Ordering::Relaxed);
    AllocationCount {
        calls: ALLOCATIONS.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}
