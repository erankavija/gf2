//! Flooding updates over QC lifted positions, stored at `edge * z + position`.
//! The min-sum rules and edge order are those of [`crate::ldpc_lanes`].

#![forbid(unsafe_code)]

use crate::ldpc_lanes::LaneMinSumRule;
use std::ops::Range;

type CheckBlockFn = fn(LaneMinSumRule, &[f32], &mut [f32], usize, usize);
type RotatedDifferenceFn = fn(&[f32], usize, &[f32], &mut [f32]);
type AccumulateFn = fn(&QcAccumulation, &mut [f32], &[f32], &[f32]);
type CheckRowRotatedFn = fn(LaneMinSumRule, &[f32], &[QcBlock], &mut [f32], &mut [f32], usize);

/// Safe QC kernels, with a portable implementation on every target.
#[derive(Clone, Copy)]
pub struct QcKernels {
    pub(crate) check_block_fn: CheckBlockFn,
    pub(crate) rotated_difference_fn: RotatedDifferenceFn,
    pub(crate) accumulate_fn: AccumulateFn,
    pub(crate) check_row_rotated_fn: CheckRowRotatedFn,
}

/// One circulant's check-to-variable messages in a variable accumulation:
/// `z` messages from `offset` of the message plane, rotated by `shift`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QcTerm {
    /// Index of the circulant's first message in the message plane.
    pub offset: usize,
    /// Lane `j` reads message `(j - shift) mod z`.
    pub shift: usize,
}

/// Variable lanes `lanes` of a column block that add the terms `terms` of
/// their [`QcAccumulation`] in that order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QcSegment {
    /// Lanes of the column block, within `0..z`.
    pub lanes: Range<usize>,
    /// Indices into the plan's terms.
    pub terms: Range<usize>,
}

/// The variable update of a QC code, validated once: per column block, lane
/// segments with their terms over a message plane of a fixed length.
///
/// Construction splits each segment at every term's wrap lane into runs in
/// which each term reads one contiguous slice of its messages, so the
/// kernels add contiguous slices in each lane's term order. Lanes no
/// segment names keep their beliefs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QcAccumulation {
    pub(crate) z: usize,
    pub(crate) messages: usize,
    /// Column block `b` owns `runs[run_offsets[b]..run_offsets[b + 1]]`.
    pub(crate) run_offsets: Vec<usize>,
    pub(crate) runs: Vec<QcRun>,
    /// Per run and term, the message index its first lane reads.
    pub(crate) bases: Vec<usize>,
}

/// Lanes `lanes` of a column block: lane `lanes.start + k` adds
/// `messages[base + k]` for each base of `bases`, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QcRun {
    pub(crate) lanes: Range<usize>,
    pub(crate) bases: Range<usize>,
}

impl QcAccumulation {
    /// Builds the plan of lifting size `z` over a plane of `messages`
    /// check-to-variable messages. Takes O(blocks + Σ t² log t) time and
    /// O(Σ t²) space over the segments, `t` a segment's term count.
    ///
    /// # Panics
    ///
    /// Panics unless `z > 0`; `segment_offsets` starts at zero, never
    /// decreases and ends at `segments.len()`; every segment's lanes lie
    /// within `0..z` and its terms within `terms`; and every term has
    /// `shift < z` and `offset + z <= messages`.
    #[must_use]
    pub fn new(
        z: usize,
        messages: usize,
        segment_offsets: Vec<usize>,
        segments: Vec<QcSegment>,
        terms: Vec<QcTerm>,
    ) -> Self {
        assert!(z > 0, "QC lifting size zero");
        assert_eq!(
            segment_offsets.first(),
            Some(&0),
            "QC segments start at zero"
        );
        assert_eq!(
            segment_offsets.last(),
            Some(&segments.len()),
            "QC segment offsets end at the segment count"
        );
        assert!(
            segment_offsets.windows(2).all(|pair| pair[0] <= pair[1]),
            "QC segment offsets decrease"
        );
        for segment in &segments {
            let (lanes, range) = (&segment.lanes, &segment.terms);
            assert!(
                lanes.start <= lanes.end && lanes.end <= z,
                "QC lanes {lanes:?} outside lifting size {z}"
            );
            assert!(
                range.start <= range.end && range.end <= terms.len(),
                "QC terms {range:?} outside {} terms",
                terms.len()
            );
        }
        for term in &terms {
            assert!(
                term.shift < z,
                "QC shift {} outside lifting size {z}",
                term.shift
            );
            assert!(
                term.offset
                    .checked_add(z)
                    .is_some_and(|end| end <= messages),
                "QC messages from {} outside a plane of {messages}",
                term.offset
            );
        }
        let mut run_offsets = vec![0];
        let mut runs = Vec::new();
        let mut bases = Vec::new();
        let mut splits = Vec::new();
        for block in segment_offsets.windows(2) {
            for segment in &segments[block[0]..block[1]] {
                let terms = &terms[segment.terms.clone()];
                let (start, end) = (segment.lanes.start, segment.lanes.end);
                // Lane j reads message (j - shift) mod z, which wraps from
                // z - 1 to 0 between lanes shift - 1 and shift.
                splits.clear();
                splits.push(start);
                splits.extend(
                    terms
                        .iter()
                        .map(|term| term.shift)
                        .filter(|&shift| start < shift && shift < end),
                );
                splits.push(end);
                splits.sort_unstable();
                splits.dedup();
                for pair in splits.windows(2) {
                    let first = bases.len();
                    bases.extend(
                        terms
                            .iter()
                            .map(|term| term.offset + (pair[0] + z - term.shift) % z),
                    );
                    runs.push(QcRun {
                        lanes: pair[0]..pair[1],
                        bases: first..bases.len(),
                    });
                }
            }
            run_offsets.push(runs.len());
        }
        Self {
            z,
            messages,
            run_offsets,
            runs,
            bases,
        }
    }

