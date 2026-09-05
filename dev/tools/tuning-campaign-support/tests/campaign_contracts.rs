use serde_json::json;
use tuning_campaign_support::campaign::*;

fn token(s: &str) -> Token {
    Token::new(s).unwrap()
}
fn identity(task: Task) -> UnitIdentity {
    UnitIdentity {
        protocol: token("owner-v1"),
        owner: token("core"),
        campaign_id: token("campaign"),
        phase: token("extent"),
        field: token("dot.chunk"),
        stratum: token("shape-1"),
        candidate: token("128"),
        task,
    }
}

#[test]
fn opaque_cases_preserve_order_but_reject_noncanonical_and_duplicate_keys() {
    let case = CanonicalJson::new(r#"{"z":1,"a":[true,"x"]}"#).unwrap();
    assert_eq!(case.as_str(), r#"{"z":1,"a":[true,"x"]}"#);
    assert!(CanonicalJson::new(r#"{ "z":1}"#).is_err());
    assert!(CanonicalJson::new(r#"{"a":1,"a":2}"#).is_err());
    assert!(Token::new("bad/path").is_err());
}

#[test]
fn progress_rejects_probe_wrong_identity_and_out_of_order_events() {
    let timed = identity(Task::Measure { execution: 0 });
    let case = CanonicalJson::from_serializable(&json!({"case": 1})).unwrap();
    let calibration = ProgressRecord::new(
        timed.clone(),
        case.digest(),
        ProgressKind::CalibrationComplete { calls: 8 },
    )
    .unwrap();
    let window = ProgressRecord::new(
        timed.clone(),
        case.digest(),
        ProgressKind::WindowComplete {
            repetition: 0,
            calls: 8,
            elapsed_ns: 1,
        },
    )
    .unwrap();
    let mut stream = ProgressTracker::new(timed.clone(), case.digest()).unwrap();
    assert!(stream.accept(window.clone()).is_err());
    // A rejected stream is poisoned; it can never turn partial evidence valid.
    assert!(stream.accept(calibration.clone()).is_err());
    let mut stream = ProgressTracker::new(timed, case.digest()).unwrap();
    stream.accept(calibration.clone()).unwrap();
    assert!(stream.accept(calibration.clone()).is_err());
    let mut probe = ProgressTracker::new(identity(Task::Probe), case.digest()).unwrap();
    assert!(probe.accept(calibration).is_err());
}

#[test]
fn clean_lifecycle_cannot_publish_before_observed_release() {
    let mut state = SessionLifecycle::prepared();
    assert!(state.terminal(SessionOutcome::Complete).is_err());
    assert!(state
        .work_finished(WorkEvidence {
            outcome: SessionOutcome::Complete,
            all_descendants_reaped: true,
            active_elapsed_ns: 1
        })
        .is_err());
    state
        .lock_held(LockEvidence {
            lock_path: "/tmp/campaign.lock".into(),
            holder_pid: 123,
            observation: token("inherited-descriptor"),
        })
        .unwrap();
    state
        .work_finished(WorkEvidence {
            outcome: SessionOutcome::Complete,
            all_descendants_reaped: true,
            active_elapsed_ns: 1,
        })
        .unwrap();
    state
        .wrapper_returned(WrapperEvidence {
            exit_code: Some(0),
            signal: None,
        })
        .unwrap();
    assert!(state.terminal(SessionOutcome::Complete).is_err());
    state.observe_release(ReleaseEvidence::CleanReaped).unwrap();
    state.terminal(SessionOutcome::Complete).unwrap();
    assert!(state.terminal(SessionOutcome::Complete).is_err());
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity,
};
use tuning_campaign_support::timing::{
    execution_windows_configured, TimingProgress, TimingSample, WINDOWS,
};
use tuning_campaign_support::transport::{encode_result_line, FRESH_CASE_VAR};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "campaign-layer-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn digest() -> Sha256Digest {
    Sha256Digest::of(b"contract")
}
fn resume_identity() -> ResumeIdentity {
    let hash = digest().as_str().to_string();
    ResumeIdentity {
        protocol_digest: hash.clone(),
        source_revision: "a".repeat(40),
        source_sha256: hash.clone(),
        ordered_work_manifest_sha256: hash.clone(),
        process_descriptors_sha256: hash.clone(),
        executable_sha256: BTreeMap::from([("core".into(), hash.clone())]),
        behavior_sha256: BTreeMap::from([("core".into(), hash.clone())]),
        lifecycle_schema: LIFECYCLE_SCHEMA.into(),
        lifecycle_behavior_sha256: hash,
        feature_contract: FEATURE_CONTRACT.into(),
        thread_contract: THREAD_CONTRACT.into(),
        host_identity: "host".into(),
    }
}
fn process() -> ProcessDescriptor {
    ProcessDescriptor {
        id: token("producer"),
        executable: "/tmp/producer".into(),
        executable_sha256: digest(),
        arguments: vec!["--fresh-tuning-process-child".into()],
        environment: measurement_environment(),
        working_directory: "/tmp".into(),
    }
}
fn manifest() -> OwnerManifest {
    let block = CandidateBlock {
        phase: token("extent"),
        field: token("dot.chunk"),
        stratum: token("shape-1"),
        base_candidates: vec![token("128"), token("256"), token("512")],
    };
    let mut units = Vec::new();
    for task in
        std::iter::once(Task::Probe).chain((0..5).map(|execution| Task::Measure { execution }))
    {
        for candidate in block.candidates_for(task).unwrap() {
            let mut id = identity(task);
            id.candidate = candidate.clone();
            let case =
                CanonicalJson::from_serializable(&json!({"candidate":candidate,"task":task}))
                    .unwrap();
            units.push(LaunchUnit::new(units.len() as u64, id, token("producer"), case).unwrap());
        }
    }
    let mut manifest = OwnerManifest {
        schema: MANIFEST_SCHEMA.into(),
        owner: token("core"),
        owner_protocol: token("owner-v1"),
        behavior_token: token("owner-v1"),
        campaign_id: token("campaign"),
        phases: vec![token("extent")],
        candidate_blocks: vec![block],
        counts: DeclaredCounts::for_cells(3).unwrap(),
        processes: vec![process()],
        ordered_units: units,
        manifest_sha256: digest(),
    };
    manifest.seal().unwrap();
    manifest
}
fn result(unit: &LaunchUnit) -> ChildResult {
    let samples = match unit.identity.task {
        Task::Probe => vec![],
        Task::Measure { execution } => (0..WINDOWS)
            .map(|repetition| TimingSample::new(execution, repetition, 8, repetition + 1).unwrap())
            .collect(),
    };
    ChildResult {
        schema: RESULT_SCHEMA.into(),
        identity: unit.identity.clone(),
        case_sha256: unit.case.digest(),
        outcome: ChildOutcome::Complete,
        samples,
        payload: CanonicalJson::from_serializable(&json!({"oracle":"equal"})).unwrap(),
    }
}
fn log(stage: &Path) -> ExecutionLog {
    let mut log = ExecutionLog::create_new(stage, "campaign", "s1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    log
}
fn complete_attempt(log: &mut ExecutionLog, unit: &LaunchUnit) -> ChildResult {
    let report = result(unit);
    let mut attempt = ChildAttempt::start(log, unit.clone(), token("attempt-1"), 123).unwrap();
    if !report.samples.is_empty() {
        let calibration = ProgressRecord::new(
            unit.identity.clone(),
            unit.case.digest(),
            ProgressKind::CalibrationComplete { calls: 8 },
        )
        .unwrap();
        let mut line = Vec::new();
        calibration.write_line(&mut line).unwrap();
        attempt
            .stderr_line(
                log,
                std::str::from_utf8(&line).unwrap().trim_end_matches('\n'),
            )
            .unwrap();
        for sample in &report.samples {
            let record = ProgressRecord::new(
                unit.identity.clone(),
                unit.case.digest(),
                ProgressKind::WindowComplete {
                    repetition: sample.repetition,
                    calls: sample.calls,
                    elapsed_ns: sample.elapsed_ns,
                },
            )
            .unwrap();
            let mut line = Vec::new();
            record.write_line(&mut line).unwrap();
            attempt
                .stderr_line(
                    log,
                    std::str::from_utf8(&line).unwrap().trim_end_matches('\n'),
                )
                .unwrap();
        }
    }
    attempt.stderr_line(log, "owner diagnostic").unwrap();
    let stdout = format!("{}\n", encode_result_line(&report).unwrap());
    attempt
        .exited(
            log,
            ProcessOutcome::Exited {
                pid: 123,
                exit_code: 0,
                elapsed_ns: 10,
                all_descendants_reaped: true,
            },
            stdout.as_bytes(),
            digest(),
        )
        .unwrap();
    attempt
        .owner_validated(
            log,
            &OwnerResponse::ValidateResult {
                unit_key: unit.key.clone(),
                result_sha256: Sha256Digest::of(&serde_json::to_vec(&report).unwrap()),
            },
        )
        .unwrap();
    report
}

#[test]
fn interleaved_manifest_accepts_rotated_candidate_blocks_and_rejects_mutations() {
    let manifest = manifest();
    manifest.validate().unwrap();
    let encoded = serde_json::to_vec(&manifest).unwrap();
    assert_eq!(OwnerManifest::decode(&encoded).unwrap(), manifest);
    let mut bad = manifest.clone();
    bad.counts.windows += 1;
    assert!(bad.seal().is_err());
    let mut bad = manifest.clone();
    bad.ordered_units.swap(0, 1);
    assert!(bad.seal().is_err());
    let mut bad = manifest.clone();
    bad.ordered_units[3].key = digest();
    assert!(bad.seal().is_err());
    let mut bad = manifest.clone();
    bad.phases.push(token("extent"));
    assert!(bad.seal().is_err());
    let mut bad = manifest.clone();
    bad.ordered_units.pop();
    assert!(bad.seal().is_err());
    let mut bad = manifest.clone();
    bad.manifest_sha256 = digest();
    assert!(bad.validate().is_err());
    let mut value = serde_json::to_value(&manifest).unwrap();
    value["unknown"] = json!(1);
    assert!(serde_json::from_value::<OwnerManifest>(value).is_err());
}

#[test]
fn process_paths_environment_and_staged_bytes_are_strict() {
    let valid = process();
    valid.validate().unwrap();
    for path in [
        "relative",
        "/tmp/../producer",
        "/tmp/./producer",
        "/tmp//producer",
    ] {
        let mut bad = valid.clone();
        bad.executable = path.into();
        assert!(bad.validate().is_err(), "{path}");
    }
    let mut bad = valid.clone();
    bad.environment.insert("BAD=KEY".into(), "x".into());
    assert!(bad.validate().is_err());
    let mut bad = valid.clone();
    bad.environment.remove(FRESH_CASE_VAR);
    assert!(bad.validate().is_err());
    for value in [None, Some("stable"), Some("1.95")] {
        let mut bad = valid.clone();
        if let Some(value) = value {
            bad.environment
                .insert("RUSTUP_TOOLCHAIN".into(), value.into());
        } else {
            bad.environment.remove("RUSTUP_TOOLCHAIN");
        }
        assert!(bad.validate().is_err());
    }
    let mut bad = valid.clone();
    bad.environment.insert("UNDECLARED".into(), "1".into());
    assert!(bad.validate().is_err());
    let tmp = Scratch::new();
    let file = tmp.0.join("producer");
    fs::write(&file, b"executable").unwrap();
    let mut staged = valid;
    staged.executable = file.clone();
    staged.working_directory = tmp.0.clone();
    staged.executable_sha256 = Sha256Digest::of(b"executable");
    staged.verify_staged().unwrap();
    fs::write(file, b"changed").unwrap();
    assert!(staged.verify_staged().is_err());
}

#[test]
fn progress_framing_and_result_consistency_are_exact() {
    let manifest = manifest();
    let unit = &manifest.ordered_units[3];
    let report = result(unit);
    let record = ProgressRecord::new(
        unit.identity.clone(),
        unit.case.digest(),
        ProgressKind::CalibrationComplete { calls: 8 },
    )
    .unwrap();
    let mut encoded = Vec::new();
    record.write_line(&mut encoded).unwrap();
    let line = std::str::from_utf8(&encoded)
        .unwrap()
        .trim_end_matches('\n');
    assert_eq!(
        ProgressRecord::parse_line(line).unwrap(),
        Some(record.clone())
    );
    assert!(ProgressRecord::parse_line(&format!("{line} ")).is_err());
    assert!(ProgressRecord::parse_line("GF2_TUNING_PROGRESS={}").is_err());
    assert_eq!(ProgressRecord::parse_line("diagnostic").unwrap(), None);
    let mut tracker = ProgressTracker::new(unit.identity.clone(), unit.case.digest()).unwrap();
    tracker.accept(record).unwrap();
    for sample in &report.samples {
        tracker
            .accept(
                ProgressRecord::new(
                    unit.identity.clone(),
                    unit.case.digest(),
                    ProgressKind::WindowComplete {
                        repetition: sample.repetition,
                        calls: sample.calls,
                        elapsed_ns: sample.elapsed_ns,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    assert_eq!(
        tracker
            .finish(&encode_result_line(&report).unwrap())
            .unwrap(),
        report
    );
    let mut different = report.clone();
    different.samples[0].elapsed_ns += 1;
    assert!(tracker
        .finish(&encode_result_line(&different).unwrap())
        .is_err());
    let mut probe = ProgressTracker::new(
        manifest.ordered_units[0].identity.clone(),
        manifest.ordered_units[0].case.digest(),
    )
    .unwrap();
    let probe_result = result(&manifest.ordered_units[0]);
    let line = encode_result_line(&probe_result).unwrap();
    assert!(probe.finish(&format!("{line}\n{line}\n")).is_err());
}

#[test]
fn wrong_identity_incomplete_stream_and_probe_windows_reject() {
    let m = manifest();
    let unit = &m.ordered_units[3];
    let mut tracker = ProgressTracker::new(unit.identity.clone(), unit.case.digest()).unwrap();
    let mut wrong = unit.identity.clone();
    wrong.candidate = token("wrong");
    assert!(tracker
        .accept(
            ProgressRecord::new(
                wrong,
                unit.case.digest(),
                ProgressKind::CalibrationComplete { calls: 8 }
            )
            .unwrap()
        )
        .is_err());
    let mut tracker = ProgressTracker::new(unit.identity.clone(), unit.case.digest()).unwrap();
    assert!(tracker
        .finish(&encode_result_line(&result(unit)).unwrap())
        .is_err());
    let probe = &m.ordered_units[0];
    let mut bad = result(probe);
    bad.samples.push(TimingSample::new(0, 0, 8, 1).unwrap());
    let mut tracker = ProgressTracker::new(probe.identity.clone(), probe.case.digest()).unwrap();
    assert!(tracker.finish(&encode_result_line(&bad).unwrap()).is_err());
}

#[test]
fn configured_timing_emits_outside_intervals_and_stops_on_callback_error() {
    use std::cell::Cell;
    use std::time::Duration;
    let calls = Cell::new(0u64);
    let mut events = Vec::new();
    let samples = execution_windows_configured(
        0,
        2,
        Duration::from_micros(5),
        &mut |_| calls.set(calls.get() + 1),
        |event| {
            events.push((event, calls.get()));
            Ok(())
        },
    )
    .unwrap();
    assert!(matches!(
        events[0].0,
        TimingProgress::CalibrationComplete { .. }
    ));
    assert_eq!(events.len(), samples.len() + 1);
    for (i, sample) in samples.iter().enumerate() {
        assert_eq!(events[i + 1].0, TimingProgress::WindowComplete(*sample));
        assert_eq!(events[i + 1].1 - events[i].1, sample.calls);
    }
    let mut seen = 0;
    assert!(
        execution_windows_configured(0, 1, Duration::from_micros(1), &mut |_| {}, |_| {
            seen += 1;
            Err(std::io::Error::other("writer failed"))
        })
        .is_err()
    );
    assert_eq!(seen, 1);
    for repetitions in [0, 6] {
        assert!(execution_windows_configured(
            0,
            repetitions,
            Duration::from_nanos(1),
            &mut |_| {},
            |_| Ok(())
        )
        .is_err());
    }
    assert!(execution_windows_configured(0, 1, Duration::ZERO, &mut |_| {}, |_| Ok(())).is_err());
}

#[test]
fn unclean_wrapper_needs_dead_tree_and_independent_lock_observation() {
    let mut state = SessionLifecycle::prepared();
    let lock = LockEvidence {
        lock_path: "/tmp/campaign.lock".into(),
        holder_pid: 123,
        observation: token("flock"),
    };
    state.lock_held(lock.clone()).unwrap();
    state
        .wrapper_returned(WrapperEvidence {
            exit_code: Some(1),
            signal: None,
        })
        .unwrap();
    assert!(state.observe_release(ReleaseEvidence::CleanReaped).is_err());
    state.release_unobserved().unwrap();
    assert!(state.terminal(SessionOutcome::Failed).is_err());
    let proof = IndependentReleaseEvidence {
        holder_dead: true,
        descendants_dead: false,
        lock_path: lock.lock_path,
        observed_utc: "2026-09-05T00:00:00Z".into(),
        lock_available: true,
    };
    assert!(state
        .observe_release(ReleaseEvidence::Independent {
            evidence: proof.clone()
        })
        .is_err());
    let mut proof = proof;
    proof.descendants_dead = true;
    proof.lock_available = false;
    assert!(state
        .observe_release(ReleaseEvidence::Independent {
            evidence: proof.clone()
        })
        .is_err());
    proof.lock_available = true;
    state
        .observe_release(ReleaseEvidence::Independent { evidence: proof })
        .unwrap();
    assert!(state.terminal(SessionOutcome::Complete).is_err());
    state.terminal(SessionOutcome::Failed).unwrap();
}

#[test]
fn prelock_failure_does_not_invent_release_and_timeout_never_accepts() {
    let mut state = SessionLifecycle::prepared();
    state
        .wrapper_returned(WrapperEvidence {
            exit_code: Some(1),
            signal: None,
        })
        .unwrap();
    assert!(state.observe_release(ReleaseEvidence::CleanReaped).is_err());
    state.terminal(SessionOutcome::Failed).unwrap();
    let timeout = ProcessOutcome::TimedOut {
        pid: 123,
        elapsed_ns: 120_000_000_000,
        kill_grace_exhausted: false,
        all_descendants_reaped: true,
    };
    assert!(!timeout.accepts_result().unwrap());
    let impossible = ProcessOutcome::TimedOut {
        pid: 123,
        elapsed_ns: 119_000_000_000,
        kill_grace_exhausted: false,
        all_descendants_reaped: true,
    };
    assert!(impossible.validate().is_err());
    assert!(may_launch_child(
        (SESSION_BUDGET_SECONDS - 125) * 1_000_000_000
    ));
    assert!(!may_launch_child(
        (SESSION_BUDGET_SECONDS - 125) * 1_000_000_000 + 1
    ));
}

fn descriptor(stage: &Path) -> SessionDescriptor {
    SessionDescriptor {
        schema: LIFECYCLE_SCHEMA.into(),
        campaign_id: token("campaign"),
        session_id: token("s1"),
        channels: SessionChannels::for_stage(stage).unwrap(),
        identity: resume_identity(),
        counts: DeclaredCounts::for_cells(2).unwrap(),
        lock_path: "/tmp/campaign.lock".into(),
    }
}

#[test]
fn durable_descriptors_reject_concurrent_writer_reuse_and_identity_change() {
    let tmp = Scratch::new();
    let descriptor = descriptor(&tmp.0);
    let mut store = SessionStore::prepare(descriptor.clone()).unwrap();
    assert!(SessionStore::reopen(descriptor.clone()).is_err());
    store.consume_mode(SessionMode::RunSession).unwrap();
    assert!(store.consume_mode(SessionMode::RunSession).is_err());
    drop(store);
    assert!(SessionStore::prepare(descriptor.clone()).is_err());
    let mut changed = descriptor.clone();
    changed.identity.host_identity = "other-host".into();
    assert!(SessionStore::reopen(changed).is_err());
    let mut reopened = SessionStore::reopen(descriptor).unwrap();
    assert!(reopened.consume_mode(SessionMode::RunSession).is_err());
}

#[test]
fn transition_projection_recovers_once_after_durable_state_commit() {
    let tmp = Scratch::new();
    let descriptor = descriptor(&tmp.0);
    let mut log = log(&tmp.0);
    let mut store = SessionStore::prepare(descriptor.clone()).unwrap();
    store.consume_mode(SessionMode::RunSession).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::LockHeld {
                evidence: LockEvidence {
                    lock_path: descriptor.lock_path.clone(),
                    holder_pid: 123,
                    observation: token("inherited"),
                },
            },
        )
        .unwrap();
    let prefix = fs::read(log.path()).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::WorkFinished {
                evidence: WorkEvidence {
                    outcome: SessionOutcome::BudgetExhausted,
                    all_descendants_reaped: true,
                    active_elapsed_ns: 1,
                },
            },
        )
        .unwrap();
    drop(store);
    drop(log);
    // Simulate the crash boundary after immutable state commit, before its journal append.
    fs::write(tmp.0.join("execution.log"), &prefix).unwrap();
    // Journal low-level reopen keeps the same live session for projection recovery.
    let mut log =
        ExecutionLog::reopen_active(tmp.0.join("execution.log"), "campaign", "s1").unwrap();
    let store = SessionStore::reopen(descriptor).unwrap();
    store.reconcile_journal(&mut log).unwrap();
    let sequence = log.next_sequence();
    store.reconcile_journal(&mut log).unwrap();
    assert_eq!(log.next_sequence(), sequence);
}

#[test]
fn checkpoints_bind_progress_then_reconcile_missing_acceptance_once() {
    let tmp = Scratch::new();
    let m = manifest();
    let unit = &m.ordered_units[3];
    let mut log = log(&tmp.0);
    let report = complete_attempt(&mut log, unit);
    let mut store =
        CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
            .unwrap();
    let before = fs::read(log.path()).unwrap();
    accept_checkpoint(&mut log, &mut store, unit, &report).unwrap();
    let (saved, bound): (LaunchUnit, BoundResult) = store.load(unit.key.as_str()).unwrap();
    assert_eq!(&saved, unit);
    assert_eq!(bound.result, report);
    drop(log);
    fs::write(tmp.0.join("execution.log"), before).unwrap();
    let mut log =
        ExecutionLog::reopen_active(tmp.0.join("execution.log"), "campaign", "s1").unwrap();
    let bundle = reconcile_checkpoints(&mut log, &store, &m).unwrap();
    assert_eq!(bundle.accepted.len(), 1);
    bundle.validate(&m, false).unwrap();
    assert!(bundle.validate(&m, true).is_err());
    let sequence = log.next_sequence();
    reconcile_checkpoints(&mut log, &store, &m).unwrap();
    assert_eq!(sequence, log.next_sequence());
    let mut changed = fs::read(log.path()).unwrap();
    changed[10] ^= 1;
    fs::write(log.path(), changed).unwrap();
    assert!(reconcile_checkpoints(&mut log, &store, &m).is_err());
}

#[test]
fn missing_progress_or_validation_never_accepts_a_checkpoint() {
    let tmp = Scratch::new();
    let m = manifest();
    let unit = &m.ordered_units[3];
    let mut log = log(&tmp.0);
    let mut store =
        CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
            .unwrap();
    assert!(accept_checkpoint(&mut log, &mut store, unit, &result(unit)).is_err());
    assert!(store.completed_keys().is_empty());
    let mut attempt = ChildAttempt::start(&mut log, unit.clone(), token("attempt"), 123).unwrap();
    assert!(attempt
        .stderr_line(&mut log, "GF2_TUNING_PROGRESS={}")
        .is_err());
    assert!(attempt
        .exited(
            &mut log,
            ProcessOutcome::Exited {
                pid: 123,
                exit_code: 0,
                elapsed_ns: 1,
                all_descendants_reaped: true
            },
            encode_result_line(&result(unit)).unwrap().as_bytes(),
            digest()
        )
        .is_err());
    assert!(accept_checkpoint(&mut log, &mut store, unit, &result(unit)).is_err());
}

#[test]
fn pending_recovery_requires_synced_journal_handshake_and_is_idempotent() {
    let tmp = Scratch::new();
    let root = tmp.0.join("checkpoints");
    drop(CheckpointStore::create_new(&root, "campaign", resume_identity()).unwrap());
    fs::write(
        root.join("pending")
            .join(format!(".{}.json.tmp-1-2-3", "ab".repeat(32))),
        b"partial",
    )
    .unwrap();
    let mut store = CheckpointStore::resume(&root, "campaign", resume_identity()).unwrap();
    let recovery = store.pending_recovery().unwrap().clone();
    let mut log = log(&tmp.0);
    acknowledge_pending_recovery(&mut log, &mut store, &token("campaign")).unwrap();
    assert!(store.pending_recovery().is_none());
    let records =
        ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| r.event == JournalEvent::PendingRecovery)
            .count(),
        1
    );
    assert_eq!(
        records.last().unwrap().details,
        serde_json::to_value(recovery).unwrap()
    );
    let sequence = log.next_sequence();
    acknowledge_pending_recovery(&mut log, &mut store, &token("campaign")).unwrap();
    assert_eq!(sequence, log.next_sequence());
}

#[test]
fn derived_manifests_cannot_change_reserved_slots_or_input_digest() {
    let m = manifest();
    let request = DeriveManifestRequest {
        campaign_id: m.campaign_id.clone(),
        original_manifest_sha256: m.manifest_sha256.clone(),
        reserved_units: m.ordered_units.clone(),
        accepted_inputs: ArtifactIdentity {
            path: "/tmp/inputs.json".into(),
            sha256: digest(),
        },
    };
    let mut derived = DerivedManifest {
        original_manifest_sha256: m.manifest_sha256.clone(),
        accepted_inputs_sha256: digest(),
        units: request.reserved_units.clone(),
        derivation: CanonicalJson::new("{}").unwrap(),
    };
    derived.units[0].case = CanonicalJson::new("{\"conditional\":true}").unwrap();
    derived.validate(&request).unwrap();
    let effective = m.apply_derivation(&request, &derived).unwrap();
    assert_ne!(effective.manifest_sha256, m.manifest_sha256);
    assert_eq!(
        effective.ordered_units[0].identity,
        m.ordered_units[0].identity
    );
    assert_eq!(effective.ordered_units[0].case, derived.units[0].case);
    derived.units[0].ordinal += 1;
    assert!(derived.validate(&request).is_err());
    derived.units = request.reserved_units.clone();
    derived.accepted_inputs_sha256 = Sha256Digest::of(b"other");
    assert!(derived.validate(&request).is_err());
}

fn checksum(
    log: &mut ExecutionLog,
    descriptor: &SessionDescriptor,
    path: &Path,
) -> ArtifactIdentity {
    let pin = SessionChecksum {
        schema: "tuning-campaign-session-checksum-v1".into(),
        descriptor_sha256: descriptor.digest().unwrap(),
        journal_prefix: JournalPrefix::sync(log, &descriptor.campaign_id).unwrap(),
        artifacts: vec![],
    };
    pin.validate(descriptor, log.path()).unwrap();
    let encoded = serde_json::to_vec(&pin).unwrap();
    tuning_campaign_support::journal::atomic_write_new(path, &encoded).unwrap();
    ArtifactIdentity {
        path: path.into(),
        sha256: Sha256Digest::of(&encoded),
    }
}

#[test]
fn budget_terminal_checksum_gap_and_same_identity_resume_preserve_the_prefix() {
    let tmp = Scratch::new();
    let first = descriptor(&tmp.0);
    let mut log = log(&tmp.0);
    let mut store = SessionStore::prepare(first.clone()).unwrap();
    let mut announcement = Vec::new();
    store
        .announce_prepared(&mut log, &mut announcement)
        .unwrap();
    assert_eq!(
        std::str::from_utf8(&announcement).unwrap(),
        format!("GF2_CAMPAIGN_EXECUTION_LOG={}\n", log.path().display())
    );
    assert!(store
        .transition(
            &mut log,
            SessionTransition::LockHeld {
                evidence: LockEvidence {
                    lock_path: first.lock_path.clone(),
                    holder_pid: 123,
                    observation: token("inherited")
                }
            }
        )
        .is_err());
    store.consume_mode(SessionMode::RunSession).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::LockHeld {
                evidence: LockEvidence {
                    lock_path: first.lock_path.clone(),
                    holder_pid: 123,
                    observation: token("inherited"),
                },
            },
        )
        .unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::WorkFinished {
                evidence: WorkEvidence {
                    outcome: SessionOutcome::BudgetExhausted,
                    all_descendants_reaped: true,
                    active_elapsed_ns: 10_800_000_000_000,
                },
            },
        )
        .unwrap();
    drop(store);
    let mut store = SessionStore::reopen(first.clone()).unwrap();
    store.consume_mode(SessionMode::FinalizeSession).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::WrapperReturned {
                evidence: WrapperEvidence {
                    exit_code: Some(0),
                    signal: None,
                },
            },
        )
        .unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::LockRelease {
                evidence: ReleaseEvidence::CleanReaped,
            },
        )
        .unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::Terminal {
                outcome: SessionOutcome::BudgetExhausted,
            },
        )
        .unwrap();
    let terminal_sequence = log.next_sequence();
    drop(store);
    drop(log);
    // Finalizer was killed after synced terminal, before checksum publication.
    let store = SessionStore::reopen(first.clone()).unwrap();
    let mut log =
        ExecutionLog::reopen_active(tmp.0.join("execution.log"), "campaign", "s1").unwrap();
    store.reconcile_journal(&mut log).unwrap();
    assert_eq!(log.next_sequence(), terminal_sequence);
    let pin = checksum(&mut log, &first, &tmp.0.join("s1-checksum.json"));
    store.retire(&mut log, &pin).unwrap();
    drop(log);
    let mut second = first.clone();
    second.session_id = token("s2");
    let store = SessionStore::prepare(second).unwrap();
    let mut log = ExecutionLog::resume(tmp.0.join("execution.log"), "campaign", "s2").unwrap();
    store.announce_prepared(&mut log, &mut Vec::new()).unwrap();
    let saved: SessionChecksum = pin.read().unwrap();
    saved.validate(&first, log.path()).unwrap();
}

#[test]
fn recovery_requires_release_then_censored_interruption_and_allows_resume() {
    let tmp = Scratch::new();
    let first = descriptor(&tmp.0);
    let mut log = log(&tmp.0);
    let mut store = SessionStore::prepare(first.clone()).unwrap();
    store.consume_mode(SessionMode::RunSession).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::LockHeld {
                evidence: LockEvidence {
                    lock_path: first.lock_path.clone(),
                    holder_pid: 123,
                    observation: token("inherited"),
                },
            },
        )
        .unwrap();
    drop(store);
    let mut store = SessionStore::reopen(first.clone()).unwrap();
    store
        .recover_transition(&mut log, SessionTransition::ReleaseUnobserved)
        .unwrap();
    assert!(store
        .recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true
            }
        )
        .is_err());
    assert!(SessionStore::prepare(first.clone()).is_err());
    store
        .recover_transition(
            &mut log,
            SessionTransition::LockRelease {
                evidence: ReleaseEvidence::Independent {
                    evidence: IndependentReleaseEvidence {
                        holder_dead: true,
                        descendants_dead: true,
                        lock_path: first.lock_path.clone(),
                        observed_utc: "2026-09-05T00:00:00Z".into(),
                        lock_available: true,
                    },
                },
            },
        )
        .unwrap();
    store
        .recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true,
            },
        )
        .unwrap();
    let pin = checksum(&mut log, &first, &tmp.0.join("interrupted-checksum.json"));
    store.retire(&mut log, &pin).unwrap();
    drop(log);
    let mut second = first;
    second.session_id = token("s2");
    let store = SessionStore::prepare(second).unwrap();
    let mut log = ExecutionLog::resume(tmp.0.join("execution.log"), "campaign", "s2").unwrap();
    store.announce_prepared(&mut log, &mut Vec::new()).unwrap();
}

