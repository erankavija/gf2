//! Pinned batch decoder arms with direct post-timing iteration observations.

use crate::arm::{
    self, ArmResult, ConversionCosts, DecoderCase, Normalization, NormalizationKind, Precision,
    Schedule, Stopping, StoppingKind, ARM_RESULT_SCHEMA,
};
use crate::driver::{report_iterations, run_cell, CellRun, IterationSource};
use gf2_coding::ldpc::DecoderAlgorithm;
pub use gf2_coding::ldpc::{BatchRoute, LaneBackend};
use gf2_coding::ldpc::{DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::LANES;

fn config(factor: f32, stopping: bool) -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(factor), stopping)
}
use crate::workload::{check_request, median_u64, Workload};
use serde::Serialize;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Instant;

/// Untimed repetitions of the conversion-stage probe.
pub const STAGE_PROBES: usize = 9;

/// The selectors an arm on `route` decodes under: the conservative family,
/// which closes the lane route, or a family that opens it to every wave size
/// and edge count, through the benchmark grid's maximum of 24 workers.
#[must_use]
pub fn route_selectors(route: BatchRoute) -> DecodeSelectors {
    match route {
        BatchRoute::SingleFrame => DecodeSelectors::CONSERVATIVE,
        BatchRoute::Lane(_) => {
            DecodeSelectors::try_new(1, usize::MAX, 24, 1).expect("a positive group count")
        }
    }
}

/// A stage of one worker batch, in execution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Conversion of a wave's recorded f32 values into [`Llr`].
    Pack,
    /// The batch decoder call on a wave.
    Decode,
    /// Extraction of the batch's information-window decisions.
    Extract,
}

/// Nanoseconds one worker batch spent in each [`Stage`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageCosts {
    pub pack_ns: u64,
    pub decode_ns: u64,
    pub extract_ns: u64,
}

/// One worker's reused batch decoder and buffers.
pub struct BatchWorker {
    decoder: LdpcBatchDecoder,
    /// Converted LLRs of one wave.
    packed: Vec<Llr>,
    decoded: Vec<BitVec>,
    outcomes: Vec<DecodeOutcome>,
    n: usize,
    k: usize,
    /// Information-window decisions of the last batch, `k` bytes per frame.
    pub decisions: Vec<u8>,
}

impl BatchWorker {
    /// Builds a worker decoder for a clone of `code` that decodes batches of
    /// `batch` frames on `route`.
    ///
    /// # Errors
    ///
    /// Returns an error when a wave of the batch takes another route than
    /// `route`, which includes a lane backend this build or host lacks.
    pub fn new(
        code: &LdpcCode,
        config: DecoderConfig,
        cap: usize,
        batch: usize,
        route: BatchRoute,
    ) -> Result<Self, String> {
        Self::from_decoder(code, batch, route.build_decoder(code, config, cap, batch)?)
    }

    /// Reuses a decoder built for `code` with codeword output.
    ///
    /// # Errors
    ///
    /// Returns an error when its wave granularity differs from [`LANES`].
    pub fn from_decoder(
        code: &LdpcCode,
        batch: usize,
        decoder: LdpcBatchDecoder,
    ) -> Result<Self, String> {
        let (n, k) = (code.n(), code.k());
        if decoder.batch_granularity() != LANES {
            return Err("the decoder groups frames in other waves than the arm".to_owned());
        }
        Ok(Self {
            decoder,
            packed: vec![Llr::zero(); LANES * n],
            decoded: vec![BitVec::new(); batch],
            outcomes: vec![DecodeOutcome::default(); batch],
            n,
            k,
            decisions: vec![0; batch * k],
        })
    }

    /// Decoder whose selectors and routes describe this worker.
    #[must_use]
    pub fn decoder(&self) -> &LdpcBatchDecoder {
        &self.decoder
    }

    /// Bytes of the decoder's lane workspace.
    #[must_use]
    pub fn workspace_bytes(&self) -> usize {
        self.decoder.workspace_bytes()
    }

    /// Decodes every frame of `llrs`, `n` values per frame, and extracts the
    /// information-window hard decisions.
    ///
    /// The frames go to the decoder in waves of [`LANES`], the grouping it
    /// applies to a longer batch itself, so the frame views live on the stack
    /// and a call allocates nothing after the first.
    ///
    /// # Panics
    ///
    /// Panics when `llrs` holds another frame count than the worker's batch.
    pub fn decode_batch(&mut self, llrs: &[f32]) {
        self.decode_batch_with_workers(llrs, NonZeroUsize::MIN);
    }