    /// Returns the number of column blocks.
    #[must_use]
    pub fn col_blocks(&self) -> usize {
        self.run_offsets.len() - 1
    }

    /// The runs of column block `block`, each with its message bases.
    pub(crate) fn block_runs(
        &self,
        block: usize,
    ) -> impl Iterator<Item = (Range<usize>, &[usize])> {
        self.runs[self.run_offsets[block]..self.run_offsets[block + 1]]
            .iter()
            .map(|run| (run.lanes.clone(), &self.bases[run.bases.clone()]))
    }
}

/// One circulant's column block in a check update: `z` beliefs from `offset`,
/// rotated by `shift`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QcBlock {
    /// Index of the column block's first belief.
    pub offset: usize,
    /// Lifted check `i` reads belief `(i + shift) mod z`.
    pub shift: usize,
}

impl QcKernels {
    /// Scalar kernels; the check kernel preserves each check's edge order.
    #[must_use]
    pub const fn portable() -> Self {
        Self {
            check_block_fn: |rule, inputs, outputs, degree, z| {
                check_block_scalar(rule, inputs, outputs, degree, z, 0);
            },
            rotated_difference_fn: rotated_difference_runs,
            accumulate_fn: accumulate_runs,
            check_row_rotated_fn: check_row_rotated_portable,
        }
    }

    /// Writes one base row's messages, with z lifted positions per edge.
    ///
    /// Input and output index `edge * z + i` belongs to lifted check i.
    /// Degree zero writes nothing; degree one writes positive zeros.
    /// Takes O(degree * z) time without allocation. Parameter domains are
    /// those of [`LaneMinSumRule`].
    ///
    /// # Panics
    ///
    /// Panics before writing unless both lengths equal `degree * z`, or
    /// when that product overflows `usize`.
    pub fn check_block(
        &self,
        rule: LaneMinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
    ) {
        let length = degree.checked_mul(z).expect("QC message count overflow");
        assert_eq!(inputs.len(), length, "QC input length");
        assert_eq!(outputs.len(), length, "QC output length");
        (self.check_block_fn)(rule, inputs, outputs, degree, z);
    }

    /// Writes `out[i] = block[(i + shift) mod z] - subtrahend[i]` for every
    /// lane `i`, with `z = block.len()`: the variable-to-check messages of a
    /// circulant from the beliefs of its column block and its check-to-variable
    /// messages. Each lane is one `f32` subtraction.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `subtrahend` and `out` hold `z` values
    /// and `shift < z`.
    pub fn rotated_difference(
        &self,
        block: &[f32],
        shift: usize,
        subtrahend: &[f32],
        out: &mut [f32],
    ) {
        let z = block.len();
        assert!(shift < z, "QC shift {shift} outside lifting size {z}");
        assert_eq!(subtrahend.len(), z, "QC subtrahend length");
        assert_eq!(out.len(), z, "QC difference length");
        (self.rotated_difference_fn)(block, shift, subtrahend, out);
    }

    /// Writes, for every segment of column block `b` and lane `j` of its
    /// lanes, `beliefs[b * z + j] = channel[b * z + j] + Σ_t
    /// messages[t.offset + (j - t.shift) mod z]`, adding the segment's terms
    /// in their order: the beliefs of every variable from its channel value
    /// and the check-to-variable messages of its circulants, in its canonical
    /// accumulation order. Lanes no segment names are unchanged. Takes
    /// O(Σ |lanes| · |terms|) time over the segments.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `beliefs` and `channel` hold
    /// `plan.col_blocks() * z` values and `messages` the plan's message count.
    pub fn accumulate(
        &self,
        plan: &QcAccumulation,
        beliefs: &mut [f32],
        channel: &[f32],
        messages: &[f32],
    ) {
        let lanes = plan.col_blocks() * plan.z;
        assert_eq!(beliefs.len(), lanes, "QC belief length");
        assert_eq!(channel.len(), lanes, "QC channel length");
        assert_eq!(messages.len(), plan.messages, "QC message length");
        (self.accumulate_fn)(plan, beliefs, channel, messages);
    }