#[test]
fn result_digest_excludes_stdout_framing_and_environment_is_canonical() {
    let m = manifest();
    let result = result(&m.ordered_units[0]);
    let body = serde_json::to_vec(&result).unwrap();
    assert_eq!(result.digest().unwrap(), Sha256Digest::of(&body));
    assert_ne!(
        result.digest().unwrap(),
        Sha256Digest::of(encode_result_line(&result).unwrap().as_bytes())
    );
    let environment = measurement_environment();
    assert_eq!(environment["RUSTUP_TOOLCHAIN"], TOOLCHAIN_CONTRACT);
    let mut descriptor = process();
    descriptor.environment = environment;
    descriptor.validate().unwrap();
}

#[test]
fn candidate_blocks_require_exact_three_candidate_rotation_and_interleaving() {
    let m = manifest();
    let expected = [
        ["128", "256", "512"], // probes
        ["128", "256", "512"], // execution 0
        ["128", "512", "256"], // execution 1: rotate then reverse
        ["512", "128", "256"], // execution 2
        ["512", "256", "128"], // execution 3
        ["256", "512", "128"], // execution 4
    ];
    for (block, order) in m.ordered_units.chunks_exact(3).zip(expected) {
        assert_eq!(
            block
                .iter()
                .map(|unit| unit.identity.candidate.as_str())
                .collect::<Vec<_>>(),
            order
        );
        assert!(block
            .iter()
            .all(|unit| unit.identity.task == block[0].identity.task));
    }
    let mut per_candidate = m.clone();
    per_candidate.ordered_units.sort_by(|a, b| {
        a.identity
            .candidate
            .cmp(&b.identity.candidate)
            .then(a.ordinal.cmp(&b.ordinal))
    });
    for (ordinal, unit) in per_candidate.ordered_units.iter_mut().enumerate() {
        unit.ordinal = ordinal as u64;
    }
    assert!(per_candidate.seal().is_err());
    let mut wrong_rotation = m.clone();
    wrong_rotation.ordered_units.swap(6, 7);
    for (ordinal, unit) in wrong_rotation.ordered_units.iter_mut().enumerate() {
        unit.ordinal = ordinal as u64;
    }
    assert!(wrong_rotation.seal().is_err());
    let mut undeclared = m.clone();
    undeclared.candidate_blocks[0].base_candidates.swap(0, 1);
    assert!(undeclared.seal().is_err());
    let two = CandidateBlock {
        phase: token("phase"),
        field: token("field"),
        stratum: token("stratum"),
        base_candidates: vec![token("a"), token("b")],
    };
    for execution in 0..5 {
        assert_eq!(
            two.candidates_for(Task::Measure { execution }).unwrap(),
            two.base_candidates
        );
    }
}

