//! Integration tests for kernel backends.
//!
//! These tests verify that all backend implementations produce identical results
//! for the same operations. This ensures correctness across scalar, SIMD, and
//! future GPU/FPGA backends.

use super::Backend;

/// Property: All backends must produce identical results for AND operation.
pub fn test_backend_and_equivalence<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (vec![0xFF, 0xFF], vec![0x0F, 0xF0], vec![0x0F, 0xF0]),
        (
            vec![0xAAAAAAAAAAAAAAAAu64],
            vec![0x5555555555555555u64],
            vec![0x0000000000000000u64],
        ),
        (
            vec![0xFFFFFFFFFFFFFFFFu64],
            vec![0xFFFFFFFFFFFFFFFFu64],
            vec![0xFFFFFFFFFFFFFFFFu64],
        ),
        (vec![0], vec![0], vec![0]),
    ];

    for (mut dst, src, expected) in test_cases {
        backend.and(&mut dst, &src);
        assert_eq!(
            dst,
            expected,
            "Backend {} produced incorrect AND result",
            backend.name()
        );
    }
}

/// Property: All backends must produce identical results for OR operation.
pub fn test_backend_or_equivalence<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (vec![0xF0, 0x0F], vec![0x0F, 0xF0], vec![0xFF, 0xFF]),
        (
            vec![0xAAAAAAAAAAAAAAAAu64],
            vec![0x5555555555555555u64],
            vec![0xFFFFFFFFFFFFFFFFu64],
        ),
        (vec![0], vec![0], vec![0]),
        (vec![0], vec![0xFF], vec![0xFF]),
    ];

    for (mut dst, src, expected) in test_cases {
        backend.or(&mut dst, &src);
        assert_eq!(
            dst,
            expected,
            "Backend {} produced incorrect OR result",
            backend.name()
        );
    }
}

/// Property: All backends must produce identical results for XOR operation.
pub fn test_backend_xor_equivalence<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (vec![0xFF, 0xFF], vec![0x0F, 0xF0], vec![0xF0, 0x0F]),
        (
            vec![0xAAAAAAAAAAAAAAAAu64],
            vec![0xAAAAAAAAAAAAAAAAu64],
            vec![0x0000000000000000u64],
        ),
        (
            vec![0xFFFFFFFFFFFFFFFFu64],
            vec![0xFFFFFFFFFFFFFFFFu64],
            vec![0x0000000000000000u64],
        ),
        (vec![0], vec![0xFF], vec![0xFF]),
    ];

    for (mut dst, src, expected) in test_cases {
        backend.xor(&mut dst, &src);
        assert_eq!(
            dst,
            expected,
            "Backend {} produced incorrect XOR result",
            backend.name()
        );
    }
}

/// Property: All backends must produce identical results for NOT operation.
pub fn test_backend_not_equivalence<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (vec![0xFFFFFFFFFFFFFFFFu64], vec![0x0000000000000000u64]),
        (vec![0x0000000000000000u64], vec![0xFFFFFFFFFFFFFFFFu64]),
        (vec![0xAAAAAAAAAAAAAAAAu64], vec![0x5555555555555555u64]),
        (vec![0x0F0F0F0F0F0F0F0Fu64], vec![0xF0F0F0F0F0F0F0F0u64]),
    ];

    for (mut input, expected) in test_cases {
        backend.not(&mut input);
        assert_eq!(
            input,
            expected,
            "Backend {} produced incorrect NOT result",
            backend.name()
        );
    }
}

/// Property: All backends must produce identical popcount results.
pub fn test_backend_popcount_equivalence<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (vec![0u64], 0u64),
        (vec![0xFFu64], 8u64),
        (vec![0xFFFFFFFFFFFFFFFFu64], 64u64),
        (vec![0xAAAAAAAAAAAAAAAAu64], 32u64),
        (vec![0xFFu64, 0xF0F0F0F0F0F0F0F0u64], 40u64),
        (vec![1u64, 2u64, 4u64, 8u64], 4u64),
    ];

    for (input, expected) in test_cases {
        let result = backend.popcount(&input);
        assert_eq!(
            result,
            expected,
            "Backend {} produced incorrect popcount: expected {}, got {}",
            backend.name(),
            expected,
            result
        );
    }
}

