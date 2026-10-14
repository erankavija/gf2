//! Heap-request counting for a section of the calling thread. The counting
//! allocator records requests only while a thread-local flag is set, so a
//! count excludes the test harness's own threads and tests running in
//! parallel.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static REALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Whether the calling thread is counting. False during thread-local teardown,
/// when the flag is unreachable.
fn counting() -> bool {
    COUNTING.try_with(Cell::get).unwrap_or(false)
}

fn bump(counter: &'static std::thread::LocalKey<Cell<u64>>, by: u64) {
    let _ = counter.try_with(|cell| cell.set(cell.get() + by));
}

/// Global allocator that forwards to [`System`] and records requests made by
/// threads inside [`count`]. A test binary installs it with
/// `#[global_allocator]`; without that static, [`count`] reports zeros.
pub struct CountingAllocator;

// SAFETY: every method forwards to `System` with the caller's arguments
// unchanged; the counters are thread-local `Cell`s with a `const` initializer
// and allocate nothing themselves.
unsafe impl GlobalAlloc for CountingAllocator {
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

/// What one counted section requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    pub bytes: u64,
}

impl Counts {
    pub fn is_zero(self) -> bool {
        self.allocations == 0 && self.reallocations == 0 && self.deallocations == 0
    }
}

/// Runs `body` with this thread's counters recording, and returns its value
/// beside them.
pub fn count<T>(body: impl FnOnce() -> T) -> (T, Counts) {
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
