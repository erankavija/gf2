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
    assert_eq!(a.len(), b.len(), "clmul_slice: operand lengths differ");
    assert_eq!(
        out.len(),
        2 * a.len(),
        "clmul_slice: product length must be twice the operand length"
    );
    clmul_into(a, b, out, split, ProductWrite::Accumulate);
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
    sum_a.copy_from_slice(a0);
    sum_b.copy_from_slice(b0);
    xor(&mut sum_a[..l], a1);
    xor(&mut sum_b[..l], b1);
    karatsuba(sum_a, sum_b, middle, rest, basecase, kernels);
    xor(middle, &out[..2 * h]);
    xor(&mut middle[..2 * l], &out[2 * h..]);
    // 3h <= 2n for every n >= 2.
    xor(&mut out[h..3 * h], middle);
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
                k9: crate::simd::maybe_gf2m_wide571(),
            };
        }
        Self { k4: None, k9: None }
    }

    /// Overwrites the `2m` words of `out` with the product of the `m`-word
    /// `a` and `b`: zero-padded through the narrowest kernel that holds
    /// them, else the word-by-word schoolbook.
    fn product(&self, a: &[u64], b: &[u64], out: &mut [u64]) {
        #[cfg(feature = "simd")]
        {
            let m = a.len();
            if let (Some(fns), true) = (self.k4, m <= 4) {
                let (mut x, mut y, mut z) = ([0u64; 4], [0u64; 4], [0u64; 8]);
                x[..m].copy_from_slice(a);
                y[..m].copy_from_slice(b);
                record_clmul_wide_lane(fns.name);
                (fns.clmul)(&x, &y, &mut z);
                out.copy_from_slice(&z[..2 * m]);
                return;
            }
            if let (Some(fns), true) = (self.k9, m <= 9) {
                let (mut x, mut y, mut z) = ([0u64; 9], [0u64; 9], [0u64; 18]);
                x[..m].copy_from_slice(a);
                y[..m].copy_from_slice(b);
                record_clmul_wide_lane(fns.name);
                (fns.clmul)(&x, &y, &mut z);
                out.copy_from_slice(&z[..2 * m]);
                return;
            }
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
