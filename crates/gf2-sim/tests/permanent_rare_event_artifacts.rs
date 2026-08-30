use gf2_sim::permanent_rare_event::artifact::*;
use gf2_stats::weighted::ScaledStudentInterval;
use num_bigint::BigUint;
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, Barrier};
use tempfile::Builder;

fn digest(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn behavior() -> BehaviorIdentityV1 {
    let closure = BehaviorClosureV1 {
        behavior_schema: BEHAVIOR_CLOSURE_SCHEMA_V1.into(),
        enabled_features: vec!["default".into()],
        repository_inputs: vec![RepositoryInputV1 {
            path: "crates/gf2-sim/src/permanent_rare_event/mod.rs".into(),
            blob_id: "1".repeat(40),
            content_sha256: digest(0x11),
        }],
        cargo_lock_sha256: digest(0x12),
        packages: vec![PackageIdentityV1 {
            package: "rand_chacha".into(),
            version: "0.9.0".into(),
            source: "registry".into(),
            checksum: digest(0x13),
        }],
        environment_input_names: vec!["RAYON_NUM_THREADS".into()],
    };
    BehaviorIdentityV1 {
        sampler: SAMPLER_V1.into(),
        address_to_seed: ADDRESS_TO_SEED_V1.into(),
        rng_algorithm: RNG_V1.into(),
        rng_crate_version: "0.9.0".into(),
        serializer: SERIALIZER_V1.into(),
        estimator_behavior_sha256: sha256_hex(&canonical_bytes(&closure).unwrap()),
        closure,
        executable_sha256: digest(0x14),
        rust_version: "rustc 1.95.0 fixture".into(),
        cargo_version: "cargo 1.95.0 fixture".into(),
        compilation_target: "x86_64-unknown-linux-gnu".into(),
        source_revision: "2".repeat(40),
        source_dirty: false,
    }
}

fn design() -> DesignIdentityV1 {
    DesignIdentityV1 {
        path: "dev/active/3f664839/design.md".into(),
        git_revision: "3".repeat(40),
        blob_id: "4".repeat(40),
        content_sha256: digest(0x15),
    }
}

fn worker() -> WorkerConfigurationV1 {
    WorkerConfigurationV1 {
        requested_workers: 2,
        effective_workers: 2,
        executor_mode: "cpu".into(),
        cpu_affinity: "unpinned".into(),
        work_queue_policy: "canonical-block-queue/v1".into(),
        block_assignment_policy: "round-robin/v1".into(),
        accelerator_selection: "none".into(),
        fallback_policy: "safe-cpu/v1".into(),
        effective_devices: vec![],
    }
}

fn config(scientific_identity: ScientificIdentityV1) -> RareEventConfigurationV1 {
    RareEventConfigurationV1 {
        configuration_schema: CONFIGURATION_SCHEMA_V1.into(),
        artifact_root: "dev/simulation_results/permanent-rare-event".into(),
        design_identity: design(),
        scientific_identity,
        behavior: behavior(),
    }
}

fn identity(scientific: ScientificIdentityV1) -> RareEventDatasetIdentityV1 {
    let configuration = config(scientific.clone());
    RareEventDatasetIdentityV1 {
        campaign: CampaignAuthorityV1::default(),
        preregistration: PreregistrationIdentityV1 {
            design: design(),
            configuration_path: "dev/simulation_results/permanent-rare-event/config.json".into(),
            configuration_schema: CONFIGURATION_SCHEMA_V1.into(),
            configuration_sha256: sha256_hex(&canonical_bytes(&configuration).unwrap()),
        },
        scientific,
        statistical: StatisticalConstantsV1::default(),
        behavior: behavior(),
    }
}

fn host() -> HostObservationV1 {
    let mut host = HostObservationV1 {
        observation_utc: "2026-08-30T08:00:00.000000000Z".into(),
        cpu_architecture: "x86_64".into(),
        cpu_vendor: "fixture-vendor".into(),
        cpu_model: "fixture-model".into(),
        sockets: 1,
        numa_nodes: 1,
        physical_cores: 1,
        logical_cpus: 1,
        online_cpus: vec![0],
        total_ram_bytes: 1024,
        available_ram_bytes: 512,
        os_name: "FixtureOS".into(),
        os_version: "1".into(),
        kernel_release: "fixture".into(),
        kernel_version: "fixture".into(),
        evidence: vec![],
    };
    host.evidence = vec![normalized_host_evidence(&host).unwrap()];
    host
}

fn not_used() -> AcceleratorObservationV1 {
    let reason = "configuration selected CPU";
    AcceleratorObservationV1::NotUsed {
        reason: reason.into(),
        evidence: vec![normalized_accelerator_not_used_evidence(reason).unwrap()],
    }
}

fn used() -> AcceleratorObservationV1 {
    let mut device = GpuObservationV1 {
        model: "fixture-gpu".into(),
        uuid: "GPU-0001".into(),
        pci_address: "0000:01:00.0".into(),
        architecture: "gfx-fixture".into(),
        driver_version: "1".into(),
        rocm_version: "1".into(),
        hip_version: "1".into(),
        kernel_name: "fixture_kernel".into(),
        code_object_sha256: digest(0x20),
        evidence: vec![],
    };
    device.evidence = vec![normalized_gpu_evidence(&device).unwrap()];
    AcceleratorObservationV1::Used {
        devices: vec![device],
    }
}

fn invocation(identity: &RareEventDatasetIdentityV1) -> InvocationV1 {
    let worker = worker();
    let scientific_mode = match identity.scientific {
        ScientificIdentityV1::Target { .. } => "target",
        ScientificIdentityV1::Coverage { .. } => "coverage",
    };
    let effective_configuration = RareEventConfigurationV1 {
        configuration_schema: CONFIGURATION_SCHEMA_V1.into(),
        artifact_root: "dev/simulation_results/permanent-rare-event".into(),
        design_identity: identity.preregistration.design.clone(),
        scientific_identity: identity.scientific.clone(),
        behavior: identity.behavior.clone(),
    };
    let effective_configuration = canonical_bytes(&effective_configuration).unwrap();
    InvocationV1 {
        argv: vec![
            "permanent_rare_event".into(),
            "worker".into(),
            "--accelerator-selection=none".into(),
            "--block-assignment-policy=round-robin/v1".into(),
            "--cpu-affinity=unpinned".into(),
            "--effective-devices=".into(),
            "--effective-workers=2".into(),
            "--executor-mode=cpu".into(),
            "--fallback-policy=safe-cpu/v1".into(),
            "--requested-workers=2".into(),
            "--work-queue-policy=canonical-block-queue/v1".into(),
        ],
        executable_path: "target/release/permanent_rare_event".into(),
        executable_sha256: identity.behavior.executable_sha256.clone(),
        process_id: 123,
        process_start_token: "fixture-start-token".into(),
        boot_identity: "fixture-boot".into(),
        configuration_path: identity.preregistration.configuration_path.clone(),
        configuration_sha256: identity.preregistration.configuration_sha256.clone(),
        effective_configuration_hex: effective_configuration
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        input_resolution: vec![
            InputResolutionV1 {
                field: "accelerator_selection".into(),
                origin: InputOriginV1::Argument,
                value: worker.accelerator_selection,
            },
            InputResolutionV1 {
                field: "artifact_root".into(),
                origin: InputOriginV1::Configuration,
                value: "dev/simulation_results/permanent-rare-event".into(),
            },
            InputResolutionV1 {
                field: "block_assignment_policy".into(),
                origin: InputOriginV1::Argument,
                value: worker.block_assignment_policy,
            },
            InputResolutionV1 {
                field: "cpu_affinity".into(),
                origin: InputOriginV1::Argument,
                value: worker.cpu_affinity,
            },
            InputResolutionV1 {
                field: "effective_devices".into(),
                origin: InputOriginV1::Argument,
                value: String::new(),
            },
            InputResolutionV1 {
                field: "effective_workers".into(),
                origin: InputOriginV1::Argument,
                value: worker.effective_workers.to_string(),
            },
            InputResolutionV1 {
                field: "executor_mode".into(),
                origin: InputOriginV1::Argument,
                value: worker.executor_mode,
            },
            InputResolutionV1 {
                field: "fallback_policy".into(),
                origin: InputOriginV1::Argument,
                value: worker.fallback_policy,
            },
            InputResolutionV1 {
                field: "requested_workers".into(),
                origin: InputOriginV1::Argument,
                value: worker.requested_workers.to_string(),
            },
            InputResolutionV1 {
                field: "scientific_identity".into(),
                origin: InputOriginV1::Configuration,
                value: scientific_mode.into(),
            },
            InputResolutionV1 {
                field: "work_queue_policy".into(),
                origin: InputOriginV1::Argument,
                value: worker.work_queue_policy,
            },
        ],
    }
}

fn start_envelope(
    identity: RareEventDatasetIdentityV1,
    accelerator: AcceleratorObservationV1,
) -> RareEventArtifactEnvelopeV1 {
    let predecessor = AttemptPredecessorV1::None {};
    let payload = ExecutionAttemptReceiptV1 {
        dataset_id: dataset_id(&identity).unwrap(),
        attempt_id: attempt_id(&identity, 0, &predecessor).unwrap(),
        dataset_identity: identity.clone(),
        attempt_ordinal: 0,
        predecessor,
        phase: AttemptPhaseV1::Start {
            resume_checkpoint_refs: vec![],
            start_utc: "2026-08-30T08:00:01.000000000Z".into(),
            start_receipt_utc: "2026-08-30T08:00:02.000000000Z".into(),
            invocation: Box::new(invocation(&identity)),
            environment_inputs: vec![EnvironmentInputV1 {
                name: "RAYON_NUM_THREADS".into(),
                value: EnvironmentValueV1::Set("2".into()),
            }],
            worker_configuration: worker(),
            host_observation: Box::new(host()),
            accelerator_observation: accelerator,
        },
    };
    RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::ExecutionAttempt,
        payload: RareEventPayloadV1::ExecutionAttempt(Box::new(payload)),
    }
}

