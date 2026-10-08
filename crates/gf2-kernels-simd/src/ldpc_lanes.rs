//! Inter-frame kernels of the flooding min-sum LDPC decoder: eight
//! independent frames sit in the eight `f32` lanes of one [`F32x8`], and a
//! step updates every lane by the per-frame rules of
//! `docs/reference/ldpc-decoder-numerical-contract.md`. [`LdpcLaneFns`]
//! bundles the check and variable steps; [`LdpcLaneFns::portable`] is the
//! safe scalar reference, available on every host, and [`detect`] publishes
//! the AVX2 bundle on a host with AVX2.

#![forbid(unsafe_code)]

/// Frames one lane vector carries.
pub const LANES: usize = 8;

/// One value of each of [`LANES`] frames, laid out as one 256-bit register.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C, align(32))]
pub struct F32x8(pub [f32; LANES]);

impl F32x8 {
    /// The zero LLR in every lane.
    pub const ZERO: Self = Self([0.0; LANES]);
}

/// The lanes a step writes: bit `l` set selects lane `l`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneMask(u8);

impl LaneMask {
    /// No lane.
    pub const NONE: Self = Self(0);
    /// Every lane.
    pub const ALL: Self = Self(u8::MAX);

    /// The mask whose lane `l` is bit `l` of `bits`.
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Bit `l` is set when lane `l` is selected.
    #[inline]
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// A check-node update rule of the min-sum family, applied to each lane.
///
/// The kernels apply the parameter as given; the domains of the numerical
/// contract (`alpha` finite in $(0, 1]$, `beta` finite and nonnegative) are
/// the caller's to hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaneMinSumRule {
    /// The excluded minimum, signed.
    Plain,
    /// The signed excluded minimum scaled by `alpha`.
    Normalized(f32),
    /// The excluded minimum reduced by `beta`, floored at zero, then signed.
    Offset(f32),
}

impl LaneMinSumRule {
    #[inline]
    fn apply(self, sign: f32, magnitude: f32) -> f32 {
        match self {
            Self::Plain => sign * magnitude,
            Self::Normalized(alpha) => alpha * (sign * magnitude),
            Self::Offset(beta) => sign * (magnitude - beta).max(0.0),
        }
    }
}

/// Check step over `(rule, check_offsets, var_to_check, check_to_var)`.
///
/// Check `c` owns the edges `check_offsets[c]..check_offsets[c + 1]`. In each
/// lane, the output on an edge is the rule applied to the product of the
/// signs and the minimum of the magnitudes of the check's other inputs: a
/// sign is the comparison `>= 0.0`, so a negative zero counts as positive and
/// a NaN as negative, and a NaN magnitude enters no minimum. The single
/// output of a degree-one check is the zero LLR, and a degree-zero check
/// writes nothing. Every lane is written.
///
/// Linear in the edge count: each input is read twice.
///
/// # Panics
///
/// Panics before any store unless `check_offsets` is nonempty, starts at
/// zero, never decreases and ends at the edge count, which is the length of
/// both `var_to_check` and `check_to_var`.
pub type LaneCheckStepFn = fn(LaneMinSumRule, &[u32], &[F32x8], &mut [F32x8]);

/// Variable step over `(active, var_offsets, var_edge_to_check_edge,
/// channel, check_to_var, beliefs, var_to_check, hard)`.
///
/// Variable `v` owns the slots `var_offsets[v]..var_offsets[v + 1]`, and
/// `var_edge_to_check_edge[slot]` is the edge of a slot. In each lane of
/// `active`, the belief of `v` is the sequential sum that starts from
/// `channel[v]` and adds `check_to_var` over the slots of `v` in order; the
/// outgoing message on an edge is that belief minus the edge's incoming
/// message; and bit `l` of `hard[v]` is 1 when the belief of lane `l` is
/// `< 0.0`. The belief, the outgoing messages and the hard-decision bit of a
/// lane outside `active` keep their values.
///
/// Linear in the edge count: each incoming message is read twice.
///
/// # Panics
///
/// Panics before any store unless `var_offsets` is nonempty, starts at zero,
/// never decreases and ends at the edge count, which is the length of
/// `var_edge_to_check_edge`, `check_to_var` and `var_to_check`, and unless
/// `channel`, `beliefs` and `hard` hold one entry per variable. Panics, with
/// earlier variables already written, when a slot names an edge at or beyond
/// the edge count.
pub type LaneVariableStepFn =
    fn(LaneMask, &[u32], &[u32], &[F32x8], &[F32x8], &mut [F32x8], &mut [F32x8], &mut [u8]);

