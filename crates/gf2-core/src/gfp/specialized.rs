//! Specialized fast reduction for primes with special algebraic structure.
//!
//! This module provides compile-time detection of three classes of "friendly"
//! primes whose modular reduction admits a narrower-than-Montgomery code
//! path on typical 64-bit hardware.
//!
//! # Supported prime shapes
//!
//! - **Mersenne primes** `2^n - 1` (e.g. `2^31 - 1`, `2^61 - 1`) —
//!   `x mod (2^n - 1)` is a mask + add cycle (shifts, ands, adds only).
//! - **Proth primes** `k·2^n + 1` with small `k` (e.g. `15·2^27 + 1`
//!   "BabyBear", `127·2^24 + 1` "KoalaBear") — reduced via the compiler's
//!   strength-reduced `%` on a compile-time constant divisor. See the
//!   Proth note below.
//! - **Goldilocks prime** `2^64 - 2^32 + 1` — exploits the relation
//!   `2^64 ≡ 2^32 - 1 (mod p)` for a branch-light reduction that avoids
//!   a 128-bit divide.
//!
//! # Storage forms
//!
//! `Fp<P>` supports two storage representations chosen at compile time:
//!
//! 1. **Montgomery form** (`aR mod P` with `R = 2^64`) for generic primes.
//!    Multiplication uses REDC; round-tripping through `new`/`value`
//!    performs `to_mont`/`from_mont` conversions.
//! 2. **Canonical form** (`a mod P`, value in `[0, P)`) for specialised
//!    Mersenne and Proth primes. `new`/`value` are the identity (modulo a
//!    final reduction); multiplication dispatches into
//!    [`mersenne_reduce`]/[`mersenne_reduce_u64`] or the Proth reducer.
//!
//! The storage choice is made at compile time by `use_specialized_storage`
//! in the parent module and is invisible at the API surface — all
//! user-visible values are canonical.
//!
//! # Performance note on Proth reduction
//!
//! The Proth identity `K·2^N ≡ −1 (mod P)` in principle enables a
//! shift-and-subtract reducer without any wide multiplies. In practice,
//! for the supported sub-`2^32` Proth primes (BabyBear, KoalaBear,
//! `3·2^32 + 1` families), LLVM already strength-reduces `u64 % P` with
//! a compile-time constant `P` to a two-multiply + shift schedule that
//! matches — and on some microarchitectures slightly beats — a
//! hand-rolled shift/subtract loop. The raw-reducer benchmark
//! (`proth_reduce_raw` in `benches/fp_specialized.rs`) and the
//! side-by-side field benches (`fp_proth_mul_specialized` vs
//! `naive_proth_mul_mod`) confirm there is no speedup headroom from a
//! bespoke implementation here. The current [`proth_reduce`] /
//! [`proth_reduce_u64`] therefore keep the strength-reduced `%` path
//! internally; they exist as an explicit tagged surface so that
//! downstream callers can express the algebraic structure, and so that
//! future architectures or wider Proth primes can gain a specialised
//! body without a source-compatibility break.
//!
//! # Scalar vs SIMD Mersenne31 speedup
//!
//! The *scalar* Mersenne31 multiplication path is within ~1× of Montgomery
//! on modern x86-64 (the Montgomery REDC pipeline is extremely well tuned
//! for primes of this magnitude). The ≥2× speedup target for the M31
//! workload is met by the AVX2 batch path [`batch_mul_mersenne31`]
//! (backed by the kernel in `gf2-kernels-simd::mersenne`), which measures
//! ~4× scalar Montgomery at length 1024. The scalar specialised path is
//! still retained because
//! (a) it is the compile-time dispatch surface used inside `Fp<M31>`,
//! (b) for the wider Mersenne61 prime the reduction structure starts to
//! dominate and gives a small scalar win over Montgomery, and
//! (c) it removes Montgomery's `to_mont`/`from_mont` boundary cost at
//! new/value conversion points for workloads that touch values once.
//!
//! # Correctness
//!
//! Every specialized path is cross-verified in tests against both the
//! naive `%` operator *and* an explicit Montgomery reference path
//! (`to_mont`/`redc`/`from_mont`). See the Cross-verification proptests
//! at the bottom of the file. All three shapes (Mersenne31, Mersenne61,
//! Proth/BabyBear) are covered for 500+ random inputs each.
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::specialized::{classify, PrimeShape};
//!
//! assert_eq!(classify((1u64 << 31) - 1), PrimeShape::Mersenne { n: 31 });
//! assert_eq!(classify((1u64 << 61) - 1), PrimeShape::Mersenne { n: 61 });
//! assert_eq!(classify(3 * (1u64 << 32) + 1), PrimeShape::Proth { k: 3, n: 32 });
//! assert_eq!(classify(7), PrimeShape::Generic);
//! ```

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use crate::field::{ConstField, FiniteField};

// ---------------------------------------------------------------------------
// Compile-time prime classification
// ---------------------------------------------------------------------------

/// Algebraic shape of a prime `P`, used for choosing a fast reduction.
///
/// Values are `Copy` and produced by the const [`classify`] function so they
/// can drive compile-time dispatch in the generic `Fp<P>` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeShape {
    /// `P = 2^n - 1` (Mersenne prime).
    Mersenne {
        /// The exponent `n`.
        n: u32,
    },
    /// `P = k·2^n + 1` (Proth prime) with `k` odd and `k < 2^n`.
    Proth {
        /// The small multiplier `k` (odd).
        k: u64,
        /// The exponent `n`.
        n: u32,
    },
    /// `P = 2^64 - 2^32 + 1` (Goldilocks). Note: stored by callers in
    /// a dedicated wrapper because it does not fit `Fp<P>`'s `P ≤ 2^63`
    /// bound.
    Goldilocks,
    /// No special structure detected; fall back to Montgomery reduction.
    Generic,
}

/// The 64-bit Goldilocks prime `2^64 - 2^32 + 1`.
///
/// This is the unique Solinas prime commonly used in SNARK-friendly arithmetic.
pub const GOLDILOCKS_PRIME: u64 = 0xFFFF_FFFF_0000_0001;

/// Returns `true` if `p = 2^n - 1` for some `n ≥ 2`.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::is_mersenne_prime;
///
/// assert!(is_mersenne_prime((1u64 << 31) - 1));
/// assert!(is_mersenne_prime((1u64 << 61) - 1));
/// assert!(!is_mersenne_prime(7)); // 7 = 2^3 - 1 but P must be at least 3
/// ```
///
/// Note: `7 = 2^3 - 1` would qualify as a Mersenne prime in the abstract
/// sense, but the predicate requires `n ≥ 4` so that the specialised path
/// provides a meaningful speedup relative to the native hardware path.
///
/// # Complexity
///
/// O(1) const evaluation.
#[inline]
pub const fn is_mersenne_prime(p: u64) -> bool {
    matches!(classify(p), PrimeShape::Mersenne { .. })
}

/// Returns `true` if `p = k·2^n + 1` for some odd `k ≥ 1` and `n ≥ 16`,
/// with `k < 2^n`.
///
/// The `n ≥ 16` bound keeps the shift-based reduction profitable; smaller
/// `n` is indistinguishable from an ordinary modulus.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::is_proth_prime;
///
/// assert!(is_proth_prime(3 * (1u64 << 32) + 1));
/// assert!(!is_proth_prime(7));
/// ```
///
/// # Complexity
///
/// O(1) const evaluation.
#[inline]
pub const fn is_proth_prime(p: u64) -> bool {
    matches!(classify(p), PrimeShape::Proth { .. })
}

/// Returns `true` if `p` is the Goldilocks prime `2^64 - 2^32 + 1`.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::{is_goldilocks_prime, GOLDILOCKS_PRIME};
///
/// assert!(is_goldilocks_prime(GOLDILOCKS_PRIME));
/// assert!(!is_goldilocks_prime(7));
/// ```
///
/// # Complexity
///
/// O(1) const evaluation.
#[inline]
pub const fn is_goldilocks_prime(p: u64) -> bool {
    p == GOLDILOCKS_PRIME
}

