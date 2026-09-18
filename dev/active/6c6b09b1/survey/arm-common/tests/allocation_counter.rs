//! Witness for the counting allocator an arm reports its per-call allocation
//! count through.
//!
//! A global allocator is a process-wide choice and the counters are
//! process-wide too, so this witness owns its own test binary: a sibling test
//! allocating on another thread would enter the armed section's count.

use byte_field_arm_common::alloc::{allocations_during, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// The counters see exactly the armed section: one exact reservation inside it
/// is one call for its own byte count, and a reservation outside it is invisible.
#[test]
fn the_counters_see_exactly_their_armed_section() {
    let mut before: Vec<u8> = Vec::new();
    before.reserve_exact(64);
    std::hint::black_box(&before);

    let armed = allocations_during(|| {
        let mut inside: Vec<u8> = Vec::new();
        inside.reserve_exact(1024);
        std::hint::black_box(&inside);
    });
    assert_eq!(armed.calls, 1);
    assert_eq!(armed.bytes, 1024);

    let quiet = allocations_during(|| {});
    assert_eq!(quiet.calls, 0);
    assert_eq!(quiet.bytes, 0);
}