fn rewrite_records(
    log: &ExecutionLog,
    mut change: impl FnMut(&mut Vec<tuning_campaign_support::journal::JournalRecord>),
) {
    let mut records =
        ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), log.campaign_id()).unwrap();
    change(&mut records);
    let mut encoded = Vec::new();
    for (sequence, record) in records.iter_mut().enumerate() {
        record.sequence = sequence as u64;
        encoded.extend(serde_json::to_vec(record).unwrap());
        encoded.push(b'\n');
    }
    fs::write(log.path(), encoded).unwrap();
}

#[test]
fn acceptance_reconciliation_rejects_unknown_keys_wrong_cases_and_duplicates() {
    for mutation in 0..4 {
        let tmp = Scratch::new();
        let m = manifest();
        let unit = &m.ordered_units[0];
        let mut log = log(&tmp.0);
        let report = complete_attempt(&mut log, unit);
        let mut checkpoints =
            CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
                .unwrap();
        accept_checkpoint(&mut log, &mut checkpoints, unit, &report).unwrap();
        rewrite_records(&log, |records| {
            let acceptance = records
                .iter_mut()
                .find(|record| record.event == JournalEvent::CheckpointAccepted)
                .unwrap();
            match mutation {
                0 => {
                    acceptance.details["unit_key"] = json!(digest());
                }
                1 => {
                    acceptance.case = Some(json!({"wrong":"case"}));
                }
                2 => {
                    let duplicate = acceptance.clone();
                    records.push(duplicate);
                }
                _ => {
                    acceptance.details["unit_key"] = json!(m.ordered_units[1].key);
                    acceptance.case =
                        Some(serde_json::to_value(&m.ordered_units[1].identity).unwrap());
                }
            }
        });
        drop(log);
        let mut log =
            ExecutionLog::reopen_active(tmp.0.join("execution.log"), "campaign", "s1").unwrap();
        assert!(reconcile_checkpoints(&mut log, &checkpoints, &m).is_err());
        assert!(
            reconcile_campaign_checkpoints(&mut log, &checkpoints, std::slice::from_ref(&m))
                .is_err()
        );
    }
}