/// The lane kernels of one backend.
///
/// The variable-to-check and check-to-variable messages are indexed by
/// canonical edge id and then by lane, channel LLRs and beliefs by variable
/// and then by lane. Hard decisions are one byte per variable whose bit `l`
/// is the decision of lane `l`. The layout slices are the check and variable
/// prefix offsets and the slot-to-edge map of the canonical edge layout
/// (`gf2_coding::ldpc::EdgeLayout`).
#[derive(Copy, Clone)]
pub struct LdpcLaneFns {
    /// Check step of the plain, normalized and offset min-sum rules.
    pub check_step_fn: LaneCheckStepFn,
    /// Variable step with hard decisions, restricted to the active lanes.
    pub variable_step_fn: LaneVariableStepFn,
}

impl LdpcLaneFns {
    /// The safe scalar bundle, which defines the bit patterns every backend
    /// reproduces.
    #[must_use]
    pub const fn portable() -> Self {
        Self {
            check_step_fn: lane_check_step_portable,
            variable_step_fn: lane_variable_step_portable,
        }
    }
}

/// The AVX2 bundle, bit-identical to [`LdpcLaneFns::portable`].
///
/// Returns `None` on a non-x86 target and on a host without AVX2.
#[must_use]
pub fn detect() -> Option<LdpcLaneFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_lanes::detect();
    }
    #[allow(unreachable_code)]
    None
}

/// Requires `offsets` to be the prefix offsets of `total` entries.
fn assert_prefix_offsets(kernel: &str, name: &str, offsets: &[u32], total: usize) {
    assert_eq!(
        offsets.first(),
        Some(&0),
        "{kernel}: {name} must start at zero"
    );
    assert!(
        offsets.windows(2).all(|pair| pair[0] <= pair[1]),
        "{kernel}: {name} must not decrease"
    );
    assert_eq!(
        offsets.last().map(|&end| end as usize),
        Some(total),
        "{kernel}: {name} must end at the edge count"
    );
}

/// The argument check every check step runs before its first store.
///
/// # Panics
///
/// Panics, with a message opening with `kernel`, as [`LaneCheckStepFn`]
/// states.
pub fn assert_check_step_lengths(
    kernel: &str,
    check_offsets: &[u32],
    var_to_check_len: usize,
    check_to_var_len: usize,
) {
    assert_eq!(
        var_to_check_len, check_to_var_len,
        "{kernel}: the two message arrays must hold the same edges"
    );
    assert_prefix_offsets(kernel, "check_offsets", check_offsets, check_to_var_len);
}

/// The argument check every variable step runs before its first store.
///
/// On return every slot range of `var_offsets` lies inside a slot map of
/// `slots_len` entries. The edge a slot names is left to the step.
///
/// # Panics
///
/// Panics, with a message opening with `kernel`, on the length relations
/// [`LaneVariableStepFn`] states.
#[allow(clippy::too_many_arguments)]
pub fn assert_variable_step_lengths(
    kernel: &str,
    var_offsets: &[u32],
    slots_len: usize,
    channel_len: usize,
    check_to_var_len: usize,
    beliefs_len: usize,
    var_to_check_len: usize,
    hard_len: usize,
) {
    assert_eq!(
        var_to_check_len, check_to_var_len,
        "{kernel}: the two message arrays must hold the same edges"
    );
    assert_eq!(
        slots_len, check_to_var_len,
        "{kernel}: var_edge_to_check_edge must hold one slot per edge"
    );
    assert_prefix_offsets(kernel, "var_offsets", var_offsets, slots_len);
    let variables = var_offsets.len() - 1;
    assert_eq!(
        channel_len, variables,
        "{kernel}: channel must hold one entry per variable"
    );
    assert_eq!(
        beliefs_len, variables,
        "{kernel}: beliefs must hold one entry per variable"
    );
    assert_eq!(
        hard_len, variables,
        "{kernel}: hard must hold one entry per variable"
    );
}