/// Property: XOR parity must satisfy: parity(a ^ b) = parity(a) ^ parity(b).
pub fn test_backend_parity_xor_property<B: Backend + ?Sized>(backend: &B) {
    let test_values = vec![
        0u64,
        1u64,
        3u64,
        7u64,
        0xFFu64,
        0xAAAAAAAAAAAAAAAAu64,
        0x5555555555555555u64,
        0xFFFFFFFFFFFFFFFFu64,
    ];

    for &a in &test_values {
        for &b in &test_values {
            let parity_a = backend.parity(a);
            let parity_b = backend.parity(b);
            let parity_xor = backend.parity(a ^ b);

            assert_eq!(
                parity_xor,
                parity_a ^ parity_b,
                "Backend {} violates XOR parity property: parity(0x{:x} ^ 0x{:x}) != parity(0x{:x}) ^ parity(0x{:x})",
                backend.name(),
                a,
                b,
                a,
                b
            );
        }
    }
}

/// Property: Parity must match count_ones() % 2 == 1.
pub fn test_backend_parity_correctness<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (0u64, false),
        (1u64, true),
        (3u64, false),                  // 0b11 = 2 bits
        (7u64, true),                   // 0b111 = 3 bits
        (0xFu64, false),                // 4 bits
        (0x1Fu64, true),                // 5 bits
        (0xFFu64, false),               // 8 bits
        (0xFFFFFFFFFFFFFFFFu64, false), // 64 bits
        (0xAAAAAAAAAAAAAAAAu64, false), // 32 bits
        (0x8000000000000000u64, true),  // 1 bit
    ];

    for (word, expected) in test_cases {
        let result = backend.parity(word);
        assert_eq!(
            result,
            expected,
            "Backend {} produced incorrect parity for 0x{:x}: expected {}, got {}",
            backend.name(),
            word,
            expected,
            result
        );
    }
}

/// Property: trailing_zeros must return position of lowest set bit.
pub fn test_backend_trailing_zeros_correctness<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (0u64, 64u32),
        (1u64, 0u32),
        (2u64, 1u32),
        (4u64, 2u32),
        (8u64, 3u32),
        (16u64, 4u32),
        (1u64 << 63, 63u32),
        (0x0FFF0000u64, 16u32),
        (0xAAAAAAAAAAAAAAAAu64, 1u32),
        (0x5555555555555555u64, 0u32),
    ];

    for (word, expected) in test_cases {
        let result = backend.trailing_zeros(word);
        assert_eq!(
            result,
            expected,
            "Backend {} produced incorrect trailing_zeros for 0x{:x}: expected {}, got {}",
            backend.name(),
            word,
            expected,
            result
        );
    }
}

/// Property: leading_zeros must return 64 - position_of_highest_bit - 1.
pub fn test_backend_leading_zeros_correctness<B: Backend + ?Sized>(backend: &B) {
    let test_cases = vec![
        (0u64, 64u32),
        (1u64, 63u32),
        (2u64, 62u32),
        (3u64, 62u32),
        (4u64, 61u32),
        (0xFFu64, 56u32),
        (0xFFFFu64, 48u32),
        (1u64 << 63, 0u32),
        (0x7FFFFFFFFFFFFFFFu64, 1u32),
    ];

    for (word, expected) in test_cases {
        let result = backend.leading_zeros(word);
        assert_eq!(
            result,
            expected,
            "Backend {} produced incorrect leading_zeros for 0x{:x}: expected {}, got {}",
            backend.name(),
            word,
            expected,
            result
        );
    }
}

/// Test that operations work correctly on empty slices.
pub fn test_backend_empty_slices<B: Backend + ?Sized>(backend: &B) {
    let mut dst: Vec<u64> = vec![];
    let src: Vec<u64> = vec![];

    backend.and(&mut dst, &src);
    assert_eq!(dst.len(), 0, "AND on empty slices should preserve empty");

    backend.or(&mut dst, &src);
    assert_eq!(dst.len(), 0, "OR on empty slices should preserve empty");

    backend.xor(&mut dst, &src);
    assert_eq!(dst.len(), 0, "XOR on empty slices should preserve empty");

    backend.not(&mut dst);
    assert_eq!(dst.len(), 0, "NOT on empty slice should preserve empty");

    let count = backend.popcount(&src);
    assert_eq!(count, 0, "popcount on empty slice should be 0");
}

