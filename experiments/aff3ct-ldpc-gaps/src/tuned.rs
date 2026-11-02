//! Batch decoder arm under the committed coding tuning envelope.

use crate::arm;
use crate::batch::{probe_stages, run_with_policy, ArmPolicy, CellContext, STAGE_PROBES};
use crate::driver::CellRun;
use gf2_coding::ldpc::{BatchRoute, DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode};
use gf2_coding::tuning::{self, CodingTuning, CodingTuningCodec};
use gf2_core::tuning::{ProfileRegistryBuilder, SectionResolution};
use gf2_kernels_simd::ldpc_lanes::LANES;
use serde::{Serialize, Serializer};
use std::num::NonZeroUsize;
use tuning_campaign_support::protocol::sha256_hex;

const ARM: &str = "gf2-batch-tuned";

/// The committed coding owner envelope, embedded so the executable digest
/// pins it.
pub const ENVELOPE: &str = include_str!(
    "../../../crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json"
);

/// The installed envelope as a plan's `tuning_profile` names it.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct EnvelopeIdentity {
    pub id: String,
    pub sha256: String,
}

/// Installs [`ENVELOPE`] as the process tuning and returns its identity.
///
/// # Errors
///
/// Returns an error when the envelope does not parse, when the process
/// tuning has already resolved, or when the coding section does not resolve
/// from the envelope.
pub fn install_envelope() -> Result<EnvelopeIdentity, String> {
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
            route: decoder.batch_route_with_workers(frames, workers),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WaveRoute {
    frames: usize,
    #[serde(serialize_with = "debug_name")]
    route: BatchRoute,
}

fn debug_name<S: Serializer>(route: &BatchRoute, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("{route:?}"))
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
    /// Probes behind each of `pack_ns`, `batch_fill_ns` and `unpack_ns`.
    stage_probes: usize,
    stage_statistic: &'static str,
    /// `batch_fill_ns` is the decoder call of [`probe_stages`].
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
        let CellContext {
            code,
            decoder_config,
            batch,
            llrs,
            ..
        } = context;
        let selectors = tuning::active().section.decode().clone();
        let routes = observations.into_iter().next().expect("first worker");
        // The probe decodes on one worker, so it takes the route the cell's
        // worker count selects rather than the envelope's one-worker route.
        let route = match routes.as_slice() {
            [first, rest @ ..] if rest.iter().all(|wave| wave.route == first.route) => first.route,
            _ => {
                return Err(format!(
                    "the waves decode on several routes {routes:?}; the stage probe takes one"
                ))
            }
        };
        let stages = probe_stages(code, decoder_config, batch, route, llrs)?;
        cell.conversion.pack_ns = stages.pack_ns;
        cell.conversion.batch_fill_ns = stages.decode_ns;
        cell.conversion.unpack_ns = stages.extract_ns;
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
                .map(|wave| format!("{} frames {:?}", wave.frames, wave.route))
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
            stage_probes: STAGE_PROBES,
            stage_statistic: "median",
            conversion: cell.conversion,
        };
        eprintln!(
            "{}",
            serde_json::to_string(&diagnostics).map_err(|e| e.to_string())?
        );

        Ok(selected_path)
    }
}

/// Runs the production decoder arm.
///
/// # Errors
/// Returns envelope installation or shared arm errors, and an error when the
/// waves of a batch decode on several routes, which the stage probe refuses.
pub fn run() -> Result<(), String> {
    let envelope = install_envelope()?;
    run_with_policy(ARM, TunedProfile { envelope })
}

#[cfg(test)]
mod tests {
    use super::ENVELOPE;
    use super::{install_envelope, wave_routes, ArmPolicy, NonZeroUsize, TunedProfile, LANES};
    use crate::arm::ConversionCosts;
    use crate::batch::CellContext;
    use crate::driver::CellRun;
    use crate::reference::{config, Gf2Worker};
    use crate::workload::Threads;
    use gf2_coding::ldpc::{BatchRoute, DecodedWord, LdpcBatchDecoder};
    use gf2_coding::test_support::{ldpc_batch_code, ldpc_batch_frames, LDPC_BATCH_CAP};
    use gf2_coding::tuning::{self, DecodeSelectors};
    use gf2_coding::tuning::{CodingTuning, CodingTuningCodec};
    use gf2_core::tuning::ProfileRegistryBuilder;

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
                    .all(|wave| wave.route == BatchRoute::SingleFrame)
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
            let mut worker = crate::batch::BatchWorker::from_decoder(
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
    fn the_report_probes_the_stages_of_the_observed_route() {
        let envelope = install_envelope().expect("install");
        let policy = TunedProfile { envelope };
        let code = ldpc_batch_code();
        let frames = ldpc_batch_frames();
        let llrs: Vec<f32> = frames.iter().flatten().map(|llr| llr.value()).collect();
        let decoder_config = config(0.75, true);
        let decoder = policy
            .build_decoder(&code, decoder_config, LDPC_BATCH_CAP, LANES)
            .unwrap();
        let report = |batch: usize, workers: usize| {
            let workers = NonZeroUsize::new(workers).unwrap();
            let mut cell = CellRun {
                windows: Vec::new(),
                conversion: ConversionCosts {
                    setup_ns: 0,
                    pack_ns: 0,
                    unpack_ns: 0,
                    batch_fill_ns: 0,
                    dispatch_ns: 0,
                },
                workers_observed: workers.get() as u32,
                threads: Threads { ready: 1, after: 1 },
            };
            let context = CellContext {
                arm_name: "probe",
                code: &code,
                decoder_config,
                batch,
                llrs: &llrs[..batch * code.n()],
            };
            policy
                .report(
                    context,
                    vec![wave_routes(&decoder, batch, workers)],
                    vec![0],
                    &mut cell,
                )
                .map(|_| cell.conversion)
        };
        // One frame, full waves on the lane route, and full waves the six
        // workers route to the single-frame decoder.
        for (batch, workers) in [(1, 1), (2 * LANES, 1), (LANES, 6)] {
            let conversion = report(batch, workers).expect("probed");
            assert!(
                conversion.batch_fill_ns > 0,
                "batch {batch} workers {workers}"
            );
        }
        let mixed = report(LANES + 1, 1).expect_err("a lane wave and a single-frame wave");
        assert!(mixed.contains("routes"), "{mixed}");
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
        assert!(wave_routes(&decoder, LANES + 1, NonZeroUsize::MIN)
            .iter()
            .all(|wave| wave.route == BatchRoute::SingleFrame));
    }
}