    /// Decodes the batch with the configured concurrency of the benchmark cell.
    ///
    /// # Panics
    ///
    /// Panics as [`Self::decode_batch`] does.
    pub fn decode_batch_with_workers(&mut self, llrs: &[f32], workers: NonZeroUsize) {
        self.run_stages(llrs, workers, |_| {});
    }

    /// Decodes as [`Self::decode_batch`] and reports the time of each stage.
    ///
    /// # Panics
    ///
    /// As [`Self::decode_batch`].
    pub fn decode_batch_staged(&mut self, llrs: &[f32]) -> StageCosts {
        let mut costs = StageCosts::default();
        let mut last = Instant::now();
        self.run_stages(llrs, NonZeroUsize::MIN, |stage| {
            let now = Instant::now();
            let elapsed = now.duration_since(last).as_nanos() as u64;
            match stage {
                Stage::Pack => costs.pack_ns += elapsed,
                Stage::Decode => costs.decode_ns += elapsed,
                Stage::Extract => costs.extract_ns += elapsed,
            }
            last = now;
        });
        costs
    }

    /// Runs the batch and calls `lap` at the end of each stage.
    fn run_stages(&mut self, llrs: &[f32], workers: NonZeroUsize, mut lap: impl FnMut(Stage)) {
        let n = self.n;
        assert_eq!(llrs.len(), self.decoded.len() * n, "batch size");
        for (wave, recorded) in llrs.chunks(LANES * n).enumerate() {
            let packed = &mut self.packed[..recorded.len()];
            for (slot, value) in packed.iter_mut().zip(recorded) {
                *slot = Llr::new(*value);
            }
            lap(Stage::Pack);
            let mut views: [&[Llr]; LANES] = [&[]; LANES];
            let mut frames = 0;
            for (view, frame) in views.iter_mut().zip(packed.chunks_exact(n)) {
                *view = frame;
                frames += 1;
            }
            let slots = wave * LANES..wave * LANES + frames;
            self.decoder.decode_batch_with_workers(
                black_box(&views[..frames]),
                &mut self.decoded[slots.clone()],
                &mut self.outcomes[slots],
                workers,
            );
            lap(Stage::Decode);
        }
        for (word, decided) in self
            .decoded
            .iter()
            .zip(self.decisions.chunks_exact_mut(self.k))
        {
            for (position, bit) in decided.iter_mut().enumerate() {
                *bit = u8::from(word.get(position));
            }
        }
        black_box(&self.decisions);
        lap(Stage::Extract);
    }
}

/// Median stage costs of [`STAGE_PROBES`] batches on a decoder with a zero
/// iteration cap.
///
/// At that cap the decoder call runs no kernel iteration: on a lane route it
/// is the transposition into the lanes, the message initialisation, the
/// terminal syndrome test and the transposition out into the decoded words,
/// and on the single-frame route the single-frame decoder's pass at the same
/// cap.
///
/// # Errors
///
/// As [`BatchWorker::new`].
pub fn probe_stages(
    code: &LdpcCode,
    config: DecoderConfig,
    batch: usize,
    route: BatchRoute,
    llrs: &[f32],
) -> Result<StageCosts, String> {
    let mut worker = BatchWorker::new(code, config, 0, batch, route)?;
    // The first call sizes the decoded words.
    worker.decode_batch(llrs);
    let probes: Vec<StageCosts> = (0..STAGE_PROBES)
        .map(|_| worker.decode_batch_staged(black_box(llrs)))
        .collect();
    let median = |stage: fn(&StageCosts) -> u64| median_u64(probes.iter().map(stage).collect());
    Ok(StageCosts {
        pack_ns: median(|costs| costs.pack_ns),
        decode_ns: median(|costs| costs.decode_ns),
        extract_ns: median(|costs| costs.extract_ns),
    })
}