/// Portable [`LaneCheckStepFn`].
///
/// # Panics
///
/// As [`LaneCheckStepFn`].
pub fn lane_check_step_portable(
    rule: LaneMinSumRule,
    check_offsets: &[u32],
    var_to_check: &[F32x8],
    check_to_var: &mut [F32x8],
) {
    assert_check_step_lengths(
        "lane_check_step_portable",
        check_offsets,
        var_to_check.len(),
        check_to_var.len(),
    );
    for bounds in check_offsets.windows(2) {
        let range = bounds[0] as usize..bounds[1] as usize;
        check_row(rule, &var_to_check[range.clone()], &mut check_to_var[range]);
    }
}

/// One check: the two smallest magnitudes, the position of the smallest and
/// the sign product of each lane, then each output from the minimum that
/// excludes its own input and the sign product with its own sign removed.
#[inline]
fn check_row(rule: LaneMinSumRule, inputs: &[F32x8], outputs: &mut [F32x8]) {
    match inputs.len() {
        0 => return,
        1 => {
            outputs[0] = F32x8::ZERO;
            return;
        }
        _ => {}
    }

    let mut min1 = [f32::INFINITY; LANES];
    let mut min2 = [f32::INFINITY; LANES];
    let mut position = [u32::MAX; LANES];
    let mut sign_product = [1.0f32; LANES];
    for (index, input) in inputs.iter().enumerate() {
        for lane in 0..LANES {
            let value = input.0[lane];
            sign_product[lane] = if value >= 0.0 {
                sign_product[lane]
            } else {
                -sign_product[lane]
            };
            let magnitude = value.abs();
            if magnitude < min1[lane] {
                min2[lane] = min1[lane];
                min1[lane] = magnitude;
                position[lane] = index as u32;
            } else if magnitude < min2[lane] {
                min2[lane] = magnitude;
            }
        }
    }

    for (index, (input, output)) in inputs.iter().zip(outputs.iter_mut()).enumerate() {
        for lane in 0..LANES {
            let sign = if input.0[lane] >= 0.0 {
                sign_product[lane]
            } else {
                -sign_product[lane]
            };
            let magnitude = if position[lane] == index as u32 {
                min2[lane]
            } else {
                min1[lane]
            };
            output.0[lane] = rule.apply(sign, magnitude);
        }
    }
}

/// Portable [`LaneVariableStepFn`].
///
/// # Panics
///
/// As [`LaneVariableStepFn`].
#[allow(clippy::too_many_arguments)]
pub fn lane_variable_step_portable(
    active: LaneMask,
    var_offsets: &[u32],
    var_edge_to_check_edge: &[u32],
    channel: &[F32x8],
    check_to_var: &[F32x8],
    beliefs: &mut [F32x8],
    var_to_check: &mut [F32x8],
    hard: &mut [u8],
) {
    assert_variable_step_lengths(
        "lane_variable_step_portable",
        var_offsets,
        var_edge_to_check_edge.len(),
        channel.len(),
        check_to_var.len(),
        beliefs.len(),
        var_to_check.len(),
        hard.len(),
    );
    let active = active.bits();
    for (var, bounds) in var_offsets.windows(2).enumerate() {
        let slots = &var_edge_to_check_edge[bounds[0] as usize..bounds[1] as usize];

        let mut belief = channel[var];
        for &edge in slots {
            let incoming = &check_to_var[edge as usize];
            for lane in 0..LANES {
                belief.0[lane] += incoming.0[lane];
            }
        }

        let mut decided = 0u8;
        for lane in 0..LANES {
            decided |= u8::from(belief.0[lane] < 0.0) << lane;
            if active >> lane & 1 == 1 {
                beliefs[var].0[lane] = belief.0[lane];
            }
        }
        hard[var] = hard[var] & !active | decided & active;

        for &edge in slots {
            let incoming = &check_to_var[edge as usize];
            let outgoing = &mut var_to_check[edge as usize];
            for lane in 0..LANES {
                if active >> lane & 1 == 1 {
                    outgoing.0[lane] = belief.0[lane] - incoming.0[lane];
                }
            }
        }
    }
}

const _: LaneCheckStepFn = lane_check_step_portable;
const _: LaneVariableStepFn = lane_variable_step_portable;