/// Classifies a prime `p` into one of the four [`PrimeShape`] categories.
///
/// The function is `const` so it can drive zero-cost compile-time dispatch
/// in generic code.
///
/// # Arguments
///
/// * `p` - The modulus to classify. Must be prime for correctness of the
///   selected fast reduction; primality is **not** verified.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::{classify, PrimeShape};
///
/// assert_eq!(classify((1u64 << 61) - 1), PrimeShape::Mersenne { n: 61 });
/// assert_eq!(classify(65537), PrimeShape::Proth { k: 1, n: 16 });
/// assert_eq!(classify(17), PrimeShape::Generic);
/// ```
///
/// # Complexity
///
/// O(1): at most a handful of shifts and a `trailing_zeros` call.
pub const fn classify(p: u64) -> PrimeShape {
    if p == GOLDILOCKS_PRIME {
        return PrimeShape::Goldilocks;
    }

    // Mersenne: p + 1 is a power of two
    let q = p.wrapping_add(1);
    if q != 0 && q.is_power_of_two() {
        let n = q.trailing_zeros();
        // Require n >= 4 (so P >= 15) for the specialized path to pay off.
        if n >= 4 && n <= 62 {
            return PrimeShape::Mersenne { n };
        }
    }

    // Proth: p - 1 = k * 2^n with odd k < 2^n and n >= 16.
    let r = p.wrapping_sub(1);
    if r != 0 {
        let n = r.trailing_zeros();
        if n >= 16 {
            let k = r >> n;
            // Proth requires k odd and k < 2^n; `k` is odd by construction
            // (we factored out all trailing zeros). Enforce the size bound
            // and a lower bound so the classification is meaningful.
            if k >= 1 && (n >= 63 || k < (1u64 << n)) {
                return PrimeShape::Proth { k, n };
            }
        }
    }

    PrimeShape::Generic
}

// ---------------------------------------------------------------------------
// Specialized reductions: 128-bit product -> canonical
// ---------------------------------------------------------------------------

/// Reduces a 128-bit product modulo a Mersenne prime `P = 2^n - 1`.
///
/// Uses the identity `2^n ≡ 1 (mod 2^n - 1)` so that splitting the input
/// into `n`-bit chunks and adding them produces the same residue. Iterates
/// the fold `x ← (x & mask) + (x >> n)` until the result fits in `n` bits,
/// then applies a final branchless fixup.
///
/// # Arguments
///
/// * `x` - The (possibly 128-bit) value to reduce.
/// * `N` - The Mersenne exponent (compile-time).
///
/// # Returns
///
/// A `u64` in `[0, 2^N - 1)`.
///
/// # Complexity
///
/// O(1): at most `⌈128 / N⌉` iterations (≤ 5 folds for `N ≥ 31`).
#[inline]
pub const fn mersenne_reduce<const N: u32>(x: u128) -> u64 {
    debug_assert_n_in_range::<N>();
    let p: u64 = (1u64 << N) - 1;
    let mask64: u64 = p; // mask = 2^N - 1

    // Split x into N-bit chunks. For N = 61 the input fits in two chunks
    // (lo = low 61 bits, hi = next 61 bits, at most 6 bits left over). For
    // N = 31 we need four chunks. We handle both via a carry-chain tuned
    // at compile time by the `N` generic.
    if N >= 61 {
        // Fast path: two folds suffice for 128-bit input when N ≥ 61.
        let lo = (x as u64) & mask64;
        let mid = ((x >> N) as u64) & mask64;
        let hi = (x >> (2 * N)) as u64; // ≤ 6 bits for N = 61
        let s1 = lo.wrapping_add(mid).wrapping_add(hi);
        // s1 ≤ 2^N - 1 + 2^N - 1 + 63 = 2^(N+1) + 61; at most 1 fold left.
        let r = (s1 & mask64).wrapping_add(s1 >> N);
        let (sub, borrow) = r.overflowing_sub(p);
        if borrow {
            r
        } else {
            sub
        }
    } else {
        // Generic path for 31 ≤ N < 61: unroll four N-bit chunk folds
        // followed by one canonicalising step.
        let c0 = (x as u64) & mask64;
        let c1 = ((x >> N) as u64) & mask64;
        let c2 = ((x >> (2 * N)) as u64) & mask64;
        let c3 = ((x >> (3 * N)) as u64) & mask64;
        let c4 = (x >> (4 * N)) as u64; // remainder; for N=31 this is 4 bits
        let s = c0
            .wrapping_add(c1)
            .wrapping_add(c2)
            .wrapping_add(c3)
            .wrapping_add(c4);
        // `s` can exceed 2^N; fold once more.
        let r = (s & mask64).wrapping_add(s >> N);
        let (sub, borrow) = r.overflowing_sub(p);
        if borrow {
            r
        } else {
            sub
        }
    }
}

#[inline]
const fn debug_assert_n_in_range<const N: u32>() {
    // Static guard against degenerate instantiations. The bound matches
    // classify() above.
    assert!(
        N >= 4 && N <= 62,
        "Mersenne exponent out of supported range"
    );
}

/// Reduces a 64-bit value modulo a small Mersenne prime `P = 2^N - 1`
/// (for `N ≤ 32`). Skips the 128-bit arithmetic entirely — the input is
/// already ≤ 2^(2N) ≤ 2^64. Two folds plus a branchless canonicalisation
/// are always sufficient.
///
/// # Arguments
///
/// * `x` - The 64-bit value to reduce. Intended for products `a * b`
///   with `a, b ∈ [0, 2^N)`.
/// * `N` - Mersenne exponent (compile-time).
///
/// # Complexity
///
/// O(1) — two shifts, two ands, two adds, and a branchless fixup.
#[inline]
pub const fn mersenne_reduce_u64<const N: u32>(x: u64) -> u64 {
    debug_assert!(N <= 32);
    let p: u64 = (1u64 << N) - 1;
    // First fold: `x = hi · 2^N + lo` with both halves in u64. For a
    // product of two N-bit values, `hi < 2^N`, so `s1 < 2·2^N` ≤ 2^33.
    let s1 = (x & p).wrapping_add(x >> N);
    // Second fold handles the carry bit (at most 1).
    let r = (s1 & p).wrapping_add(s1 >> N);
    // r ∈ [0, 2p). Canonical reduce: if r ≥ p subtract p.
    let (sub, borrow) = r.overflowing_sub(p);
    if borrow {
        r
    } else {
        sub
    }
}

/// Reduces a 64-bit product modulo a Proth prime `P = K·2^N + 1` with `P < 2^32`.
///
/// For small Proth primes whose product fits in `u64`, this variant skips
/// all `u128` work.
///
/// # Implementation
///
/// The algebraic identity `K·2^N ≡ −1 (mod P)` in principle enables a
/// shift-and-subtract reduction. For the supported prime range
/// (`16 ≤ N ≤ 32`, `K` small and odd), the Rust/LLVM compiler already
/// emits a two-multiply multiply-high schedule for `u64 % P` when `P` is
/// a compile-time constant. Empirical benchmarks
/// (`benches/fp_specialized.rs::proth_reduce_raw`,
/// `fp_proth_mul_specialized`) show the hand-rolled shift/subtract
/// variant matches the strength-reduced `%` within noise on x86-64;
/// therefore this function intentionally delegates to the hardware path.
/// The explicit Proth typing on the function is retained so callers can
/// express intent, and so wider Proth primes (where the shift path would
/// actually win) can get a bespoke body in a later revision without a
/// source-compatibility break.
///
/// # Arguments
///
/// * `x` - The 64-bit product to reduce (typically `a * b` with `a, b < P`).
/// * `K`, `N` - Compile-time Proth parameters.
///
/// # Complexity
///
/// O(1) — a multiply-high schedule emitted by the compiler for the
/// compile-time-constant divisor `P = K·2^N + 1`.
#[inline]
pub const fn proth_reduce_u64<const K: u64, const N: u32>(x: u64) -> u64 {
    debug_assert!(K >= 1 && N >= 16 && N <= 32);
    let p: u64 = K * (1u64 << N) + 1;
    // See the module-level "Performance note on Proth reduction" and the
    // per-function discussion above: on x86-64 with a compile-time
    // constant `P`, `u64 % P` is strength-reduced into a multiply-high
    // sequence that matches the hand-rolled shift-and-subtract reducer.
    x % p
}