/// Test that operations work correctly on single-word slices.
pub fn test_backend_single_word<B: Backend + ?Sized>(backend: &B) {
    let mut dst = vec![0xFFFFFFFFFFFFFFFFu64];
    let src = vec![0x0F0F0F0F0F0F0F0Fu64];

    backend.and(&mut dst, &src);
    assert_eq!(dst, vec![0x0F0F0F0F0F0F0F0Fu64]);

    let mut dst = vec![0xF0F0F0F0F0F0F0F0u64];
    backend.or(&mut dst, &src);
    assert_eq!(dst, vec![0xFFFFFFFFFFFFFFFFu64]);

    let mut dst = vec![0xFFFFFFFFFFFFFFFFu64];
    backend.xor(&mut dst, &src);
    assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64]);

    backend.not(&mut dst);
    assert_eq!(dst, vec![0x0F0F0F0F0F0F0F0Fu64]);
}

/// Test that operations work correctly on large slices (stress test).
pub fn test_backend_large_slice<B: Backend + ?Sized>(backend: &B) {
    const SIZE: usize = 1024;
    let mut dst = vec![0xAAAAAAAAAAAAAAAAu64; SIZE];
    let src = vec![0x5555555555555555u64; SIZE];

    backend.xor(&mut dst, &src);
    assert!(dst.iter().all(|&w| w == 0xFFFFFFFFFFFFFFFFu64));

    backend.not(&mut dst);
    assert!(dst.iter().all(|&w| w == 0u64));

    let count = backend.popcount(&src);
    assert_eq!(count, SIZE as u64 * 32);
}

#[cfg(test)]
mod backend_tests {
    use super::*;
    use crate::kernels::scalar::ScalarBackend;

    // Run all tests on the scalar backend
    #[test]
    fn scalar_and_equivalence() {
        test_backend_and_equivalence(&ScalarBackend);
    }

    #[test]
    fn scalar_or_equivalence() {
        test_backend_or_equivalence(&ScalarBackend);
    }

    #[test]
    fn scalar_xor_equivalence() {
        test_backend_xor_equivalence(&ScalarBackend);
    }

    #[test]
    fn scalar_not_equivalence() {
        test_backend_not_equivalence(&ScalarBackend);
    }

    #[test]
    fn scalar_popcount_equivalence() {
        test_backend_popcount_equivalence(&ScalarBackend);
    }

    #[test]
    fn scalar_parity_xor_property() {
        test_backend_parity_xor_property(&ScalarBackend);
    }

    #[test]
    fn scalar_parity_correctness() {
        test_backend_parity_correctness(&ScalarBackend);
    }

    #[test]
    fn scalar_trailing_zeros_correctness() {
        test_backend_trailing_zeros_correctness(&ScalarBackend);
    }

    #[test]
    fn scalar_leading_zeros_correctness() {
        test_backend_leading_zeros_correctness(&ScalarBackend);
    }

    #[test]
    fn scalar_empty_slices() {
        test_backend_empty_slices(&ScalarBackend);
    }

    #[test]
    fn scalar_single_word() {
        test_backend_single_word(&ScalarBackend);
    }

    #[test]
    fn scalar_large_slice() {
        test_backend_large_slice(&ScalarBackend);
    }

    // SIMD backend tests - only run when SIMD feature is enabled
    #[cfg(feature = "simd")]
    mod simd_tests {
        use super::*;

        #[test]
        fn simd_and_equivalence() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_and_equivalence(backend);
            }
        }

        #[test]
        fn simd_or_equivalence() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_or_equivalence(backend);
            }
        }

        #[test]
        fn simd_xor_equivalence() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_xor_equivalence(backend);
            }
        }

        #[test]
        fn simd_not_equivalence() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_not_equivalence(backend);
            }
        }

        #[test]
        fn simd_popcount_equivalence() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_popcount_equivalence(backend);
            }
        }

        #[test]
        fn simd_parity_xor_property() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_parity_xor_property(backend);
            }
        }

        #[test]
        fn simd_parity_correctness() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_parity_correctness(backend);
            }
        }

        #[test]
        fn simd_trailing_zeros_correctness() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_trailing_zeros_correctness(backend);
            }
        }

        #[test]
        fn simd_leading_zeros_correctness() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_leading_zeros_correctness(backend);
            }
        }

        #[test]
        fn simd_empty_slices() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_empty_slices(backend);
            }
        }

        #[test]
        fn simd_single_word() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_single_word(backend);
            }
        }

        #[test]
        fn simd_large_slice() {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                test_backend_large_slice(backend);
            }
        }
    }
}
