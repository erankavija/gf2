//! Family QC: intra-frame vectorization over the lifted positions of one frame.
//!
//! A quasi-cyclic code's parity-check matrix is a base matrix of circulant
//! blocks of size `Z`. Block `(r, c)` with shift `s` joins check `r*Z + i` to
//! variable `c*Z + ((i + s) mod Z)`, so the `Z` lifted positions of one block
//! are `Z` distinct checks that carry the same base-graph edge. Putting them in
//! adjacent lanes vectorizes one frame's update: no batch is filled and no
//! frame's latency waits for another.
//!
//! # Numerical contract
//!
//! Unchanged from the canonical flooding decoder, and tested rather than
//! asserted. Lanes are lifted positions, never a node's own edges, so no
//! reduction inside a check and no variable's accumulation is reassociated.
//! The orders match because the block indices order the canonical ones: a
//! check's variables ordered by increasing index are its base edges ordered by
//! column block, and a variable's checks ordered by increasing index are its
//! base edges ordered by row block.

use gf2_coding::ldpc::{MinSumRule, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;

/// One circulant block of the base matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaseEdge {
    row_block: usize,
    col_block: usize,
    shift: usize,
}

/// The block structure of a quasi-cyclic code, in both traversal orders.
///
/// `check_edges` is ordered by row block then column block, so a row block owns
/// one contiguous run; `var_edges` holds indices into it ordered by column block
/// then row block, so a column block owns one contiguous run of those indices.
pub struct QcLayout {
    z: usize,
    row_blocks: usize,
    col_blocks: usize,
    check_edges: Vec<BaseEdge>,
    check_offsets: Vec<usize>,
    var_edges: Vec<usize>,
    var_offsets: Vec<usize>,
}

