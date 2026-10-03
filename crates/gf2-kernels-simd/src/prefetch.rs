//! Safe software-prefetch wrappers, so that safe crates can schedule
//! look-ahead table fetches.

/// Issues a best-effort temporal L1 data prefetch for `ptr`.
///
/// The pointer is never dereferenced by Rust code. Architectures without a
/// stable prefetch intrinsic compile this to a no-op.
#[inline(always)]
pub fn prefetch_read_l1(ptr: *const u8) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::{_mm_prefetch, _MM_HINT_T0};
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};

        // SAFETY: PREFETCHT0 is a hint. It does not dereference `ptr` in Rust
        // and has no architectural effect beyond cache state; invalid pointers
        // are permitted for x86 prefetch instructions.
        unsafe { _mm_prefetch(ptr.cast::<i8>(), _MM_HINT_T0) };
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        let _ = ptr;
    }
}
