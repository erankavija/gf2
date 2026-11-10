//! Layered quantized min-sum decoding. Each layer is a set of checks sharing
//! no variable, updated at once with one SIMD lane per check; the rules are
//! in `docs/reference/ldpc-decoder-numerical-contract.md`.

use super::{DecodeOutcome, LaneBackend, LdpcCode};
use crate::llr::Llr;
use gf2_core::BitVec;
pub use gf2_kernels_simd::ldpc_layered::LayeredRule;
use gf2_kernels_simd::ldpc_layered::{LayerKernel, LayerParams, LAYER_LANES, LAYER_SATURATION};

/// Quantization and update rules of a [`LayeredDecoder`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayeredConfig {
    params: LayerParams,
    channel_scale: f32,
    channel_limit: u8,
    early_termination: bool,
}

impl LayeredConfig {
    /// A channel LLR `x` quantizes to `round(x * channel_scale)` bounded by
    /// ±`channel_limit`; check messages are bounded by ±`message_limit`.
    ///
    /// # Panics
    ///
    /// Panics if `channel_scale` is not finite and positive, `channel_limit`
    /// is outside `1..=127`, `message_limit` is outside `1..=126`, or the rule's parameter is
    /// outside the domain stated by [`LayeredRule`].
    #[must_use]
    pub fn new(
        rule: LayeredRule,
        channel_scale: f32,
        channel_limit: u8,
        message_limit: u8,
        early_termination: bool,
    ) -> Self {
        assert!(
            channel_scale.is_finite() && channel_scale > 0.0,
            "channel scale must be finite and positive"
        );
        assert!(
            (1..=127).contains(&channel_limit),
            "channel limit must be in 1..=127"
        );
        Self {
            params: LayerParams::new(rule, message_limit),
            channel_scale,
            channel_limit,
            early_termination,
        }
    }

    /// The 5G NR setting: normalized min-sum at 6/8, scale 4, channel limit
    /// 23, message limit 24, early termination.
    ///
    /// The normalization factor is tuned per code family; this one and
    /// [`Self::dvb_t2`]'s were chosen on frame error rate under
    /// `crates/gf2-coding/tests/ldpc_layered_fer.rs`. NR base graph 1 has
    /// columns of degree up to 30 whose beliefs saturate, and each check
    /// turning against one removes twice the message limit, so the bound
    /// of 127 must hold several messages: with message limit 40 the decoder
    /// leaves decoded NR codewords at high SNR, and with 31 it keeps an
    /// error floor. The message limit exceeds the channel limit so that one
    /// check can overrule the channel on a degree-2 parity bit.
    ///
    /// The setting assumes natural-log channel LLRs, `2y/σ²` for BPSK over
    /// AWGN; LLRs at `r` times that scale decode as at scale `4r`, so a
    /// caller with another convention passes scale `4/r` to
    /// [`Self::new`].
    #[must_use]
    pub fn nr_5g() -> Self {
        Self::new(LayeredRule::Normalized { eighths: 6 }, 4.0, 23, 24, true)
    }

    /// The DVB-T2 setting: normalized min-sum at 7/8, scale 4, channel
    /// limit 31, message limit 40, early termination, for natural-log channel
    /// LLRs as in [`Self::nr_5g`]. DVB-T2 columns have degree at most 13,
    /// which leaves this message limit stable.
    #[must_use]
    pub fn dvb_t2() -> Self {
        Self::new(LayeredRule::Normalized { eighths: 7 }, 4.0, 31, 40, true)
    }

    /// The check rule.
    #[must_use]
    pub fn rule(&self) -> LayeredRule {
        self.params.rule()
    }

    /// Quanta per unit of channel LLR.
    #[must_use]
    pub fn channel_scale(&self) -> f32 {
        self.channel_scale
    }

    /// The magnitude bound of a quantized channel LLR.
    #[must_use]
    pub fn channel_limit(&self) -> u8 {
        self.channel_limit
    }

    /// The magnitude bound of a check-to-variable message.
    #[must_use]
    pub fn message_limit(&self) -> u8 {
        self.params.message_limit()
    }

    /// Whether decoding stops after the first settled iteration, as
    /// [`LayeredDecoder::decode_codeword_into`] states.
    #[must_use]
    pub fn early_termination(&self) -> bool {
        self.early_termination
    }

