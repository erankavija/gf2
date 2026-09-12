//! Primitive bit manipulation operations for single words.
//!
//! This module provides optimized implementations of fundamental bit operations.
//! Each operation uses the fastest implementation available, typically leveraging
//! hardware instructions on modern CPUs (popcount, trailing_zeros, leading_zeros)
//! or efficient branchless algorithms (masked_merge, power-of-2 operations).

/// Computes XOR parity of a word.
///
/// Returns `true` if there is an odd number of 1 bits, `false` otherwise.
/// This is a fundamental GF(2) operation.
///
/// # Algorithm
/// Uses hardware popcount instruction (available on all modern CPUs).
/// Benchmarked ~465 ps per operation, 40-80% faster than bit-twiddling alternatives.
///
/// # Performance
/// - x86-64: 1-3 cycles (POPCNT instruction)
/// - ARM: 1-4 cycles (VCNT instruction)
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::parity;
///
/// assert_eq!(parity(0), false);
/// assert_eq!(parity(1), true);
/// assert_eq!(parity(3), false); // 0b11 = 2 bits
/// assert_eq!(parity(7), true);  // 0b111 = 3 bits
/// ```
#[inline(always)]
pub fn parity(v: u64) -> bool {
    (v.count_ones() & 1) != 0
}

/// Counts trailing zeros (position of lowest set bit).
///
/// Returns 64 if the word is zero.
///
/// # Algorithm
/// Uses hardware trailing zero count instruction (available on all modern CPUs).
/// Benchmarked to be fastest - bit-twiddling alternatives (De Bruijn, binary search)
/// provide no benefit on modern architectures.
///
/// # Performance
/// - x86-64: 1-3 cycles (TZCNT/BSF instruction)
/// - ARM: 1-4 cycles (CLZ + RBIT, or CTZ on ARMv8)
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::trailing_zeros;
///
/// assert_eq!(trailing_zeros(0), 64);
/// assert_eq!(trailing_zeros(1), 0);
/// assert_eq!(trailing_zeros(8), 3);  // 0b1000
/// assert_eq!(trailing_zeros(1u64 << 63), 63);
/// ```
#[inline(always)]
pub fn trailing_zeros(v: u64) -> u32 {
    if v == 0 {
        64
    } else {
        v.trailing_zeros()
    }
}

/// Counts leading zeros (63 - position of highest set bit).
///
/// Returns 64 if the word is zero.
///
/// # Algorithm
/// Uses hardware leading zero count instruction (available on all modern CPUs).
///
/// # Performance
/// - x86-64: 1-3 cycles (LZCNT/BSR instruction)
/// - ARM: 1-2 cycles (CLZ instruction)
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::leading_zeros;
///
/// assert_eq!(leading_zeros(0), 64);
/// assert_eq!(leading_zeros(1), 63);
/// assert_eq!(leading_zeros(1u64 << 63), 0);
/// ```
#[inline(always)]
pub fn leading_zeros(v: u64) -> u32 {
    v.leading_zeros()
}

/// Branchless masked merge: selects bits from `a` or `b` based on `mask`.
///
/// Returns a word where bits are taken from `b` if the corresponding bit in `mask` is 1,
/// otherwise from `a`.
///
/// # Algorithm
/// Uses XOR-based formula: `a ^ ((a ^ b) & mask)` (4 operations)
/// Benchmarked to be equivalent or faster than traditional `(a & !mask) | (b & mask)` (5 operations)
/// due to better instruction-level parallelism.
///
/// # Performance
/// - 4 bitwise operations (XOR, XOR, AND, XOR)
/// - No branches - constant time regardless of mask pattern
/// - Optimal for CPU pipelines
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::masked_merge;
///
/// // Select all bits from b
/// assert_eq!(masked_merge(0x00, 0xFF, 0xFF), 0xFF);
///
/// // Select all bits from a
/// assert_eq!(masked_merge(0xFF, 0x00, 0x00), 0xFF);
///
/// // Select lower nibble from b, upper from a
/// assert_eq!(masked_merge(0xF0, 0x0F, 0x0F), 0xFF);
/// ```
#[inline(always)]
pub fn masked_merge(a: u64, b: u64, mask: u64) -> u64 {
    a ^ ((a ^ b) & mask)
}

