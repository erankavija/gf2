//! One-byte GF(2^8) elements with the modulus fixed at compile time, so that
//! `FieldVec` and `FieldMatrix` over GF(2^8) hold one byte per element.

use super::byte_table::{gemm_region, split_tables, GemmShape, Gf256SplitTables};
use super::{Gf2mWide, Gf2mWideConfig};
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// An element of GF(2^8) under the modulus of `Cfg`, stored in one byte: the
/// coefficients of its polynomial-basis representative, bit `i` for `x^i`.
///
/// `Cfg` is a [`Gf2mWideConfig<1>`] with `M = 8`; `MODULUS[0]` holds the
/// modulus's low eight bits. Every constructor rejects another degree at
/// compile time. Products run through the 8 KiB split tables of the modulus,
/// built once per process. [`crate::field::FieldVec::axpy`] and the dense
/// product of [`crate::field::matrix::FieldMatrix`] run the byte-region kernels
/// `Gf2mField::gf256_axpy` and `Gf2mField::gf256_gemm` use.
#[repr(transparent)]
pub struct Gf256<Cfg: Gf2mWideConfig<1>> {
    value: u8,
    _config: PhantomData<fn() -> Cfg>,
}

impl<Cfg: Gf2mWideConfig<1>> Gf256<Cfg> {
    const DEGREE_IS_8: () = assert!(Cfg::M == 8, "Gf256 needs a configuration of degree 8");

    /// The element whose polynomial-basis coefficients are the bits of `value`.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        let () = Self::DEGREE_IS_8;
        Self {
            value,
            _config: PhantomData,
        }
    }

    /// The polynomial-basis coefficients of the element.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.value
    }

    /// The additive identity.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0)
    }

    /// The multiplicative identity.
    #[must_use]
    pub const fn one() -> Self {
        Self::new(1)
    }

    /// The multiplicative inverse, `None` for zero.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        if self.value == 0 {
            return None;
        }
        // a^254 = a^-1 in the multiplicative group of order 255.
        let tables = Self::tables();
        let mut result = 1u8;
        let mut base = self.value;
        let mut exponent = 254u32;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = tables[usize::from(result)].apply(base);
            }
            base = tables[usize::from(base)].apply(base);
            exponent >>= 1;
        }
        Some(Self::new(result))
    }

    pub(crate) fn tables() -> &'static Gf256SplitTables {
        let () = Self::DEGREE_IS_8;
        split_tables((Cfg::MODULUS[0] & 0xff) as u8)
    }

    fn product(self, rhs: Self) -> Self {
        Self::new(Self::tables()[usize::from(self.value)].apply(rhs.value))
    }
}

impl<Cfg: Gf2mWideConfig<1>> From<Gf2mWide<1, Cfg>> for Gf256<Cfg> {
    fn from(value: Gf2mWide<1, Cfg>) -> Self {
        Self::new(value.words()[0] as u8)
    }
}

impl<Cfg: Gf2mWideConfig<1>> From<Gf256<Cfg>> for Gf2mWide<1, Cfg> {
    fn from(value: Gf256<Cfg>) -> Self {
        Gf2mWide::from_u64(u64::from(value.value))
    }
}

impl<Cfg: Gf2mWideConfig<1>> Clone for Gf256<Cfg> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Cfg: Gf2mWideConfig<1>> Copy for Gf256<Cfg> {}

impl<Cfg: Gf2mWideConfig<1>> PartialEq for Gf256<Cfg> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<Cfg: Gf2mWideConfig<1>> Eq for Gf256<Cfg> {}

impl<Cfg: Gf2mWideConfig<1>> Hash for Gf256<Cfg> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<Cfg: Gf2mWideConfig<1>> fmt::Debug for Gf256<Cfg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Writes `GF(2^8):0x<value>`, the form of `Gf2mWide`.
impl<Cfg: Gf2mWideConfig<1>> fmt::Display for Gf256<Cfg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GF(2^8):0x{:x}", self.value)
    }
}

macro_rules! by_value_and_reference {
    ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:expr) => {
        impl<Cfg: Gf2mWideConfig<1>> $trait for Gf256<Cfg> {
            type Output = Self;
            fn $method(self, rhs: Self) -> Self {
                $op(self, rhs)
            }
        }

        impl<Cfg: Gf2mWideConfig<1>> $trait<&Gf256<Cfg>> for Gf256<Cfg> {
            type Output = Self;
            fn $method(self, rhs: &Self) -> Self {
                $op(self, *rhs)
            }
        }

        impl<Cfg: Gf2mWideConfig<1>> $assign_trait for Gf256<Cfg> {
            fn $assign_method(&mut self, rhs: Self) {
                *self = $op(*self, rhs);
            }
        }

        impl<Cfg: Gf2mWideConfig<1>> $assign_trait<&Gf256<Cfg>> for Gf256<Cfg> {
            fn $assign_method(&mut self, rhs: &Self) {
                *self = $op(*self, *rhs);
            }
        }
    };
}