    /// Quantizes one channel LLR, rounding half away from zero; NaN maps to
    /// zero and infinities to the channel limit.
    #[must_use]
    pub fn quantize(&self, llr: Llr) -> i8 {
        // Adding 1.5 * 2^23 rounds a magnitude below 2^22 to an integer in
        // the low mantissa bits, ties to even; the exact remainder finds the
        // ties to round up. This vectorizes where `f32::round` (a libm call
        // on baseline x86-64) and saturating casts do not. Clamping before
        // rounding is exact because the limit is an integer.
        const MAGIC: f32 = 12_582_912.0;
        let scaled = llr.value() * self.channel_scale;
        let magnitude = if scaled.is_nan() {
            0.0
        } else {
            scaled.abs().min(f32::from(self.channel_limit))
        };
        let shifted = magnitude + MAGIC;
        let tie = u32::from(magnitude - (shifted - MAGIC) == 0.5);
        let level = (shifted.to_bits() - MAGIC.to_bits() + tie) as i8;
        if scaled.is_sign_negative() {
            -level
        } else {
            level
        }
    }
}

/// Why a layer partition is not a schedule of a code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerError {
    /// A layer names a check index at or beyond the code's `m`.
    CheckOutOfRange {
        /// The offending check.
        check: usize,
    },
    /// A check appears in more than one layer position.
    DuplicateCheck {
        /// The offending check.
        check: usize,
    },
    /// A check appears in no layer.
    MissingCheck {
        /// The first missing check.
        check: usize,
    },
    /// Two checks of one layer share a variable.
    SharedVariable {
        /// The layer's position in the partition.
        layer: usize,
        /// The shared variable.
        variable: usize,
    },
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CheckOutOfRange { check } => write!(f, "check {check} is out of range"),
            Self::DuplicateCheck { check } => write!(f, "check {check} appears twice"),
            Self::MissingCheck { check } => write!(f, "check {check} is in no layer"),
            Self::SharedVariable { layer, variable } => {
                write!(f, "layer {layer} shares variable {variable}")
            }
        }
    }
}

impl std::error::Error for LayerError {}

/// Each check's variables in ascending order.
fn code_rows(code: &LdpcCode) -> Vec<Vec<usize>> {
    let h = code.parity_check_matrix();
    (0..code.m())
        .map(|check| {
            let mut columns: Vec<usize> = h.row_iter(check).collect();
            columns.sort_unstable();
            columns
        })
        .collect()
}

/// Splits each group of checks into layers: in group order, each check joins
/// the first of its group's layers it shares no variable with. Takes O(E * s)
/// time for E edges and at most s layers per group.
fn split_conflicts(rows: &[Vec<usize>], n: usize, groups: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut layers: Vec<Vec<usize>> = Vec::new();
    // stamps[s][v] == group + 1 when variable v is in the group's layer s.
    let mut stamps: Vec<Vec<usize>> = Vec::new();
    for (group, checks) in groups.iter().enumerate() {
        let first = layers.len();
        for &check in checks {
            let open = layers.len() - first;
            let sub = stamps[..open]
                .iter()
                .position(|stamp| rows[check].iter().all(|&v| stamp[v] != group + 1))
                .unwrap_or(open);
            if sub == open {
                layers.push(Vec::new());
                if stamps.len() == sub {
                    stamps.push(vec![0; n]);
                }
            }
            for &v in &rows[check] {
                stamps[sub][v] = group + 1;
            }
            layers[first + sub].push(check);
        }
    }
    layers
}

/// A contiguous copy between the posterior and a layer buffer.
#[derive(Debug, Clone, Copy)]
struct Run {
    offset: u32,
    var: u32,
    len: u32,
}

/// Schedule positions `schedule..schedule + len` hold the code variables
/// `code + i * step`.
#[derive(Debug, Clone, Copy)]
struct Stride {
    schedule: u32,
    code: u32,
    step: u32,
    len: u32,
}

/// Splits a variable order into maximal arithmetic runs of positive step;
/// the column orders of DVB-T2 parity are transposes, a few hundred runs.
fn strides(order: &[u32]) -> Vec<Stride> {
    let mut strides = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let code = order[start];
        let step = match order.get(start + 1) {
            Some(&next) if next > code => next - code,
            _ => 1,
        };
        let len = order[start..]
            .iter()
            .zip(0u32..)
            .take_while(|&(&v, i)| u64::from(v) == u64::from(code) + u64::from(i) * u64::from(step))
            .count();
        strides.push(Stride {
            schedule: start as u32,
            code,
            step,
            len: len as u32,
        });
        start += len;
    }
    strides
}

/// One layer: its checks of degree two or more, as lanes.
#[derive(Debug, Clone)]
struct Layer {
    degree: usize,
    /// Checks of the layer, the first `width` of its `stride` lanes.
    width: usize,
    stride: usize,
    messages: usize,
    runs: std::ops::Range<usize>,
    absent: std::ops::Range<usize>,
}

