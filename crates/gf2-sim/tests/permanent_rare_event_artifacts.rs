use gf2_sim::permanent_rare_event::artifact::*;

fn digest(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn evidence(source: &str, bytes: &[u8]) -> ObservationEvidenceV1 {
    ObservationEvidenceV1 {
        source: source.into(),
        evidence_hex: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        evidence_sha256: sha256_hex(bytes),
    }
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
        worker_configuration: worker(),
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
    HostObservationV1 {
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
        evidence: vec![evidence("fixture-host-api", b"host")],
    }
}

fn not_used() -> AcceleratorObservationV1 {
    AcceleratorObservationV1::NotUsed {
        reason: "configuration selected CPU".into(),
        evidence: vec![evidence("fixture-selection-api", b"cpu")],
    }
}

fn used() -> AcceleratorObservationV1 {
    AcceleratorObservationV1::Used {
        devices: vec![GpuObservationV1 {
            model: "fixture-gpu".into(),
            uuid: "GPU-0001".into(),
            pci_address: "0000:01:00.0".into(),
            architecture: "gfx-fixture".into(),
            driver_version: "1".into(),
            rocm_version: "1".into(),
            hip_version: "1".into(),
            kernel_name: "fixture_kernel".into(),
            code_object_sha256: digest(0x20),
            evidence: vec![evidence("fixture-driver-api", b"gpu")],
        }],
    }
}

fn invocation(identity: &RareEventDatasetIdentityV1) -> InvocationV1 {
    InvocationV1 {
        argv: vec!["permanent_rare_event".into(), "worker".into()],
        executable_path: "target/release/permanent_rare_event".into(),
        executable_sha256: identity.behavior.executable_sha256.clone(),
        process_id: 123,
        process_start_token: "fixture-start-token".into(),
        boot_identity: "fixture-boot".into(),
        configuration_path: identity.preregistration.configuration_path.clone(),
        configuration_sha256: identity.preregistration.configuration_sha256.clone(),
        effective_configuration_hex: "00".into(),
        input_resolution: vec![InputResolutionV1 {
            field: "scientific_identity".into(),
            origin: InputOriginV1::Configuration,
            value: "target".into(),
        }],
    }
}

fn start_envelope(
    identity: RareEventDatasetIdentityV1,
    accelerator: AcceleratorObservationV1,
) -> RareEventArtifactEnvelopeV1 {
    let predecessor = AttemptPredecessorV1::None;
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
        predecessor: AttemptPredecessorV1::None,
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
                AttemptOutcomeV1::Completed
            },
            checkpoint_refs: vec![],
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
        producer: ProducerBackendV1::Cpu,
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

fn target_final(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    target_final_envelope(
        identity,
        TargetResultPayloadV1 {
            expected_trajectory_count: 524_288,
            exact_result_path: "dev/simulation_results/permanent-rare-event/exact.json".into(),
            exact_result_sha256: digest(0x30),
            exact_raw_count: "1".into(),
            exact_total: "3".into(),
            exact_probability: ExactDecimalV1::new("1", "3"),
            cross_check_estimate: ExactDecimalV1::new("1", "3"),
            independent_run_variance: ExactDecimalV1::new("0", "1"),
            interval_lower: "3.333333333333333330e-1".into(),
            interval_upper: "3.333333333333333340e-1".into(),
            final_weight_ess: ExactDecimalV1::new("524288", "1"),
            ess_fraction: ExactDecimalV1::new("1", "1"),
            exponent_histogram: vec![ExponentBinV1 {
                exponent: 1,
                count: 524_288,
            }],
            run_means: vec![ExactDecimalV1::new("1", "3"); 32],
            extinction_reasons: vec![],
            degeneracy: false,
            verdict: CrossCheckVerdictV1::Agreement,
        },
        ValidatedExecutionLineage {
            attempts: vec![],
            checkpoint_refs: checkpoint_refs_target(),
        },
    )
    .unwrap()
}

fn coverage_final(identity: RareEventDatasetIdentityV1) -> RareEventArtifactEnvelopeV1 {
    let mut replicates = Vec::new();
    for (q, numerator, denominator) in [
        (3, "907", "2187"),
        (5, "17581", "78125"),
        (7, "126295", "823543"),
    ] {
        for replicate in 0..200 {
            replicates.push(CoverageReplicateV1 {
                q,
                replicate,
                exact_anchor: ExactDecimalV1::new(numerator, denominator),
                estimate: ExactDecimalV1::new(numerator, denominator),
                interval_lower: "0e+0".into(),
                interval_upper: "1.00000000000000000e+0".into(),
                contains_anchor: true,
                ess_fraction: ExactDecimalV1::new("1", "1"),
                extinction_reasons: vec![],
            });
        }
    }
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
        ValidatedExecutionLineage {
            attempts: vec![],
            checkpoint_refs: checkpoint_refs_coverage(),
        },
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
            terminal_envelope(target.clone(), &cpu_start, false),
        ),
        (coverage.clone(), gpu_start.clone()),
        (
            coverage.clone(),
            terminal_envelope(coverage.clone(), &gpu_start, true),
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
        attempt_id(&expected, 0, &AttemptPredecessorV1::None).unwrap(),
        attempt_id(
            &expected,
            1,
            &AttemptPredecessorV1::Terminal {
                terminal_sha256: digest(0xab),
            },
        )
        .unwrap()
    );
}
