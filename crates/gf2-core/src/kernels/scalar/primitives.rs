//! Single-word bit primitives.

/// Returns `true` if `v` has an odd number of 1 bits.
#[inline(always)]
pub fn parity(v: u64) -> bool {
    (v.count_ones() & 1) != 0
}

/// Counts trailing zeros (position of lowest set bit); 64 if `v` is zero.
#[inline(always)]
pub fn trailing_zeros(v: u64) -> u32 {
    if v == 0 {
        64
    } else {
        v.trailing_zeros()
    }
}

/// Counts leading zeros (63 - position of highest set bit); 64 if `v` is zero.
#[inline(always)]
pub fn leading_zeros(v: u64) -> u32 {
    v.leading_zeros()
}

/// Takes each bit from `b` where `mask` is 1 and from `a` where it is 0,
/// without a branch.
#[inline(always)]
pub fn masked_merge(a: u64, b: u64, mask: u64) -> u64 {
    a ^ ((a ^ b) & mask)
}

/// Returns `true` if `v` has exactly one bit set.
#[inline(always)]
pub fn is_power_of_2(v: u64) -> bool {
    v != 0 && (v & (v.wrapping_sub(1))) == 0
}

/// Returns the smallest power of 2 greater than or equal to `v`, or 0 if `v`
/// is 0 or exceeds 2^63.
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

    #[test]
    fn test_trailing_zeros_finds_lowest_bit() {
        for i in 0..63 {
            let v = 1u64 << i;
            assert_eq!(trailing_zeros(v), i, "Should find bit at position {}", i);

            let v_with_more = v | (u64::MAX << (i + 1));
            assert_eq!(
                trailing_zeros(v_with_more),
                i,
                "Lowest bit should still be at position {}",
                i
            );
        }

        assert_eq!(trailing_zeros(1u64 << 63), 63);
    }

    #[test]
    fn test_masked_merge_basic() {
        assert_eq!(masked_merge(0x00, 0xFF, 0xFF), 0xFF);

        assert_eq!(masked_merge(0xFF, 0x00, 0x00), 0xFF);

        assert_eq!(masked_merge(0xF0, 0x0F, 0x0F), 0xFF);

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
            assert_eq!(masked_merge(a, b, 0), a);

            assert_eq!(masked_merge(a, b, u64::MAX), b);

            assert_eq!(masked_merge(a, a, 0x123456789ABCDEF0), a);
        }
    }

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

            assert_eq!(
                next_power_of_2(pow2),
                pow2,
                "next_power_of_2(2^{}) should be 2^{}",
                n,
                n
            );

            if pow2 > 1 {
                // next_power_of_2(1) is 1, not 2.
                let prev = pow2 - 1;
                let expected = if prev == 1 { 1 } else { pow2 };
                assert_eq!(
                    next_power_of_2(prev),
                    expected,
                    "next_power_of_2({}) failed",
                    prev
                );
            }

            if n < 62 {
                assert_eq!(next_power_of_2(pow2 + 1), pow2 << 1);
            }
        }
    }

    #[test]
    fn test_next_power_of_2_overflow() {
        assert_eq!(next_power_of_2(1u64 << 63), 1u64 << 63);
        assert_eq!(next_power_of_2((1u64 << 63) + 1), 0);
        assert_eq!(next_power_of_2(u64::MAX), 0);
    }

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
