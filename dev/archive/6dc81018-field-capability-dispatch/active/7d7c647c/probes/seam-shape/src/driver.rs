//! Mirrors the block-recursive drivers in `gf2_core::field`, in the two shapes
//! the seam design chooses between.

use crate::field::Selector;

/// Today's shape: the driver reads the associated constant off the dictionary.
pub fn route_const<F: Selector>(n: usize, x: &F) -> u64 {
    if n <= F::THRESHOLD {
        x.value()
    } else {
        x.value() + 1
    }
}

/// The seam's shape: the driver reads a free function in a separate module.
pub fn route_seam<F: Selector>(n: usize, x: &F) -> u64 {
    if n <= crate::tuning::winograd_min_dim() {
        x.value()
    } else {
        x.value() + 1
    }
}

/// Probe M4: reaches `Hooked::value` only, so `has_panel_kernel` is a
/// defaulted method nothing calls and `UNREAD` a constant nothing reads.
pub fn route_hooked<F: crate::field::Hooked>(x: &F) -> u64 {
    x.value()
}

/// Probe M5: a concrete entry point, so the impl rather than the generic
/// driver is the reachable root — the shape `--start-from 'gf2_core::gfp'`
/// takes against the opaque `gf2_core::field`.
pub fn route_concrete(x: &crate::field::Elem) -> u64 {
    crate::field::Hooked::value(x)
}