/// Untimed diagnostics an arm writes to standard error, which the runner
/// journals as a child diagnostic.
#[derive(Debug, Serialize)]
struct Diagnostics<'a> {
    kind: &'static str,
    arm: &'a str,
    /// The route of every wave, as the decoder reports it.
    route: String,
    lane_min_frames: usize,
    lane_max_edges: usize,
    lane_max_workers: usize,
    granules_per_task: usize,
    batch_frames: usize,
    wave_frames: usize,
    /// Lane workspace bytes each worker's decoder reports, in worker order.
    workspace_bytes: Vec<usize>,
    /// Probes behind each of `pack_ns`, `batch_fill_ns` and `unpack_ns`.
    stage_probes: usize,
    stage_statistic: &'static str,
    /// `batch_fill_ns` is the decoder call of [`probe_stages`].
    conversion: ConversionCosts,
}

/// Decoder construction and route diagnostics for the shared cell lifecycle.
pub trait ArmPolicy: Sync {
    /// Per-worker route observations, collected before dispatch.
    type Observation: Send;

    /// Builds a codeword decoder, rejecting routes the arm does not declare.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested route is unavailable.
    fn build_decoder(
        &self,
        code: &LdpcCode,
        config: DecoderConfig,
        cap: usize,
        batch: usize,
    ) -> Result<LdpcBatchDecoder, String>;

    /// Observes the routes with the cell's actual worker count.
    fn observe(
        &self,
        decoder: &LdpcBatchDecoder,
        batch: usize,
        workers: NonZeroUsize,
    ) -> Self::Observation;

    /// Writes untimed diagnostics and returns the result's selected path.
    ///
    /// # Errors
    ///
    /// Returns an error when a diagnostic probe or serialization fails.
    fn report(
        &self,
        context: CellContext<'_>,
        observations: Vec<Self::Observation>,
        workspace_bytes: Vec<usize>,
        cell: &mut CellRun,
    ) -> Result<String, String>;
}

/// Inputs available to an arm's untimed diagnostic probes.
pub struct CellContext<'a> {
    pub arm_name: &'a str,
    pub code: &'a LdpcCode,
    pub decoder_config: DecoderConfig,
    pub batch: usize,
    pub llrs: &'a [f32],
}

impl ArmPolicy for BatchRoute {
    type Observation = ();

    fn build_decoder(
        &self,
        code: &LdpcCode,
        config: DecoderConfig,
        cap: usize,
        batch: usize,
    ) -> Result<LdpcBatchDecoder, String> {
        let route = *self;
        let decoder = LdpcBatchDecoder::with_selectors(
            code.clone(),
            config,
            cap,
            DecodedWord::Codeword,
            route_selectors(route),
        );
        let decoder = match route {
            BatchRoute::Lane(LaneBackend::Portable) => decoder.with_portable_lanes(),
            _ => decoder,
        };
        for frames in [batch.min(LANES), batch % LANES] {
            let observed = decoder.batch_route(frames);
            if frames > 0 && observed != route {
                return Err(format!(
                    "a wave of {frames} frames decodes on {observed:?}; the arm declares {route:?}"
                ));
            }
        }
        Ok(decoder)
    }

    fn observe(&self, _: &LdpcBatchDecoder, _: usize, _: NonZeroUsize) {}

    fn report(
        &self,
        context: CellContext<'_>,
        _: Vec<()>,
        workspace_bytes: Vec<usize>,
        cell: &mut CellRun,
    ) -> Result<String, String> {
        let route = *self;
        let CellContext {
            arm_name,
            code,
            decoder_config,
            batch,
            llrs,
        } = context;
        let stages = probe_stages(code, decoder_config, batch, route, llrs)?;
        cell.conversion.pack_ns = stages.pack_ns;
        cell.conversion.batch_fill_ns = stages.decode_ns;
        cell.conversion.unpack_ns = stages.extract_ns;

        let selectors = route_selectors(route);
        let diagnostics = Diagnostics {
            kind: "batch-decoder-diagnostics",
            arm: arm_name,
            route: format!("{route:?}"),
            lane_min_frames: selectors.lane_min_frames(),
            lane_max_edges: selectors.lane_max_edges(),
            lane_max_workers: selectors.lane_max_workers().get(),
            granules_per_task: selectors.granules_per_task().get(),
            batch_frames: batch,
            wave_frames: LANES,
            workspace_bytes: workspace_bytes.clone(),
            stage_probes: STAGE_PROBES,
            stage_statistic: "median",
            conversion: cell.conversion,
        };
        eprintln!(
            "{}",
            serde_json::to_string(&diagnostics).map_err(|e| e.to_string())?
        );

        Ok(format!(
            "gf2-coding LdpcBatchDecoder f32 flooding normalized min-sum, route {route:?} under \
             selectors lane_min_frames {} lane_max_edges {}, batches of {batch} frames in waves \
             of {LANES}, lane workspace {} bytes per worker, reused decoder per worker; \
             {} pinned workers, {} process threads",
            selectors.lane_min_frames(),
            selectors.lane_max_edges(),
            workspace_bytes.first().copied().unwrap_or_default(),
            cell.workers_observed,
            cell.threads.ready
        ))
    }
}

