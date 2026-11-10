//! Intra-frame flooding over QC blocks. Check lanes are lifted positions;
//! each column block splits into lane segments that share one canonical
//! accumulation order, so variable sums equal the canonical decoder's.

use super::{DecodeOutcome, DecoderConfig, LaneBackend, LdpcCode, MinSumRule, QuasiCyclicLdpc};
use crate::llr::Llr;
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_qc::{QcKernels, QcTerm};
use std::ops::Range;

/// One circulant of the block description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaseEdge {
    col_block: usize,
    shift: usize,
}

/// Variable lanes `lanes` of a column block accumulate the circulants
/// `seg_terms[edges]` in that order; the segments of a block cover its lanes.
struct Segment {
    lanes: Range<usize>,
    edges: Range<usize>,
}

/// The block structure of a quasi-cyclic code, in both traversal orders.
///
/// `check_edges` is ordered by row block, so a row block owns one contiguous
/// run, and `masked` by edge, so it owns `masked[masked_offsets[r]..]` up to
/// the next offset. A masked (edge, check lane) pair carries a +∞
/// variable-to-check message, which the check rule skips as it skips an
/// absent edge.
struct QcLayout {
    z: usize,
    row_blocks: usize,
    col_blocks: usize,
    check_edges: Vec<BaseEdge>,
    check_offsets: Vec<usize>,
    segments: Vec<Segment>,
    segment_offsets: Vec<usize>,
    seg_terms: Vec<QcTerm>,
    /// (edge, check lane) pairs without an entry, ascending.
    masked: Vec<(usize, usize)>,
    masked_offsets: Vec<usize>,
    max_row_degree: usize,
    /// Variable of each lifted column and lifted column of each variable;
    /// `None` is the identity.
    col_order: Option<(Vec<usize>, Vec<usize>)>,
}

impl QcLayout {
    /// `None` when a lifted check with a masked entry keeps exactly one edge,
    /// whose canonical output is the zero LLR rather than the masked rule's.
    fn new(code: &QuasiCyclicLdpc) -> Option<Self> {
        let z = code.expansion_factor();
        let row_blocks = code.base_rows();
        let col_blocks = code.base_cols();

        let mut check_edges = Vec::new();
        let mut edge_rows = Vec::new();
        let mut masked = Vec::new();
        let mut check_offsets = vec![0usize; row_blocks + 1];
        for circulant in code.circulants() {
            let index = check_edges.len();
            check_edges.push(BaseEdge {
                col_block: circulant.col_block(),
                shift: circulant.shift(),
            });
            edge_rows.push(circulant.row_block());
            masked.extend(circulant.masked_rows().iter().map(|&lane| (index, lane)));
            check_offsets[circulant.row_block() + 1] += 1;
        }
        for row_block in 0..row_blocks {
            check_offsets[row_block + 1] += check_offsets[row_block];
        }
        for row_block in 0..row_blocks {
            let edges = check_offsets[row_block]..check_offsets[row_block + 1];
            let mut lanes: Vec<usize> = masked
                .iter()
                .filter(|(edge, _)| edges.contains(edge))
                .map(|&(_, lane)| lane)
                .collect();
            lanes.sort_unstable();
            lanes.dedup();
            for lane in lanes {
                let absent = masked
                    .iter()
                    .filter(|&&(edge, l)| l == lane && edges.contains(&edge))
                    .count();
                if edges.len() - absent == 1 {
                    return None;
                }
            }
        }

        let mut var_offsets = vec![0usize; col_blocks + 1];
        for edge in &check_edges {
            var_offsets[edge.col_block + 1] += 1;
        }
        for col_block in 0..col_blocks {
            var_offsets[col_block + 1] += var_offsets[col_block];
        }
        let mut cursor = var_offsets.clone();
        let mut var_edges = vec![0usize; check_edges.len()];
        for (index, edge) in check_edges.iter().enumerate() {
            var_edges[cursor[edge.col_block]] = index;
            cursor[edge.col_block] += 1;
        }

        // A lane's canonical accumulation order is its edges by ascending
        // parity check; adjacent lanes with one order share a segment.
        let check_of = |row: usize| code.row_order().map_or(row, |order| order[row]);
        let mut segments: Vec<Segment> = Vec::new();
        let mut segment_offsets = vec![0usize];
        let mut seg_edges = Vec::new();
        let mut order: Vec<(usize, usize)> = Vec::new();
        for col_block in 0..col_blocks {
            let run = &var_edges[var_offsets[col_block]..var_offsets[col_block + 1]];
            let first = segments.len();
            for lane in 0..z {
                order.clear();
                for &index in run {
                    let edge = check_edges[index];
                    let check_lane = (lane + z - edge.shift) % z;
                    if masked.binary_search(&(index, check_lane)).is_err() {
                        order.push((check_of(edge_rows[index] * z + check_lane), index));
                    }
                }
                order.sort_unstable();
                if let Some(last) = segments[first..].last_mut() {
                    let previous = &seg_edges[last.edges.clone()];
                    if previous.len() == order.len()
                        && previous.iter().zip(&order).all(|(&a, &(_, b))| a == b)
                    {
                        last.lanes.end = lane + 1;
                        continue;
                    }
                }
                let start = seg_edges.len();
                seg_edges.extend(order.iter().map(|&(_, index)| index));
                segments.push(Segment {
                    lanes: lane..lane + 1,
                    edges: start..seg_edges.len(),
                });
            }
            segment_offsets.push(segments.len());
        }

        let col_order = code.col_order().map(|order| {
            let mut inverse = vec![0usize; order.len()];
            for (column, &variable) in order.iter().enumerate() {
                inverse[variable] = column;
            }
            (order.to_vec(), inverse)
        });

        let masked_offsets = check_offsets
            .iter()
            .map(|&edge| masked.partition_point(|&(index, _)| index < edge))
            .collect();
        let seg_terms = seg_edges
            .iter()
            .map(|&index| QcTerm {
                offset: index * z,
                shift: check_edges[index].shift,
            })
            .collect();
        let max_row_degree = check_offsets
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .max()
            .unwrap_or(0);

        Some(Self {
            z,
            row_blocks,
            col_blocks,
            check_edges,
            check_offsets,
            segments,
            segment_offsets,
            seg_terms,
            masked,
            masked_offsets,
            max_row_degree,
            col_order,
        })
    }