/// Layers over the code's checks, with edge maps into the posterior.
///
/// Lane `i`, slot `k` of a layer is the `k`-th variable, ascending, of the
/// layer's `i`-th check of degree two or more. A slot past a check's degree is
/// absent and enters each update as a saturated positive input with a zero
/// message, which leaves the check's real messages unchanged. Checks of degree
/// at most one send only zero messages and appear only in the syndrome.
#[derive(Debug, Clone)]
struct Schedule {
    n: usize,
    layers: Vec<Layer>,
    runs: Vec<Run>,
    absent: Vec<u32>,
    singletons: Vec<u32>,
    message_len: usize,
    buffer_len: usize,
    max_stride: usize,
}

impl Schedule {
    fn new(n: usize, rows: &[Vec<usize>], partition: &[Vec<usize>]) -> Result<Self, LayerError> {
        let mut seen = vec![false; rows.len()];
        let mut owner = vec![usize::MAX; n];
        let mut schedule = Self {
            n,
            layers: Vec::new(),
            runs: Vec::new(),
            absent: Vec::new(),
            singletons: Vec::new(),
            message_len: 0,
            buffer_len: 0,
            max_stride: 0,
        };
        for (index, layer) in partition.iter().enumerate() {
            let mut lanes: Vec<Vec<usize>> = Vec::new();
            for &check in layer {
                if check >= rows.len() {
                    return Err(LayerError::CheckOutOfRange { check });
                }
                if std::mem::replace(&mut seen[check], true) {
                    return Err(LayerError::DuplicateCheck { check });
                }
                let columns = &rows[check];
                for &variable in columns {
                    if std::mem::replace(&mut owner[variable], index) == index {
                        return Err(LayerError::SharedVariable {
                            layer: index,
                            variable,
                        });
                    }
                }
                match columns.len() {
                    0 => {}
                    1 => schedule.singletons.push(columns[0] as u32),
                    _ => lanes.push(columns.clone()),
                }
            }
            if !lanes.is_empty() {
                schedule.push_layer(&lanes);
            }
        }
        if let Some(check) = seen.iter().position(|&s| !s) {
            return Err(LayerError::MissingCheck { check });
        }
        Ok(schedule)
    }

    fn push_layer(&mut self, lanes: &[Vec<usize>]) {
        let degree = lanes.iter().map(Vec::len).max().unwrap_or(0);
        let stride = lanes.len().div_ceil(LAYER_LANES) * LAYER_LANES;
        let (runs_start, absent_start) = (self.runs.len(), self.absent.len());
        for slot in 0..degree {
            let mut open: Option<Run> = None;
            for (lane, columns) in lanes.iter().enumerate() {
                let offset = (slot * stride + lane) as u32;
                let Some(&var) = columns.get(slot) else {
                    self.absent.push(offset);
                    self.runs.extend(open.take());
                    continue;
                };
                let var = var as u32;
                match &mut open {
                    Some(run) if run.offset + run.len == offset && run.var + run.len == var => {
                        run.len += 1;
                    }
                    _ => {
                        self.runs.extend(open.replace(Run {
                            offset,
                            var,
                            len: 1,
                        }));
                    }
                }
            }
            self.runs.extend(open);
        }
        self.layers.push(Layer {
            degree,
            width: lanes.len(),
            stride,
            messages: self.message_len,
            runs: runs_start..self.runs.len(),
            absent: absent_start..self.absent.len(),
        });
        self.message_len += degree * stride;
        self.buffer_len = self.buffer_len.max(degree * stride);
        self.max_stride = self.max_stride.max(stride);
    }
}

/// Layered min-sum decoder over saturating `i8` messages, vectorized across
/// the checks of each layer.
///
/// Its quality measure is frame error rate; its decisions are not those of
/// the flooding [`super::LdpcDecoder`] frame for frame.
pub struct LayeredDecoder {
    schedule: Schedule,
    config: LayeredConfig,
    kernel: LayerKernel,
    backend: LaneBackend,
    /// Beliefs in schedule variable order.
    posterior: Vec<i8>,
    /// The code variables of the schedule variables, when the orders differ.
    strides: Option<Vec<Stride>>,
    /// Beliefs in code variable order, when the orders differ.
    output: Vec<i8>,
    messages: Vec<i8>,
    /// Packed decisions of the last decode.
    words: Vec<u64>,
    buffer: Vec<i8>,
    hard: Vec<u8>,
    parity: Vec<u8>,
    last_iterations: usize,
}