/// The behavioural contract of a lane kernel bundle, as one function any
/// backend is handed.
///
/// This module's own tests run [`contract::assert_lane_contract`] over
/// [`LdpcLaneFns::portable`].
#[cfg(any(test, feature = "test-support"))]
pub mod contract {
    use super::{F32x8, LaneMask, LaneMinSumRule, LdpcLaneFns, LANES};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const RULES: [LaneMinSumRule; 4] = [
        LaneMinSumRule::Plain,
        LaneMinSumRule::Normalized(0.75),
        LaneMinSumRule::Offset(0.5),
        LaneMinSumRule::Offset(0.0),
    ];

    /// Values a hashed case draws from: both zeros, NaN of either sign, both
    /// infinities, finite extrema, subnormals, repeated magnitudes that tie,
    /// magnitudes on either side of the offset `0.5`, and a pair whose sum
    /// with `1.0` depends on the order of accumulation.
    const POOL: [f32; 26] = [
        0.0,
        -0.0,
        f32::NAN,
        -f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        f32::MIN,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        1.0e-45,
        -1.0e-45,
        1.0,
        -1.0,
        1.0,
        0.5,
        -0.5,
        0.25,
        0.75,
        -2.0,
        2.0,
        3.5,
        -7.25,
        16_777_216.0,
        -16_777_216.0,
        -1.0,
    ];

    /// Check rows by degree. Lane `l` reads a row rotated by `l` places, and
    /// negated from lane four up, so a tie, a zero and a NaN sit at a
    /// different position and under a different sign in each lane.
    const CHECK_ROWS: [&[f32]; 27] = [
        &[],
        &[-3.0],
        &[f32::NAN],
        &[1.5, -2.5],
        &[2.0, 2.0],
        &[-0.0, 0.0],
        &[-0.0, 1.0],
        &[f32::NAN, f32::NAN],
        &[f32::NAN, 2.0],
        &[f32::INFINITY, f32::NEG_INFINITY],
        &[f32::MAX, f32::MIN_POSITIVE],
        &[],
        &[3.0, -2.0, 4.0],
        &[2.0, 2.0, 5.0],
        &[-2.0, 2.0, 5.0],
        &[5.0, 2.0, -2.0, 2.0],
        &[3.0, 3.0, 3.0, 3.0],
        &[-0.0, 1.0, 2.0],
        &[0.0, -0.0, 2.0],
        &[-0.0, -0.0, -3.0],
        &[f32::MAX, f32::MIN, 1.0],
        &[f32::MIN_POSITIVE, -f32::MIN_POSITIVE, 1.0],
        &[f32::INFINITY, 2.0, -1.0],
        &[f32::NAN, 2.0, -1.0],
        &[f32::NAN, f32::NAN, -f32::NAN],
        &[f32::NAN, f32::NAN, 4.0, -5.0],
        &[0.5, 0.75, 0.25, -0.5],
    ];

    /// Degrees of the checks whose inputs are hashed from [`POOL`]
    /// independently per lane.
    const HASHED_DEGREES: [usize; 4] = [9, 19, 20, 33];

