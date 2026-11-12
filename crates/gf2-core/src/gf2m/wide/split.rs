//! The long carry-less product: Karatsuba down to the widest kernel the
//! canonical dispatch runs.

use std::cell::RefCell;
use std::num::NonZeroUsize;

#[cfg(feature = "simd")]
use super::clmul_wide_dispatch_enabled;
use super::{record_clmul_wide_lane, ProductWrite, PORTABLE_LANE};

/// Bounds of the recursive split in [`clmul_slice_with`]: operands of at most
/// `basecase_words` words multiply as one schoolbook product, through a
/// kernel when one is that wide. A longer operand either halves or splits at
/// the smallest `leaf_words · 2^j` at least half its length, so the two
/// full-width subproducts recurse onto whole leaves, whichever needs fewer
/// basecase word products: a basecase of `m` words costs the square of the
/// narrowest kernel width that holds it, or `m^2` without one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClmulSplit {
    basecase_words: NonZeroUsize,
    leaf_words: NonZeroUsize,
}

impl ClmulSplit {
    /// The split with basecase width `basecase_words`, aligned to
    /// `leaf_words`.
    #[must_use]
    pub const fn new(basecase_words: NonZeroUsize, leaf_words: NonZeroUsize) -> Self {
        Self {
            basecase_words,
            leaf_words,
        }
    }

    /// The width of the widest kernel the canonical dispatch runs on this
    /// host as the basecase (9 words, else 4, else one word on the portable
    /// lane, where the split reduces to single-word products), aligned to the
    /// widest power-of-two kernel width (8, else 4, else 1).
    ///
    #[must_use]
    pub fn structural() -> Self {
        let kernels = Kernels::detect();
        let basecase = match kernels {
            Kernels { k9: Some(_), .. } => 9,
            Kernels { k4: Some(_), .. } => 4,
            _ => 1,
        };
        let leaf = kernels.leaf_words();
        Self::new(
            NonZeroUsize::new(basecase).expect("a positive width"),
            NonZeroUsize::new(leaf).expect("a positive width"),
        )
    }

    /// Returns the basecase width in words.
    #[must_use]
    pub const fn basecase_words(self) -> usize {
        self.basecase_words.get()
    }

    /// Returns the width split points align to, in words.
    #[must_use]
    pub const fn leaf_words(self) -> usize {
        self.leaf_words.get()
    }
}

/// The basecase, alignment and kernels one product splits by.
struct Plan {
    basecase: usize,
    leaf: usize,
    kernels: Kernels,
}

impl Plan {
    /// The aligned split point of an `n`-word operand, `n > leaf`: the
    /// smallest `leaf · 2^j` at least half of `n`, which is below `n`.
    fn aligned_point(&self, n: usize) -> usize {
        let mut h = self.leaf;
        while 2 * h < n {
            h *= 2;
        }
        h
    }

    /// Word products of one basecase product of `m` words.
    fn basecase_cost(&self, m: usize) -> u64 {
        let width = self.kernels.width_for(m).unwrap_or(m) as u64;
        width * width
    }

    /// Basecase word products of halving `n` words down to the basecase.
    fn halving_cost(&self, n: usize) -> u64 {
        let (mut m, mut leaves) = (n, 1u64);
        while m > self.basecase {
            m = m.div_ceil(2);
            leaves *= 3;
        }
        leaves * self.basecase_cost(m)
    }

    /// Basecase word products of the cheaper split of `n` words.
    fn cost(&self, n: usize) -> u64 {
        if n <= self.basecase {
            return self.basecase_cost(n);
        }
        let halving = self.halving_cost(n);
        if n <= self.leaf {
            return halving;
        }
        let h = self.aligned_point(n);
        halving.min(2 * self.halving_cost(h) + self.cost(n - h))
    }

    /// The split point of `n > basecase` words, and whether the parts halve
    /// from there on: halving whenever the parts of an aligned split cost
    /// more, the aligned point otherwise.
    fn split(&self, n: usize, halve: bool) -> (usize, bool) {
        let half = n.div_ceil(2);
        if halve || n <= self.leaf {
            return (half, true);
        }
        let h = self.aligned_point(n);
        if h == half || self.halving_cost(n) <= 2 * self.halving_cost(h) + self.cost(n - h) {
            (half, true)
        } else {
            (h, false)
        }
    }

