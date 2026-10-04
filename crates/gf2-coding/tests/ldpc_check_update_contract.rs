//! Behavioural contract of the LDPC check-node update: [`min_sum_check_row`]
//! matches a scalar reference restated here on every output of a check, and the
//! decoder matches a per-edge reference decoder bit for bit for every algorithm
//! and both early-termination settings.

use gf2_coding::ldpc::{
    min_sum_check_row, DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, MinSumRule,
    QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// The supported scalar reference over an already-excluded input set: the sign
/// taken by comparison against zero, the magnitude taken by the `f32::min` fold
/// from infinity, and the rule's scaling applied to the two.
fn scalar_contract_reduce(rule: MinSumRule, others: &[Llr]) -> Llr {
    if others.is_empty() {
        return Llr::zero();
    }
    let sign: f32 = others
        .iter()
        .map(|llr| if llr.value() >= 0.0 { 1.0 } else { -1.0 })
        .product();
    let magnitude = others
        .iter()
        .map(|llr| llr.value().abs())
        .fold(f32::INFINITY, f32::min);
    Llr::new(match rule {
        MinSumRule::Plain => sign * magnitude,
        MinSumRule::Normalized(alpha) => alpha * (sign * magnitude),
        MinSumRule::Offset(beta) => sign * (magnitude - beta).max(0.0),
    })
}

/// The supported scalar reference for one output of a check.
fn scalar_reference(rule: MinSumRule, inputs: &[Llr], excluded: usize) -> f32 {
    let others: Vec<Llr> = inputs
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != excluded)
        .map(|(_, llr)| *llr)
        .collect();
    scalar_contract_reduce(rule, &others).value()
}

const RULES: [MinSumRule; 4] = [
    MinSumRule::Plain,
    MinSumRule::Normalized(0.75),
    MinSumRule::Normalized(0.875),
    MinSumRule::Offset(0.5),
];

fn assert_row_matches_reference(values: &[f32]) {
    let inputs: Vec<Llr> = values.iter().copied().map(Llr::new).collect();
    for rule in RULES {
        let mut outputs = vec![Llr::new(f32::NAN); inputs.len()];
        min_sum_check_row(rule, &inputs, &mut outputs);
        for (index, output) in outputs.iter().enumerate() {
            let want = scalar_reference(rule, &inputs, index);
            let got = output.value();
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "rule {rule:?}, output {index} of {values:?}: got {got}, want {want}"
            );
        }
    }
}

#[test]
fn reduction_matches_the_scalar_reference_on_ordinary_inputs() {
    assert_row_matches_reference(&[3.0, -2.0, 4.0]);
    assert_row_matches_reference(&[-1.5, -2.5]);
    assert_row_matches_reference(&[0.25, 8.0, -0.5, 7.0, 6.0, -9.0, 1.0]);
    // Seven, eight, nine and seventeen inputs bracket an eight-lane kernel's
    // vector width in both directions.
    assert_row_matches_reference(&[1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0]);
    assert_row_matches_reference(&[1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0, 9.0]);
    let seventeen: Vec<f32> = (1..=17)
        .map(|i| if i % 3 == 0 { -(i as f32) } else { i as f32 })
        .collect();
    assert_row_matches_reference(&seventeen);
}

#[test]
fn reduction_matches_the_scalar_reference_on_ties() {
    assert_row_matches_reference(&[2.0, 2.0, 5.0]);
    assert_row_matches_reference(&[-2.0, 2.0, 5.0]);
    assert_row_matches_reference(&[3.0, 3.0, 3.0, 3.0]);
    assert_row_matches_reference(&[-4.0, 4.0, -4.0, 4.0, 9.0]);
    // A tie at the minimum that straddles the eighth input.
    assert_row_matches_reference(&[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 1.0, 1.0, 9.0]);
}

#[test]
fn reduction_matches_the_scalar_reference_on_signed_zero() {
    assert_row_matches_reference(&[-0.0, 1.0, 2.0]);
    assert_row_matches_reference(&[0.0, -0.0, 2.0]);
    assert_row_matches_reference(&[-0.0, -0.0, -3.0]);
    assert_row_matches_reference(&[1.0, -0.0]);
    // Negative zeros in both a lane position and a remainder position of an
    // eight-lane kernel: the reduction takes the comparison rule at every
    // position, so a negative zero counts as positive wherever it sits.
    assert_row_matches_reference(&[-0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, -0.0]);
    assert_row_matches_reference(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, -0.0, 9.0]);
}

