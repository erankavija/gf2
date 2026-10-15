//! gf2 steady-state arm of the LDPC batch decoder under the committed coding
//! envelope.
//!
//! The cell of `ldpc_batch_arms::run`, with each worker's decoder built by
//! [`LdpcBatchDecoder::new`] after [`ENVELOPE`] is installed, so every wave
//! takes the route the envelope's decode selectors decide on the detected lane
//! backend. The envelope identity and the observed routes go to standard
//! error as untimed diagnostics. Needs the `tuning-profile` cargo feature of
//! `gf2-coding`.

use gf2_coding::ldpc::{DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::{self, CodingTuning, CodingTuningCodec};
use gf2_core::tuning::{ProfileRegistryBuilder, SectionResolution};
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::LANES;
use ldpc_survey::arm::{
    self, ArmResult, DecoderCase, Normalization, NormalizationKind, Precision, Schedule, Stopping,
    StoppingKind, ARM_RESULT_SCHEMA,
};
use ldpc_throughput::driver::run_cell;
use ldpc_throughput::gf2::config;
use ldpc_throughput::workload::{check_request, Workload};
use serde::Serialize;
use std::hint::black_box;
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::Instant;
use tuning_campaign_support::protocol::sha256_hex;

const ARM: &str = "gf2-batch-tuned";

/// The committed coding owner envelope, embedded so the executable digest
/// pins it. The path counts directory levels under the exception
/// `harness-path-dependencies`.
const ENVELOPE: &str = include_str!(
    "../../../../../../../crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json"
);

/// The installed envelope as a plan's `tuning_profile` names it.
#[derive(Debug, PartialEq, Eq, Serialize)]
struct EnvelopeIdentity {
    id: String,
    sha256: String,
}

/// Installs [`ENVELOPE`] as the process tuning and returns its identity.
///
/// # Errors
///
/// Returns an error when the envelope does not parse, when the process
/// tuning has already resolved, or when the coding section does not resolve
/// from the envelope.
fn install_envelope() -> Result<EnvelopeIdentity, String> {
    let registry = ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .and_then(ProfileRegistryBuilder::build)
        .map_err(|error| error.to_string())?;
    let prepared = registry
        .from_json(ENVELOPE)
        .map_err(|error| error.to_string())?;
    gf2_core::tuning::install(prepared).map_err(|error| error.to_string())?;
    match tuning::active().resolution {
        SectionResolution::Installed { profile_id, .. } => Ok(EnvelopeIdentity {
            id: profile_id.to_string(),
            sha256: sha256_hex(ENVELOPE.as_bytes()),
        }),
        other => Err(format!("the coding section resolves as {other:?}")),
    }
}

/// The route of each wave size of a batch of `batch` frames: the first wave,
/// and a shorter final wave when the batch has one.
fn wave_routes(decoder: &LdpcBatchDecoder, batch: usize) -> Vec<WaveRoute> {
    let last = batch % LANES;
    std::iter::once(batch.min(LANES))
        .chain((batch > LANES && last > 0).then_some(last))
        .map(|frames| WaveRoute {
            frames,
            route: format!("{:?}", decoder.batch_route(frames)),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WaveRoute {
    frames: usize,
    route: String,
}
// A copy of the arms' worker under @/issue/e5f8b3ae: a `lib.rs` change moves pinned digests.
/// One worker's reused default-constructed decoder and buffers.
struct TunedWorker {
    decoder: LdpcBatchDecoder,
    /// Converted LLRs of one wave.
    packed: Vec<Llr>,
    decoded: Vec<BitVec>,
    outcomes: Vec<DecodeOutcome>,
    n: usize,
    k: usize,
    /// Information-window decisions of the last batch, `k` bytes per frame.
    decisions: Vec<u8>,
}

impl TunedWorker {
    /// # Errors
    ///
    /// Returns an error when the decoder groups frames in other waves than
    /// [`LANES`].
    fn new(
        code: &LdpcCode,
        config: DecoderConfig,
        cap: usize,
        batch: usize,
    ) -> Result<Self, String> {
        let (n, k) = (code.n(), code.k());
        let decoder = LdpcBatchDecoder::new(code.clone(), config, cap, DecodedWord::Codeword);
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

    /// Decodes every frame of `llrs`, `n` values per frame, through
    /// [`BatchSoftDecoder::decode_batch`] in waves of [`LANES`], as the
    /// explicit-selector arms do, and extracts the information-window hard
    /// decisions.
    ///
    /// # Panics
    ///
    /// Panics when `llrs` holds another frame count than the worker's batch.
    fn decode_batch(&mut self, llrs: &[f32]) {
        let n = self.n;
        assert_eq!(llrs.len(), self.decoded.len() * n, "batch size");
        for (wave, recorded) in llrs.chunks(LANES * n).enumerate() {
            let packed = &mut self.packed[..recorded.len()];
            for (slot, value) in packed.iter_mut().zip(recorded) {
                *slot = Llr::new(*value);
            }
            let mut views: [&[Llr]; LANES] = [&[]; LANES];
            let mut frames = 0;
            for (view, frame) in views.iter_mut().zip(packed.chunks_exact(n)) {
                *view = frame;
                frames += 1;
            }
            let slots = wave * LANES..wave * LANES + frames;
            BatchSoftDecoder::decode_batch(
                &mut self.decoder,
                black_box(&views[..frames]),
                &mut self.decoded[slots.clone()],
                &mut self.outcomes[slots],
            );
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
    }
}

/// Untimed diagnostics, journalled by the runner as a child diagnostic.
#[derive(Debug, Serialize)]
struct Diagnostics<'a> {
    kind: &'static str,
    arm: &'a str,
    tuning_profile: EnvelopeIdentity,
    /// The routes of the first worker's decoder.
    wave_routes: Vec<WaveRoute>,
    lane_min_frames: usize,
    lane_max_edges: usize,
    granules_per_task: usize,
    batch_frames: usize,
    wave_frames: usize,
    /// Lane workspace bytes each worker's decoder reports, in worker order.
    workspace_bytes: Vec<usize>,
    conversion: arm::ConversionCosts,
}
// A copy of the arms' cell under @/issue/e5f8b3ae: a `lib.rs` change moves pinned digests.
fn run() -> Result<(), String> {
    let envelope = install_envelope()?;
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
    let code = ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    if code.n() != workload.manifest.n
        || code.k() != workload.manifest.k
        || code.parity_check_matrix().nnz() != workload.manifest.nnz
    {
        return Err("the bundle dimensions differ from the built code".to_owned());
    }
    let decoder_config = config(case.normalization_factor, case.syndrome_stopping);
    let cap = case.iteration_cap as usize;
    let workers = Mutex::new(vec![(0usize, Vec::new()); request.cpus.len()]);
    let cell = run_cell(
        ARM,
        &request,
        &workload,
        &quality,
        setup_start,
        0,
        |worker| {
            let state = TunedWorker::new(&code, decoder_config, cap, batch)?;
            workers.lock().expect("worker list")[worker] = (
                state.decoder.workspace_bytes(),
                wave_routes(&state.decoder, batch),
            );
            Ok(state)
        },
        |worker: &mut TunedWorker| worker.decode_batch(llrs),
        |worker: &TunedWorker| &worker.decisions,
    )?;

    let selectors = tuning::active().section.decode().clone();
    let mut workers = workers.into_inner().expect("worker list");
    let workspace_bytes: Vec<usize> = workers.iter().map(|(bytes, _)| *bytes).collect();
    let routes = std::mem::take(&mut workers[0].1);
    let selected_path = format!(
        "gf2-coding LdpcBatchDecoder f32 flooding normalized min-sum, default construction \
         under tuning profile {} (lane_min_frames {} lane_max_edges {}), wave routes {}, \
         batches of {batch} frames in waves of {LANES}, lane workspace {} bytes per worker, \
         reused decoder per worker; {} pinned workers, {} process threads",
        envelope.id,
        selectors.lane_min_frames(),
        selectors.lane_max_edges(),
        routes
            .iter()
            .map(|wave| format!("{} frames {}", wave.frames, wave.route))
            .collect::<Vec<_>>()
            .join(", "),
        workspace_bytes.first().copied().unwrap_or_default(),
        cell.workers_observed,
        cell.threads.ready
    );
    let diagnostics = Diagnostics {
        kind: "batch-decoder-diagnostics",
        arm: ARM,
        tuning_profile: envelope,
        wave_routes: routes,
        lane_min_frames: selectors.lane_min_frames(),
        lane_max_edges: selectors.lane_max_edges(),
        granules_per_task: selectors.granules_per_task().get(),
        batch_frames: batch,
        wave_frames: LANES,
        workspace_bytes,
        conversion: cell.conversion,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&diagnostics).map_err(|e| e.to_string())?
    );

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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-tuned-arm: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{install_envelope, wave_routes, TunedWorker, ENVELOPE, LANES};
    use gf2_coding::ldpc::{BatchRoute, DecodedWord, LdpcBatchDecoder};
    use gf2_coding::test_support::{ldpc_batch_code, ldpc_batch_frames, LDPC_BATCH_CAP};
    use gf2_coding::tuning::{self, DecodeSelectors};
    use ldpc_throughput::gf2::{config, Gf2Worker};

    // The one test of this binary that resolves the process tuning.
    #[test]
    fn default_construction_decodes_under_the_installed_envelope() {
        let envelope = install_envelope().expect("install");
        let committed: serde_json::Value = serde_json::from_str(ENVELOPE).expect("envelope");
        assert_eq!(envelope.id, committed["profile_id"]);
        assert!(install_envelope().is_err(), "the tuning installs once");

        let code = ldpc_batch_code();
        let frames = ldpc_batch_frames();
        let llrs: Vec<f32> = frames.iter().flatten().map(|llr| llr.value()).collect();
        let decoder_config = config(0.75, true);
        for batch in [1, LANES - 1, LANES, LANES + 1, frames.len()] {
            let llrs = &llrs[..batch * code.n()];
            let mut worker =
                TunedWorker::new(&code, decoder_config, LDPC_BATCH_CAP, batch).expect("worker");
            assert_eq!(
                worker.decoder.selectors(),
                tuning::active().section.decode()
            );
            let mut reference = Gf2Worker::new(&code, decoder_config, batch, LDPC_BATCH_CAP);
            reference.decode_batch(llrs);
            worker.decode_batch(llrs);
            assert_eq!(worker.decisions, reference.decisions, "batch {batch}");
        }
    }

    #[test]
    fn the_route_report_names_each_wave_size_once() {
        let decoder = LdpcBatchDecoder::with_selectors(
            ldpc_batch_code(),
            config(0.75, true),
            LDPC_BATCH_CAP,
            DecodedWord::Codeword,
            DecodeSelectors::CONSERVATIVE,
        );
        let sizes = |batch| -> Vec<usize> {
            wave_routes(&decoder, batch)
                .iter()
                .map(|wave| wave.frames)
                .collect()
        };
        assert_eq!(sizes(3), [3]);
        assert_eq!(sizes(LANES), [LANES]);
        assert_eq!(sizes(2 * LANES), [LANES]);
        assert_eq!(sizes(2 * LANES + 3), [LANES, 3]);
        let route = format!("{:?}", BatchRoute::SingleFrame);
        assert!(wave_routes(&decoder, LANES + 1)
            .iter()
            .all(|wave| wave.route == route));
    }
}