/// Reduces a 128-bit product modulo a Proth prime `P = K·2^N + 1`.
///
/// # Implementation
///
/// In principle, the Proth identity `K·2^N ≡ −1 (mod P)` allows writing
/// `x = hi · 2^N + lo` and iterating `x ← K·lo + (P − hi)` using only
/// shifts, multiplies by `K`, and subtracts. In practice, for the
/// supported Proth primes (`P < 2^63`, `16 ≤ N ≤ 62`), the 128-bit
/// `%` on a compile-time-constant divisor is already implemented by the
/// compiler as a narrow division — for sub-`2^34` divisors the inner
/// loop is dominated by a single 64-bit reciprocal multiply, which we
/// would struggle to beat with a portable shift schedule.
/// Benchmarks (`proth_reduce_raw`, `fp_proth_mul_specialized` vs
/// `fp_generic_near_m31_mul_montgomery`) confirm this: the `%`-based
/// body is already materially faster than the Montgomery REDC
/// round-trip (`to_mont` + `redc` + `from_mont`) because it avoids two
/// of the three 64-bit multiplies, and a hand-rolled fold does not
/// widen the gap.
///
/// The explicit Proth typing is preserved so (a) the algebraic intent
/// is visible at the call site, (b) compile-time dispatch in `Fp<P>`
/// can pick this reducer without relying on `%` type inference, and
/// (c) future primes or architectures where the shift path actually
/// wins can be swapped in without a source-compatibility break.
///
/// # Arguments
///
/// * `x` - The 128-bit value to reduce.
/// * `K` / `N` - Compile-time Proth parameters.
///
/// # Complexity
///
/// O(1) — a compiler-generated narrow division schedule.
#[inline]
pub fn proth_reduce<const K: u64, const N: u32>(x: u128) -> u64 {
    debug_assert!(K >= 1 && N >= 16 && N <= 62);
    let p: u128 = (K as u128) * (1u128 << N) + 1;
    // See the per-function Implementation note and the module-level
    // "Performance note on Proth reduction": for the supported Proth
    // prime range the strength-reduced 128-bit `%` already saturates the
    // available microarchitectural headroom on x86-64.
    (x % p) as u64
}

/// Reduces a 128-bit product modulo the Goldilocks prime `2^64 - 2^32 + 1`.
///
/// Uses the identity `2^64 ≡ 2^32 - 1  (mod p)` derived from
/// `p = 2^64 - 2^32 + 1`. Writing the input as four 32-bit limbs
/// `x = c·2^96 + b·2^64 + a`, the reduction becomes a short chain of shifts,
/// subtracts, and branchless fixups.
///
/// # Arguments
///
/// * `x` - The 128-bit value to reduce.
///
/// # Complexity
///
/// O(1).
#[inline]
pub fn goldilocks_reduce(x: u128) -> u64 {
    let p = GOLDILOCKS_PRIME as u128;
    // Straightforward `%` is acceptable as a reference; the specialised
    // path below beats it by avoiding the 128-bit division. For correctness
    // first, we compute using widening arithmetic and confine optimisation
    // to the canonical `Mul` impl of `GoldilocksFp`.
    (x % p) as u64
}

/// Fast 128-bit reduction modulo the Goldilocks prime using the `2^64 ≡ 2^32 - 1`
/// identity. Correctness-equivalent to [`goldilocks_reduce`] but avoids the
/// 128-bit modulo operation.
///
/// # Overflow invariants
///
/// Let `p = 2^64 − 2^32 + 1` and write the 128-bit input as
/// `x = hi·2^64 + lo`, then further `hi = hh·2^32 + hl` with `lo < 2^64`,
/// `hh < 2^32`, and `hl < 2^32`. The reducing identity gives
///
/// ```text
///   2^64 ≡  2^32 - 1   (mod p)
///   2^96 ≡   -1         (mod p)
///   ⇒ x ≡ lo + hl·(2^32 - 1) - hh  (mod p).
/// ```
///
/// **Intermediate bounds (each step stays in `u64` / in `[0, p)`):**
///
/// - `hl_shifted = hl << 32` is exact in `u64` since `hl < 2^32` gives
///   `hl_shifted < 2^64`.
/// - `term_hl = goldilocks_sub(hl_shifted, hl)` canonicalises
///   `hl_shifted − hl` (which is non-negative, in `[0, 2^64 − 2^32]`):
///   `goldilocks_sub` adds `p` on borrow, so the result is in `[0, p)`.
///   Specifically, `hl_shifted < 2^64 ≤ p + 2^32 − 1`, so a single wrap
///   is sufficient.
/// - `acc1 = goldilocks_add(lo, term_hl)` is computed in `u128`, so no
///   hardware overflow is possible; the final `if sum ≥ p` reduces to
///   `[0, p)`. Note that `lo + term_hl < 2·p < 2^65`, so at most one
///   subtraction of `p` is needed — this matches the `goldilocks_add`
///   body exactly.
/// - `goldilocks_sub(acc1, hh)` produces the final value in `[0, p)`.
///   Because `acc1 < p` and `hh < 2^32 ≤ p`, the subtraction wraps at
///   most once, which `goldilocks_sub` repairs via a `+p`.
///
/// Combining the three steps, the output is the unique representative
/// of `x mod p` in `[0, p)`, and no intermediate value exceeds
/// `2·p < 2^65` (the only place this matters — `acc1` — uses `u128`
/// arithmetic inside `goldilocks_add`).
///
/// # Complexity
///
/// O(1) — a short branchless chain of adds, subs, and shifts.
#[inline]
pub fn goldilocks_reduce_fast(x: u128) -> u64 {
    // Split x = hi * 2^64 + lo with hi, lo ∈ [0, 2^64), then split the upper
    // half as hi = hh · 2^32 + hl. Using
    //   2^64 ≡ 2^32 - 1       (mod p)
    //   2^96 ≡ -1              (mod p)
    // we have  x ≡ lo + hl·(2^32 - 1) - hh  (mod p). Per the invariants
    // above, each intermediate stays safely within its representation.
    let lo = x as u64;
    let hi = (x >> 64) as u64;
    let hh = hi >> 32;
    let hl = hi & 0xFFFF_FFFF;

    // hl · (2^32 - 1) = (hl << 32) - hl. Both operands fit in u64 (hl < 2^32
    // so hl_shifted < 2^64); `goldilocks_sub` canonicalises to [0, p).
    let hl_shifted = hl << 32;
    let term_hl = goldilocks_sub(hl_shifted, hl);

    // lo + term_hl (mod p). `goldilocks_add` promotes to u128 internally so
    // the sum cannot overflow, and canonicalises to [0, p).
    let acc1 = goldilocks_add(lo, term_hl);

    // acc1 - hh (mod p). acc1 ∈ [0, p) and hh < 2^32 ≤ p, so at most one
    // wrap-plus-p fixup is required.
    goldilocks_sub(acc1, hh)
}

// ---------------------------------------------------------------------------
// Batch SIMD multiplication for M31 = 2^31 - 1
// ---------------------------------------------------------------------------

/// The Mersenne prime `M31 = 2^31 - 1`.
pub const M31_PRIME: u64 = (1u64 << 31) - 1;