    fn z(&self) -> usize {
        self.z
    }

    fn n(&self) -> usize {
        self.col_blocks * self.z
    }

    fn base_edges(&self) -> usize {
        self.check_edges.len()
    }

    fn row_edges(&self, row_block: usize) -> Range<usize> {
        self.check_offsets[row_block]..self.check_offsets[row_block + 1]
    }

    fn row_masked(&self, row_block: usize) -> &[(usize, usize)] {
        &self.masked[self.masked_offsets[row_block]..self.masked_offsets[row_block + 1]]
    }
}

/// Flooding min-sum decoder vectorized across one frame's lifted positions.
///
/// The workspace holds one belief per lifted column and one check-to-variable
/// message per lifted edge. A check row reads the variable-to-check messages
/// `belief - message` of the previous iteration into a row scratch, so the
/// messages are updated in place, and the variable step then rebuilds every
/// belief from the channel value and the new messages.
pub struct QcDecoder {
    layout: QcLayout,
    rule: MinSumRule,
    config: DecoderConfig,
    /// Channel values and beliefs by lifted column.
    channel: Vec<f32>,
    belief: Vec<f32>,
    check_to_var: Vec<f32>,
    /// Variable-to-check messages of one row block.
    row_scratch: Vec<f32>,
    /// Terminal beliefs by variable.
    posterior: Vec<Llr>,
    /// Hard decisions by lifted column and the parities of one row block.
    hard_bits: Vec<u8>,
    parity: Vec<u8>,
    kernel: QcKernels,
    backend: LaneBackend,
    last_iterations: usize,
}

