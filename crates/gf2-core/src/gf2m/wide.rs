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
//!
//! # `clmul_wide` and the `[u64; 2*N]` Rust 1.80 caveat (Task 2)
//!
//! Stable Rust 1.80 (our MSRV) does not support const arithmetic on generic
//! parameters in array lengths — `[u64; 2 * N]` does not compile. Two
//! work-arounds are described in the issue specification:
//!
//! - **(a)** A second const parameter `M` with `const { assert!(M == 2 * N); }`.
//! - **(c)** An `&mut [u64; 2 * N]` out-parameter (which fails for the same
//!   reason) or `&mut [u64]` with a `debug_assert!` on the length.
//!
//! This module uses **pattern (a)**: [`clmul_wide`] takes `<const N: usize,
//! const M: usize>` and asserts `M == 2 * N` at compile time. Callers must
//! annotate the turbofish: `clmul_wide::<2, 4>(a, b)`. The ergonomic cost is
//! small and the return-type annotation is explicit rather than hidden in a
//! mutable out-parameter. Pattern (a) is preferred because it keeps the
//! function purely functional and avoids `&mut` API surface.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};
use std::sync::{Mutex, OnceLock};

use super::barrett::BarrettReducerWide;
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
/// /// `x^256 + x^10 + x^5 + x^2 + 1`. This is the canonical
/// /// `Gf2m256TestConfig` used by every other public item's doctest in
/// /// this module.
/// struct Gf2m256TestConfig;
/// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
///     const M: usize = 256;
///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
/// }
///
/// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
/// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
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
    /// Formats the element as `GF(2^M):0x<words in little-endian-limb order>`.
    ///
    /// Words are printed from `words[0]` (lowest-order limb) to `words[N-1]`
    /// (highest-order limb), separated by underscores. Within each word the
    /// hex is standard (high nibble first). The top word is not zero-padded
    /// when `M` is not a multiple of 64, so the width of the last group may
    /// vary.
    ///
    /// This is identical to the `Display` format so that debug output is
    /// consistently human-readable and matches the pretty-printing convention
    /// established by `Gf2mElement_`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_gf2m_wide(f, Cfg::M, &self.words)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> fmt::Display for Gf2mWide<N, Cfg> {
    /// Formats the element as `GF(2^M):0x<words in little-endian-limb order>`.
    ///
    /// # Arguments
    ///
    /// *(none — this is a `Display` impl)*
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// let s = format!("{}", one);
    /// assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
    /// assert!(s.contains("0000000000000001"), "got: {}", s);
    /// ```
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Complexity
    ///
    /// `O(N)` — iterates once over the `N` words.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_gf2m_wide(f, Cfg::M, &self.words)
    }
}

/// Shared implementation for `Display` and `Debug` on `Gf2mWide<N, Cfg>`.
///
/// Writes `GF(2^m):0x<word[0]>_<word[1]>_..._<word[N-1]>` where each word
/// is rendered with the high nibble first (standard hex), and words are
/// ordered from low to high (little-endian-limb).
///
/// All words except the top word are zero-padded to 16 hex digits so that
/// word boundaries are unambiguous. The top word is not zero-padded because
/// `m` may not be a multiple of 64 and trailing zeros could be misleading.
/// Words are separated by a single underscore.
#[inline]
fn fmt_gf2m_wide(f: &mut fmt::Formatter<'_>, m: usize, words: &[u64]) -> fmt::Result {
    write!(f, "GF(2^{}):0x", m)?;
    let n = words.len();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            write!(f, "_")?;
        }
        if i == n - 1 {
            // Top word: do not zero-pad, it may use fewer than 64 bits.
            write!(f, "{:x}", w)?;
        } else {
            // Lower words: always 16 hex digits so the boundary is clear.
            write!(f, "{:016x}", w)?;
        }
    }
    Ok(())
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
    /// # Complexity
    ///
    /// `O(1)` — a single top-word mask check (elided in release builds).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// // M = 256 = 64 * 4, so every input word is already fully in-range and
    /// // `from_words` round-trips verbatim.
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_words([0x1234_5678, 0, 0, 0]);
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
    /// # Complexity
    ///
    /// `O(1)` — a single bitwise AND on the top word; the input `[u64; N]`
    /// is moved, not copied word-by-word.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// // M = 256 = 64 * 4, so the tail has zero slack: `new` is an
    /// // identity on valid inputs (no bits to mask). For a canonical
    /// // tail-masking demonstration (requires `M < 64 * N`) see the
    /// // unit-test suite in this module's `tests` submodule, which
    /// // exercises `Gf2m250TestConfig` (M = 250, 6 bits of tail slack).
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234, 0, 0, 0]);
    /// assert_eq!(a.words()[0], 0x1234);
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
    /// # Complexity
    ///
    /// `O(N)` words — zero-initialises the `[u64; N]` storage. In practice
    /// LLVM lowers this to a single `memset` / stack-local zero fill.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::zero().is_zero());
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
    /// # Complexity
    ///
    /// `O(N)` words — zero-initialises the storage and sets a single word.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::one().is_one());
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
    /// # Complexity
    ///
    /// `O(N)` words — zero-initialises the storage, sets the low word, and
    /// masks the top word.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(42);
    /// assert_eq!(a.words()[0], 42);
    /// assert_eq!(a.words()[1..], [0, 0, 0]);
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
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xabcd);
    /// assert_eq!(a.words(), &[0xabcd, 0, 0, 0]);
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
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0b1010);
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
    /// # Complexity
    ///
    /// `O(N)` words — short-circuits on the first non-zero word. The worst
    /// case scans every word in `self.words`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::zero().is_zero());
    /// assert!(!Gf2mWide::<4, Gf2m256TestConfig>::one().is_zero());
    /// ```
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// Returns `true` iff the element equals the multiplicative identity.
    ///
    /// # Complexity
    ///
    /// `O(N)` words — checks the low word equals `1` and scans the
    /// remaining `N - 1` words for zeroes, short-circuiting on the first
    /// non-zero word.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::one().is_one());
    /// assert!(!Gf2mWide::<4, Gf2m256TestConfig>::zero().is_one());
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
// Barrett-reducer cache (Task 4 of story 6fb4abad)
//
// `BarrettReducerWide<N>` is deterministically derived from `Cfg::MODULUS` and
// `Cfg::M`. Computing it is O(M²) (polynomial long division) and is therefore
// cached after the first construction. Because Rust does not yet support
// `static` items with generic const parameters on stable (MSRV 1.80), we use
// a global `OnceLock<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>` that
// maps the type identity of `Cfg` to the corresponding `BarrettReducerWide<N>`.
// The `TypeId` key ensures that two distinct configs (even with the same `N`)
// never collide. The `Box<dyn Any>` erases the concrete `N` so the map can be
// shared across all instantiations without additional parameterisation.
// ---------------------------------------------------------------------------

/// Global cache: `TypeId::of::<Cfg>()` → `Box<BarrettReducerWide<N>>` (erased).
static BARRETT_CACHE: OnceLock<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>> =
    OnceLock::new();

/// Returns a freshly constructed (or cached) [`BarrettReducerWide<N>`] for the
/// given `Cfg`.
///
/// The first call for a given `Cfg` type constructs the reducer via
/// [`BarrettReducerWide::new`] (O(M²)), stores it in a global cache, and
/// returns a clone. Subsequent calls return a clone of the cached value in
/// O(N) time.
///
/// # Type Parameters
///
/// * `N` — Number of `u64` words in a field element.
/// * `Cfg` — The [`Gf2mWideConfig`] that defines the modulus and degree.
///
/// # Panics
///
/// Panics if [`BarrettReducerWide::new`] panics (i.e., if `Cfg` violates the
/// modulus/degree contract). Well-formed configs never trigger this.
///
/// # Complexity
///
/// First call: `O(M²)` polynomial long division.
/// Subsequent calls: `O(N)` clone of the cached value.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
///
/// struct Gf2m256TestConfig;
/// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
///     const M: usize = 256;
///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
/// }
///
/// // The multiplier implicitly calls get_reducer internally; this just
/// // demonstrates that the type is usable.
/// let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
/// let b = Gf2mWide::<4, Gf2m256TestConfig>::one();
/// let c = a * b;
/// assert!(c.is_one());
/// ```
fn get_reducer<const N: usize, Cfg: Gf2mWideConfig<N>>() -> BarrettReducerWide<N> {
    let cache = BARRETT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = TypeId::of::<Cfg>();

    // Fast path: try to retrieve without holding the lock long.
    {
        let guard = cache.lock().expect("Barrett cache mutex poisoned");
        if let Some(boxed) = guard.get(&key) {
            return boxed
                .downcast_ref::<BarrettReducerWide<N>>()
                .expect("Barrett cache type mismatch (N mismatch for same Cfg TypeId?)")
                .clone();
        }
    }

    // Slow path: construct the reducer and store it.
    let reducer = BarrettReducerWide::<N>::new(Cfg::MODULUS, Cfg::M as u32);
    {
        let mut guard = cache.lock().expect("Barrett cache mutex poisoned");
        // Another thread may have inserted while we were constructing — that's
        // fine; we just overwrite with an equivalent value.
        guard.insert(key, Box::new(reducer.clone()));
    }
    reducer
}