    /// SplitMix64 finalizer (`@/citation/Steele2014`) of a case index.
    fn mix(index: u64) -> u64 {
        let mut z = index.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pooled(index: u64) -> f32 {
        POOL[(mix(index) % POOL.len() as u64) as usize]
    }

    /// The canonical output of one edge of one frame, stated edge by edge:
    /// the rule applied to the sign product and the minimum magnitude of the
    /// row's other inputs.
    fn excluded_reduction(rule: LaneMinSumRule, row: &[f32], excluded: usize) -> f32 {
        if row.len() == 1 {
            return 0.0;
        }
        let others = || {
            row.iter()
                .enumerate()
                .filter(move |(index, _)| *index != excluded)
                .map(|(_, &value)| value)
        };
        let sign: f32 = others()
            .map(|value| if value >= 0.0 { 1.0 } else { -1.0 })
            .product();
        let magnitude = others().map(f32::abs).fold(f32::INFINITY, f32::min);
        match rule {
            LaneMinSumRule::Plain => sign * magnitude,
            LaneMinSumRule::Normalized(alpha) => alpha * (sign * magnitude),
            LaneMinSumRule::Offset(beta) => sign * (magnitude - beta).max(0.0),
        }
    }

    /// Requires `got` and `want` to agree in every bit of every lane.
    fn assert_same_bits(label: &str, array: &str, got: &[F32x8], want: &[F32x8]) {
        assert_eq!(got.len(), want.len(), "{label}: {array} length");
        for (index, (got, want)) in got.iter().zip(want).enumerate() {
            for lane in 0..LANES {
                assert_eq!(
                    got.0[lane].to_bits(),
                    want.0[lane].to_bits(),
                    "{label}: {array}[{index}] lane {lane}: got {:?}, want {:?}",
                    got.0[lane],
                    want.0[lane]
                );
            }
        }
    }

    /// Requires `call` to panic.
    fn assert_rejects(label: &str, case: &str, call: impl FnOnce()) {
        assert!(
            catch_unwind(AssertUnwindSafe(call)).is_err(),
            "{label}: accepted {case}"
        );
    }

    /// The check offsets and variable-to-check messages of the check cases.
    fn check_case() -> (Vec<u32>, Vec<F32x8>) {
        let mut offsets = vec![0u32];
        let mut inputs = Vec::new();
        for row in CHECK_ROWS {
            for index in 0..row.len() {
                let mut edge = F32x8::ZERO;
                for lane in 0..LANES {
                    let value = row[(index + lane) % row.len()];
                    edge.0[lane] = if lane < LANES / 2 { value } else { -value };
                }
                inputs.push(edge);
            }
            offsets.push(inputs.len() as u32);
        }
        for degree in HASHED_DEGREES {
            for _ in 0..degree {
                let base = (inputs.len() * LANES) as u64;
                let mut edge = F32x8::ZERO;
                for lane in 0..LANES {
                    edge.0[lane] = pooled(base + lane as u64);
                }
                inputs.push(edge);
            }
            offsets.push(inputs.len() as u32);
        }
        (offsets, inputs)
    }

    fn assert_check_step(label: &str, fns: &LdpcLaneFns) {
        const UNWRITTEN: F32x8 = F32x8([99.0; LANES]);
        let (offsets, inputs) = check_case();
        let edges = inputs.len();

        for rule in RULES {
            let mut want = vec![UNWRITTEN; edges];
            for bounds in offsets.windows(2) {
                let range = bounds[0] as usize..bounds[1] as usize;
                for lane in 0..LANES {
                    let row: Vec<f32> = inputs[range.clone()]
                        .iter()
                        .map(|edge| edge.0[lane])
                        .collect();
                    for position in 0..row.len() {
                        want[range.start + position].0[lane] =
                            excluded_reduction(rule, &row, position);
                    }
                }
            }
            let mut got = vec![UNWRITTEN; edges];
            (fns.check_step_fn)(rule, &offsets, &inputs, &mut got);
            assert_same_bits(label, &format!("check_to_var under {rule:?}"), &got, &want);
        }

        let rejections: [(&str, &[u32], usize, usize); 6] = [
            ("empty check offsets", &[], 0, 0),
            ("check offsets starting above zero", &[1, 3], 3, 3),
            ("decreasing check offsets", &[0, 2, 1, 3], 3, 3),
            ("check offsets ending below the edge count", &[0, 2], 3, 3),
            ("check offsets ending above the edge count", &[0, 4], 3, 3),
            ("message arrays of different lengths", &[0, 3], 3, 2),
        ];
        for (case, offsets, inputs_len, outputs_len) in rejections {
            let inputs = vec![F32x8([1.0; LANES]); inputs_len];
            let mut outputs = vec![UNWRITTEN; outputs_len];
            assert_rejects(label, case, || {
                (fns.check_step_fn)(LaneMinSumRule::Plain, offsets, &inputs, &mut outputs);
            });
            assert_same_bits(label, case, &outputs, &vec![UNWRITTEN; outputs_len]);
        }
    }

    /// Every array a variable step reads or writes.
    #[derive(Clone)]
    struct VariableCase {
        var_offsets: Vec<u32>,
        slots: Vec<u32>,
        channel: Vec<F32x8>,
        check_to_var: Vec<F32x8>,
        beliefs: Vec<F32x8>,
        var_to_check: Vec<F32x8>,
        hard: Vec<u8>,
    }

    impl VariableCase {
        fn step(&mut self, fns: &LdpcLaneFns, active: LaneMask) {
            (fns.variable_step_fn)(
                active,
                &self.var_offsets,
                &self.slots,
                &self.channel,
                &self.check_to_var,
                &mut self.beliefs,
                &mut self.var_to_check,
                &mut self.hard,
            );
        }

        fn assert_outputs(&self, label: &str, case: &str, want: &Self) {
            assert_same_bits(
                label,
                &format!("beliefs, {case}"),
                &self.beliefs,
                &want.beliefs,
            );
            assert_same_bits(
                label,
                &format!("var_to_check, {case}"),
                &self.var_to_check,
                &want.var_to_check,
            );
            assert_eq!(self.hard, want.hard, "{label}: hard, {case}");
        }
    }

    /// Variables of degree 0, 1, 2, 3 and 7 over eight checks, with prior
    /// outputs every lane can be told apart from.
    ///
    /// Variable 4 carries the designed lanes of its `columns`; every other
    /// value is hashed from [`POOL`]. A NaN that would meet a NaN belief is
    /// replaced, so no sum has two NaN operands and its payload does not
    /// depend on operand order.
    fn variable_case() -> VariableCase {
        const VARIABLES: usize = 7;
        let checks: [&[u32]; 8] = [
            &[1, 2, 3, 4],
            &[2, 3, 4],
            &[],
            &[3, 4, 5],
            &[4],
            &[4, 5],
            &[4],
            &[4, 6],
        ];
        let edge_var: Vec<u32> = checks
            .iter()
            .flat_map(|check| check.iter().copied())
            .collect();
        let edges = edge_var.len();
        let mut var_offsets = vec![0u32];
        let mut slots = Vec::new();
        for var in 0..VARIABLES as u32 {
            slots.extend((0..edges as u32).filter(|&edge| edge_var[edge as usize] == var));
            var_offsets.push(slots.len() as u32);
        }

        // Channel LLR, then the seven incoming messages in slot order.
        let columns: [[f32; 8]; 7] = [
            [
                16_777_216.0,
                1.0,
                1.0,
                -16_777_216.0,
                0.5,
                -0.5,
                0.25,
                -0.25,
            ],
            [-0.0; 8],
            [f32::NAN, 1.0, -2.0, 0.0, -0.0, f32::MAX, f32::MIN, 3.0],
            [-f32::NAN, 1.0, -2.0, 0.0, -0.0, f32::INFINITY, 2.0, 3.0],
            [
                f32::INFINITY,
                1.0,
                f32::NEG_INFINITY,
                2.0,
                -1.0,
                0.5,
                0.25,
                4.0,
            ],
            [f32::MAX, f32::MAX, -1.0, 0.0, 2.0, -3.0, 0.5, 1.0],
            [-1.0, 1.0, -0.0, 0.0, -0.0, 0.0, -0.0, 0.0],
        ];

        let mut channel = vec![F32x8::ZERO; VARIABLES];
        let mut check_to_var = vec![F32x8::ZERO; edges];
        for var in 0..VARIABLES {
            let range = var_offsets[var] as usize..var_offsets[var + 1] as usize;
            for lane in 0..LANES {
                let designed = (var == 4 && lane < columns.len()).then(|| &columns[lane]);
                let mut belief = match designed {
                    Some(column) => column[0],
                    None => pooled((1 << 20) + (var * LANES + lane) as u64),
                };
                channel[var].0[lane] = belief;
                for (place, &edge) in slots[range.clone()].iter().enumerate() {
                    let mut incoming = match designed {
                        Some(column) => column[1 + place],
                        None => pooled((2 << 20) + (edge as usize * LANES + lane) as u64),
                    };
                    if belief.is_nan() && incoming.is_nan() {
                        incoming = 1.0;
                    }
                    check_to_var[edge as usize].0[lane] = incoming;
                    belief += incoming;
                }
            }
        }

        let prior = |seed: u64, len: usize| -> Vec<F32x8> {
            (0..len)
                .map(|index| {
                    let mut lanes = F32x8::ZERO;
                    for lane in 0..LANES {
                        lanes.0[lane] = pooled((seed << 20) + (index * LANES + lane) as u64);
                    }
                    lanes
                })
                .collect()
        };
        VariableCase {
            var_offsets,
            slots,
            channel,
            check_to_var,
            beliefs: prior(3, VARIABLES),
            var_to_check: prior(4, edges),
            hard: (0..VARIABLES as u64)
                .map(|var| mix((5 << 20) + var) as u8)
                .collect(),
        }
    }

    /// The canonical variable update of every frame of `case`: the state
    /// after a step with every lane active.
    fn canonical_variable_update(case: &VariableCase) -> VariableCase {
        let mut updated = case.clone();
        for var in 0..case.channel.len() {
            let range = case.var_offsets[var] as usize..case.var_offsets[var + 1] as usize;
            updated.hard[var] = 0;
            for lane in 0..LANES {
                let mut belief = case.channel[var].0[lane];
                for &edge in &case.slots[range.clone()] {
                    belief += case.check_to_var[edge as usize].0[lane];
                }
                updated.beliefs[var].0[lane] = belief;
                for &edge in &case.slots[range.clone()] {
                    updated.var_to_check[edge as usize].0[lane] =
                        belief - case.check_to_var[edge as usize].0[lane];
                }
                updated.hard[var] |= u8::from(belief < 0.0) << lane;
            }
        }
        updated
    }

    fn assert_variable_step(label: &str, fns: &LdpcLaneFns) {
        let prior = variable_case();
        let updated = canonical_variable_update(&prior);

        for bits in 0..=u8::MAX {
            let lane_of = |from: &F32x8, to: &F32x8| {
                let mut lanes = *from;
                for lane in 0..LANES {
                    if bits >> lane & 1 == 1 {
                        lanes.0[lane] = to.0[lane];
                    }
                }
                lanes
            };
            let mut want = prior.clone();
            for (want, to) in want.beliefs.iter_mut().zip(&updated.beliefs) {
                *want = lane_of(want, to);
            }
            for (want, to) in want.var_to_check.iter_mut().zip(&updated.var_to_check) {
                *want = lane_of(want, to);
            }
            for (want, to) in want.hard.iter_mut().zip(&updated.hard) {
                *want = *want & !bits | to & bits;
            }

            let mut got = prior.clone();
            got.step(fns, LaneMask::from_bits(bits));
            got.assert_outputs(label, &format!("active lanes {bits:#010b}"), &want);
        }

        type Damage = fn(&mut VariableCase);
        let rejections: [(&str, Damage); 10] = [
            ("empty variable offsets", |case| case.var_offsets.clear()),
            ("variable offsets starting above zero", |case| {
                case.var_offsets[0] = 1;
            }),
            ("decreasing variable offsets", |case| {
                case.var_offsets.swap(2, 3);
            }),
            ("variable offsets ending below the edge count", |case| {
                *case.var_offsets.last_mut().expect("offsets") -= 1;
            }),
            ("a slot map shorter than the edge count", |case| {
                case.slots.pop();
            }),
            ("message arrays of different lengths", |case| {
                case.var_to_check.pop();
            }),
            ("a short channel array", |case| {
                case.channel.pop();
            }),
            ("a short belief array", |case| {
                case.beliefs.pop();
            }),
            ("a long belief array", |case| {
                case.beliefs.push(F32x8::ZERO);
            }),
            ("a short hard-decision array", |case| {
                case.hard.pop();
            }),
        ];
        for (case, damage) in rejections {
            let mut damaged = prior.clone();
            damage(&mut damaged);
            let want = damaged.clone();
            assert_rejects(label, case, || damaged.step(fns, LaneMask::ALL));
            damaged.assert_outputs(label, case, &want);
        }

        let mut stray = prior.clone();
        let edges = stray.check_to_var.len() as u32;
        *stray.slots.last_mut().expect("slots") = edges;
        assert_rejects(label, "a slot naming an edge beyond the edge count", || {
            stray.step(fns, LaneMask::ALL);
        });
    }

    /// One check per row of `rows`, every lane of an edge holding the row's
    /// value, stepped under the plain rule.
    fn uniform_check_outputs(fns: &LdpcLaneFns, rows: &[&[f32]]) -> Vec<F32x8> {
        let mut offsets = vec![0u32];
        let mut inputs = Vec::new();
        for row in rows {
            inputs.extend(row.iter().map(|&value| F32x8([value; LANES])));
            offsets.push(inputs.len() as u32);
        }
        let mut outputs = vec![F32x8([99.0; LANES]); inputs.len()];
        (fns.check_step_fn)(LaneMinSumRule::Plain, &offsets, &inputs, &mut outputs);
        outputs
    }

    /// Asserts that a negative-zero message counts as positive in every lane.
    ///
    /// A lane that takes the IEEE sign bit of `-0.0` negates the outputs of
    /// the check's other edges.
    ///
    /// # Panics
    ///
    /// Panics naming `label` when an output differs from the comparison rule.
    pub fn assert_negative_zero_counts_positive(label: &str, fns: &LdpcLaneFns) {
        let rows: [&[f32]; 3] = [&[-0.0, 2.0, 3.0], &[2.0, -0.0, 3.0], &[2.0, 3.0, -0.0]];
        let want: Vec<F32x8> = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0]
            .map(|value| F32x8([value; LANES]))
            .to_vec();
        let got = uniform_check_outputs(fns, &rows);
        assert_same_bits(label, "check_to_var beside a negative zero", &got, &want);
    }

