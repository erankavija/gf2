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

    /// Whether decoding stops at the first iteration whose syndrome passes.
    #[must_use]
    pub fn early_termination(&self) -> bool {
        self.early_termination
    }

    /// Quantizes one channel LLR, rounding half away from zero; NaN maps to
    /// zero and infinities to the channel limit.
    #[must_use]
    pub fn quantize(&self, llr: Llr) -> i8 {
        let limit = f32::from(self.channel_limit);
        let value = (llr.value() * self.channel_scale)
            .round()
            .clamp(-limit, limit);
        // NaN stays NaN through clamp, and the saturating cast maps it to 0.
        value as i8
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

/// One layer: its checks of degree two or more, as lanes.
#[derive(Debug, Clone)]
struct Layer {
    degree: usize,
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
    /// The code variable of each schedule variable, when they differ.
    variables: Option<Vec<u32>>,
    /// Beliefs in code variable order, when the orders differ.
    output: Vec<i8>,
    messages: Vec<i8>,
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
            variables,
            messages: vec![0; schedule.message_len],
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
        if self.variables.is_some() {
            &self.output
        } else {
            &self.posterior
        }
    }

    /// Decodes a mother-code frame, overwriting `codeword` with all n hard bits.
    ///
    /// One iteration updates every layer in order; each layer reads the
    /// beliefs its predecessors wrote. Each iteration takes O(E) time. Reuses
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
        match &self.variables {
            Some(variables) => {
                for (slot, &v) in self.posterior.iter_mut().zip(variables) {
                    *slot = self.config.quantize(llrs[v as usize]);
                }
            }
            None => {
                for (slot, &llr) in self.posterior.iter_mut().zip(llrs) {
                    *slot = self.config.quantize(llr);
                }
            }
        }
        self.messages.fill(0);

        let mut iterations = 0;
        for iteration in 0..max_iterations {
            iterations = iteration + 1;
            for index in 0..self.schedule.layers.len() {
                self.update_layer(index);
            }
            if self.config.early_termination && self.syndrome_passes() {
                break;
            }
        }
        self.last_iterations = iterations;
        let syndrome_check_passed = self.syndrome_passes();
        codeword.clear();
        match &self.variables {
            Some(variables) => {
                for (&v, &value) in variables.iter().zip(&self.posterior) {
                    self.output[v as usize] = value;
                }
                for &value in &self.output {
                    codeword.push_bit(value < 0);
                }
            }
            None => {
                for &bit in &self.hard {
                    codeword.push_bit(bit != 0);
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

    fn update_layer(&mut self, index: usize) {
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
        self.kernel.update_layer(
            self.config.params,
            buffer,
            messages,
            layer.degree,
            layer.stride,
        );
        for run in runs {
            let (offset, var, len) = (run.offset as usize, run.var as usize, run.len as usize);
            self.posterior[var..var + len].copy_from_slice(&buffer[offset..offset + len]);
        }
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
