//! gf2 steady-state arm of the LDPC batch decoder under the committed coding
//! envelope.
//!
//! The shared cell of `ldpc_batch_arms`, with each worker's decoder built by
//! [`LdpcBatchDecoder::new`] after [`ENVELOPE`] is installed, so every wave
//! takes the route the envelope's decode selectors decide on the detected lane
//! backend. The envelope identity and the observed routes are untimed
//! diagnostics. Needs the `tuning-profile` cargo feature of `gf2-coding`.

use gf2_coding::ldpc::{DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode};
use gf2_coding::tuning::{self, CodingTuning, CodingTuningCodec};
use gf2_core::tuning::{ProfileRegistryBuilder, SectionResolution};
use gf2_kernels_simd::ldpc_lanes::LANES;
use ldpc_batch_arms::{run_with_policy, ArmPolicy, CellContext};
use ldpc_survey::arm;
use ldpc_throughput::driver::CellRun;
use serde::Serialize;
use std::num::NonZeroUsize;
use std::process::ExitCode;
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
fn wave_routes(decoder: &LdpcBatchDecoder, batch: usize, workers: NonZeroUsize) -> Vec<WaveRoute> {
    let last = batch % LANES;
    std::iter::once(batch.min(LANES))
        .chain((batch > LANES && last > 0).then_some(last))
        .map(|frames| WaveRoute {
            frames,
            route: format!("{:?}", decoder.batch_route_with_workers(frames, workers)),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WaveRoute {
    frames: usize,
    route: String,
}

/// Untimed diagnostics, journalled by the runner as a child diagnostic.
#[derive(Debug, Serialize)]
struct Diagnostics<'a> {
    kind: &'static str,
    arm: &'a str,
    tuning_profile: &'a EnvelopeIdentity,
    /// The routes of the first worker's decoder.
    wave_routes: Vec<WaveRoute>,
    lane_min_frames: usize,
    lane_max_edges: usize,
    lane_max_workers: usize,
    granules_per_task: usize,
    batch_frames: usize,
    wave_frames: usize,
    /// Lane workspace bytes each worker's decoder reports, in worker order.
    workspace_bytes: Vec<usize>,
    conversion: arm::ConversionCosts,
}

struct TunedProfile {
    envelope: EnvelopeIdentity,
}

impl ArmPolicy for TunedProfile {
    type Observation = Vec<WaveRoute>;

    fn build_decoder(
        &self,
        code: &LdpcCode,
        config: DecoderConfig,
        cap: usize,
        _: usize,
    ) -> Result<LdpcBatchDecoder, String> {
        Ok(LdpcBatchDecoder::new(
            code.clone(),
            config,
            cap,
            DecodedWord::Codeword,
        ))
    }

    fn observe(
        &self,
        decoder: &LdpcBatchDecoder,
        batch: usize,
        workers: NonZeroUsize,
    ) -> Self::Observation {
        wave_routes(decoder, batch, workers)
    }

    fn report(
        &self,
        context: CellContext<'_>,
        observations: Vec<Self::Observation>,
        workspace_bytes: Vec<usize>,
        cell: &mut CellRun,
    ) -> Result<String, String> {
        let batch = context.batch;
        let selectors = tuning::active().section.decode().clone();
        let routes = observations.into_iter().next().expect("first worker");
        let selected_path = format!(
            "gf2-coding LdpcBatchDecoder f32 flooding normalized min-sum, default construction \
         under tuning profile {} (lane_min_frames {} lane_max_edges {}), wave routes {}, \
         batches of {batch} frames in waves of {LANES}, lane workspace {} bytes per worker, \
         reused decoder per worker; {} pinned workers, {} process threads",
            self.envelope.id,
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
            tuning_profile: &self.envelope,
            wave_routes: routes,
            lane_min_frames: selectors.lane_min_frames(),
            lane_max_edges: selectors.lane_max_edges(),
            lane_max_workers: selectors.lane_max_workers().get(),
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

        Ok(selected_path)
    }
}

fn run() -> Result<(), String> {
    let envelope = install_envelope()?;
    run_with_policy(ARM, TunedProfile { envelope })
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
    use super::{
        install_envelope, wave_routes, ArmPolicy, CodingTuning, CodingTuningCodec, NonZeroUsize,
        ProfileRegistryBuilder, TunedProfile, ENVELOPE, LANES,
    };
    use gf2_coding::ldpc::{BatchRoute, DecodedWord, LdpcBatchDecoder};
    use gf2_coding::test_support::{ldpc_batch_code, ldpc_batch_frames, LDPC_BATCH_CAP};
    use gf2_coding::tuning::{self, DecodeSelectors};
    use ldpc_throughput::gf2::{config, Gf2Worker};

    #[test]
    fn committed_profile_routes_multicore_cells_to_single_frame() {
        let registry = ProfileRegistryBuilder::new()
            .register::<CodingTuning, CodingTuningCodec>()
            .unwrap()
            .build()
            .unwrap();
        let prepared = registry.from_json(ENVELOPE).unwrap();
        let section = prepared.section::<CodingTuning>().unwrap().unwrap().section;
        let decoder = LdpcBatchDecoder::with_selectors(
            ldpc_batch_code(),
            config(0.75, true),
            LDPC_BATCH_CAP,
            DecodedWord::Codeword,
            section.decode().clone(),
        );
        for workers in [6, 12, 24] {
            assert!(
                wave_routes(&decoder, LANES + 1, NonZeroUsize::new(workers).unwrap())
                    .iter()
                    .all(|wave| wave.route == format!("{:?}", BatchRoute::SingleFrame))
            );
        }
    }

    #[test]
    fn default_construction_decodes_under_the_installed_envelope() {
        let envelope = install_envelope().expect("install");
        let committed: serde_json::Value = serde_json::from_str(ENVELOPE).expect("envelope");
        assert_eq!(envelope.id, committed["profile_id"]);
        assert!(install_envelope().is_err(), "the tuning installs once");
        let policy = TunedProfile { envelope };

        let code = ldpc_batch_code();
        let frames = ldpc_batch_frames();
        let llrs: Vec<f32> = frames.iter().flatten().map(|llr| llr.value()).collect();
        let decoder_config = config(0.75, true);
        for batch in [1, LANES - 1, LANES, LANES + 1, frames.len()] {
            let llrs = &llrs[..batch * code.n()];
            let mut worker = ldpc_batch_arms::BatchWorker::from_decoder(
                &code,
                batch,
                policy
                    .build_decoder(&code, decoder_config, LDPC_BATCH_CAP, batch)
                    .unwrap(),
            )
            .expect("worker");
            assert_eq!(
                worker.decoder().selectors(),
                tuning::active().section.decode()
            );
            let mut reference = Gf2Worker::new(&code, decoder_config, batch, LDPC_BATCH_CAP);
            reference.decode_batch(llrs);
            for workers in [1, 6, 12, 24] {
                worker.decode_batch_with_workers(llrs, NonZeroUsize::new(workers).unwrap());
                assert_eq!(
                    worker.decisions, reference.decisions,
                    "batch {batch} workers {workers}"
                );
            }
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
            wave_routes(&decoder, batch, NonZeroUsize::MIN)
                .iter()
                .map(|wave| wave.frames)
                .collect()
        };
        assert_eq!(sizes(3), [3]);
        assert_eq!(sizes(LANES), [LANES]);
        assert_eq!(sizes(2 * LANES), [LANES]);
        assert_eq!(sizes(2 * LANES + 3), [LANES, 3]);
        let route = format!("{:?}", BatchRoute::SingleFrame);
        assert!(wave_routes(&decoder, LANES + 1, NonZeroUsize::MIN)
            .iter()
            .all(|wave| wave.route == route));
    }
}
