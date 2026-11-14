//! A counted section records its own thread's heap requests and none of a
//! sibling thread's, even when the sibling allocates throughout the section.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

use thread_allocations::{count, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn sibling_thread_allocations_stay_outside_the_count() {
    static STOP: AtomicBool = AtomicBool::new(false);
    static SIBLING_ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

    let sibling = thread::spawn(|| {
        while !STOP.load(Ordering::Acquire) {
            black_box(vec![0u8; 64]);
            SIBLING_ALLOCATIONS.fetch_add(1, Ordering::Release);
        }
    });

    // Spinning allocates nothing, so the section is quiet on this thread while
    // the sibling completes at least this many allocations inside it.
    let ((), quiet) = count(|| {
        let start = SIBLING_ALLOCATIONS.load(Ordering::Acquire);
        while SIBLING_ALLOCATIONS.load(Ordering::Acquire) < start + 1000 {
            std::hint::spin_loop();
        }
    });
    let (_, own) = count(|| black_box(vec![0u8; 64]));

    STOP.store(true, Ordering::Release);
    sibling.join().expect("sibling thread");

    assert!(quiet.is_zero(), "sibling requests were counted: {quiet:?}");
    assert_eq!(own.allocations, 1, "own request uncounted: {own:?}");
}
