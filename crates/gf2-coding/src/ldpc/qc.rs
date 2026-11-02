//! Intra-frame flooding over QC blocks. Check lanes are lifted positions;
//! column-block check order and row-block variable order preserve canonical sums.

use super::{DecodeOutcome, DecoderConfig, LdpcCode, MinSumRule, QuasiCyclicLdpc};
use crate::llr::Llr;
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_qc::QcCheckFn;

/// One circulant block of the base matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaseEdge {
    col_block: usize,
    shift: usize,
}

/// The block structure of a quasi-cyclic code, in both traversal orders.
///
/// `check_edges` is ordered by row block then column block, so a row block owns
/// one contiguous run; `var_edges` holds indices into it ordered by column block
/// then row block, so a column block owns one contiguous run of those indices.
struct QcLayout {
    z: usize,
    row_blocks: usize,
    col_blocks: usize,
    check_edges: Vec<BaseEdge>,
    check_offsets: Vec<usize>,
    var_edges: Vec<usize>,
    var_offsets: Vec<usize>,
}

impl QcLayout {
    fn new(code: &QuasiCyclicLdpc) -> Self {
        let z = code.expansion_factor();
        let base = code.base_matrix();
        let row_blocks = code.base_rows();
        let col_blocks = code.base_cols();

        let mut check_edges = Vec::new();
        let mut check_offsets = vec![0usize; row_blocks + 1];
        for (row_block, row) in base.iter().enumerate() {
            for (col_block, &entry) in row.iter().enumerate() {
                if entry >= 0 {
                    check_edges.push(BaseEdge {
                        col_block,
                        shift: entry as usize,
                    });
                }
            }
            check_offsets[row_block + 1] = check_edges.len();
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

        Self {
            z,
            row_blocks,
            col_blocks,
            check_edges,
            check_offsets,
            var_edges,
            var_offsets,
        }
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
}

/// Flooding min-sum decoder vectorized across one frame's lifted positions.
pub struct QcDecoder {
    layout: QcLayout,
    rule: MinSumRule,
    config: DecoderConfig,
    channel: Vec<f32>,
    check_to_var: Vec<f32>,
    var_to_check: Vec<f32>,
    posterior: Vec<Llr>,
    hard_bits: Vec<bool>,
    kernel: QcCheckFn,
    last_iterations: usize,
}

impl QcDecoder {
    /// Prepares flooding min-sum decoding from the code's retained QC structure.
    ///
    /// Returns `None` without retained QC structure or for sum-product. Uses
    /// AVX2 when the `simd` feature and host support it, otherwise portable code.
    /// Construction takes O(B + n + m + E) time and O(n + m + E) space,
    /// for B base-matrix entries and E lifted edges.
    #[must_use]
    pub fn new(code: &LdpcCode, config: DecoderConfig) -> Option<Self> {
        let rule = config.algorithm().min_sum_rule()?;
        let layout = QcLayout::new(code.quasi_cyclic()?);
        let messages = layout.base_edges() * layout.z();
        let kernel = QcCheckFn::portable();
        #[cfg(feature = "simd")]
        let kernel = gf2_kernels_simd::ldpc_qc::detect().unwrap_or(kernel);
        Some(Self {
            channel: vec![0.0; layout.n()],
            check_to_var: vec![0.0; messages],
            var_to_check: vec![0.0; messages],
            posterior: vec![Llr::zero(); layout.n()],
            hard_bits: vec![false; layout.n()],
            kernel,
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
        self
    }

    /// Iterations the last decode performed.
    #[must_use]
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// Terminal beliefs, or zeros before the first decode; a zero cap copies
    /// the channel LLRs. Hard decisions use `Llr::hard_decision`.
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
        for (slot, &llr) in self.channel.iter_mut().zip(llrs.iter()) {
            *slot = llr.value();
        }

        if max_iterations == 0 {
            self.posterior.copy_from_slice(llrs);
        }
        let z = self.layout.z;
        self.check_to_var.fill(0.0);
        for (index, edge) in self.layout.check_edges.iter().enumerate() {
            let base = edge.col_block * z;
            for i in 0..z {
                self.var_to_check[index * z + i] = self.channel[base + (i + edge.shift) % z];
            }
        }

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
        for &bit in &self.hard_bits {
            codeword.push_bit(bit);
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
            let run = self.layout.var_offsets[col_block]..self.layout.var_offsets[col_block + 1];
            let block_start = col_block * z;
            let belief = &mut self.posterior[block_start..block_start + z];
            for (slot, &channel) in belief
                .iter_mut()
                .zip(&self.channel[block_start..block_start + z])
            {
                *slot = Llr::new(channel);
            }

            for &index in &self.layout.var_edges[run.clone()] {
                let shift = self.layout.check_edges[index].shift;
                let incoming = &self.check_to_var[index * z..(index + 1) * z];
                // Lifted position j of the variable block reads lifted position
                // (j - shift) mod Z of the check block: two contiguous runs.
                for (j, slot) in belief[..shift].iter_mut().enumerate() {
                    *slot = Llr::new(slot.value() + incoming[j + z - shift]);
                }
                for (offset, slot) in belief[shift..].iter_mut().enumerate() {
                    *slot = Llr::new(slot.value() + incoming[offset]);
                }
            }

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
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, &posterior) in self.hard_bits.iter_mut().zip(self.posterior.iter()) {
            *bit = posterior.hard_decision();
        }
        let z = self.layout.z;
        for row_block in 0..self.layout.row_blocks {
            let start = self.layout.check_offsets[row_block];
            let end = self.layout.check_offsets[row_block + 1];
            for i in 0..z {
                let mut parity = false;
                for edge in &self.layout.check_edges[start..end] {
                    parity ^= self.hard_bits[edge.col_block * z + (i + edge.shift) % z];
                }
                if parity {
                    return false;
                }
            }
        }
        true
    }
}