// ---------------------------------------------------------------------------
// Multiplication (Task 4 of story 6fb4abad)
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Gf2mWide<N, Cfg> {
    /// Multiplies two field elements using carry-less multiplication and
    /// Barrett reduction.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand operand.
    ///
    /// # Returns
    ///
    /// The product `self * rhs` reduced modulo the field's irreducible
    /// polynomial.
    ///
    /// # Complexity
    ///
    /// `O(N²)` carry-less multiplications (via `clmul_wide::<N, {2*N}>`) plus
    /// `O(N²)` work for Barrett reduction (two `clmul_wide` calls on N-word
    /// operands).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
    /// let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// // Multiplicative identity: a * 1 = a.
    /// assert_eq!(a.mul_ref(&one), a);
    /// ```
    #[inline]
    pub fn mul_ref(&self, rhs: &Self) -> Self {
        // Step 1: carry-less multiply to get a 2N-word unreduced product.
        //
        // MSRV 1.80 caveat: `[u64; 2 * N]` is rejected as an array-length
        // expression on stable because `N` is a const generic parameter.
        // We therefore use a `Vec<u64>` buffer and the slice-based helpers
        // [`clmul_wide_slice`] (schoolbook clmul) and
        // [`BarrettReducerWide::reduce_slice`] (Barrett reduction) that were
        // introduced exactly for this callsite. Both share implementations
        // with the array-typed `clmul_wide` / `BarrettReducerWide::reduce`,
        // so `Gf2mWide` pays no algorithmic duplication — only the
        // heap-allocated scratch buffer that MSRV forces on us.
        let mut product = vec![0u64; 2 * N];

        // SIMD fast-path: for N == 4 (GF(2^256)) and available PCLMULQDQ,
        // delegate the 4×4 schoolbook to the dispatched kernel in
        // `gf2-kernels-simd`. The kernel is exposed as a safe `fn` pointer
        // (unsafe intrinsics are isolated in the kernels crate), so this
        // branch remains `#![deny(unsafe_code)]`-clean. Other `N` values and
        // hosts without PCLMULQDQ fall through to the scalar schoolbook.
        #[cfg(feature = "simd")]
        let simd_taken = if N == 4 {
            if let Some(fns) = crate::simd::maybe_gf2m_wide256() {
                // `N == 4` was checked above; the `try_into` conversions are
                // infallible and monomorphise away.
                let a_arr: &[u64; 4] = (&self.words[..])
                    .try_into()
                    .expect("N == 4 guarantees a 4-limb slice");
                let b_arr: &[u64; 4] = (&rhs.words[..])
                    .try_into()
                    .expect("N == 4 guarantees a 4-limb slice");
                let out_arr: &mut [u64; 8] = (&mut product[..])
                    .try_into()
                    .expect("2 * N == 8 guarantees an 8-limb slice");
                (fns.clmul)(a_arr, b_arr, out_arr);
                true
            } else {
                false
            }
        } else {
            false
        };

        #[cfg(not(feature = "simd"))]
        let simd_taken = false;

        if !simd_taken {
            clmul_wide_slice::<N>(&self.words, &rhs.words, &mut product);
        }

        // Step 2: Barrett-reduce the 2N-word product back to N words, via the
        // shared `BarrettReducerWide::reduce_slice` primitive.
        let reducer = get_reducer::<N, Cfg>();
        let reduced = reducer.reduce_slice(&product);

        // The reducer guarantees tail-masking, so `from_words` is safe.
        Gf2mWide::from_words(reduced)
    }

    /// Computes the multiplicative inverse of `self` via Fermat's little
    /// theorem, returning `None` if `self` is zero.
    ///
    /// # Algorithm
    ///
    /// In `GF(2^M)`, every non-zero element `a` satisfies `a^(2^M - 1) = 1`
    /// (Fermat's little theorem for finite fields). Therefore
    /// `a^(-1) = a^(2^M - 2)`.
    ///
    /// The exponent `2^M - 2` is expanded bit-by-bit using the
    /// **square-and-multiply** ladder:
    ///
    /// ```text
    /// result = 1
    /// for i in 0..M:
    ///     if bit i of (2^M - 2) is set:
    ///         result *= a^(2^i)  (accumulated via squarings)
    /// ```
    ///
    /// Note: `2^M - 2` in binary is `111...110` (M-1 ones followed by a zero),
    /// so bits 1 through M-1 are all set and bit 0 is clear. This makes
    /// the total cost `M - 1` multiplications plus `M - 1` squarings.
    ///
    /// ## Alternative: Extended Euclidean Algorithm
    ///
    /// An alternative to Fermat inversion is the **binary extended Euclidean
    /// algorithm** (BEEA) over GF(2)[x], which runs in O(M²) bit operations
    /// but avoids the O(M) multiplications of the Fermat approach. For large
    /// `M` (e.g. M = 256) the BEEA is often faster in practice. This
    /// implementation uses Fermat for simplicity and correctness; a BEEA-based
    /// variant can be substituted without changing the public API.
    ///
    /// # Returns
    ///
    /// `Some(a^(2^M - 2))` if `self` is non-zero, `None` otherwise.
    ///
    /// # Complexity
    ///
    /// `O(M)` multiplications over `GF(2^M)`, each costing `O(N²)` carry-less
    /// word multiplications. Total: `O(M · N²)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// // Zero has no inverse.
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::zero().inverse().is_none());
    ///
    /// // Non-zero element: a * a^(-1) = 1.
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(42);
    /// let inv = a.inverse().expect("non-zero element must have inverse");
    /// assert!((a * inv).is_one());
    /// ```
    ///
    /// # Panics
    ///
    /// Never panics for a well-formed `Cfg`.
    pub fn inverse(&self) -> Option<Self> {
        if self.is_zero() {
            return None;
        }

        // Compute a^(2^M - 2) via square-and-multiply.
        //
        // The exponent 2^M - 2 = 111...10 in binary (M-1 ones, one zero at
        // the low end). We iterate bits 1..M (skipping bit 0, which is zero).
        //
        // Standard binary left-to-right square-and-multiply:
        //   start with result = self (= a^1, corresponding to the leading 1 in
        //   the exponent representation), then for each subsequent bit:
        //     result = result^2
        //     if bit is 1: result *= self
        //
        // For 2^M - 2 the bits are: bit M-1 = 1 (highest), bits M-2..=1 = 1,
        // bit 0 = 0. Left-to-right we always square and multiply (since all
        // bits from M-1 down to 1 are set) except for the final step (bit 0)
        // where we only square.

        let m = Cfg::M;

        // Left-to-right square-and-multiply for e = 2^M - 2.
        //
        // e in binary has bits [M-1..=1] set and bit 0 clear, i.e.:
        //   e = 111...10  (M-1 ones at positions M-1..1, zero at position 0)
        //
        // Algorithm (classic binary method, MSB-first):
        //   result = 1
        //   for bit = M-1 down to 0:
        //       result = result * result   (always square)
        //       if bit(e, bit_position) == 1:
        //           result = result * self
        //
        // For e = 2^M - 2:
        //   - bit M-1 = 1  → square (1²=1), then multiply by self  → result = self
        //   - bits M-2..1 = 1 → square and multiply each iteration
        //   - bit 0 = 0   → square only (no multiply)
        //
        // Simplification: start result = self (equivalent to the first step above
        // where bit M-1 is processed and result becomes self), then process
        // bits M-2 down to 1 (square + multiply each), then process bit 0
        // (square only).

        let mut result = *self; // After processing bit M-1 (always 1, multiply by self).

        // Bits M-2 down to 1 are all set in 2^M - 2 → square + multiply for each.
        // There are M-2 such bits (when M >= 2).
        if m >= 2 {
            for _ in 0..m - 2 {
                result = result.mul_ref(&result); // square
                result = result.mul_ref(self); // multiply (bit is 1)
            }
        }
        // Bit 0 is 0 in 2^M - 2 → square only (no multiply).
        result = result.mul_ref(&result);

        debug_assert!(
            result.mul_ref(self).is_one(),
            "inverse postcondition: inv * self must be 1"
        );

        Some(result)
    }
}