impl LayeredDecoder {
    /// Layers a code with retained QC structure by base-matrix row block, in
    /// ascending order; returns `None` without that structure.
    ///
    /// The schedule runs over the lifted matrix, so the variables of a
    /// circulant are contiguous whatever the structure's row and column
    /// orders. A row block holding several circulants of one column block
    /// is split into layers sharing no variable, each check joining the first
    /// that admits it in lifted-row order. Uses AVX2 when the `simd` feature
    /// and host support it, otherwise portable code. Construction takes
    /// O(E (log d + s)) time and O(n s + E) space for E edges, largest check
    /// degree d and at most s layers per row block.
    #[must_use]
    pub fn new(code: &LdpcCode, config: LayeredConfig) -> Option<Self> {
        let qc = code.quasi_cyclic()?;
        let (z, m, n) = (
            qc.expansion_factor(),
            qc.expanded_rows(),
            qc.expanded_cols(),
        );
        let lifted = qc.clone().with_orders((0..m).collect(), (0..n).collect());
        let mut rows = vec![Vec::new(); m];
        for (row, column) in lifted.to_edges() {
            rows[row].push(column);
        }
        for row in &mut rows {
            row.sort_unstable();
        }
        let blocks: Vec<Vec<usize>> = (0..qc.base_rows())
            .map(|block| (block * z..(block + 1) * z).collect())
            .collect();
        let partition = split_conflicts(&rows, n, &blocks);
        let schedule = Schedule::new(n, &rows, &partition).ok()?;
        let variables = qc
            .col_order()
            .map(|order| order.iter().map(|&v| v as u32).collect());
        Some(Self::from_schedule(schedule, variables, config))
    }

    /// Layers a code by an explicit partition of its checks, updated in the
    /// given order; within a layer, lanes follow the listed check order.
    ///
    /// # Errors
    ///
    /// Returns a [`LayerError`] unless every check appears exactly once and
    /// no two checks of one layer share a variable.
    pub fn with_layers(
        code: &LdpcCode,
        partition: &[Vec<usize>],
        config: LayeredConfig,
    ) -> Result<Self, LayerError> {
        let schedule = Schedule::new(code.n(), &code_rows(code), partition)?;
        Ok(Self::from_schedule(schedule, None, config))
    }

    fn from_schedule(
        schedule: Schedule,
        variables: Option<Vec<u32>>,
        config: LayeredConfig,
    ) -> Self {
        let (kernel, backend) = (LayerKernel::portable(), LaneBackend::Portable);
        #[cfg(feature = "simd")]
        let (kernel, backend) = gf2_kernels_simd::ldpc_layered::detect()
            .map_or((kernel, backend), |fns| (fns, LaneBackend::Avx2));
        Self {
            posterior: vec![0; schedule.n],
            output: if variables.is_some() {
                vec![0; schedule.n]
            } else {
                Vec::new()
            },
            strides: variables.as_deref().map(strides),
            messages: vec![0; schedule.message_len],
            words: Vec::with_capacity(schedule.n.div_ceil(64)),
            buffer: vec![0; schedule.buffer_len],
            hard: vec![0; schedule.n],
            parity: vec![0; schedule.max_stride],
            schedule,
            config,
            kernel,
            backend,
            last_iterations: 0,
        }
    }

    /// Selects the portable kernel on every host.
    #[must_use]
    pub fn with_portable_kernel(mut self) -> Self {
        self.kernel = LayerKernel::portable();
        self.backend = LaneBackend::Portable;
        self
    }