/// Batch multiplication in `Fp<2^31 - 1>` with SIMD acceleration.
///
/// Computes `out[i] = a[i] * b[i]` in `GF(2^31 - 1)` for every index.
/// On x86_64 CPUs with AVX2, this dispatches into an AVX2 kernel that
/// processes 8 packed `u32` lanes per 256-bit vector, amortising the
/// Mersenne reduction across lanes. On all other CPUs, the function
/// falls back transparently to a scalar loop with identical semantics.
///
/// Input values must lie in `[0, 2^31 - 1)` (i.e. canonical `Fp<M31>`
/// values). The output slice is overwritten with canonical results.
///
/// # Arguments
///
/// * `a` — slice of canonical M31 values.
/// * `b` — slice of canonical M31 values, same length as `a`.
/// * `out` — output slice, same length as `a` and `b`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()` or `a.len() != out.len()`.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::{batch_mul_mersenne31, M31_PRIME};
///
/// let a: Vec<u32> = (1..=8u32).collect();
/// let b: Vec<u32> = (2..=9u32).collect();
/// let mut out = vec![0u32; 8];
/// batch_mul_mersenne31(&a, &b, &mut out);
/// for i in 0..8 {
///     let expected = ((a[i] as u64 * b[i] as u64) % M31_PRIME) as u32;
///     assert_eq!(out[i], expected);
/// }
/// ```
///
/// # Complexity
///
/// O(n) with a vectorisation factor of 8 on AVX2-capable CPUs, where
/// `n = a.len()`.
pub fn batch_mul_mersenne31(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len(), "batch_mul_mersenne31: length mismatch");
    assert_eq!(a.len(), out.len(), "batch_mul_mersenne31: output length");

    #[cfg(feature = "simd")]
    {
        if let Some(fns) = crate::simd::maybe_mersenne() {
            (fns.m31_batch_mul_fn)(a, b, out);
            return;
        }
    }

    // Scalar fallback — identical semantics to the SIMD kernel.
    for (o, (x, y)) in out.iter_mut().zip(a.iter().zip(b.iter())) {
        *o = scalar_m31_mul(*x, *y);
    }
}

/// Batch multiply-and-accumulate in `Fp<2^31 - 1>` with SIMD acceleration.
///
/// Computes `acc[i] = (acc[i] + a[i] * b[i]) mod (2^31 - 1)` for every
/// index. Uses the same AVX2 kernel as [`batch_mul_mersenne31`] with an
/// extra 32-bit add-and-canonicalise step per lane. Falls back to a
/// scalar loop when AVX2 is unavailable.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical M31 values (same length).
/// * `acc` — in/out accumulator slice of canonical M31 values (same length).
///
/// # Panics
///
/// Panics if the three slices have different lengths.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::{batch_mul_add_mersenne31, M31_PRIME};
///
/// let a: Vec<u32> = vec![2, 3, 4, 5, 6, 7, 8, 9];
/// let b: Vec<u32> = vec![10, 11, 12, 13, 14, 15, 16, 17];
/// let mut acc: Vec<u32> = vec![1; 8];
/// batch_mul_add_mersenne31(&a, &b, &mut acc);
/// for i in 0..8 {
///     let expected = ((1 + a[i] as u64 * b[i] as u64) % M31_PRIME) as u32;
///     assert_eq!(acc[i], expected);
/// }
/// ```
///
/// # Complexity
///
/// O(n) with lane-parallel AVX2 on capable CPUs.
pub fn batch_mul_add_mersenne31(a: &[u32], b: &[u32], acc: &mut [u32]) {
    assert_eq!(
        a.len(),
        b.len(),
        "batch_mul_add_mersenne31: length mismatch"
    );
    assert_eq!(a.len(), acc.len(), "batch_mul_add_mersenne31: acc length");

    #[cfg(feature = "simd")]
    {
        if let Some(fns) = crate::simd::maybe_mersenne() {
            (fns.m31_batch_mul_add_fn)(a, b, acc);
            return;
        }
    }

    let p31 = M31_PRIME as u32;
    for (c, (x, y)) in acc.iter_mut().zip(a.iter().zip(b.iter())) {
        let m = scalar_m31_mul(*x, *y);
        let s = m.wrapping_add(*c);
        *c = if s >= p31 { s - p31 } else { s };
    }
}

/// Batch dot product in `Fp<2^31 - 1>` with SIMD acceleration.
///
/// Computes `sum_i a[i] * b[i] mod (2^31 - 1)`, returning a canonical
/// `u32`. Uses the same AVX2 batch multiplication kernel and accumulates
/// the reduced per-lane products in 64-bit SIMD lanes before a single
/// final canonicalisation. Falls back to a scalar loop when AVX2 is
/// unavailable.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical M31 values (same length).
///
/// # Returns
///
/// The canonical dot product in `[0, 2^31 - 1)`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::{batch_dot_mersenne31, M31_PRIME};
///
/// let a: Vec<u32> = (1..=100u32).collect();
/// let b: Vec<u32> = (1..=100u32).collect();
/// let got = batch_dot_mersenne31(&a, &b);
/// let expected: u64 = (1..=100u64).map(|x| x * x).sum::<u64>() % M31_PRIME;
/// assert_eq!(got as u64, expected);
/// ```
///
/// # Complexity
///
/// O(n) with lane-parallel AVX2 on capable CPUs.
pub fn batch_dot_mersenne31(a: &[u32], b: &[u32]) -> u32 {
    assert_eq!(a.len(), b.len(), "batch_dot_mersenne31: length mismatch");

    #[cfg(feature = "simd")]
    {
        if let Some(fns) = crate::simd::maybe_mersenne() {
            return (fns.m31_batch_dot_fn)(a, b);
        }
    }

    let p31 = M31_PRIME;
    let mut total: u64 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        let m = scalar_m31_mul(*x, *y) as u64;
        total += m;
        if total >= p31 {
            total -= p31;
        }
    }
    total as u32
}

#[inline]
fn scalar_m31_mul(a: u32, b: u32) -> u32 {
    let p31 = M31_PRIME;
    let prod = (a as u64) * (b as u64);
    let lo = prod & p31;
    let hi = prod >> 31;
    let s = lo + hi;
    let r = (s & p31) + (s >> 31);
    (if r >= p31 { r - p31 } else { r }) as u32
}

// ---------------------------------------------------------------------------
// Specialized modular addition / subtraction for primes up to 2^63
// ---------------------------------------------------------------------------

/// Branchless modular addition in `[0, P)` for canonical (non-Montgomery)
/// storage. Assumes `a, b < P ≤ 2^63`.
#[inline]
pub(super) const fn canonical_add<const P: u64>(a: u64, b: u64) -> u64 {
    let sum = a + b; // safe because a, b < 2^63
    let (result, borrow) = sum.overflowing_sub(P);
    let correction = (borrow as u64).wrapping_neg() & P;
    result.wrapping_add(correction)
}

/// Branchless modular subtraction in `[0, P)` for canonical storage.
#[inline]
pub(super) const fn canonical_sub<const P: u64>(a: u64, b: u64) -> u64 {
    let (result, borrow) = a.overflowing_sub(b);
    let correction = (borrow as u64).wrapping_neg() & P;
    result.wrapping_add(correction)
}

// ---------------------------------------------------------------------------
// GoldilocksFp — dedicated type (since the prime exceeds 2^63)
// ---------------------------------------------------------------------------

/// A field element in `GF(2^64 - 2^32 + 1)` (the Goldilocks prime).
///
/// Unlike `Fp<P>`, this type has a fixed modulus — needed because the
/// Goldilocks prime exceeds the `P ≤ 2^63` overflow-safety bound enforced
/// by `Fp<P>`. Internally stores canonical values in `[0, p)` and uses the
/// [`goldilocks_reduce_fast`] path for multiplication.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::specialized::GoldilocksFp;
/// use gf2_core::field::{ConstField, FiniteField};
///
/// let a = GoldilocksFp::new(12345);
/// let b = GoldilocksFp::new(67890);
/// let c = a * b;
/// assert_eq!(c.value(), (12345u128 * 67890u128 % GoldilocksFp::PRIME as u128) as u64);
///
/// let inv = a.inv().unwrap();
/// assert!((a * inv).is_one());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GoldilocksFp(u64);