#[test]
fn reduction_matches_the_scalar_reference_on_extrema_and_non_finite() {
    assert_row_matches_reference(&[f32::MAX, f32::MIN, 1.0]);
    assert_row_matches_reference(&[f32::MIN_POSITIVE, -f32::MIN_POSITIVE, 1.0]);
    assert_row_matches_reference(&[f32::INFINITY, 2.0, -1.0]);
    assert_row_matches_reference(&[f32::NEG_INFINITY, f32::INFINITY]);
    assert_row_matches_reference(&[f32::NEG_INFINITY, f32::NEG_INFINITY, 3.0]);
    assert_row_matches_reference(&[f32::NAN, 2.0, -1.0]);
    assert_row_matches_reference(&[f32::NAN, f32::NAN]);
    assert_row_matches_reference(&[f32::NAN, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn reduction_handles_degree_one_and_degree_zero() {
    for rule in RULES {
        let mut outputs = [Llr::new(7.0)];
        min_sum_check_row(rule, &[Llr::new(-3.0)], &mut outputs);
        assert_eq!(
            outputs[0].value().to_bits(),
            0.0f32.to_bits(),
            "a degree-one check has no other input for its one output"
        );
        min_sum_check_row(rule, &[], &mut []);
    }
}

/// Each rule is the crate's public reduction API applied
/// to the excluded input set, on inputs for which every backend of that API
/// agrees: finite, non-zero magnitudes. This grounds the reference above in the
/// shipped operation rather than only in its restatement.
#[test]
fn reduction_matches_the_public_reduction_api_on_ordinary_inputs() {
    let cases: [&[f32]; 4] = [
        &[3.0, -2.0, 4.0],
        &[0.25, 8.0, -0.5, 7.0, 6.0, -9.0, 1.0],
        &[1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0, 9.0],
        &[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 1.0, 1.0, 9.0],
    ];
    for values in cases {
        let inputs: Vec<Llr> = values.iter().copied().map(Llr::new).collect();
        for (rule, apply) in [
            (
                MinSumRule::Plain,
                Box::new(|others: &[Llr]| Llr::boxplus_minsum_n(others))
                    as Box<dyn Fn(&[Llr]) -> Llr>,
            ),
            (
                MinSumRule::Normalized(0.75),
                Box::new(|others: &[Llr]| Llr::boxplus_normalized_minsum_n(others, 0.75)),
            ),
            (
                MinSumRule::Offset(0.5),
                Box::new(|others: &[Llr]| Llr::boxplus_offset_minsum_n(others, 0.5)),
            ),
        ] {
            let mut outputs = vec![Llr::zero(); inputs.len()];
            min_sum_check_row(rule, &inputs, &mut outputs);
            for (index, output) in outputs.iter().enumerate() {
                let others: Vec<Llr> = inputs
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != index)
                    .map(|(_, llr)| *llr)
                    .collect();
                let want = apply(&others);
                assert_eq!(
                    output.value().to_bits(),
                    want.value().to_bits(),
                    "rule {rule:?}, output {index} of {values:?}"
                );
            }
        }
    }
}

/// A per-edge, neighbour-searching flooding decoder: messages in jagged per-node
/// vectors, a linear search of the variable's check list for each gathered
/// input, one leave-one-out reduction per outgoing edge through the crate's
/// public LLR operations, and a full hard-decision word and syndrome per
/// iteration.
struct ReferenceDecoder {
    code: LdpcCode,
    beliefs: Vec<Llr>,
    check_to_var: Vec<Vec<Llr>>,
    var_to_check: Vec<Vec<Llr>>,
    check_neighbors: Vec<Vec<usize>>,
    var_neighbors: Vec<Vec<usize>>,
    temp_inputs: Vec<Llr>,
    config: DecoderConfig,
    reduction: Reduction,
}

/// Which min-sum reduction a reference decoder performs per outgoing edge.
///
/// The two agree on every input a channel produces. They part on a negative
/// zero or a NaN among nine or more inputs, which is where the public API's
/// AVX2 kernel takes the IEEE sign bit and propagates a NaN through its vector
/// lanes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Reduction {
    /// The crate's public LLR operations, which dispatch to a SIMD kernel under
    /// the default `simd` cargo feature.
    PublicApi,
    /// The supported scalar reference, restated by [`scalar_contract_reduce`].
    ScalarContract,
}

#[derive(Debug, PartialEq)]
struct ReferenceOutcome {
    codeword: BitVec,
    iterations: usize,
    converged: bool,
    syndrome_check_passed: bool,
}

impl ReferenceDecoder {
    fn new(code: LdpcCode, config: DecoderConfig, reduction: Reduction) -> Self {
        let n = code.n();
        let m = code.m();
        let h = code.parity_check_matrix();
        let check_neighbors: Vec<Vec<usize>> =
            (0..m).map(|check| h.row_iter(check).collect()).collect();
        let var_neighbors: Vec<Vec<usize>> = (0..n).map(|var| h.col_iter(var).collect()).collect();
        let max_check_degree = check_neighbors.iter().map(Vec::len).max().unwrap_or(0);
        let check_to_var = check_neighbors
            .iter()
            .map(|neighbors| vec![Llr::zero(); neighbors.len()])
            .collect();
        let var_to_check = var_neighbors
            .iter()
            .map(|neighbors| vec![Llr::zero(); neighbors.len()])
            .collect();
        Self {
            code,
            beliefs: vec![Llr::zero(); n],
            check_to_var,
            var_to_check,
            check_neighbors,
            var_neighbors,
            temp_inputs: Vec::with_capacity(max_check_degree),
            config,
            reduction,
        }
    }

    fn find_check_position(&self, var: usize, target_check: usize) -> usize {
        self.var_neighbors[var]
            .iter()
            .position(|&check| check == target_check)
            .expect("check not found in the variable's neighbours")
    }

    fn check_to_var_message(&self, var: usize, var_check_pos: usize) -> Llr {
        let check = self.var_neighbors[var][var_check_pos];
        let check_var_pos = self.check_neighbors[check]
            .iter()
            .position(|&v| v == var)
            .expect("variable not found in the check's neighbours");
        self.check_to_var[check][check_var_pos]
    }

    fn check_node_update(&mut self) {
        for check in 0..self.check_neighbors.len() {
            let degree = self.check_neighbors[check].len();
            for pos in 0..degree {
                self.temp_inputs.clear();
                for other_pos in 0..degree {
                    if other_pos != pos {
                        let other_var = self.check_neighbors[check][other_pos];
                        let var_check_pos = self.find_check_position(other_var, check);
                        self.temp_inputs
                            .push(self.var_to_check[other_var][var_check_pos]);
                    }
                }
                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    match (self.config.algorithm(), self.reduction) {
                        (DecoderAlgorithm::SumProduct, _) => Llr::boxplus_n(&self.temp_inputs),
                        (DecoderAlgorithm::MinSum, Reduction::PublicApi) => {
                            Llr::boxplus_minsum_n(&self.temp_inputs)
                        }
                        (DecoderAlgorithm::NormalizedMinSum(alpha), Reduction::PublicApi) => {
                            Llr::boxplus_normalized_minsum_n(&self.temp_inputs, alpha)
                        }
                        (DecoderAlgorithm::OffsetMinSum(beta), Reduction::PublicApi) => {
                            Llr::boxplus_offset_minsum_n(&self.temp_inputs, beta)
                        }
                        (DecoderAlgorithm::MinSum, Reduction::ScalarContract) => {
                            scalar_contract_reduce(MinSumRule::Plain, &self.temp_inputs)
                        }
                        (DecoderAlgorithm::NormalizedMinSum(alpha), Reduction::ScalarContract) => {
                            scalar_contract_reduce(MinSumRule::Normalized(alpha), &self.temp_inputs)
                        }
                        (DecoderAlgorithm::OffsetMinSum(beta), Reduction::ScalarContract) => {
                            scalar_contract_reduce(MinSumRule::Offset(beta), &self.temp_inputs)
                        }
                    }
                };
                self.check_to_var[check][pos] = message;
            }
        }
    }

    fn variable_node_update(&mut self, channel_llrs: &[Llr]) {
        for (var, &channel_llr) in channel_llrs.iter().enumerate().take(self.code.n()) {
            let degree = self.var_neighbors[var].len();
            let mut belief = channel_llr;
            for pos in 0..degree {
                belief = Llr::new(belief.value() + self.check_to_var_message(var, pos).value());
            }
            self.beliefs[var] = belief;
            for pos in 0..degree {
                let incoming = self.check_to_var_message(var, pos);
                self.var_to_check[var][pos] = Llr::new(belief.value() - incoming.value());
            }
        }
    }

    fn hard_decode(&self) -> BitVec {
        let mut decoded = BitVec::with_capacity(self.code.n());
        for &belief in &self.beliefs {
            decoded.push_bit(belief.hard_decision());
        }
        decoded
    }

    fn decode(&mut self, llrs: &[Llr], max_iterations: usize) -> ReferenceOutcome {
        assert_eq!(llrs.len(), self.code.n());
        for messages in &mut self.check_to_var {
            for message in messages.iter_mut() {
                *message = Llr::zero();
            }
        }
        for (var, &llr) in llrs.iter().enumerate().take(self.code.n()) {
            for pos in 0..self.var_to_check[var].len() {
                self.var_to_check[var][pos] = llr;
            }
        }

        let mut iterations = 0;
        let mut converged = false;
        let early_termination = self.config.early_termination();
        for iter in 0..max_iterations {
            iterations = iter + 1;
            self.check_node_update();
            self.variable_node_update(llrs);
            if early_termination {
                let decoded = self.hard_decode();
                if self.code.is_valid_codeword(&decoded) {
                    converged = true;
                    break;
                }
            }
        }

        let codeword = self.hard_decode();
        let syndrome_check_passed = self.code.is_valid_codeword(&codeword);
        if !early_termination {
            converged = syndrome_check_passed;
        }
        ReferenceOutcome {
            codeword,
            iterations,
            converged,
            syndrome_check_passed,
        }
    }
}

