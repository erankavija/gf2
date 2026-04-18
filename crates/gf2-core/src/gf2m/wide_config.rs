//! Configuration trait for multi-word GF(2^m) extensions.
//!
//! [`Gf2mWideConfig`] is the multi-word analogue of
//! [`crate::gfpn::ExtConfig`]: a zero-sized marker type parameterises a field
//! implementation (here [`crate::gf2m::Gf2mWide`]) with the compile-time
//! constants that define the irreducible polynomial and the extension degree.
//!
//! # Design
//!
//! The config is a zero-sized marker type — no runtime state, no per-element
//! overhead. The irreducible polynomial and the extension degree are
//! associated constants, which keeps `Gf2mWide<N, Cfg>` `Copy` and
//! `ConstField`-friendly in downstream tasks.
//!
//! # Representation
//!
//! The irreducible polynomial is stored as its **low-order `M` bits** in
//! `MODULUS`, packed little-endian across `N` `u64` words (bit `i` lives in
//! `MODULUS[i >> 6]` at position `1u64 << (i & 63)`). The leading coefficient
//! at bit `M` is **implicit and always 1**, matching the convention already
//! used by [`crate::gf2m::Gf2mField_::new`]. The invariant
//! `64 * (N - 1) < M <= 64 * N` ensures every `[u64; N]` modulus word is
//! meaningful and that no bit of a reduced element sits above the top word.
//!
//! # Example
//!
//! ```
//! use gf2_core::gf2m::Gf2mWideConfig;
//!
//! /// GF(2^256) with the irreducible pentanomial x^256 + x^10 + x^5 + x^2 + 1.
//! /// Cited from Seroussi, "Table of Low-Weight Binary Irreducible Polynomials",
//! /// HP Laboratories technical report HPL-98-135 (1998), Table 1 row m = 256.
//! struct Gf2m256Config;
//!
//! impl Gf2mWideConfig<4> for Gf2m256Config {
//!     const M: usize = 256;
//!     // x^10 + x^5 + x^2 + 1 = 0b100_0010_0101 = 0x425
//!     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
//! }
//! ```
//!
//! See [`crate::gf2m::wide::Gf2mWide`] for the element type that consumes
//! this configuration.

/// Zero-sized configuration specifying an irreducible polynomial for
/// GF(2^M), packed into `N` little-endian `u64` words.
///
/// # Type Parameter
///
/// * `N` - Number of `u64` words used to store a field element. Must satisfy
///   `64 * (N - 1) < M <= 64 * N`.
///
/// # Constants
///
/// * [`M`](Self::M) - Extension degree; the field has `2^M` elements.
/// * [`MODULUS`](Self::MODULUS) - Low-order `M` bits of the irreducible
///   polynomial, with bit `M` implicit = 1. Encoded little-endian across `N`
///   words.
///
/// # Overridable helpers
///
/// [`MODULUS_HIGH_BIT_WORD`](Self::MODULUS_HIGH_BIT_WORD) and
/// [`MODULUS_HIGH_BIT_MASK`](Self::MODULUS_HIGH_BIT_MASK) default to values
/// derived from `M`. Implementations rarely need to override them; they are
/// exposed so downstream tasks (multiplication, Barrett reduction) can reach
/// cached constants without re-deriving at each call site.
///
/// [`NAME`](Self::NAME) defaults to `"Gf2mWide"` and is used by the manual
/// `Debug` implementation on `Gf2mWide` to tag the field name.
///
/// # Irreducibility contract
///
/// Implementors **must** guarantee that `MODULUS` (together with the implicit
/// high bit at position `M`) is irreducible over GF(2). Violating this
/// breaks correctness of every multiplicative operation built on top of the
/// config — addition still works because it is word-wise XOR and is
/// polynomial independent.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::Gf2mWideConfig;
///
/// struct Gf2m256Config;
///
/// impl Gf2mWideConfig<4> for Gf2m256Config {
///     const M: usize = 256;
///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
/// }
///
/// assert_eq!(Gf2m256Config::M, 256);
/// // M = 256 means the high bit of a reduced element is bit 255, word 3.
/// assert_eq!(Gf2m256Config::MODULUS_HIGH_BIT_WORD, 3);
/// assert_eq!(Gf2m256Config::MODULUS_HIGH_BIT_MASK, 1u64 << 63);
/// ```
pub trait Gf2mWideConfig<const N: usize>: 'static {
    /// Extension degree: the field has `2^M` elements.
    ///
    /// Must satisfy `64 * (N - 1) < M <= 64 * N`. Implementations are
    /// encouraged to enforce this with a `const _: () = assert!(...);` line.
    const M: usize;

    /// Low-order `M` bits of the irreducible polynomial, little-endian across
    /// `N` `u64` words.
    ///
    /// The leading coefficient at bit `M` is **implicit** and always equal
    /// to one. For example, the polynomial `x^256 + x^10 + x^5 + x^2 + 1`
    /// is stored as `[0x425, 0, 0, 0]`.
    const MODULUS: [u64; N];

    /// Index into a `[u64; N]` element at which the highest reduced bit lives.
    ///
    /// For a reduced element of degree at most `M - 1`, the top bit occupies
    /// word `(M - 1) >> 6` at mask `1u64 << ((M - 1) & 63)`. This constant
    /// caches the word index for fast tail-masking and shift-accumulator
    /// paths in downstream code.
    const MODULUS_HIGH_BIT_WORD: usize = (Self::M - 1) >> 6;

    /// Mask selecting the highest bit of a reduced element within
    /// [`MODULUS_HIGH_BIT_WORD`](Self::MODULUS_HIGH_BIT_WORD).
    const MODULUS_HIGH_BIT_MASK: u64 = 1u64 << ((Self::M - 1) & 63);

    /// Human-readable name of the field, used by `Debug` on
    /// [`crate::gf2m::Gf2mWide`].
    const NAME: &'static str = "Gf2mWide";
}
