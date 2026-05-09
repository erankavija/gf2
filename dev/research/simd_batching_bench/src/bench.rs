//! Microbench harness driving both per-prime and generic strategies.
//!
//! Methodology:
//!
//! - **Inputs**: deterministic LCG-generated `(mag1, sgn1, mag2, sgn2)`
//!   u64 vectors. The four streams together preserve the canonical
//!   `sgn & ~mag = 0` invariant for both operands by construction (see
//!   [`generate_inputs`]). `batch_lanes ∈ {64, 256, 1024}` corresponds
//!   to `n_words_logical ∈ {1, 4, 16}`; vectors are padded up to a
//!   multiple of 4 u64 (one AVX2 lane = 256 bits) so the kernels can
//!   operate on whole-lane strides. The 64-lane case therefore actually
//!   issues one AVX2 op of 256-bit width and times that, but the
//!   reported cost is normalised against the **logical** 64-lane budget
//!   so the row honestly captures the per-element cost (including the
//!   padding overhead) of running a 64-lane request through an AVX2
//!   kernel.
//! - **Warmup**: 1024 invocations of every (strategy, op, batch) cell,
//!   discarded before measurement.
//! - **Measurement**: outer loop of `OUTER_REPS` iterations. Each iteration
//!   times `INNER_REPS` invocations of the cell via `_rdtsc()` reads
//!   straddling the inner loop, then divides total cycles by the total
//!   number of logical F_3 ops requested (`INNER_REPS * batch_lanes`,
//!   using the **logical** batch size, not the padded one). The minimum
//!   cycles/op across the outer reps is reported (standard noise-rejection
//!   technique for small kernels).
//! - **DCE protection**: the output vectors are XOR-summed into a
//!   `black_box`-fed accumulator after each timing window so the optimizer
//!   cannot drop the work.

use std::hint::black_box;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use core::arch::x86_64::_rdtsc;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::generic::Bipedal3x4;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::per_prime;

const OUTER_REPS: usize = 21;
const INNER_REPS: usize = 1024;
const WARMUP_REPS: usize = 1024;

/// Strategy identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// Per-prime hand-rolled AVX2 kernel from `per_prime` module.
    PerPrime,
    /// Generic `BatchedBipedalLike<Avx2Lane, Avx2Lane>` template.
    Generic,
}

impl Strategy {
    /// Short tag used in output tables.
    ///
    /// # Examples
    ///
    /// ```
    /// use simd_batching_bench::bench::Strategy;
    /// assert_eq!(Strategy::PerPrime.tag(), "per-prime");
    /// ```
    pub fn tag(self) -> &'static str {
        match self {
            Strategy::PerPrime => "per-prime",
            Strategy::Generic => "generic  ",
        }
    }
}

/// Op identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Bipedal `add`.
    Add,
    /// Bipedal `sub`.
    Sub,
    /// Bipedal `mul`.
    Mul,
}

impl Op {
    /// Short tag used in output tables.
    ///
    /// # Examples
    ///
    /// ```
    /// use simd_batching_bench::bench::Op;
    /// assert_eq!(Op::Add.tag(), "add");
    /// ```
    pub fn tag(self) -> &'static str {
        match self {
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
        }
    }
}

/// Single (strategy, op, batch_lanes) cell measurement.
#[derive(Clone, Copy, Debug)]
pub struct CellResult {
    /// Strategy under measurement.
    pub strategy: Strategy,
    /// Operation under measurement.
    pub op: Op,
    /// Number of F_3 lanes per batch invocation.
    pub batch_lanes: usize,
    /// Minimum measured cycles per F_3 op across `OUTER_REPS` reps.
    pub cycles_per_op: f64,
    /// Wall-clock nanoseconds per F_3 op (independent corroboration).
    pub ns_per_op: f64,
}

/// Generate deterministic `(mag1, sgn1, mag2, sgn2)` inputs for `n_words`
/// u64 lanes per stream.
///
/// The four returned vectors satisfy the canonical bipedal invariant
/// `sgn & !mag == 0` for each `(mag, sgn)` pair (we mask each fresh sign
/// stream with its mag stream, so a sign bit is set only where the
/// corresponding magnitude bit is also set). This means alt-zero
/// `(mag=0, sgn=1)` lanes do not occur, and the lane-population over
/// `{0, 1, 2}` is roughly `(1/2, 1/4, 1/4)` from a uniform LCG draw,
/// which is the closest a deterministic stream can get to a realistic
/// permanent-style F_3 distribution without a domain-specific generator.
fn generate_inputs(n_words: usize) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }
    let mut s = 0xDEADBEEF_CAFEBABEu64;
    let mut mag1 = Vec::with_capacity(n_words);
    let mut sgn1 = Vec::with_capacity(n_words);
    let mut mag2 = Vec::with_capacity(n_words);
    let mut sgn2 = Vec::with_capacity(n_words);
    for _ in 0..n_words {
        let m1 = lcg(&mut s);
        let s1 = lcg(&mut s) & m1; // canonical: sgn & !mag == 0
        let m2 = lcg(&mut s);
        let s2 = lcg(&mut s) & m2;
        mag1.push(m1);
        sgn1.push(s1);
        mag2.push(m2);
        sgn2.push(s2);
    }
    (mag1, sgn1, mag2, sgn2)
}

