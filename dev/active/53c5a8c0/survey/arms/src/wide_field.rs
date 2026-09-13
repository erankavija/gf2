//! The wide GF(2^m) fields whose whole-consumer product this study measures
//! (jit:53c5a8c0).
//!
//! `Gf2mWide::<N, Cfg>::mul_ref` is the consumer operation: the unreduced
//! `2N`-word long product followed by `BarrettReducerWide::<N>::reduce_slice`.
//! The two configurations below are the widths the `gf2-kernels-simd` wide
//! kernels cover, so the composed cell measures a consumer that reaches a
//! kernel for its product and the shared reducer for its reduction. An
//! external arm that supplies only the long product composes the same reducer
//! on its own product, which is why the reducer is exposed here rather than
//! hidden inside one arm.

use gf2_core::gf2m::barrett::BarrettReducerWide;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

/// GF(2^256) with the irreducible pentanomial `x^256 + x^10 + x^5 + x^2 + 1`.
pub struct Gf2m256Config;

impl Gf2mWideConfig<4> for Gf2m256Config {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "GF(2^256)";
}

/// GF(2^571) with the irreducible pentanomial `x^571 + x^10 + x^5 + x^2 + 1`.
pub struct Gf2m571Config;

impl Gf2mWideConfig<9> for Gf2m571Config {
    const M: usize = 571;
    const MODULUS: [u64; 9] = [0x425, 0, 0, 0, 0, 0, 0, 0, 0];
    const NAME: &'static str = "GF(2^571)";
}

/// Element type of the 4-word field.
pub type Wide256 = Gf2mWide<4, Gf2m256Config>;
/// Element type of the 9-word field.
pub type Wide571 = Gf2mWide<9, Gf2m571Config>;

/// A wide-field reducer for one of the two widths a kernel covers.
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
            4 => Some(Self::Gf256(BarrettReducerWide::new(
                Gf2m256Config::MODULUS,
                Gf2m256Config::M as u32,
            ))),
            9 => Some(Self::Gf571(BarrettReducerWide::new(
                Gf2m571Config::MODULUS,
                Gf2m571Config::M as u32,
            ))),
            _ => None,
        }
    }

    /// Extension degree `m`.
    pub fn degree(&self) -> usize {
        match self {
            Self::Gf256(_) => Gf2m256Config::M,
            Self::Gf571(_) => Gf2m571Config::M,
        }
    }

    /// The field element whose coefficients are the low `m` bits of `words`.
    ///
    /// Random fixture words span `64N` bits; a GF(2^571) element occupies 571
    /// of the 576, so the reduction precondition `deg < 2m` holds only for
    /// masked operands.
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

    /// Writes the field element of `words` into `out`, without allocating.
    ///
    /// The allocating [`Self::element`] serves the setup paths; a timed
    /// whole-consumer call uses this form so the arm does not charge itself an
    /// allocation its counterpart does not pay.
    ///
    /// # Panics
    ///
    /// Panics if `out` is shorter than `words`.
    pub fn element_into(&self, words: &[u64], out: &mut [u64]) {
        let degree = self.degree();
        for (index, word) in words.iter().enumerate() {
            let low = 64 * index;
            out[index] = if low + 64 <= degree {
                *word
            } else if low >= degree {
                0
            } else {
                word & ((1u64 << (degree - low)) - 1)
            };
        }
    }

    /// Reduces an unreduced `2N`-word product to its `N`-word field element.
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

    /// Reduces `product` in place into `out`, without allocating a `Vec`.
    ///
    /// # Panics
    ///
    /// Panics if `product` does not hold exactly `2N` words or `out` fewer
    /// than `N`.
    pub fn reduce_into(&self, product: &[u64], out: &mut [u64]) {
        match self {
            Self::Gf256(reducer) => out[..4].copy_from_slice(&reducer.reduce_slice(product)),
            Self::Gf571(reducer) => out[..9].copy_from_slice(&reducer.reduce_slice(product)),
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
