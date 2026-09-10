//! Sealed trait for integer types used as GF(2^m) element representations.
//!
//! [`UintExt`] abstracts over unsigned integer types (`u8`, `u16`, `u32`, `u64`, `u128`),
//! enabling generic GF(2^m) arithmetic that works at any width while keeping the common
//! `u64` path zero-cost via monomorphization.

use std::fmt::{Binary, Debug, Display};
use std::hash::Hash;
use std::ops::{BitAnd, BitOr, BitXor, BitXorAssign, Shl, Shr};

mod private {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for u128 {}
}

/// Trait for unsigned integer types that can represent GF(2^m) field elements.
///
/// This trait is sealed — only `u8`, `u16`, `u32`, `u64`, and `u128` implement it.
/// It provides the minimal set of operations needed for GF(2^m) arithmetic:
/// bitwise operations, shifts, and conversions.
///
/// # Design
///
/// - **Sealed**: Only primitive unsigned integers make sense as Galois field representations.
/// - **`as_u64_truncated`**: Lossy cast for SIMD dispatch (identity for u64, truncating for u128).
/// - **`from_u64`**: Truncating conversion for smaller types, widening for larger.
pub trait UintExt:
    Copy
    + Clone
    + Eq
    + PartialEq
    + Hash
    + Binary
    + Debug
    + Display
    + Default
    + Send
    + Sync
    + 'static
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + BitXorAssign
    + Shl<u32, Output = Self>
    + Shr<u32, Output = Self>
    + private::Sealed
{
    /// Number of bits in this integer type.
    const BITS: u32;

    /// The zero value.
    const ZERO: Self;

    /// The one value.
    const ONE: Self;

    /// Returns true if this value is zero.
    fn is_zero(self) -> bool;

    /// Returns true if the bit at position `pos` is set.
    fn bit(self, pos: u32) -> bool;

    /// Returns a mask with the lowest `bits` bits set: `(1 << bits) - 1`.
    ///
    /// # Panics
    ///
    /// Panics if `bits > Self::BITS`.
    fn low_mask(bits: u32) -> Self;

    /// Returns the number of ones in the binary representation.
    fn count_ones(self) -> u32;

    /// Returns the number of leading zeros.
    fn leading_zeros(self) -> u32;

    /// Truncating cast to `u64`. Identity for `u64`, truncates for `u128`.
    fn as_u64_truncated(self) -> u64;

    /// Converts from `u64`. Truncates for smaller types, zero-extends for larger.
    fn from_u64(v: u64) -> Self;

    /// Converts to `usize` for table indexing. Truncates if necessary.
    fn to_usize(self) -> usize;

    /// Converts from `u16` (for table lookups).
    fn from_u16(v: u16) -> Self;

    /// True only for `u64`, used for SIMD dispatch without `TypeId`.
    const IS_U64: bool;
}

macro_rules! impl_galois_int {
    ($ty:ty, $bits:expr, $is_u64:expr) => {
        impl UintExt for $ty {
            const BITS: u32 = $bits;
            const ZERO: Self = 0;
            const ONE: Self = 1;
            const IS_U64: bool = $is_u64;

            #[inline(always)]
            fn is_zero(self) -> bool {
                self == 0
            }

            #[inline(always)]
            fn bit(self, pos: u32) -> bool {
                (self >> pos) & 1 != 0
            }

            #[inline(always)]
            fn low_mask(bits: u32) -> Self {
                debug_assert!(bits <= $bits);
                if bits == 0 {
                    0
                } else if bits >= $bits {
                    !0
                } else {
                    (1 << bits) - 1
                }
            }

            #[inline(always)]
            fn count_ones(self) -> u32 {
                <$ty>::count_ones(self)
            }

            #[inline(always)]
            fn leading_zeros(self) -> u32 {
                <$ty>::leading_zeros(self)
            }

            #[inline(always)]
            fn as_u64_truncated(self) -> u64 {
                self as u64
            }

            #[inline(always)]
            fn from_u64(v: u64) -> Self {
                v as Self
            }

            #[inline(always)]
            fn to_usize(self) -> usize {
                self as usize
            }

            #[inline(always)]
            fn from_u16(v: u16) -> Self {
                v as Self
            }
        }
    };
}

impl_galois_int!(u8, 8, false);
impl_galois_int!(u16, 16, false);
impl_galois_int!(u32, 32, false);
impl_galois_int!(u64, 64, true);
impl_galois_int!(u128, 128, false);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_one() {
        assert!(u64::ZERO.is_zero());
        assert!(!u64::ONE.is_zero());
        assert!(u128::ZERO.is_zero());
    }

    #[test]
    fn test_bit() {
        assert!(10u64.bit(1)); // 1010 -> bit 1 is set
        assert!(!10u64.bit(2)); // bit 2 is not set
        assert!(10u64.bit(3)); // bit 3 is set
    }

    #[test]
    fn test_low_mask() {
        assert_eq!(u64::low_mask(0), 0);
        assert_eq!(u64::low_mask(1), 1);
        assert_eq!(u64::low_mask(4), 0b1111);
        assert_eq!(u64::low_mask(64), u64::MAX);
        assert_eq!(u128::low_mask(128), u128::MAX);
        assert_eq!(u8::low_mask(8), u8::MAX);
    }

    #[test]
    fn test_roundtrip_u64() {
        let v = 0xDEAD_BEEF_u64;
        assert_eq!(u64::from_u64(v).as_u64_truncated(), v);
    }

    #[test]
    fn test_from_u64_truncates_for_u8() {
        assert_eq!(u8::from_u64(0x1FF), 0xFF);
    }

    #[test]
    fn test_u128_low_mask() {
        assert_eq!(u128::low_mask(65), (1u128 << 65) - 1);
    }
}
