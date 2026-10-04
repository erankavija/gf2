//! Family Q: flooding min-sum over a symmetric integer alphabet.
//!
//! The schedule, edge order and syndrome rule are the canonical decoder's; the
//! message alphabet is `i8` or `i16`, restricted to `[-LIMIT, LIMIT]` with
//! `LIMIT` one below the type's magnitude range. Excluding the type minimum is
//! a contract clause: the magnitude of the type minimum is not representable,
//! and an AVX2 `vpabsb` over an alphabet that admits it returns a negative
//! magnitude.
//!
//! Accumulation saturates at every step and keeps the canonical order, so the
//! order is observable rather than incidental: a reassociated saturating sum
//! can differ by the whole clip excess, not only in the last bits.

use gf2_coding::ldpc::{EdgeLayout, LdpcCode};
use gf2_coding::llr::Llr;

/// A symmetric integer message alphabet.
pub trait Alphabet: Copy + PartialEq + std::fmt::Debug {
    /// The largest representable magnitude; `-LIMIT` is the smallest value.
    const LIMIT: i32;
    /// A short name for the width, used in evidence records.
    const NAME: &'static str;

    /// The additive identity.
    fn zero() -> Self;
    /// Widens to the accumulator type.
    fn widen(self) -> i32;
    /// Narrows with symmetric clipping.
    fn clip(value: i32) -> Self;

    /// Saturating sum inside the alphabet.
    fn sat_add(self, other: Self) -> Self {
        Self::clip(self.widen() + other.widen())
    }

    /// Saturating difference inside the alphabet.
    fn sat_sub(self, other: Self) -> Self {
        Self::clip(self.widen() - other.widen())
    }

    /// Quantizes a channel LLR: round half away from zero, then clip.
    fn quantize(llr: Llr, scale: f32) -> Self {
        let scaled = llr.value() * scale;
        if scaled.is_nan() {
            return Self::zero();
        }
        Self::clip(scaled.round().clamp(-1.0e9, 1.0e9) as i32)
    }

    /// Whether the message decides to bit one.
    fn is_negative(self) -> bool {
        self.widen() < 0
    }
}

impl Alphabet for i8 {
    const LIMIT: i32 = 127;
    const NAME: &'static str = "i8";

    fn zero() -> Self {
        0
    }

    fn widen(self) -> i32 {
        i32::from(self)
    }

    fn clip(value: i32) -> Self {
        value.clamp(-Self::LIMIT, Self::LIMIT) as i8
    }
}

impl Alphabet for i16 {
    const LIMIT: i32 = 32767;
    const NAME: &'static str = "i16";

    fn zero() -> Self {
        0
    }

    fn widen(self) -> i32 {
        i32::from(self)
    }

    fn clip(value: i32) -> Self {
        value.clamp(-Self::LIMIT, Self::LIMIT) as i16
    }
}

/// The check-node rule, with its normalization expressed in the alphabet.
///
/// A float `alpha` has no exact image in an integer alphabet, so the
/// normalized rule is a fixed-point multiply-shift and the offset rule an
/// integer subtraction floored at zero. Both are declared, not approximated
/// from the float constant at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizedRule {
    /// The signed excluded minimum.
    Plain,
    /// `(magnitude * numerator) >> shift`, truncating toward zero.
    Normalized {
        /// Fixed-point numerator.
        numerator: i32,
        /// Right shift applied after the multiply.
        shift: u32,
    },
    /// `max(magnitude - offset, 0)`.
    Offset {
        /// Quantized offset in alphabet units.
        offset: i32,
    },
}

impl QuantizedRule {
    fn apply(self, negative: bool, magnitude: i32) -> i32 {
        let scaled = match self {
            Self::Plain => magnitude,
            Self::Normalized { numerator, shift } => (magnitude * numerator) >> shift,
            Self::Offset { offset } => (magnitude - offset).max(0),
        };
        if negative {
            -scaled
        } else {
            scaled
        }
    }
}