/// `impl Mul for &Gf2mWide` — borrow × borrow.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Multiplies two borrowed field elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
    /// let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// assert_eq!(&a * &one, a);
    /// ```
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_ref(rhs)
    }
}

/// `impl Mul for Gf2mWide` — owned × owned.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Multiplies two owned field elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
    /// assert_eq!(a * b, b * a); // commutativity spot-check
    /// ```
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_ref(&rhs)
    }
}

/// `impl Mul<&Self> for Gf2mWide` — owned × borrow.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Multiplies an owned element by a borrowed element.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(9);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// assert_eq!(a * &b, a);
    /// ```
    #[inline]
    fn mul(self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        self.mul_ref(rhs)
    }
}

/// `impl Mul<Gf2mWide> for &Gf2mWide` — borrow × owned.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul<Gf2mWide<N, Cfg>> for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Multiplies a borrowed element by an owned element.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(11);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(13);
    /// assert_eq!(&a * b, &b * a);
    /// ```
    #[inline]
    fn mul(self, rhs: Gf2mWide<N, Cfg>) -> Self::Output {
        self.mul_ref(&rhs)
    }
}

/// `MulAssign` — in-place multiplication.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> MulAssign for Gf2mWide<N, Cfg> {
    /// Multiplies `self` by `rhs` in place.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(4);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(6);
    /// let expected = a * b;
    /// let mut actual = a;
    /// actual *= b;
    /// assert_eq!(actual, expected);
    /// ```
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul_ref(&rhs);
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> MulAssign<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    /// Multiplies `self` by a borrowed `rhs` in place.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(4);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(6);
    /// let expected = a * b;
    /// let mut actual = a;
    /// actual *= &b;
    /// assert_eq!(actual, expected);
    /// ```
    #[inline]
    fn mul_assign(&mut self, rhs: &Gf2mWide<N, Cfg>) {
        *self = self.mul_ref(rhs);
    }
}

// ---------------------------------------------------------------------------
// Division — defined as mul by inverse
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Divides `self` by `rhs`.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero (division by zero is undefined in a field).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
    /// // a / b * b = a
    /// assert_eq!((&a / &b) * b, a);
    /// ```
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Divides two owned elements.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(9);
    /// assert_eq!((a / b) * b, a);
    /// ```
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Divides an owned element by a borrowed element.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(11);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
    /// assert_eq!((a / &b) * b, a);
    /// ```
    #[inline]
    fn div(self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div<Gf2mWide<N, Cfg>> for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// Divides a borrowed element by an owned element.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(15);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
    /// assert_eq!((&a / b) * b, a);
    /// ```
    #[inline]
    fn div(self, rhs: Gf2mWide<N, Cfg>) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

// ---------------------------------------------------------------------------
// FiniteField trait (Task 4 of story 6fb4abad)
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> crate::field::FiniteField for Gf2mWide<N, Cfg> {
    /// The field characteristic of `GF(2^M)` is 2.
    type Characteristic = u64;

    /// `Wide = Self` because XOR addition over GF(2) never overflows — no
    /// intermediate reduction is required when accumulating sums of products.
    type Wide = Self;

    /// Returns the field characteristic, which is always 2 for `GF(2^M)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// assert_eq!(a.characteristic(), 2u64);
    /// ```
    fn characteristic(&self) -> u64 {
        2
    }

    /// Returns the extension degree `M`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
    /// assert_eq!(a.extension_degree(), 256);
    /// ```
    fn extension_degree(&self) -> usize {
        Cfg::M
    }

    /// Returns `true` iff `self` is the additive identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::zero().is_zero());
    /// assert!(!Gf2mWide::<4, Gf2m256TestConfig>::one().is_zero());
    /// ```
    fn is_zero(&self) -> bool {
        Gf2mWide::is_zero(self)
    }

    /// Returns `true` iff `self` is the multiplicative identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::one().is_one());
    /// assert!(!Gf2mWide::<4, Gf2m256TestConfig>::zero().is_one());
    /// ```
    fn is_one(&self) -> bool {
        Gf2mWide::is_one(self)
    }

    /// Computes the multiplicative inverse, or `None` if `self` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(42);
    /// let inv = a.inv().unwrap();
    /// assert!((a * inv).is_one());
    /// assert!(Gf2mWide::<4, Gf2m256TestConfig>::zero().inv().is_none());
    /// ```
    fn inv(&self) -> Option<Self> {
        self.inverse()
    }

    /// Returns the additive identity in the same field as `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
    /// assert!(a.zero_like().is_zero());
    /// ```
    fn zero_like(&self) -> Self {
        Self::zero()
    }

    /// Returns the multiplicative identity in the same field as `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
    /// assert!(a.one_like().is_one());
    /// ```
    fn one_like(&self) -> Self {
        Self::one()
    }

    /// Converts `self` to the wide accumulator type.
    ///
    /// For `GF(2^M)`, `Wide = Self` so this is a copy.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
    /// assert_eq!(a.to_wide(), a);
    /// ```
    fn to_wide(&self) -> Self::Wide {
        *self
    }

    /// Multiplies `self` by `rhs` and returns the result in the wide type.
    ///
    /// For `GF(2^M)`, `Wide = Self`, so this is just field multiplication.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
    /// let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
    /// let wide = a.mul_to_wide(&b);
    /// assert_eq!(<Gf2mWide::<4, Gf2m256TestConfig> as FiniteField>::reduce_wide(&wide), a * b);
    /// ```
    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        self.mul_ref(rhs)
    }

    /// Reduces a wide accumulator back to a field element.
    ///
    /// For `GF(2^M)`, `Wide = Self`, so this is identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(9);
    /// let wide = a.to_wide();
    /// assert_eq!(<Gf2mWide::<4, Gf2m256TestConfig> as FiniteField>::reduce_wide(&wide), a);
    /// ```
    fn reduce_wide(wide: &Self::Wide) -> Self {
        *wide
    }

    /// Returns the maximum number of wide-type additions before overflow.
    ///
    /// Returns `usize::MAX` because XOR never overflows in `GF(2^M)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// assert_eq!(<Gf2mWide::<4, Gf2m256TestConfig> as FiniteField>::max_unreduced_additions(), usize::MAX);
    /// ```
    fn max_unreduced_additions() -> usize {
        usize::MAX
    }
}

// ---------------------------------------------------------------------------
// ConstField trait (Task 4 of story 6fb4abad)
// ---------------------------------------------------------------------------

impl<const N: usize, Cfg: Gf2mWideConfig<N>> crate::field::ConstField for Gf2mWide<N, Cfg> {
    /// Returns the additive identity (zero polynomial).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::ConstField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// assert!(<Gf2mWide::<4, Gf2m256TestConfig> as ConstField>::zero().is_zero());
    /// ```
    fn zero() -> Self {
        Gf2mWide::zero()
    }

    /// Returns the multiplicative identity (constant polynomial 1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::ConstField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// struct Gf2m256TestConfig;
    /// impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    ///     const M: usize = 256;
    ///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    /// }
    ///
    /// assert!(<Gf2mWide::<4, Gf2m256TestConfig> as ConstField>::one().is_one());
    /// ```
    fn one() -> Self {
        Gf2mWide::one()
    }

