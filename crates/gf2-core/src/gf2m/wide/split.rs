//! The long carry-less product: Karatsuba down to the widest kernel the
//! canonical dispatch runs.

use std::cell::RefCell;
use std::num::NonZeroUsize;

#[cfg(feature = "simd")]
use super::clmul_wide_dispatch_enabled;
use super::{record_clmul_wide_lane, ProductWrite, PORTABLE_LANE};

/// Bound of the recursive split in [`clmul_slice_with`]: operands of at most
/// `basecase_words` words multiply as one schoolbook product, through a
/// kernel when one is that wide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClmulSplit {
    basecase_words: NonZeroUsize,
}

impl ClmulSplit {
    /// The split with basecase width `basecase_words`.
    #[must_use]
    pub const fn new(basecase_words: NonZeroUsize) -> Self {
        Self { basecase_words }
    }

    /// The width of the widest kernel the canonical dispatch runs on this
    /// host: 9 words, else 4, else one word on the portable lane, where the
    /// split reduces to single-word products.
    #[must_use]
    pub fn structural() -> Self {
        let words = match Kernels::detect() {
            Kernels { k9: Some(_), .. } => 9,
            Kernels { k4: Some(_), .. } => 4,
            _ => 1,
        };
        Self::new(NonZeroUsize::new(words).expect("a positive width"))
    }

    /// Returns the basecase width in words.
    #[must_use]
    pub const fn basecase_words(self) -> usize {
        self.basecase_words.get()
    }
}

/// XOR-accumulates the carry-less product of two `n`-word operands into the
/// `2n`-word `out`, splitting at [`ClmulSplit::structural`].
///
/// Bit `i` of a slice is bit `i & 63` of word `i >> 6`, as in
/// [`super::clmul_wide_slice`], which routes its long widths here.
///
/// # Panics
///
/// Panics unless `a.len() == b.len()` and `out.len() == 2 * a.len()`.
///
/// # Complexity
///
/// `O(n^log2(3))` basecase products and `O(n log n)` word XORs, with scratch
/// of about `8n` words held per thread across calls.
pub fn clmul_slice(a: &[u64], b: &[u64], out: &mut [u64]) {
    clmul_slice_with(a, b, out, ClmulSplit::structural());
}

/// [`clmul_slice`] under the caller's `split`.
///
/// # Panics
///
/// As [`clmul_slice`].
pub fn clmul_slice_with(a: &[u64], b: &[u64], out: &mut [u64], split: ClmulSplit) {
    check_lengths(a, b, out);
    clmul_into(a, b, out, split, ProductWrite::Accumulate);
}

/// Replaces the `2n` words of `out` with the carry-less product of two
/// `n`-word operands, as [`clmul_slice`] computes it without reading `out`.
///
/// # Panics
///
/// As [`clmul_slice`].
pub fn clmul_slice_overwrite(a: &[u64], b: &[u64], out: &mut [u64]) {
    clmul_slice_overwrite_with(a, b, out, ClmulSplit::structural());
}

/// [`clmul_slice_overwrite`] under the caller's `split`.
///
/// # Panics
///
/// As [`clmul_slice`].
pub fn clmul_slice_overwrite_with(a: &[u64], b: &[u64], out: &mut [u64], split: ClmulSplit) {
    check_lengths(a, b, out);
    clmul_into(a, b, out, split, ProductWrite::Overwrite);
}

fn check_lengths(a: &[u64], b: &[u64], out: &[u64]) {
    assert_eq!(a.len(), b.len(), "clmul_slice: operand lengths differ");
    assert_eq!(
        out.len(),
        2 * a.len(),
        "clmul_slice: product length must be twice the operand length"
    );
}

/// The split product written to `out` as `write` says; lengths as
/// [`clmul_slice`] checks them.
pub(super) fn clmul_into(
    a: &[u64],
    b: &[u64],
    out: &mut [u64],
    split: ClmulSplit,
    write: ProductWrite,
) {
    let n = a.len();
    if n == 0 {
        return;
    }
    let kernels = Kernels::detect();
    let basecase = split.basecase_words();
    let product_words = if write == ProductWrite::Accumulate {
        2 * n
    } else {
        0
    };
    let mut run = |scratch: &mut Vec<u64>| {
        let need = product_words + scratch_words(n, basecase);
        if scratch.len() < need {
            scratch.resize(need, 0);
        }
        let (product, rest) = scratch.split_at_mut(product_words);
        match write {
            ProductWrite::Overwrite => karatsuba(a, b, out, rest, basecase, &kernels),
            ProductWrite::Accumulate => {
                karatsuba(a, b, product, rest, basecase, &kernels);
                xor(out, product);
            }
        }
    };
    SCRATCH.with(|cell| match cell.try_borrow_mut() {
        Ok(mut scratch) => run(&mut scratch),
        // A product issued while this thread's scratch is held runs on its own.
        Err(_) => run(&mut Vec::new()),
    });
}

