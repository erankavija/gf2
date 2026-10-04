//! Multi-word `GF(2^M)` elements backed by a fixed-size `[u64; N]` array:
//! [`Gf2mWide`] is the const-generic, stack-allocated analogue of
//! [`crate::gf2m::Gf2mElement_`]. Elements are `Copy` and carry their
//! configuration at the type level via a zero-sized [`Gf2mWideConfig`] marker.

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
/// Bit `i` of the element lives at `words[i >> 6] >> (i & 63) & 1`. `Cfg`
/// encodes the extension degree and the defining polynomial as compile-time
/// constants; see [`Gf2mWideConfig`] for its contract.
///
/// # Invariants
///
/// Bits at positions `>= Cfg::M` in the top word must be zero. This
/// module's constructors (except [`Gf2mWide::from_words`]) and mutating
/// operators preserve this invariant automatically.
pub struct Gf2mWide<const N: usize, Cfg: Gf2mWideConfig<N>> {
    words: [u64; N],
    _marker: PhantomData<fn() -> Cfg>,
}

// `Copy` / `Clone` / `PartialEq` / `Hash` are written by hand to keep `Cfg`, a
// ZST marker, off their bounds.

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_gf2m_wide(f, Cfg::M, &self.words)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> fmt::Display for Gf2mWide<N, Cfg> {
    /// Formats the element as `GF(2^M):0x<words in little-endian-limb order>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_gf2m_wide(f, Cfg::M, &self.words)
    }
}

