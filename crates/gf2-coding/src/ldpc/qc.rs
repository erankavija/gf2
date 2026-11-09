//! Intra-frame flooding over QC blocks. Check lanes are lifted positions;
//! each column block splits into lane segments that share one canonical
//! accumulation order, so variable sums equal the canonical decoder's.

use super::{DecodeOutcome, DecoderConfig, LaneBackend, LdpcCode, MinSumRule, QuasiCyclicLdpc};
use crate::llr::Llr;
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_qc::QcCheckFn;
use std::ops::Range;

/// One circulant of the block description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaseEdge {
    col_block: usize,
    shift: usize,
}

/// Variable lanes `lanes` of a column block accumulate the edges
/// `seg_edges[edges]` in that order.
struct Segment {
    lanes: Range<usize>,
    edges: Range<usize>,
}

/// The block structure of a quasi-cyclic code, in both traversal orders.
///
/// `check_edges` is ordered by row block, so a row block owns one contiguous
/// run; `var_edges` holds indices into it grouped by column block. A masked
/// (edge, check lane) pair carries a +∞ variable-to-check message, which the
/// check rule skips as it skips an absent edge.
struct QcLayout {
    z: usize,
    row_blocks: usize,
    col_blocks: usize,
    check_edges: Vec<BaseEdge>,
    check_offsets: Vec<usize>,
    var_edges: Vec<usize>,
    var_offsets: Vec<usize>,
    segments: Vec<Segment>,
    segment_offsets: Vec<usize>,
    seg_edges: Vec<usize>,
    /// (edge, check lane) pairs without an entry, ascending.
    masked: Vec<(usize, usize)>,
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