    /// Asserts that a NaN magnitude enters no minimum in any lane.
    ///
    /// The NaN counts as a negative sign, so the other edges carry the
    /// negated minimum of the finite magnitudes.
    ///
    /// # Panics
    ///
    /// Panics naming `label` when an output differs from that minimum.
    pub fn assert_nan_enters_no_minimum(label: &str, fns: &LdpcLaneFns) {
        let nan = f32::NAN;
        let rows: [&[f32]; 3] = [&[nan, 2.0, 3.0], &[2.0, nan, 3.0], &[2.0, 3.0, nan]];
        let want: Vec<F32x8> = [2.0, -3.0, -2.0, -3.0, 2.0, -2.0, -3.0, -2.0, 2.0]
            .map(|value| F32x8([value; LANES]))
            .to_vec();
        let got = uniform_check_outputs(fns, &rows);
        assert_same_bits(label, "check_to_var beside a NaN", &got, &want);
    }

    /// Asserts the whole lane-kernel contract for one bundle.
    ///
    /// The check step is held, for each rule and bit for bit in every lane,
    /// to the edge-by-edge reduction over a check's other inputs, on checks
    /// of degree zero, one, two and beyond nineteen whose inputs cover
    /// signed zeros, NaN of either sign, infinities, finite extrema and
    /// equal smallest magnitudes at lane-dependent positions. The variable
    /// step is held to the sequential per-frame update for every active-lane
    /// mask, with every inactive lane required to keep its prior belief,
    /// outgoing messages and hard-decision bit. Both steps must reject
    /// inconsistent lengths, and every rejection but a stray edge index must
    /// leave the outputs as found.
    ///
    /// # Panics
    ///
    /// Panics naming `label` and the failing case when the bundle breaks any
    /// of them.
    pub fn assert_lane_contract(label: &str, fns: &LdpcLaneFns) {
        assert_negative_zero_counts_positive(label, fns);
        assert_nan_enters_no_minimum(label, fns);
        assert_check_step(label, fns);
        assert_variable_step(label, fns);
    }
}