fn terminal_envelope(
    identity: RareEventDatasetIdentityV1,
    start: &RareEventArtifactEnvelopeV1,
    failed: bool,
    checkpoint_refs: Vec<CheckpointRefV1>,
) -> RareEventArtifactEnvelopeV1 {
    let RareEventPayloadV1::ExecutionAttempt(start_payload) = &start.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Start {
        start_utc,
        host_observation,
        accelerator_observation,
        ..
    } = &start_payload.phase
    else {
        unreachable!()
    };
    let start_digest = sha256_hex(&canonical_bytes(start).unwrap());
    let payload = ExecutionAttemptReceiptV1 {
        dataset_id: dataset_id(&identity).unwrap(),
        attempt_id: start_payload.attempt_id.clone(),
        dataset_identity: identity,
        attempt_ordinal: 0,
        predecessor: AttemptPredecessorV1::None {},
        phase: AttemptPhaseV1::Terminal {
            attempt_start_sha256: start_digest,
            start_utc: start_utc.clone(),
            end_utc: "2026-08-30T08:00:03.000000000Z".into(),
            end_time_meaning: EndTimeMeaningV1::ProcessObserved,
            monotonic_elapsed_ns: Some(1_000_000_000),
            host_observation_sha256: sha256_hex(&canonical_bytes(host_observation).unwrap()),
            accelerator_observation_sha256: sha256_hex(
                &canonical_bytes(accelerator_observation).unwrap(),
            ),
            outcome_observer: OutcomeObserverV1::SupervisingLauncher {
                launcher_sha256: digest(0x21),
            },
            outcome: if failed {
                AttemptOutcomeV1::Failed {
                    wait_status: "exit:1".into(),
                    failure_category: Some("fixture".into()),
                }
            } else {
                AttemptOutcomeV1::Completed {}
            },
            checkpoint_refs,
        },
    };
    RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::ExecutionAttempt,
        payload: RareEventPayloadV1::ExecutionAttempt(Box::new(payload)),
    }
}

fn target_checkpoint(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    let run_address = RunAddressV1::Target { run: 0 };
    let records = (0..256)
        .map(|trajectory| TargetTrajectoryRecordV1 {
            run: 0,
            trajectory,
            stream_index: u64::from(trajectory),
            exponent: trajectory % 3073,
        })
        .collect();
    let payload = TrajectoryCheckpointV1 {
        dataset_id: dataset_id(&identity).unwrap(),
        run_id: run_id(&identity, &run_address).unwrap(),
        dataset_identity: identity,
        run_address,
        block_index: 0,
        trajectory_start: 0,
        trajectory_end: 256,
        attempt_id: digest(0x22),
        attempt_start_sha256: digest(0x23),
        producer: ProducerBackendV1::Cpu {},
        accelerator_observation_sha256: digest(0x24),
        records: TrajectoryRecordsV1::Target(records),
    };
    RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::TrajectoryCheckpoint,
        payload: RareEventPayloadV1::TrajectoryCheckpoint(Box::new(payload)),
    }
}

fn coverage_checkpoint(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    let run_address = RunAddressV1::Coverage {
        q: 3,
        replicate: 0,
        run: 0,
    };
    let records = (0..4096)
        .map(|trajectory| CoverageTrajectoryRecordV1 {
            q: 3,
            replicate: 0,
            run: 0,
            trajectory,
            stream_index: (1_u64 << 54) | u64::from(trajectory),
            exponent: trajectory % 10,
        })
        .collect();
    let payload = TrajectoryCheckpointV1 {
        dataset_id: dataset_id(&identity).unwrap(),
        run_id: run_id(&identity, &run_address).unwrap(),
        dataset_identity: identity,
        run_address,
        block_index: 0,
        trajectory_start: 0,
        trajectory_end: 4096,
        attempt_id: digest(0x25),
        attempt_start_sha256: digest(0x26),
        producer: ProducerBackendV1::Gpu {
            device_uuid: "GPU-0001".into(),
            kernel_name: "fixture_kernel".into(),
            code_object_sha256: digest(0x20),
        },
        accelerator_observation_sha256: digest(0x27),
        records: TrajectoryRecordsV1::Coverage(records),
    };
    RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::TrajectoryCheckpoint,
        payload: RareEventPayloadV1::TrajectoryCheckpoint(Box::new(payload)),
    }
}

fn checkpoint_refs_target() -> Vec<CheckpointRefV1> {
    let mut refs = Vec::new();
    for run in 0..32 {
        for block in 0..64 {
            let block_address = format!("target/{run:02}/{block:04}");
            refs.push(CheckpointRefV1 {
                checkpoint_sha256: sha256_hex(block_address.as_bytes()),
                block_address,
            });
        }
    }
    refs
}

fn checkpoint_refs_coverage() -> Vec<CheckpointRefV1> {
    let mut refs = Vec::new();
    for q in [3, 5, 7] {
        for replicate in 0..200 {
            for run in 0..32 {
                let block_address = format!("coverage/q{q}/b{replicate:03}/r{run:02}/0000");
                refs.push(CheckpointRefV1 {
                    checkpoint_sha256: sha256_hex(block_address.as_bytes()),
                    block_address,
                });
            }
        }
    }
    refs
}

fn validated_final_inputs(
    identity: &RareEventDatasetIdentityV1,
    accelerator: AcceleratorObservationV1,
    producer: ProducerBackendV1,
    checkpoint_refs: Vec<CheckpointRefV1>,
) -> (ValidatedCheckpointSet, ValidatedExecutionLineage) {
    let start = start_envelope(identity.clone(), accelerator.clone());
    let start_files = encode_artifact_files(&start).unwrap();
    let validated_start = validate_attempt_artifact_files(
        &start_files.artifact_json,
        &start_files.artifact_sha256,
        identity,
    )
    .unwrap();
    let RareEventPayloadV1::ExecutionAttempt(start_payload) = &start.payload else {
        unreachable!()
    };
    let accelerator_sha256 = sha256_hex(&canonical_bytes(&accelerator).unwrap());
    let exponent_histograms = fixture_exponent_histograms(identity, checkpoint_refs.len());
    let checkpoints = validated_checkpoint_set_fixture(
        identity,
        checkpoint_refs.clone(),
        &start_payload.attempt_id,
        &start_files.digest,
        producer,
        &accelerator_sha256,
        exponent_histograms,
    )
    .unwrap();
    let terminal = terminal_envelope(identity.clone(), &start, false, checkpoint_refs);
    let terminal_files = encode_artifact_files(&terminal).unwrap();
    let validated_terminal = validate_attempt_artifact_files(
        &terminal_files.artifact_json,
        &terminal_files.artifact_sha256,
        identity,
    )
    .unwrap();
    let lineage = validate_execution_lineage(
        identity,
        &[validated_start, validated_terminal],
        &checkpoints,
    )
    .unwrap();
    (checkpoints, lineage)
}

