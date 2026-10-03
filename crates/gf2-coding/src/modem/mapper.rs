//! Batched bit-to-symbol mapper trait.

use super::{ModemScalar, ModemView};

/// Batched bit-to-symbol mapper.
///
/// # Bit ordering
///
/// `bits` is symbol-major: the first `bits_per_symbol()` entries form the
/// first symbol's label, the next `bits_per_symbol()` form the second
/// symbol, and so on. Within each symbol the order is **MSB-first**, i.e.
/// the entry at offset `i * bits_per_symbol() + 0` corresponds to the
/// most-significant bit of the [`super::LabelWord`] at the chosen
/// constellation point.
pub trait BatchMapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this mapper was
    /// constructed for.
    fn spec(&self) -> ModemView<'_, S>;

    /// Maps a batch of bits into one I/Q symbol per
    /// `self.spec().bits_per_symbol()` input bits.
    ///
    /// # Panics
    ///
    /// Implementations panic if `bits.len()` is not a multiple of
    /// `bits_per_symbol` or if `out_i.len()` or `out_q.len()` differs from
    /// `bits.len() / bits_per_symbol`.
    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]);
}

/// Lets the boxed trait object returned by
/// [`super::ModemSpec::preferred_mapper`] satisfy an `M: BatchMapper<S>`
/// bound.
impl<S: ModemScalar, T: BatchMapper<S> + ?Sized> BatchMapper<S> for Box<T> {
    #[inline]
    fn spec(&self) -> ModemView<'_, S> {
        (**self).spec()
    }

    #[inline]
    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]) {
        (**self).map_bits(bits, out_i, out_q);
    }
}