#[test]
fn every_bound_child_record_requires_its_exact_structured_case() {
    for event in [
        JournalEvent::ChildSpawn,
        JournalEvent::ExecutionProgress,
        JournalEvent::WindowProgress,
        JournalEvent::ChildDiagnostic,
        JournalEvent::ChildExit,
        JournalEvent::ResultValidated,
    ] {
        let tmp = Scratch::new();
        let m = manifest();
        let unit = &m.ordered_units[3];
        let mut log = log(&tmp.0);
        let report = complete_attempt(&mut log, unit);
        rewrite_records(&log, |records| {
            records
                .iter_mut()
                .find(|record| record.event == event)
                .unwrap()
                .case = Some(json!({"wrong":"case"}));
        });
        let mut checkpoints =
            CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
                .unwrap();
        assert!(
            accept_checkpoint(&mut log, &mut checkpoints, unit, &report).is_err(),
            "{event:?}"
        );
        assert!(checkpoints.completed_keys().is_empty());
    }
}

#[test]
fn prefix_replay_rejects_overlapping_spawns_and_resampling_before_validation() {
    for insert_after in [JournalEvent::ChildSpawn, JournalEvent::ChildExit] {
        let tmp = Scratch::new();
        let m = manifest();
        let unit = &m.ordered_units[0];
        let mut log = log(&tmp.0);
        let report = complete_attempt(&mut log, unit);
        rewrite_records(&log, |records| {
            let mut spawn = records
                .iter()
                .find(|record| record.event == JournalEvent::ChildSpawn)
                .unwrap()
                .clone();
            spawn.details["attempt"] = json!("overlapping-attempt");
            spawn.details["pid"] = json!(456);
            let index = records
                .iter()
                .position(|record| record.event == insert_after)
                .unwrap();
            records.insert(index + 1, spawn);
        });
        drop(log);
        let mut log =
            ExecutionLog::reopen_active(tmp.0.join("execution.log"), "campaign", "s1").unwrap();
        let mut checkpoints =
            CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
                .unwrap();
        assert!(accept_checkpoint(&mut log, &mut checkpoints, unit, &report).is_err());
        assert!(checkpoints.completed_keys().is_empty());
    }
}