/// Shared implementation for `Display` and `Debug` on `Gf2mWide<N, Cfg>`.
///
/// Writes `GF(2^m):0x<word[0]>_<word[1]>_..._<word[N-1]>`, words ordered low to
/// high. All words except the top one are zero-padded to 16 hex digits; the top
/// word is not, because `m` may not be a multiple of 64.
#[inline]
fn fmt_gf2m_wide(f: &mut fmt::Formatter<'_>, m: usize, words: &[u64]) -> fmt::Result {
    write!(f, "GF(2^{}):0x", m)?;
    let n = words.len();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            write!(f, "_")?;
        }
        if i == n - 1 {
            write!(f, "{:x}", w)?;
        } else {
            write!(f, "{:016x}", w)?;
        }
    }
    Ok(())
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Gf2mWide<N, Cfg> {
    /// Constructs an element directly from `words`, asserting in debug
    /// builds that the tail above bit `M` is already zero.
    ///
    /// For fast paths that re-pack the output of an operation known to be
    /// tail-masked; [`Gf2mWide::new`] masks arbitrary input.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if any bit at position `>= Cfg::M` in
    /// `words` is set. In release builds this check is elided; callers
    /// must uphold the invariant themselves.
    #[inline]
    pub const fn from_words(words: [u64; N]) -> Self {
        // `tail_is_masked` keeps this check const-evaluable.
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
    #[inline]
    pub fn new(mut words: [u64; N]) -> Self {
        Self::mask_tail_in_place(&mut words);
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Returns the additive identity (the zero polynomial).
    #[inline]
    pub const fn zero() -> Self {
        Gf2mWide {
            words: [0u64; N],
            _marker: PhantomData,
        }
    }

    /// Returns the multiplicative identity (the constant polynomial 1).
    ///
    /// # Panics
    ///
    /// Panics if `N == 0`.
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
    #[inline]
    pub fn from_u64(v: u64) -> Self {
        let mut words = [0u64; N];
        if N >= 1 {
            words[0] = v;
        }
        // Tail-mask in case M < 64.
        Self::mask_tail_in_place(&mut words);
        Gf2mWide {
            words,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying `[u64; N]` representation.
    ///
    /// Bit `i` of the element lives at `words[i >> 6] >> (i & 63) & 1`.
    #[inline]
    pub fn words(&self) -> &[u64; N] {
        &self.words
    }

    /// Returns the coefficient of `x^i` in the polynomial representation.
    ///
    /// # Panics
    ///
    /// Panics if `i >= Cfg::M`.
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
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// Returns `true` iff the element equals the multiplicative identity.
    #[inline]
    pub fn is_one(&self) -> bool {
        if N == 0 {
            return false;
        }
        self.words[0] == 1 && self.words[1..].iter().all(|w| *w == 0)
    }

    /// Computes the mask that selects bits `[0, M - 64 * (N - 1))` of the
    /// top word. Bits above this mask are the "tail" that must always be
    /// zero in reduced elements.
    #[inline]
    const fn top_word_mask() -> u64 {
        let bits_in_top = Cfg::M - 64 * (N - 1);
        // `bits_in_top` lies in `1..=64` by the `64 * (N - 1) < M <= 64 * N`
        // contract; 64 takes the branch because `1u64 << 64` overflows.
        if bits_in_top >= 64 {
            u64::MAX
        } else {
            (1u64 << bits_in_top) - 1
        }
    }

    /// Zeros bits at positions `>= Cfg::M` in the top word in place.
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
    /// Lets the `debug_assert!` in [`Gf2mWide::from_words`] stay `const fn`.
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

// `BarrettReducerWide<N>` is derived from `Cfg::MODULUS` and `Cfg::M` and is
// cached after its first construction. Stable Rust has no generic statics,
// so one global map from `(TypeId::of::<Cfg>(), N)` holds the reducers,
// type-erased to `dyn Any`. `N` is part of the key because one marker type
// may implement `Gf2mWideConfig<N>` for several `N`.

/// Type-erased Barrett reducer stored in the global cache.
type CachedReducer = Box<dyn Any + Send + Sync>;

/// Cache key `(TypeId::of::<Cfg>(), N)`.
type BarrettCacheKey = (TypeId, usize);

/// Global cache: `(Cfg, N)` → `BarrettReducerWide<N>` (erased to `Any`).
static BARRETT_CACHE: OnceLock<Mutex<HashMap<BarrettCacheKey, CachedReducer>>> = OnceLock::new();

/// Returns the [`BarrettReducerWide<N>`] for `Cfg`, constructing and caching it
/// on the first call for a `(Cfg, N)` pair and cloning the cached value
/// afterwards.
///
/// # Panics
///
/// Panics if [`BarrettReducerWide::new`] panics, i.e. if `Cfg` violates the
/// modulus/degree contract.
fn get_reducer<const N: usize, Cfg: Gf2mWideConfig<N>>() -> BarrettReducerWide<N> {
    let cache = BARRETT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (TypeId::of::<Cfg>(), N);

    // The lock is held across check-and-insert so concurrent callers for one
    // `(Cfg, N)` pair construct the reducer once.
    let mut guard = cache.lock().expect("Barrett cache mutex poisoned");
    if let Some(boxed) = guard.get(&key) {
        // The `(TypeId, N)` key guarantees the boxed reducer was
        // constructed for this exact `N`; downcast cannot fail.
        return boxed
            .downcast_ref::<BarrettReducerWide<N>>()
            .expect("Barrett cache type mismatch — (TypeId, N) key broken?")
            .clone();
    }

    let reducer = BarrettReducerWide::<N>::new(Cfg::MODULUS, Cfg::M as u32);
    guard.insert(key, Box::new(reducer.clone()));
    reducer
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Gf2mWide<N, Cfg> {
    /// Multiplies two field elements using carry-less multiplication and
    /// Barrett reduction.
    ///
    /// The unreduced product and the two products inside Barrett reduction all
    /// run through `clmul_wide_dispatch`; see its rustdoc for the exact
    /// dispatch predicate. Each call heap-allocates its `2 * N`-word scratch
    /// buffers.
    ///
    /// # Complexity
    ///
    /// Three `N`-word carry-less products, `O(N²)` word products each on the
    /// portable path.
    #[inline]
    pub fn mul_ref(&self, rhs: &Self) -> Self {
        // `[u64; 2 * N]` is not expressible on stable Rust, so the product lives
        // in a `Vec<u64>` and goes through the slice-based helpers.
        let mut product = vec![0u64; 2 * N];
        clmul_wide_dispatch::<N>(
            &self.words,
            &rhs.words,
            &mut product,
            ProductWrite::Overwrite,
        );

        let reducer = get_reducer::<N, Cfg>();
        let reduced = reducer.reduce_slice(&product);

        // The reducer guarantees tail-masking, so `from_words` is safe.
        Gf2mWide::from_words(reduced)
    }

    /// Computes the multiplicative inverse of `self` as `self^(2^M - 2)`
    /// (Fermat's little theorem), returning `None` if `self` is zero.
    ///
    /// # Complexity
    ///
    /// `M - 1` squarings and `M - 2` multiplications for `M >= 2`, each a
    /// [`Gf2mWide::mul_ref`].
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
    pub fn inverse(&self) -> Option<Self> {
        if self.is_zero() {
            return None;
        }

        // Left-to-right square-and-multiply for e = 2^M - 2, whose bits M-1..=1 are
        // set and bit 0 is clear. Processing bit M-1 gives `self`; bits M-2..=1
        // each square and multiply; bit 0 only squares.

        let m = Cfg::M;

        let mut result = *self;

        if m >= 2 {
            for _ in 0..m - 2 {
                result = result.mul_ref(&result);
                result = result.mul_ref(self);
            }
        }
        result = result.mul_ref(&result);

        debug_assert!(
            result.mul_ref(self).is_one(),
            "inverse postcondition: inv * self must be 1"
        );

        Some(result)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_ref(rhs)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_ref(&rhs)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn mul(self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        self.mul_ref(rhs)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul<Gf2mWide<N, Cfg>> for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    #[inline]
    fn mul(self, rhs: Gf2mWide<N, Cfg>) -> Self::Output {
        self.mul_ref(&rhs)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> MulAssign for Gf2mWide<N, Cfg> {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul_ref(&rhs);
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> MulAssign<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    #[inline]
    fn mul_assign(&mut self, rhs: &Gf2mWide<N, Cfg>) {
        *self = self.mul_ref(rhs);
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div<&Gf2mWide<N, Cfg>> for Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    fn div(self, rhs: &Gf2mWide<N, Cfg>) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Div<Gf2mWide<N, Cfg>> for &Gf2mWide<N, Cfg> {
    type Output = Gf2mWide<N, Cfg>;

    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    fn div(self, rhs: Gf2mWide<N, Cfg>) -> Self::Output {
        let inv = rhs.inverse().expect("division by zero in Gf2mWide");
        self.mul_ref(&inv)
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> crate::field::FiniteField for Gf2mWide<N, Cfg> {
    type Characteristic = u64;

    /// `Wide = Self` because XOR addition over GF(2) never overflows — no
    /// intermediate reduction is required when accumulating sums of products.
    type Wide = Self;

    fn characteristic(&self) -> u64 {
        2
    }

    fn extension_degree(&self) -> usize {
        Cfg::M
    }

    fn is_zero(&self) -> bool {
        Gf2mWide::is_zero(self)
    }

    fn is_one(&self) -> bool {
        Gf2mWide::is_one(self)
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
        // Gf2mWide is always ConstField; delegate to the typed zero.
        Some(<Self as crate::field::ConstField>::zero())
    }

    /// Static cardinality hint: `M` (since `|GF(2^M)| = 2^M`). Always
    /// safe — never panics, even for `M >= 128` where
    /// [`crate::field::ConstField::order`] would. See
    /// [`crate::field::FiniteField::cardinality_log2_hint`].
    #[inline]
    fn cardinality_log2_hint() -> Option<u32> {
        Some(Cfg::M as u32)
    }

    fn to_wide(&self) -> Self::Wide {
        *self
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        self.mul_ref(rhs)
    }

    fn reduce_wide(wide: &Self::Wide) -> Self {
        *wide
    }

    /// XOR never overflows in `GF(2^M)`.
    fn max_unreduced_additions() -> usize {
        usize::MAX
    }

    /// Routes a GF(2^8) fused multiply-add through the cached byte product
    /// table, in place and without allocation.
    ///
    /// `crate::gf2m::byte_table::gf256_table_dispatch` states the predicate
    /// that selects this lane; every other degree and every multi-word
    /// configuration declines and keeps the caller's scalar element loop.
    fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {
        let Some(table) = crate::gf2m::byte_table::gf256_table_dispatch(
            Cfg::M,
            N == 1,
            (Cfg::MODULUS.first().copied().unwrap_or(0) & 0xff) as u8,
            || true,
        ) else {
            return false;
        };
        debug_assert_eq!(N, 1, "the dispatch accepts only the single-word width");

        let row = table.row(a.words[0] as u8);
        crate::gf2m::byte_table::axpy_region(
            y,
            x,
            row,
            |source| source.words[0] as u8,
            |destination, product| destination.words[0] ^= u64::from(product),
        );
        true
    }

    fn try_gf2m_u64_batch_dot_product(
        a: &[Self],
        b: &[Self],
        _zero: &Self,
        scratch_a: &mut Vec<u64>,
        scratch_b: &mut Vec<u64>,
        scratch_products: &mut Vec<u64>,
    ) -> Option<Self> {
        debug_assert_eq!(a.len(), b.len());
        if N != 1 || !matches!(Cfg::M, 8 | 16 | 32) {
            return None;
        }

        scratch_a.clear();
        scratch_b.clear();
        scratch_products.clear();
        scratch_a.reserve(a.len());
        scratch_b.reserve(b.len());
        scratch_products.resize(a.len(), 0);

        for (x, y) in a.iter().zip(b.iter()) {
            scratch_a.push(x.words[0]);
            scratch_b.push(y.words[0]);
        }

        let modulus = (1u64 << Cfg::M) | Cfg::MODULUS[0];
        crate::gf2m::batch::batch_mul_raw(Cfg::M, modulus, scratch_a, scratch_b, scratch_products);
        let value = scratch_products.iter().fold(0u64, |acc, &x| acc ^ x);
        Some(Self::from_u64(value))
    }

    /// Computes the dense product over the cached GF(2^8) byte product table
    /// when `crate::gf2m::byte_table::gf256_table_dispatch` accepts.
    ///
    /// Otherwise single-word GF(2^m) for `m` in {8, 16, 32} takes the
    /// `crate::simd::maybe_gf2m_gemm` kernel when the `simd` feature is on and
    /// the host provides it, and the scalar panelized fallback when not; every
    /// other configuration declines.
    fn try_simd_gemm_classical(
        a: &[Self],
        b_t: &[Self],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [Self],
    ) -> bool {
        if let Some(table) = crate::gf2m::byte_table::gf256_table_dispatch(
            Cfg::M,
            N == 1,
            (Cfg::MODULUS.first().copied().unwrap_or(0) & 0xff) as u8,
            || true,
        ) {
            crate::gf2m::byte_table::gemm_region(
                a,
                b_t,
                &crate::gf2m::byte_table::GemmShape { m, k, n },
                out,
                table,
                |source| source.words[0] as u8,
                |destination, product| destination.words[0] = u64::from(product),
            );
            return true;
        }

        if N != 1 || !matches!(Cfg::M, 8 | 16 | 32) {
            return false;
        }

        #[cfg(feature = "simd")]
        {
            if let Some(fns) = crate::simd::maybe_gf2m_gemm() {
                let modulus = (1u64 << Cfg::M) | Cfg::MODULUS[0];
                let mu = crate::gf2m::barrett::BarrettReducer::new(modulus as u128, Cfg::M as u32)
                    .mu() as u64;

                // Extract A to flat u64 buffer: a_flat[i*k + ki] = A[i,ki].
                let mut a_flat: Vec<u64> = Vec::with_capacity(m * k);
                for e in a.iter() {
                    a_flat.push(e.words[0]);
                }

                // b_t is n×k (B transposed, row-major): b_t[j*k + ki] = B[ki,j].
                // We need b_flat (k×n, row-major): b_flat[ki*n + j] = B[ki,j].
                let mut b_flat: Vec<u64> = vec![0u64; k * n];
                for j in 0..n {
                    for ki in 0..k {
                        b_flat[ki * n + j] = b_t[j * k + ki].words[0];
                    }
                }

                let mut out_flat: Vec<u64> = vec![0u64; m * n];

                (fns.gemm_fn)(
                    &a_flat,
                    &b_flat,
                    &mut out_flat,
                    m,
                    k,
                    n,
                    mu,
                    modulus,
                    Cfg::M as u32,
                );

                for (i, val) in out_flat.into_iter().enumerate() {
                    out[i] = Self::from_u64(val);
                }

                return true;
            }
        }

        // Scalar panelized GEMM: the `simd` feature is off or no kernel is available.
        if N == 1 && matches!(Cfg::M, 8 | 16 | 32) {
            Self::scalar_panelized_gemm_fallback_inline(a, b_t, m, k, n, out);
            return true;
        }

        false
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> Gf2mWide<N, Cfg> {
    /// Body of the scalar panelized GEMM fallback, separate from
    /// `try_simd_gemm_classical` so tests reach it without runtime SIMD
    /// detection. Callers guarantee `N == 1` and `Cfg::M in {8, 16, 32}`
    /// (debug-asserted).
    fn scalar_panelized_gemm_fallback_inline(
        a: &[Self],
        b_t: &[Self],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [Self],
    ) {
        debug_assert_eq!(N, 1);
        debug_assert!(matches!(Cfg::M, 8 | 16 | 32));
        let modulus = (1u64 << Cfg::M) | Cfg::MODULUS[0];

        let mut a_flat: Vec<u64> = Vec::with_capacity(m * k);
        for e in a.iter() {
            a_flat.push(e.words[0]);
        }

        // Re-transpose b_t (n*k) to b_flat (k*n).
        let mut b_flat: Vec<u64> = vec![0u64; k * n];
        for j in 0..n {
            for ki in 0..k {
                b_flat[ki * n + j] = b_t[j * k + ki].words[0];
            }
        }

        for e in out.iter_mut() {
            *e = <Self as crate::field::ConstField>::zero();
        }

        let degree = Cfg::M;
        for i in 0..m {
            let out_row = &mut out[i * n..(i + 1) * n];
            for ki in 0..k {
                let a_ik = a_flat[i * k + ki];
                if a_ik == 0 {
                    continue;
                }
                let b_row = &b_flat[ki * n..(ki + 1) * n];
                for (j, &b_kj) in b_row.iter().enumerate() {
                    if b_kj != 0 {
                        let p = crate::gf2m::mul_raw::gf2m_mul_raw(a_ik, b_kj, degree, modulus);
                        let cur = out_row[j].words[0];
                        out_row[j] = Self::from_u64(cur ^ p);
                    }
                }
            }
        }
    }

    /// Test entry point for the scalar fallback, bypassing the table and SIMD
    /// branches of `try_simd_gemm_classical`. Returns `false` without writing
    /// `out` unless `N == 1` and `Cfg::M in {8, 16, 32}`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn scalar_panelized_gemm_fallback_for_test(
        a: &[Self],
        b_t: &[Self],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [Self],
    ) -> bool {
        if N == 1 && matches!(Cfg::M, 8 | 16 | 32) {
            Self::scalar_panelized_gemm_fallback_inline(a, b_t, m, k, n, out);
            true
        } else {
            false
        }
    }
}

impl<const N: usize, Cfg: Gf2mWideConfig<N>> crate::field::ConstField for Gf2mWide<N, Cfg> {
    fn zero() -> Self {
        Gf2mWide::zero()
    }

    fn one() -> Self {
        Gf2mWide::one()
    }

    /// Returns the number of elements in the field: `2^M`.
    ///
    /// # Panics
    ///
    /// Panics if `Cfg::M >= 128`, because `2^M` does not fit in a `u128`;
    /// [`order_log2`](crate::field::ConstField::order_log2) covers those fields.
    fn order() -> u128 {
        let m = Cfg::M;
        if m >= 128 {
            panic!("Gf2mWide::order exceeds u128 for M = {}", m);
        }
        1u128 << m
    }

    /// Returns `M`. Unlike [`order`](crate::field::ConstField::order), this does
    /// not panic for `M >= 128`.
    fn order_log2() -> u32 {
        Cfg::M as u32
    }
}

/// Name the carry-less product dispatch reports when it runs the portable
/// bit-by-bit schoolbook rather than a capability-dispatched kernel.
///
/// The dispatched lanes report the names `gf2_kernels_simd::gf2m_wide`
/// publishes (`"avx2+vpclmulqdq-ymm"`, `"pclmulqdq-scalar-xmm"`), so one
/// vocabulary covers every path a product can take.
pub const PORTABLE_LANE: &str = "portable-scalar";

/// Records the lane a carry-less product just ran on, for the conformance
/// suite's dispatch witness.
///
/// Compiled away outside test and `test-support` builds, where the recorder
/// is an empty inlined function.
#[cfg(any(test, feature = "test-support"))]
#[inline]
fn record_clmul_wide_lane(lane: &'static str) {
    LAST_CLMUL_WIDE_LANE.with(|cell| cell.set(lane));
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
fn record_clmul_wide_lane(_lane: &'static str) {}

#[cfg(any(test, feature = "test-support"))]
std::thread_local! {
    /// Lane of the most recent carry-less product on this thread.
    static LAST_CLMUL_WIDE_LANE: std::cell::Cell<&'static str> =
        const { std::cell::Cell::new(PORTABLE_LANE) };
}

/// The lane the most recent carry-less wide product ran on in this thread:
/// [`PORTABLE_LANE`] or the dispatched kernel's name.
///
/// The witness the shared carry-less product conformance suite reads to prove
/// which path a public call took. A thread that has run no product yet reports
/// [`PORTABLE_LANE`].
#[cfg(any(test, feature = "test-support"))]
pub fn last_clmul_wide_lane() -> &'static str {
    LAST_CLMUL_WIDE_LANE.with(std::cell::Cell::get)
}

#[cfg(any(test, feature = "test-support"))]
static FORCE_SCALAR_CLMUL_WIDE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Holds every wide carry-less product on the portable schoolbook, or releases
/// it back to capability dispatch, and reports the previous setting.
///
/// One switch covers every caller of the canonical dispatch — the public
/// long-product API, `Gf2mWide` multiplication and the wide Barrett reducer —
/// so the portable fallback stays reachable under test on a host that has the
/// accelerated kernels. Every lane computes the same words, so a concurrent
/// product that observes the switch writes the same bits either way.
#[cfg(any(test, feature = "test-support"))]
pub fn force_scalar_clmul_wide(forced: bool) -> bool {
    FORCE_SCALAR_CLMUL_WIDE.swap(forced, std::sync::atomic::Ordering::Relaxed)
}

/// Whether capability dispatch is free to select a kernel.
#[cfg(any(test, feature = "test-support"))]
#[inline]
#[cfg_attr(not(feature = "simd"), allow(dead_code))]
fn clmul_wide_dispatch_enabled() -> bool {
    !FORCE_SCALAR_CLMUL_WIDE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
#[cfg_attr(not(feature = "simd"), allow(dead_code))]
fn clmul_wide_dispatch_enabled() -> bool {
    true
}

/// How a carry-less product reaches the caller's destination.
///
/// The dispatched kernels write a complete `2N`-word product, so they can fill
/// the destination directly for [`Self::Overwrite`]; [`Self::Accumulate`]
/// serves the callers that XOR a product into a running total and costs a
/// scratch buffer plus one XOR pass on a dispatched width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductWrite {
    /// Replace the destination with the product.
    Overwrite,
    /// XOR the product into the destination.
    Accumulate,
}

/// The canonical carry-less product dispatch: every wide product in the crate,
/// public or internal, selects its kernel here.
///
/// # Dispatch predicate
///
/// This is the authoritative statement of when a kernel runs; every other
/// mention of this dispatch (the public API, `Gf2mWide::mul_ref`, the module
/// docs of this module and [`crate::gf2m::barrett`]) cites it by name rather
/// than restate it.
///
/// A kernel runs only for the two widths `gf2-kernels-simd` publishes one for
/// — 4 words (GF(2^256)) and 9 words (GF(2^571)) — and only when *all* of the
/// following hold, exactly as `gf2_kernels_simd::gf2m_wide::detect_x86_wide`
/// evaluates them:
///
/// 1. The crate's `simd` feature is enabled (it is not a default feature).
/// 2. The target is `x86` or `x86_64`.
/// 3. The runtime host reports, for the preferred YMM lane, **AVX2 and
///    VPCLMULQDQ and SSE4.1**; failing that, for the XMM lane, **PCLMULQDQ
///    and SSE4.1**. No other flag (no BMI, no AVX-512) is checked.
///
/// Every other width, every other target, a `simd`-disabled build, and a host
/// that clears neither flag set all run [`clmul_wide_slice_portable`]. Width
/// is a compile-time question alone, since `N` is a const parameter; the flag
/// check is the only part decided at runtime.
///
/// # Panics
///
/// Panics in debug builds if `out.len() != 2 * N`.
///
/// # Complexity
///
/// `O(N²)` word products. A dispatched width performs them in the kernel's
/// vector lanes; every other width performs `N²` bit-by-bit `clmul` calls.
#[inline]
pub(crate) fn clmul_wide_dispatch<const N: usize>(
    a: &[u64; N],
    b: &[u64; N],
    out: &mut [u64],
    write: ProductWrite,
) {
    debug_assert_eq!(out.len(), 2 * N);

    #[cfg(feature = "simd")]
    if clmul_wide_dispatch_enabled() {
        if N == 4 {
            if let Some(fns) = crate::simd::maybe_gf2m_wide256() {
                let a_arr: &[u64; 4] = (&a[..])
                    .try_into()
                    .expect("N == 4 guarantees a 4-limb slice");
                let b_arr: &[u64; 4] = (&b[..])
                    .try_into()
                    .expect("N == 4 guarantees a 4-limb slice");
                record_clmul_wide_lane(fns.name);
                match write {
                    ProductWrite::Overwrite => {
                        let out_arr: &mut [u64; 8] = out
                            .try_into()
                            .expect("2 * N == 8 guarantees an 8-limb slice");
                        (fns.clmul)(a_arr, b_arr, out_arr);
                    }
                    ProductWrite::Accumulate => {
                        let mut scratch = [0u64; 8];
                        (fns.clmul)(a_arr, b_arr, &mut scratch);
                        xor_into(out, &scratch);
                    }
                }
                return;
            }
        } else if N == 9 {
            if let Some(fns) = crate::simd::maybe_gf2m_wide571() {
                let a_arr: &[u64; 9] = (&a[..])
                    .try_into()
                    .expect("N == 9 guarantees a 9-limb slice");
                let b_arr: &[u64; 9] = (&b[..])
                    .try_into()
                    .expect("N == 9 guarantees a 9-limb slice");
                record_clmul_wide_lane(fns.name);
                match write {
                    ProductWrite::Overwrite => {
                        let out_arr: &mut [u64; 18] = out
                            .try_into()
                            .expect("2 * N == 18 guarantees an 18-limb slice");
                        (fns.clmul)(a_arr, b_arr, out_arr);
                    }
                    ProductWrite::Accumulate => {
                        let mut scratch = [0u64; 18];
                        (fns.clmul)(a_arr, b_arr, &mut scratch);
                        xor_into(out, &scratch);
                    }
                }
                return;
            }
        }
    }

    if write == ProductWrite::Overwrite {
        out.fill(0);
    }
    clmul_wide_slice_portable::<N>(a, b, out);
}

/// XOR-folds a kernel's scratch product into the caller's destination.
#[inline]
#[cfg_attr(not(feature = "simd"), allow(dead_code))]
fn xor_into(out: &mut [u64], scratch: &[u64]) {
    for (destination, word) in out.iter_mut().zip(scratch) {
        *destination ^= word;
    }
}

/// Carry-less multiplication of two `N`-word GF(2)-polynomial operands,
/// producing an unreduced `M`-word result where `M == 2 * N`.
///
/// The result is **unreduced** — no modular reduction with respect to an
/// irreducible polynomial is applied. Reduction back to `N` words is
/// performed by [`crate::gf2m::barrett::BarrettReducerWide::reduce_slice`];
/// see [`Gf2mWide::mul_ref`] for the combined clmul + Barrett path.
///
/// # Mechanism
///
/// The product runs through `clmul_wide_dispatch`, the canonical selection
/// this crate's wide arithmetic shares; see its rustdoc for the exact
/// dispatch predicate. Every case the predicate does not satisfy runs
/// [`clmul_wide_slice_portable`].
///
/// # Stable-Rust caveat: why two const parameters?
///
/// On stable Rust the compiler cannot evaluate `2 * N` in an array-length
/// position (`[u64; 2 * N]` is rejected). This function therefore takes a
/// second const parameter `M` and asserts `M == 2 * N` at compile time. Pass
/// the double manually: `clmul_wide::<N, {2 * N}>(a, b)`.
///
/// # Panics
///
/// Fails to compile if `M != 2 * N`.
///
/// # Complexity
///
/// `O(N²)` word products; the portable schoolbook performs `N²` bit-by-bit
/// `clmul` calls.
pub fn clmul_wide<const N: usize, const M: usize>(a: &[u64; N], b: &[u64; N]) -> [u64; M] {
    const { assert!(M == 2 * N, "clmul_wide: M must equal 2 * N") }
    let mut out = [0u64; M];
    clmul_wide_dispatch::<N>(a, b, &mut out, ProductWrite::Overwrite);
    out
}

/// Slice-taking variant of [`clmul_wide`] for callers that cannot produce a
/// `[u64; 2 * N]` output array under stable-Rust generics.
///
/// `out` must have length exactly `2 * N`. The function XOR-accumulates the
/// carry-less product `a * b` into `out`; callers are responsible for
/// zero-initialising `out` before the call if they want the raw product.
///
/// The product runs through `clmul_wide_dispatch` as [`clmul_wide`] does. On a
/// dispatched width, accumulating costs a scratch product and one XOR pass,
/// which [`clmul_wide`] avoids.
///
/// # Panics
///
/// Panics in debug builds if `out.len() != 2 * N`; a release build with a
/// shorter `out` panics on an out-of-range index or drops the high words.
///
/// # Complexity
///
/// `O(N²)` word products, as [`clmul_wide`] describes, plus the `O(N)` XOR
/// pass on a dispatched width.
#[inline]
pub fn clmul_wide_slice<const N: usize>(a: &[u64; N], b: &[u64; N], out: &mut [u64]) {
    clmul_wide_dispatch::<N>(a, b, out, ProductWrite::Accumulate);
}

/// The portable bit-by-bit schoolbook carry-less product: the fallback
/// `clmul_wide_dispatch` takes on a host or a width without a kernel, and
/// the reference every dispatched lane is checked against.
///
/// XOR-accumulates `a * b` into `out`, which must have length exactly
/// `2 * N`. It reaches no capability dispatch, so a caller that wants the
/// portable path whatever the host offers calls it directly.
///
/// # Panics
///
/// Panics in debug builds if `out.len() != 2 * N`.
///
/// # Complexity
///
/// `O(N²)` carry-less-multiply-plus-XOR operations, each `clmul` walking the
/// set bits of its operand.
#[inline]
pub fn clmul_wide_slice_portable<const N: usize>(a: &[u64; N], b: &[u64; N], out: &mut [u64]) {
    debug_assert_eq!(out.len(), 2 * N);
    record_clmul_wide_lane(PORTABLE_LANE);
    for i in 0..N {
        for j in 0..N {
            let product: u128 = super::barrett::clmul(a[i], b[j]);
            out[i + j] ^= product as u64;
            out[i + j + 1] ^= (product >> 64) as u64;
        }
    }
}

#[cfg(test)]
#[allow(clippy::op_ref)]
mod tests {
    use super::*;

    /// One marker type implementing `Gf2mWideConfig<N>` for two `N`: the
    /// Barrett-reducer cache must keep those instantiations apart.
    struct Gf2mMultiNConfig;
    impl Gf2mWideConfig<1> for Gf2mMultiNConfig {
        // GF(2^64): x^64 + x^4 + x^3 + x + 1 (low bits only; implicit high bit).
        const M: usize = 64;
        const MODULUS: [u64; 1] = [0x1b];
    }
    impl Gf2mWideConfig<2> for Gf2mMultiNConfig {
        // GF(2^128): x^128 + x^7 + x^2 + x + 1 (low bits only; implicit high bit).
        const M: usize = 128;
        const MODULUS: [u64; 2] = [0x87, 0];
    }

    /// Many threads asking for the same `(Cfg, N)` reducer all get a working
    /// instance: covers the single-lock check-and-insert path in `get_reducer`.
    #[test]
    fn test_barrett_cache_concurrent_contention_same_key() {
        use std::thread;

        struct Gf2mConcurrentCfg;
        impl Gf2mWideConfig<2> for Gf2mConcurrentCfg {
            const M: usize = 128;
            const MODULUS: [u64; 2] = [0x87, 0];
        }

        let handles: Vec<_> = (0..32)
            .map(|_| {
                thread::spawn(|| {
                    let a = <Gf2mWide<2, Gf2mConcurrentCfg> as crate::field::ConstField>::one();
                    let b = <Gf2mWide<2, Gf2mConcurrentCfg> as crate::field::ConstField>::one();
                    let c = a * b;
                    assert!(c.is_one(), "concurrent GF(2^128) identity multiplication");
                })
            })
            .collect();
        for h in handles {
            h.join()
                .expect("thread panicked during concurrent reducer access");
        }
    }

    struct ScalarFallbackGf2m8Cfg;
    impl Gf2mWideConfig<1> for ScalarFallbackGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
    }

    struct ScalarFallbackGf2m16Cfg;
    impl Gf2mWideConfig<1> for ScalarFallbackGf2m16Cfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
    }

    struct ScalarFallbackGf2m32Cfg;
    impl Gf2mWideConfig<1> for ScalarFallbackGf2m32Cfg {
        const M: usize = 32;
        const MODULUS: [u64; 1] = [0x0000_8299];
    }

    fn naive_gf2m_gemm<Cfg: Gf2mWideConfig<1>>(
        a: &[Gf2mWide<1, Cfg>],
        b: &[Gf2mWide<1, Cfg>],
        m: usize,
        k: usize,
        n: usize,
    ) -> Vec<Gf2mWide<1, Cfg>> {
        let mut out = vec![<Gf2mWide<1, Cfg> as crate::field::ConstField>::zero(); m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc = <Gf2mWide<1, Cfg> as crate::field::ConstField>::zero();
                for ki in 0..k {
                    acc += a[i * k + ki] * b[ki * n + j];
                }
                out[i * n + j] = acc;
            }
        }
        out
    }

    fn build_test_matrix<Cfg: Gf2mWideConfig<1>>(
        rows: usize,
        cols: usize,
        seed: u64,
    ) -> Vec<Gf2mWide<1, Cfg>> {
        let mask = if Cfg::M == 64 {
            u64::MAX
        } else {
            (1u64 << Cfg::M) - 1
        };
        let mut out = Vec::with_capacity(rows * cols);
        let mut x = seed;
        for _ in 0..rows * cols {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = x & mask;
            out.push(<Gf2mWide<1, Cfg>>::from_u64(v));
        }
        out
    }

    /// Transpose `b` from row-major `(k, n)` to row-major `(n, k)` —
    /// the form `try_simd_gemm_classical` and the scalar fallback
    /// expect for the right-hand operand.
    fn transpose_for_gemm<Cfg: Gf2mWideConfig<1>>(
        b: &[Gf2mWide<1, Cfg>],
        k: usize,
        n: usize,
    ) -> Vec<Gf2mWide<1, Cfg>> {
        let mut b_t = vec![<Gf2mWide<1, Cfg> as crate::field::ConstField>::zero(); n * k];
        for ki in 0..k {
            for j in 0..n {
                b_t[j * k + ki] = b[ki * n + j];
            }
        }
        b_t
    }

    #[test]
    fn test_scalar_panelized_gemm_fallback_matches_naive() {
        fn check<Cfg: Gf2mWideConfig<1>>(m: usize, k: usize, n: usize, seed: u64) {
            let a = build_test_matrix::<Cfg>(m, k, seed);
            let b = build_test_matrix::<Cfg>(k, n, seed.wrapping_add(0x1234_5678));
            let b_t = transpose_for_gemm::<Cfg>(&b, k, n);
            let mut got = vec![<Gf2mWide<1, Cfg> as crate::field::ConstField>::zero(); m * n];
            let used = Gf2mWide::<1, Cfg>::scalar_panelized_gemm_fallback_for_test(
                &a, &b_t, m, k, n, &mut got,
            );
            assert!(used, "scalar fallback should run for m={m} k={k} n={n}");
            let want = naive_gf2m_gemm::<Cfg>(&a, &b, m, k, n);
            assert_eq!(got, want, "scalar fallback mismatch on m={m} k={k} n={n}");
        }
        check::<ScalarFallbackGf2m8Cfg>(4, 4, 4, 0xA1);
        check::<ScalarFallbackGf2m8Cfg>(8, 16, 8, 0xA2);
        check::<ScalarFallbackGf2m8Cfg>(13, 7, 11, 0xA3);
        check::<ScalarFallbackGf2m16Cfg>(4, 4, 4, 0xB1);
        check::<ScalarFallbackGf2m16Cfg>(8, 16, 8, 0xB2);
        check::<ScalarFallbackGf2m16Cfg>(11, 9, 13, 0xB3);
        check::<ScalarFallbackGf2m32Cfg>(4, 4, 4, 0xC1);
        check::<ScalarFallbackGf2m32Cfg>(8, 16, 8, 0xC2);
        check::<ScalarFallbackGf2m32Cfg>(7, 11, 9, 0xC3);
    }

    #[test]
    fn test_scalar_panelized_gemm_fallback_zero_input_zero_output() {
        type Cfg = ScalarFallbackGf2m8Cfg;
        let m = 4;
        let k = 4;
        let n = 4;
        let zero = <Gf2mWide<1, Cfg> as crate::field::ConstField>::zero();
        let a = vec![zero; m * k];
        let b_t = vec![zero; n * k];
        let mut got = vec![<Gf2mWide<1, Cfg>>::from_u64(0xFF); m * n];
        let used = Gf2mWide::<1, Cfg>::scalar_panelized_gemm_fallback_for_test(
            &a, &b_t, m, k, n, &mut got,
        );
        assert!(used, "scalar fallback should run for the all-zero case");
        for cell in got.iter() {
            assert_eq!(*cell, zero, "zero inputs must produce zero output");
        }
    }

    #[test]
    fn test_barrett_cache_distinguishes_n_for_same_cfg_type() {
        let a1 = <Gf2mWide<1, Gf2mMultiNConfig> as crate::field::ConstField>::one();
        let b1 = <Gf2mWide<1, Gf2mMultiNConfig> as crate::field::ConstField>::one();
        let c1 = a1 * b1;
        assert!(c1.is_one(), "GF(2^64) identity multiplication");

        let a2 = <Gf2mWide<2, Gf2mMultiNConfig> as crate::field::ConstField>::one();
        let b2 = <Gf2mWide<2, Gf2mMultiNConfig> as crate::field::ConstField>::one();
        let c2 = a2 * b2;
        assert!(c2.is_one(), "GF(2^128) identity multiplication");
    }

    /// Test config for GF(2^256) using the pentanomial
    /// `x^256 + x^10 + x^5 + x^2 + 1` (`@/citation/Seroussi1998`, Table 1 row
    /// `m = 256`).
    pub(super) struct Gf2m256TestConfig;

    impl Gf2mWideConfig<4> for Gf2m256TestConfig {
        const M: usize = 256;
        const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
        const NAME: &'static str = "Gf2m256TestConfig";
    }

    /// GF(2^571) B-571/K-571 binary-field polynomial (`@/citation/Nist2013`,
    /// Appendix D): `x^571 + x^10 + x^5 + x^2 + 1`.
    pub(super) struct Gf2m571TestConfig;

    impl Gf2mWideConfig<9> for Gf2m571TestConfig {
        const M: usize = 571;
        // Low terms x^10 + x^5 + x^2 + 1; the x^571 term is implicit.
        const MODULUS: [u64; 9] = [0x425, 0, 0, 0, 0, 0, 0, 0, 0];
        const NAME: &'static str = "Gf2m571TestConfig";
    }

    /// Synthetic test config with `M = 250` to exercise the tail-masking
    /// path (the top word must zero its high 6 bits).
    struct Gf2m250TestConfig;

    impl Gf2mWideConfig<4> for Gf2m250TestConfig {
        const M: usize = 250;
        // Not irreducible: this config only tests the tail-masking invariant, and
        // no multiplicative operation runs against it.
        const MODULUS: [u64; 4] = [0x1, 0, 0, 0];
    }

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
    fn test_config_modulus_high_bit_word_and_mask_m571() {
        assert_eq!(Gf2m571TestConfig::MODULUS_HIGH_BIT_WORD, 8);
        // M - 1 = 570; 570 & 63 = 58; mask = 1 << 58.
        assert_eq!(Gf2m571TestConfig::MODULUS_HIGH_BIT_MASK, 1u64 << 58);
    }

    #[test]
    fn test_config_default_name() {
        assert_eq!(Gf2m250TestConfig::NAME, "Gf2mWide");
    }

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
        assert!(!a.bit(100));
        assert!(!a.bit(255));
    }

    #[test]
    #[should_panic]
    fn test_bit_accessor_out_of_range_panics() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::zero();
        let _ = a.bit(256);
    }

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

    #[test]
    fn test_neg_is_identity_char2() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        assert_eq!(-a, a);
        assert_eq!(-&a, a);
    }

    #[test]
    fn test_new_tail_masking_m256_fills_top_word() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([u64::MAX; 4]);
        assert_eq!(a.words()[3], u64::MAX);
        // `1u64 << 64` would overflow; the constructor takes the
        // `bits_in_top >= 64` branch instead.
        let shift = 256 - 64 * 3;
        assert_eq!(shift, 64);
    }

    #[test]
    fn test_new_tail_masking_m250_zeroes_top_6_bits() {
        let a = Gf2mWide::<4, Gf2m250TestConfig>::new([u64::MAX; 4]);
        assert_eq!(a.words()[0], u64::MAX);
        assert_eq!(a.words()[1], u64::MAX);
        assert_eq!(a.words()[2], u64::MAX);
        assert_eq!(a.words()[3], (1u64 << 58) - 1);
        for bit_offset in 0..6 {
            assert_eq!((a.words()[3] >> (58 + bit_offset)) & 1, 0);
        }
    }

    #[test]
    fn test_from_u64_masks_when_m_small() {
        struct TinyCfg;
        impl Gf2mWideConfig<1> for TinyCfg {
            const M: usize = 7;
            const MODULUS: [u64; 1] = [0b11]; // unused by this test
        }
        let a = Gf2mWide::<1, TinyCfg>::from_u64(u64::MAX);
        assert_eq!(a.words()[0], 0b0111_1111);
    }

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

    #[test]
    fn test_copy_clone_eq_hash() {
        use std::collections::HashSet;
        let a = Gf2mWide::<4, Gf2m256TestConfig>::new([0x11, 0x22, 0x33, 0x44]);
        let b = a;
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
        assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
        assert!(s.contains("0000000000000001"), "got: {}", s);
        let display = format!("{}", a);
        assert_eq!(s, display, "Debug and Display must be identical");
    }

    /// `M = 1`: the single-bit field, the low extreme of the
    /// `64 * (N - 1) < M <= 64 * N` range. `MODULUS = [0x1]` is `x + 1`.
    struct Gf2m1TestConfig;

    impl Gf2mWideConfig<1> for Gf2m1TestConfig {
        const M: usize = 1;
        const MODULUS: [u64; 1] = [0x1];
    }

    /// `M = 63`: top (and only) word uses 63 of 64 bits — one high bit
    /// must be masked.
    struct Gf2m63TestConfig;

    impl Gf2mWideConfig<1> for Gf2m63TestConfig {
        const M: usize = 63;
        const MODULUS: [u64; 1] = [0x1b]; // unused by the tests
    }

    /// `M = 64`: top (and only) word is fully used — `top_word_mask`
    /// must be `u64::MAX`.
    struct Gf2m64TestConfig;

    impl Gf2mWideConfig<1> for Gf2m64TestConfig {
        const M: usize = 64;
        const MODULUS: [u64; 1] = [0x1b]; // unused by the tests
    }

    /// `M = 65`: storage spans two words — top word uses only 1 bit
    /// (the other 63 must be masked). This is the smallest multi-word
    /// config possible.
    struct Gf2m65TestConfig;

    impl Gf2mWideConfig<2> for Gf2m65TestConfig {
        const M: usize = 65;
        const MODULUS: [u64; 2] = [0x1b, 0]; // unused by the tests
    }

    #[test]
    fn test_boundary_m1_degenerate_field() {
        let zero = Gf2mWide::<1, Gf2m1TestConfig>::new([0x0]);
        let one = Gf2mWide::<1, Gf2m1TestConfig>::new([0x1]);
        let all_ones = Gf2mWide::<1, Gf2m1TestConfig>::new([u64::MAX]);

        assert_eq!(zero.words()[0], 0);
        assert_eq!(one.words()[0], 1);
        assert_eq!(all_ones.words()[0], 1);

        assert!(zero.is_zero());
        assert!(one.is_one());
        assert_eq!((one + one).words()[0], 0);
        assert_eq!((one + zero).words()[0], 1);
        assert_eq!((zero + zero).words()[0], 0);

        assert_eq!((-one).words()[0], 1);
        assert_eq!((-zero).words()[0], 0);
    }

    #[test]
    fn test_boundary_m63_tail_masking() {
        let a = Gf2mWide::<1, Gf2m63TestConfig>::new([u64::MAX]);
        assert_eq!(a.words()[0], (1u64 << 63) - 1);
        let b = Gf2mWide::<1, Gf2m63TestConfig>::from_u64(u64::MAX);
        assert_eq!(b.words()[0], (1u64 << 63) - 1);
        assert!(Gf2mWide::<1, Gf2m63TestConfig>::zero().is_zero());
        assert!(Gf2mWide::<1, Gf2m63TestConfig>::one().is_one());
        let x = Gf2mWide::<1, Gf2m63TestConfig>::new([0x12_3456_789a]);
        assert!((x + x).is_zero());
    }

    #[test]
    fn test_boundary_m64_tail_masking() {
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
        let a = Gf2mWide::<2, Gf2m65TestConfig>::new([u64::MAX; 2]);
        assert_eq!(a.words()[0], u64::MAX);
        assert_eq!(a.words()[1], 1);
        let b = Gf2mWide::<2, Gf2m65TestConfig>::from_u64(u64::MAX);
        assert_eq!(b.words(), &[u64::MAX, 0]);
        assert!(Gf2mWide::<2, Gf2m65TestConfig>::zero().is_zero());
        assert!(Gf2mWide::<2, Gf2m65TestConfig>::one().is_one());
        let x = Gf2mWide::<2, Gf2m65TestConfig>::new([0xfeed_face, 1]);
        assert!((x + x).is_zero());
    }

    // `ProptestConfig::with_cases(64)` keeps the suite within the fast-tier
    // budget (`@/inv/test-tier-budgets`).

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Four fully random words; configs mask the top word via `Gf2mWide::new`.
        fn any_4_words() -> impl Strategy<Value = [u64; 4]> {
            (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>())
                .prop_map(|(a, b, c, d)| [a, b, c, d])
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]


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
                let a = Gf2mWide::<4, Gf2m256TestConfig>::new(xs);
                let top_mask: u64 = if 256 - 64 * 3 >= 64 {
                    u64::MAX
                } else {
                    (1u64 << (256 - 64 * 3)) - 1
                };
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }


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
                let a = Gf2mWide::<4, Gf2m250TestConfig>::new(xs);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }

            #[test]
            fn prop_tail_masked_after_from_u64_m250(v in any::<u64>()) {
                let a = Gf2mWide::<4, Gf2m250TestConfig>::from_u64(v);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(a.words()[3] & !top_mask, 0);
            }

            #[test]
            fn prop_tail_masked_zero_one_from_u64_m250(v in any::<u64>()) {
                let z = Gf2mWide::<4, Gf2m250TestConfig>::zero();
                let o = Gf2mWide::<4, Gf2m250TestConfig>::one();
                let f = Gf2mWide::<4, Gf2m250TestConfig>::from_u64(v);
                let top_mask: u64 = (1u64 << (250 - 64 * 3)) - 1;
                prop_assert_eq!(z.words()[3] & !top_mask, 0);
                prop_assert_eq!(o.words()[3] & !top_mask, 0);
                prop_assert_eq!(f.words()[3] & !top_mask, 0);
            }


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

    mod clmul_wide_tests {
        use super::super::clmul_wide;
        use crate::gf2m::barrett::clmul;
        use proptest::prelude::*;

        #[test]
        fn test_clmul_wide_one_times_one() {
            let out = clmul_wide::<1, 2>(&[1u64], &[1u64]);
            assert_eq!(out, [1u64, 0u64]);
        }

        #[test]
        fn test_clmul_wide_x_times_x() {
            let out = clmul_wide::<1, 2>(&[0b10u64], &[0b10u64]);
            assert_eq!(out[0], 0b100);
            assert_eq!(out[1], 0);
        }

        #[test]
        fn test_clmul_wide_x_plus_one_squared() {
            let out = clmul_wide::<1, 2>(&[0b11u64], &[0b11u64]);
            assert_eq!(out[0], 0b101);
            assert_eq!(out[1], 0);
        }

        /// `(sum_{i=0}^{63} x^i)² = sum_{i=0}^{63} x^{2i}` (even powers).
        #[test]
        fn test_clmul_wide_all_ones_squared_n1() {
            let a = [u64::MAX];
            let out = clmul_wide::<1, 2>(&a, &a);
            let expected_word0: u64 = 0x5555_5555_5555_5555u64;
            let expected_word1: u64 = 0x5555_5555_5555_5555u64;
            assert_eq!(
                out[0], expected_word0,
                "word 0 mismatch for all-ones squared"
            );
            assert_eq!(
                out[1], expected_word1,
                "word 1 mismatch for all-ones squared"
            );
        }

        /// Bit `i` of the input lands at bit `2i` of the output, and the
        /// cross-terms `a[0]*a[1]` and `a[1]*a[0]` cancel, so every output word
        /// is `0x5555_5555_5555_5555`.
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

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(128))]

            #[test]
            fn prop_clmul_wide_commutative_n2(
                a0 in any::<u64>(), a1 in any::<u64>(),
                b0 in any::<u64>(), b1 in any::<u64>(),
            ) {
                let a = [a0, a1];
                let b = [b0, b1];
                prop_assert_eq!(clmul_wide::<2, 4>(&a, &b), clmul_wide::<2, 4>(&b, &a));
            }

            #[test]
            fn prop_clmul_wide_commutative_n1(a in any::<u64>(), b in any::<u64>()) {
                prop_assert_eq!(clmul_wide::<1, 2>(&[a], &[b]), clmul_wide::<1, 2>(&[b], &[a]));
            }

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
            #[test]
            fn prop_clmul_wide_n2_matches_reference(
                a0 in any::<u64>(), a1 in any::<u64>(),
                b0 in any::<u64>(), b1 in any::<u64>(),
            ) {
                let a = [a0, a1];
                let b = [b0, b1];

                let p00: u128 = clmul(a0, b0);
                let p01: u128 = clmul(a0, b1);
                let p10: u128 = clmul(a1, b0);
                let p11: u128 = clmul(a1, b1);

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

    /// GF(2^128) config using x^128 + x^7 + x^2 + x + 1; `MODULUS = 0x87`
    /// encodes the low terms. M = 128 is the smallest degree whose order
    /// exceeds `u128`.
    pub(super) struct Gf2m128TestConfig;

    impl Gf2mWideConfig<2> for Gf2m128TestConfig {
        const M: usize = 128;
        const MODULUS: [u64; 2] = [0x87, 0];
        const NAME: &'static str = "Gf2m128TestConfig";
    }

    /// GF(2^127) using x^127 + x + 1 (primitive trinomial); its order `2^127`
    /// fits in `u128`.
    pub(super) struct Gf2m127TestConfig;

    impl Gf2mWideConfig<2> for Gf2m127TestConfig {
        const M: usize = 127;
        const MODULUS: [u64; 2] = [3, 0];
        const NAME: &'static str = "Gf2m127TestConfig";
    }

    #[test]
    fn test_mul_identity_m256() {
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
    fn test_mul_m571_matches_scalar_reference() {
        let a = Gf2mWide::<9, Gf2m571TestConfig>::new([
            0xDEAD_BEEF_CAFE_BABE,
            0x0123_4567_89AB_CDEF,
            0xFEDC_BA98_7654_3210,
            0xAAAA_5555_AAAA_5555,
            0x1357_9BDF_2468_ACE0,
            0x0F0F_F0F0_3333_CCCC,
            0xFFFF_0000_FFFF_0000,
            0x1111_2222_3333_4444,
            0x07FF_FFFF_FFFF_FFFF,
        ]);
        let b = Gf2mWide::<9, Gf2m571TestConfig>::new([
            0x5555_AAAA_5555_AAAA,
            0x1122_3344_5566_7788,
            0xFFFF_FFFF_0000_0000,
            0x0F0F_F0F0_0F0F_F0F0,
            0x2468_ACE0_1357_9BDF,
            0x3333_CCCC_0F0F_F0F0,
            0x0000_FFFF_0000_FFFF,
            0x4444_3333_2222_1111,
            0x03FF_FFFF_FFFF_FFFF,
        ]);
        let got = a * b;
        let mut product = [0u64; 18];
        clmul_wide_slice_portable::<9>(a.words(), b.words(), &mut product);
        let reduced = crate::gf2m::barrett::reference_reduce_wide::<9, 18>(
            &product,
            &Gf2m571TestConfig::MODULUS,
            571,
        );
        let expected = Gf2mWide::<9, Gf2m571TestConfig>::from_words(reduced);
        assert_eq!(got, expected);
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
        assert_eq!((a / b) * b, a);
        assert!((a / a).is_one());
    }

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

    #[test]
    fn test_const_field_zero_one_m127() {
        use crate::field::ConstField;
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

    #[test]
    fn test_display_format_one_m256() {
        let a = Gf2mWide::<4, Gf2m256TestConfig>::one();
        let s = format!("{}", a);
        assert!(s.starts_with("GF(2^256):0x"), "got: {}", s);
        assert!(s.contains("0000000000000001"), "got: {}", s);
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
        let a = Gf2mWide::<4, Gf2m256TestConfig>::from_u64(0xdead_beef_cafe_f00d);
        let s = format!("{}", a);
        assert!(s.contains("deadbeefcafef00d"), "got: {}", s);
    }

    #[test]
    fn test_display_format_m127() {
        let a = Gf2mWide::<2, Gf2m127TestConfig>::one();
        let s = format!("{}", a);
        assert!(s.starts_with("GF(2^127):0x"), "got: {}", s);
        assert_eq!(s, "GF(2^127):0x0000000000000001_0", "got: {}", s);
    }

    mod mul_proptests {
        use super::*;
        use proptest::prelude::*;

        fn any_4_words() -> impl Strategy<Value = [u64; 4]> {
            (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>())
                .prop_map(|(a, b, c, d)| [a, b, c, d])
        }

        fn any_9_words_m571() -> impl Strategy<Value = [u64; 9]> {
            (
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
            )
                .prop_map(|(a, b, c, d, e, f, g, h, i)| {
                    [a, b, c, d, e, f, g, h, i & ((1u64 << 59) - 1)]
                })
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

        /// Reference product: the portable schoolbook carry-less multiply reduced
        /// by the test-only shift-and-XOR reducer, sharing no kernel dispatch and
        /// no Barrett code with `Gf2mWide::mul_ref`.
        fn scalar_reference_mul(
            a: &Gf2mWide<4, Gf2m256TestConfig>,
            b: &Gf2mWide<4, Gf2m256TestConfig>,
        ) -> Gf2mWide<4, Gf2m256TestConfig> {
            let mut product = [0u64; 8];
            super::clmul_wide_slice_portable::<4>(a.words(), b.words(), &mut product);
            let reduced = crate::gf2m::barrett::reference_reduce_wide::<4, 8>(
                &product,
                &Gf2m256TestConfig::MODULUS,
                256,
            );
            Gf2mWide::<4, Gf2m256TestConfig>::from_words(reduced)
        }

        fn scalar_reference_mul_m571(
            a: &Gf2mWide<9, Gf2m571TestConfig>,
            b: &Gf2mWide<9, Gf2m571TestConfig>,
        ) -> Gf2mWide<9, Gf2m571TestConfig> {
            let mut product = [0u64; 18];
            super::clmul_wide_slice_portable::<9>(a.words(), b.words(), &mut product);
            let reduced = crate::gf2m::barrett::reference_reduce_wide::<9, 18>(
                &product,
                &Gf2m571TestConfig::MODULUS,
                571,
            );
            Gf2mWide::<9, Gf2m571TestConfig>::from_words(reduced)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

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

            /// The N = 9 counterpart: covers the initial product and Barrett's two
            /// internal products on whichever lane the dispatch selects.
            #[test]
            fn prop_simd_matches_scalar_reference_m571(
                xs in any_9_words_m571(),
                ys in any_9_words_m571(),
            ) {
                let a = Gf2mWide::<9, Gf2m571TestConfig>::new(xs);
                let b = Gf2mWide::<9, Gf2m571TestConfig>::new(ys);
                let got = a * b;
                let expected = scalar_reference_mul_m571(&a, &b);
                prop_assert_eq!(got, expected,
                    "SIMD/scalar m571 disagreement for a={:?}, b={:?}", a, b);
            }
        }
    }
}