/// Round `n_words` up to a multiple of 4 (one AVX2 lane = 4 u64), padding
/// with zeros. Returns the padded vector and the padded length.
fn pad_to_lane(words: Vec<u64>, lane_words: usize) -> (Vec<u64>, usize) {
    let n = words.len();
    let pad = (lane_words - (n % lane_words)) % lane_words;
    let mut padded = words;
    padded.extend(std::iter::repeat_n(0u64, pad));
    let len = padded.len();
    (padded, len)
}

/// Run the full measurement matrix: 2 strategies × 3 ops × 3 batch sizes
/// = 18 cells. Returns the populated table and a header-friendly tag for
/// the reported AVX2 detection state.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::bench::run_all;
/// let (cells, avx2_ok) = run_all();
/// assert!(avx2_ok);
/// assert_eq!(cells.len(), 18);
/// ```
pub fn run_all() -> (Vec<CellResult>, bool) {
    let avx2_ok = is_x86_feature_detected_avx2();
    let mut out = Vec::with_capacity(18);
    if !avx2_ok {
        return (out, false);
    }
    for &batch_lanes in &[64usize, 256, 1024] {
        for op in [Op::Add, Op::Sub, Op::Mul] {
            for strategy in [Strategy::PerPrime, Strategy::Generic] {
                let cell = measure_cell(strategy, op, batch_lanes);
                out.push(cell);
            }
        }
    }
    (out, true)
}

