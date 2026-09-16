//! Allocation witness for the GF(2^8) cached-table axpy lane.
//!
//! The lane's whole claim is that the table is cached rather than built per
//! call and that destinations are updated in place, so a call allocates
//! nothing. Witnessing that from outside the crate takes a counting global
//! allocator, a global allocator is a process-wide choice, and the counter is
//! process-wide too, so this witness owns its own test binary.
//!
//! `unsafe` appears here for the [`GlobalAlloc`] implementation alone. This is
//! a test binary; `gf2-core` keeps `#![deny(unsafe_code)]`.

#![cfg(feature = "test-support")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use gf2_core::field::FieldVec;
use gf2_core::gf2m::{last_gf256_axpy_lane, Gf2mField, Gf2mWide, Gf2mWideConfig, GF256_TABLE_LANE};

/// Forwards to the system allocator and counts every allocating call made
/// while [`ARMED`] is set.
struct CountingAllocator;

/// Whether allocations are currently being counted.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Allocating calls observed since the counter was last reset.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

fn record() {
    if ARMED.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards its arguments unchanged to the system
// allocator, which satisfies the `GlobalAlloc` contract, and the counter
// touches no allocator of its own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `body` with the counter armed and returns how many allocating calls it
/// made.
fn allocations_during(body: impl FnOnce()) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    body();
    ARMED.store(false, Ordering::Relaxed);
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// GF(2^8) under `x^8 + x^4 + x^3 + x^2 + 1` (0x11D).
struct Gf256Poly11dCfg;
impl Gf2mWideConfig<1> for Gf256Poly11dCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1d];
}

#[test]
fn neither_representation_allocates_on_the_table_lane() {
    let field = Gf2mField::new(8, 0x11d);
    let coefficient = field.element(0x53);
    let mut y = FieldVec::from((0..137u64).map(|v| field.element(v & 0xff)).collect::<Vec<_>>());
    let x = FieldVec::from(
        (0..137u64)
            .map(|v| field.element((v * 7 + 3) & 0xff))
            .collect::<Vec<_>>(),
    );

    let wide_coefficient = Gf2mWide::<1, Gf256Poly11dCfg>::from_u64(0x53);
    let mut wide_y = FieldVec::from(
        (0..137u64)
            .map(|v| Gf2mWide::<1, Gf256Poly11dCfg>::from_u64(v & 0xff))
            .collect::<Vec<_>>(),
    );
    let wide_x = FieldVec::from(
        (0..137u64)
            .map(|v| Gf2mWide::<1, Gf256Poly11dCfg>::from_u64((v * 7 + 3) & 0xff))
            .collect::<Vec<_>>(),
    );

    // First touch builds the table and first use initialises the lane witness;
    // both are process-wide and paid once, so they run before the counter arms.
    y.axpy(&coefficient, &x);
    wide_y.axpy(&wide_coefficient, &wide_x);
    assert_eq!(last_gf256_axpy_lane(), GF256_TABLE_LANE);

    let element_allocations = allocations_during(|| y.axpy(&coefficient, &x));
    assert_eq!(last_gf256_axpy_lane(), GF256_TABLE_LANE);
    let wide_allocations = allocations_during(|| wide_y.axpy(&wide_coefficient, &wide_x));
    assert_eq!(last_gf256_axpy_lane(), GF256_TABLE_LANE);

    assert_eq!(element_allocations, 0);
    assert_eq!(wide_allocations, 0);
}