#[test]
fn invalid_utf8_stdout_keeps_exact_exit_bytes_and_cannot_accept() {
    let tmp = Scratch::new();
    let m = manifest();
    let unit = &m.ordered_units[0];
    let mut log = log(&tmp.0);
    let mut attempt =
        ChildAttempt::start(&mut log, unit.clone(), token("invalid-output"), 123).unwrap();
    let stdout = b"GF2_TUNING_RESULT=\xff\n";
    assert!(attempt
        .exited(
            &mut log,
            ProcessOutcome::Exited {
                pid: 123,
                exit_code: 0,
                elapsed_ns: 1,
                all_descendants_reaped: true
            },
            stdout,
            digest()
        )
        .is_err());
    let records =
        ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
    let exit = records.last().unwrap();
    assert_eq!(exit.event, JournalEvent::ChildExit);
    assert_eq!(
        exit.details["stdout_sha256"],
        json!(Sha256Digest::of(stdout))
    );
    let mut checkpoints =
        CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
            .unwrap();
    assert!(accept_checkpoint(&mut log, &mut checkpoints, unit, &result(unit)).is_err());
}

#[test]
fn active_claim_and_torn_log_recover_in_place_before_release_and_replacement() {
    use std::io::Write;
    let tmp = Scratch::new();
    let first = descriptor(&tmp.0);
    let mut log = log(&tmp.0);
    let mut store = SessionStore::prepare(first.clone()).unwrap();
    store.consume_mode(SessionMode::RunSession).unwrap();
    store
        .transition(
            &mut log,
            SessionTransition::LockHeld {
                evidence: LockEvidence {
                    lock_path: first.lock_path.clone(),
                    holder_pid: 123,
                    observation: token("inherited"),
                },
            },
        )
        .unwrap();
    drop(store);
    drop(log);
    let torn = b"{\"uncommitted\":true";
    std::fs::OpenOptions::new()
        .append(true)
        .open(tmp.0.join("execution.log"))
        .unwrap()
        .write_all(torn)
        .unwrap();
    let mut second = first.clone();
    second.session_id = token("s2");
    assert!(SessionStore::prepare(second.clone()).is_err());
    let mut store = SessionStore::reopen(first.clone()).unwrap();
    let log = store.repair_log().unwrap();
    let repaired =
        ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
    assert_eq!(repaired.last().unwrap().event, JournalEvent::Recovery);
    assert_eq!(log.session_id(), "s1");
    assert!(repaired.iter().all(|record| !matches!(
        record.event,
        JournalEvent::Interrupted | JournalEvent::SessionStart | JournalEvent::LockRelease
    )));
    let diagnostic = repaired.last().unwrap().details["diagnostic_path"]
        .as_str()
        .unwrap();
    assert_eq!(fs::read(diagnostic).unwrap(), torn);
    let sequence = log.next_sequence();
    drop(log);
    let mut log = store.repair_log().unwrap();
    assert_eq!(log.next_sequence(), sequence);
    store
        .recover_transition(&mut log, SessionTransition::ReleaseUnobserved)
        .unwrap();
    assert!(store
        .recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true
            }
        )
        .is_err());
    store
        .recover_transition(
            &mut log,
            SessionTransition::LockRelease {
                evidence: ReleaseEvidence::Independent {
                    evidence: IndependentReleaseEvidence {
                        holder_dead: true,
                        descendants_dead: true,
                        lock_path: first.lock_path.clone(),
                        observed_utc: "2026-09-05T00:00:00Z".into(),
                        lock_available: true,
                    },
                },
            },
        )
        .unwrap();
    store
        .recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true,
            },
        )
        .unwrap();
    let pin = checksum(&mut log, &first, &tmp.0.join("repaired-checksum.json"));
    store.retire(&mut log, &pin).unwrap();
    drop(log);
    let store = SessionStore::prepare(second).unwrap();
    let mut log = ExecutionLog::resume(tmp.0.join("execution.log"), "campaign", "s2").unwrap();
    store.announce_prepared(&mut log, &mut Vec::new()).unwrap();
    assert_eq!(log.session_id(), "s2");
}

