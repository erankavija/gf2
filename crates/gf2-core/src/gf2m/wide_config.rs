//! Configuration trait for multi-word GF(2^m) extensions.
//!
//! [`Gf2mWideConfig`] is the multi-word analogue of
//! [`crate::gfpn::ExtConfig`]: a zero-sized marker type parameterises
//! [`crate::gf2m::Gf2mWide`] with the extension degree and the irreducible
//! polynomial. `MODULUS` leaves the leading coefficient at bit `M` implicit,
//! whereas [`crate::gf2m::Gf2mField_::new`] takes it explicit at bit `m`; the
//! field identity of a `Gf2mWide` appends it, so both carriers name the same
//! monic modulus.

/// Zero-sized configuration specifying an irreducible polynomial for
/// GF(2^M), packed into `N` little-endian `u64` words.
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
    const MODULUS_HIGH_BIT_WORD: usize = (Self::M - 1) >> 6;

    /// Mask selecting the highest bit of a reduced element within
    /// [`MODULUS_HIGH_BIT_WORD`](Self::MODULUS_HIGH_BIT_WORD).
    const MODULUS_HIGH_BIT_MASK: u64 = 1u64 << ((Self::M - 1) & 63);

    /// Human-readable name of the field.
    const NAME: &'static str = "Gf2mWide";
}