fn fixture_exponent_histograms(
    identity: &RareEventDatasetIdentityV1,
    checkpoint_count: usize,
) -> Vec<Vec<ExponentBinV1>> {
    match &identity.scientific {
        ScientificIdentityV1::Target { .. } => vec![
            vec![ExponentBinV1 {
                exponent: 1,
                count: 256,
            }];
            checkpoint_count
        ],
        ScientificIdentityV1::Coverage { .. } => (0..checkpoint_count)
            .map(|index| {
                let q = [3_u8, 5, 7][index / (200 * 32)];
                let numerator = match q {
                    3 => 907,
                    5 => 17_581,
                    7 => 126_295,
                    _ => unreachable!(),
                };
                coverage_run_histogram(q, numerator)
            })
            .collect(),
    }
}

fn target_final(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    let exact_raw_count = BigUint::from(3_u8).pow(3_071).to_string();
    let exact_total = BigUint::from(3_u8).pow(3_072).to_string();
    let (checkpoints, lineage) = validated_final_inputs(
        &identity,
        not_used(),
        ProducerBackendV1::Cpu {},
        checkpoint_refs_target(),
    );
    let target_interval = zero_variance_interval(1, 3, 3, 1);
    target_final_envelope(
        identity,
        TargetResultPayloadV1 {
            expected_trajectory_count: 524_288,
            exact_result_path: "dev/simulation_results/permanent-rare-event/exact.json".into(),
            exact_result_sha256: digest(0x30),
            exact_raw_count,
            exact_total,
            exact_probability: ExactDecimalV1::new("1", "3"),
            cross_check_estimate: ExactDecimalV1::new("1", "3"),
            independent_run_variance: ExactDecimalV1::new("0", "1"),
            interval_lower: target_interval.0,
            interval_upper: target_interval.1,
            final_weight_ess: ExactDecimalV1::new("524288", "1"),
            per_run_ess: vec![ExactDecimalV1::new("16384", "1"); 32],
            ess_fraction: ExactDecimalV1::new("1", "1"),
            largest_weight_share: ExactDecimalV1::new("1", "524288"),
            largest_run_mean_share: ExactDecimalV1::new("1", "32"),
            exponent_histogram: vec![ExponentBinV1 {
                exponent: 1,
                count: 524_288,
            }],
            minimum_exponent: 1,
            maximum_exponent: 1,
            run_means: vec![ExactDecimalV1::new("1", "3"); 32],
            extinction_reasons: vec![],
            degeneracy: false,
            verdict: CrossCheckVerdictV1::Agreement,
        },
        &checkpoints,
        &lineage,
    )
    .unwrap()
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn exact_u128(numerator: u128, denominator: u128) -> ExactDecimalV1 {
    let divisor = gcd_u128(numerator, denominator);
    ExactDecimalV1::new(
        (numerator / divisor).to_string(),
        (denominator / divisor).to_string(),
    )
}

fn zero_variance_interval(
    numerator: u64,
    denominator: u64,
    base: u32,
    minimum_exponent: u32,
) -> (String, String) {
    let rendered = ScaledStudentInterval::from_exact_independent_runs(
        numerator,
        denominator,
        0_u8,
        1_u8,
        1_019_756_723_u64,
        500_000_000_u64,
        32,
        base,
        minimum_exponent,
    )
    .unwrap()
    .render_outward(18)
    .unwrap();
    (rendered.lower, rendered.upper)
}

fn coverage_run_histogram(q: u8, anchor_numerator: u64) -> Vec<ExponentBinV1> {
    const PER_RUN: u64 = 4_096;
    let mut remaining = PER_RUN * anchor_numerator * u64::from(q).pow(2) - PER_RUN;
    let mut used = 0_u64;
    let mut per_run = Vec::new();
    for exponent in 0..9 {
        let increment = u64::from(q).pow(9 - exponent) - 1;
        let count = (remaining / increment).min(PER_RUN - used);
        if count != 0 {
            per_run.push((exponent, count));
            used += count;
            remaining -= count * increment;
        }
    }
    assert_eq!(remaining, 0);
    if used < PER_RUN {
        per_run.push((9, PER_RUN - used));
    }
    per_run
        .into_iter()
        .map(|(exponent, count)| ExponentBinV1 { exponent, count })
        .collect()
}

fn coverage_histogram(q: u8, anchor_numerator: u64) -> Vec<ExponentBinV1> {
    coverage_run_histogram(q, anchor_numerator)
        .into_iter()
        .map(|bin| ExponentBinV1 {
            exponent: bin.exponent,
            count: bin.count * 32,
        })
        .collect()
}

fn histogram_ess(q: u8, bins: &[ExponentBinV1]) -> ExactDecimalV1 {
    let maximum = bins.last().unwrap().exponent;
    let sum: u128 = bins
        .iter()
        .map(|bin| u128::from(bin.count) * u128::from(q).pow(maximum - bin.exponent))
        .sum();
    let squares: u128 = bins
        .iter()
        .map(|bin| u128::from(bin.count) * u128::from(q).pow(2 * (maximum - bin.exponent)))
        .sum();
    exact_u128(sum * sum, squares)
}

fn coverage_final(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    let mut replicates = Vec::new();
    for (q, numerator, denominator) in [
        (3, "907", "2187"),
        (5, "17581", "78125"),
        (7, "126295", "823543"),
    ] {
        let histogram = coverage_histogram(q, numerator.parse().unwrap());
        let interval = zero_variance_interval(
            numerator.parse().unwrap(),
            denominator.parse().unwrap(),
            u32::from(q),
            histogram.first().unwrap().exponent,
        );
        let final_weight_ess = histogram_ess(q, &histogram);
        let ess_numerator: u128 = final_weight_ess.numerator.parse().unwrap();
        let ess_denominator: u128 = final_weight_ess.denominator.parse().unwrap();
        for replicate in 0..200 {
            replicates.push(CoverageReplicateV1 {
                q,
                replicate,
                exact_anchor: ExactDecimalV1::new(numerator, denominator),
                estimate: ExactDecimalV1::new(numerator, denominator),
                independent_run_variance: ExactDecimalV1::new("0", "1"),
                interval_lower: interval.0.clone(),
                interval_upper: interval.1.clone(),
                contains_anchor: true,
                ess_fraction: exact_u128(ess_numerator, ess_denominator * 131_072),
                final_weight_ess: final_weight_ess.clone(),
                exponent_histogram: histogram.clone(),
                run_means: vec![ExactDecimalV1::new(numerator, denominator); 32],
                extinction_reasons: vec![],
                degeneracy: false,
            });
        }
    }
    let (checkpoints, lineage) = validated_final_inputs(
        &identity,
        used(),
        ProducerBackendV1::Gpu {
            device_uuid: "GPU-0001".into(),
            kernel_name: "fixture_kernel".into(),
            code_object_sha256: digest(0x20),
        },
        checkpoint_refs_coverage(),
    );
    coverage_final_envelope(
        identity,
        CoverageResultPayloadV1 {
            expected_trajectory_count: 78_643_200,
            replicates,
            coverage_counts: vec![
                CoverageCountV1 { q: 3, count: 200 },
                CoverageCountV1 { q: 5, count: 200 },
                CoverageCountV1 { q: 7, count: 200 },
            ],
            verdict: CoverageVerdictV1::Adequate,
        },
        &checkpoints,
        &lineage,
    )
    .unwrap()
}

fn all_envelopes() -> Vec<(RareEventDatasetIdentityV1, RareEventArtifactEnvelopeV1)> {
    let target = identity(ScientificIdentityV1::target());
    let coverage = identity(ScientificIdentityV1::coverage());
    let cpu_start = start_envelope(target.clone(), not_used());
    let gpu_start = start_envelope(coverage.clone(), used());
    vec![
        (target.clone(), target_checkpoint(target.clone())),
        (coverage.clone(), coverage_checkpoint(coverage.clone())),
        (target.clone(), cpu_start.clone()),
        (
            target.clone(),
            terminal_envelope(target.clone(), &cpu_start, false, vec![]),
        ),
        (coverage.clone(), gpu_start.clone()),
        (
            coverage.clone(),
            terminal_envelope(coverage.clone(), &gpu_start, true, vec![]),
        ),
        (target.clone(), target_final(target)),
        (coverage.clone(), coverage_final(coverage)),
    ]
}

fn value_bytes(value: serde_json::Value) -> Vec<u8> {
    canonical_bytes(&value).unwrap()
}

fn files_from_json(json: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let sidecar = format!("{}  artifact.json\n", sha256_hex(&json)).into_bytes();
    (json, sidecar)
}

fn insert_unknown(value: &mut serde_json::Value) {
    value
        .as_object_mut()
        .unwrap()
        .insert("unknown_nested_field".into(), serde_json::json!(true));
}

fn rehash_result(value: &mut serde_json::Value) {
    value["payload"]["result_sha256"] = serde_json::json!(sha256_hex(
        &canonical_bytes(&value["payload"]["result_payload"]).unwrap()
    ));
}

fn independent_domain_digest(domain: &[u8], input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

fn decode_hex_fixture(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn rare_event_artifact_schema_roundtrip_v1() {
    for scientific in [
        ScientificIdentityV1::target(),
        ScientificIdentityV1::coverage(),
    ] {
        let configuration = config(scientific);
        let bytes = canonical_bytes(&configuration).unwrap();
        assert_eq!(decode_configuration(&bytes).unwrap(), configuration);
    }
    for (expected_identity, envelope) in all_envelopes() {
        let files = encode_artifact_files(&envelope).unwrap();
        assert_eq!(files.artifact_json.last(), Some(&b'\n'));
        assert!(!files.artifact_json.windows(2).any(|window| window == b"  "));
        assert_eq!(
            verify_artifact_files(
                &files.artifact_json,
                &files.artifact_sha256,
                &expected_identity,
            )
            .unwrap(),
            envelope
        );
    }

    let target = identity(ScientificIdentityV1::target());
    let coverage = identity(ScientificIdentityV1::coverage());
    let canonical_goldens = [
        target_checkpoint(target.clone()),
        start_envelope(target.clone(), not_used()),
        target_final(target),
        coverage_final(coverage),
    ]
    .map(|envelope| {
        let bytes = canonical_bytes(&envelope).unwrap();
        (bytes.len(), sha256_hex(&bytes))
    });
    assert_eq!(
        canonical_goldens,
        [
            (
                18_821,
                "14d644ecf167427d22e10787bba99db67801154ef7d16df95bf6123d2a2fd3c9".into(),
            ),
            (
                10_334,
                "d8fa41e55de0aeedaf65def3d544180ff86c73bdede829e01b6bcc98d93ea976".into(),
            ),
            (
                259_584,
                "65fa1e2ad195045f4cbbd3e59429057343dc5331c4b86438cdcaa0e483c26867".into(),
            ),
            (
                3_852_105,
                "32b8f831d4b344b766a64799d7dc3a61864afdc09cefb5d419f9457fbaeaa712".into(),
            ),
        ]
    );

    let target = identity(ScientificIdentityV1::target());
    let coverage = identity(ScientificIdentityV1::coverage());
    let target_dataset = dataset_id(&target).unwrap();
    let coverage_dataset = dataset_id(&coverage).unwrap();
    let target_run = run_id(&target, &RunAddressV1::Target { run: 17 }).unwrap();
    let coverage_run = run_id(
        &coverage,
        &RunAddressV1::Coverage {
            q: 7,
            replicate: 199,
            run: 31,
        },
    )
    .unwrap();
    let first_attempt = attempt_id(&target, 0, &AttemptPredecessorV1::None {}).unwrap();
    let second_attempt = attempt_id(
        &target,
        1,
        &AttemptPredecessorV1::Terminal {
            terminal_sha256: digest(0xab),
        },
    )
    .unwrap();
    assert_eq!(
        [
            target_dataset.clone(),
            coverage_dataset.clone(),
            target_run.clone(),
            coverage_run.clone(),
            first_attempt.clone(),
            second_attempt.clone(),
        ],
        [
            "f9f9576fa6b599e7c0f104e043ef8a76e3ae7d7a404546639b8d4cd37a00484d",
            "8f713d001bc135aae651a356f78d91d839bd0df5763609c0f51668fcc9b95a27",
            "af48e27a7fc5bd40cff8ca9f0597100f5e406557c26b50d2b4340006f334d860",
            "339702b2fdff6091a12958c2b0fa1f5525e069429ab900518d3ae5023050a674",
            "5a9a79e123d21c4b6f02dcc4b67f18e59971fbc2dd04c03ff2f34661a25f782a",
            "fdc229d1fd863d07ca071333105c76c22f9c7bf1b80e4416d838d33e353fc5ee",
        ]
    );
    assert_eq!(
        target_dataset,
        independent_domain_digest(
            b"gf2-rare-event-dataset-identity-v1",
            &canonical_bytes(&target).unwrap(),
        )
    );
    let mut independent_run_input = decode_hex_fixture(&target_dataset);
    independent_run_input.extend(canonical_bytes(&RunAddressV1::Target { run: 17 }).unwrap());
    assert_eq!(
        target_run,
        independent_domain_digest(b"gf2-rare-event-run-identity-v1", &independent_run_input)
    );
    let mut independent_attempt_input = decode_hex_fixture(&target_dataset);
    independent_attempt_input.extend(0_u64.to_le_bytes());
    independent_attempt_input.push(0);
    assert_eq!(
        first_attempt,
        independent_domain_digest(
            b"gf2-rare-event-attempt-identity-v1",
            &independent_attempt_input,
        )
    );
}

#[test]
fn rare_event_artifact_unknown_versions_rejected() {
    let target = identity(ScientificIdentityV1::target());
    let envelope = start_envelope(target.clone(), not_used());
    let mut base = serde_json::to_value(&envelope).unwrap();
    for version in ["v0", "v2"] {
        let mut value = base.clone();
        value["envelope_schema"] =
            serde_json::json!(format!("gf2.rare-event-artifact-envelope/{version}"));
        assert!(decode_envelope(&value_bytes(value)).is_err());

        let mut value = base.clone();
        value["payload"]["payload_schema"] =
            serde_json::json!(format!("gf2.rare-event-execution-attempt/{version}"));
        assert!(decode_envelope(&value_bytes(value)).is_err());
    }
    base["artifact_kind"] = serde_json::json!("unknown");
    assert!(decode_envelope(&value_bytes(base.clone())).is_err());

    let mut value = serde_json::to_value(&envelope).unwrap();
    value["payload"]["phase"] = serde_json::json!("unknown");
    assert!(decode_envelope(&value_bytes(value)).is_err());

    let mut value = serde_json::to_value(&envelope).unwrap();
    value["payload"]["unknown_nested_field"] = serde_json::json!(1);
    assert!(decode_envelope(&value_bytes(value)).is_err());

    for field in [
        "address_partition",
        "sampler",
        "rng_algorithm",
        "serializer",
    ] {
        let mut value = serde_json::to_value(&envelope).unwrap();
        let target = match field {
            "address_partition" => &mut value["payload"]["dataset_identity"]["campaign"][field],
            _ => &mut value["payload"]["dataset_identity"]["behavior"][field],
        };
        *target = serde_json::json!("unknown/v2");
        assert!(decode_envelope(&value_bytes(value)).is_err());
    }

    let configuration = config(ScientificIdentityV1::target());
    for version in ["v0", "v2"] {
        let mut value = serde_json::to_value(&configuration).unwrap();
        value["configuration_schema"] =
            serde_json::json!(format!("gf2.rare-event-configuration/{version}"));
        assert!(decode_configuration(&value_bytes(value)).is_err());
    }

    let direct_unknowns: Vec<(serde_json::Value, &str)> = vec![
        (
            serde_json::to_value(ScientificIdentityV1::target()).unwrap(),
            "scientific",
        ),
        (
            serde_json::to_value(RunAddressV1::Target { run: 0 }).unwrap(),
            "run address",
        ),
        (
            serde_json::to_value(ProducerBackendV1::Cpu {}).unwrap(),
            "producer",
        ),
        (
            serde_json::to_value(TrajectoryRecordsV1::Target(vec![])).unwrap(),
            "trajectory records",
        ),
        (
            serde_json::to_value(AttemptPredecessorV1::None {}).unwrap(),
            "predecessor",
        ),
        (
            serde_json::to_value(EnvironmentValueV1::Set("fixture".into())).unwrap(),
            "environment value",
        ),
        (
            serde_json::to_value(AcceleratorObservationV1::NotUsed {
                reason: "fixture".into(),
                evidence: vec![],
            })
            .unwrap(),
            "accelerator",
        ),
        (
            serde_json::to_value(OutcomeObserverV1::SupervisingLauncher {
                launcher_sha256: digest(1),
            })
            .unwrap(),
            "observer",
        ),
        (
            serde_json::to_value(AttemptOutcomeV1::Completed {}).unwrap(),
            "outcome",
        ),
    ];
    for (mut value, kind) in direct_unknowns {
        insert_unknown(&mut value);
        let rejected = match kind {
            "scientific" => serde_json::from_value::<ScientificIdentityV1>(value).is_err(),
            "run address" => serde_json::from_value::<RunAddressV1>(value).is_err(),
            "producer" => serde_json::from_value::<ProducerBackendV1>(value).is_err(),
            "trajectory records" => serde_json::from_value::<TrajectoryRecordsV1>(value).is_err(),
            "predecessor" => serde_json::from_value::<AttemptPredecessorV1>(value).is_err(),
            "environment value" => serde_json::from_value::<EnvironmentValueV1>(value).is_err(),
            "accelerator" => serde_json::from_value::<AcceleratorObservationV1>(value).is_err(),
            "observer" => serde_json::from_value::<OutcomeObserverV1>(value).is_err(),
            "outcome" => serde_json::from_value::<AttemptOutcomeV1>(value).is_err(),
            _ => unreachable!(),
        };
        assert!(
            rejected,
            "direct {kind} deserialization accepted an unknown field"
        );
    }

    let mut payload = serde_json::to_value(&envelope.payload).unwrap();
    insert_unknown(&mut payload);
    assert!(serde_json::from_value::<RareEventPayloadV1>(payload).is_err());
    let RareEventPayloadV1::ExecutionAttempt(attempt) = &envelope.payload else {
        unreachable!()
    };
    let mut attempt = serde_json::to_value(attempt).unwrap();
    insert_unknown(&mut attempt);
    assert!(serde_json::from_value::<ExecutionAttemptReceiptV1>(attempt).is_err());
}

#[test]
fn rare_event_artifact_checksum_tamper_rejected() {
    for (expected_identity, envelope) in all_envelopes() {
        let files = encode_artifact_files(&envelope).unwrap();

        let mut changed = files.artifact_json.clone();
        let index = changed.iter().position(|&byte| byte == b'0').unwrap();
        changed[index] = b'1';
        assert!(
            verify_artifact_files(&changed, &files.artifact_sha256, &expected_identity).is_err()
        );

        let mut sidecar = files.artifact_sha256.clone();
        sidecar[0] = if sidecar[0] == b'0' { b'1' } else { b'0' };
        assert!(verify_artifact_files(&files.artifact_json, &sidecar, &expected_identity).is_err());

        let renamed_sidecar = format!("{}  renamed.json\n", files.digest).into_bytes();
        assert!(
            verify_artifact_files(&files.artifact_json, &renamed_sidecar, &expected_identity)
                .is_err()
        );

        let mut whitespace = files.artifact_json.clone();
        whitespace.insert(1, b' ');
        let (json, sidecar) = files_from_json(whitespace);
        assert!(verify_artifact_files(&json, &sidecar, &expected_identity).is_err());

        let mut missing_lf = files.artifact_json.clone();
        missing_lf.pop();
        let (json, sidecar) = files_from_json(missing_lf);
        assert!(verify_artifact_files(&json, &sidecar, &expected_identity).is_err());

        let value = serde_json::to_value(&envelope).unwrap();
        let reordered = format!(
            "{{\"envelope_schema\":{},\"artifact_kind\":{},\"payload\":{}}}\n",
            serde_json::to_string(&value["envelope_schema"]).unwrap(),
            serde_json::to_string(&value["artifact_kind"]).unwrap(),
            serde_json::to_string(&value["payload"]).unwrap(),
        )
        .into_bytes();
        let (json, sidecar) = files_from_json(reordered);
        assert!(verify_artifact_files(&json, &sidecar, &expected_identity).is_err());
    }
}

#[test]
fn rare_event_artifact_identity_mismatch_rejected() {
    let expected = identity(ScientificIdentityV1::target());
    let envelope = target_checkpoint(expected.clone());
    let mutations: &[&[&str]] = &[
        &["campaign", "campaign_id"],
        &["campaign", "manifest_sha256"],
        &["campaign", "root_seed"],
        &["preregistration", "design", "blob_id"],
        &["preregistration", "configuration_sha256"],
        &["behavior", "executable_sha256"],
        &["behavior", "compilation_target"],
    ];
    for path in mutations {
        let mut value = serde_json::to_value(&envelope).unwrap();
        let mut field = &mut value["payload"]["dataset_identity"];
        for component in *path {
            field = &mut field[*component];
        }
        *field = if path.last() == Some(&"compilation_target") {
            serde_json::json!("different-target")
        } else if path.last() == Some(&"campaign_id") {
            serde_json::json!("different-campaign")
        } else if path.last() == Some(&"root_seed") {
            serde_json::json!("0x0000000000000000")
        } else {
            serde_json::json!(digest(0xee))
        };
        let (json, sidecar) = files_from_json(value_bytes(value));
        assert!(verify_artifact_files(&json, &sidecar, &expected).is_err());
    }

    let mut foreign = expected.clone();
    foreign.scientific = ScientificIdentityV1::coverage();
    let files = encode_artifact_files(&envelope).unwrap();
    assert!(verify_artifact_files(&files.artifact_json, &files.artifact_sha256, &foreign).is_err());

    assert_ne!(
        dataset_id(&expected).unwrap(),
        dataset_id(&identity(ScientificIdentityV1::coverage())).unwrap()
    );
    let target_run = RunAddressV1::Target { run: 0 };
    assert_ne!(
        run_id(&expected, &target_run).unwrap(),
        run_id(&expected, &RunAddressV1::Target { run: 1 }).unwrap()
    );
    assert_ne!(
        attempt_id(&expected, 0, &AttemptPredecessorV1::None {}).unwrap(),
        attempt_id(
            &expected,
            1,
            &AttemptPredecessorV1::Terminal {
                terminal_sha256: digest(0xab),
            },
        )
        .unwrap()
    );

    let start = start_envelope(expected.clone(), not_used());
    for path in [
        &["executable_sha256"][..],
        &["configuration_path"][..],
        &["configuration_sha256"][..],
        &["effective_configuration_hex"][..],
    ] {
        let mut value = serde_json::to_value(&start).unwrap();
        let invocation = &mut value["payload"]["invocation"];
        invocation[path[0]] = match path[0] {
            "configuration_path" => serde_json::json!("different/config.json"),
            "effective_configuration_hex" => serde_json::json!("00"),
            _ => serde_json::json!(digest(0xef)),
        };
        assert!(decode_envelope(&value_bytes(value)).is_err());
    }
    for mutation in 0..4 {
        let mut value = serde_json::to_value(&start).unwrap();
        match mutation {
            0 => {
                value["payload"]["invocation"]["input_resolution"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            1 => {
                value["payload"]["invocation"]["input_resolution"][0]["origin"] =
                    serde_json::json!("configuration");
            }
            2 => {
                value["payload"]["invocation"]["input_resolution"][0]["value"] =
                    serde_json::json!("different");
            }
            3 => {
                value["payload"]["invocation"]["argv"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|token| token != "--accelerator-selection=none");
            }
            _ => unreachable!(),
        }
        assert!(decode_envelope(&value_bytes(value)).is_err());
    }

    let mut raw_root = config(ScientificIdentityV1::target());
    raw_root.artifact_root =
        "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829".into();
    assert!(decode_configuration(&canonical_bytes(&raw_root).unwrap()).is_err());

    let target_receipt = target_final(expected.clone());
    let base = serde_json::to_value(&target_receipt).unwrap();
    let target_mutations = [
        ("exact_raw_count", serde_json::json!("01")),
        (
            "exact_probability",
            serde_json::json!({"numerator":"2","denominator":"3"}),
        ),
        (
            "independent_run_variance",
            serde_json::json!({"numerator":"1","denominator":"1"}),
        ),
        (
            "interval_lower",
            serde_json::json!("3.40000000000000000e-1"),
        ),
        (
            "interval_upper",
            serde_json::json!("1.00000000000000000e-9223372036854775808"),
        ),
        (
            "final_weight_ess",
            serde_json::json!({"numerator":"1","denominator":"1"}),
        ),
        (
            "ess_fraction",
            serde_json::json!({"numerator":"1","denominator":"2"}),
        ),
        (
            "largest_weight_share",
            serde_json::json!({"numerator":"1","denominator":"2"}),
        ),
        (
            "largest_run_mean_share",
            serde_json::json!({"numerator":"1","denominator":"2"}),
        ),
        ("degeneracy", serde_json::json!(true)),
        ("verdict", serde_json::json!("contradiction")),
    ];
    for (field, replacement) in target_mutations {
        let mut value = base.clone();
        value["payload"]["result_payload"][field] = replacement;
        rehash_result(&mut value);
        assert!(
            decode_envelope(&value_bytes(value)).is_err(),
            "accepted target {field}"
        );
    }
    for (field, replacement) in [
        ("count", serde_json::json!(524_287)),
        ("exponent", serde_json::json!(3_073)),
    ] {
        let mut value = base.clone();
        value["payload"]["result_payload"]["exponent_histogram"][0][field] = replacement;
        rehash_result(&mut value);
        assert!(decode_envelope(&value_bytes(value)).is_err());
    }
    let mut value = base.clone();
    value["payload"]["result_payload"]["run_means"][0] =
        serde_json::json!({"numerator":"1","denominator":"2"});
    rehash_result(&mut value);
    assert!(decode_envelope(&value_bytes(value)).is_err());
    let mut value = base.clone();
    value["payload"]["result_payload"]["per_run_ess"][0] =
        serde_json::json!({"numerator":"0","denominator":"1"});
    rehash_result(&mut value);
    assert!(decode_envelope(&value_bytes(value)).is_err());

    let coverage_identity = identity(ScientificIdentityV1::coverage());
    let coverage_receipt = coverage_final(coverage_identity.clone());
    let coverage_base = serde_json::to_value(&coverage_receipt).unwrap();
    for (field, replacement) in [
        (
            "exact_anchor",
            serde_json::json!({"numerator":"1","denominator":"2"}),
        ),
        (
            "estimate",
            serde_json::json!({"numerator":"1","denominator":"2"}),
        ),
        (
            "independent_run_variance",
            serde_json::json!({"numerator":"1","denominator":"1"}),
        ),
        (
            "interval_upper",
            serde_json::json!("1.10000000000000000e+0"),
        ),
        ("contains_anchor", serde_json::json!(false)),
        (
            "final_weight_ess",
            serde_json::json!({"numerator":"1","denominator":"1"}),
        ),
        (
            "ess_fraction",
            serde_json::json!({"numerator":"1","denominator":"1"}),
        ),
        ("degeneracy", serde_json::json!(true)),
    ] {
        let mut value = coverage_base.clone();
        value["payload"]["result_payload"]["replicates"][0][field] = replacement;
        rehash_result(&mut value);
        assert!(
            decode_envelope(&value_bytes(value)).is_err(),
            "accepted coverage {field}"
        );
    }
    let mut value = coverage_base.clone();
    value["payload"]["result_payload"]["replicates"][0]["exponent_histogram"][0]["count"] =
        serde_json::json!(1);
    rehash_result(&mut value);
    assert!(decode_envelope(&value_bytes(value)).is_err());
    let mut value = coverage_base.clone();
    value["payload"]["result_payload"]["coverage_counts"][0]["count"] = serde_json::json!(199);
    rehash_result(&mut value);
    assert!(decode_envelope(&value_bytes(value)).is_err());

    let mut value = serde_json::to_value(target_receipt).unwrap();
    value["payload"]["dataset_identity"] = serde_json::to_value(&coverage_identity).unwrap();
    value["payload"]["dataset_id"] = serde_json::json!(dataset_id(&coverage_identity).unwrap());
    assert!(decode_envelope(&value_bytes(value)).is_err());
    let mut value = serde_json::to_value(coverage_receipt).unwrap();
    value["payload"]["dataset_identity"] = serde_json::to_value(&expected).unwrap();
    value["payload"]["dataset_id"] = serde_json::json!(dataset_id(&expected).unwrap());
    assert!(decode_envelope(&value_bytes(value)).is_err());
}

#[test]
fn rare_event_artifact_address_sets_rejected() {
    let target = identity(ScientificIdentityV1::target());
    let checkpoint = target_checkpoint(target.clone());
    let base = serde_json::to_value(&checkpoint).unwrap();
    let mut mutations = Vec::new();

    let mut value = base.clone();
    value["payload"]["records"]["items"]
        .as_array_mut()
        .unwrap()
        .pop();
    mutations.push(value);
    let mut value = base.clone();
    let extra = value["payload"]["records"]["items"][0].clone();
    value["payload"]["records"]["items"]
        .as_array_mut()
        .unwrap()
        .push(extra);
    mutations.push(value);
    let mut value = base.clone();
    value["payload"]["records"]["items"][1] = value["payload"]["records"]["items"][0].clone();
    mutations.push(value);
    let mut value = base.clone();
    value["payload"]["records"]["items"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    mutations.push(value);
    for (field, replacement) in [
        ("trajectory", serde_json::json!(256)),
        ("stream_index", serde_json::json!(999)),
        ("run", serde_json::json!(1)),
        ("exponent", serde_json::json!(3_073)),
    ] {
        let mut value = base.clone();
        value["payload"]["records"]["items"][0][field] = replacement;
        mutations.push(value);
    }
    let mut value = base.clone();
    value["payload"]["records"]["items"][1]["trajectory"] = serde_json::json!(0);
    value["payload"]["records"]["items"][1]["stream_index"] = serde_json::json!(0);
    value["payload"]["records"]["items"][1]["exponent"] = serde_json::json!(2);
    mutations.push(value);
    for value in mutations {
        let (json, sidecar) = files_from_json(value_bytes(value));
        assert!(validate_checkpoint_artifact_files(&json, &sidecar, &target).is_err());
    }

    let coverage = identity(ScientificIdentityV1::coverage());
    let coverage_checkpoint = coverage_checkpoint(coverage.clone());
    let coverage_base = serde_json::to_value(&coverage_checkpoint).unwrap();
    for (field, replacement) in [
        ("trajectory", serde_json::json!(4_096)),
        ("stream_index", serde_json::json!(0)),
        ("q", serde_json::json!(5)),
        ("exponent", serde_json::json!(10)),
    ] {
        let mut value = coverage_base.clone();
        value["payload"]["records"]["items"][0][field] = replacement;
        let (json, sidecar) = files_from_json(value_bytes(value));
        assert!(validate_checkpoint_artifact_files(&json, &sidecar, &coverage).is_err());
    }

    let references = checkpoint_refs_target();
    let summaries = fixture_exponent_histograms(&target, references.len());
    for mutation in 0..4 {
        let mut changed_refs = references.clone();
        let mut changed_summaries = summaries.clone();
        match mutation {
            0 => {
                changed_refs.pop();
                changed_summaries.pop();
            }
            1 => {
                changed_refs.push(changed_refs.last().unwrap().clone());
                changed_summaries.push(changed_summaries.last().unwrap().clone());
            }
            2 => changed_refs[1] = changed_refs[0].clone(),
            3 => changed_refs.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(validated_checkpoint_set_fixture(
            &target,
            changed_refs,
            &digest(0x22),
            &digest(0x23),
            ProducerBackendV1::Cpu {},
            &digest(0x24),
            changed_summaries,
        )
        .is_err());
    }

    let final_receipt = target_final(target.clone());
    let final_files = encode_artifact_files(&final_receipt).unwrap();
    let (checkpoints, lineage) = validated_final_inputs(
        &target,
        not_used(),
        ProducerBackendV1::Cpu {},
        checkpoint_refs_target(),
    );
    validate_final_artifact_files(
        &final_files.artifact_json,
        &final_files.artifact_sha256,
        &target,
        &checkpoints,
        &lineage,
    )
    .unwrap();
    for mutation in 0..5 {
        let mut value = serde_json::to_value(&final_receipt).unwrap();
        let refs = value["payload"]["execution_provenance"]["checkpoint_refs"]
            .as_array_mut()
            .unwrap();
        match mutation {
            0 => {
                refs.pop();
            }
            1 => refs.push(refs.last().unwrap().clone()),
            2 => refs[1] = refs[0].clone(),
            3 => refs.swap(0, 1),
            4 => refs[0]["checkpoint_sha256"] = serde_json::json!(digest(0xee)),
            _ => unreachable!(),
        }
        let (json, sidecar) = files_from_json(value_bytes(value));
        assert!(
            validate_final_artifact_files(&json, &sidecar, &target, &checkpoints, &lineage,)
                .is_err()
        );
    }
}

fn validated_attempt(
    envelope: &RareEventArtifactEnvelopeV1,
    identity: &RareEventDatasetIdentityV1,
) -> ValidatedAttemptArtifact {
    let files = encode_artifact_files(envelope).unwrap();
    validate_attempt_artifact_files(&files.artifact_json, &files.artifact_sha256, identity).unwrap()
}

fn artifact_tempdir(prefix: &str) -> tempfile::TempDir {
    fs::create_dir_all("target").unwrap();
    Builder::new().prefix(prefix).tempdir_in("target").unwrap()
}

#[test]
fn rare_event_artifact_partial_publish_recovery() {
    let identity = identity(ScientificIdentityV1::target());
    let checkpoint = target_checkpoint(identity.clone());
    let start = start_envelope(identity.clone(), not_used());
    let cuts = [
        PublicationCutPoint::BeforeFileCreation,
        PublicationCutPoint::AfterJsonWrite,
        PublicationCutPoint::AfterJsonSync,
        PublicationCutPoint::AfterSidecarWrite,
        PublicationCutPoint::AfterSidecarSync,
        PublicationCutPoint::AfterStagingDirectorySync,
        PublicationCutPoint::BeforeNoReplace,
        PublicationCutPoint::AfterNoReplace,
    ];

    for (case, (name, envelope)) in [("0000", &checkpoint), ("start", &start)]
        .into_iter()
        .enumerate()
    {
        for (cut_index, cut) in cuts.into_iter().enumerate() {
            let root = artifact_tempdir(&format!("rare-event-cut-{case}-{cut_index}-"));
            let parent = root.path().join("publication");
            fs::create_dir(&parent).unwrap();
            let result = publish_artifact_dir_at_cutpoint(&parent, name, envelope, cut);
            assert!(result.is_err(), "cut {cut:?} did not interrupt publication");
            let expected = vec![name.to_owned()];
            let recovered = recover_artifact_parent(&parent, &expected, &identity).unwrap();
            if cut == PublicationCutPoint::AfterNoReplace {
                assert_eq!(recovered.len(), 1);
                assert_eq!(
                    recovered[0].digest(),
                    encode_artifact_files(envelope).unwrap().digest
                );
            } else {
                assert!(recovered.is_empty());
                let entries: Vec<_> = fs::read_dir(&parent).unwrap().collect();
                assert_eq!(
                    entries.len(),
                    1,
                    "cut {cut:?} lost its diagnostic staging dir"
                );
            }
        }
    }

    let probe = artifact_tempdir("rare-event-atomic-probe-");
    verify_atomic_publication_support(probe.path()).unwrap();
    assert_eq!(fs::read_dir(probe.path()).unwrap().count(), 0);

    let strict = artifact_tempdir("rare-event-strict-directory-");
    let published = publish_artifact_dir(strict.path(), "0000", &checkpoint).unwrap();
    assert_eq!(fs::read_dir(published.path()).unwrap().count(), 2);
    fs::write(published.path().join("extra"), b"unexpected").unwrap();
    assert!(verify_artifact_dir(published.path(), &identity).is_err());
    assert!(recover_artifact_parent(strict.path(), &["0000".into()], &identity).is_err());

    let corrupt = artifact_tempdir("rare-event-corrupt-winner-");
    let published = publish_artifact_dir(corrupt.path(), "0000", &checkpoint).unwrap();
    assert!(publish_artifact_dir_at_cutpoint(
        corrupt.path(),
        "0000",
        &checkpoint,
        PublicationCutPoint::BeforeNoReplace,
    )
    .is_err());
    let mut corrupt_json = fs::read(published.path().join(ARTIFACT_JSON)).unwrap();
    corrupt_json[0] ^= 1;
    fs::write(published.path().join(ARTIFACT_JSON), corrupt_json).unwrap();
    assert!(recover_artifact_parent(corrupt.path(), &["0000".into()], &identity).is_err());
    assert!(publish_artifact_dir(corrupt.path(), "0000", &checkpoint).is_err());
    assert!(fs::read_dir(corrupt.path()).unwrap().count() >= 2);

    let identical = artifact_tempdir("rare-event-identical-race-");
    let parent = Arc::new(identical.path().to_owned());
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let parent = Arc::clone(&parent);
            let barrier = Arc::clone(&barrier);
            let checkpoint = checkpoint.clone();
            std::thread::spawn(move || {
                barrier.wait();
                publish_artifact_dir(&parent, "0000", &checkpoint)
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert!(results.iter().all(Result::is_ok));
    assert_eq!(
        recover_artifact_parent(&parent, &["0000".into()], &identity)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fs::read_dir(&*parent).unwrap().count(), 1);

    let conflicting = artifact_tempdir("rare-event-conflicting-race-");
    let parent = Arc::new(conflicting.path().to_owned());
    let barrier = Arc::new(Barrier::new(2));
    let mut different_start = start.clone();
    let RareEventPayloadV1::ExecutionAttempt(payload) = &mut different_start.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Start {
        start_receipt_utc, ..
    } = &mut payload.phase
    else {
        unreachable!()
    };
    *start_receipt_utc = "2026-08-30T08:00:02.000000001Z".into();
    let envelopes = [start.clone(), different_start];
    let threads: Vec<_> = envelopes
        .into_iter()
        .map(|envelope| {
            let parent = Arc::clone(&parent);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                publish_artifact_dir(&parent, "start", &envelope)
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(
        recover_artifact_parent(&parent, &["start".into()], &identity)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fs::read_dir(&*parent).unwrap().count(), 2);

    let invalid_name = artifact_tempdir("rare-event-invalid-name-");
    fs::create_dir(invalid_name.path().join("unexpected")).unwrap();
    assert!(recover_artifact_parent(invalid_name.path(), &["start".into()], &identity).is_err());

    let finished_root = artifact_tempdir("rare-event-finished-attempt-");
    let dataset_dir = finished_root.path().join(dataset_id(&identity).unwrap());
    fs::create_dir(&dataset_dir).unwrap();
    let validated_start = begin_attempt(&dataset_dir, &start, &[], &[]).unwrap();
    let terminal = terminal_envelope(identity.clone(), &start, false, vec![]);
    finish_attempt(&dataset_dir, &validated_start, &terminal, &[]).unwrap();
    assert!(begin_attempt(&dataset_dir, &start, &[], &[]).is_err());

    let recovery_root = artifact_tempdir("rare-event-recovered-attempt-");
    let recovery_dataset = recovery_root.path().join(dataset_id(&identity).unwrap());
    fs::create_dir(&recovery_dataset).unwrap();
    let recovery_start = begin_attempt(&recovery_dataset, &start, &[], &[]).unwrap();
    let live = RecoveryTerminalV1 {
        observation_utc: "2026-08-30T08:00:04.000000000Z".into(),
        launcher_sha256: digest(0x31),
        liveness: process_liveness_observation(
            123,
            Some(123),
            Some("fixture-start-token".into()),
            Some("fixture-boot".into()),
        )
        .unwrap(),
    };
    assert!(recover_interrupted_attempt(&recovery_dataset, &recovery_start, live, &[]).is_err());
    assert!(!recovery_dataset
        .join("attempts/000000000000/terminal")
        .exists());

    let mut normalized_mismatch = process_liveness_observation(
        123,
        Some(123),
        Some("reused-start-token".into()),
        Some("fixture-boot".into()),
    )
    .unwrap();
    normalized_mismatch.observed_boot_identity = Some("substituted-boot".into());
    assert!(recover_interrupted_attempt(
        &recovery_dataset,
        &recovery_start,
        RecoveryTerminalV1 {
            observation_utc: "2026-08-30T08:00:04.000000000Z".into(),
            launcher_sha256: digest(0x31),
            liveness: normalized_mismatch,
        },
        &[],
    )
    .is_err());

    let reused = RecoveryTerminalV1 {
        observation_utc: "2026-08-30T08:00:04.000000000Z".into(),
        launcher_sha256: digest(0x31),
        liveness: process_liveness_observation(
            123,
            Some(123),
            Some("reused-start-token".into()),
            Some("fixture-boot".into()),
        )
        .unwrap(),
    };
    recover_interrupted_attempt(&recovery_dataset, &recovery_start, reused, &[]).unwrap();
    let terminal = verify_artifact_dir(
        &recovery_dataset.join("attempts/000000000000/terminal"),
        &identity,
    )
    .unwrap();
    let RareEventPayloadV1::ExecutionAttempt(payload) = &terminal.envelope().payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Terminal {
        end_time_meaning,
        outcome_observer,
        outcome,
        ..
    } = &payload.phase
    else {
        unreachable!()
    };
    assert_eq!(*end_time_meaning, EndTimeMeaningV1::ResumeObservation);
    assert!(matches!(
        outcome_observer,
        OutcomeObserverV1::ResumingLauncher { .. }
    ));
    assert!(matches!(
        outcome,
        AttemptOutcomeV1::TerminationUnobservedOnResume {}
    ));
}

#[test]
fn rare_event_execution_attempt_lineage_rejected() {
    let identity = identity(ScientificIdentityV1::target());
    let start = start_envelope(identity.clone(), not_used());
    let start_files = encode_artifact_files(&start).unwrap();
    let RareEventPayloadV1::ExecutionAttempt(start_payload) = &start.payload else {
        unreachable!()
    };
    let references = checkpoint_refs_target();
    let accelerator_sha256 = sha256_hex(&canonical_bytes(&not_used()).unwrap());
    let checkpoints = validated_checkpoint_set_fixture(
        &identity,
        references.clone(),
        &start_payload.attempt_id,
        &start_files.digest,
        ProducerBackendV1::Cpu {},
        &accelerator_sha256,
        fixture_exponent_histograms(&identity, references.len()),
    )
    .unwrap();
    let terminal = terminal_envelope(identity.clone(), &start, false, references.clone());
    let valid_start = validated_attempt(&start, &identity);
    let valid_terminal = validated_attempt(&terminal, &identity);
    validate_execution_lineage(
        &identity,
        &[valid_start.clone(), valid_terminal.clone()],
        &checkpoints,
    )
    .unwrap();

    let mut recovered_terminal = terminal.clone();
    let RareEventPayloadV1::ExecutionAttempt(payload) = &mut recovered_terminal.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Terminal {
        end_time_meaning,
        monotonic_elapsed_ns,
        outcome_observer,
        outcome,
        ..
    } = &mut payload.phase
    else {
        unreachable!()
    };
    *end_time_meaning = EndTimeMeaningV1::ResumeObservation;
    *monotonic_elapsed_ns = None;
    *outcome_observer = OutcomeObserverV1::ResumingLauncher {
        launcher_sha256: digest(0x31),
        liveness_evidence: process_liveness_observation(
            123,
            Some(123),
            Some("reused-start-token".into()),
            Some("fixture-boot".into()),
        )
        .unwrap()
        .evidence,
    };
    *outcome = AttemptOutcomeV1::TerminationUnobservedOnResume {};
    validate_execution_lineage(
        &identity,
        &[
            valid_start.clone(),
            validated_attempt(&recovered_terminal, &identity),
        ],
        &checkpoints,
    )
    .unwrap();

    let RareEventPayloadV1::ExecutionAttempt(payload) = &mut recovered_terminal.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Terminal {
        outcome_observer, ..
    } = &mut payload.phase
    else {
        unreachable!()
    };
    *outcome_observer = OutcomeObserverV1::ResumingLauncher {
        launcher_sha256: digest(0x31),
        liveness_evidence: process_liveness_observation(
            123,
            Some(123),
            Some("fixture-start-token".into()),
            Some("fixture-boot".into()),
        )
        .unwrap()
        .evidence,
    };
    assert!(validate_execution_lineage(
        &identity,
        &[
            valid_start.clone(),
            validated_attempt(&recovered_terminal, &identity),
        ],
        &checkpoints,
    )
    .is_err());

    for phases in [
        vec![valid_start.clone()],
        vec![valid_terminal.clone(), valid_start.clone()],
        vec![valid_start.clone(), valid_start.clone()],
        vec![
            valid_start.clone(),
            valid_terminal.clone(),
            valid_terminal.clone(),
        ],
        vec![
            valid_start.clone(),
            valid_terminal.clone(),
            valid_start.clone(),
            valid_terminal.clone(),
        ],
    ] {
        assert!(validate_execution_lineage(&identity, &phases, &checkpoints).is_err());
    }

    let mut omitted_terminal = terminal_envelope(identity.clone(), &start, false, references);
    let RareEventPayloadV1::ExecutionAttempt(payload) = &mut omitted_terminal.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Terminal {
        checkpoint_refs, ..
    } = &mut payload.phase
    else {
        unreachable!()
    };
    checkpoint_refs.pop();
    assert!(validate_execution_lineage(
        &identity,
        &[
            valid_start.clone(),
            validated_attempt(&omitted_terminal, &identity)
        ],
        &checkpoints,
    )
    .is_err());

    let mismatched_checkpoints = validated_checkpoint_set_fixture(
        &identity,
        checkpoint_refs_target(),
        &start_payload.attempt_id,
        &start_files.digest,
        ProducerBackendV1::Gpu {
            device_uuid: "GPU-0001".into(),
            kernel_name: "fixture_kernel".into(),
            code_object_sha256: digest(0x20),
        },
        &accelerator_sha256,
        fixture_exponent_histograms(&identity, 2_048),
    )
    .unwrap();
    assert!(validate_execution_lineage(
        &identity,
        &[valid_start.clone(), valid_terminal.clone()],
        &mismatched_checkpoints,
    )
    .is_err());

    let mut self_asserted = serde_json::to_value(&terminal).unwrap();
    self_asserted["payload"]["outcome_observer"]["observer"] = serde_json::json!("child");
    assert!(decode_envelope(&value_bytes(self_asserted)).is_err());

    let mut normalized_host_mismatch = serde_json::to_value(&start).unwrap();
    normalized_host_mismatch["payload"]["host_observation"]["cpu_model"] =
        serde_json::json!("substituted-model");
    assert!(decode_envelope(&value_bytes(normalized_host_mismatch)).is_err());
    let gpu_start = start_envelope(crate::identity(ScientificIdentityV1::coverage()), used());
    let mut normalized_gpu_mismatch = serde_json::to_value(gpu_start).unwrap();
    normalized_gpu_mismatch["payload"]["accelerator_observation"]["devices"][0]["model"] =
        serde_json::json!("substituted-gpu");
    assert!(decode_envelope(&value_bytes(normalized_gpu_mismatch)).is_err());

    let mut normalized_liveness_mismatch = process_liveness_observation(
        123,
        Some(123),
        Some("reused-start-token".into()),
        Some("fixture-boot".into()),
    )
    .unwrap();
    normalized_liveness_mismatch.observed_boot_identity = Some("substituted-boot".into());
    assert!(validate_process_liveness_observation(&normalized_liveness_mismatch).is_err());

    let mut wrong_start_link = terminal.clone();
    let RareEventPayloadV1::ExecutionAttempt(payload) = &mut wrong_start_link.payload else {
        unreachable!()
    };
    let AttemptPhaseV1::Terminal {
        attempt_start_sha256,
        ..
    } = &mut payload.phase
    else {
        unreachable!()
    };
    *attempt_start_sha256 = digest(0xfe);
    assert!(validate_execution_lineage(
        &identity,
        &[valid_start, validated_attempt(&wrong_start_link, &identity)],
        &checkpoints,
    )
    .is_err());
}