#[test]
fn campaign_reconciliation_validates_the_complete_all_owner_universe() {
    let tmp = Scratch::new();
    let first = manifest();
    let mut second = first.clone();
    second.owner = token("algebra");
    second.owner_protocol = token("algebra-v1");
    second.behavior_token = token("algebra-v1");
    for unit in &mut second.ordered_units {
        unit.identity.owner = second.owner.clone();
        unit.identity.protocol = second.owner_protocol.clone();
        unit.key = unit.identity.key().unwrap();
    }
    second.seal().unwrap();
    let mut log = log(&tmp.0);
    let mut checkpoints =
        CheckpointStore::create_new(tmp.0.join("checkpoints"), "campaign", resume_identity())
            .unwrap();
    for manifest in [&first, &second] {
        let unit = &manifest.ordered_units[0];
        let result = complete_attempt(&mut log, unit);
        accept_checkpoint(&mut log, &mut checkpoints, unit, &result).unwrap();
    }
    assert!(reconcile_checkpoints(&mut log, &checkpoints, &first).is_err());
    let bundles =
        reconcile_campaign_checkpoints(&mut log, &checkpoints, &[first.clone(), second.clone()])
            .unwrap();
    assert_eq!(bundles.len(), 2);
    assert_eq!(bundles[0].accepted[0].unit.identity.owner, first.owner);
    assert_eq!(bundles[1].accepted[0].unit.identity.owner, second.owner);
    bundles[0].validate(&first, false).unwrap();
    bundles[1].validate(&second, false).unwrap();
}