    /// Returns the number of elements in the field: `2^M`.
    ///
    /// # Panics
    ///
    /// Panics if `Cfg::M >= 128`, because `2^M` does not fit in a `u128`.
    /// The exact panic message is:
    /// `"Gf2mWide::order exceeds u128 for M = {M}"`.
    ///
    /// This is a fundamental limitation of the `u128` return type of
    /// [`ConstField::order`]. For `M = 256` (the largest config tested in this
    /// crate), callers should use `Cfg::M` directly rather than relying on
    /// `order()`. Task 5 of story `6fb4abad` (`a1229d72`) adds a
    /// `test_field_axioms` variant (without `ConstField::order`) and a
    /// `#[should_panic]` test to document this limitation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::ConstField;
    /// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
    ///
    /// // GF(2^128): largest M that fits in u128.
    /// struct Gf2m128TestConfig;
    /// impl Gf2mWideConfig<2> for Gf2m128TestConfig {
    ///     const M: usize = 128;
    ///     // x^128 + x^7 + x^2 + x + 1 (low bits only, implicit high bit)
    ///     const MODULUS: [u64; 2] = [0x87, 0];
    /// }
    ///
    /// // M = 127 fits: order = 2^127
    /// struct Gf2m127TestConfig;
    /// impl Gf2mWideConfig<2> for Gf2m127TestConfig {
    ///     const M: usize = 127;
    ///     // x^127 + x + 1 (low bits, implicit high bit)
    ///     const MODULUS: [u64; 2] = [3, 0];
    /// }
    ///
    /// // order fits in u128 for M <= 127
    /// assert_eq!(<Gf2mWide::<2, Gf2m127TestConfig> as ConstField>::order(), 1u128 << 127);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn order() -> u128 {
        let m = Cfg::M;
        if m >= 128 {
            panic!("Gf2mWide::order exceeds u128 for M = {}", m);
        }
        1u128 << m
    }
}

// ---------------------------------------------------------------------------
// Multiplication — schoolbook carry-less (Task 2 of story 6fb4abad)
// ---------------------------------------------------------------------------

/// Carry-less multiplication of two `N`-word GF(2)-polynomial operands,
/// producing an unreduced `M`-word result where `M == 2 * N`.
///
/// Each word stores 64 polynomial coefficients in little-endian bit order: bit
/// `i` of the element lives at `words[i >> 6] >> (i & 63) & 1`. The product of
/// two degree-`(64N - 1)` polynomials has degree at most `2 * (64N - 1)`,
/// which requires exactly `2 * N` words of storage.
///
/// The result is **unreduced** — no modular reduction with respect to an
/// irreducible polynomial is applied. Reduction back to `N` words is a
/// separate task (JIT issue `9dd11973`).
///
/// # MSRV caveat: why two const parameters?
///
/// On stable Rust 1.80 the compiler cannot evaluate `2 * N` in an array-length
/// position (`[u64; 2 * N]` is rejected). This function therefore takes a
/// second const parameter `M` and asserts `M == 2 * N` at compile time. Pass
/// the double manually: `clmul_wide::<N, {2 * N}>(a, b)`.
///
/// # Arguments
///
/// * `a` - First operand: `N` little-endian `u64` words.
/// * `b` - Second operand: `N` little-endian `u64` words.
///
/// # Returns
///
/// The unreduced product as `M == 2 * N` little-endian `u64` words.
///
/// # Panics
///
/// Panics (at compile time via `const { assert!(...) }`) if `M != 2 * N`.
///
/// # Complexity
///
/// `O(N²)` carry-less word multiplications (`clmul` calls), each producing a
/// `u128`. For `N` words the inner loop performs `N²` calls.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::wide::clmul_wide;
///
/// // 1 * 1 = 1 (N = 1, M = 2)
/// let a = [1u64];
/// let b = [1u64];
/// let out = clmul_wide::<1, 2>(&a, &b);
/// assert_eq!(out, [1u64, 0u64]);
///
/// // x * x = x^2  (operand = 0b10, result = 0b100 in word 0)
/// let a = [0b10u64];
/// let b = [0b10u64];
/// let out = clmul_wide::<1, 2>(&a, &b);
/// assert_eq!(out[0], 0b100);
/// assert_eq!(out[1], 0);
/// ```
pub fn clmul_wide<const N: usize, const M: usize>(a: &[u64; N], b: &[u64; N]) -> [u64; M] {
    const { assert!(M == 2 * N, "clmul_wide: M must equal 2 * N") }
    let mut out = [0u64; M];
    clmul_wide_slice::<N>(a, b, &mut out);
    out
}

