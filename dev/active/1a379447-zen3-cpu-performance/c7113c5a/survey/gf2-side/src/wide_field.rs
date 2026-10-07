//! The field reduction a wide GF(2^m) consumer applies after the unreduced
//! long product (jit:c7113c5a).
//!
//! `Gf2mWide::<N, _>::mul_ref` computes the unreduced `2N`-word product and
//! then reduces it with `BarrettReducerWide::<N>::reduce_slice`. The 4-word and
//! 9-word long-product cells measure the unreduced stage only; this module
//! supplies the separated reduction stage, with the moduli gf2's own
//! wide-multiplication benchmark uses, so both arms can report its cost on
//! their own product and the validator can check that the composition equals
//! the field product.

use gf2_core::gf2m::barrett::BarrettReducerWide;

/// Low words of `x^256 + x^10 + x^5 + x^2 + 1`; the leading term is implicit.
pub const GF2_256_MODULUS: [u64; 4] = [0x425, 0, 0, 0];

/// Low words of `x^571 + x^10 + x^5 + x^2 + 1`; the leading term is implicit.
pub const GF2_571_MODULUS: [u64; 9] = [0x425, 0, 0, 0, 0, 0, 0, 0, 0];

/// A wide-field reducer for one of the two dispatched operand widths.
pub enum WideReducer {
    /// GF(2^256): 4-word elements.
    Gf256(BarrettReducerWide<4>),
    /// GF(2^571): 9-word elements.
    Gf571(BarrettReducerWide<9>),
}

impl WideReducer {
    /// The reducer for `words`-word operands, when a wide field of that width
    /// has a dispatched kernel.
    pub fn for_words(words: usize) -> Option<Self> {
        match words {
            4 => Some(Self::Gf256(BarrettReducerWide::new(GF2_256_MODULUS, 256))),
            9 => Some(Self::Gf571(BarrettReducerWide::new(GF2_571_MODULUS, 571))),
            _ => None,
        }
    }

    /// Extension degree `m`.
    pub fn degree(&self) -> usize {
        match self {
            Self::Gf256(_) => 256,
            Self::Gf571(_) => 571,
        }
    }

    /// Low words of the modulus, leading term implicit.
    pub fn modulus(&self) -> &[u64] {
        match self {
            Self::Gf256(_) => &GF2_256_MODULUS,
            Self::Gf571(_) => &GF2_571_MODULUS,
        }
    }

    /// The field element whose coefficients are the low `m` bits of `words`.
    ///
    /// Random fixture words span `64N` bits; GF(2^571) elements occupy 571 of
    /// the 576, so the reduction precondition `deg < 2m` holds only for masked
    /// operands.
    pub fn element(&self, words: &[u64]) -> Vec<u64> {
        let degree = self.degree();
        words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                let low = 64 * index;
                if low + 64 <= degree {
                    *word
                } else if low >= degree {
                    0
                } else {
                    word & ((1u64 << (degree - low)) - 1)
                }
            })
            .collect()
    }

    /// Reduces an unreduced `2N`-word product of two field elements to its
    /// `N`-word field element.
    ///
    /// # Panics
    ///
    /// Panics if `product` does not hold exactly `2N` words.
    pub fn reduce(&self, product: &[u64]) -> Vec<u64> {
        match self {
            Self::Gf256(reducer) => reducer.reduce_slice(product).to_vec(),
            Self::Gf571(reducer) => reducer.reduce_slice(product).to_vec(),
        }
    }

    /// Amortised cost of one reduction of `product`, rounded up to whole
    /// nanoseconds.
    pub fn probe_ns(&self, product: &[u64]) -> u64 {
        match self {
            Self::Gf256(reducer) => crate::amortised_probe_ns(|| {
                std::hint::black_box(reducer.reduce_slice(std::hint::black_box(product)));
            }),
            Self::Gf571(reducer) => crate::amortised_probe_ns(|| {
                std::hint::black_box(reducer.reduce_slice(std::hint::black_box(product)));
            }),
        }
    }
}

/// The separated field-reduction cost of one long-product cell, or zero when
/// the operand width has no wide-field consumer.
///
/// `multiply` is the arm's own unreduced product; it runs once on the bank's
/// operands masked to field elements, and the reduction of that product is
/// what the probe times.
pub fn reduction_probe_ns(
    words: usize,
    bank: &crate::Bank,
    mut multiply: impl FnMut(&mut crate::Bank),
) -> u64 {
    let Some(reducer) = WideReducer::for_words(words) else {
        return 0;
    };
    let mut field = crate::Bank {
        a: reducer.element(&bank.a),
        b: reducer.element(&bank.b),
        out: vec![0; 2 * words],
    };
    multiply(&mut field);
    reducer.probe_ns(&field.out)
}
