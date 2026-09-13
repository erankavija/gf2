//! The prepared variable-to-check message array both kernel arms read.
//!
//! # The declared preparation
//!
//! A check-node pass is one step of a flooding decode, so the array it reads
//! has to be a state a flooding decode reaches. The preparation is therefore
//! the decoder's own first `warmup_rounds` flooding rounds over the frozen
//! recorded LLRs of a frame, stopped after a variable update:
//!
//! 1. every edge starts at its variable's recorded channel LLR, which is the
//!    array a flooding decode enters its first check pass with, since the
//!    check-to-variable messages start at zero;
//! 2. each round performs one check-node pass through
//!    [`min_sum_check_row`](gf2_coding::ldpc::min_sum_check_row) and one
//!    variable-node pass, `posterior = channel + sum of incoming`, each edge
//!    taking the posterior less its own incoming message.
//!
//! The result is a deterministic function of the frozen LLRs, the graph and
//! `warmup_rounds`, with no timing and no host dependence. Both arms call this
//! module, so both read the same messages; each reports the checksum of the
//! array it prepared, in the canonical check-major edge order, and refuses to
//! publish when it differs from the checksum the cell declares.
//!
//! # Layout
//!
//! The prepared array is returned in the canonical check-major edge order of
//! [`EdgeLayout`], which is the order the gf2 check update reads. The AFF3CT
//! arm permutes it into the variable-major order its update rule indexes
//! through, so each arm reads the messages in the layout its own production
//! decoder holds them in, with the message on every edge identical.

use gf2_coding::ldpc::{min_sum_check_row, EdgeLayout, LdpcCode, MinSumRule};
use gf2_coding::llr::Llr;
use serde::{Deserialize, Serialize};

/// The case an isolated check-node cell declares, forwarded to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelCase {
    /// Directory of the recorded input bundle.
    pub bundle: String,
    /// `ComparisonCode` name; must equal the bundle's.
    pub code: String,
    /// Normalized min-sum factor of the update rule under test.
    pub normalization_factor: f32,
    /// Flooding rounds the preparation runs before it freezes the array.
    pub warmup_rounds: u32,
    /// Frames whose prepared arrays one timed call passes over.
    pub frames: u32,
    /// Checksum the prepared input array must have, or empty to record it.
    pub input_checksum: String,
    /// Checksum the written output array must have, or empty to record it.
    pub output_checksum: String,
}

/// The graph, the prepared messages and an output buffer of the same shape.
pub struct Prepared {
    /// The canonical edge layout of the recorded graph.
    pub layout: EdgeLayout,
    /// Frames the arrays hold.
    pub frames: usize,
    /// `frames * edges` prepared messages in canonical check-major order.
    pub var_to_chk: Vec<f32>,
}

impl Prepared {
    /// Edges of one frame.
    #[must_use]
    pub fn edges(&self) -> usize {
        self.layout.edges()
    }

    /// An output array of the same shape as the prepared input.
    #[must_use]
    pub fn empty_output(&self) -> Vec<f32> {
        vec![0.0; self.var_to_chk.len()]
    }
}

/// The update rule a case declares.
#[must_use]
pub fn rule(case: &KernelCase) -> MinSumRule {
    MinSumRule::Normalized(case.normalization_factor)
}

/// Derives the prepared array of the first `case.frames` recorded frames.
///
/// `llrs` holds those frames back to back, `code.n()` recorded values each.
///
/// # Errors
///
/// Returns an error when `llrs` does not hold whole frames of the code.
pub fn prepare(code: &LdpcCode, llrs: &[f32], case: &KernelCase) -> Result<Prepared, String> {
    let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
    let (n, frames) = (layout.n(), case.frames as usize);
    if llrs.len() != frames * n {
        return Err(format!(
            "{} recorded values are not {frames} frames of {n}",
            llrs.len()
        ));
    }
    let edges = layout.edges();
    let rule = rule(case);
    let mut var_to_chk = vec![0.0f32; frames * edges];
    let mut vtc = vec![Llr::zero(); edges];
    let mut ctv = vec![Llr::zero(); edges];
    for (frame, channel) in llrs.chunks_exact(n).enumerate() {
        for (edge, slot) in vtc.iter_mut().enumerate() {
            *slot = Llr::new(channel[layout.check_edge_var()[edge] as usize]);
        }
        for _ in 0..case.warmup_rounds {
            check_pass(&layout, rule, &vtc, &mut ctv);
            variable_pass(&layout, channel, &ctv, &mut vtc);
        }
        let target = &mut var_to_chk[frame * edges..(frame + 1) * edges];
        for (slot, message) in target.iter_mut().zip(&vtc) {
            *slot = message.value();
        }
    }
    Ok(Prepared {
        layout,
        frames,
        var_to_chk,
    })
}

/// One flooding check-node pass over the canonical check-major arrays.
///
/// This is the production path the gf2 arm times: every check reads its
/// contiguous run of incoming messages and writes the same run of outgoing
/// ones through the shared reduction.
pub fn check_pass(layout: &EdgeLayout, rule: MinSumRule, inputs: &[Llr], outputs: &mut [Llr]) {
    for check in 0..layout.m() {
        let range = layout.check_range(check);
        min_sum_check_row(rule, &inputs[range.clone()], &mut outputs[range]);
    }
}

/// One flooding variable-node pass, used by the preparation only.
fn variable_pass(layout: &EdgeLayout, channel: &[f32], incoming: &[Llr], outgoing: &mut [Llr]) {
    let slots = layout.var_edge_to_check_edge();
    for var in 0..layout.n() {
        let range = layout.var_range(var);
        let mut posterior = channel[var];
        for slot in range.clone() {
            posterior += incoming[slots[slot] as usize].value();
        }
        for slot in range {
            let edge = slots[slot] as usize;
            outgoing[edge] = Llr::new(posterior - incoming[edge].value());
        }
    }
}
