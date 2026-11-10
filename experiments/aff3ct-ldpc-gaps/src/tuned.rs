//! Batch decoder arms under the committed coding tuning envelope and under the
//! candidate envelope of the QC route's confirmation.

use crate::arm;
use crate::batch::{probe_stages, run_with_policy, ArmPolicy, CellContext, STAGE_PROBES};
use crate::driver::CellRun;
use gf2_coding::ldpc::{
    decide_batch_route, BatchRoute, DecodedWord, DecoderConfig, EdgeLayout, LaneBackend,
    LaneCapability, LdpcBatchDecoder, LdpcCode, QcCapability, QuasiCyclicLdpc,
};
use gf2_coding::tuning::{self, CodingTuning, CodingTuningCodec, DecodeSelectors};
use gf2_core::tuning::{ProfileRegistryBuilder, SectionResolution};
use gf2_kernels_simd::ldpc_lanes::LANES;
use serde::{Serialize, Serializer};
use std::num::NonZeroUsize;
use tuning_campaign_support::protocol::sha256_hex;

const ARM: &str = "gf2-batch-tuned";
const CANDIDATE_ARM: &str = "gf2-batch-qc-candidate";

/// The corpus code of the QC route, as a case names it.
const QC_CORPUS: &str = "nr-bg1-r12";

/// The committed coding owner envelope, embedded so the executable digest
/// pins it.
pub const ENVELOPE: &str = include_str!(
    "../../../crates/gf2-coding/data/tuning-profiles/gf2-c62356d2-20261110t053902z-287464.json"
);

/// The candidate envelope `55c3e433-integrated-adoption` confirmed: the
/// selectors of [`ENVELOPE`] with open QC route bounds, under an inherited
/// measurement, embedded so the executable digest pins it.
pub const CANDIDATE_ENVELOPE: &str = include_str!("../tuning-profiles/qc-route-candidate.json");

/// The installed envelope as a plan's `tuning_profile` names it.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct EnvelopeIdentity {
    pub id: String,
    pub sha256: String,
}

/// Installs `envelope` as the process tuning and returns its identity.
///
/// # Errors
///
/// Returns an error when the envelope does not parse, when the process
/// tuning has already resolved, or when the coding section does not resolve
/// from the envelope.
pub fn install_envelope(envelope: &str) -> Result<EnvelopeIdentity, String> {
    let registry = ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .and_then(ProfileRegistryBuilder::build)
        .map_err(|error| error.to_string())?;
    let prepared = registry
        .from_json(envelope)
        .map_err(|error| error.to_string())?;
    gf2_core::tuning::install(prepared).map_err(|error| error.to_string())?;
    match tuning::active().resolution {
        SectionResolution::Installed { profile_id, .. } => Ok(EnvelopeIdentity {
            id: profile_id.to_string(),
            sha256: sha256_hex(envelope.as_bytes()),
        }),
        other => Err(format!("the coding section resolves as {other:?}")),
    }
}

/// The route production selects for the first wave of a batch of `batch`
/// frames, from the whole batch's context. A shorter final wave takes a route
/// of its own, which the decoder reports only as it decodes.
fn wave_routes(decoder: &LdpcBatchDecoder, batch: usize, workers: NonZeroUsize) -> Vec<WaveRoute> {
    vec![WaveRoute {
        frames: batch.min(LANES),
        route: decoder.batch_route_with_workers(batch, workers),
    }]
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
    qc_structure_attached: bool,
    /// The routes of the first worker's decoder.
    wave_routes: Vec<WaveRoute>,
    /// The route of each worker's last decoded wave, in worker order.
    executed_routes: Vec<String>,
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
    arm: &'static str,
    envelope: EnvelopeIdentity,
}