    /// The backend of the selected kernel.
    #[must_use]
    pub fn backend(&self) -> LaneBackend {
        self.backend
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &LayeredConfig {
        &self.config
    }

    /// Iterations the last decode performed.
    #[must_use]
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// Terminal quantized beliefs, or zeros before the first decode; a zero
    /// cap leaves the quantized channel values. A negative value decides 1.
    #[must_use]
    pub fn posterior(&self) -> &[i8] {
        if self.strides.is_some() {
            &self.output
        } else {
            &self.posterior
        }
    }

    /// Decodes a mother-code frame, overwriting `codeword` with all n hard bits.
    ///
    /// One iteration updates every layer in order; each layer reads the
    /// beliefs its predecessors wrote. Early termination stops after the
    /// first iteration in which no decision changed and every check was
    /// satisfied at its update. Each iteration takes O(E) time. Reuses
    /// all workspace; only `codeword` may grow.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the code's `n`.
    pub fn decode_codeword_into(
        &mut self,
        llrs: &[Llr],
        max_iterations: usize,
        codeword: &mut BitVec,
    ) -> DecodeOutcome {
        assert_eq!(llrs.len(), self.schedule.n, "LLR length must equal n");
        // Quantize in code order, which vectorizes, then permute bytes.
        let quantized = match &self.strides {
            Some(_) => &mut self.output,
            None => &mut self.posterior,
        };
        for (slot, &llr) in quantized.iter_mut().zip(llrs) {
            *slot = self.config.quantize(llr);
        }
        if let Some(strides) = &self.strides {
            for run in strides {
                let (schedule, len) = (run.schedule as usize, run.len as usize);
                let (code, step) = (run.code as usize, run.step as usize);
                let to = &mut self.posterior[schedule..schedule + len];
                let from = &self.output[code..=code + (len - 1) * step];
                for (slot, &value) in to.iter_mut().zip(from.iter().step_by(step)) {
                    *slot = value;
                }
            }
        }
        self.messages.fill(0);

        let mut iterations = 0;
        let mut settled = false;
        for iteration in 0..max_iterations {
            iterations = iteration + 1;
            settled = true;
            for index in 0..self.schedule.layers.len() {
                settled &= self.update_layer(index);
            }
            // A settled sweep changed no decision and found every check of
            // degree two or more satisfied, so the syndrome passes exactly
            // when the single-variable checks do.
            settled &= self
                .schedule
                .singletons
                .iter()
                .all(|&v| self.posterior[v as usize] >= 0);
            if self.config.early_termination && settled {
                break;
            }
        }
        self.last_iterations = iterations;
        let syndrome_check_passed = settled || self.syndrome_passes();
        codeword.clear();
        let beliefs = match &self.strides {
            Some(strides) => {
                for run in strides {
                    let (schedule, len) = (run.schedule as usize, run.len as usize);
                    let (code, step) = (run.code as usize, run.step as usize);
                    let from = &self.posterior[schedule..schedule + len];
                    let to = &mut self.output[code..=code + (len - 1) * step];
                    for (slot, &value) in to.iter_mut().step_by(step).zip(from) {
                        *slot = value;
                    }
                }
                &self.output
            }
            None => &self.posterior,
        };
        self.words.resize(beliefs.len().div_ceil(64), 0);
        self.kernel.pack_signs(beliefs, &mut self.words);
        codeword.resize(beliefs.len(), false);
        codeword.write_bits(0, &self.words, beliefs.len());
        DecodeOutcome {
            iterations,
            converged: syndrome_check_passed,
            syndrome_check_passed,
            queries: None,
        }
    }

    /// Updates one layer and returns whether it is settled.
    fn update_layer(&mut self, index: usize) -> bool {
        let layer = &self.schedule.layers[index];
        let length = layer.degree * layer.stride;
        let buffer = &mut self.buffer[..length];
        let messages = &mut self.messages[layer.messages..layer.messages + length];
        let runs = &self.schedule.runs[layer.runs.clone()];
        for run in runs {
            let (offset, var, len) = (run.offset as usize, run.var as usize, run.len as usize);
            buffer[offset..offset + len].copy_from_slice(&self.posterior[var..var + len]);
        }
        for &offset in &self.schedule.absent[layer.absent.clone()] {
            buffer[offset as usize] = LAYER_SATURATION;
            messages[offset as usize] = 0;
        }
        let settled = self.kernel.update_layer(
            self.config.params,
            buffer,
            messages,
            layer.degree,
            layer.stride,
            layer.width,
        );
        for run in runs {
            let (offset, var, len) = (run.offset as usize, run.var as usize, run.len as usize);
            self.posterior[var..var + len].copy_from_slice(&buffer[offset..offset + len]);
        }
        settled
    }

    fn syndrome_passes(&mut self) -> bool {
        for (bit, &value) in self.hard.iter_mut().zip(&self.posterior) {
            *bit = u8::from(value < 0);
        }
        if self
            .schedule
            .singletons
            .iter()
            .any(|&v| self.hard[v as usize] != 0)
        {
            return false;
        }
        for layer in &self.schedule.layers {
            let parity = &mut self.parity[..layer.stride];
            parity.fill(0);
            for run in &self.schedule.runs[layer.runs.clone()] {
                let lane = run.offset as usize % layer.stride;
                let (var, len) = (run.var as usize, run.len as usize);
                for (p, &h) in parity[lane..lane + len]
                    .iter_mut()
                    .zip(&self.hard[var..var + len])
                {
                    *p ^= h;
                }
            }
            if parity.iter().any(|&p| p != 0) {
                return false;
            }
        }
        true
    }
}