    /// Updates one base row's check-to-variable messages in place from the
    /// beliefs: the result of [`Self::rotated_difference`] of each edge's
    /// column block `beliefs[edge.offset..edge.offset + z]`, rotated by
    /// `edge.shift`, minus its messages `messages[e * z..(e + 1) * z]`,
    /// followed by [`Self::check_block`] over the `edges.len()` edges, bit
    /// for bit. `scratch` is workspace whose contents are unspecified
    /// afterwards. Takes O(degree * z) time without allocation.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `z > 0`, every edge has `shift < z` and
    /// `offset + z <= beliefs.len()`, `messages` holds `degree * z` values
    /// and `scratch` at least as many.
    pub fn check_row_rotated(
        &self,
        rule: LaneMinSumRule,
        beliefs: &[f32],
        edges: &[QcBlock],
        messages: &mut [f32],
        scratch: &mut [f32],
        z: usize,
    ) {
        assert!(z > 0, "QC lifting size zero");
        for edge in edges {
            assert!(
                edge.shift < z,
                "QC shift {} outside lifting size {z}",
                edge.shift
            );
            assert!(
                edge.offset
                    .checked_add(z)
                    .is_some_and(|end| end <= beliefs.len()),
                "QC column block from {} outside {} beliefs",
                edge.offset,
                beliefs.len()
            );
        }
        let length = edges
            .len()
            .checked_mul(z)
            .expect("QC message count overflow");
        assert_eq!(messages.len(), length, "QC message length");
        assert!(scratch.len() >= length, "QC scratch length");
        (self.check_row_rotated_fn)(rule, beliefs, edges, messages, scratch, z);
    }
}

/// AVX2 kernels when supported by the host; `None` on other targets or hosts.
#[must_use]
pub fn detect() -> Option<QcKernels> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_qc::detect();
    }
    #[allow(unreachable_code)]
    None
}

/// [`QcKernels::check_row_rotated`] through the scratch: the rotated
/// differences, then the check kernel.
fn check_row_rotated_portable(
    rule: LaneMinSumRule,
    beliefs: &[f32],
    edges: &[QcBlock],
    messages: &mut [f32],
    scratch: &mut [f32],
    z: usize,
) {
    let degree = edges.len();
    let scratch = &mut scratch[..degree * z];
    for ((edge, out), messages) in edges
        .iter()
        .zip(scratch.chunks_exact_mut(z))
        .zip(messages.chunks_exact(z))
    {
        rotated_difference_runs(
            &beliefs[edge.offset..edge.offset + z],
            edge.shift,
            messages,
            out,
        );
    }
    check_block_scalar(rule, scratch, messages, degree, z, 0);
}

/// [`QcKernels::rotated_difference`] as two contiguous runs.
fn rotated_difference_runs(block: &[f32], shift: usize, subtrahend: &[f32], out: &mut [f32]) {
    let split = block.len() - shift;
    let (head_out, tail_out) = out.split_at_mut(split);
    let (head_sub, tail_sub) = subtrahend.split_at(split);
    for ((slot, &value), &sub) in head_out.iter_mut().zip(&block[shift..]).zip(head_sub) {
        *slot = value - sub;
    }
    for ((slot, &value), &sub) in tail_out.iter_mut().zip(&block[..shift]).zip(tail_sub) {
        *slot = value - sub;
    }
}

/// [`QcKernels::accumulate`] one run at a time, one term at a time.
fn accumulate_runs(plan: &QcAccumulation, beliefs: &mut [f32], channel: &[f32], messages: &[f32]) {
    let z = plan.z;
    for (block, (beliefs, channel)) in beliefs
        .chunks_exact_mut(z)
        .zip(channel.chunks_exact(z))
        .enumerate()
    {
        for (lanes, bases) in plan.block_runs(block) {
            let len = lanes.len();
            let beliefs = &mut beliefs[lanes.clone()];
            beliefs.copy_from_slice(&channel[lanes]);
            for &base in bases {
                for (slot, &value) in beliefs.iter_mut().zip(&messages[base..base + len]) {
                    *slot += value;
                }
            }
        }
    }
}

pub(crate) fn check_block_scalar(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
    start: usize,
) {
    if degree == 0 || z == 0 {
        return;
    }
    if degree == 1 {
        outputs[..z].fill(0.0);
        return;
    }
    for lane in start..z {
        let mut min1 = f32::INFINITY;
        let mut min2 = f32::INFINITY;
        let mut arg1 = usize::MAX;
        let mut sign_product = 1.0f32;
        for edge in 0..degree {
            let value = inputs[edge * z + lane];
            sign_product = if value >= 0.0 {
                sign_product
            } else {
                -sign_product
            };
            let magnitude = value.abs();
            if magnitude < min1 {
                min2 = min1;
                min1 = magnitude;
                arg1 = edge;
            } else if magnitude < min2 {
                min2 = magnitude;
            }
        }
        for edge in 0..degree {
            let value = inputs[edge * z + lane];
            let sign = if value >= 0.0 {
                sign_product
            } else {
                -sign_product
            };
            let magnitude = if edge == arg1 { min2 } else { min1 };
            outputs[edge * z + lane] = rule.apply(sign, magnitude);
        }
    }
}