/// Checks that every worker decoded the route its decoder reported before
/// timing, and that the QC route ran exactly where [`decide_batch_route`]
/// selects it under the installed `selectors`, so that another route cannot
/// stand in for it. Returns the executed routes.
fn verify_executed_routes(
    context: &CellContext<'_>,
    observations: &[Vec<WaveRoute>],
    selectors: &DecodeSelectors,
) -> Result<Vec<BatchRoute>, String> {
    let workers = context.executed_routes.len();
    let backend = LaneBackend::detected();
    let qc = match context.code.quasi_cyclic() {
        Some(structure) => QcCapability::Present {
            lifting_size: structure.expansion_factor(),
            backend,
        },
        None => QcCapability::Absent,
    };
    let documented = decide_batch_route(
        selectors,
        context.decoder_config.algorithm(),
        LaneCapability::Present(backend),
        qc,
        context.batch.min(LANES),
        EdgeLayout::from_parity_check(context.code.parity_check_matrix()).edges(),
        NonZeroUsize::new(workers).ok_or("a cell runs on at least one worker")?,
    );
    let selected = matches!(documented, BatchRoute::Qc(_));
    let mut executed = Vec::with_capacity(workers);
    for (worker, (route, reported)) in context.executed_routes.iter().zip(observations).enumerate()
    {
        let route = route.ok_or_else(|| format!("worker {worker} decoded no wave"))?;
        // A final wave that takes another route than the first leaves the
        // stage probe without one route to measure.
        if reported.last().map(|wave| wave.route) != Some(route) {
            return Err(format!(
                "worker {worker} decoded on {route:?} after reporting {reported:?}"
            ));
        }
        if matches!(route, BatchRoute::Qc(_)) != selected {
            return Err(format!(
                "worker {worker} decoded on {route:?}; the QC route is selected: {selected}"
            ));
        }
        executed.push(route);
    }
    Ok(executed)
}

impl ArmPolicy for TunedProfile {
    type Observation = Vec<WaveRoute>;

