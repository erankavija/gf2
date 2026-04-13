//! Batched bit-to-symbol mapper trait.
//!
//! This module defines the public, backend-agnostic interface that the
//! arbitrary-constellation reference path, the optimized Gray square-QAM
//! fast path, future SIMD kernels, and any future GPU backend implement.
//!
//! The trait deliberately:
//!
//! - Operates on flat I/Q output buffers so SIMD and GPU backends never
//!   need to reshape per call.
//! - Takes caller-provided output slices, so no allocation happens on the
//!   hot path.
//! - Uses MSB-first intra-symbol bit ordering, matching
//!   [`super::LabelWord`] and the canonical LLR ordering documented in the
//!   constellation data model plan.
//!
//! Concrete backends land in tasks `51334873` (reference path) and
//! `52112411` (Gray-QAM fast path).
//!
//! See `dev/active/d4851c3d-modem-framework-design.md` for the full design
//! rationale, especially the "Public API draft" section.

use super::{ModemScalar, ModemView};

/// Batched bit-to-symbol mapper.
///
/// Implementations consume a flat buffer of input bits and write one
/// constellation point per symbol into caller-provided I and Q slices.
/// No allocation occurs in the trait method itself; backends must size
/// any internal scratch buffers at construction time.
///
/// # Bit ordering
///
/// `bits` is symbol-major: the first `bits_per_symbol()` entries form the
/// first symbol's label, the next `bits_per_symbol()` form the second
/// symbol, and so on. Within each symbol the order is **MSB-first**, i.e.
/// the entry at offset `i * bits_per_symbol() + 0` corresponds to the
/// most-significant bit of the [`super::LabelWord`] at the chosen
/// constellation point.
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::modem::{BatchMapper, ModemScalar, ModemSpec, ModemView};
///
/// fn map_one_batch<S: ModemScalar, M: BatchMapper<S>>(
///     mapper: &M,
///     bits: &[bool],
///     out_i: &mut [S],
///     out_q: &mut [S],
/// ) {
///     let _view: ModemView<'_, S> = mapper.spec();
///     mapper.map_bits(bits, out_i, out_q);
/// }
///
/// # fn main() {}
/// ```
pub trait BatchMapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this mapper was
    /// constructed for.
    ///
    /// Backends use the view to access the constellation geometry and bit
    /// labels they were configured against; callers use it to read
    /// `bits_per_symbol()`, `num_symbols()`, and per-bit metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::modem::{BatchMapper, ModemScalar};
    /// fn bps<S: ModemScalar, M: BatchMapper<S>>(m: &M) -> u8 {
    ///     m.spec().bits_per_symbol()
    /// }
    /// # fn main() {}
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    fn spec(&self) -> ModemView<'_, S>;

    /// Maps a batch of bits into a batch of I/Q symbols.
    ///
    /// # Arguments
    ///
    /// * `bits` - Flattened, MSB-first within each symbol. Length must be
    ///   a multiple of `self.spec().bits_per_symbol()`. The number of
    ///   symbols mapped is `bits.len() / bits_per_symbol`.
    /// * `out_i` - Output slice of in-phase coordinates, one element per
    ///   mapped symbol.
    /// * `out_q` - Output slice of quadrature coordinates, one element per
    ///   mapped symbol.
    ///
    /// `out_i.len()` and `out_q.len()` must both equal
    /// `bits.len() / bits_per_symbol`.
    ///
    /// # Panics
    ///
    /// Implementations must panic with a descriptive message if any of the
    /// length contracts above are violated.
    ///
    /// # Complexity
    ///
    /// Implementation-dependent: the reference path is `O(num_symbols)`
    /// and the Gray-QAM fast path is `O(num_symbols)` with a much smaller
    /// per-symbol constant.
    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]);
}
