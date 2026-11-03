//! Batch decoder arms under the committed coding tuning envelope and under the
//! candidate envelope of the QC route's confirmation.

use crate::arm;
use crate::batch::{probe_stages, run_with_policy, ArmPolicy, CellContext, STAGE_PROBES};
use crate::driver::CellRun;
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode, QuasiCyclicLdpc,
};
use gf2_coding::tuning::{self, CodingTuning, CodingTuningCodec};
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
    "../../../crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json"
);

/// The committed envelope's lane selectors with the QC route of `qc-go-rule`
/// selected, embedded so the executable digest pins it. Its section is
/// inherited because the predicate has no measured threshold.
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
    attach_qc: bool,
}

/// Checks that every worker decoded the route its decoder reported before
/// timing, and that the QC route ran on exactly the installed cases, so that
/// another route cannot stand in for it. Returns the executed routes.
fn verify_executed_routes(
    context: &CellContext<'_>,
    observations: &[Vec<WaveRoute>],
    cases: &[gf2_coding::tuning::QcRouteCase],
) -> Result<Vec<BatchRoute>, String> {
    let workers = context.executed_routes.len();
    let selected = context.code.quasi_cyclic().is_some()
        && cases
            .iter()
            .any(|case| case.workers().get() == workers && case.frames().get() == context.batch);
    let mut executed = Vec::with_capacity(workers);
    for (worker, (route, reported)) in context.executed_routes.iter().zip(observations).enumerate()
    {
        let route = route.ok_or_else(|| format!("worker {worker} decoded no wave"))?;
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

    /// A candidate arm retains the NR BG1 Z=384 structure on the code it
    /// decodes only when the lifted matrix equals the corpus matrix exactly.
    fn prepare_code(&self, mut code: LdpcCode, corpus: &str) -> Result<LdpcCode, String> {
        if self.attach_qc
            && !code.attach_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384))
            && corpus == QC_CORPUS
        {
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
        let executed = verify_executed_routes(&context, &observations, selectors.qc_route_cases())?;
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
         under tuning profile {} (lane_min_frames {} lane_max_edges {}), QC structure attached \
         {}, wave routes {}, executed routes by worker {:?}, batches of {batch} frames in waves of {LANES}, lane workspace {} bytes per worker, \
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

/// Runs the production decoder arm.
///
/// # Errors
/// Returns envelope installation or shared arm errors, and an error when the
/// waves of a batch decode on several routes, which the stage probe refuses.
pub fn run() -> Result<(), String> {
    let envelope = install_envelope(ENVELOPE)?;
    run_with_policy(
        ARM,
        TunedProfile {
            arm: ARM,
            envelope,
            attach_qc: false,
        },
    )
}

/// Runs the candidate arm: the production decoder under
/// [`CANDIDATE_ENVELOPE`], with QC structure attached outside timing.
///
/// # Errors
/// As [`run`], and an error when the NR corpus matrix differs from NR BG1
/// Z=384 or a worker decodes other than the route the envelope selects.
pub fn run_candidate() -> Result<(), String> {
    let envelope = install_envelope(CANDIDATE_ENVELOPE)?;
    run_with_policy(
        CANDIDATE_ARM,
        TunedProfile {
            arm: CANDIDATE_ARM,
            envelope,
            attach_qc: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{install_envelope, wave_routes, ArmPolicy, NonZeroUsize, TunedProfile, LANES};
    use super::{CANDIDATE_ARM, CANDIDATE_ENVELOPE, ENVELOPE, QC_CORPUS};
    use crate::arm::ConversionCosts;
    use crate::batch::CellContext;
    use crate::driver::CellRun;
    use crate::reference::{config, Gf2Worker};
    use crate::workload::Threads;
    use gf2_coding::ldpc::{
        BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, LaneBackend, LdpcBatchDecoder,
        LdpcCode, QuasiCyclicLdpc,
    };
    use gf2_coding::llr::Llr;
    use gf2_coding::test_support::{ldpc_batch_code, ldpc_batch_frames, LDPC_BATCH_CAP};
    use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
    use gf2_coding::traits::BatchSoftDecoder;
    use gf2_coding::tuning::{self, DecodeSelectors, QcRouteCase};
    use gf2_coding::tuning::{CodingTuning, CodingTuningCodec};
    use gf2_core::tuning::ProfileRegistryBuilder;

    fn committed_policy(envelope: super::EnvelopeIdentity) -> TunedProfile {
        TunedProfile {
            arm: "probe",
            envelope,
            attach_qc: false,
        }
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

    fn nr_corpus_code() -> LdpcCode {
        let qc = QuasiCyclicLdpc::nr_5g(1, 384);
        LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols(), &qc.to_edges())
    }

    fn candidate_decoder(code: LdpcCode, config: DecoderConfig, cap: usize) -> LdpcBatchDecoder {
        LdpcBatchDecoder::with_selectors(
            code,
            config,
            cap,
            DecodedWord::Codeword,
            decode_section(CANDIDATE_ENVELOPE),
        )
    }

    fn candidate_policy(envelope: super::EnvelopeIdentity) -> TunedProfile {
        TunedProfile {
            arm: CANDIDATE_ARM,
            envelope,
            attach_qc: true,
        }
    }

    #[test]
    fn the_candidate_envelope_adds_the_frozen_cases_to_the_committed_selectors() {
        let committed = decode_section(ENVELOPE);
        assert!(committed.qc_route_cases().is_empty());
        let case = |workers, frames| QcRouteCase::try_new(1, 384, workers, frames).unwrap();
        assert_eq!(
            decode_section(CANDIDATE_ENVELOPE),
            committed.with_qc_route_cases(vec![case(1, 1), case(12, 8)])
        );
    }

    #[test]
    fn the_candidate_envelope_routes_each_cell_of_the_nr_code_and_keeps_the_others() {
        let policy = candidate_policy(super::EnvelopeIdentity {
            id: "candidate".to_owned(),
            sha256: String::new(),
        });
        let attached = policy.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        let nms = config(0.75, true);
        let workers = |count| NonZeroUsize::new(count).unwrap();
        let qc = BatchRoute::Qc(LaneBackend::detected());
        let lane = BatchRoute::Lane(LaneBackend::detected());
        let decoder = candidate_decoder(attached.clone(), nms, 1);
        for (frames, count, expected) in [
            (1, 1, qc),
            (LANES, 12, qc),
            (LANES, 1, lane),
            (LANES, 6, BatchRoute::SingleFrame),
            (LANES, 24, BatchRoute::SingleFrame),
        ] {
            assert_eq!(
                decoder.batch_route_with_workers(frames, workers(count)),
                expected,
                "{frames} frames, {count} workers"
            );
        }
        for algorithm in [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ] {
            let other = candidate_decoder(attached.clone(), DecoderConfig::new(algorithm, true), 1);
            assert_eq!(other.batch_route(1), BatchRoute::SingleFrame);
            assert_eq!(
                other.batch_route_with_workers(LANES, workers(12)),
                BatchRoute::SingleFrame
            );
        }
        let unattached = candidate_decoder(nr_corpus_code(), nms, 1);
        assert_eq!(unattached.batch_route(1), BatchRoute::SingleFrame);
    }

    #[test]
    fn structure_attaches_only_after_an_exact_comparison() {
        let policy = candidate_policy(super::EnvelopeIdentity {
            id: "candidate".to_owned(),
            sha256: String::new(),
        });
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

        let other = policy
            .prepare_code(
                LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2),
                "dvb-t2-r12",
            )
            .unwrap();
        assert!(other.quasi_cyclic().is_none());

        let committed = committed_policy(super::EnvelopeIdentity {
            id: "committed".to_owned(),
            sha256: String::new(),
        });
        let untouched = committed.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        assert!(untouched.quasi_cyclic().is_none());
    }

    /// A report on one cell of the NR code under the installed candidate
    /// envelope, with `executed` as each worker's last route.
    fn candidate_report(
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
                arm_name: CANDIDATE_ARM,
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
    fn the_candidate_report_names_every_worker_route_and_refuses_another_route() {
        let envelope = install_envelope(CANDIDATE_ENVELOPE).expect("install");
        let policy = candidate_policy(envelope);
        let attached = policy.prepare_code(nr_corpus_code(), QC_CORPUS).unwrap();
        let qc = BatchRoute::Qc(LaneBackend::detected());
        let lane = BatchRoute::Lane(LaneBackend::detected());

        let path = candidate_report(&policy, &attached, LANES, 12, vec![Some(qc); 12]).unwrap();
        assert_eq!(path.matches(&format!("{qc:?}")).count(), 13, "{path}");
        assert!(path.contains("QC structure attached true"), "{path}");
        let path = candidate_report(&policy, &attached, 1, 1, vec![Some(qc)]).unwrap();
        assert!(path.contains(&format!("{qc:?}")), "{path}");
        let path = candidate_report(&policy, &attached, LANES, 1, vec![Some(lane)]).unwrap();
        assert!(!path.contains("Qc("), "{path}");

        // A single worker off the QC route refuses the cell.
        let mut one_off = vec![Some(qc); 12];
        one_off[7] = Some(BatchRoute::SingleFrame);
        let refused = candidate_report(&policy, &attached, LANES, 12, one_off).unwrap_err();
        assert!(refused.contains("worker 7"), "{refused}");

        // Structure that never attached leaves the cell off the QC route, and
        // such a run reports it.
        let unattached = nr_corpus_code();
        let path = candidate_report(
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

    #[test]
    fn the_candidate_route_satisfies_the_batch_contract_under_the_installed_envelope() {
        install_envelope(CANDIDATE_ENVELOPE).expect("install");
        let qc = QuasiCyclicLdpc::nr_5g(1, 384);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let nms = config(0.75, true);
        let mut single = gf2_coding::ldpc::LdpcDecoder::with_config(code.clone(), nms);
        // One frame per call: each call is the selected one-worker case.
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
                decoded: &mut [gf2_core::BitVec],
                outcomes: &mut [gf2_coding::traits::DecodeOutcome],
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
        let frames: Vec<Vec<Llr>> = (0..4)
            .map(|frame: usize| {
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
            .collect();
        let mut decoder = OneFrameCalls(LdpcBatchDecoder::new(code, nms, 2, DecodedWord::Codeword));
        batch_soft_decoder_contract(
            &mut decoder,
            |llrs| single.decode_to_codeword(llrs, 2),
            &frames,
            ConvergenceMix::Any,
        );
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
        let envelope = install_envelope(ENVELOPE).expect("install");
        let committed: serde_json::Value = serde_json::from_str(ENVELOPE).expect("envelope");
        assert_eq!(envelope.id, committed["profile_id"]);
        assert!(
            install_envelope(ENVELOPE).is_err(),
            "the tuning installs once"
        );
        let policy = committed_policy(envelope);

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
        let envelope = install_envelope(ENVELOPE).expect("install");
        let policy = committed_policy(envelope);
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