    /// The arm retains the NR BG1 Z=384 structure on the code it decodes
    /// only when the lifted matrix equals the corpus matrix exactly.
    fn prepare_code(&self, mut code: LdpcCode, corpus: &str) -> Result<LdpcCode, String> {
        if !code.attach_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384)) && corpus == QC_CORPUS {
            return Err(format!(
                "the parity-check matrix of {corpus} differs from NR BG1 Z=384"
            ));
        }
        Ok(code)
    }

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
        let selectors = tuning::active().section.decode().clone();
        let executed = verify_executed_routes(&context, &observations, &selectors)?;
        let CellContext {
            code,
            decoder_config,
            batch,
            llrs,
            ..
        } = context;
        let routes = observations.into_iter().next().expect("first worker");
        // The probe decodes on one worker, so it takes the route the cell's
        // worker count selects rather than the envelope's one-worker route.
        let route = routes.first().expect("a first wave").route;
        let stages = probe_stages(code, decoder_config, batch, route, llrs)?;
        cell.conversion.pack_ns = stages.pack_ns;
        cell.conversion.batch_fill_ns = stages.decode_ns;
        cell.conversion.unpack_ns = stages.extract_ns;
        let selected_path = format!(
            "gf2-coding LdpcBatchDecoder f32 flooding normalized min-sum, default construction \
         under tuning profile {} (lane_min_frames {} lane_max_edges {}), QC structure attached \
         {}, first-wave route {}, executed routes by worker {:?}, batches of {batch} frames in waves of {LANES}, lane workspace {} bytes per worker, \
         reused decoder per worker; {} pinned workers, {} process threads",
            self.envelope.id,
            selectors.lane_min_frames(),
            selectors.lane_max_edges(),
            code.quasi_cyclic().is_some(),
            routes
                .iter()
                .map(|wave| format!("{} frames {:?}", wave.frames, wave.route))
                .collect::<Vec<_>>()
                .join(", "),
            executed,
            workspace_bytes.first().copied().unwrap_or_default(),
            cell.workers_observed,
            cell.threads.ready
        );
        let diagnostics = Diagnostics {
            kind: "batch-decoder-diagnostics",
            arm: self.arm,
            tuning_profile: &self.envelope,
            qc_structure_attached: code.quasi_cyclic().is_some(),
            wave_routes: routes,
            executed_routes: executed.iter().map(|route| format!("{route:?}")).collect(),
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

/// Runs the production decoder arm under [`ENVELOPE`], with QC structure
/// attached outside timing.
///
/// # Errors
/// Returns envelope installation or shared arm errors, an error when the
/// waves of a batch decode on several routes, which the stage probe refuses,
/// and an error when the NR corpus matrix differs from NR BG1 Z=384 or a
/// worker decodes other than the route the envelope selects.
pub fn run() -> Result<(), String> {
    let envelope = install_envelope(ENVELOPE)?;
    run_with_policy(ARM, TunedProfile { arm: ARM, envelope })
}

/// Runs the candidate arm: the production decoder under
/// [`CANDIDATE_ENVELOPE`], with QC structure attached outside timing.
///
/// # Errors
/// As [`run`].
pub fn run_candidate() -> Result<(), String> {
    let envelope = install_envelope(CANDIDATE_ENVELOPE)?;
    run_with_policy(
        CANDIDATE_ARM,
        TunedProfile {
            arm: CANDIDATE_ARM,
            envelope,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{install_envelope, wave_routes, ArmPolicy, NonZeroUsize, TunedProfile, LANES};
    use super::{ARM, CANDIDATE_ENVELOPE, ENVELOPE, QC_CORPUS};
    use crate::arm::ConversionCosts;
    use crate::batch::CellContext;
    use crate::driver::CellRun;
    use crate::reference::{config, Gf2Worker};
    use crate::workload::Threads;
    use gf2_coding::ldpc::{
        BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, LaneBackend, LdpcBatchDecoder,
        LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
    };
    use gf2_coding::llr::Llr;
    use gf2_coding::test_support::{ldpc_batch_code, ldpc_batch_frames, LDPC_BATCH_CAP};
    use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
    use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
    use gf2_coding::tuning::QcRouteBounds;
    use gf2_coding::tuning::{self, DecodeSelectors};
    use gf2_coding::tuning::{CodingTuning, CodingTuningCodec};
    use gf2_core::tuning::ProfileRegistryBuilder;
    use gf2_core::BitVec;

    fn policy(envelope: super::EnvelopeIdentity) -> TunedProfile {
        TunedProfile { arm: ARM, envelope }
    }

    fn unnamed_policy() -> TunedProfile {
        policy(super::EnvelopeIdentity {
            id: "probe".to_owned(),
            sha256: String::new(),
        })
    }

    fn decode_section(envelope: &str) -> DecodeSelectors {
        let registry = ProfileRegistryBuilder::new()
            .register::<CodingTuning, CodingTuningCodec>()
            .unwrap()
            .build()
            .unwrap();
        let prepared = registry.from_json(envelope).unwrap();
        let section = prepared.section::<CodingTuning>().unwrap().unwrap().section;
        section.decode().clone()
    }

    /// The LDPC batch suite's code without its QC structure, whose waves the
    /// committed lane bounds divide between the lane and single-frame routes.
    fn batch_code() -> LdpcCode {
        ldpc_batch_code().without_quasi_cyclic()
    }

    fn nr_corpus_code() -> LdpcCode {
        let qc = QuasiCyclicLdpc::nr_5g(1, 384);
        LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols(), &qc.to_edges())
    }

    fn committed_decoder(code: LdpcCode, config: DecoderConfig, cap: usize) -> LdpcBatchDecoder {
        LdpcBatchDecoder::with_selectors(
            code,
            config,
            cap,
            DecodedWord::Codeword,
            decode_section(ENVELOPE),
        )
    }

    #[test]
    fn the_committed_envelope_selects_the_candidates_decode_family_and_qc_bounds() {
        let committed = decode_section(ENVELOPE);
        let candidate = decode_section(CANDIDATE_ENVELOPE);
        assert_eq!(candidate.qc_route(), QcRouteBounds::OPEN);
        assert_ne!(committed.qc_route(), QcRouteBounds::OPEN);
        assert_eq!(committed, candidate.with_qc_route(committed.qc_route()));
    }

    #[test]
    fn the_committed_envelope_routes_every_min_sum_cell_of_the_attached_nr_code_to_qc() {
        let attached = unnamed_policy()
            .prepare_code(nr_corpus_code(), QC_CORPUS)
            .unwrap();
        let workers = |count| NonZeroUsize::new(count).unwrap();
        let qc = BatchRoute::Qc(LaneBackend::detected());
        for algorithm in [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            DecoderAlgorithm::OffsetMinSum(0.5),
        ] {
            let decoder =
                committed_decoder(attached.clone(), DecoderConfig::new(algorithm, true), 1);
            for frames in [1, 2, LANES, LANES + 1] {
                for count in [1, 6, 12, 24] {
                    assert_eq!(
                        decoder.batch_route_with_workers(frames, workers(count)),
                        qc,
                        "{algorithm:?}, {frames} frames, {count} workers"
                    );
                }
            }
        }
        let sum_product = committed_decoder(
            attached,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            1,
        );
        assert_eq!(sum_product.batch_route(1), BatchRoute::SingleFrame);
        let unattached = committed_decoder(nr_corpus_code(), config(0.75, true), 1);
        assert_eq!(unattached.batch_route(1), BatchRoute::SingleFrame);
        assert_eq!(
            unattached.batch_route(LANES),
            BatchRoute::Lane(LaneBackend::detected())
        );
    }

    #[test]
    fn structure_attaches_only_after_an_exact_comparison() {
        let policy = unnamed_policy();
        let prepared = policy.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        assert_eq!(
            prepared.quasi_cyclic(),
            Some(&QuasiCyclicLdpc::nr_5g(1, 384))
        );

        let qc = QuasiCyclicLdpc::nr_5g(1, 384);
        let mut edges = qc.to_edges();
        edges.pop();
        let altered = LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols(), &edges);
        let refused = policy
            .prepare_code(altered, QC_CORPUS)
            .expect_err("refused");
        assert!(refused.contains("differs"), "{refused}");

        // Another corpus code keeps the structure it retains.
        let dvb = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
        let other = policy.prepare_code(dvb.clone(), "dvb-t2-r12").unwrap();
        assert_eq!(other.quasi_cyclic(), dvb.quasi_cyclic());
    }

    /// A report on one cell of `code` under the installed envelope, with
    /// `executed` as each worker's last route.
    fn cell_report(
        policy: &TunedProfile,
        code: &LdpcCode,
        batch: usize,
        workers: usize,
        executed: Vec<Option<BatchRoute>>,
    ) -> Result<String, String> {
        let decoder_config = config(0.75, true);
        let decoder = policy
            .build_decoder(code, decoder_config, LDPC_BATCH_CAP, batch)
            .unwrap();
        let observed = wave_routes(&decoder, batch, NonZeroUsize::new(workers).unwrap());
        let llrs = vec![2.0; batch * code.n()];
        let mut cell = CellRun {
            windows: Vec::new(),
            conversion: ConversionCosts {
                setup_ns: 0,
                pack_ns: 0,
                unpack_ns: 0,
                batch_fill_ns: 0,
                dispatch_ns: 0,
            },
            workers_observed: workers as u32,
            threads: Threads { ready: 1, after: 1 },
        };
        policy.report(
            CellContext {
                arm_name: ARM,
                code,
                decoder_config,
                batch,
                llrs: &llrs,
                executed_routes: &executed,
            },
            vec![observed; workers],
            vec![0; workers],
            &mut cell,
        )
    }

    #[test]
    fn the_report_names_every_worker_route_and_refuses_another_route() {
        let envelope = install_envelope(ENVELOPE).expect("install");
        let policy = policy(envelope);
        let attached = policy.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        let qc = BatchRoute::Qc(LaneBackend::detected());
        let lane = BatchRoute::Lane(LaneBackend::detected());

        let path = cell_report(&policy, &attached, LANES, 12, vec![Some(qc); 12]).unwrap();
        assert_eq!(path.matches(&format!("{qc:?}")).count(), 13, "{path}");
        assert!(path.contains("QC structure attached true"), "{path}");
        let path = cell_report(&policy, &attached, 1, 1, vec![Some(qc)]).unwrap();
        assert!(path.contains(&format!("{qc:?}")), "{path}");
        let refused = cell_report(&policy, &attached, LANES, 1, vec![Some(lane)]).unwrap_err();
        assert!(refused.contains("worker 0"), "{refused}");

        // A single worker off the QC route refuses the cell.
        let mut one_off = vec![Some(qc); 12];
        one_off[7] = Some(BatchRoute::SingleFrame);
        let refused = cell_report(&policy, &attached, LANES, 12, one_off).unwrap_err();
        assert!(refused.contains("worker 7"), "{refused}");

        // Structure that never attached leaves the cell off the QC route, and
        // such a run reports it.
        let unattached = nr_corpus_code();
        let path = cell_report(
            &policy,
            &unattached,
            1,
            1,
            vec![Some(BatchRoute::SingleFrame)],
        )
        .unwrap();
        assert!(path.contains("QC structure attached false"), "{path}");
        assert!(!path.contains("Qc("), "{path}");
    }

    /// One frame per call, each on the QC route.
    struct OneFrameCalls(LdpcBatchDecoder);

    impl BatchSoftDecoder for OneFrameCalls {
        fn frame_len(&self) -> usize {
            self.0.frame_len()
        }
        fn decoded_len(&self) -> usize {
            self.0.decoded_len()
        }
        fn batch_granularity(&self) -> usize {
            1
        }
        fn decode_batch(
            &mut self,
            frames: &[&[Llr]],
            decoded: &mut [BitVec],
            outcomes: &mut [DecodeOutcome],
        ) {
            assert_eq!(frames.len(), decoded.len());
            assert_eq!(frames.len(), outcomes.len());
            for (index, frame) in frames.iter().enumerate() {
                self.0.decode_batch(
                    std::slice::from_ref(frame),
                    &mut decoded[index..=index],
                    &mut outcomes[index..=index],
                );
                assert!(matches!(self.0.last_wave_route(), Some(BatchRoute::Qc(_))));
            }
        }
    }

    fn nr_frames(code: &LdpcCode, count: usize) -> Vec<Vec<Llr>> {
        (0..count)
            .map(|frame| {
                (0..code.n())
                    .map(|i| {
                        Llr::new(if (i + frame).is_multiple_of(31) {
                            -1.0
                        } else {
                            2.0
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// Under the installed committed envelope, default construction decodes
    /// one-frame and twelve-worker calls on the QC route of the detected backend and of the
    /// portable fallback, with the single-frame decoder's results.
    #[test]
    fn the_qc_route_satisfies_the_batch_contract_under_the_installed_envelope() {
        install_envelope(ENVELOPE).expect("install");
        let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
        let nms = config(0.75, true);
        let mut single = LdpcDecoder::with_config(code.clone(), nms);
        let frames = nr_frames(&code, LANES);
        for portable in [false, true] {
            let decoder = |cap| {
                let decoder = LdpcBatchDecoder::new(code.clone(), nms, cap, DecodedWord::Codeword);
                if portable {
                    decoder.with_portable_lanes()
                } else {
                    decoder
                }
            };
            let backend = if portable {
                LaneBackend::Portable
            } else {
                LaneBackend::detected()
            };

            batch_soft_decoder_contract(
                &mut OneFrameCalls(decoder(2)),
                |llrs| single.decode_to_codeword(llrs, 2),
                &frames[..4],
                ConvergenceMix::Any,
            );

            let mut decoder = decoder(2);
            let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
            let mut decoded = vec![BitVec::new(); LANES];
            let mut outcomes = vec![DecodeOutcome::default(); LANES];
            decoder.decode_batch_with_workers(
                &views,
                &mut decoded,
                &mut outcomes,
                NonZeroUsize::new(12).unwrap(),
            );
            assert_eq!(decoder.last_wave_route(), Some(BatchRoute::Qc(backend)));
            for (frame, view) in views.iter().enumerate() {
                let expected = single.decode_to_codeword(view, 2);
                assert_eq!(decoded[frame], expected.decoded_bits, "frame {frame}");
                assert_eq!(outcomes[frame], expected.outcome(), "frame {frame}");
            }
        }
    }

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
            batch_code(),
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
        let envelope = install_envelope(ENVELOPE).expect("install");
        let committed: serde_json::Value = serde_json::from_str(ENVELOPE).expect("envelope");
        assert_eq!(envelope.id, committed["profile_id"]);
        assert!(
            install_envelope(ENVELOPE).is_err(),
            "the tuning installs once"
        );
        let policy = policy(envelope);

        let code = batch_code();
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
        let envelope = install_envelope(ENVELOPE).expect("install");
        let policy = policy(envelope);
        let code = batch_code();
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
            let observed = wave_routes(&decoder, batch, workers);
            let context = CellContext {
                arm_name: "probe",
                code: &code,
                decoder_config,
                batch,
                llrs: &llrs[..batch * code.n()],
                executed_routes: &[observed.last().map(|wave| wave.route)],
            };
            policy
                .report(context, vec![observed], vec![0], &mut cell)
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
        let mixed = cell_report(
            &policy,
            &code,
            LANES + 1,
            1,
            vec![Some(BatchRoute::SingleFrame)],
        )
        .expect_err("a lane wave and a single-frame wave");
        assert!(mixed.contains("decoded on"), "{mixed}");
    }

    #[test]
    fn the_route_report_names_the_first_wave_of_the_batch() {
        let decoder = LdpcBatchDecoder::with_selectors(
            batch_code(),
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
        assert_eq!(sizes(2 * LANES + 3), [LANES]);
        assert!(wave_routes(&decoder, LANES + 1, NonZeroUsize::MIN)
            .iter()
            .all(|wave| wave.route == BatchRoute::SingleFrame));
    }

    /// An attached NR BG1 Z=384 batch longer than one wave takes the QC route
    /// one direct production call of that batch takes, at every worker count;
    /// the arm decodes it and the report accepts it.
    #[test]
    fn a_batch_beyond_one_wave_takes_the_production_route() {
        let envelope = install_envelope(ENVELOPE).expect("install");
        let policy = policy(envelope);
        let attached = policy.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        let nms = config(0.75, true);
        let frames = nr_frames(&attached, 2 * LANES);
        let llrs: Vec<f32> = frames.iter().flatten().map(|llr| llr.value()).collect();
        let mut single = LdpcDecoder::with_config(attached.clone(), nms);
        for workers in [1, 12] {
            let count = NonZeroUsize::new(workers).unwrap();
            let mut direct = LdpcBatchDecoder::new(attached.clone(), nms, 2, DecodedWord::Codeword);
            let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
            let mut decoded = vec![BitVec::new(); views.len()];
            let mut outcomes = vec![DecodeOutcome::default(); views.len()];
            direct.decode_batch_with_workers(&views, &mut decoded, &mut outcomes, count);
            let production = direct.last_wave_route().expect("decoded");
            assert!(matches!(production, BatchRoute::Qc(_)));

            let mut worker = crate::batch::BatchWorker::from_decoder(
                &attached,
                views.len(),
                policy
                    .build_decoder(&attached, nms, 2, views.len())
                    .unwrap(),
            )
            .expect("worker");
            worker.decode_batch_with_workers(&llrs, count);
            assert_eq!(worker.decoder().last_wave_route(), Some(production));
            for (index, view) in views.iter().enumerate() {
                let expected = single.decode_to_codeword(view, 2);
                let decisions = &worker.decisions[index * attached.k()..][..attached.k()];
                let bits: Vec<u8> = (0..attached.k())
                    .map(|bit| u8::from(expected.decoded_bits.get(bit)))
                    .collect();
                assert_eq!(decisions, bits, "frame {index} workers {workers}");
            }

            let path = cell_report(
                &policy,
                &attached,
                views.len(),
                workers,
                vec![Some(production); workers],
            )
            .unwrap();
            assert!(path.contains("Qc("), "{path}");
        }
    }
}