/// Channel LLRs for a code: a seeded BPSK-like sign pattern with magnitudes
/// spread over several decades, so the reduction meets ties and both signs.
fn channel_llrs(n: usize, seed: u64) -> Vec<Llr> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|_| {
            let magnitude: f32 = rng.gen_range(0.05f32..6.0);
            let sign = if rng.gen_bool(0.35) { -1.0 } else { 1.0 };
            Llr::new(sign * magnitude)
        })
        .collect()
}

fn assert_decoder_matches_reference(code: &LdpcCode, label: &str, seeds: &[u64], cap: usize) {
    for algorithm in [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
        DecoderAlgorithm::SumProduct,
    ] {
        for early_termination in [true, false] {
            let config = DecoderConfig::new(algorithm, early_termination);
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            let mut reference = ReferenceDecoder::new(code.clone(), config, Reduction::PublicApi);
            let mut codeword = BitVec::new();
            for &seed in seeds {
                let llrs = channel_llrs(code.n(), seed);
                let want = reference.decode(&llrs, cap);
                let got = decoder.decode_codeword_into(&llrs, cap, &mut codeword);
                let context = format!(
                    "{label}, {algorithm:?}, early_termination={early_termination}, seed={seed}"
                );
                assert_eq!(codeword, want.codeword, "codeword: {context}");
                assert_eq!(got.iterations, want.iterations, "iterations: {context}");
                assert_eq!(got.converged, want.converged, "converged: {context}");
                assert_eq!(
                    got.syndrome_check_passed, want.syndrome_check_passed,
                    "syndrome: {context}"
                );
            }
        }
    }
}