impl QcDecoder {
    /// Prepares flooding min-sum decoding from the code's retained QC structure.
    ///
    /// Returns `None` without retained QC structure, for sum-product, or when
    /// a lifted check with a masked entry has exactly one remaining edge.
    /// Uses AVX2 when the `simd` feature and host support it, otherwise
    /// portable code. Construction takes O(B + n·d log d + E) time and
    /// O(n + E) space, for B circulants, E lifted edges and column-block
    /// degree d.
    #[must_use]
    pub fn new(code: &LdpcCode, config: DecoderConfig) -> Option<Self> {
        let rule = config.algorithm().min_sum_rule()?;
        let layout = QcLayout::new(code.quasi_cyclic()?)?;
        let messages = layout.base_edges() * layout.z();
        let (kernel, backend) = (QcKernels::portable(), LaneBackend::Portable);
        #[cfg(feature = "simd")]
        let (kernel, backend) = gf2_kernels_simd::ldpc_qc::detect()
            .map_or((kernel, backend), |fns| (fns, LaneBackend::Avx2));
        Some(Self {
            channel: vec![0.0; layout.n()],
            belief: vec![0.0; layout.n()],
            check_to_var: vec![0.0; messages],
            row_scratch: vec![0.0; layout.max_row_degree * layout.z()],
            posterior: vec![Llr::zero(); layout.n()],
            hard_bits: vec![0; layout.n()],
            parity: vec![0; layout.z()],
            kernel,
            backend,
            layout,
            rule,
            config,
            last_iterations: 0,
        })
    }

    /// Selects the portable kernel on every host.
    #[must_use]
    pub fn with_portable_kernel(mut self) -> Self {
        self.kernel = QcKernels::portable();
        self.backend = LaneBackend::Portable;
        self
    }

    /// The lifting size $Z$ of the decoded structure.
    #[must_use]
    pub fn lifting_size(&self) -> usize {
        self.layout.z
    }

    /// The backend of the selected kernel.
    #[must_use]
    pub fn backend(&self) -> LaneBackend {
        self.backend
    }

    /// Iterations the last decode performed.
    #[must_use]
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// Terminal beliefs by variable, or zeros before the first decode; a zero
    /// cap copies the channel LLRs. Hard decisions use `Llr::hard_decision`.
    #[must_use]
    pub fn posterior_llrs(&self) -> &[Llr] {
        &self.posterior
    }

    /// Bytes of the decoding workspace: channel values, beliefs, messages,
    /// row scratch, posteriors and hard decisions, without the layout.
    #[must_use]
    pub fn workspace_bytes(&self) -> usize {
        std::mem::size_of_val(self.channel.as_slice())
            + std::mem::size_of_val(self.belief.as_slice())
            + std::mem::size_of_val(self.check_to_var.as_slice())
            + std::mem::size_of_val(self.row_scratch.as_slice())
            + std::mem::size_of_val(self.posterior.as_slice())
            + std::mem::size_of_val(self.hard_bits.as_slice())
            + std::mem::size_of_val(self.parity.as_slice())
    }

    /// Decodes a mother-code frame, overwriting `codeword` with all n hard bits.
    ///
    /// Follows `docs/reference/ldpc-decoder-numerical-contract.md`, including
    /// channel posteriors and a syndrome test at cap zero. Each iteration takes
    /// O(n + m + E) time. Reuses all workspace; only `codeword` may grow.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the lifted code's `n`.
    pub fn decode_codeword_into(
        &mut self,
        llrs: &[Llr],
        max_iterations: usize,
        codeword: &mut BitVec,
    ) -> DecodeOutcome {
        assert_eq!(llrs.len(), self.layout.n(), "LLR length must equal n");
        match &self.layout.col_order {
            None => {
                for (slot, &llr) in self.channel.iter_mut().zip(llrs) {
                    *slot = llr.value();
                }
            }
            Some((order, _)) => {
                for (slot, &variable) in self.channel.iter_mut().zip(order) {
                    *slot = llrs[variable].value();
                }
            }
        }
        // With zero messages the first check row reads `channel - 0.0`, the
        // channel value of every lane.
        self.belief.copy_from_slice(&self.channel);
        self.check_to_var.fill(0.0);

        let mut iterations = 0;
        let mut passed = None;
        for iteration in 0..max_iterations {
            iterations = iteration + 1;
            self.check_update();
            self.variable_update();
            if self.config.early_termination() && self.syndrome_passes() {
                passed = Some(true);
                break;
            }
        }
        self.last_iterations = iterations;
        let syndrome_check_passed = passed.unwrap_or_else(|| self.syndrome_passes());
        codeword.clear();
        match &self.layout.col_order {
            None => {
                for (slot, &belief) in self.posterior.iter_mut().zip(&self.belief) {
                    *slot = Llr::new(belief);
                }
                for &bit in &self.hard_bits {
                    codeword.push_bit(bit != 0);
                }
            }
            Some((_, inverse)) => {
                for (slot, &column) in self.posterior.iter_mut().zip(inverse) {
                    *slot = Llr::new(self.belief[column]);
                }
                for &column in inverse {
                    codeword.push_bit(self.hard_bits[column] != 0);
                }
            }
        }
        DecodeOutcome {
            iterations,
            converged: syndrome_check_passed,
            syndrome_check_passed,
            queries: None,
        }
    }