std::thread_local! {
    static SCRATCH: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Scratch words [`karatsuba`] uses for `n`-word operands.
fn scratch_words(n: usize, basecase: usize) -> usize {
    if n <= basecase {
        0
    } else {
        let h = n.div_ceil(2);
        4 * h + scratch_words(h, basecase)
    }
}

/// Overwrites the `2n` words of `out` with the product of the `n`-word `a`
/// and `b`: with `h = ceil(n/2)` and `a = a0 + x^(64h) a1`, `b` alike, the
/// product is `P0 + x^(64h) (P0 + P1 + P2) + x^(128h) P2` over `P0 = a0 b0`,
/// `P2 = a1 b1` and `P1 = (a0 + a1)(b0 + b1)`.
fn karatsuba(
    a: &[u64],
    b: &[u64],
    out: &mut [u64],
    scratch: &mut [u64],
    basecase: usize,
    kernels: &Kernels,
) {
    let n = a.len();
    if n <= basecase {
        kernels.product(a, b, out);
        return;
    }
    let h = n.div_ceil(2);
    let l = n - h;
    let (a0, a1) = a.split_at(h);
    let (b0, b1) = b.split_at(h);
    let (sum_a, rest) = scratch.split_at_mut(h);
    let (sum_b, rest) = rest.split_at_mut(h);
    let (middle, rest) = rest.split_at_mut(2 * h);
    {
        let (low, high) = out.split_at_mut(2 * h);
        karatsuba(a0, b0, low, rest, basecase, kernels);
        karatsuba(a1, b1, high, rest, basecase, kernels);
    }
    for i in 0..h {
        let (x, y) = if i < l { (a1[i], b1[i]) } else { (0, 0) };
        sum_a[i] = a0[i] ^ x;
        sum_b[i] = b0[i] ^ y;
    }
    karatsuba(sum_a, sum_b, middle, rest, basecase, kernels);
    // With out = [L0 L1 H0 H1] in h-word quarters (H1 holds 2l - h words,
    // 3h <= 2n for n >= 2) and middle = [M0 M1], adding P1 + P0 + P2 at word h
    // gives L1 += L0 + H0 + M0 and H0 += L1 + H1 + M1, sharing L1 + H0.
    let (low, high) = out.split_at_mut(2 * h);
    let (l0, l1) = low.split_at_mut(h);
    let (h0, h1) = high.split_at_mut(h);
    let (m0, m1) = middle.split_at(h);
    for i in 0..h {
        let shared = l1[i] ^ h0[i];
        let top = h1.get(i).copied().unwrap_or(0);
        l1[i] = shared ^ l0[i] ^ m0[i];
        h0[i] = shared ^ top ^ m1[i];
    }
}

fn xor(destination: &mut [u64], source: &[u64]) {
    for (word, value) in destination.iter_mut().zip(source) {
        *word ^= value;
    }
}

/// The basecase kernels the canonical dispatch runs on this host.
#[derive(Clone, Copy)]
struct Kernels {
    #[cfg(feature = "simd")]
    k4: Option<&'static gf2_kernels_simd::gf2m_wide::ClmulWide256Fns>,
    #[cfg(feature = "simd")]
    k8: Option<&'static gf2_kernels_simd::gf2m_wide::ClmulWide512Fns>,
    #[cfg(feature = "simd")]
    k9: Option<&'static gf2_kernels_simd::gf2m_wide::ClmulWide571Fns>,
    #[cfg(not(feature = "simd"))]
    k4: Option<()>,
    #[cfg(not(feature = "simd"))]
    k9: Option<()>,
}

impl Kernels {
    fn detect() -> Self {
        #[cfg(feature = "simd")]
        if clmul_wide_dispatch_enabled() {
            return Self {
                k4: crate::simd::maybe_gf2m_wide256(),
                k8: crate::simd::maybe_gf2m_wide().map(|fns| &fns.wide512),
                k9: crate::simd::maybe_gf2m_wide571(),
            };
        }
        Self {
            k4: None,
            #[cfg(feature = "simd")]
            k8: None,
            k9: None,
        }
    }

    /// Overwrites the `2m` words of `out` with the product of the `m`-word
    /// `a` and `b`: zero-padded through the narrowest kernel that holds
    /// them, else the word-by-word schoolbook.
    fn product(&self, a: &[u64], b: &[u64], out: &mut [u64]) {
        #[cfg(feature = "simd")]
        {
            let m = a.len();
            // An exact width runs on the caller's words; a narrower operand
            // is zero-padded to the kernel's width.
            macro_rules! basecase {
                ($fns:expr, $width:literal) => {
                    if let (Some(fns), true) = ($fns, m <= $width) {
                        record_clmul_wide_lane(fns.name);
                        if let (Ok(x), Ok(y), Ok(z)) =
                            (a.try_into(), b.try_into(), (&mut *out).try_into())
                        {
                            (fns.clmul)(x, y, z);
                        } else {
                            let (mut x, mut y) = ([0u64; $width], [0u64; $width]);
                            let mut z = [0u64; 2 * $width];
                            x[..m].copy_from_slice(a);
                            y[..m].copy_from_slice(b);
                            (fns.clmul)(&x, &y, &mut z);
                            out.copy_from_slice(&z[..2 * m]);
                        }
                        return;
                    }
                };
            }
            basecase!(self.k4, 4);
            basecase!(self.k8, 8);
            basecase!(self.k9, 9);
        }
        record_clmul_wide_lane(PORTABLE_LANE);
        out.fill(0);
        for (i, &x) in a.iter().enumerate() {
            for (j, &y) in b.iter().enumerate() {
                let product = crate::gf2m::barrett::clmul(x, y);
                out[i + j] ^= product as u64;
                out[i + j + 1] ^= (product >> 64) as u64;
            }
        }
    }
}