/// One check node's whole update over the integer alphabet.
///
/// The two passes, the tie rule and the sign rule are the canonical
/// reduction's: pass one keeps the two smallest magnitudes, the position of
/// the first smallest under a strict comparison, and the parity of the
/// negative inputs; pass two gives each output the minimum excluding its own
/// input and the shared parity with its own sign removed.
///
/// # Panics
///
/// Panics if `inputs` and `outputs` have different lengths.
pub fn quantized_check_row<A: Alphabet>(rule: QuantizedRule, inputs: &[A], outputs: &mut [A]) {
    assert_eq!(
        inputs.len(),
        outputs.len(),
        "a check's outputs and inputs sit on the same edges"
    );
    match inputs.len() {
        0 => return,
        1 => {
            outputs[0] = A::zero();
            return;
        }
        _ => {}
    }

    let mut min1 = i32::MAX;
    let mut min2 = i32::MAX;
    let mut arg1 = usize::MAX;
    let mut negative = false;
    for (index, input) in inputs.iter().enumerate() {
        let value = input.widen();
        negative ^= value < 0;
        let magnitude = value.abs();
        if magnitude < min1 {
            min2 = min1;
            min1 = magnitude;
            arg1 = index;
        } else if magnitude < min2 {
            min2 = magnitude;
        }
    }

    for (index, (input, output)) in inputs.iter().zip(outputs.iter_mut()).enumerate() {
        let sign = negative ^ (input.widen() < 0);
        let magnitude = if index == arg1 { min2 } else { min1 };
        *output = A::clip(rule.apply(sign, magnitude));
    }
}

/// Flooding min-sum decoder over a symmetric integer alphabet.
pub struct QuantizedDecoder<A: Alphabet> {
    layout: EdgeLayout,
    rule: QuantizedRule,
    scale: f32,
    channel: Vec<A>,
    check_to_var: Vec<A>,
    var_to_check: Vec<A>,
    beliefs: Vec<A>,
    hard_bits: Vec<bool>,
    scratch: Vec<A>,
    last_iterations: usize,
}

/// What one decode observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizedOutcome {
    /// Iterations performed.
    pub iterations: usize,
    /// Whether the final hard decisions satisfy every check.
    pub syndrome_check_passed: bool,
}

impl<A: Alphabet> QuantizedDecoder<A> {
    /// Builds a decoder for `code` with the declared rule and channel scale.
    #[must_use]
    pub fn new(code: &LdpcCode, rule: QuantizedRule, scale: f32) -> Self {
        let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
        let edges = layout.edges();
        let width = layout.max_check_degree();
        Self {
            channel: vec![A::zero(); layout.n()],
            check_to_var: vec![A::zero(); edges],
            var_to_check: vec![A::zero(); edges],
            beliefs: vec![A::zero(); layout.n()],
            hard_bits: vec![false; layout.n()],
            scratch: vec![A::zero(); width],
            layout,
            rule,
            scale,
            last_iterations: 0,
        }
    }

    /// The channel scale this decoder quantizes with.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Iterations the last decode performed.
    #[must_use]
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// The hard decisions of the last decode, one per codeword position.
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
        max_iterations: usize,
        early_termination: bool,
    ) -> QuantizedOutcome {
        assert_eq!(llrs.len(), self.layout.n(), "LLR length must equal n");
        for (slot, &llr) in self.channel.iter_mut().zip(llrs.iter()) {
            *slot = A::quantize(llr, self.scale);
        }

        for slot in &mut self.check_to_var {
            *slot = A::zero();
        }
        let edge_var = self.layout.check_edge_var();
        for (message, &var) in self.var_to_check.iter_mut().zip(edge_var.iter()) {
            *message = self.channel[var as usize];
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
        QuantizedOutcome {
            iterations,
            syndrome_check_passed: self.syndrome_passes(),
        }
    }

    fn check_update(&mut self) {
        for check in 0..self.layout.m() {
            let range = self.layout.check_range(check);
            let degree = range.len();
            let scratch = &mut self.scratch[..degree];
            scratch.copy_from_slice(&self.var_to_check[range.clone()]);
            quantized_check_row(self.rule, scratch, &mut self.check_to_var[range]);
        }
    }

    fn variable_update(&mut self) {
        let slots = self.layout.var_edge_to_check_edge();
        for var in 0..self.layout.n() {
            let range = self.layout.var_range(var);
            let mut belief = self.channel[var];
            for slot in range.clone() {
                let edge = slots[slot] as usize;
                belief = belief.sat_add(self.check_to_var[edge]);
            }
            self.beliefs[var] = belief;
            for slot in range {
                let edge = slots[slot] as usize;
                self.var_to_check[edge] = belief.sat_sub(self.check_to_var[edge]);
            }
        }
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, belief) in self.hard_bits.iter_mut().zip(self.beliefs.iter()) {
            *bit = belief.is_negative();
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