impl QcLayout {
    /// Builds the block layout of `code`.
    ///
    /// # Panics
    ///
    /// Panics if the expansion factor is zero.
    #[must_use]
    pub fn new(code: &QuasiCyclicLdpc) -> Self {
        let z = code.expansion_factor();
        assert!(z > 0, "a lifted code has a positive expansion factor");
        let base = code.base_matrix();
        let row_blocks = code.base_rows();
        let col_blocks = code.base_cols();

        let mut check_edges = Vec::new();
        let mut check_offsets = vec![0usize; row_blocks + 1];
        for (row_block, row) in base.iter().enumerate() {
            for (col_block, &entry) in row.iter().enumerate() {
                if entry >= 0 {
                    check_edges.push(BaseEdge {
                        row_block,
                        col_block,
                        shift: (entry as usize) % z,
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

    /// The lifting size.
    #[must_use]
    pub fn z(&self) -> usize {
        self.z
    }

    /// Codeword length of the lifted code.
    #[must_use]
    pub fn n(&self) -> usize {
        self.col_blocks * self.z
    }

    /// Check count of the lifted code.
    #[must_use]
    pub fn m(&self) -> usize {
        self.row_blocks * self.z
    }

    /// Base edges, which is the message block count.
    #[must_use]
    pub fn base_edges(&self) -> usize {
        self.check_edges.len()
    }
}

/// Flooding min-sum decoder vectorized across one frame's lifted positions.
pub struct QcDecoder {
    layout: QcLayout,
    rule: MinSumRule,
    channel: Vec<f32>,
    check_to_var: Vec<f32>,
    var_to_check: Vec<f32>,
    posterior: Vec<f32>,
    hard_bits: Vec<bool>,
    use_avx2: bool,
    last_iterations: usize,
}

/// What one decode observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QcOutcome {
    /// Flooding iterations performed.
    pub iterations: usize,
    /// Whether the final hard decisions satisfy every check.
    pub syndrome_check_passed: bool,
}

impl QcDecoder {
    /// Builds a decoder for `code`, selecting the AVX2 check kernel when the
    /// host supports it.
    #[must_use]
    pub fn new(code: &QuasiCyclicLdpc, rule: MinSumRule) -> Self {
        Self::with_dispatch(code, rule, detect_avx2())
    }

    /// Builds a decoder with the check kernel chosen explicitly.
    ///
    /// The scalar path is the reference the AVX2 path is asserted against, so
    /// tests construct both over one code.
    #[must_use]
    pub fn with_dispatch(code: &QuasiCyclicLdpc, rule: MinSumRule, use_avx2: bool) -> Self {
        let layout = QcLayout::new(code);
        let messages = layout.base_edges() * layout.z();
        Self {
            channel: vec![0.0; layout.n()],
            check_to_var: vec![0.0; messages],
            var_to_check: vec![0.0; messages],
            posterior: vec![0.0; layout.n()],
            hard_bits: vec![false; layout.n()],
            use_avx2: use_avx2 && detect_avx2(),
            layout,
            rule,
            last_iterations: 0,
        }
    }

    /// Whether this decoder runs the AVX2 check kernel.
    #[must_use]
    pub fn uses_avx2(&self) -> bool {
        self.use_avx2
    }

    /// The block layout.
    #[must_use]
    pub fn layout(&self) -> &QcLayout {
        &self.layout
    }

    /// Iterations the last decode performed.
    #[must_use]
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// The hard decisions of the last decode.
    #[must_use]
    pub fn hard_bits(&self) -> &[bool] {
        &self.hard_bits
    }

    /// The posterior of each codeword position after the last decode.
    #[must_use]
    pub fn posterior(&self) -> &[f32] {
        &self.posterior
    }

    /// Decodes `llrs` and leaves the hard decisions in [`Self::hard_bits`].
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the lifted code's `n`.
    pub fn decode(
        &mut self,
        llrs: &[Llr],
        max_iterations: usize,
        early_termination: bool,
    ) -> QcOutcome {
        assert_eq!(llrs.len(), self.layout.n(), "LLR length must equal n");
        for (slot, &llr) in self.channel.iter_mut().zip(llrs.iter()) {
            *slot = llr.value();
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
            if early_termination && self.syndrome_passes() {
                break;
            }
        }
        self.last_iterations = iterations;
        QcOutcome {
            iterations,
            syndrome_check_passed: self.syndrome_passes(),
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
            if self.use_avx2 {
                // SAFETY: `use_avx2` is set only when `is_x86_feature_detected!`
                // observed AVX2 at run time, and both slices carry exactly
                // `degree * z` elements, which is the kernel's precondition.
                unsafe { avx2::check_block(self.rule, inputs, outputs, degree, z) };
            } else {
                check_block_scalar(self.rule, inputs, outputs, degree, z);
            }
        }
    }

    fn variable_update(&mut self) {
        let z = self.layout.z;
        for col_block in 0..self.layout.col_blocks {
            let run = self.layout.var_offsets[col_block]..self.layout.var_offsets[col_block + 1];
            let block_start = col_block * z;
            let belief = &mut self.posterior[block_start..block_start + z];
            belief.copy_from_slice(&self.channel[block_start..block_start + z]);

            for &index in &self.layout.var_edges[run.clone()] {
                let shift = self.layout.check_edges[index].shift;
                let incoming = &self.check_to_var[index * z..(index + 1) * z];
                // Lifted position j of the variable block reads lifted position
                // (j - shift) mod Z of the check block: two contiguous runs.
                for (j, slot) in belief[..shift].iter_mut().enumerate() {
                    *slot += incoming[j + z - shift];
                }
                for (offset, slot) in belief[shift..].iter_mut().enumerate() {
                    *slot += incoming[offset];
                }
            }

            for &index in &self.layout.var_edges[run] {
                let shift = self.layout.check_edges[index].shift;
                let outgoing = &mut self.var_to_check[index * z..(index + 1) * z];
                let incoming = &self.check_to_var[index * z..(index + 1) * z];
                let split = z - shift;
                for i in 0..split {
                    outgoing[i] = belief[i + shift] - incoming[i];
                }
                for i in split..z {
                    outgoing[i] = belief[i + shift - z] - incoming[i];
                }
            }
        }
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, &posterior) in self.hard_bits.iter_mut().zip(self.posterior.iter()) {
            *bit = posterior < 0.0;
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

/// The scalar reference the AVX2 kernel is asserted against.
///
/// Each lane is one lifted position; the fold across `degree` base edges is
/// the canonical two-pass reduction, in the base edges' column-block order.
fn check_block_scalar(
    rule: MinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
) {
    if degree == 1 {
        outputs[..z].fill(0.0);
        return;
    }
    for lane in 0..z {
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
            outputs[edge * z + lane] = apply_rule(rule, sign, magnitude);
        }
    }
}

fn apply_rule(rule: MinSumRule, sign: f32, magnitude: f32) -> f32 {
    match rule {
        MinSumRule::Plain => sign * magnitude,
        MinSumRule::Normalized(alpha) => alpha * (sign * magnitude),
        MinSumRule::Offset(beta) => sign * (magnitude - beta).max(0.0),
    }
}

fn detect_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use super::{check_block_scalar, MinSumRule};
    use std::arch::x86_64::*;

    /// One row block's whole check update, eight lifted positions at a time.
    ///
    /// Every comparison mirrors the scalar reference's branch, so the two
    /// agree bit for bit including on non-finite inputs: `_CMP_LT_OS` is the
    /// reference's `<`, and `_CMP_NGE_UQ` is the negation of its `>= 0.0`,
    /// which is true for a NaN exactly as the reference's `else` branch is.
    ///
    /// # Safety
    ///
    /// The caller detected AVX2 at run time, and `inputs` and `outputs` each
    /// hold exactly `degree * z` elements.
    pub unsafe fn check_block(
        rule: MinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
    ) {
        if degree == 1 {
            outputs[..z].fill(0.0);
            return;
        }
        let groups = z / 8;
        if groups > 0 {
            // SAFETY: the caller's precondition; each load and store below is
            // eight elements at `edge * z + lane` with `lane + 8 <= z` and
            // `edge < degree`, so every access lies inside `degree * z`.
            unsafe { check_block_lanes(rule, inputs, outputs, degree, z, groups) };
        }
        let tail = groups * 8;
        if tail < z {
            check_block_tail(rule, inputs, outputs, degree, z, tail);
        }
    }

    /// The residual lanes, which the scalar reference decides.
    fn check_block_tail(
        rule: MinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
        tail: usize,
    ) {
        let width = z - tail;
        let mut gathered = vec![0.0f32; degree * width];
        for edge in 0..degree {
            gathered[edge * width..(edge + 1) * width]
                .copy_from_slice(&inputs[edge * z + tail..edge * z + z]);
        }
        let mut written = vec![0.0f32; degree * width];
        check_block_scalar(rule, &gathered, &mut written, degree, width);
        for edge in 0..degree {
            outputs[edge * z + tail..edge * z + z]
                .copy_from_slice(&written[edge * width..(edge + 1) * width]);
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn check_block_lanes(
        rule: MinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
        groups: usize,
    ) {
        unsafe {
            let sign_bit = _mm256_castsi256_ps(_mm256_set1_epi32(-0x8000_0000i32));
            let zero = _mm256_setzero_ps();
            for group in 0..groups {
                let lane = group * 8;
                let mut min1 = _mm256_set1_ps(f32::INFINITY);
                let mut min2 = _mm256_set1_ps(f32::INFINITY);
                let mut arg1 = _mm256_set1_epi32(-1);
                let mut sign_product = _mm256_set1_ps(1.0);

                for edge in 0..degree {
                    let value = _mm256_loadu_ps(inputs.as_ptr().add(edge * z + lane));
                    let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
                    sign_product = _mm256_blendv_ps(
                        sign_product,
                        _mm256_xor_ps(sign_product, sign_bit),
                        negative,
                    );
                    let magnitude = _mm256_andnot_ps(sign_bit, value);
                    let below1 = _mm256_cmp_ps(magnitude, min1, _CMP_LT_OS);
                    let below2 = _mm256_cmp_ps(magnitude, min2, _CMP_LT_OS);
                    let promoted = _mm256_blendv_ps(min2, magnitude, below2);
                    min2 = _mm256_blendv_ps(promoted, min1, below1);
                    min1 = _mm256_blendv_ps(min1, magnitude, below1);
                    arg1 = _mm256_blendv_epi8(
                        arg1,
                        _mm256_set1_epi32(edge as i32),
                        _mm256_castps_si256(below1),
                    );
                }

                for edge in 0..degree {
                    let value = _mm256_loadu_ps(inputs.as_ptr().add(edge * z + lane));
                    let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
                    let sign = _mm256_blendv_ps(
                        sign_product,
                        _mm256_xor_ps(sign_product, sign_bit),
                        negative,
                    );
                    let at_min = _mm256_castsi256_ps(_mm256_cmpeq_epi32(
                        arg1,
                        _mm256_set1_epi32(edge as i32),
                    ));
                    let magnitude = _mm256_blendv_ps(min1, min2, at_min);
                    let out = match rule {
                        MinSumRule::Plain => _mm256_mul_ps(sign, magnitude),
                        MinSumRule::Normalized(alpha) => {
                            _mm256_mul_ps(_mm256_set1_ps(alpha), _mm256_mul_ps(sign, magnitude))
                        }
                        MinSumRule::Offset(beta) => _mm256_mul_ps(
                            sign,
                            _mm256_max_ps(_mm256_sub_ps(magnitude, _mm256_set1_ps(beta)), zero),
                        ),
                    };
                    _mm256_storeu_ps(outputs.as_mut_ptr().add(edge * z + lane), out);
                }
            }
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
mod avx2 {
    use super::{check_block_scalar, MinSumRule};

    /// # Safety
    ///
    /// Unreachable on this architecture; the scalar path decides every block.
    pub unsafe fn check_block(
        rule: MinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
    ) {
        check_block_scalar(rule, inputs, outputs, degree, z);
    }
}