#[test]
fn decoder_matches_the_reference_on_a_small_irregular_code() {
    // Check 2 has degree one, so its single output takes the degree-one case.
    let code = LdpcCode::from_edges(
        3,
        6,
        &[
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 1),
            (1, 3),
            (1, 4),
            (1, 5),
            (2, 5),
        ],
    );
    assert_decoder_matches_reference(&code, "irregular-3x6", &[1, 2, 3, 4, 5], 20);
}

#[test]
fn decoder_matches_the_reference_on_dvb_t2_short() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    assert_decoder_matches_reference(&code, "dvb-t2-short-r12", &[11, 12], 12);
}

#[test]
fn decoder_matches_the_reference_on_nr_base_graph_one() {
    // Base graph 1 has degree-19 checks, which is where an eight-lane kernel's
    // vector path engages and where the canonical reduction has to keep the
    // scalar contract.
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 8));
    assert_decoder_matches_reference(&code, "nr-bg1-z8", &[21, 22, 23], 20);
}

/// Codeword-position patterns that drive the reduction into clipping, zero
/// magnitudes, repeated minima and, through the belief sum, non-finite
/// messages.
fn extreme_patterns(n: usize) -> Vec<Vec<Llr>> {
    vec![
        vec![Llr::new(f32::MAX); n],
        (0..n)
            .map(|i| Llr::new(if i % 2 == 0 { 0.0 } else { -f32::MAX }))
            .collect(),
        (0..n)
            .map(|i| match i % 4 {
                0 => Llr::new(0.0),
                1 => Llr::new(f32::MIN_POSITIVE),
                2 => Llr::new(-f32::MIN_POSITIVE),
                _ => Llr::new(1.0),
            })
            .collect(),
    ]
}