    /// An upper bound on the scratch words [`karatsuba`] uses for `n`-word
    /// operands. A node splitting at `h` takes `4h` words and hands the rest
    /// to its children, which run one at a time. A halving chain from `n`
    /// takes `4 Σ ⌈n / 2^k⌉ < 4(n + log2 n) <= 8n`. An aligned point
    /// `h` is `leaf · 2^j`, whose part halves exactly within `4h`, beside a
    /// remainder within `8(n - h)` by induction, so the node takes at most
    /// `max(8h, 8n - 4h) <= 8n`.
    fn scratch_words(&self, n: usize) -> usize {
        if n <= self.basecase {
            0
        } else {
            8 * n
        }
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
/// `O(n^log2(3))` basecase products and word XORs, with at most `10n` words
/// of scratch (`8n` for [`clmul_slice_overwrite`]) held per thread across
/// calls.
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
    let plan = Plan {
        basecase: split.basecase_words(),
        leaf: split.leaf_words(),
        kernels: Kernels::detect(),
    };
    let product_words = if write == ProductWrite::Accumulate {
        2 * n
    } else {
        0
    };
    let mut run = |scratch: &mut Vec<u64>| {
        let need = product_words + plan.scratch_words(n);
        if scratch.len() < need {
            scratch.resize(need, 0);
        }
        let (product, rest) = scratch.split_at_mut(product_words);
        match write {
            ProductWrite::Overwrite => karatsuba(a, b, out, rest, &plan, false),
            ProductWrite::Accumulate => {
                karatsuba(a, b, product, rest, &plan, false);
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

/// Overwrites the `2n` words of `out` with the product of the `n`-word `a`
/// and `b`: with `h` the split point of [`Plan::split`] and
/// `a = a0 + x^(64h) a1`, `b` alike, the
/// product is `P0 + x^(64h) (P0 + P1 + P2) + x^(128h) P2` over `P0 = a0 b0`,
/// `P2 = a1 b1` and `P1 = (a0 + a1)(b0 + b1)`.
fn karatsuba(a: &[u64], b: &[u64], out: &mut [u64], scratch: &mut [u64], plan: &Plan, halve: bool) {
    let n = a.len();
    let (basecase, kernels) = (plan.basecase, &plan.kernels);
    if n <= basecase {
        kernels.product(a, b, out);
        return;
    }
    // Most nodes of a power-of-two product sit right above its 8-word
    // leaves; they run on fixed-size arrays.
    #[cfg(feature = "simd")]
    if let (16, true, Some(fns)) = (n, basecase >= 8, kernels.k8) {
        karatsuba_16(fns, a, b, out);
        return;
    }
    let (h, halve) = plan.split(n, halve);
    let l = n - h;
    let (a0, a1) = a.split_at(h);
    let (b0, b1) = b.split_at(h);
    let (sum_a, rest) = scratch.split_at_mut(h);
    let (sum_b, rest) = rest.split_at_mut(h);
    let (middle, rest) = rest.split_at_mut(2 * h);
    {
        let (low, high) = out.split_at_mut(2 * h);
        karatsuba(a0, b0, low, rest, plan, halve);
        karatsuba(a1, b1, high, rest, plan, halve);
    }
    // The shared run of the two parts vectorizes.
    for (((sa, sb), (x0, y0)), (x1, y1)) in sum_a
        .iter_mut()
        .zip(sum_b.iter_mut())
        .zip(a0.iter().zip(b0))
        .zip(a1.iter().zip(b1))
    {
        *sa = x0 ^ x1;
        *sb = y0 ^ y1;
    }
    sum_a[l..].copy_from_slice(&a0[l..]);
    sum_b[l..].copy_from_slice(&b0[l..]);
    karatsuba(sum_a, sum_b, middle, rest, plan, halve);
    if 2 * l < h {
        // P0 + P1 + P2 has degree below 64(h + l), so it ends inside out.
        xor(middle, &out[..2 * h]);
        xor(&mut middle[..2 * l], &out[2 * h..]);
        let end = out.len();
        xor(&mut out[h..end], &middle[..end - h]);
        return;
    }
    // With out = [L0 L1 H0 H1] in h-word quarters (H1 holds 2l - h words)
    // and middle = [M0 M1], adding P1 + P0 + P2 at word h gives
    // L1 += L0 + H0 + M0 and H0 += L1 + H1 + M1, sharing L1 + H0.
    let (low, high) = out.split_at_mut(2 * h);
    let (l0, l1) = low.split_at_mut(h);
    let (h0, h1) = high.split_at_mut(h);
    let (m0, m1) = middle.split_at(h);
    let top = h1.len();
    for i in 0..h {
        let shared = l1[i] ^ h0[i];
        l1[i] = shared ^ l0[i] ^ m0[i];
        h0[i] = shared ^ m1[i];
    }
    for (word, value) in h0[..top].iter_mut().zip(h1.iter()) {
        *word ^= value;
    }
}

/// [`karatsuba`] of 16-word operands over three exact 8-word kernel calls.
#[cfg(feature = "simd")]
fn karatsuba_16(
    fns: &gf2_kernels_simd::gf2m_wide::ClmulWide512Fns,
    a: &[u64],
    b: &[u64],
    out: &mut [u64],
) {
    let half = |words: &[u64], at: usize| -> [u64; 8] {
        words[at..at + 8].try_into().expect("16-word operands")
    };
    let (a0, a1, b0, b1) = (half(a, 0), half(a, 8), half(b, 0), half(b, 8));
    let (low, high) = out.split_at_mut(16);
    let low: &mut [u64; 16] = low.try_into().expect("a 32-word product");
    let high: &mut [u64; 16] = high.try_into().expect("a 32-word product");
    record_clmul_wide_lane(fns.name);
    (fns.clmul)(&a0, &b0, low);
    (fns.clmul)(&a1, &b1, high);
    let sum_a: [u64; 8] = std::array::from_fn(|i| a0[i] ^ a1[i]);
    let sum_b: [u64; 8] = std::array::from_fn(|i| b0[i] ^ b1[i]);
    let mut middle = [0u64; 16];
    (fns.clmul)(&sum_a, &sum_b, &mut middle);
    for i in 0..8 {
        let shared = low[8 + i] ^ high[i];
        low[8 + i] = shared ^ low[i] ^ middle[i];
        high[i] = shared ^ high[8 + i] ^ middle[8 + i];
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
    /// The width of the narrowest kernel that holds `m` words, if any.
    fn width_for(&self, m: usize) -> Option<usize> {
        #[cfg(feature = "simd")]
        {
            if self.k4.is_some() && m <= 4 {
                return Some(4);
            }
            if self.k8.is_some() && m <= 8 {
                return Some(8);
            }
            if self.k9.is_some() && m <= 9 {
                return Some(9);
            }
        }
        let _ = m;
        None
    }

    /// The widest power-of-two kernel width: 8 words, else 4, else 1.
    fn leaf_words(&self) -> usize {
        #[cfg(feature = "simd")]
        if self.k8.is_some() {
            return 8;
        }
        if self.k4.is_some() {
            4
        } else {
            1
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs every split on exactly [`Plan::scratch_words`] words, so a
    /// recursion that needs more panics on its `split_at_mut`.
    #[test]
    fn karatsuba_fits_the_scratch_bound() {
        let kernels = Kernels::detect();
        let reference = Plan {
            basecase: 1,
            leaf: 1,
            kernels,
        };
        for (basecase, leaf) in [
            (1, 1),
            (1, 8),
            (2, 1),
            (3, 2),
            (4, 4),
            (9, 8),
            (9, 1),
            (5, 64),
        ] {
            let plan = Plan {
                basecase,
                leaf,
                kernels,
            };
            // Each `leaf · 2^j + 1` is an aligned split's widest point.
            let lengths = (1..=160).chain((0..7).map(|j| (leaf << j) + 1));
            for n in lengths {
                let a: Vec<u64> = (0..n as u64)
                    .map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15))
                    .collect();
                let b: Vec<u64> = a.iter().map(|x| x.rotate_left(17) ^ 0xA5A5).collect();
                let mut scratch = vec![0; plan.scratch_words(n)];
                let mut got = vec![0; 2 * n];
                karatsuba(&a, &b, &mut got, &mut scratch, &plan, false);
                let mut want = vec![0; 2 * n];
                let mut wide = vec![0; reference.scratch_words(n)];
                karatsuba(&a, &b, &mut want, &mut wide, &reference, false);
                assert_eq!(got, want, "basecase {basecase}, leaf {leaf}, n {n}");
            }
        }
    }
}