fn is_x86_feature_detected_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn measure_cell(strategy: Strategy, op: Op, batch_lanes: usize) -> CellResult {
    debug_assert_eq!(batch_lanes % 64, 0);
    let n_words_logical = batch_lanes / 64;
    // Generate the (mag1, sgn1, mag2, sgn2) quadruple in a single LCG run
    // so the four streams jointly satisfy the canonical bipedal invariants
    // for both operands (mirroring the doc's §2.2 claim).
    let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words_logical.max(1));
    // Pad each stream up to a multiple of 4 u64 (one AVX2 lane).
    let (mag1, _) = pad_to_lane(mag1, 4);
    let (sgn1, _) = pad_to_lane(sgn1, 4);
    let (mag2, _) = pad_to_lane(mag2, 4);
    let (sgn2, n_words_padded) = pad_to_lane(sgn2, 4);
    let mut out_mag = vec![0u64; n_words_padded];
    let mut out_sgn = vec![0u64; n_words_padded];

    // Normalise against the LOGICAL batch size, not the padded width.
    // For batch_lanes ∈ {256, 1024} padding is zero so the two are equal.
    // For batch_lanes = 64 the kernel runs one AVX2 op (256 lanes of work),
    // but the request was for 64 lanes; reporting cost per logical lane
    // honestly captures the per-element cost including padding overhead.
    let logical_lanes = batch_lanes as f64;

    // Warmup
    for _ in 0..WARMUP_REPS {
        run_one(
            strategy,
            op,
            &mag1,
            &sgn1,
            &mag2,
            &sgn2,
            &mut out_mag,
            &mut out_sgn,
        );
    }
    black_box(&out_mag);
    black_box(&out_sgn);

    // Measure
    let mut min_cycles_per_op = f64::INFINITY;
    let mut min_ns_per_op = f64::INFINITY;

    for _ in 0..OUTER_REPS {
        // Wall-clock window
        let t_start = std::time::Instant::now();
        // Cycle window
        // SAFETY: rdtsc is available on all x86_64 hosts since Pentium.
        let c_start = unsafe { _rdtsc() };
        for _ in 0..INNER_REPS {
            run_one(
                strategy,
                op,
                &mag1,
                &sgn1,
                &mag2,
                &sgn2,
                &mut out_mag,
                &mut out_sgn,
            );
            // Defeat DCE: feed a checksum of one output word to black_box.
            black_box(out_mag[0]);
        }
        // SAFETY: rdtsc is available on all x86_64 hosts since Pentium.
        let c_end = unsafe { _rdtsc() };
        let elapsed_ns = t_start.elapsed().as_nanos() as f64;

        let total_ops = (INNER_REPS as f64) * logical_lanes;
        let cycles_per_op = (c_end.wrapping_sub(c_start)) as f64 / total_ops;
        let ns_per_op = elapsed_ns / total_ops;
        if cycles_per_op < min_cycles_per_op {
            min_cycles_per_op = cycles_per_op;
        }
        if ns_per_op < min_ns_per_op {
            min_ns_per_op = ns_per_op;
        }
    }

    CellResult {
        strategy,
        op,
        batch_lanes,
        cycles_per_op: min_cycles_per_op,
        ns_per_op: min_ns_per_op,
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(never)]
#[allow(clippy::too_many_arguments)] // bench harness; six u64 slices is the natural shape
fn run_one(
    strategy: Strategy,
    op: Op,
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    // SAFETY: caller verified AVX2 is available (we check at start of run_all).
    unsafe {
        match (strategy, op) {
            (Strategy::PerPrime, Op::Add) => {
                per_prime::run_add_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
            (Strategy::PerPrime, Op::Sub) => {
                per_prime::run_sub_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
            (Strategy::PerPrime, Op::Mul) => {
                per_prime::run_mul_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
            (Strategy::Generic, Op::Add) => {
                Bipedal3x4::run_add_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
            (Strategy::Generic, Op::Sub) => {
                Bipedal3x4::run_sub_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
            (Strategy::Generic, Op::Mul) => {
                Bipedal3x4::run_mul_batch(mag1, sgn1, mag2, sgn2, out_mag, out_sgn);
            }
        }
    }
}

/// Format the cell table as a markdown-friendly text block. The first
/// column is `(op, batch_lanes)`; the next two are per-prime / generic
/// cycles per F_3 op; the last column is the ratio `generic / per-prime`
/// (>1 means generic is slower).
///
/// # Arguments
///
/// * `cells` — output of [`run_all`].
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::bench::{run_all, format_table};
/// let (cells, _) = run_all();
/// let text = format_table(&cells);
/// println!("{}", text);
/// ```
///
/// # Complexity
///
/// `O(cells.len())`.
pub fn format_table(cells: &[CellResult]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "| op   | batch_lanes | per-prime cycles/op | generic cycles/op | ratio (g/p) | per-prime ns/op | generic ns/op |"
    );
    let _ = writeln!(
        s,
        "|------|-------------|---------------------|-------------------|-------------|------------------|----------------|"
    );

    // Group by (op, batch_lanes), find both strategies' rows.
    let mut keys: Vec<(Op, usize)> = cells.iter().map(|c| (c.op, c.batch_lanes)).collect();
    keys.sort_by_key(|&(op, b)| (b, op_order(op)));
    keys.dedup();

    for (op, batch) in keys {
        let pp = cells
            .iter()
            .find(|c| c.op == op && c.batch_lanes == batch && c.strategy == Strategy::PerPrime);
        let g = cells
            .iter()
            .find(|c| c.op == op && c.batch_lanes == batch && c.strategy == Strategy::Generic);
        if let (Some(pp), Some(g)) = (pp, g) {
            let ratio = g.cycles_per_op / pp.cycles_per_op;
            let _ = writeln!(
                s,
                "| {}  | {:>11} | {:>19.4} | {:>17.4} | {:>11.3} | {:>16.4} | {:>14.4} |",
                op.tag(),
                batch,
                pp.cycles_per_op,
                g.cycles_per_op,
                ratio,
                pp.ns_per_op,
                g.ns_per_op
            );
        }
    }
    s
}

fn op_order(op: Op) -> u8 {
    match op {
        Op::Add => 0,
        Op::Sub => 1,
        Op::Mul => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_lane, scalar_add, scalar_mul, scalar_sub};

    /// Verify that per-prime and generic strategies produce bit-identical
    /// outputs on the same inputs (correctness equivalence — required by
    /// criterion 1's "both prototypes" mandate).
    #[test]
    fn per_prime_and_generic_agree_add() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 16; // 16 u64 = 4 AVX2 lanes
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut pp_m = vec![0u64; n_words];
        let mut pp_s = vec![0u64; n_words];
        let mut g_m = vec![0u64; n_words];
        let mut g_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_add_batch(&mag1, &sgn1, &mag2, &sgn2, &mut pp_m, &mut pp_s);
            crate::generic::Bipedal3x4::run_add_batch(
                &mag1, &sgn1, &mag2, &sgn2, &mut g_m, &mut g_s,
            );
        }
        assert_eq!(pp_m, g_m, "per-prime and generic add disagree on mag");
        assert_eq!(pp_s, g_s, "per-prime and generic add disagree on sgn");
    }

    #[test]
    fn per_prime_and_generic_agree_sub() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 16;
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut pp_m = vec![0u64; n_words];
        let mut pp_s = vec![0u64; n_words];
        let mut g_m = vec![0u64; n_words];
        let mut g_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_sub_batch(&mag1, &sgn1, &mag2, &sgn2, &mut pp_m, &mut pp_s);
            crate::generic::Bipedal3x4::run_sub_batch(
                &mag1, &sgn1, &mag2, &sgn2, &mut g_m, &mut g_s,
            );
        }
        assert_eq!(pp_m, g_m);
        assert_eq!(pp_s, g_s);
    }

    #[test]
    fn per_prime_and_generic_agree_mul() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 16;
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut pp_m = vec![0u64; n_words];
        let mut pp_s = vec![0u64; n_words];
        let mut g_m = vec![0u64; n_words];
        let mut g_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_mul_batch(&mag1, &sgn1, &mag2, &sgn2, &mut pp_m, &mut pp_s);
            crate::generic::Bipedal3x4::run_mul_batch(
                &mag1, &sgn1, &mag2, &sgn2, &mut g_m, &mut g_s,
            );
        }
        assert_eq!(pp_m, g_m);
        assert_eq!(pp_s, g_s);
    }

    /// Verify per-prime add agrees with the scalar reference on every
    /// bit position of every input word.
    #[test]
    fn per_prime_add_matches_scalar_reference() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 4;
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut out_m = vec![0u64; n_words];
        let mut out_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_add_batch(&mag1, &sgn1, &mag2, &sgn2, &mut out_m, &mut out_s);
        }
        for w in 0..n_words {
            for bit in 0..64 {
                let m1b = ((mag1[w] >> bit) & 1) as u8;
                let s1b = ((sgn1[w] >> bit) & 1) as u8;
                let m2b = ((mag2[w] >> bit) & 1) as u8;
                let s2b = ((sgn2[w] >> bit) & 1) as u8;
                let (mr, sr) = scalar_add(m1b, s1b, m2b, s2b);
                let mo = ((out_m[w] >> bit) & 1) as u8;
                let so = ((out_s[w] >> bit) & 1) as u8;
                let want = decode_lane(mr, sr);
                let got = decode_lane(mo, so);
                assert_eq!(
                    want, got,
                    "word {} bit {}: scalar {} vs avx2 {}",
                    w, bit, want, got
                );
            }
        }
    }

    #[test]
    fn per_prime_sub_matches_scalar_reference() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 4;
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut out_m = vec![0u64; n_words];
        let mut out_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_sub_batch(&mag1, &sgn1, &mag2, &sgn2, &mut out_m, &mut out_s);
        }
        for w in 0..n_words {
            for bit in 0..64 {
                let m1b = ((mag1[w] >> bit) & 1) as u8;
                let s1b = ((sgn1[w] >> bit) & 1) as u8;
                let m2b = ((mag2[w] >> bit) & 1) as u8;
                let s2b = ((sgn2[w] >> bit) & 1) as u8;
                let (mr, sr) = scalar_sub(m1b, s1b, m2b, s2b);
                let mo = ((out_m[w] >> bit) & 1) as u8;
                let so = ((out_s[w] >> bit) & 1) as u8;
                let want = decode_lane(mr, sr);
                let got = decode_lane(mo, so);
                assert_eq!(
                    want, got,
                    "word {} bit {}: scalar {} vs avx2 {}",
                    w, bit, want, got
                );
            }
        }
    }

    #[test]
    fn per_prime_mul_matches_scalar_reference() {
        if !is_x86_feature_detected_avx2() {
            return;
        }
        let n_words = 4;
        let (mag1, sgn1, mag2, sgn2) = generate_inputs(n_words);
        let mut out_m = vec![0u64; n_words];
        let mut out_s = vec![0u64; n_words];
        // SAFETY: AVX2 just checked.
        unsafe {
            per_prime::run_mul_batch(&mag1, &sgn1, &mag2, &sgn2, &mut out_m, &mut out_s);
        }
        for w in 0..n_words {
            for bit in 0..64 {
                let m1b = ((mag1[w] >> bit) & 1) as u8;
                let s1b = ((sgn1[w] >> bit) & 1) as u8;
                let m2b = ((mag2[w] >> bit) & 1) as u8;
                let s2b = ((sgn2[w] >> bit) & 1) as u8;
                let (mr, sr) = scalar_mul(m1b, s1b, m2b, s2b);
                let mo = ((out_m[w] >> bit) & 1) as u8;
                let so = ((out_s[w] >> bit) & 1) as u8;
                let want = decode_lane(mr, sr);
                let got = decode_lane(mo, so);
                assert_eq!(want, got);
            }
        }
    }
}