#[test]
fn decoder_matches_the_scalar_contract_on_saturating_and_extreme_llrs() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 8));
    for algorithm in [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ] {
        let config = DecoderConfig::new(algorithm, true);
        let mut decoder = LdpcDecoder::with_config(code.clone(), config);
        let mut reference = ReferenceDecoder::new(code.clone(), config, Reduction::ScalarContract);
        let mut codeword = BitVec::new();
        for (index, llrs) in extreme_patterns(code.n()).iter().enumerate() {
            let want = reference.decode(llrs, 8);
            let got = decoder.decode_codeword_into(llrs, 8, &mut codeword);
            let context = format!("{algorithm:?}, pattern {index}");
            assert_eq!(codeword, want.codeword, "codeword: {context}");
            assert_eq!(got.iterations, want.iterations, "iterations: {context}");
            assert_eq!(got.converged, want.converged, "converged: {context}");
            assert_eq!(
                got.syndrome_check_passed, want.syndrome_check_passed,
                "syndrome: {context}"
            );
        }
    }
}

/// On a code with checks of degree nine or more, an input set that reaches a
/// NaN message decodes differently under the public reduction API, whose AVX2
/// kernel propagates a NaN through its vector lanes, than under the supported
/// scalar reference, whose `f32::min` fold skips it. The decoder follows the
/// scalar reference.
#[test]
fn the_public_reduction_api_and_the_scalar_contract_part_on_non_finite_messages() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 8));
    assert!(
        (0..code.m())
            .map(|check| code.parity_check_matrix().row_iter(check).count())
            .any(|degree| degree >= 9),
        "the divergence needs a check whose excluded input set fills a vector lane"
    );
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let mut public = ReferenceDecoder::new(code.clone(), config, Reduction::PublicApi);
    let mut scalar = ReferenceDecoder::new(code.clone(), config, Reduction::ScalarContract);

    // The second pattern's belief sums overflow to infinities of both signs, so
    // a variable-to-check message becomes a NaN.
    let llrs = &extreme_patterns(code.n())[1];
    let from_public = public.decode(llrs, 8);
    let from_scalar = scalar.decode(llrs, 8);
    assert_ne!(
        from_public.codeword, from_scalar.codeword,
        "if the two reductions agree here, the disclosed change no longer exists"
    );

    let mut decoder = LdpcDecoder::with_config(code, config);
    let mut codeword = BitVec::new();
    decoder.decode_codeword_into(llrs, 8, &mut codeword);
    assert_eq!(
        codeword, from_scalar.codeword,
        "the decoder follows the supported scalar reference"
    );
}

#[test]
fn owning_entry_points_agree_with_the_prepared_buffer_entry_point() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let llrs = channel_llrs(code.n(), 7);

    let mut into_decoder = LdpcDecoder::with_config(code.clone(), config);
    let mut codeword = BitVec::new();
    let outcome = into_decoder.decode_codeword_into(&llrs, 25, &mut codeword);

    let mut owning_decoder = LdpcDecoder::with_config(code.clone(), config);
    let owned = owning_decoder.decode_to_codeword(&llrs, 25);
    assert_eq!(owned.decoded_bits, codeword);
    assert_eq!(owned.iterations, outcome.iterations);
    assert_eq!(owned.converged, outcome.converged);
    assert_eq!(owned.syndrome_check_passed, outcome.syndrome_check_passed);

    let mut iterative_decoder = LdpcDecoder::with_config(code, config);
    let iterative = iterative_decoder.decode_iterative(&llrs, 25);
    assert_eq!(iterative.iterations, outcome.iterations);
    assert_eq!(iterative.converged, outcome.converged);
    assert_eq!(
        iterative.syndrome_check_passed,
        outcome.syndrome_check_passed
    );
}