    fn check_update(&mut self) {
        let z = self.layout.z;
        for row_block in 0..self.layout.row_blocks {
            let edges = self.layout.row_edges(row_block);
            let degree = edges.len();
            if degree == 0 {
                continue;
            }
            let scratch = &mut self.row_scratch[..degree * z];
            for ((edge, out), messages) in self.layout.check_edges[edges.clone()]
                .iter()
                .zip(scratch.chunks_exact_mut(z))
                .zip(self.check_to_var[edges.start * z..edges.end * z].chunks_exact(z))
            {
                let block = &self.belief[edge.col_block * z..(edge.col_block + 1) * z];
                self.kernel
                    .rotated_difference(block, edge.shift, messages, out);
            }
            for &(edge, lane) in self.layout.row_masked(row_block) {
                scratch[(edge - edges.start) * z + lane] = f32::INFINITY;
            }
            self.kernel.check_block(
                self.rule.into(),
                scratch,
                &mut self.check_to_var[edges.start * z..edges.end * z],
                degree,
                z,
            );
        }
    }

    fn variable_update(&mut self) {
        let z = self.layout.z;
        for col_block in 0..self.layout.col_blocks {
            let lanes = col_block * z..(col_block + 1) * z;
            let belief = &mut self.belief[lanes.clone()];
            let channel = &self.channel[lanes];
            let segments =
                self.layout.segment_offsets[col_block]..self.layout.segment_offsets[col_block + 1];
            for segment in &self.layout.segments[segments] {
                self.kernel.accumulate_rotated(
                    belief,
                    segment.lanes.clone(),
                    channel,
                    &self.check_to_var,
                    &self.layout.seg_terms[segment.edges.clone()],
                );
            }
        }
    }

    /// Whether every lifted check of the current hard decisions has even
    /// parity, testing one row block at a time.
    fn syndrome_passes(&mut self) -> bool {
        for (bit, &belief) in self.hard_bits.iter_mut().zip(&self.belief) {
            *bit = u8::from(Llr::new(belief).hard_decision());
        }
        let z = self.layout.z;
        for row_block in 0..self.layout.row_blocks {
            self.parity.fill(0);
            let edges = self.layout.row_edges(row_block);
            for edge in &self.layout.check_edges[edges.clone()] {
                // Lifted check i reads lifted column (i + shift) mod Z.
                let block = &self.hard_bits[edge.col_block * z..(edge.col_block + 1) * z];
                xor_rotated(&mut self.parity, block, edge.shift);
            }
            for &(index, lane) in self.layout.row_masked(row_block) {
                let edge = self.layout.check_edges[index];
                self.parity[lane] ^= self.hard_bits[edge.col_block * z + (lane + edge.shift) % z];
            }
            if self.parity.iter().any(|&parity| parity != 0) {
                return false;
            }
        }
        true
    }
}

/// `parity[i] ^= block[(i + shift) mod z]` for every lane `i`. Kept out of
/// line so that its slices stay distinct arguments, which lets the loops
/// vectorize; inlined, both buffers are reached through the decoder.
#[inline(never)]
fn xor_rotated(parity: &mut [u8], block: &[u8], shift: usize) {
    let (head, tail) = parity.split_at_mut(block.len() - shift);
    for (parity, &bit) in head.iter_mut().zip(&block[shift..]) {
        *parity ^= bit;
    }
    for (parity, &bit) in tail.iter_mut().zip(&block[..shift]) {
        *parity ^= bit;
    }
}