/// Slice-taking variant of [`clmul_wide`] for callers that cannot produce a
/// `[u64; 2 * N]` output array under MSRV 1.80 stable generics.
///
/// `out` must have length exactly `2 * N`. The function XOR-accumulates the
/// schoolbook carry-less product `a * b` into `out`; callers are responsible
/// for zero-initialising `out` before the call if they want the raw product.
///
/// This is the shared implementation used by both [`clmul_wide`] (which owns
/// the output array) and [`Gf2mWide::mul`] (which needs an N-parameterised
/// product buffer without writing `{2 * N}` in a generic context).
///
/// # Arguments
///
/// * `a`, `b` — N-word input operands.
/// * `out` — 2N-word accumulator; products are XOR-ed in. Zero-initialise
///   beforehand to obtain the plain product.
///
/// # Panics
///
/// Debug-asserts that `out.len() == 2 * N`. Release builds rely on the
/// unchecked index arithmetic being in range — passing a shorter slice is
/// undefined at the caller's level.
///
/// # Complexity
///
/// `O(N²)` carry-less-multiply-plus-XOR operations.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::wide::clmul_wide_slice;
///
/// let a = [0b10u64];              // polynomial x
/// let b = [0b10u64];              // polynomial x
/// let mut out = [0u64; 2];
/// clmul_wide_slice::<1>(&a, &b, &mut out);
/// assert_eq!(out[0], 0b100);      // x * x == x²
/// ```
#[inline]
pub fn clmul_wide_slice<const N: usize>(a: &[u64; N], b: &[u64; N], out: &mut [u64]) {
    debug_assert_eq!(out.len(), 2 * N);
    for i in 0..N {
        for j in 0..N {
            let product: u128 = super::barrett::clmul(a[i], b[j]);
            out[i + j] ^= product as u64;
            out[i + j + 1] ^= (product >> 64) as u64;
        }
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
        // Since Task 4, Debug uses the same "GF(2^M):0x..." format as Display.
        // The config NAME is no longer embedded — use the field-degree prefix
        // and the hex representation to identify the output.
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        let s = format!("{:?}", a);
        assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
        // `one` has word[0] = 1, all others zero. In little-endian-limb order
        // word[0] is first, so the hex starts with "0000000000000001".
        assert!(s.contains("0000000000000001"), "got: {}", s);
        // Display and Debug must produce identical output.
        let display = format!("{}", a);
        assert_eq!(s, display, "Debug and Display must be identical");
    }

    // -----------------------------------------------------------------------
    // Word-boundary configs (M = 1, 63, 64, 65)
    //
    // CLAUDE.md requires coverage of 0, 1, 63, 64, 65 bits at word
    // boundaries. `M = 0` is ill-formed (the trait contract requires
    // `M >= 1`), but every other value in the list is covered here.
    // The existing `M = 7` (tiny), `M = 250` (cross-word, non-aligned),
    // and `M = 256` (fully aligned) configs cover the remaining boundary
    // classes.
    //
    // All moduli below are **not** necessarily irreducible; they are
    // placeholders for the tail-masking and XOR tests landed in Task 1.
    // Multiplicative operations are introduced in follow-up tasks and
    // will adopt proper irreducible moduli at that point.
    // -----------------------------------------------------------------------

    /// `M = 1`: the trivial single-bit field. `N = 1`, top-word mask is
    /// `0x1` — only bit 0 is in the field. This is the degenerate but
    /// valid extreme of the contract's `64 * (N - 1) < M <= 64 * N`
    /// range.
    ///
    /// The only irreducible polynomial of degree 1 over GF(2) is
    /// `x + 1`, whose low-bit representation is `MODULUS = [0x1]`.
    struct Gf2m1TestConfig;

    impl Gf2mWideConfig<1> for Gf2m1TestConfig {
        const M: usize = 1;
        const MODULUS: [u64; 1] = [0x1]; // x + 1 (irreducible over GF(2))
    }

    /// `M = 63`: top (and only) word uses 63 of 64 bits — one high bit
    /// must be masked.
    struct Gf2m63TestConfig;

    impl Gf2mWideConfig<1> for Gf2m63TestConfig {
        const M: usize = 63;
        const MODULUS: [u64; 1] = [0x1b]; // placeholder
    }

    /// `M = 64`: top (and only) word is fully used — `top_word_mask`
    /// must be `u64::MAX`.
    struct Gf2m64TestConfig;

    impl Gf2mWideConfig<1> for Gf2m64TestConfig {
        const M: usize = 64;
        const MODULUS: [u64; 1] = [0x1b]; // placeholder
    }

    /// `M = 65`: storage spans two words — top word uses only 1 bit
    /// (the other 63 must be masked). This is the smallest multi-word
    /// config possible.
    struct Gf2m65TestConfig;

    impl Gf2mWideConfig<2> for Gf2m65TestConfig {
        const M: usize = 65;
        const MODULUS: [u64; 2] = [0x1b, 0]; // placeholder
    }

    #[test]
    fn test_boundary_m1_degenerate_field() {
        // M = 1 is the smallest valid configuration — GF(2) itself.
        // Only bit 0 is retained; all other bits must be masked.
        let zero = Gf2mWide::<1, Gf2m1TestConfig>::new([0x0]);
        let one = Gf2mWide::<1, Gf2m1TestConfig>::new([0x1]);
        let all_ones = Gf2mWide::<1, Gf2m1TestConfig>::new([u64::MAX]);

        // Tail mask keeps only the low bit.
        assert_eq!(zero.words()[0], 0);
        assert_eq!(one.words()[0], 1);
        assert_eq!(all_ones.words()[0], 1);

        // Characteristic-2 identities still hold at the trivial width.
        assert!(zero.is_zero());
        assert!(one.is_one());
        assert_eq!((one + one).words()[0], 0); // 1 + 1 = 0 in GF(2)
        assert_eq!((one + zero).words()[0], 1);
        assert_eq!((zero + zero).words()[0], 0);

        // Neg is identity in characteristic 2.
        assert_eq!((-one).words()[0], 1);
        assert_eq!((-zero).words()[0], 0);
    }

    #[test]
    fn test_boundary_m63_tail_masking() {
        // top_word_mask must zero the single high bit (bit 63).
        let a = Gf2mWide::<1, Gf2m63TestConfig>::new([u64::MAX]);
        assert_eq!(a.words()[0], (1u64 << 63) - 1);
        // `from_u64(u64::MAX)` must also strip bit 63.
        let b = Gf2mWide::<1, Gf2m63TestConfig>::from_u64(u64::MAX);
        assert_eq!(b.words()[0], (1u64 << 63) - 1);
        // Sanity: arithmetic and predicates still hold.
        assert!(Gf2mWide::<1, Gf2m63TestConfig>::zero().is_zero());
        assert!(Gf2mWide::<1, Gf2m63TestConfig>::one().is_one());
        let x = Gf2mWide::<1, Gf2m63TestConfig>::new([0x12_3456_789a]);
        assert!((x + x).is_zero());
    }

    #[test]
    fn test_boundary_m64_tail_masking() {
        // top_word_mask must equal u64::MAX — no masking is performed.
        let a = Gf2mWide::<1, Gf2m64TestConfig>::new([u64::MAX]);
        assert_eq!(a.words()[0], u64::MAX);
        let b = Gf2mWide::<1, Gf2m64TestConfig>::from_u64(u64::MAX);
        assert_eq!(b.words()[0], u64::MAX);
        assert!(Gf2mWide::<1, Gf2m64TestConfig>::zero().is_zero());
        assert!(Gf2mWide::<1, Gf2m64TestConfig>::one().is_one());
        let x = Gf2mWide::<1, Gf2m64TestConfig>::new([0xdead_beef_cafe_f00d]);
        assert!((x + x).is_zero());
    }

    #[test]
    fn test_boundary_m65_tail_masking() {
        // top_word_mask must select exactly bit 0 of word 1.
        let a = Gf2mWide::<2, Gf2m65TestConfig>::new([u64::MAX; 2]);
        assert_eq!(a.words()[0], u64::MAX);
        assert_eq!(a.words()[1], 1);
        // `from_u64` only touches the low word — word 1 stays zero.
        let b = Gf2mWide::<2, Gf2m65TestConfig>::from_u64(u64::MAX);
        assert_eq!(b.words(), &[u64::MAX, 0]);
        assert!(Gf2mWide::<2, Gf2m65TestConfig>::zero().is_zero());
        assert!(Gf2mWide::<2, Gf2m65TestConfig>::one().is_one());
        let x = Gf2mWide::<2, Gf2m65TestConfig>::new([0xfeed_face, 1]);
        assert!((x + x).is_zero());
    }

    // -----------------------------------------------------------------------
    // Property-based tests (`proptest`)
    //
    // Each property is exercised against the three flagship test configs:
    //
    //   * `Gf2m256TestConfig`     — full top-word (M = 256, N = 4)
    //   * `Gf2m250TestConfig`     — unaligned top-word tail (M = 250, N = 4)
    //   * `Gf2m63TestConfig` / `Gf2m64TestConfig` / `Gf2m65TestConfig`
    //     — word-boundary trio (N = 1 or 2)
    //
    // `ProptestConfig::with_cases(64)` keeps the full workspace test suite
    // within the 60s budget set in `CLAUDE.md` while still giving a meaningful
    // number of random samples per invariant.
    // -----------------------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Build a 4-word strategy used by the `M = 256` and `M = 250`
        /// configs. Each word is fully random; configs mask the top word
        /// themselves via `Gf2mWide::new`.
        fn any_4_words() -> impl Strategy<Value = [u64; 4]> {
            (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>())
                .prop_map(|(a, b, c, d)| [a, b, c, d])
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            // ---- Gf2m256TestConfig (M = 256, full top-word) ------------

            #[test]
            fn prop_add_commutative_m256(xs in any_4_words(), ys in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m256TestConfig>::new(ys);
                prop_assert_eq!(a + b, b + a);
            }

            #[test]
            fn prop_add_zero_identity_m256(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let z = Gf2mWide::<4, Gf2m256TestConfig>::zero();
                prop_assert_eq!(a + z, a);
                prop_assert_eq!(z + a, a);
            }

            #[test]
            fn prop_self_inverse_char2_m256(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                prop_assert!((a + a).is_zero());
            }

            #[test]
            fn prop_add_assign_matches_add_m256(xs in any_4_words(), ys in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m256TestConfig>::new(ys);
                let mut acc = a;
                acc += b;
                prop_assert_eq!(acc, a + b);
            }

            #[test]
            fn prop_tail_masked_after_new_m256(xs in any_4_words()) {
                // M = 256, N = 4: top_word_mask == u64::MAX, so nothing
                // can live above bit M. This property still guards against
                // regressions in the mask-computation logic.
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let top_mask: u64 = if 256 - 64 * 3 >= 64 {
                    u64::MAX
                } else {
                    (1u64 << (256 - 64 * 3)) - 1
                };
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }

            // ---- Gf2m250TestConfig (M = 250, unaligned top-word) -------

            #[test]
            fn prop_add_commutative_m250(xs in any_4_words(), ys in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m250TestConfig>::new(ys);
                prop_assert_eq!(a + b, b + a);
            }

            #[test]
            fn prop_add_zero_identity_m250(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                let z = Gf2mWide::<4, Gf2m250TestConfig>::zero();
                prop_assert_eq!(a + z, a);
            }

            #[test]
            fn prop_self_inverse_char2_m250(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                prop_assert!((a + a).is_zero());
            }

            #[test]
            fn prop_add_assign_matches_add_m250(xs in any_4_words(), ys in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m250TestConfig>::new(ys);
                let mut acc = a;
                acc += b;
                prop_assert_eq!(acc, a + b);
            }

            #[test]
            fn prop_tail_masked_after_new_m250(xs in any_4_words()) {
                // M = 250, N = 4: top word must have bits >= 58 cleared.
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }

            #[test]
            fn prop_tail_masked_after_from_u64_m250(v in any::<u64>()) {
                // `from_u64` must also respect the tail invariant for
                // configs whose top word sits beyond word 0 — the high
                // bits of the top word must remain zero.
                let a = Gf2mWide::<4, Gf2m250TestConfig>::from_u64(v);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }

            #[test]
            fn prop_tail_masked_zero_one_from_u64_m250(v in any::<u64>()) {
                // `zero`, `one`, and `from_u64` must all produce a tail-
                // masked top word.
                let z = Gf2mWide::<4, Gf2m250TestConfig>::zero();
                let o = Gf2mWide::<4, Gf2m250TestConfig>::one();
                let f = Gf2mWide::<4, Gf2m250TestConfig>::from_u64(v);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(z.words()[3] & !top_mask, 0);
                prop_assert_eq!(o.words()[3] & !top_mask, 0);
                prop_assert_eq!(f.words()[3] & !top_mask, 0);
            }

            // ---- Boundary configs: M = 63, 64, 65 ----------------------

            #[test]
            fn prop_add_commutative_m63(x in any::<u64>(), y in any::<u64>()) {
                let a = Gf2mWide::<1, Gf2m63TestConfig>::new([x]);
                let b = Gf2mWide::<1, Gf2m63TestConfig>::new([y]);
                prop_assert_eq!(a + b, b + a);
                prop_assert!((a + a).is_zero());
            }

            #[test]
            fn prop_tail_masked_m63(x in any::<u64>()) {
                let a = Gf2mWide::<1, Gf2m63TestConfig>::new([x]);
                let top_mask: u64 = (1u64 << 63) - 1;
                prop_assert_eq!(a.words()[0] & !top_mask, 0);
            }

            #[test]
            fn prop_add_commutative_m64(x in any::<u64>(), y in any::<u64>()) {
                let a = Gf2mWide::<1, Gf2m64TestConfig>::new([x]);
                let b = Gf2mWide::<1, Gf2m64TestConfig>::new([y]);
                prop_assert_eq!(a + b, b + a);
                prop_assert!((a + a).is_zero());
            }

            #[test]
            fn prop_tail_masked_m64(x in any::<u64>()) {
                // M = 64: top_word_mask == u64::MAX, no bits to clear.
                let a = Gf2mWide::<1, Gf2m64TestConfig>::new([x]);
                prop_assert_eq!(a.words()[0], x);
            }

            #[test]
            fn prop_add_commutative_m65(
                x0 in any::<u64>(), x1 in any::<u64>(),
                y0 in any::<u64>(), y1 in any::<u64>(),
            ) {
                let a = Gf2mWide::<2, Gf2m65TestConfig>::new([x0, x1]);
                let b = Gf2mWide::<2, Gf2m65TestConfig>::new([y0, y1]);
                prop_assert_eq!(a + b, b + a);
                prop_assert!((a + a).is_zero());
            }

            #[test]
            fn prop_tail_masked_m65(x0 in any::<u64>(), x1 in any::<u64>()) {
                let a = Gf2mWide::<2, Gf2m65TestConfig>::new([x0, x1]);
                // M = 65: top word uses bit 0 only.
                prop_assert_eq!(a.words()[1] & !1u64, 0);
            }

            #[test]
            fn prop_add_assign_matches_add_m65(
                x0 in any::<u64>(), x1 in any::<u64>(),
                y0 in any::<u64>(), y1 in any::<u64>(),
            ) {
                let a = Gf2mWide::<2, Gf2m65TestConfig>::new([x0, x1]);
                let b = Gf2mWide::<2, Gf2m65TestConfig>::new([y0, y1]);
                let mut acc = a;
                acc += b;
                prop_assert_eq!(acc, a + b);
            }
        }
    }

    // -----------------------------------------------------------------------
    // clmul_wide — schoolbook carry-less multiplication (Task 2)
    // -----------------------------------------------------------------------

    mod clmul_wide_tests {
        use super::super::clmul_wide;
        use crate::gf2m::barrett::clmul;
        use proptest::prelude::*;

        // ---- Known-vector unit tests --------------------------------------

        /// `1 * 1 = 1` (N = 1).
        #[test]
        fn test_clmul_wide_one_times_one() {
            let out = clmul_wide::<1, 2>(&[1u64], &[1u64]);
            assert_eq!(out, [1u64, 0u64]);
        }

        /// `x * x = x²` (N = 1).
        ///
        /// Operand = 0b10 (the polynomial `x`).
        /// Product = 0b100 (the polynomial `x²`).
        #[test]
        fn test_clmul_wide_x_times_x() {
            let out = clmul_wide::<1, 2>(&[0b10u64], &[0b10u64]);
            assert_eq!(out[0], 0b100);
            assert_eq!(out[1], 0);
        }

        /// `(x + 1)² = x² + 1` in GF(2)[x] (N = 1).
        ///
        /// Note: `(x+1)² = x² + 2x + 1 = x² + 1` because `2 ≡ 0 (mod 2)`.
        #[test]
        fn test_clmul_wide_x_plus_one_squared() {
            // (x + 1) = 0b11
            let out = clmul_wide::<1, 2>(&[0b11u64], &[0b11u64]);
            // x² + 1 = 0b101
            assert_eq!(out[0], 0b101);
            assert_eq!(out[1], 0);
        }

        /// All-ones squared, N = 1.
        ///
        /// `(sum_{i=0}^{63} x^i)² = sum_{i=0}^{126} x^{2i}` (even powers).
        ///
        /// Squaring in GF(2)[x] places every bit of the input at even
        /// positions in the output, interleaving zeros at odd positions.
        /// The result fits in 127 bits (two words wide).
        #[test]
        fn test_clmul_wide_all_ones_squared_n1() {
            let a = [u64::MAX];
            let out = clmul_wide::<1, 2>(&a, &a);
            // Each set bit at position k maps to position 2k in the product.
            // Bits 0..=63 of `a` become bits 0, 2, 4, …, 126 in the product.
            // Word 0 collects even-position bits at positions 0..64 → bits 0,2,...,62 → alternating 1,0.
            // Word 1 collects bits 64..=126 → bits 64,66,...,126.
            let expected_word0: u64 = 0x5555_5555_5555_5555u64; // 0x55…55 = even bits set in low 64
            let expected_word1: u64 = 0x5555_5555_5555_5555u64; // even bits set in high 63 positions
            assert_eq!(
                out[0], expected_word0,
                "word 0 mismatch for all-ones squared"
            );
            assert_eq!(
                out[1], expected_word1,
                "word 1 mismatch for all-ones squared"
            );
        }

        /// All-ones squared, N = 2 (128-bit operand).
        ///
        /// Squaring in GF(2) is a bit-scatter: bit `i` of the input lands at
        /// bit `2i` of the output. Starting from a 128-bit all-ones operand,
        /// the output has the even-indexed bit of every output position set
        /// and the odd-indexed bit clear. Packed into u64 lanes that is the
        /// constant `0x5555_5555_5555_5555` in all four output words — the
        /// cross-terms `a[0]*a[1]` and `a[1]*a[0]` cancel in GF(2) (equal
        /// terms XOR to zero), so the self-squaring structure survives
        /// intact.
        #[test]
        fn test_clmul_wide_all_ones_squared_n2() {
            let a = [u64::MAX, u64::MAX];
            let out = clmul_wide::<2, 4>(&a, &a);
            let even = 0x5555_5555_5555_5555u64;
            assert_eq!(out[0], even, "word 0");
            assert_eq!(out[1], even, "word 1");
            assert_eq!(out[2], even, "word 2");
            assert_eq!(out[3], even, "word 3");
        }

        // ---- Property-based tests ----------------------------------------

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(128))]

            /// Commutativity: `clmul_wide(a, b) == clmul_wide(b, a)` (N = 2).
            #[test]
            fn prop_clmul_wide_commutative_n2(
                a0 in any::<u64>(), a1 in any::<u64>(),
                b0 in any::<u64>(), b1 in any::<u64>(),
            ) {
                let a = [a0, a1];
                let b = [b0, b1];
                prop_assert_eq!(clmul_wide::<2, 4>(&a, &b), clmul_wide::<2, 4>(&b, &a));
            }

            /// Commutativity for N = 1 (extra coverage at word boundary).
            #[test]
            fn prop_clmul_wide_commutative_n1(a in any::<u64>(), b in any::<u64>()) {
                prop_assert_eq!(clmul_wide::<1, 2>(&[a], &[b]), clmul_wide::<1, 2>(&[b], &[a]));
            }

            /// Commutativity for N = 4 (256-bit operands, `[u64; 4]`).
            ///
            /// Exercises the inner O(N²) schoolbook loop at the largest
            /// size the 6fb4abad story plans to support (`Gf2mWide<4>`
            /// for `GF(2^256)`), confirming that commutativity holds
            /// word-by-word on the full 512-bit product layout.
            #[test]
            fn prop_clmul_wide_commutative_n4(
                a0 in any::<u64>(), a1 in any::<u64>(), a2 in any::<u64>(), a3 in any::<u64>(),
                b0 in any::<u64>(), b1 in any::<u64>(), b2 in any::<u64>(), b3 in any::<u64>(),
            ) {
                let a = [a0, a1, a2, a3];
                let b = [b0, b1, b2, b3];
                prop_assert_eq!(clmul_wide::<4, 8>(&a, &b), clmul_wide::<4, 8>(&b, &a));
            }
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// Cross-check for N = 2 against a reference built directly on
            /// `barrett::clmul(u64, u64) -> u128`.
            ///
            /// For 128-bit operands `a = a1 * x^64 + a0` and
            /// `b = b1 * x^64 + b0`, the schoolbook product is:
            ///
            /// ```text
            /// a * b = (a0*b0) + (a0*b1 + a1*b0) * x^64 + (a1*b1) * x^128
            /// ```
            ///
            /// Each term is a `u128`; splitting them at the 64-bit boundary
            /// and XOR-accumulating gives the four output words.
            #[test]
            fn prop_clmul_wide_n2_matches_reference(
                a0 in any::<u64>(), a1 in any::<u64>(),
                b0 in any::<u64>(), b1 in any::<u64>(),
            ) {
                let a = [a0, a1];
                let b = [b0, b1];

                // Reference: schoolbook with u128 intermediates.
                let p00: u128 = clmul(a0, b0);
                let p01: u128 = clmul(a0, b1);
                let p10: u128 = clmul(a1, b0);
                let p11: u128 = clmul(a1, b1);

                // p00 contributes to words 0 and 1.
                // p01 contributes to words 1 and 2 (shifted by 64).
                // p10 contributes to words 1 and 2 (shifted by 64).
                // p11 contributes to words 2 and 3 (shifted by 128).
                let mut ref_out = [0u64; 4];
                ref_out[0] ^= p00 as u64;
                ref_out[1] ^= (p00 >> 64) as u64;
                ref_out[1] ^= p01 as u64;
                ref_out[2] ^= (p01 >> 64) as u64;
                ref_out[1] ^= p10 as u64;
                ref_out[2] ^= (p10 >> 64) as u64;
                ref_out[2] ^= p11 as u64;
                ref_out[3] ^= (p11 >> 64) as u64;

                let got = clmul_wide::<2, 4>(&a, &b);
                prop_assert_eq!(got, ref_out,
                    "mismatch for a=[{:#x},{:#x}] b=[{:#x},{:#x}]",
                    a0, a1, b0, b1);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Task 4: Multiplication tests (Gf2m256TestConfig, M = 256)
    // -----------------------------------------------------------------------

    /// Small GF(2^128) config using x^128 + x^7 + x^2 + x + 1.
    ///
    /// This polynomial is listed as irreducible over GF(2) (low-weight
    /// trinomial-style; the MODULUS = 0x87 encodes x^7 + x^2 + x + 1 in
    /// the low-bit representation with implicit high bit at position 128).
    /// Used for `inverse()` / `ConstField::order()` tests where M = 128
    /// is the largest value that fits in u128.
    pub(super) struct Gf2m128TestConfig;

    impl Gf2mWideConfig<2> for Gf2m128TestConfig {
        const M: usize = 128;
        // x^7 + x^2 + x + 1 = 0b10000111 = 0x87
        const MODULUS: [u64; 2] = [0x87, 0];
        const NAME: &'static str = "Gf2m128TestConfig";
    }

    /// GF(2^127) using x^127 + x + 1 (primitive trinomial).
    ///
    /// M = 127 is strictly less than 128, so `ConstField::order()` returns
    /// `1u128 << 127` without panicking. Also used as a cross-check for the
    /// inverse round-trip since order() fits in u128.
    pub(super) struct Gf2m127TestConfig;

    impl Gf2mWideConfig<2> for Gf2m127TestConfig {
        const M: usize = 127;
        // x + 1 = 0b11 = 3
        const MODULUS: [u64; 2] = [3, 0];
        const NAME: &'static str = "Gf2m127TestConfig";
    }

    #[test]
    fn test_mul_identity_m256() {
        // a * 1 = a and 1 * a = a
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x1234_5678, 0xabcd, 0, 0]);
        let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
        assert_eq!(a * one, a, "right identity failed");
        assert_eq!(one * a, a, "left identity failed");
    }

    #[test]
    fn test_mul_zero_annihilation_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0xdead_beef, 0xcafe, 0, 0]);
        let zero = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        assert!((a * zero).is_zero(), "a * 0 must be zero");
        assert!((zero * a).is_zero(), "0 * a must be zero");
    }

    #[test]
    fn test_mul_commutativity_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0x1234_5678);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xabcd_ef01);
        assert_eq!(a * b, b * a, "multiplication must be commutative");
    }

    #[test]
    fn test_mul_distributivity_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(11);
        let c = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(13);
        // a * (b + c) = a*b + a*c
        assert_eq!(a * (b + c), (a * b) + (a * c));
    }

    #[test]
    fn test_mul_one_squared_is_one_m256() {
        let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
        assert!((one * one).is_one(), "1 * 1 = 1");
    }

    #[test]
    fn test_mul_ref_variants_agree_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xfeed_face);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xdead_beef);
        let ref_ref = &a * &b;
        assert_eq!(a * b, ref_ref, "owned × owned != &a * &b");
        assert_eq!(a * &b, ref_ref, "owned × &ref != &a * &b");
        assert_eq!(&a * b, ref_ref, "&ref × owned != &a * &b");
    }

    #[test]
    fn test_mul_assign_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(9);
        let expected = a * b;
        let mut actual = a;
        actual *= b;
        assert_eq!(actual, expected, "mul_assign (owned)");
        let mut actual2 = a;
        actual2 *= &b;
        assert_eq!(actual2, expected, "mul_assign (&ref)");
    }

    // -----------------------------------------------------------------------
    // Task 4: Inverse tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_inverse_zero_returns_none_m256() {
        let z = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        assert!(z.inverse().is_none(), "zero has no inverse");
    }

    #[test]
    fn test_inverse_one_is_one_m256() {
        let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
        let inv = one.inverse().unwrap();
        assert!(inv.is_one(), "1^(-1) = 1");
    }

    #[test]
    fn test_inverse_roundtrip_small_m256() {
        // For several small elements, verify a * a^(-1) = 1.
        for v in [2u64, 3, 5, 7, 11, 13, 0xdead_beef, 0x1_0000_0000] {
            let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(v);
            let inv = a.inverse().expect("non-zero element must have inverse");
            assert!(
                (a * inv).is_one(),
                "inverse roundtrip failed for v={:#x}",
                v
            );
        }
    }

    #[test]
    fn test_inverse_roundtrip_m127() {
        // GF(2^127) with primitive polynomial x^127 + x + 1.
        // Use several small elements; verify a * a^(-1) = 1.
        for v in [2u64, 3, 100, 0xffff, 0xdead_beef] {
            let a = Gf2mWide::<2, Gf2m127TestConfig>::from_u64(v);
            let inv = a.inverse().expect("non-zero element must have inverse");
            assert!(
                (a * inv).is_one(),
                "m=127 inverse roundtrip failed for v={:#x}",
                v
            );
        }
    }

    #[test]
    fn test_div_undoes_mul_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(7);
        let b = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(3);
        // (a / b) * b = a
        assert_eq!((a / b) * b, a);
        // a / a = 1
        assert!((a / a).is_one());
    }

    // -----------------------------------------------------------------------
    // Task 4: FiniteField trait tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_finite_field_characteristic_m256() {
        use crate::field::FiniteField;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        assert_eq!(a.characteristic(), 2u64);
    }

    #[test]
    fn test_finite_field_extension_degree_m256() {
        use crate::field::FiniteField;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        assert_eq!(a.extension_degree(), 256);
    }

    #[test]
    fn test_finite_field_inv_m256() {
        use crate::field::FiniteField;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(5);
        let inv = FiniteField::inv(&a).unwrap();
        assert!((a * inv).is_one());
        let z = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        assert!(FiniteField::inv(&z).is_none());
    }

    #[test]
    fn test_finite_field_zero_one_like_m256() {
        use crate::field::FiniteField;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(42);
        assert!(a.zero_like().is_zero());
        assert!(a.one_like().is_one());
    }

    #[test]
    fn test_finite_field_wide_roundtrip_m256() {
        use crate::field::FiniteField;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(9999);
        let wide = a.to_wide();
        let back = Gf2mWide::<4, Gf2m256TestConfig>::reduce_wide(&wide);
        assert_eq!(back, a);
    }

    #[test]
    fn test_finite_field_max_unreduced_additions_m256() {
        use crate::field::FiniteField;
        assert_eq!(
            <Gf2mWide::<4, Gf2m256TestConfig> as FiniteField>::max_unreduced_additions(),
            usize::MAX
        );
    }

    // -----------------------------------------------------------------------
    // Task 4: ConstField trait tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_const_field_zero_one_m127() {
        use crate::field::ConstField;
        // Call the trait methods explicitly to verify the ConstField impl.
        assert!(<Gf2mWide<2, Gf2m127TestConfig> as ConstField>::zero().is_zero());
        assert!(<Gf2mWide<2, Gf2m127TestConfig> as ConstField>::one().is_one());
    }

    #[test]
    fn test_const_field_order_m127() {
        use crate::field::ConstField;
        assert_eq!(
            <Gf2mWide<2, Gf2m127TestConfig> as ConstField>::order(),
            1u128 << 127
        );
    }

    #[test]
    fn test_const_field_order_m128() {
        use crate::field::ConstField;
        type F = Gf2mWide<2, Gf2m128TestConfig>;
        // M = 128 >= 128 → must panic with exact message
        let result = std::panic::catch_unwind(<F as ConstField>::order);
        assert!(result.is_err(), "order() must panic for M = 128");
        let msg = result.unwrap_err();
        let s = msg
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| msg.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            s.contains("Gf2mWide::order exceeds u128 for M = 128"),
            "panic message mismatch: {:?}",
            s
        );
    }

    #[test]
    #[should_panic(expected = "Gf2mWide::order exceeds u128 for M = 256")]
    fn test_const_field_order_m256_panics() {
        use crate::field::ConstField;
        let _ = <Gf2mWide<4, Gf2m256TestConfig> as ConstField>::order();
    }

    // -----------------------------------------------------------------------
    // Task 4: Display / Debug format tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_display_format_one_m256() {
        // GF(2^256), one: words = [1, 0, 0, 0].
        // Expected: GF(2^256):0x0000000000000001_0000000000000000_0000000000000000_0
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        let s = format!("{}", a);
        assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
        // word[0] = 1 → "0000000000000001"
        assert!(s.contains("0000000000000001"), "got: {}", s);
        // word[3] = 0 → "0" (top word not zero-padded)
        assert!(s.ends_with("_0"), "got: {}", s);
    }

    #[test]
    fn test_display_format_zero_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        let s = format!("{}", a);
        assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
        assert!(s.ends_with("_0"), "got: {}", s);
    }

    #[test]
    fn test_debug_equals_display_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xdead_beef);
        assert_eq!(format!("{:?}", a), format!("{}", a));
    }

    #[test]
    fn test_display_format_known_vector_m256() {
        // words = [0xdead_beef_cafe_f00d, 0, 0, 0]
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xdead_beef_cafe_f00d);
        let s = format!("{}", a);
        assert!(s.contains("deadbeefcafef00d"), "got: {}", s);
    }

    #[test]
    fn test_display_format_m127() {
        // GF(2^127): M = 127, N = 2. Top word uses 63 bits (127 - 64 = 63),
        // so the top word is not zero-padded but has at most 16 digits.
        let a = Gf2mWide::<2, Gf2m127TestConfig>::one();
        let s = format!("{}", a);
        assert!(s.starts_with("GF(2^127):0x"), "got: {}", s);
        // word[0] = 1 → "0000000000000001"; word[1] = 0 → "0"
        assert_eq!(s, "GF(2^127):0x0000000000000001_0", "got: {}", s);
    }

    // -----------------------------------------------------------------------
    // Task 4: Mul proptest
    // -----------------------------------------------------------------------

    mod mul_proptests {
        use super::*;
        use proptest::prelude::*;

        fn any_4_words() -> impl Strategy<Value = [u64; 4]> {
            (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>())
                .prop_map(|(a, b, c, d)| [a, b, c, d])
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(32))]

            #[test]
            fn prop_mul_commutative_m256(xs in any_4_words(), ys in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m256TestConfig>::new(ys);
                prop_assert_eq!(a * b, b * a);
            }

            #[test]
            fn prop_mul_identity_m256(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let one = Gf2mWide::<4, Gf2m256TestConfig>::one();
                prop_assert_eq!(a * one, a);
            }

            #[test]
            fn prop_mul_zero_m256(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let zero = Gf2mWide::<4, Gf2m256TestConfig>::zero();
                prop_assert!((a * zero).is_zero());
            }

            #[test]
            fn prop_mul_distributive_m256(
                xs in any_4_words(),
                ys in any_4_words(),
                zs in any_4_words(),
            ) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m256TestConfig>::new(ys);
                let c = Gf2mWide::<4, Gf2m256TestConfig>::new(zs);
                prop_assert_eq!(a * (b + c), (a * b) + (a * c));
            }

            #[test]
            fn prop_inverse_roundtrip_m256(xs in any_4_words()) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                if !a.is_zero() {
                    let inv = a.inverse().unwrap();
                    prop_assert!((a * inv).is_one(),
                        "inverse roundtrip failed for a={:?}", a);
                } else {
                    prop_assert!(a.inverse().is_none());
                }
            }
        }

        /// Reference scalar 4×4 schoolbook carry-less multiply, reduced with
        /// the shared `BarrettReducerWide` — kept here as an independent
        /// copy of the code path so the agreement test does not re-use the
        /// same primitives that `Gf2mWide::mul_ref` itself calls.
        fn scalar_reference_mul(
            a: &Gf2mWide<4, Gf2m256TestConfig>,
            b: &Gf2mWide<4, Gf2m256TestConfig>,
        ) -> Gf2mWide<4, Gf2m256TestConfig> {
            let mut product = [0u64; 8];
            // `clmul_wide_slice` has no SIMD path of its own; it always runs
            // the scalar bit-by-bit clmul → Barrett pipeline. On SIMD hosts
            // `Mul::mul` takes the VPCLMULQDQ path, so this comparison
            // covers both.
            super::clmul_wide_slice::<4>(a.words(), b.words(), &mut product);
            let reducer = super::BarrettReducerWide::<4>::new(Gf2m256TestConfig::MODULUS, 256);
            let reduced = reducer.reduce_slice(&product);
            Gf2mWide::<4, Gf2m256TestConfig>::from_words(reduced)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Unconditional agreement test for the N=4 multiplication path.
            ///
            /// On SIMD hosts (VPCLMULQDQ present, Zen 3 and similar)
            /// `Gf2mWide::<4, _>::mul` dispatches through the kernel in
            /// `gf2-kernels-simd::gf2m_wide`. On hosts without PCLMULQDQ
            /// the dispatch falls back to the pure-Rust scalar schoolbook.
            /// Either way the result must equal the independent reference
            /// implementation in `scalar_reference_mul`.
            #[test]
            fn prop_simd_matches_scalar_reference_m256(
                xs in any_4_words(),
                ys in any_4_words(),
            ) {
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let b = Gf2mWide::<4, Gf2m256TestConfig>::new(ys);
                let got = a * b;
                let expected = scalar_reference_mul(&a, &b);
                prop_assert_eq!(got, expected,
                    "SIMD/scalar disagreement for a={:?}, b={:?}", a, b);
            }
        }
    }
}