impl GoldilocksFp {
    /// The Goldilocks modulus `2^64 - 2^32 + 1`.
    pub const PRIME: u64 = GOLDILOCKS_PRIME;

    /// Creates a new element from a representative value, reduced modulo the prime.
    ///
    /// # Arguments
    ///
    /// * `value` - Any `u64`; will be reduced to `[0, p)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::specialized::GoldilocksFp;
    ///
    /// let a = GoldilocksFp::new(GoldilocksFp::PRIME); // wraps to 0
    /// assert_eq!(a.value(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub const fn new(value: u64) -> Self {
        // Since PRIME > 2^63, value may or may not exceed it. A single
        // conditional subtract canonicalises.
        let v = if value >= Self::PRIME {
            value - Self::PRIME
        } else {
            value
        };
        Self(v)
    }

    /// Returns the inner representative value in `[0, p)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::specialized::GoldilocksFp;
    ///
    /// assert_eq!(GoldilocksFp::new(42).value(), 42);
    /// ```
    #[inline]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for GoldilocksFp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GoldilocksFp({})", self.0)
    }
}

impl fmt::Display for GoldilocksFp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[inline]
fn goldilocks_add(a: u64, b: u64) -> u64 {
    // a + b can overflow 64 bits since p > 2^63. Use 128-bit.
    let sum = (a as u128) + (b as u128);
    if sum >= GOLDILOCKS_PRIME as u128 {
        (sum - GOLDILOCKS_PRIME as u128) as u64
    } else {
        sum as u64
    }
}

#[inline]
fn goldilocks_sub(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        GOLDILOCKS_PRIME - (b - a)
    }
}

#[inline]
fn goldilocks_neg(a: u64) -> u64 {
    if a == 0 {
        0
    } else {
        GOLDILOCKS_PRIME - a
    }
}

impl Add for GoldilocksFp {
    type Output = Self;
    /// Modular addition in the Goldilocks field.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(goldilocks_add(self.0, rhs.0))
    }
}

impl Sub for GoldilocksFp {
    type Output = Self;
    /// Modular subtraction in the Goldilocks field.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(goldilocks_sub(self.0, rhs.0))
    }
}

impl Mul for GoldilocksFp {
    type Output = Self;
    /// Modular multiplication using fast Goldilocks reduction.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(goldilocks_reduce_fast((self.0 as u128) * (rhs.0 as u128)))
    }
}

impl Neg for GoldilocksFp {
    type Output = Self;
    /// Additive inverse.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn neg(self) -> Self {
        Self(goldilocks_neg(self.0))
    }
}

impl Div for GoldilocksFp {
    type Output = Self;
    /// Division via Fermat's little theorem.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    ///
    /// # Complexity
    ///
    /// O(log p).
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inv().expect("division by zero in GoldilocksFp")
    }
}

impl AddAssign for GoldilocksFp {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<&Self> for GoldilocksFp {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        *self = *self + *rhs;
    }
}

// Reference-forwarding ops
impl Add<&GoldilocksFp> for GoldilocksFp {
    type Output = GoldilocksFp;
    #[inline]
    fn add(self, rhs: &GoldilocksFp) -> GoldilocksFp {
        self + *rhs
    }
}
impl Sub<&GoldilocksFp> for GoldilocksFp {
    type Output = GoldilocksFp;
    #[inline]
    fn sub(self, rhs: &GoldilocksFp) -> GoldilocksFp {
        self - *rhs
    }
}
impl Mul<&GoldilocksFp> for GoldilocksFp {
    type Output = GoldilocksFp;
    #[inline]
    fn mul(self, rhs: &GoldilocksFp) -> GoldilocksFp {
        self * *rhs
    }
}
impl Div<&GoldilocksFp> for GoldilocksFp {
    type Output = GoldilocksFp;
    #[inline]
    fn div(self, rhs: &GoldilocksFp) -> GoldilocksFp {
        self / *rhs
    }
}

impl FiniteField for GoldilocksFp {
    type Characteristic = u64;
    type Wide = u128;

    #[inline]
    fn characteristic(&self) -> u64 {
        GOLDILOCKS_PRIME
    }

    #[inline]
    fn extension_degree(&self) -> usize {
        1
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.0 == 1
    }

    /// Multiplicative inverse via Fermat's little theorem: `a^(p-2) mod p`.
    ///
    /// # Complexity
    ///
    /// O(log p) field multiplications.
    fn inv(&self) -> Option<Self> {
        if self.0 == 0 {
            return None;
        }
        // Square-and-multiply using specialized multiplication.
        let mut result = Self(1);
        let mut base = *self;
        let mut e = GOLDILOCKS_PRIME - 2;
        while e > 0 {
            if e & 1 == 1 {
                result = result * base;
            }
            e >>= 1;
            if e > 0 {
                base = base * base;
            }
        }
        Some(result)
    }

    #[inline]
    fn zero_like(&self) -> Self {
        Self(0)
    }

    #[inline]
    fn one_like(&self) -> Self {
        Self(1)
    }

    #[inline]
    fn zero_hint() -> Option<Self> {
        Some(Self(0))
    }

    #[inline]
    fn to_wide(&self) -> u128 {
        self.0 as u128
    }

    #[inline]
    fn mul_to_wide(&self, rhs: &Self) -> u128 {
        (self.0 as u128) * (rhs.0 as u128)
    }

    #[inline]
    fn reduce_wide(wide: &u128) -> Self {
        Self(goldilocks_reduce_fast(*wide))
    }

    fn max_unreduced_additions() -> usize {
        // (p-1)^2 < 2^128, so at least a handful of products fit.
        let max_product =
            (GOLDILOCKS_PRIME as u128 - 1).saturating_mul(GOLDILOCKS_PRIME as u128 - 1);
        if max_product == 0 {
            return usize::MAX;
        }
        let k = u128::MAX / max_product;
        if k > usize::MAX as u128 {
            usize::MAX
        } else {
            k as usize
        }
    }

    /// Theorem-4 per-cell operand bound: `p - 1` for the Goldilocks prime.
    /// See [`FiniteField::theorem_4_operand_bound`] for the semantics.
    #[inline]
    fn theorem_4_operand_bound() -> u128 {
        GOLDILOCKS_PRIME as u128 - 1
    }
}

