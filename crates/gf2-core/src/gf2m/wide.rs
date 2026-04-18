//! Multi-word `GF(2^M)` elements backed by a fixed-size `[u64; N]` array.
//!
//! [`Gf2mWide`] is the const-generic, stack-allocated analogue of
//! [`crate::gf2m::Gf2mElement_`] for extension degrees that do not fit in a
//! single integer register (including the `u128` storage path, which caps at
//! `m <= 127`). Elements are `Copy`, carry their configuration purely at the
//! type level via a zero-sized [`Gf2mWideConfig`] marker, and therefore
//! impose no heap or reference-counting overhead on downstream users.
//!
//! # Scope of this module (Task 1 of story 6fb4abad)
//!
//! This file delivers the **type shell and XOR-only operators**. Specifically
//! implemented here: construction, accessors, `Add`, `Sub`, `Neg`,
//! `AddAssign`, `SubAssign`, and the tail-masking invariant. Multiplication,
//! Barrett reduction, inversion, and `FiniteField` / `ConstField` trait
//! implementations land in follow-up tasks 2–4 of the same story. Treat this
//! module as the algebraic skeleton onto which those pieces will attach.
//!
//! # Tail-masking invariant
//!
//! Every mutating operation on a [`Gf2mWide`] value must leave all bits at
//! positions `>= Cfg::M` in the top word equal to zero. This is the
//! multi-word generalisation of the project-wide tail-masking invariant
//! documented in `CLAUDE.md` ("Key design invariants: Tail masking"). The
//! private helper [`Gf2mWide::mask_tail_in_place`] enforces this. In release
//! builds of XOR-based operations the invariant is preserved automatically
//! because XOR of two zero-tailed operands is zero-tailed; callers that
//! fabricate words from raw input must use [`Gf2mWide::new`] (which masks)
//! rather than [`Gf2mWide::from_words`] (which only debug-asserts).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use super::wide_config::Gf2mWideConfig;

/// A fixed-width element of `GF(2^M)` stored as `N` little-endian `u64` words.
///
/// Bit `i` of the element lives at `words[i >> 6] >> (i & 63) & 1`. Addition
/// (equivalently, subtraction; see below) is word-wise XOR. Negation is
/// identity, because every element of a characteristic-2 field is its own
/// additive inverse.
///
/// The config type `Cfg` encodes the extension degree `M` and the defining
/// polynomial as compile-time constants, so no `Gf2mWide` value carries any
/// runtime field parameters. See [`Gf2mWideConfig`] for the required
/// contract.
///
/// # Invariants
///
/// Bits at positions `>= Cfg::M` in the top word must be zero. This
/// module's constructors (except [`Gf2mWide::from_words`]) and mutating
/// operators preserve this invariant automatically.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
///
/// /// GF(2^256), Seroussi HPL-98-135 Table 1 row m = 256:
/// /// `x^256 + x^10 + x^5 + x^2 + 1`.
/// struct Gf2m256;
/// impl Gf2mWideConfig<4> for Gf2m256 {
///     const M: usize = 256;
///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
/// }
///
/// let a = Gf2mWide::<4, Gf2m256>::from_u64(5);
/// let b = Gf2mWide::<4, Gf2m256>::from_u64(3);
/// let sum = a + b;
/// assert_eq!(sum.words()[0], 5 ^ 3);
/// // Characteristic 2: a + a == 0.
/// assert!((a + a).is_zero());
/// ```
pub struct Gf2mWide<const N: usize, Cfg: Gf2mWideConfig<N>> {
    words: [u64; N],
    _marker: PhantomData<fn() -> Cfg>,
}

// ---------------------------------------------------------------------------
// Basic trait derivations — written by hand so we can keep `Cfg` off the
// `Copy` / `Clone` / `PartialEq` / `Hash` bounds (it is a ZST marker and
// doesn't actually need those traits).
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Copy for Gf2mWide<N, Cfg> {}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Clone for Gf2mWide<N, Cfg> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> PartialEq for Gf2mWide<N, Cfg> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.words == other.words
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Eq for Gf2mWide<N, Cfg> {}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Hash for Gf2mWide<N, Cfg> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.words.hash(state);
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> fmt::Debug for Gf2mWide<N, Cfg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render as `GF(2^M){NAME}(0x<hex words, high-word first>)`. Using
        // high-word-first makes hex dumps read naturally as a single big
        // integer when debugging, while the underlying storage stays
        // little-endian across words.
        write!(f, "GF(2^{}){}", Cfg::M, Cfg::NAME)?;
        write!(f, "(0x")?;
        for (i, w) in self.words.iter().rev().enumerate() {
            if i == 0 {
                // Don't zero-pad the top word — it may have fewer than 64
                // significant bits.
                write!(f, "{:x}", w)?;
            } else {
                write!(f, "_{:016x}", w)?;
            }
        }
        write!(f, ")")
    }
}