/// Runs one steady-state cell of the batch decoder on `route` and writes the
/// arm result.
///
/// # Errors
///
/// Returns an error when the request, the bundle or the prepared quality is
/// unusable, when a wave leaves `route`, or when a worker violates its
/// placement or decodes other than the prepared per-frame error counts.
pub fn run(arm_name: &str, route: BatchRoute) -> Result<(), String> {
    run_with_policy(arm_name, route)
}

/// Runs the shared cell lifecycle with arm-specific construction and diagnostics.
///
/// # Errors
///
/// As [`run`], including errors from `policy`.
pub fn run_with_policy(arm_name: &str, policy: impl ArmPolicy) -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;
    // The settings of the single-frame decode that produced the prepared
    // record: each frame of a batch equals that decode bit for bit.
    let settings = arm::ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(f64::from(case.normalization_factor)),
        },
        iteration_cap: case.iteration_cap,
        stopping: Stopping {
            kind: if case.syndrome_stopping {
                StoppingKind::Syndrome
            } else {
                StoppingKind::Fixed
            },
            crc: None,
        },
        batch_size: 1,
    };
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    let setup_start = Instant::now();
    let code = crate::bundle::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    if code.n() != workload.manifest.n
        || code.k() != workload.manifest.k
        || code.parity_check_matrix().nnz() != workload.manifest.nnz
    {
        return Err("the bundle dimensions differ from the built code".to_owned());
    }
    let decoder_config = config(case.normalization_factor, case.syndrome_stopping);
    let cap = case.iteration_cap as usize;
    let concurrency = NonZeroUsize::new(request.cpus.len());
    let observations = Mutex::new(
        std::iter::repeat_with(|| None)
            .take(request.cpus.len())
            .collect::<Vec<_>>(),
    );
    let mut cell = run_cell(
        arm_name,
        &request,
        &workload,
        &quality,
        setup_start,
        0,
        |worker| {
            let decoder = policy.build_decoder(&code, decoder_config, cap, batch)?;
            let state = BatchWorker::from_decoder(&code, batch, decoder)?;
            let observed =
                policy.observe(state.decoder(), batch, concurrency.expect("nonempty pool"));
            observations.lock().expect("worker observations")[worker] =
                Some((state.workspace_bytes(), observed));
            Ok(state)
        },
        |worker: &mut BatchWorker| {
            worker.decode_batch_with_workers(llrs, concurrency.expect("nonempty pool"))
        },
        |worker: &BatchWorker| &worker.decisions,
        |index, worker| {
            let iterations: Vec<u32> = worker
                .outcomes
                .iter()
                .map(|outcome| outcome.iterations as u32)
                .collect();
            report_iterations(arm_name, index, IterationSource::Direct, &iterations);
            Ok(())
        },
    )?;
    let observations = observations.into_inner().expect("worker observations");
    let (workspace_bytes, routes) = observations.into_iter().map(Option::unwrap).unzip();
    let selected_path = policy.report(
        CellContext {
            arm_name,
            code: &code,
            decoder_config,
            batch,
            llrs,
        },
        routes,
        workspace_bytes,
        &mut cell,
    )?;

    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(selected_path),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

#[cfg(test)]
mod tests {
    use super::{probe_stages, BatchWorker, NonZeroUsize, LANES};
    use crate::reference::{config, Gf2Worker};
    use gf2_coding::ldpc::{BatchRoute, LaneBackend, LdpcCode, LdpcEncoder, QuasiCyclicLdpc};
    use gf2_coding::traits::BlockEncoder;
    use gf2_core::BitVec;

    const CAP: usize = 12;