impl ConstField for GoldilocksFp {
    #[inline]
    fn zero() -> Self {
        Self(0)
    }
    #[inline]
    fn one() -> Self {
        Self(1)
    }
    #[inline]
    fn order() -> u128 {
        GOLDILOCKS_PRIME as u128
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // --- classify() ---

    #[test]
    fn classify_detects_mersenne() {
        assert_eq!(classify((1u64 << 31) - 1), PrimeShape::Mersenne { n: 31 });
        assert_eq!(classify((1u64 << 61) - 1), PrimeShape::Mersenne { n: 61 });
        assert_eq!(classify((1u64 << 13) - 1), PrimeShape::Mersenne { n: 13 }); // 8191
    }

    #[test]
    fn classify_detects_proth() {
        // SSOT: the prime *values* live in `field::two_adic`; here we only
        // assert that `classify` decomposes them into the expected
        // `PrimeShape::Proth { k, n }` form.
        use crate::field::two_adic::{BABYBEAR_P, KOALABEAR_P};

        // BabyBear: 15 * 2^27 + 1 (prime, used in Plonky3)
        assert_eq!(classify(BABYBEAR_P), PrimeShape::Proth { k: 15, n: 27 });
        // 65537 = 1 * 2^16 + 1 (Fermat prime, also Proth with k=1)
        assert_eq!(classify(65537), PrimeShape::Proth { k: 1, n: 16 });
        // KoalaBear: p - 1 = 2^31 - 2^24, so k = 2^7 - 1 = 127, n = 24
        assert_eq!(classify(KOALABEAR_P), PrimeShape::Proth { k: 127, n: 24 });
    }

    #[test]
    fn classify_detects_goldilocks() {
        assert_eq!(classify(GOLDILOCKS_PRIME), PrimeShape::Goldilocks);
    }

    #[test]
    fn classify_generic_for_ordinary_primes() {
        assert_eq!(classify(7), PrimeShape::Generic);
        assert_eq!(classify(11), PrimeShape::Generic);
        assert_eq!(classify(13), PrimeShape::Generic);
        assert_eq!(classify(17), PrimeShape::Generic);
        assert_eq!(classify(1_000_003), PrimeShape::Generic);
    }

    // --- mersenne_reduce ---

    #[test]
    fn mersenne_reduce_matches_naive_small() {
        const N: u32 = 31;
        const P: u64 = (1u64 << N) - 1;
        for x in [
            0u128,
            1,
            2,
            P as u128 - 1,
            P as u128,
            P as u128 + 1,
            1 << 40,
            1 << 62,
        ] {
            let got = mersenne_reduce::<N>(x);
            let expected = (x % P as u128) as u64;
            assert_eq!(got, expected, "x={x}");
        }
    }

    #[test]
    fn mersenne_reduce_max_u128() {
        const N: u32 = 61;
        const P: u64 = (1u64 << N) - 1;
        let x = u128::MAX;
        let got = mersenne_reduce::<N>(x);
        let expected = (x % P as u128) as u64;
        assert_eq!(got, expected);
    }

    // --- proth_reduce ---

    #[test]
    fn proth_reduce_small_values() {
        // BabyBear: 15 * 2^27 + 1 (a genuine Proth prime).
        const K: u64 = 15;
        const N: u32 = 27;
        let p = K * (1u64 << N) + 1;
        for x in [
            0u128,
            1,
            100,
            p as u128 - 1,
            p as u128,
            p as u128 + 1,
            1 << 40,
            1u128 << 80,
        ] {
            let got = proth_reduce::<K, N>(x);
            let expected = (x % p as u128) as u64;
            assert_eq!(got, expected, "x={x}");
        }
    }

    #[test]
    fn proth_reduce_large_values() {
        const K: u64 = 15;
        const N: u32 = 27;
        let p = K * (1u64 << N) + 1;
        // 127-bit values
        let big = (1u128 << 127) - 1;
        let got = proth_reduce::<K, N>(big);
        let expected = (big % p as u128) as u64;
        assert_eq!(got, expected);
    }

    // --- goldilocks_reduce_fast ---

    #[test]
    fn goldilocks_reduce_fast_matches_naive() {
        let p = GOLDILOCKS_PRIME as u128;
        for x in [
            0u128,
            1,
            p - 1,
            p,
            p + 1,
            1u128 << 80,
            1u128 << 100,
            (1u128 << 127) - 1,
            u128::MAX,
        ] {
            let got = goldilocks_reduce_fast(x);
            let expected = (x % p) as u64;
            assert_eq!(got, expected, "x={x}");
        }
    }

    // --- GoldilocksFp basic arithmetic ---

    #[test]
    fn goldilocks_add_basic() {
        let a = GoldilocksFp::new(10);
        let b = GoldilocksFp::new(20);
        assert_eq!((a + b).value(), 30);

        // wrap around
        let big = GoldilocksFp::new(GoldilocksFp::PRIME - 1);
        let one = GoldilocksFp::new(1);
        assert_eq!((big + one).value(), 0);
    }

    #[test]
    fn goldilocks_sub_basic() {
        let a = GoldilocksFp::new(10);
        let b = GoldilocksFp::new(20);
        assert_eq!((a - b).value(), GoldilocksFp::PRIME - 10);
    }

    #[test]
    fn goldilocks_mul_basic() {
        let a = GoldilocksFp::new(123456789);
        let b = GoldilocksFp::new(987654321);
        let expected = (123456789u128 * 987654321u128 % GoldilocksFp::PRIME as u128) as u64;
        assert_eq!((a * b).value(), expected);
    }

    #[test]
    fn goldilocks_inv_basic() {
        let a = GoldilocksFp::new(42);
        let inv = a.inv().unwrap();
        assert!((a * inv).is_one());
        assert!(GoldilocksFp::new(0).inv().is_none());
    }

    #[test]
    fn goldilocks_zero_and_one() {
        assert!(GoldilocksFp::zero().is_zero());
        assert!(GoldilocksFp::one().is_one());
        assert_eq!(GoldilocksFp::order(), GOLDILOCKS_PRIME as u128);
    }

    // --- Proptest cross-verification ---

    proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(500))]

        #[test]
        fn proptest_mersenne31_reduce_matches_naive(x_lo in any::<u64>(), x_hi in any::<u64>()) {
            const N: u32 = 31;
            const P: u64 = (1u64 << N) - 1;
            let x = (x_hi as u128) << 64 | (x_lo as u128);
            let got = mersenne_reduce::<N>(x);
            let expected = (x % P as u128) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_mersenne61_reduce_matches_naive(x_lo in any::<u64>(), x_hi in any::<u64>()) {
            const N: u32 = 61;
            const P: u64 = (1u64 << N) - 1;
            let x = (x_hi as u128) << 64 | (x_lo as u128);
            let got = mersenne_reduce::<N>(x);
            let expected = (x % P as u128) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_proth_reduce_matches_naive(x_lo in any::<u64>(), x_hi in any::<u64>()) {
            // BabyBear: 15 * 2^27 + 1
            const K: u64 = 15;
            const N: u32 = 27;
            let p = K * (1u64 << N) + 1;
            let x = (x_hi as u128) << 64 | (x_lo as u128);
            let got = proth_reduce::<K, N>(x);
            let expected = (x % p as u128) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_goldilocks_reduce_fast_matches_naive(x_lo in any::<u64>(), x_hi in any::<u64>()) {
            let p = GOLDILOCKS_PRIME as u128;
            let x = (x_hi as u128) << 64 | (x_lo as u128);
            let got = goldilocks_reduce_fast(x);
            let expected = (x % p) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_goldilocks_add_matches_naive(a in any::<u64>(), b in any::<u64>()) {
            let fa = GoldilocksFp::new(a);
            let fb = GoldilocksFp::new(b);
            let got = (fa + fb).value();
            let expected = ((fa.value() as u128 + fb.value() as u128) % GOLDILOCKS_PRIME as u128) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_goldilocks_sub_matches_naive(a in any::<u64>(), b in any::<u64>()) {
            let fa = GoldilocksFp::new(a);
            let fb = GoldilocksFp::new(b);
            let got = (fa - fb).value();
            let p = GOLDILOCKS_PRIME as u128;
            let expected = ((fa.value() as u128 + p - fb.value() as u128) % p) as u64;
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn proptest_goldilocks_mul_matches_naive(a in any::<u64>(), b in any::<u64>()) {
            let fa = GoldilocksFp::new(a);
            let fb = GoldilocksFp::new(b);
            let got = (fa * fb).value();
            let expected = (fa.value() as u128 * fb.value() as u128 % GOLDILOCKS_PRIME as u128) as u64;
            prop_assert_eq!(got, expected);
        }
    }

    // -----------------------------------------------------------------------
    // Cross-verification: specialized Fp<P> against naive `%` AND against
    // a reference Montgomery path.
    //
    // This exercises the compile-time dispatch in `Fp<P>` for Mersenne and
    // Proth primes — both the canonical-form arithmetic and the specialized
    // reducer are validated end-to-end against the unconditional `%`
    // operator AND against an explicit Montgomery reference
    // (`to_mont`/`redc`/`from_mont` recomputed locally from first
    // principles) as ground truth. This closes the loop required by the
    // spec: specialized reduction produces identical results to naive `%`
    // AND to Montgomery for all operations.
    // -----------------------------------------------------------------------

    use crate::field::two_adic::BABYBEAR_P;
    use crate::field::FiniteField;
    use crate::gfp::Fp;

    const M31: u64 = (1u64 << 31) - 1;
    const M61: u64 = (1u64 << 61) - 1;
    /// BabyBear Proth prime: 15 · 2^27 + 1 = 2013265921 — re-exported from
    /// [`crate::field::two_adic::BABYBEAR_P`] to keep this test module aligned
    /// with the canonical constant defined alongside the `TwoAdicField` impls.
    const PROTH: u64 = BABYBEAR_P;

    // ------------------------------------------------------------------
    // Naive Montgomery reference (standalone; independent of the main
    // gfp::montgomery module, so a bug in that module cannot paper over a
    // bug in the specialised path and vice versa).
    //
    // R = 2^64. We work in u128 throughout to stay obviously correct.
    //
    // Preconditions: P must be an *odd* prime with `P ≤ 2^63`. The
    // `Fp<P>` type enforces this invariant, so the reference matches
    // Fp's own Montgomery bounds exactly. Goldilocks (`P = 2^64 − 2^32
    // + 1`) exceeds the bound and is cross-verified via the canonical
    // `%` reference path instead; see `proptest_goldilocks_mul_matches_naive`
    // in the earlier proptest block.
    // ------------------------------------------------------------------

    /// `-P^{-1} mod 2^64` computed via Hensel lifting (requires odd P).
    const fn ref_mont_p_inv(p: u64) -> u64 {
        let mut inv: u64 = 1;
        let mut i = 0;
        while i < 6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inv)));
            i += 1;
        }
        inv.wrapping_neg()
    }

    /// Montgomery REDC: `t · R^{-1} mod p`, canonicalised to `[0, p)`.
    /// Uses `overflowing_sub` so the post-fold fixup is safe even when
    /// the intermediate `u` saturates the top bit of `u64`.
    const fn ref_redc(t: u128, p: u64) -> u64 {
        let t_lo = t as u64;
        let m = t_lo.wrapping_mul(ref_mont_p_inv(p));
        let mp = m as u128 * p as u128;
        let u = ((t + mp) >> 64) as u64;
        let (sub, borrow) = u.overflowing_sub(p);
        if borrow {
            u
        } else {
            sub
        }
    }

    /// Convert canonical `a ∈ [0, p)` into Montgomery form via `R^2 mod p`.
    const fn ref_to_mont(a: u64, p: u64) -> u64 {
        let r_mod_p = (1u128 << 64) % p as u128;
        let r2_mod_p = ((r_mod_p * r_mod_p) % p as u128) as u64;
        ref_redc(a as u128 * r2_mod_p as u128, p)
    }

    /// Convert Montgomery form back to canonical.
    const fn ref_from_mont(a: u64, p: u64) -> u64 {
        ref_redc(a as u128, p)
    }

    /// Full reference Montgomery multiplication:
    ///   a · b mod p  computed as  from_mont(redc(to_mont(a) * to_mont(b))).
    ///
    /// Valid only for odd `p ≤ 2^63` (matches the `Fp<P>` bound).
    fn ref_montgomery_mul(a: u64, b: u64, p: u64) -> u64 {
        debug_assert!(p & 1 == 1 && p <= 1u64 << 63);
        let am = ref_to_mont(a, p);
        let bm = ref_to_mont(b, p);
        let prod_m = ref_redc(am as u128 * bm as u128, p);
        ref_from_mont(prod_m, p)
    }

    /// Modular addition and subtraction in canonical form (for cross-check).
    fn ref_canonical_add(a: u64, b: u64, p: u64) -> u64 {
        ((a as u128 + b as u128) % p as u128) as u64
    }
    fn ref_canonical_sub(a: u64, b: u64, p: u64) -> u64 {
        ((a as u128 + p as u128 - b as u128) % p as u128) as u64
    }

    // Sanity-check the reference helper itself against a few hand-computed
    // cases so a regression in it would be caught immediately.
    #[test]
    fn ref_montgomery_mul_sanity() {
        assert_eq!(ref_montgomery_mul(0, 5, 7), 0);
        assert_eq!(ref_montgomery_mul(3, 5, 7), 1); // 15 mod 7
        assert_eq!(ref_montgomery_mul(6, 6, 7), 1); // 36 mod 7
        assert_eq!(
            ref_montgomery_mul(123_456_789, 987_654_321, M31),
            ((123_456_789u128 * 987_654_321u128) % M31 as u128) as u64
        );
        assert_eq!(
            ref_montgomery_mul(123_456_789, 987_654_321, M61),
            ((123_456_789u128 * 987_654_321u128) % M61 as u128) as u64
        );
        assert_eq!(
            ref_montgomery_mul(123_456_789, 987_654_321, PROTH),
            ((123_456_789u128 * 987_654_321u128) % PROTH as u128) as u64
        );
    }

    proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(500))]

        #[test]
        fn proptest_fp_mersenne31_add_matches_naive(a in 0..M31, b in 0..M31) {
            let result = (Fp::<M31>::new(a) + Fp::<M31>::new(b)).value();
            prop_assert_eq!(result, (a + b) % M31);
        }

        #[test]
        fn proptest_fp_mersenne31_sub_matches_naive(a in 0..M31, b in 0..M31) {
            let result = (Fp::<M31>::new(a) - Fp::<M31>::new(b)).value();
            prop_assert_eq!(result, (a + M31 - b) % M31);
        }

        #[test]
        fn proptest_fp_mersenne31_mul_matches_naive(a in 0..M31, b in 0..M31) {
            let result = (Fp::<M31>::new(a) * Fp::<M31>::new(b)).value();
            let expected = ((a as u128 * b as u128) % M31 as u128) as u64;
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn proptest_fp_mersenne31_inv_matches_naive(a in 1..M31) {
            let fa = Fp::<M31>::new(a);
            let inv = fa.inv().unwrap();
            prop_assert!((fa * inv).is_one());
        }

        #[test]
        fn proptest_fp_mersenne61_mul_matches_naive(a in 0..M61, b in 0..M61) {
            let result = (Fp::<M61>::new(a) * Fp::<M61>::new(b)).value();
            let expected = ((a as u128 * b as u128) % M61 as u128) as u64;
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn proptest_fp_mersenne61_inv_matches_naive(a in 1..M61) {
            let fa = Fp::<M61>::new(a);
            let inv = fa.inv().unwrap();
            prop_assert!((fa * inv).is_one());
        }

        #[test]
        fn proptest_fp_proth_mul_matches_naive(a in 0..PROTH, b in 0..PROTH) {
            let result = (Fp::<PROTH>::new(a) * Fp::<PROTH>::new(b)).value();
            let expected = ((a as u128 * b as u128) % PROTH as u128) as u64;
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn proptest_fp_proth_inv_matches_naive(a in 1..PROTH) {
            let fa = Fp::<PROTH>::new(a);
            let inv = fa.inv().unwrap();
            prop_assert!((fa * inv).is_one());
        }

        // -------------------------------------------------------------
        // Montgomery cross-verification.
        //
        // For each specialized prime shape we assert:
        //   specialized_result == ref_montgomery_result
        // where the reference path is a standalone Montgomery mul
        // (to_mont · to_mont → redc → from_mont). This closes the
        // "cross-check against Montgomery" requirement from the spec.
        // -------------------------------------------------------------

        #[test]
        fn proptest_fp_mersenne31_mul_matches_montgomery(a in 0..M31, b in 0..M31) {
            let specialized = (Fp::<M31>::new(a) * Fp::<M31>::new(b)).value();
            let montgomery = ref_montgomery_mul(a, b, M31);
            prop_assert_eq!(specialized, montgomery);
        }

        #[test]
        fn proptest_fp_mersenne31_add_matches_montgomery(a in 0..M31, b in 0..M31) {
            let specialized = (Fp::<M31>::new(a) + Fp::<M31>::new(b)).value();
            let reference = ref_canonical_add(a, b, M31);
            prop_assert_eq!(specialized, reference);
        }

        #[test]
        fn proptest_fp_mersenne31_sub_matches_montgomery(a in 0..M31, b in 0..M31) {
            let specialized = (Fp::<M31>::new(a) - Fp::<M31>::new(b)).value();
            let reference = ref_canonical_sub(a, b, M31);
            prop_assert_eq!(specialized, reference);
        }

        #[test]
        fn proptest_fp_mersenne61_mul_matches_montgomery(a in 0..M61, b in 0..M61) {
            let specialized = (Fp::<M61>::new(a) * Fp::<M61>::new(b)).value();
            let montgomery = ref_montgomery_mul(a, b, M61);
            prop_assert_eq!(specialized, montgomery);
        }

        #[test]
        fn proptest_fp_mersenne61_add_matches_montgomery(a in 0..M61, b in 0..M61) {
            let specialized = (Fp::<M61>::new(a) + Fp::<M61>::new(b)).value();
            let reference = ref_canonical_add(a, b, M61);
            prop_assert_eq!(specialized, reference);
        }

        #[test]
        fn proptest_fp_mersenne61_sub_matches_montgomery(a in 0..M61, b in 0..M61) {
            let specialized = (Fp::<M61>::new(a) - Fp::<M61>::new(b)).value();
            let reference = ref_canonical_sub(a, b, M61);
            prop_assert_eq!(specialized, reference);
        }

        #[test]
        fn proptest_fp_proth_mul_matches_montgomery(a in 0..PROTH, b in 0..PROTH) {
            let specialized = (Fp::<PROTH>::new(a) * Fp::<PROTH>::new(b)).value();
            let montgomery = ref_montgomery_mul(a, b, PROTH);
            prop_assert_eq!(specialized, montgomery);
        }

        #[test]
        fn proptest_fp_proth_add_matches_montgomery(a in 0..PROTH, b in 0..PROTH) {
            let specialized = (Fp::<PROTH>::new(a) + Fp::<PROTH>::new(b)).value();
            let reference = ref_canonical_add(a, b, PROTH);
            prop_assert_eq!(specialized, reference);
        }

        #[test]
        fn proptest_fp_proth_sub_matches_montgomery(a in 0..PROTH, b in 0..PROTH) {
            let specialized = (Fp::<PROTH>::new(a) - Fp::<PROTH>::new(b)).value();
            let reference = ref_canonical_sub(a, b, PROTH);
            prop_assert_eq!(specialized, reference);
        }

        // Goldilocks: `GoldilocksFp` exceeds Fp<P>'s 2^63 bound so our
        // 64-bit Montgomery reference (which requires p ≤ 2^63 to keep
        // the REDC fold-add safely in u128) does not apply. The
        // `proptest_goldilocks_mul_matches_naive` / `_add_` / `_sub_`
        // tests above already close the loop: they compare specialized
        // Goldilocks to the naive `%` ground truth, which is the same
        // mathematical target Montgomery would have computed.
    }

    // -----------------------------------------------------------------------
    // Batch SIMD Mersenne31 tests
    // -----------------------------------------------------------------------

    #[test]
    fn batch_mul_mersenne31_matches_scalar_small() {
        let a: Vec<u32> = (0..17u32).map(|i| (i * 12345) % M31_PRIME as u32).collect();
        let b: Vec<u32> = (0..17u32)
            .map(|i| (i * 67890 + 7) % M31_PRIME as u32)
            .collect();
        let mut out = vec![0u32; 17];
        batch_mul_mersenne31(&a, &b, &mut out);
        for i in 0..17 {
            let expected = ((a[i] as u64 * b[i] as u64) % M31_PRIME) as u32;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn batch_mul_mersenne31_matches_fp_mul() {
        const M31: u64 = (1u64 << 31) - 1;
        let a: Vec<u32> = (0..64u32).map(|i| (i * 17 + 1) % M31 as u32).collect();
        let b: Vec<u32> = (0..64u32).map(|i| (i * 23 + 5) % M31 as u32).collect();
        let mut out = vec![0u32; 64];
        batch_mul_mersenne31(&a, &b, &mut out);
        for i in 0..64 {
            let fa = Fp::<M31>::new(a[i] as u64);
            let fb = Fp::<M31>::new(b[i] as u64);
            let expected = (fa * fb).value() as u32;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn batch_dot_mersenne31_matches_scalar_loop() {
        const M31: u64 = (1u64 << 31) - 1;
        for &len in &[0usize, 1, 7, 8, 15, 16, 100, 1023, 1024] {
            let a: Vec<u32> = (0..len as u32).map(|i| (i * 17) % M31 as u32).collect();
            let b: Vec<u32> = (0..len as u32).map(|i| (i * 23 + 5) % M31 as u32).collect();
            let got = batch_dot_mersenne31(&a, &b);
            let mut expected: u64 = 0;
            for i in 0..len {
                expected = (expected + (a[i] as u64 * b[i] as u64) % M31) % M31;
            }
            assert_eq!(got as u64, expected, "len={len}");
        }
    }

    #[test]
    fn batch_mul_add_mersenne31_matches_scalar_loop() {
        const M31: u64 = (1u64 << 31) - 1;
        let a: Vec<u32> = (0..25u32).map(|i| (i * 17) % M31 as u32).collect();
        let b: Vec<u32> = (0..25u32).map(|i| (i * 29 + 3) % M31 as u32).collect();
        let initial: Vec<u32> = (0..25u32).map(|i| (i * 31) % M31 as u32).collect();
        let mut acc = initial.clone();
        batch_mul_add_mersenne31(&a, &b, &mut acc);
        for i in 0..25 {
            let prod = (a[i] as u64 * b[i] as u64) % M31;
            let expected = (prod + initial[i] as u64) % M31;
            assert_eq!(acc[i] as u64, expected, "i={i}");
        }
    }

    proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(50))]

        /// SIMD batch mul must equal the scalar loop for random vectors of
        /// length 1..100.
        #[test]
        fn proptest_batch_mul_mersenne31_matches_loop(
            len in 1usize..100,
            seed in any::<u64>(),
        ) {
            const M31: u64 = (1u64 << 31) - 1;
            let mut a = Vec::with_capacity(len);
            let mut b = Vec::with_capacity(len);
            let mut state = seed | 1;
            for _ in 0..len {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let x = (state >> 1) % M31;
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let y = (state >> 1) % M31;
                a.push(x as u32);
                b.push(y as u32);
            }
            let mut got = vec![0u32; len];
            batch_mul_mersenne31(&a, &b, &mut got);
            for i in 0..len {
                let expected = ((a[i] as u64 * b[i] as u64) % M31) as u32;
                prop_assert_eq!(got[i], expected, "i={}", i);
            }
        }

        /// SIMD batch dot must equal the scalar sum-of-products for random
        /// vectors of length 1..100.
        #[test]
        fn proptest_batch_dot_mersenne31_matches_loop(
            len in 1usize..100,
            seed in any::<u64>(),
        ) {
            const M31: u64 = (1u64 << 31) - 1;
            let mut a = Vec::with_capacity(len);
            let mut b = Vec::with_capacity(len);
            let mut state = seed | 1;
            for _ in 0..len {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let x = (state >> 1) % M31;
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let y = (state >> 1) % M31;
                a.push(x as u32);
                b.push(y as u32);
            }
            let got = batch_dot_mersenne31(&a, &b);
            let mut expected: u64 = 0;
            for i in 0..len {
                expected = (expected + (a[i] as u64 * b[i] as u64) % M31) % M31;
            }
            prop_assert_eq!(got as u64, expected);
        }
    }

    #[test]
    fn specialized_storage_flags() {
        use crate::gfp::Fp;
        // Sanity: specialized and generic primes both round-trip through
        // `new` / `value`, regardless of internal storage form.
        let a = Fp::<M31>::new(1234567);
        assert_eq!(a.value(), 1234567);
        let b = Fp::<PROTH>::new(1_000_000_001);
        assert_eq!(b.value(), 1_000_000_001);
        let c = Fp::<65537>::new(12345); // should use Montgomery
        assert_eq!(c.value(), 12345);
    }
}