        Some(Self {
            z,
            row_blocks,
            col_blocks,
            check_edges,
            check_offsets,
            var_edges,
            var_offsets,
            segments,
            segment_offsets,
            seg_edges,
            masked,
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

    fn mask_messages(&self, var_to_check: &mut [f32]) {
        for &(edge, lane) in &self.masked {
            var_to_check[edge * self.z + lane] = f32::INFINITY;
        }
    }
}

/// Flooding min-sum decoder vectorized across one frame's lifted positions.
pub struct QcDecoder {
    layout: QcLayout,
    rule: MinSumRule,
    config: DecoderConfig,
    /// Channel values and beliefs by lifted column.
    channel: Vec<f32>,
    belief: Vec<Llr>,
    check_to_var: Vec<f32>,
    var_to_check: Vec<f32>,
    /// Terminal beliefs by variable.
    posterior: Vec<Llr>,
    hard_bits: Vec<bool>,
    kernel: QcCheckFn,
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
        let (kernel, backend) = (QcCheckFn::portable(), LaneBackend::Portable);
        #[cfg(feature = "simd")]
        let (kernel, backend) = gf2_kernels_simd::ldpc_qc::detect()
            .map_or((kernel, backend), |fns| (fns, LaneBackend::Avx2));
        Some(Self {
            channel: vec![0.0; layout.n()],
            belief: vec![Llr::zero(); layout.n()],
            check_to_var: vec![0.0; messages],
            var_to_check: vec![0.0; messages],
            posterior: vec![Llr::zero(); layout.n()],
            hard_bits: vec![false; layout.n()],
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
        self.kernel = QcCheckFn::portable();
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
                if max_iterations == 0 {
                    self.belief.copy_from_slice(llrs);
                }
            }
            Some((order, _)) => {
                for (slot, &variable) in self.channel.iter_mut().zip(order) {
                    *slot = llrs[variable].value();
                }
                if max_iterations == 0 {
                    for (slot, &variable) in self.belief.iter_mut().zip(order) {
                        *slot = llrs[variable];
                    }
                }
            }
        }

        let z = self.layout.z;
        self.check_to_var.fill(0.0);
        for (index, edge) in self.layout.check_edges.iter().enumerate() {
            let base = edge.col_block * z;
            for i in 0..z {
                self.var_to_check[index * z + i] = self.channel[base + (i + edge.shift) % z];
            }
        }
        self.layout.mask_messages(&mut self.var_to_check);

        let mut iterations = 0;
        for iteration in 0..max_iterations {
            iterations = iteration + 1;
            self.check_update();
            self.variable_update();
            if self.config.early_termination() && self.syndrome_passes() {
                break;
            }
        }
        self.last_iterations = iterations;
        let syndrome_check_passed = self.syndrome_passes();
        codeword.clear();
        match &self.layout.col_order {
            None => {
                self.posterior.copy_from_slice(&self.belief);
                for &bit in &self.hard_bits {
                    codeword.push_bit(bit);
                }
            }
            Some((_, inverse)) => {
                for (slot, &column) in self.posterior.iter_mut().zip(inverse) {
                    *slot = self.belief[column];
                }
                for &column in inverse {
                    codeword.push_bit(self.hard_bits[column]);
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
            let start = self.layout.check_offsets[row_block];
            let end = self.layout.check_offsets[row_block + 1];
            let degree = end - start;
            if degree == 0 {
                continue;
            }
            let inputs = &self.var_to_check[start * z..end * z];
            let outputs = &mut self.check_to_var[start * z..end * z];
            self.kernel
                .check_block(self.rule.into(), inputs, outputs, degree, z);
        }
    }

    fn variable_update(&mut self) {
        let z = self.layout.z;
        for col_block in 0..self.layout.col_blocks {
            let block_start = col_block * z;
            let belief = &mut self.belief[block_start..block_start + z];
            for (slot, &channel) in belief
                .iter_mut()
                .zip(&self.channel[block_start..block_start + z])
            {
                *slot = Llr::new(channel);
            }

            let segments =
                self.layout.segment_offsets[col_block]..self.layout.segment_offsets[col_block + 1];
            for segment in &self.layout.segments[segments] {
                let (start, end) = (segment.lanes.start, segment.lanes.end);
                for &index in &self.layout.seg_edges[segment.edges.clone()] {
                    let shift = self.layout.check_edges[index].shift;
                    let incoming = &self.check_to_var[index * z..(index + 1) * z];
                    // Lifted position j of the variable block reads lifted
                    // position (j - shift) mod Z of the check block: lanes
                    // below the shift wrap.
                    let split = shift.clamp(start, end);
                    if start < split {
                        for (slot, &value) in belief[start..split]
                            .iter_mut()
                            .zip(&incoming[start + z - shift..split + z - shift])
                        {
                            *slot = Llr::new(slot.value() + value);
                        }
                    }
                    if split < end {
                        for (slot, &value) in belief[split..end]
                            .iter_mut()
                            .zip(&incoming[split - shift..end - shift])
                        {
                            *slot = Llr::new(slot.value() + value);
                        }
                    }
                }
            }

            let run = self.layout.var_offsets[col_block]..self.layout.var_offsets[col_block + 1];
            for &index in &self.layout.var_edges[run] {
                let shift = self.layout.check_edges[index].shift;
                let outgoing = &mut self.var_to_check[index * z..(index + 1) * z];
                let incoming = &self.check_to_var[index * z..(index + 1) * z];
                let split = z - shift;
                for i in 0..split {
                    outgoing[i] = belief[i + shift].value() - incoming[i];
                }
                for i in split..z {
                    outgoing[i] = belief[i + shift - z].value() - incoming[i];
                }
            }
        }
        self.layout.mask_messages(&mut self.var_to_check);
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, &belief) in self.hard_bits.iter_mut().zip(self.belief.iter()) {
            *bit = belief.hard_decision();
        }
        let z = self.layout.z;
        let masked = &self.layout.masked;
        for row_block in 0..self.layout.row_blocks {
            let start = self.layout.check_offsets[row_block];
            let end = self.layout.check_offsets[row_block + 1];
            let row_masked = &masked[masked.partition_point(|&(edge, _)| edge < start)
                ..masked.partition_point(|&(edge, _)| edge < end)];
            for i in 0..z {
                let mut parity = false;
                for (offset, edge) in self.layout.check_edges[start..end].iter().enumerate() {
                    if row_masked.is_empty() || !row_masked.contains(&(start + offset, i)) {
                        parity ^= self.hard_bits[edge.col_block * z + (i + edge.shift) % z];
                    }
                }
                if parity {
                    return false;
                }
            }
        }
        true
    }
}