    fn code() -> LdpcCode {
        let base = vec![
            vec![0, 3, 7, 1, 0, -1],
            vec![5, 0, 11, -1, 2, 9],
            vec![9, 13, 0, 6, -1, 4],
        ];
        LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16))
    }

    /// Recorded-style f32 LLRs of `frames` noisy codewords; the noise grows
    /// with the frame index so a batch mixes converging and failing frames.
    fn recorded(code: &LdpcCode, frames: usize) -> Vec<f32> {
        let encoder = LdpcEncoder::new(code.clone());
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut llrs = Vec::with_capacity(frames * code.n());
        for frame in 0..frames {
            let codeword = encoder.encode(&BitVec::random_seeded(encoder.k(), frame as u64));
            let spread = 1.0 + frame as f32 * 0.6;
            for position in 0..codeword.len() {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let unit = (state >> 40) as f32 / (1u64 << 24) as f32;
                let sign = if codeword.get(position) { -1.0 } else { 1.0 };
                llrs.push(sign * 2.0 + (unit - 0.5) * 2.0 * spread);
            }
        }
        llrs
    }

    fn routes() -> Vec<BatchRoute> {
        vec![
            BatchRoute::SingleFrame,
            BatchRoute::Lane(LaneBackend::Portable),
            BatchRoute::Lane(LaneBackend::detected()),
        ]
    }

    #[test]
    fn each_route_decodes_the_single_frame_decisions() {
        let code = code();
        let decoder_config = config(0.75, true);
        for batch in [1, LANES - 1, LANES, LANES + 1, 2 * LANES + 1] {
            let llrs = recorded(&code, batch);
            let mut reference = Gf2Worker::new(&code, decoder_config, batch, CAP);
            reference.decode_batch(&llrs);
            for route in routes() {
                let mut worker =
                    BatchWorker::new(&code, decoder_config, CAP, batch, route).expect("route");
                for workers in [1, 6, 12, 24] {
                    worker.decode_batch_with_workers(&llrs, NonZeroUsize::new(workers).unwrap());
                    assert_eq!(
                        worker.decisions, reference.decisions,
                        "{route:?} batch {batch} workers {workers}"
                    );
                }
                assert_eq!(
                    worker.decisions, reference.decisions,
                    "{route:?} batch {batch}"
                );
                worker.decode_batch_staged(&llrs);
                assert_eq!(
                    worker.decisions, reference.decisions,
                    "{route:?} batch {batch}"
                );
            }
        }
    }

    #[test]
    fn an_arm_refuses_a_wave_on_another_route() {
        let code = code();
        let absent = match LaneBackend::detected() {
            LaneBackend::Portable => LaneBackend::Avx2,
            LaneBackend::Avx2 => return,
        };
        let refused = BatchWorker::new(
            &code,
            config(0.75, true),
            CAP,
            LANES,
            BatchRoute::Lane(absent),
        );
        assert!(refused.err().expect("refused").contains("the arm declares"));
    }

    #[test]
    fn the_stage_probe_decodes_the_channel_hard_decisions() {
        let code = code();
        let batch = LANES + 3;
        let llrs = recorded(&code, batch);
        for route in routes() {
            let mut worker =
                BatchWorker::new(&code, config(0.75, true), 0, batch, route).expect("route");
            worker.decode_batch(&llrs);
            for (frame, decided) in worker.decisions.chunks_exact(code.k()).enumerate() {
                let channel = &llrs[frame * code.n()..][..code.k()];
                let expected: Vec<u8> = channel.iter().map(|llr| u8::from(*llr < 0.0)).collect();
                assert_eq!(decided, expected, "{route:?} frame {frame}");
            }
            probe_stages(&code, config(0.75, true), batch, route, &llrs).expect("probe");
        }
    }

    #[test]
    fn selectors_route_every_wave_size_to_the_declared_route() {
        let code = code();
        for route in routes() {
            let worker =
                BatchWorker::new(&code, config(0.75, true), CAP, LANES, route).expect("route");
            for frames in 1..=LANES {
                for workers in [1, 6, 12, 24] {
                    assert_eq!(
                        worker
                            .decoder
                            .batch_route_with_workers(frames, NonZeroUsize::new(workers).unwrap()),
                        route,
                        "{frames} frames, {workers} workers"
                    );
                }
            }
        }
    }
}
