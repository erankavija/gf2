//! Byte views of `Gf256` slices, so the GF(2^8) hooks hand element storage
//! to the byte-region kernels without copying.
//!
//! This module is the one exception to gf2-core's rule that only the kernel
//! crates contain `unsafe`, by owner decision of 2026-11-10: the orphan rule
//! keeps the field arithmetic of a one-byte element in gf2-core, and copying
//! elements into bytes and back halved the generic API's throughput. It holds
//! these two views and nothing else.
#![allow(unsafe_code)]

use super::gf256::Gf256;
use super::Gf2mWideConfig;

/// `Gf256<Cfg>` has the size and alignment of `u8`.
const fn layout_matches<Cfg: Gf2mWideConfig<1>>() -> bool {
    core::mem::size_of::<Gf256<Cfg>>() == 1 && core::mem::align_of::<Gf256<Cfg>>() == 1
}

/// The bytes of `elements`, one per element, in order.
pub(super) fn as_bytes<Cfg: Gf2mWideConfig<1>>(elements: &[Gf256<Cfg>]) -> &[u8] {
    const { assert!(layout_matches::<Cfg>()) };
    // SAFETY: `Gf256<Cfg>` is `#[repr(transparent)]` over its `u8` value; its
    // only other field is a zero-sized `PhantomData`, so it has the size,
    // alignment and bit validity of `u8` (asserted above). The returned slice
    // covers the same `elements.len()` bytes and borrows `elements`.
    unsafe { core::slice::from_raw_parts(elements.as_ptr().cast::<u8>(), elements.len()) }
}

/// The bytes of `elements`, one per element, in order, writable.
pub(super) fn as_bytes_mut<Cfg: Gf2mWideConfig<1>>(elements: &mut [Gf256<Cfg>]) -> &mut [u8] {
    const { assert!(layout_matches::<Cfg>()) };
    // SAFETY: as in `as_bytes`; in addition every `u8` bit pattern is a valid
    // `Gf256<Cfg>`, the polynomial-basis coefficients of an element of
    // GF(2^8), so any byte written through the view leaves a valid element.
    // The returned slice holds the unique borrow of `elements`.
    unsafe { core::slice::from_raw_parts_mut(elements.as_mut_ptr().cast::<u8>(), elements.len()) }
}