#[cfg(test)]
mod tests {
    use super::contract::{
        assert_lane_contract, assert_nan_enters_no_minimum, assert_negative_zero_counts_positive,
    };
    use super::{detect, F32x8, LdpcLaneFns};
    use crate::contract::require_detected;

    #[test]
    fn the_portable_bundle_answers_the_lane_contract() {
        assert_lane_contract("portable", &LdpcLaneFns::portable());
    }

    #[test]
    fn detect_publishes_a_bundle_exactly_on_an_avx2_host() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let avx2 = std::arch::is_x86_feature_detected!("avx2");
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let avx2 = false;
        assert_eq!(detect().is_some(), avx2);
    }

    #[test]
    fn the_avx2_bundle_answers_the_lane_contract() {
        let fns = require_detected("ldpc_lanes::detect()", detect());
        assert_lane_contract("avx2", &fns);
    }

    #[test]
    fn an_avx2_lane_counts_a_negative_zero_message_as_positive() {
        let fns = require_detected("ldpc_lanes::detect()", detect());
        assert_negative_zero_counts_positive("avx2", &fns);
    }

    #[test]
    fn an_avx2_lane_keeps_a_nan_magnitude_out_of_the_minimum() {
        let fns = require_detected("ldpc_lanes::detect()", detect());
        assert_nan_enters_no_minimum("avx2", &fns);
    }

    #[test]
    fn a_lane_vector_is_one_aligned_256_bit_register() {
        assert_eq!(std::mem::align_of::<F32x8>(), 32);
        assert_eq!(std::mem::size_of::<F32x8>(), 32);
    }
}
