//! Integration tests for backend selection.
//!
//! Verifies that the backend selection logic works correctly with and without
//! the SIMD feature enabled.

use gf2_core::kernels::backend::select_backend_for_size;
use gf2_core::kernels::Backend;

#[test]
fn test_backend_selection_small_buffers() {
    // Small buffers should always use scalar backend
    assert_eq!(select_backend_for_size(0).name(), "scalar");
    assert_eq!(select_backend_for_size(1).name(), "scalar");
    assert_eq!(select_backend_for_size(7).name(), "scalar");
}

#[test]
fn test_backend_selection_threshold() {
    // At threshold (8 words = 512 bytes), should use SIMD if available
    let backend = select_backend_for_size(8);

    #[cfg(feature = "simd")]
    assert_eq!(backend.name(), "simd", "Should select SIMD at threshold");

    #[cfg(not(feature = "simd"))]
    assert_eq!(
        backend.name(),
        "scalar",
        "Should use scalar when SIMD disabled"
    );
}

#[test]
fn test_backend_selection_large_buffers() {
    // Large buffers should use SIMD if available
    let backend16 = select_backend_for_size(16);
    let backend256 = select_backend_for_size(256);
    let backend1024 = select_backend_for_size(1024);

    #[cfg(feature = "simd")]
    {
        assert_eq!(backend16.name(), "simd");
        assert_eq!(backend256.name(), "simd");
        assert_eq!(backend1024.name(), "simd");
    }

    #[cfg(not(feature = "simd"))]
    {
        assert_eq!(backend16.name(), "scalar");
        assert_eq!(backend256.name(), "scalar");
        assert_eq!(backend1024.name(), "scalar");
    }
}

#[test]
#[cfg(feature = "simd")]
fn test_simd_backend_availability() {
    use gf2_core::kernels::simd::maybe_simd;

    // Test that we can query SIMD availability
    // This may be None on CPUs without AVX2/NEON
    let simd = maybe_simd();

    if let Some(backend) = simd {
        println!("SIMD backend available: {}", backend.name());
        assert!(!backend.name().is_empty());
    } else {
        println!("SIMD backend not available (CPU doesn't support required instructions)");
    }
}

#[test]
fn test_operations_work_with_selected_backend() {
    use gf2_core::kernels::ops::{and_inplace, not_inplace, or_inplace, popcount, xor_inplace};

    // Test with small buffer (scalar)
    let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 4];
    let src = vec![0x0F0F0F0F0F0F0F0Fu64; 4];
    xor_inplace(&mut dst, &src);
    assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64; 4]);

    // Test with large buffer (SIMD if available)
    let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 64];
    let src = vec![0x5555555555555555u64; 64];

    and_inplace(&mut dst, &src);
    assert_eq!(dst, vec![0x5555555555555555u64; 64]);

    or_inplace(&mut dst, &vec![0xAAAAAAAAAAAAAAAAu64; 64]);
    assert_eq!(dst, vec![0xFFFFFFFFFFFFFFFFu64; 64]);

    not_inplace(&mut dst);
    assert_eq!(dst, vec![0x0000000000000000u64; 64]);

    let count = popcount(&vec![0xFFFFFFFFFFFFFFFFu64; 64]);
    assert_eq!(count, 64 * 64);
}
