//! Family L: horizontal-layered min-sum over the canonical edge layout.
//!
//! One layer is one check. A layer reads the posteriors the preceding layers
//! of the same sweep already updated, so a sweep is not a flooding iteration
//! and their iteration counts are not comparable. The alphabet, the min-sum
//! rules and the syndrome rule are the canonical decoder's; only the schedule
//! differs.

use gf2_coding::ldpc::{min_sum_check_row, EdgeLayout, LdpcCode, MinSumRule};
use gf2_coding::llr::Llr;

/// Horizontal-layered min-sum decoder.
pub struct LayeredDecoder {
    layout: EdgeLayout,
    rule: MinSumRule,
    posterior: Vec<Llr>,
    check_to_var: Vec<Llr>,
    inputs: Vec<Llr>,
    outputs: Vec<Llr>,
    hard_bits: Vec<bool>,
    last_sweeps: usize,
}

/// What one decode observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayeredOutcome {
    /// Layer sweeps performed; one sweep visits every check once.
    pub sweeps: usize,
    /// Whether the final hard decisions satisfy every check.
    pub syndrome_check_passed: bool,
}

impl LayeredDecoder {
    /// Builds a decoder for `code` with the declared min-sum rule.
    #[must_use]
    pub fn new(code: &LdpcCode, rule: MinSumRule) -> Self {
        let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
        let edges = layout.edges();
        let width = layout.max_check_degree();
        Self {
            posterior: vec![Llr::zero(); layout.n()],
            check_to_var: vec![Llr::zero(); edges],
            inputs: vec![Llr::zero(); width],
            outputs: vec![Llr::zero(); width],
            hard_bits: vec![false; layout.n()],
            layout,
            rule,
            last_sweeps: 0,
        }
    }

    /// Sweeps the last decode performed.
    #[must_use]
    pub fn last_sweeps(&self) -> usize {
        self.last_sweeps
    }

    /// The hard decisions of the last decode.
    #[must_use]
    pub fn hard_bits(&self) -> &[bool] {
        &self.hard_bits
    }

    /// Decodes `llrs` and leaves the hard decisions in [`Self::hard_bits`].
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the code's `n`.
    pub fn decode(
        &mut self,
        llrs: &[Llr],
        max_sweeps: usize,
        early_termination: bool,
    ) -> LayeredOutcome {
        assert_eq!(llrs.len(), self.layout.n(), "LLR length must equal n");
        self.posterior.copy_from_slice(llrs);
        for message in &mut self.check_to_var {
            *message = Llr::zero();
        }

        let mut sweeps = 0;
        for sweep in 0..max_sweeps {
            sweeps = sweep + 1;
            self.layer_sweep();
            if early_termination && self.syndrome_passes() {
                break;
            }
        }
        self.last_sweeps = sweeps;
        LayeredOutcome {
            sweeps,
            syndrome_check_passed: self.syndrome_passes(),
        }
    }

    /// One pass over every check, each reading the posteriors as they stand.
    fn layer_sweep(&mut self) {
        let edge_var = self.layout.check_edge_var();
        for check in 0..self.layout.m() {
            let range = self.layout.check_range(check);
            let degree = range.len();
            if degree == 0 {
                continue;
            }
            let inputs = &mut self.inputs[..degree];
            for (slot, edge) in range.clone().enumerate() {
                let var = edge_var[edge] as usize;
                inputs[slot] =
                    Llr::new(self.posterior[var].value() - self.check_to_var[edge].value());
            }
            let outputs = &mut self.outputs[..degree];
            min_sum_check_row(self.rule, inputs, outputs);
            for (slot, edge) in range.enumerate() {
                let var = edge_var[edge] as usize;
                self.posterior[var] = Llr::new(inputs[slot].value() + outputs[slot].value());
                self.check_to_var[edge] = outputs[slot];
            }
        }
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, posterior) in self.hard_bits.iter_mut().zip(self.posterior.iter()) {
            *bit = posterior.hard_decision();
        }
        let edge_var = self.layout.check_edge_var();
        for check in 0..self.layout.m() {
            let mut parity = false;
            for edge in self.layout.check_range(check) {
                parity ^= self.hard_bits[edge_var[edge] as usize];
            }
            if parity {
                return false;
            }
        }
        true
    }
}