fn xor<Cfg: Gf2mWideConfig<1>>(a: Gf256<Cfg>, b: Gf256<Cfg>) -> Gf256<Cfg> {
    Gf256::new(a.value ^ b.value)
}

/// # Panics
///
/// Panics when `b` is zero.
fn quotient<Cfg: Gf2mWideConfig<1>>(a: Gf256<Cfg>, b: Gf256<Cfg>) -> Gf256<Cfg> {
    a.product(b.inverse().expect("division by zero in GF(2^8)"))
}

by_value_and_reference!(Add, add, AddAssign, add_assign, xor);
by_value_and_reference!(Sub, sub, SubAssign, sub_assign, xor);
by_value_and_reference!(Mul, mul, MulAssign, mul_assign, Gf256::product);
by_value_and_reference!(Div, div, DivAssign, div_assign, quotient);

impl<Cfg: Gf2mWideConfig<1>> Neg for Gf256<Cfg> {
    type Output = Self;
    fn neg(self) -> Self {
        self
    }
}

/// Elements one step of the axpy hook copies into bytes.
const CHUNK: usize = 1024;

impl<Cfg: Gf2mWideConfig<1>> crate::field::FiniteField for Gf256<Cfg> {
    type Characteristic = u64;

    /// XOR addition never overflows, so sums of products need no reduction.
    type Wide = Self;

    fn characteristic(&self) -> u64 {
        2
    }

    fn extension_degree(&self) -> usize {
        8
    }

    fn is_zero(&self) -> bool {
        self.value == 0
    }

    fn is_one(&self) -> bool {
        self.value == 1
    }

    fn inv(&self) -> Option<Self> {
        self.inverse()
    }

    fn zero_like(&self) -> Self {
        Self::zero()
    }

    fn one_like(&self) -> Self {
        Self::one()
    }

    #[inline]
    fn zero_hint() -> Option<Self> {
        Some(Self::zero())
    }

    #[inline]
    fn cardinality_log2_hint() -> Option<u32> {
        Some(8)
    }

    fn to_wide(&self) -> Self {
        *self
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self {
        self.product(*rhs)
    }

    fn reduce_wide(wide: &Self) -> Self {
        *wide
    }

    fn max_unreduced_additions() -> usize {
        usize::MAX
    }

    /// One table lookup per term, the tables fetched once.
    fn try_simd_dot_product(a: &[Self], b: &[Self]) -> Option<Self> {
        let tables = Self::tables();
        let sum = a.iter().zip(b).fold(0u8, |sum, (x, y)| {
            sum ^ tables[usize::from(x.value)].apply(y.value)
        });
        Some(Self::new(sum))
    }

    /// Multiplies stack copies of [`CHUNK`] source bytes through the byte
    /// kernel, then XORs the products into `y` in one pass; a call allocates
    /// nothing.
    fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {
        let table = &Self::tables()[usize::from(a.value)];
        let mul = crate::gf256_region().mul;
        let (mut sources, mut products) = ([0u8; CHUNK], [0u8; CHUNK]);
        for (y, x) in y.chunks_mut(CHUNK).zip(x.chunks(CHUNK)) {
            let (sources, products) = (&mut sources[..x.len()], &mut products[..x.len()]);
            for (byte, element) in sources.iter_mut().zip(x) {
                *byte = element.value;
            }
            mul(table, sources, products);
            for (element, &product) in y.iter_mut().zip(products.iter()) {
                element.value ^= product;
            }
        }
        true
    }

    /// Runs the blocked byte product of `Gf2mField::gf256_gemm`.
    fn try_simd_gemm_classical(
        a: &[Self],
        b_t: &[Self],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [Self],
    ) -> bool {
        gemm_region(
            a,
            b_t,
            &GemmShape { m, k, n },
            out,
            Self::tables(),
            |element| element.value,
            |element, value| element.value = value,
        );
        true
    }
}

impl<Cfg: Gf2mWideConfig<1>> crate::field::ConstField for Gf256<Cfg> {
    fn zero() -> Self {
        Gf256::zero()
    }

    fn one() -> Self {
        Gf256::one()
    }

    fn order() -> u128 {
        256
    }

    fn order_log2() -> u32 {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Poly11d;
    impl Gf2mWideConfig<1> for Poly11d {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1d];
    }

    #[test]
    fn an_element_is_one_byte() {
        assert_eq!(core::mem::size_of::<Gf256<Poly11d>>(), 1);
    }

    #[test]
    fn every_product_and_inverse_matches_the_wide_representation() {
        for a in 0..=255u8 {
            let (narrow, wide) = (
                Gf256::<Poly11d>::new(a),
                Gf2mWide::<1, Poly11d>::from_u64(a.into()),
            );
            assert_eq!(narrow.inverse().map(Gf2mWide::from), wide.inverse());
            for b in 0..=255u8 {
                let product = narrow * Gf256::new(b);
                assert_eq!(Gf2mWide::from(product), wide * Gf2mWide::from_u64(b.into()));
            }
        }
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn division_by_zero_panics() {
        let _ = Gf256::<Poly11d>::one() / Gf256::zero();
    }
}