// ---------------------------------------------------------------------------
// Construction and accessors
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Gf2mWide<N, Cfg> {
    /// Constructs an element directly from `words`, asserting in debug
    /// builds that the tail above bit `M` is already zero.
    ///
    /// Prefer [`Gf2mWide::new`] if the caller cannot guarantee that the
    /// input is already tail-masked; this constructor is intended for
    /// internal fast paths that re-pack the output of a known-correct
    /// operation (XOR, shift-and-reduce, etc.) where re-masking would be
    /// redundant work.
    ///
    /// # Arguments
    ///
    /// * `words` - Little-endian `[u64; N]` representation of the element.
    ///   Bits at positions `>= Cfg::M` in the top word must be zero.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if any bit at position `>= Cfg::M` in
    /// `words` is set. In release builds this check is elided; callers
    /// must uphold the invariant themselves.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b]; // irreducible placeholder
    /// }
    /// let a = Gf2mWide::<1, Cfg>::from_words([0x1234_5678]);
    /// assert_eq!(a.words()[0], 0x1234_5678);
    /// ```
    #[inline]
    pub const fn from_words(words: [u64; N]) -> Self {
        // We can't call `tail_mask_top_word` in a `const fn` context
        // directly *and* `debug_assert!` the result matches the input in
        // stable Rust 1.80 without monomorphisation surprises. Structure
        // the check as a const-friendly bitwise comparison.
        debug_assert!(
            Self::tail_is_masked(&words),
            "Gf2mWide::from_words: input has non-zero bits at positions >= M; \
             use Gf2mWide::new to mask the tail automatically"
        );
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Constructs an element from `words`, masking any bits at positions
    /// `>= Cfg::M` to zero.
    ///
    /// This is the safe constructor for arbitrary input; it maintains the
    /// tail-masking invariant unconditionally at the cost of one bitwise
    /// AND on the top word.
    ///
    /// # Arguments
    ///
    /// * `words` - Little-endian `[u64; N]` candidate representation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<4> for Cfg {
    ///     const M: usize = 250; // leaves 6 high bits to mask
    ///     const MODULUS: [u64; 4] = [0x1, 0, 0, 0];
    /// }
    /// let a = Gf2mWide::<4, Cfg>::new([u64::MAX; 4]);
    /// // The top 6 bits of the top word must be zero.
    /// assert_eq!(a.words()[3] >> (250 - 64 * 3), 0);
    /// ```
    #[inline]
    pub fn new(mut words: [u64; N]) -> Self {
        Self::mask_tail_in_place(&mut words);
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Returns the additive identity (the zero polynomial).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// assert!(Gf2mWide::<1, Cfg>::zero().is_zero());
    /// ```
    #[inline]
    pub const fn zero() -> Self {
        Gf2mWide {
            words: [0u64; N],
            _marker: PhantomData,
        }
    }

    /// Returns the multiplicative identity (the constant polynomial 1).
    ///
    /// Requires `N >= 1`; `Gf2mWideConfig<0>` is ill-formed anyway
    /// (`64 * (0 - 1)` underflows and `M` must be `<= 0`).
    ///
    /// # Panics
    ///
    /// Panics if `N == 0`. Implementations of [`Gf2mWideConfig`] must
    /// satisfy `N >= 1` by the `64 * (N - 1) < M <= 64 * N` range
    /// constraint.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// assert!(Gf2mWide::<1, Cfg>::one().is_one());
    /// ```
    #[inline]
    pub fn one() -> Self {
        assert!(N >= 1, "Gf2mWide requires N >= 1");
        let mut words = [0u64; N];
        words[0] = 1;
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Constructs an element whose low word is `v` and all higher words
    /// are zero, with the tail above bit `M` masked off.
    ///
    /// Useful for constructing small test values and for implementing
    /// `From<u64>` conversions in follow-up tasks.
    ///
    /// # Arguments
    ///
    /// * `v` - Value to place in the low word.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// let a = Gf2mWide::<1, Cfg>::from_u64(42);
    /// assert_eq!(a.words()[0], 42);
    /// ```
    #[inline]
    pub fn from_u64(v: u64) -> Self {
        let mut words = [0u64; N];
        if N >= 1 {
            words[0] = v;
        }
        // Tail-mask in case M < 64 (e.g., an odd small M in a single-word
        // config) or the caller passes a value above M in later multi-word
        // configurations that legitimately live partly in word 0.
        Self::mask_tail_in_place(&mut words);
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying `[u64; N]` representation.
    ///
    /// Bit `i` of the element lives at `words[i >> 6] >> (i & 63) & 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// let a = Gf2mWide::<1, Cfg>::from_u64(0xabcd);
    /// assert_eq!(a.words(), &[0xabcd]);
    /// ```
    #[inline]
    pub fn words(&self) -> &[u64; N] {
        &self.words
    }

    /// Returns the coefficient of `x^i` in the polynomial representation.
    ///
    /// # Arguments
    ///
    /// * `i` - Bit index, must satisfy `i < Cfg::M`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= Cfg::M`. Reduced elements never have a set bit at
    /// or above position `M`, so the check is a correctness guard.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// let a = Gf2mWide::<1, Cfg>::from_u64(0b1010);
    /// assert!(!a.bit(0));
    /// assert!(a.bit(1));
    /// assert!(!a.bit(2));
    /// assert!(a.bit(3));
    /// ```
    #[inline]
    pub fn bit(&self, i: usize) -> bool {
        assert!(
            i < Cfg::M,
            "Gf2mWide::bit: index {} out of range for GF(2^{})",
            i,
            Cfg::M
        );
        (self.words[i >> 6] >> (i & 63)) & 1 == 1
    }

    /// Returns `true` iff every coefficient is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// assert!(Gf2mWide::<1, Cfg>::zero().is_zero());
    /// assert!(!Gf2mWide::<1, Cfg>::one().is_zero());
    /// ```
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// Returns `true` iff the element equals the multiplicative identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Cfg;
    /// impl Gf2mWideConfig<1> for Cfg {
    ///     const M: usize = 64;
    ///     const MODULUS: [u64; 1] = [0x1b];
    /// }
    /// assert!(Gf2mWide::<1, Cfg>::one().is_one());
    /// assert!(!Gf2mWide::<1, Cfg>::zero().is_one());
    /// ```
    #[inline]
    pub fn is_one(&self) -> bool {
        if N == 0 {
            return false;
        }
        self.words[0] == 1 && self.words[1..].iter().all(|w| *w == 0)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Computes the mask that selects bits `[0, M - 64 * (N - 1))` of the
    /// top word. Bits above this mask are the "tail" that must always be
    /// zero in reduced elements.
    ///
    /// For `M = 64 * N` (the top word is fully used) this returns
    /// `u64::MAX`. For any smaller `M` in the top word, it returns
    /// `(1u64 << k) - 1` where `k = M - 64 * (N - 1)`.
    #[inline]
    const fn top_word_mask() -> u64 {
        let bits_in_top = Cfg::M - 64 * (N - 1);
        // `bits_in_top` is guaranteed to lie in `1..=64` by the
        // `64 * (N - 1) < M <= 64 * N` contract. We still clamp defensively
        // because the contract is informal (nothing in the trait forces
        // implementations to satisfy it).
        if bits_in_top >= 64 {
            u64::MAX
        } else {
            (1u64 << bits_in_top) - 1
        }
    }

    /// Zeros bits at positions `>= Cfg::M` in the top word in place.
    ///
    /// This is the multi-word analogue of the project-wide tail-masking
    /// invariant documented in `CLAUDE.md` ("Key design invariants: Tail
    /// masking"). Every mutating operation that can produce bits at or
    /// above position `M` must call this helper.
    ///
    /// # Arguments
    ///
    /// * `words` - Mutable reference to the `[u64; N]` storage being
    ///   normalised.
    #[inline]
    fn mask_tail_in_place(words: &mut [u64; N]) {
        if N == 0 {
            return;
        }
        let top = N - 1;
        words[top] &= Self::top_word_mask();
    }

    /// Const-evaluable check that `words` has no bits at positions
    /// `>= Cfg::M`.
    ///
    /// Used by the `debug_assert!` in [`Gf2mWide::from_words`]. Kept
    /// separate so that path can stay `const fn` on stable Rust 1.80.
    #[inline]
    const fn tail_is_masked(words: &[u64; N]) -> bool {
        if N == 0 {
            // An `N == 0` config is ill-formed; treat any tail as "masked"
            // rather than emitting a false positive panic here.
            return true;
        }
        let top = N - 1;
        (words[top] & !Self::top_word_mask()) == 0
    }
}

// ---------------------------------------------------------------------------
// Addition / subtraction (word-wise XOR in characteristic 2).
//
// Match the five-variant pattern already established for
// `Gf2mElement_<V>` in `gf2m/field.rs`: `&Self op &Self`, `Self op Self`,
// `Self op &Self`, plus `SubAssign` mirroring `AddAssign`.
// ---------------------------------------------------------------------------

#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Add for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let mut words = self.words;
        for (w, r) in words.iter_mut().zip(rhs.words.iter()) {
            *w ^= *r;
        }
        // XOR of two tail-masked operands is tail-masked; no need to
        // re-mask.
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Add for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self += &rhs;
        self
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Add<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn add(mut self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        self += rhs;
        self
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Sub for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self + rhs
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Sub for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= &rhs;
        self
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Sub<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn sub(mut self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        self -= rhs;
        self
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> AddAssign for Gf2mWide<N, Cfg> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        for (w, r) in self.words.iter_mut().zip(rhs.words.iter()) {
            *w ^= *r;
        }
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> AddAssign<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    #[inline]
    fn add_assign(&mut self, rhs: &Gf2mWide<N, Cfg>) {
        for (w, r) in self.words.iter_mut().zip(rhs.words.iter()) {
            *w ^= *r;
        }
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> SubAssign for Gf2mWide<N, Cfg> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self += rhs;
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl<const N: usize, Cfg: Gf2mWideConfig<N>> SubAssign<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    #[inline]
    fn sub_assign(&mut self, rhs: &Gf2mWide<N, Cfg>) {
        *self += rhs;
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Neg for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn neg(self) -> Self::Output {
        // In characteristic 2, negation is the identity.
        *self
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Neg for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn neg(self) -> Self::Output {
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::op_ref)]
mod tests {
    use super::*;

    /// Test config for GF(2^256) using the pentanomial
    /// `x^256 + x^10 + x^5 + x^2 + 1` from Seroussi, *Table of Low-Weight
    /// Binary Irreducible Polynomials*, HP Laboratories technical report
    /// HPL-98-135 (1998), Table 1 row `m = 256`.
    ///
    /// Low-weight pentanomials are listed by Seroussi with their interior
    /// exponents `(a, b, c)` such that the polynomial is
    /// `x^m + x^a + x^b + x^c + 1`. For `m = 256` the table entry is
    /// `(10, 5, 2)`, giving the polynomial used here.
    ///
    /// # Irreducibility (sage cross-check)
    ///
    /// Regenerate / re-verify with:
    ///
    /// ```text
    /// sage: R.<x> = GF(2)[]
    /// sage: p = x^256 + x^10 + x^5 + x^2 + 1
    /// sage: p.is_irreducible()
    /// True
    /// ```
    ///
    /// (one-off lookup; no dedicated `.sage` script is committed for a
    /// single polynomial.)
    pub(super) struct Gf2m256TestConfig;

    impl Gf2mWideConfig<4> for Gf2m256TestConfig {
        const M: usize = 256;
        // x^10 + x^5 + x^2 + 1 = 1024 + 32 + 4 + 1 = 1061 = 0x425
        const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
        const NAME: &'static str = "Gf2m256TestConfig";
    }

    /// Synthetic test config with `M = 250` to exercise the tail-masking
    /// path (the top word must zero its high 6 bits).
    struct Gf2m250TestConfig;

    impl Gf2mWideConfig<4> for Gf2m250TestConfig {
        const M: usize = 250;
        // Deliberately *not* a real irreducible — this config exists only
        // to test the tail-masking invariant; no multiplicative operation
        // is exercised against it in Task 1.
        const MODULUS: [u64; 4] = [0x1, 0, 0, 0];
    }

    // -----------------------------------------------------------------------
    // Config constants
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_modulus_high_bit_word_and_mask_m256() {
        assert_eq!(Gf2m256TestConfig::MODULUS_HIGH_BIT_WORD, 3);
        assert_eq!(Gf2m256TestConfig::MODULUS_HIGH_BIT_MASK, 1u64 << 63);
    }

    #[test]
    fn test_config_modulus_high_bit_word_and_mask_m250() {
        assert_eq!(Gf2m250TestConfig::MODULUS_HIGH_BIT_WORD, 3);
        // M - 1 = 249; 249 & 63 = 57; mask = 1 << 57
        assert_eq!(Gf2m250TestConfig::MODULUS_HIGH_BIT_MASK, 1u64 << 57);
    }

    #[test]
    fn test_config_default_name() {
        assert_eq!(Gf2m250TestConfig::NAME, "Gf2mWide");
    }

    // -----------------------------------------------------------------------
    // Construction and accessors
    // -----------------------------------------------------------------------

    #[test]
    fn test_zero_is_zero() {
        let z = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        assert!(z.is_zero());
        assert_eq!(z.words(), &[0u64; 4]);
    }

    #[test]
    fn test_one_is_one() {
        let o = Gf2mWide::<4, Gf2m256TestConfig>::one();
        assert!(o.is_one());
        assert_eq!(o.words(), &[1, 0, 0, 0]);
    }

    #[test]
    fn test_zero_and_one_distinct() {
        assert_ne!(
            Gf2mWide::<4, Gf2m256TestConfig>::zero(),
            Gf2mWide::<4, Gf2m256TestConfig>::one()
        );
    }

    #[test]
    fn test_from_u64_low_word() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xdead_beef);
        assert_eq!(a.words(), &[0xdead_beef, 0, 0, 0]);
    }

    #[test]
    fn test_bit_accessor() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0b1010);
        assert!(!a.bit(0));
        assert!(a.bit(1));
        assert!(!a.bit(2));
        assert!(a.bit(3));
        // Bit above low word: all zero for a `from_u64` construction.
        assert!(!a.bit(100));
        assert!(!a.bit(255));
    }

    #[test]
    #[should_panic]
    fn test_bit_accessor_out_of_range_panics() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        let _ = a.bit(256);
    }

    // -----------------------------------------------------------------------
    // Addition
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_zero_identity() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234, 0xabcd, 0xfeed, 0xbeef]);
        let z = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        assert_eq!(&a + &z, a);
        assert_eq!(&z + &a, a);
    }

    #[test]
    fn test_add_commutative() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1111, 0x2222, 0x3333, 0x4444]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0xaaaa, 0xbbbb, 0xcccc, 0xdddd]);
        assert_eq!(&a + &b, &b + &a);
    }

    #[test]
    fn test_add_associative() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1111, 0x2222, 0x3333, 0x4444]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0xaaaa, 0xbbbb, 0xcccc, 0xdddd]);
        let c = Gf2mWide::<4, Gf2m256TestConfig>::new([0xdead, 0xbeef, 0xcafe, 0xf00d]);
        assert_eq!(&(&a + &b) + &c, &a + &(&b + &c));
    }

    #[test]
    fn test_add_self_is_zero_char2() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234, 0xabcd, 0xfeed, 0xbeef]);
        let sum = &a + &a;
        assert!(sum.is_zero());
    }

    #[test]
    fn test_sub_eq_add_char2() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234, 0xabcd, 0xfeed, 0xbeef]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0x5555, 0x6666, 0x7777, 0x8888]);
        assert_eq!(&a - &b, &a + &b);
    }

    #[test]
    fn test_sub_self_is_zero() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234, 0xabcd, 0xfeed, 0xbeef]);
        assert!((&a - &a).is_zero());
    }

    #[test]
    fn test_add_owned_and_mixed_receivers() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0xaa, 0xbb, 0xcc, 0xdd]);
        let ref_sum = &a + &b;
        assert_eq!(a + b, ref_sum);
        assert_eq!(a + &b, ref_sum);
    }

    #[test]
    fn test_add_assign() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0xaa, 0xbb, 0xcc, 0xdd]);
        let mut acc = a;
        acc += b;
        assert_eq!(acc, &a + &b);
        let mut acc2 = a;
        acc2 += &b;
        assert_eq!(acc2, &a + &b);
    }

    #[test]
    fn test_sub_assign() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::new([0xaa, 0xbb, 0xcc, 0xdd]);
        let mut acc = a;
        acc -= b;
        assert_eq!(acc, &a + &b); // In char 2, a - b == a + b.
        let mut acc2 = a;
        acc2 -= &b;
        assert_eq!(acc2, &a + &b);
    }

    // -----------------------------------------------------------------------
    // Negation (identity in characteristic 2)
    // -----------------------------------------------------------------------

    #[test]
    fn test_neg_is_identity_char2() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        assert_eq!(-a, a);
        assert_eq!(-&a, a);
    }

    // -----------------------------------------------------------------------
    // Tail masking
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_tail_masking_m256_fills_top_word() {
        // M = 256, N = 4: the top word uses all 64 bits, so `new` with
        // all-ones input must preserve every bit.
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([u64::MAX; 4]);
        assert_eq!(a.words()[3], u64::MAX);
        // Double-check the bit-width arithmetic quoted in the plan:
        // `(1u64 << (256 - 64*3)) - 1 == u64::MAX` would overflow the
        // shift, but the mathematical identity `1u64 << 64 - 1 == MAX` is
        // what the constructor uses via the `bits_in_top >= 64` branch.
        let shift = 256 - 64 * 3;
        assert_eq!(shift, 64);
    }

    #[test]
    fn test_new_tail_masking_m250_zeroes_top_6_bits() {
        // M = 250, N = 4: the top word uses 58 bits (250 - 192 = 58).
        // The top 6 bits (positions 58..=63) must be zeroed.
        let a = Gf2mWide::<4, Gf2m250TestConfig>::new([u64::MAX; 4]);
        assert_eq!(a.words()[0], u64::MAX);
        assert_eq!(a.words()[1], u64::MAX);
        assert_eq!(a.words()[2], u64::MAX);
        assert_eq!(a.words()[3], (1u64 << 58) - 1);
        // Equivalently: no bit at or above position 250 is set.
        for bit_offset in 0..6 {
            assert_eq!((a.words()[3] >> (58 + bit_offset)) & 1, 0);
        }
    }

    #[test]
    fn test_from_u64_masks_when_m_small() {
        // Construct a tiny single-word config to confirm `from_u64` masks
        // off high bits when `M < 64`.
        struct TinyCfg;
        impl Gf2mWideConfig<1> for TinyCfg {
            const M: usize = 7;
            const MODULUS: [u64; 1] = [0b11]; // placeholder, not used here
        }
        let a = Gf2mWide::<1, TinyCfg>::from_u64(u64::MAX);
        assert_eq!(a.words()[0], 0b0111_1111);
    }

    // -----------------------------------------------------------------------
    // `from_words` debug-assert
    // -----------------------------------------------------------------------

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Gf2mWide::from_words")]
    fn test_from_words_debug_asserts_unmasked_tail() {
        // With M = 250, bit 250 must be zero; set bit 250 to trigger the
        // debug_assert. Bit 250 in word 3 is at local offset 250 - 192 = 58.
        let mut words = [0u64; 4];
        words[3] = 1u64 << 58;
        let _ = Gf2mWide::<4, Gf2m250TestConfig>::from_words(words);
    }

    #[test]
    fn test_from_words_accepts_masked_input() {
        let mut words = [0u64; 4];
        words[3] = (1u64 << 58) - 1; // legal for M = 250
        let a = Gf2mWide::<4, Gf2m250TestConfig>::from_words(words);
        assert_eq!(a.words(), &words);
    }

    // -----------------------------------------------------------------------
    // Other derived-trait checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_copy_clone_eq_hash() {
        use std::collections::HashSet;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        let b = a; // Copy
        assert_eq!(a, b);
        #[allow(clippy::clone_on_copy)]
        let c = a.clone();
        assert_eq!(a, c);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn test_debug_contains_name_and_degree() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        let s = format!("{:?}", a);
        assert!(s.contains("GF(2^256)"), "got: {}", s);
        assert!(s.contains("Gf2m256TestConfig"), "got: {}", s);
        assert!(s.contains("0x"), "got: {}", s);
    }
}