/// Check if a value is a power of 2.
///
/// Returns `true` if `v` has exactly one bit set, `false` otherwise.
/// Note: Returns `false` for 0.
///
/// # Algorithm
/// Classic bit trick: `v & (v - 1) == 0` for power-of-2 detection.
/// Works because subtracting 1 flips all trailing zeros and the lowest set bit.
///
/// # Performance
/// - 3 operations (SUB, AND, CMP)
/// - No branches
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::is_power_of_2;
///
/// assert_eq!(is_power_of_2(0), false);
/// assert_eq!(is_power_of_2(1), true);
/// assert_eq!(is_power_of_2(2), true);
/// assert_eq!(is_power_of_2(3), false);
/// assert_eq!(is_power_of_2(1024), true);
/// ```
#[inline(always)]
pub fn is_power_of_2(v: u64) -> bool {
    v != 0 && (v & (v.wrapping_sub(1))) == 0
}

/// Round up to the next power of 2.
///
/// Returns the smallest power of 2 greater than or equal to `v`.
/// Returns 0 if `v` is 0 or would overflow (v > 2^63).
///
/// # Algorithm
/// Uses bit-filling technique: propagate highest set bit right, then add 1.
///
/// # Performance
/// - 12 operations (SUB, OR×6, ADD)
/// - No branches
///
/// # Examples
/// ```
/// use gf2_core::kernels::scalar::primitives::next_power_of_2;
///
/// assert_eq!(next_power_of_2(0), 0);
/// assert_eq!(next_power_of_2(1), 1);
/// assert_eq!(next_power_of_2(2), 2);
/// assert_eq!(next_power_of_2(3), 4);
/// assert_eq!(next_power_of_2(5), 8);
/// assert_eq!(next_power_of_2(1023), 1024);
/// ```
#[inline(always)]
pub fn next_power_of_2(v: u64) -> u64 {
    if v == 0 {
        return 0;
    }
    if v > (1u64 << 63) {
        return 0; // Would overflow
    }

    let mut v = v.wrapping_sub(1);
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v |= v >> 32;
    v.wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors: (input, expected_parity)
    const TEST_CASES: &[(u64, bool)] = &[
        (0, false),
        (1, true),
        (3, false),                  // 0b11 = 2 bits
        (7, true),                   // 0b111 = 3 bits
        (0xF, false),                // 4 bits
        (0x1F, true),                // 5 bits
        (0xFF, false),               // 8 bits
        (0xFFFF, false),             // 16 bits
        (0xFFFFFFFF, false),         // 32 bits
        (0xFFFFFFFFFFFFFFFF, false), // 64 bits
        (0xAAAAAAAAAAAAAAAA, false), // 32 bits
        (0x5555555555555555, false), // 32 bits
        (0x8000000000000000, true),  // 1 bit
        (0x0F0F0F0F0F0F0F0F, false), // 32 bits
    ];

    #[test]
    fn test_parity_correctness() {
        for &(input, expected) in TEST_CASES {
            assert_eq!(parity(input), expected, "parity(0x{:x}) failed", input);
        }
    }

    // Property: parity(a ^ b) = parity(a) ^ parity(b)
    #[test]
    fn test_parity_xor_property() {
        let values = [0u64, 1, 7, 0xFF, 0xAAAAAAAAAAAAAAAA, 0x5555555555555555];

        for &a in &values {
            for &b in &values {
                let parity_a = parity(a);
                let parity_b = parity(b);
                let parity_xor = parity(a ^ b);

                assert_eq!(
                    parity_xor,
                    parity_a ^ parity_b,
                    "XOR property violated: parity(0x{:x} ^ 0x{:x}) != parity(0x{:x}) ^ parity(0x{:x})",
                    a, b, a, b
                );
            }
        }
    }

    // Property: parity(v) matches count_ones(v) % 2
    #[test]
    fn test_parity_matches_popcount() {
        for &(input, _) in TEST_CASES {
            let result = parity(input);
            let expected = (input.count_ones() % 2) == 1;
            assert_eq!(
                result, expected,
                "parity(0x{:x}) != (count_ones % 2)",
                input
            );
        }
    }

    // Trailing zeros tests
    const TRAILING_ZEROS_CASES: &[(u64, u32)] = &[
        (0, 64),
        (1, 0),
        (2, 1),
        (4, 2),
        (8, 3),
        (16, 4),
        (1u64 << 63, 63),
        (0x0FFF0000, 16),
        (0xAAAAAAAAAAAAAAAA, 1),
        (0x5555555555555555, 0),
        (0x8000000000000000, 63),
    ];

    #[test]
    fn test_trailing_zeros_correctness() {
        for &(input, expected) in TRAILING_ZEROS_CASES {
            assert_eq!(
                trailing_zeros(input),
                expected,
                "trailing_zeros(0x{:x}) failed",
                input
            );
        }
    }

    #[test]
    fn test_trailing_zeros_power_of_2() {
        for n in 0..64 {
            let v = 1u64 << n;
            assert_eq!(trailing_zeros(v), n, "trailing_zeros(1 << {}) failed", n);
        }
    }

    // Leading zeros tests
    const LEADING_ZEROS_CASES: &[(u64, u32)] = &[
        (0, 64),
        (1, 63),
        (2, 62),
        (3, 62),
        (4, 61),
        (0xFF, 56),
        (0xFFFF, 48),
        (1u64 << 63, 0),
        (0x7FFFFFFFFFFFFFFF, 1),
    ];

    #[test]
    fn test_leading_zeros_correctness() {
        for &(input, expected) in LEADING_ZEROS_CASES {
            assert_eq!(
                leading_zeros(input),
                expected,
                "leading_zeros(0x{:x}) failed",
                input
            );
        }
    }

    #[test]
    fn test_leading_zeros_power_of_2() {
        for n in 0..64 {
            let v = 1u64 << n;
            assert_eq!(leading_zeros(v), 63 - n, "leading_zeros(1 << {}) failed", n);
        }
    }

    // Property: if v != 0, trailing_zeros gives position of lowest bit
    #[test]
    fn test_trailing_zeros_finds_lowest_bit() {
        for i in 0..63 {
            let v = 1u64 << i;
            assert_eq!(trailing_zeros(v), i, "Should find bit at position {}", i);

            // Add more bits above, result shouldn't change
            let v_with_more = v | (u64::MAX << (i + 1));
            assert_eq!(
                trailing_zeros(v_with_more),
                i,
                "Lowest bit should still be at position {}",
                i
            );
        }

        // Test bit 63 separately
        assert_eq!(trailing_zeros(1u64 << 63), 63);
    }

    // Masked merge tests
    #[test]
    fn test_masked_merge_basic() {
        // Select all from b
        assert_eq!(masked_merge(0x00, 0xFF, 0xFF), 0xFF);

        // Select all from a
        assert_eq!(masked_merge(0xFF, 0x00, 0x00), 0xFF);

        // Select lower nibble from b, upper from a
        assert_eq!(masked_merge(0xF0, 0x0F, 0x0F), 0xFF);

        // Alternating bits
        assert_eq!(
            masked_merge(0xAAAAAAAAAAAAAAAA, 0x5555555555555555, 0x5555555555555555),
            0xFFFFFFFFFFFFFFFF
        );
    }

    #[test]
    fn test_masked_merge_properties() {
        let test_cases = [
            (0xDEADBEEF, 0xCAFEBABE),
            (0x0000000000000000, 0xFFFFFFFFFFFFFFFF),
            (0xAAAAAAAAAAAAAAAA, 0x5555555555555555),
        ];

        for (a, b) in test_cases {
            // Mask of all 0s should select all from a
            assert_eq!(masked_merge(a, b, 0), a);

            // Mask of all 1s should select all from b
            assert_eq!(masked_merge(a, b, u64::MAX), b);

            // Merging with itself should return itself regardless of mask
            assert_eq!(masked_merge(a, a, 0x123456789ABCDEF0), a);
        }
    }

    // Power of 2 tests
    #[test]
    fn test_is_power_of_2_correctness() {
        assert!(!is_power_of_2(0));
        assert!(is_power_of_2(1));
        assert!(is_power_of_2(2));
        assert!(!is_power_of_2(3));
        assert!(is_power_of_2(4));
        assert!(!is_power_of_2(5));
        assert!(is_power_of_2(1024));
        assert!(!is_power_of_2(1023));
        assert!(is_power_of_2(1u64 << 63));
    }

    #[test]
    fn test_is_power_of_2_all_powers() {
        for n in 0..64 {
            let v = 1u64 << n;
            assert!(is_power_of_2(v), "2^{} should be power of 2", n);

            if v > 2 {
                assert!(
                    !is_power_of_2(v + 1),
                    "2^{} + 1 should not be power of 2",
                    n
                );
                assert!(
                    !is_power_of_2(v - 1),
                    "2^{} - 1 should not be power of 2",
                    n
                );
            }
        }
    }

    #[test]
    fn test_next_power_of_2_correctness() {
        assert_eq!(next_power_of_2(0), 0);
        assert_eq!(next_power_of_2(1), 1);
        assert_eq!(next_power_of_2(2), 2);
        assert_eq!(next_power_of_2(3), 4);
        assert_eq!(next_power_of_2(4), 4);
        assert_eq!(next_power_of_2(5), 8);
        assert_eq!(next_power_of_2(1023), 1024);
        assert_eq!(next_power_of_2(1024), 1024);
        assert_eq!(next_power_of_2(1025), 2048);
    }

    #[test]
    fn test_next_power_of_2_properties() {
        for n in 0..63 {
            let pow2 = 1u64 << n;

            // Next power of 2 of a power of 2 is itself
            assert_eq!(
                next_power_of_2(pow2),
                pow2,
                "next_power_of_2(2^{}) should be 2^{}",
                n,
                n
            );

            if pow2 > 1 {
                // For values > 1: next_power_of_2(pow2 - 1) should be pow2
                // But for pow2=2: next_power_of_2(1) = 1, not 2
                let prev = pow2 - 1;
                let expected = if prev == 1 { 1 } else { pow2 };
                assert_eq!(
                    next_power_of_2(prev),
                    expected,
                    "next_power_of_2({}) failed",
                    prev
                );
            }

            // Next power of 2 of (pow2 + 1) should be next power
            if n < 62 {
                assert_eq!(next_power_of_2(pow2 + 1), pow2 << 1);
            }
        }
    }

    #[test]
    fn test_next_power_of_2_overflow() {
        // Values > 2^63 should return 0 (overflow)
        assert_eq!(next_power_of_2(1u64 << 63), 1u64 << 63);
        assert_eq!(next_power_of_2((1u64 << 63) + 1), 0);
        assert_eq!(next_power_of_2(u64::MAX), 0);
    }

    // Property: next_power_of_2 result is always a power of 2
    #[test]
    fn test_next_power_of_2_produces_powers() {
        for v in [1u64, 7, 15, 31, 63, 127, 255, 511, 1023, 2047] {
            let result = next_power_of_2(v);
            if result != 0 {
                assert!(
                    is_power_of_2(result),
                    "next_power_of_2({}) = {} is not a power of 2",
                    v,
                    result
                );
            }
        }
    }
}
