use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tuning_campaign_support::journal::{
    atomic_replace, atomic_write_new, CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity,
    TerminalState,
};
use tuning_campaign_support::seed::{bank_role, fixture_seed, EXTENT_SEED_ROOT};
use tuning_campaign_support::statistics::{
    analyze_extent, analyze_gemm, analyze_joint_vector, classify_curve, empirical_summary,
    select_threshold, CandidateSeries, CurveShape, DecisionReason, GemmGridBoundary,
    JointVectorReason, ThresholdDirection, ThresholdFallback, ThresholdPoint, ThresholdSelection,
    WindowStatistics,
};
use tuning_campaign_support::timing::{
    calibrated_calls_with, execution_window_start, TimingSample, MAX_CALLS,
};
use tuning_campaign_support::transport::{
    decode_case, encode_case, encode_result_line, parse_result, FRESH_CASE_VALUE,
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Message {
    owner: String,
    value: u64,
}

fn scratch(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "tuning-campaign-support-{name}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn child_v2_framing_is_canonical_and_fail_closed() {
    let message = Message {
        owner: "core".into(),
        value: 7,
    };
    let encoded = encode_case(&message).unwrap();
    assert_eq!(decode_case::<Message>(&encoded).unwrap(), message);
    assert!(decode_case::<Message>(&format!("{encoded}\n")).is_err());
    assert!(decode_case::<Message>(r#"{"owner":"core","value":7,"extra":0}"#).is_err());

    let line = encode_result_line(&message).unwrap();
    assert_eq!(parse_result::<Message>(&line).unwrap(), message);
    assert!(parse_result::<Message>(&format!("noise\n{line}")).is_err());
    assert!(parse_result::<Message>(&format!("{line}\n{line}")).is_err());
    assert!(parse_result::<Message>("{}").is_err());
    assert_eq!(FRESH_CASE_VALUE, "child-v2");
}

#[test]
fn call_calibration_and_window_rotation_preserve_the_protocol() {
    let mut observed = Vec::new();
    let calls = calibrated_calls_with(Duration::from_millis(250), &mut |_| {}, |count, body| {
        observed.push(count);
        for index in 0..count {
            body((index as usize) & 7);
        }
        Duration::from_millis(count)
    });
    assert_eq!(observed, [1, 2, 4, 8, 16, 32]);
    assert_eq!(calls, 250);
    assert_eq!(execution_window_start(0, 0), 0);
    assert_eq!(execution_window_start(1, 0), 5);
    assert_eq!(execution_window_start(1, 4), 1);
    assert_eq!(MAX_CALLS, 1 << 32);
    assert!(TimingSample::new(0, 0, 0, 1).is_err());
    assert!(TimingSample::new(0, 0, 1, 0).is_err());
}

fn series(
    candidate: u64,
    schedule: &[&str],
    strata: &[&[[f64; 5]]],
) -> CandidateSeries<u64, String> {
    CandidateSeries {
        candidate,
        schedules: schedule.iter().map(|value| (*value).to_owned()).collect(),
        strata: strata
            .iter()
            .flat_map(|group| group.iter().copied())
            .map(|values| values.to_vec())
            .collect(),
    }
}

#[test]
fn extent_argmin_handles_unique_wins_plateaus_and_curve_reversals() {
    let a = [[10.0; 5]];
    let b = [[8.0; 5]];
    let c = [[9.0; 5]];
    let candidates = [
        series(1, &["s1"], &[&a]),
        series(2, &["s2"], &[&b]),
        series(3, &["s3"], &[&c]),
    ];
    let decision = analyze_extent(&candidates, &1).unwrap();
    assert_eq!(decision.selected, 2);
    assert_eq!(decision.reason, DecisionReason::SelectedNonDefault);
    assert_eq!(decision.aggregate_curve, CurveShape::Unimodal);

    let plateau = [
        series(1, &["s1"], &[&a]),
        series(2, &["same"], &[&b]),
        series(3, &["same"], &[&b]),
    ];
    let decision = analyze_extent(&plateau, &1).unwrap();
    assert_eq!(decision.selected, 1);
    assert_eq!(decision.reason, DecisionReason::StructuralScheduleTie);

    let nonmonotone = classify_curve(&[&[10.0; 5][..], &[12.0; 5][..], &[8.0; 5][..]]).unwrap();
    assert_eq!(nonmonotone, CurveShape::NonMonotone);
}

fn threshold_stat(median: f64, relative_iqr: f64) -> WindowStatistics {
    WindowStatistics {
        median,
        relative_iqr,
        samples: Vec::new(),
    }
}

#[test]
fn retained_threshold_rule_keeps_its_existing_direction_and_noise_semantics() {
    let points = [
        ThresholdPoint {
            size: 16,
            conservative: Some(threshold_stat(100.0, 0.01)),
            asymptotic: Some(threshold_stat(120.0, 0.01)),
        },
        ThresholdPoint {
            size: 32,
            conservative: Some(threshold_stat(100.0, 0.01)),
            asymptotic: Some(threshold_stat(80.0, 0.01)),
        },
        ThresholdPoint {
            size: 64,
            conservative: Some(threshold_stat(100.0, 0.01)),
            asymptotic: Some(threshold_stat(70.0, 0.01)),
        },
    ];
    assert!(matches!(
        select_threshold(&points, 1024, ThresholdDirection::LowerBound),
        ThresholdSelection::Crossover {
            value: 32,
            crossover: 32,
            ..
        }
    ));
    assert!(matches!(
        select_threshold(&points, 1024, ThresholdDirection::UpperBound { floor: 0 }),
        ThresholdSelection::Crossover {
            value: 16,
            crossover: 32,
            ..
        }
    ));
}

fn scalar_series<C: Copy>(candidate: C, values: f64) -> CandidateSeries<C, String> {
    CandidateSeries {
        candidate,
        schedules: vec![format!("schedule-{values}")],
        strata: vec![vec![values; 5]],
    }
}

#[test]
fn coupled_gemm_requires_all_slices_and_one_strict_pair_winner() {
    let rows = [16, 32, 64];
    let cols = [32, 64, 128];
    let candidates: Vec<_> = rows
        .into_iter()
        .flat_map(|row| {
            cols.into_iter().map(move |col| {
                let value =
                    10.0 + if row == 32 { 0.0 } else { 2.0 } + if col == 64 { 0.0 } else { 1.0 };
                scalar_series((row, col), value)
            })
        })
        .collect();
    let decision = analyze_gemm(&candidates, (16, 32)).unwrap();
    assert_eq!(decision.selected, (32, 64));
    assert_eq!(decision.reason, DecisionReason::SelectedNonDefault);
    assert_eq!(
        decision.grid_boundary_limited,
        Some(GemmGridBoundary {
            row: false,
            column: false,
        })
    );
    assert!(decision
        .slices
        .iter()
        .all(|slice| slice.shape == CurveShape::Unimodal));

    let mut reversed = candidates;
    reversed[0].strata[0] = vec![10.0; 5];
    reversed[1].strata[0] = vec![12.0; 5];
    reversed[2].strata[0] = vec![8.0; 5];
    let decision = analyze_gemm(&reversed, (16, 32)).unwrap();
    assert_eq!(decision.selected, (16, 32));
    assert_eq!(decision.reason, DecisionReason::NonMonotoneCurve);
}

#[test]
fn coupled_gemm_records_corner_edge_and_interior_grid_boundaries() {
    let rows = [16, 32, 64];
    let cols = [32, 64, 128];
    for (target_row, target_col, expected) in [
        (
            0,
            0,
            GemmGridBoundary {
                row: true,
                column: true,
            },
        ),
        (
            0,
            1,
            GemmGridBoundary {
                row: true,
                column: false,
            },
        ),
        (
            1,
            1,
            GemmGridBoundary {
                row: false,
                column: false,
            },
        ),
    ] {
        let candidates: Vec<_> = (0_usize..3)
            .flat_map(|row| {
                (0_usize..3).map(move |col| {
                    let value = 10.0
                        + 3.0 * row.abs_diff(target_row) as f64
                        + 2.0 * col.abs_diff(target_col) as f64;
                    scalar_series((rows[row], cols[col]), value)
                })
            })
            .collect();
        let decision = analyze_gemm(&candidates, (64, 128)).unwrap();
        assert_eq!(decision.selected, (rows[target_row], cols[target_col]));
        assert_eq!(decision.grid_boundary_limited, Some(expected));
    }
}

#[test]
fn coupled_gemm_ties_and_cross_stratum_regressions_keep_the_pair_default() {
    let rows = [16, 32, 64];
    let cols = [32, 64, 128];
    let base: Vec<_> = rows
        .into_iter()
        .flat_map(|row| {
            cols.into_iter().map(move |col| {
                let value =
                    10.0 + if row == 32 { 0.0 } else { 2.0 } + if col == 64 { 0.0 } else { 1.0 };
                scalar_series((row, col), value)
            })
        })
        .collect();
    let mut tied = base;
    tied[3].strata[0] = tied[4].strata[0].clone();
    let decision = analyze_gemm(&tied, (16, 32)).unwrap();
    assert_eq!(decision.selected, (16, 32));
    assert_eq!(decision.reason, DecisionReason::UnresolvedMinimum);

    let cross: Vec<_> = (0..3)
        .flat_map(|row| {
            (0..3).map(move |col| CandidateSeries {
                candidate: (rows[row], cols[col]),
                schedules: vec![format!("s0-{row}-{col}"), format!("s1-{row}-{col}")],
                strata: vec![
                    vec![10.0 + row as f64 + col as f64; 5],
                    vec![10.0 + 20.0 * row.abs_diff(1) as f64 + 20.0 * col.abs_diff(1) as f64; 5],
                ],
            })
        })
        .collect();
    let decision = analyze_gemm(&cross, (16, 32)).unwrap();
    assert_eq!(decision.selected, (16, 32));
    assert_eq!(decision.reason, DecisionReason::CrossStratumConflict);
}

#[test]
fn m4rm_joint_vector_rejects_any_stratum_regression() {
    let conservative = CandidateSeries {
        candidate: [1, 1, 1, 1, 1],
        schedules: vec!["a", "b"],
        strata: vec![vec![10.0; 5], vec![10.0; 5]],
    };
    let accepted = CandidateSeries {
        candidate: [2, 2, 2, 2, 2],
        schedules: vec!["a", "b"],
        strata: vec![vec![8.0; 5], vec![9.0; 5]],
    };
    assert_eq!(
        analyze_joint_vector(&conservative, &accepted)
            .unwrap()
            .reason,
        JointVectorReason::Accepted
    );
    let regressing = CandidateSeries {
        candidate: [3, 3, 3, 3, 3],
        schedules: vec!["a", "b"],
        strata: vec![vec![11.0; 5], vec![5.0; 5]],
    };
    let decision = analyze_joint_vector(&conservative, &regressing).unwrap();
    assert_eq!(decision.selected, conservative.candidate);
    assert_eq!(decision.reason, JointVectorReason::JointValidationDefault);
    let same = analyze_joint_vector(&conservative, &conservative).unwrap();
    assert_eq!(same.reason, JointVectorReason::ConservativeVector);
}

#[test]
fn extent_statistics_cover_equal_weighting_quartiles_and_fail_closed_paths() {
    let summary = empirical_summary(&[100.0, 2.0, 4.0, 3.0, 1.0]).unwrap();
    assert_eq!(
        (summary.median, summary.iqr_low, summary.iqr_high),
        (3.0, 2.0, 4.0)
    );

    let equal_weight = [
        CandidateSeries {
            candidate: 1,
            schedules: vec!["a", "b"],
            strata: vec![vec![10.0; 5], vec![10.0; 5]],
        },
        CandidateSeries {
            candidate: 2,
            schedules: vec!["c", "d"],
            strata: vec![vec![5.0; 5], vec![20.0; 5]],
        },
    ];
    let decision = analyze_extent(&equal_weight, &1).unwrap();
    assert_eq!(decision.scores[1].summary.median, 1.0);
    assert_eq!(decision.reason, DecisionReason::UnresolvedMinimum);

    let endpoint = [
        scalar_series(1, 8.0),
        scalar_series(2, 10.0),
        scalar_series(3, 12.0),
    ];
    let decision = analyze_extent(&endpoint, &2).unwrap();
    assert_eq!(decision.selected, 1);
    assert!(decision.grid_boundary_limited);
    let default_wins = [
        scalar_series(1, 12.0),
        scalar_series(2, 8.0),
        scalar_series(3, 10.0),
    ];
    assert_eq!(
        analyze_extent(&default_wins, &2).unwrap().reason,
        DecisionReason::MeasuredDefault
    );

    let nonadjacent = [
        CandidateSeries {
            candidate: 1,
            schedules: vec!["a"],
            strata: vec![vec![10.0; 5]],
        },
        CandidateSeries {
            candidate: 2,
            schedules: vec!["b"],
            strata: vec![vec![9.0; 5]],
        },
        CandidateSeries {
            candidate: 3,
            schedules: vec!["a"],
            strata: vec![vec![8.0; 5]],
        },
    ];
    assert_eq!(
        analyze_extent(&nonadjacent, &1).unwrap().reason,
        DecisionReason::NonMonotoneSchedule
    );
    let per_stratum_reversal = [
        CandidateSeries {
            candidate: 1,
            schedules: vec!["a0", "a1"],
            strata: vec![vec![10.0; 5], vec![10.0; 5]],
        },
        CandidateSeries {
            candidate: 2,
            schedules: vec!["b0", "b1"],
            strata: vec![vec![12.0; 5], vec![5.0; 5]],
        },
        CandidateSeries {
            candidate: 3,
            schedules: vec!["c0", "c1"],
            strata: vec![vec![8.0; 5], vec![4.0; 5]],
        },
    ];
    assert_eq!(
        analyze_extent(&per_stratum_reversal, &1).unwrap().reason,
        DecisionReason::NonMonotoneCurve
    );
    let mut malformed = endpoint;
    malformed[1].strata[0][0] = f64::NAN;
    assert!(analyze_extent(&malformed, &2).is_err());
}

#[test]
fn threshold_fallbacks_cover_missing_no_win_and_reversal() {
    let missing = [ThresholdPoint {
        size: 1,
        conservative: None,
        asymptotic: None,
    }];
    assert!(matches!(
        select_threshold(&missing, 7, ThresholdDirection::LowerBound),
        ThresholdSelection::KeptDefault {
            reason: ThresholdFallback::NoComparableGridPoint,
            ..
        }
    ));
    let point = |size, asymptotic| ThresholdPoint {
        size,
        conservative: Some(threshold_stat(100.0, 0.01)),
        asymptotic: Some(threshold_stat(asymptotic, 0.01)),
    };
    assert!(matches!(
        select_threshold(&[point(1, 120.0)], 7, ThresholdDirection::LowerBound),
        ThresholdSelection::KeptDefault {
            reason: ThresholdFallback::NoGridPointWins,
            ..
        }
    ));
    assert!(matches!(
        select_threshold(
            &[point(1, 80.0), point(2, 120.0)],
            7,
            ThresholdDirection::LowerBound
        ),
        ThresholdSelection::KeptDefault {
            reason: ThresholdFallback::NonMonotone { .. },
            ..
        }
    ));
}

#[test]
fn seed_mixer_matches_the_committed_vector_and_banked_roles() {
    assert_eq!(EXTENT_SEED_ROOT, 0x5ecc_9bf8_0000_0000);
    assert_eq!(
        fixture_seed(EXTENT_SEED_ROOT, 1, 4, 0xA),
        0x35af_77d8_8d1e_27e1
    );
    assert_eq!(bank_role(0x200, 3), 0x3_0200);
    assert_eq!(
        fixture_seed(EXTENT_SEED_ROOT, 16, 0, bank_role(0x100, 7)),
        fixture_seed(EXTENT_SEED_ROOT, 16, 0, 0x7_0100)
    );
}

#[test]
fn execution_log_is_durable_append_only_and_resumes_sequence() {
    let stage = scratch("journal");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    let mut announcement = Vec::new();
    log.announce(&mut announcement).unwrap();
    assert_eq!(
        String::from_utf8(announcement).unwrap(),
        format!("GF2_CAMPAIGN_EXECUTION_LOG={}\n", log.path().display())
    );
    log.append(JournalEvent::CampaignStart, None, json!({"cells": 717}))
        .unwrap();
    log.append(
        JournalEvent::CellComplete,
        Some(json!({"field": "transpose", "candidate": 2})),
        json!({"accepted": true}),
    )
    .unwrap();
    log.terminal(TerminalState::Paused, json!({"cause": "test"}))
        .unwrap();
    drop(log);

    let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
    assert_eq!(resumed.next_sequence(), 4);
    drop(resumed);

    let text = fs::read_to_string(log_path(&stage)).unwrap();
    assert_eq!(text.lines().count(), 4);
    assert!(text
        .lines()
        .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()));
    fs::remove_dir_all(stage).unwrap();
}

fn log_path(stage: &std::path::Path) -> std::path::PathBuf {
    stage.join("execution.log")
}

fn recovery_intent_path(stage: &std::path::Path) -> Option<std::path::PathBuf> {
    fs::read_dir(stage)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("execution.log.recovery-intent-")
        })
}

#[derive(Clone, Serialize)]
struct InjectedRecoveryIntent {
    schema: String,
    recovery_id: String,
    campaign_id: String,
    prior_session_id: String,
    resume_session_id: String,
    original_log_len: u64,
    original_log_sha256: String,
    complete_prefix_len: u64,
    complete_prefix_sha256: String,
    complete_prefix_records: u64,
    suffix_sha256: String,
    diagnostic_path: String,
    diagnostic_sha256: String,
    prefix_terminal: Option<JournalEvent>,
    phase: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_test_recovery_intent(
    stage: &std::path::Path,
    path: &std::path::Path,
    intent: &mut InjectedRecoveryIntent,
    phase: &str,
) {
    intent.phase = phase.to_owned();
    fs::write(path, serde_json::to_vec(intent).unwrap()).unwrap();
    fs::File::open(path).unwrap().sync_all().unwrap();
    fs::File::open(stage).unwrap().sync_all().unwrap();
}

fn journal_records(stage: &std::path::Path) -> Vec<serde_json::Value> {
    fs::read_to_string(log_path(stage))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn newline_prefix(bytes: &[u8], count: usize) -> Vec<u8> {
    let boundary = bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'\n')
        .nth(count - 1)
        .map(|(index, _)| index + 1)
        .expect("requested complete record prefix");
    bytes[..boundary].to_vec()
}

#[test]
fn journal_recovery_records_interruption_and_preserves_torn_tail() {
    use std::io::Write;

    let stage = scratch("torn-journal");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    drop(log);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log_path(&stage))
        .unwrap();
    file.write_all(br#"{"schema":"torn""#).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let original = fs::read(log_path(&stage)).unwrap();

    let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
    assert_eq!(resumed.next_sequence(), 4);
    drop(resumed);
    let records = journal_records(&stage);
    assert_eq!(records[1]["event"], "recovery");
    assert_eq!(records[1]["session_id"], "session-1");
    assert_eq!(records[2]["event"], "interrupted");
    assert_eq!(records[2]["session_id"], "session-1");
    assert_eq!(records[3]["event"], "session-start");
    assert_eq!(records[3]["session_id"], "session-2");
    let details = &records[1]["details"];
    let prefix_len = details["complete_prefix_len"].as_u64().unwrap() as usize;
    let diagnostic = fs::read(details["diagnostic_path"].as_str().unwrap()).unwrap();
    assert_eq!(diagnostic, original[prefix_len..]);
    assert_eq!(
        details["diagnostic_sha256"],
        format!("{:x}", Sha256::digest(&diagnostic))
    );
    assert!(recovery_intent_path(&stage).is_none());
    fs::write(details["diagnostic_path"].as_str().unwrap(), b"changed").unwrap();
    assert!(ExecutionLog::resume(log_path(&stage), "campaign-1", "session-3").is_err());
    fs::remove_dir_all(stage).unwrap();
}

#[test]
fn journal_recovery_intent_is_idempotent_at_every_durable_boundary() {
    use std::io::Write;

    let stage = scratch("recovery-idempotency");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    drop(log);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log_path(&stage))
        .unwrap();
    file.write_all(br#"{"schema":"torn""#).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let original = fs::read(log_path(&stage)).unwrap();
    let prefix_len = original.iter().rposition(|byte| *byte == b'\n').unwrap() + 1;
    let prefix = &original[..prefix_len];
    let suffix = &original[prefix_len..];
    let original_sha256 = sha256(&original);
    let complete_prefix_sha256 = sha256(prefix);
    let suffix_sha256 = sha256(suffix);
    let recovery_id = sha256(
        format!("campaign-1:{original_sha256}:{complete_prefix_sha256}:{suffix_sha256}").as_bytes(),
    );
    let diagnostic_path = stage.join(format!("execution.log.torn-{recovery_id}.bin"));
    fs::write(&diagnostic_path, suffix).unwrap();
    let diagnostic_path = fs::canonicalize(diagnostic_path).unwrap();
    let intent_path = stage.join(format!("execution.log.recovery-intent-{recovery_id}.json"));
    let mut intent = InjectedRecoveryIntent {
        schema: "tuning-campaign-log-recovery-v1".into(),
        recovery_id,
        campaign_id: "campaign-1".into(),
        prior_session_id: "session-1".into(),
        resume_session_id: "session-2".into(),
        original_log_len: original.len() as u64,
        original_log_sha256: original_sha256,
        complete_prefix_len: prefix_len as u64,
        complete_prefix_sha256,
        complete_prefix_records: 1,
        suffix_sha256: suffix_sha256.clone(),
        diagnostic_path: diagnostic_path.to_string_lossy().into_owned(),
        diagnostic_sha256: suffix_sha256,
        prefix_terminal: None,
        phase: String::new(),
    };

    write_test_recovery_intent(&stage, &intent_path, &mut intent, "intent-synced");
    let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
    assert_eq!(resumed.next_sequence(), 4);
    drop(resumed);
    let complete_recovery_bytes = fs::read(log_path(&stage)).unwrap();

    let retry_states = [
        ("intent-synced", original[..prefix_len].to_vec()),
        ("log-truncated", newline_prefix(&complete_recovery_bytes, 2)),
        (
            "recovery-recorded",
            newline_prefix(&complete_recovery_bytes, 3),
        ),
        (
            "interrupted-recorded",
            newline_prefix(&complete_recovery_bytes, 4),
        ),
        ("session-started", complete_recovery_bytes.clone()),
    ];
    for (phase, log_bytes) in retry_states {
        fs::write(log_path(&stage), log_bytes).unwrap();
        write_test_recovery_intent(&stage, &intent_path, &mut intent, phase);
        let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
        assert_eq!(resumed.next_sequence(), 4);
        drop(resumed);
        let retried = journal_records(&stage);
        assert_eq!(
            retried
                .iter()
                .filter(|record| record["event"] == "recovery")
                .count(),
            1
        );
        assert_eq!(
            retried
                .iter()
                .filter(|record| record["event"] == "interrupted")
                .count(),
            1
        );
        assert_eq!(
            retried
                .iter()
                .filter(|record| record["event"] == "session-start")
                .count(),
            1
        );
        assert!(recovery_intent_path(&stage).is_none());
    }
    fs::remove_dir_all(stage).unwrap();
}

#[test]
fn journal_recovery_after_a_paused_prefix_records_recovery_without_interruption() {
    use std::io::Write;

    let stage = scratch("paused-torn-journal");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    log.terminal(TerminalState::Paused, json!({"cause": "budget"}))
        .unwrap();
    drop(log);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log_path(&stage))
        .unwrap();
    file.write_all(br#"{"partial":true"#).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
    assert_eq!(resumed.next_sequence(), 4);
    drop(resumed);
    let records = journal_records(&stage);
    assert_eq!(records[1]["event"], "paused");
    assert_eq!(records[2]["event"], "recovery");
    assert_eq!(records[3]["event"], "session-start");
    assert!(!records
        .iter()
        .any(|record| record["event"] == "interrupted"));
    assert!(
        std::path::Path::new(records[2]["details"]["diagnostic_path"].as_str().unwrap()).exists()
    );
    fs::remove_dir_all(stage).unwrap();
}

#[test]
fn journal_recovery_treats_a_complete_record_without_newline_as_torn() {
    use std::io::Write;

    let stage = scratch("missing-newline");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    let first = log
        .append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    drop(log);
    let mut torn = serde_json::to_value(first).unwrap();
    torn["sequence"] = json!(1);
    torn["event"] = json!("cell-start");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log_path(&stage))
        .unwrap();
    write!(file, "{}", serde_json::to_string(&torn).unwrap()).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let resumed = ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").unwrap();
    assert_eq!(resumed.next_sequence(), 4);
    fs::remove_dir_all(stage).unwrap();
}

#[test]
fn terminal_journal_lifecycle_fails_closed() {
    let stage = scratch("closed-journal");
    fs::create_dir(&stage).unwrap();
    let mut log = ExecutionLog::create_new(&stage, "campaign-1", "session-1").unwrap();
    assert!(log
        .append(JournalEvent::CellStart, None, json!({}))
        .is_err());
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    log.terminal(TerminalState::Complete, json!({})).unwrap();
    drop(log);
    assert!(ExecutionLog::resume(log_path(&stage), "campaign-1", "session-2").is_err());
    fs::remove_dir_all(stage).unwrap();

    let budget = scratch("budget-journal");
    fs::create_dir(&budget).unwrap();
    let mut log = ExecutionLog::create_new(&budget, "campaign-1", "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    log.terminal(TerminalState::BudgetExhausted, json!({}))
        .unwrap();
    drop(log);
    assert!(ExecutionLog::resume(log_path(&budget), "campaign-1", "session-2").is_ok());
    fs::remove_dir_all(budget).unwrap();

    let malformed = scratch("malformed-journal");
    fs::create_dir(&malformed).unwrap();
    let mut log = ExecutionLog::create_new(&malformed, "campaign-1", "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!({}))
        .unwrap();
    drop(log);
    let mut value: serde_json::Value =
        serde_json::from_str(fs::read_to_string(log_path(&malformed)).unwrap().trim_end()).unwrap();
    value["sequence"] = json!(9);
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log_path(&malformed))
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&value).unwrap()).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let before = fs::read(log_path(&malformed)).unwrap();
    assert!(ExecutionLog::resume(log_path(&malformed), "campaign-1", "session-2").is_err());
    assert_eq!(fs::read(log_path(&malformed)).unwrap(), before);
    fs::remove_dir_all(malformed).unwrap();
}

fn identity(protocol: &str) -> ResumeIdentity {
    ResumeIdentity {
        protocol_digest: if protocol == "p1" { "11" } else { "22" }.repeat(32),
        source_revision: "01".repeat(20),
        source_sha256: "bb".repeat(32),
        ordered_work_manifest_sha256: "dd".repeat(32),
        process_descriptors_sha256: "ee".repeat(32),
        executable_sha256: [("core".to_owned(), "aa".repeat(32))].into(),
        behavior_sha256: [("support".to_owned(), "cc".repeat(32))].into(),
        lifecycle_schema: "tuning-campaign-lifecycle-v1".into(),
        lifecycle_behavior_sha256: "ff".repeat(32),
        feature_contract: "parallel,simd,test-support,tuning-profile".into(),
        thread_contract: "RAYON_NUM_THREADS=4".into(),
        host_identity: "host-a".into(),
    }
}

#[test]
fn checkpoint_units_are_immutable_and_resume_validates_identity_and_bytes() {
    let root = scratch("checkpoint");
    let mut store = CheckpointStore::create_new(&root, "campaign-1", identity("p1")).unwrap();
    store
        .accept(
            "core/transpose/2/probe",
            &json!({"case": 1}),
            &json!({"ok": true}),
        )
        .unwrap();
    assert!(store
        .accept(
            "core/transpose/2/probe",
            &json!({"case": 1}),
            &json!({"ok": true})
        )
        .is_err());
    drop(store);

    let prelink_name = format!(".{}.json.tmp-1-2-3", "22".repeat(32));
    let prelink_bytes = b"partial";
    fs::write(root.join("pending").join(&prelink_name), prelink_bytes).unwrap();
    let linked = fs::read_dir(root.join("units"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let linked_name = linked.file_name().unwrap().to_string_lossy();
    let postlink_name = format!(".{linked_name}.tmp-1-2-3");
    fs::hard_link(&linked, root.join("pending").join(&postlink_name)).unwrap();
    let linked_sha256 = format!("{:x}", Sha256::digest(fs::read(&linked).unwrap()));

    let mut resumed = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert_eq!(resumed.completed_keys(), ["core/transpose/2/probe"]);
    assert!(root.join("pending/pending-recovery.json").exists());
    assert_eq!(resumed.recovered_pending().len(), 2);
    assert!(resumed.recovered_pending().iter().any(|pending| {
        pending.file_name == prelink_name
            && pending.sha256 == format!("{:x}", Sha256::digest(prelink_bytes))
            && !pending.accepted_unit_present
    }));
    assert!(resumed.recovered_pending().iter().any(|pending| {
        pending.file_name == postlink_name
            && pending.sha256 == linked_sha256
            && pending.accepted_unit_present
    }));
    assert!(resumed
        .accept("must-wait-for-recovery", &json!({}), &json!({}))
        .is_err());
    let recovery_id = resumed.pending_recovery().unwrap().recovery_id.clone();
    assert!(resumed
        .acknowledge_pending_recovery(&"00".repeat(32))
        .is_err());
    resumed.acknowledge_pending_recovery(&recovery_id).unwrap();
    assert_eq!(fs::read_dir(root.join("pending")).unwrap().count(), 0);
    assert!(CheckpointStore::resume(&root, "campaign-1", identity("different")).is_err());

    let unit = fs::read_dir(root.join("units"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&unit, b"{}").unwrap();
    assert!(resumed
        .load::<serde_json::Value, serde_json::Value>("core/transpose/2/probe")
        .is_err());
    assert!(CheckpointStore::resume(&root, "campaign-1", identity("p1")).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_manifest_and_temp_namespace_fail_closed() {
    let invalid_root = scratch("invalid-identity");
    let mut invalid_identity = identity("p1");
    invalid_identity.executable_sha256.clear();
    assert!(CheckpointStore::create_new(&invalid_root, "campaign-1", invalid_identity).is_err());
    assert!(!invalid_root.exists());

    for (label, mutate) in [
        (
            "bad-work-manifest",
            (|identity: &mut ResumeIdentity| {
                identity.ordered_work_manifest_sha256 = "AA".repeat(32)
            }) as fn(&mut ResumeIdentity),
        ),
        (
            "bad-process-descriptors",
            (|identity: &mut ResumeIdentity| identity.process_descriptors_sha256 = "1".repeat(63))
                as fn(&mut ResumeIdentity),
        ),
        (
            "bad-lifecycle-behavior",
            (|identity: &mut ResumeIdentity| identity.lifecycle_behavior_sha256 = "GG".repeat(32))
                as fn(&mut ResumeIdentity),
        ),
        (
            "bad-source",
            (|identity: &mut ResumeIdentity| identity.source_revision = "AB".repeat(20))
                as fn(&mut ResumeIdentity),
        ),
        (
            "empty-lifecycle-schema",
            (|identity: &mut ResumeIdentity| identity.lifecycle_schema.clear())
                as fn(&mut ResumeIdentity),
        ),
        (
            "empty-behavior-map",
            (|identity: &mut ResumeIdentity| identity.behavior_sha256.clear())
                as fn(&mut ResumeIdentity),
        ),
        (
            "empty-host",
            (|identity: &mut ResumeIdentity| identity.host_identity.clear())
                as fn(&mut ResumeIdentity),
        ),
    ] {
        let invalid_root = scratch(label);
        let mut invalid_identity = identity("p1");
        mutate(&mut invalid_identity);
        assert!(
            CheckpointStore::create_new(&invalid_root, "campaign-1", invalid_identity).is_err()
        );
        assert!(!invalid_root.exists());
    }

    let root = scratch("bad-temp");
    let store = CheckpointStore::create_new(&root, "campaign-1", identity("p1")).unwrap();
    drop(store);
    fs::write(root.join("pending").join("unknown"), b"partial").unwrap();
    assert!(CheckpointStore::resume(&root, "campaign-1", identity("p1")).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_identity_is_validated_before_pending_recovery() {
    let root = scratch("pending-before-identity");
    let store = CheckpointStore::create_new(&root, "campaign-1", identity("p1")).unwrap();
    drop(store);
    let pending_name = format!(".{}.json.tmp-1-2-3", "33".repeat(32));
    let pending_path = root.join("pending").join(&pending_name);
    let pending_bytes = b"recoverable-pending-unit";
    fs::write(&pending_path, pending_bytes).unwrap();

    assert!(CheckpointStore::resume(&root, "campaign-1", identity("different")).is_err());
    assert_eq!(fs::read(&pending_path).unwrap(), pending_bytes);

    let resumed = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert!(!pending_path.exists());
    assert!(root.join("pending/pending-recovery.json").exists());
    assert_eq!(
        resumed.recovered_pending(),
        [tuning_campaign_support::journal::RecoveredPending {
            file_name: pending_name,
            sha256: format!("{:x}", Sha256::digest(pending_bytes)),
            accepted_unit_present: false,
        }]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_pending_recovery_replays_every_deletion_boundary() {
    let root = scratch("pending-deletion-boundaries");
    let mut store = CheckpointStore::create_new(&root, "campaign-1", identity("p1")).unwrap();
    store
        .accept("accepted-unit", &json!({"case": 1}), &json!({"ok": true}))
        .unwrap();
    drop(store);

    let accepted = fs::read_dir(root.join("units"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let accepted_name = accepted.file_name().unwrap().to_string_lossy();
    let prelink_name = format!(".{}.json.tmp-7-8-9", "55".repeat(32));
    let postlink_name = format!(".{accepted_name}.tmp-7-8-9");
    let prelink_bytes = b"pre-link-bytes";
    let pending = root.join("pending");
    fs::write(pending.join(&prelink_name), prelink_bytes).unwrap();
    fs::hard_link(&accepted, pending.join(&postlink_name)).unwrap();
    fs::write(pending.join(".pending-recovery.intent"), b"partial-intent").unwrap();

    let first = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    let expected = first.pending_recovery().unwrap().clone();
    assert_eq!(expected.entries.len(), 2);
    assert!(!pending.join(&prelink_name).exists());
    assert!(!pending.join(&postlink_name).exists());
    assert!(!pending.join(".pending-recovery.intent").exists());
    assert!(pending.join("pending-recovery.json").exists());
    drop(first);

    // Crash before the first deletion: every manifested pending file remains.
    fs::write(pending.join(&prelink_name), prelink_bytes).unwrap();
    fs::hard_link(&accepted, pending.join(&postlink_name)).unwrap();
    let before = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert_eq!(before.pending_recovery(), Some(&expected));
    assert_eq!(before.recovered_pending().len(), 2);
    drop(before);

    // Crash within deletion: one manifested file is gone and one remains.
    fs::write(pending.join(&prelink_name), prelink_bytes).unwrap();
    let within = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert_eq!(within.pending_recovery(), Some(&expected));
    assert_eq!(within.recovered_pending().len(), 2);
    drop(within);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::write(pending.join(&prelink_name), prelink_bytes).unwrap();
        let original_permissions = fs::metadata(&pending).unwrap().permissions();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o500)).unwrap();
        let failed = CheckpointStore::resume(&root, "campaign-1", identity("p1"));
        fs::set_permissions(&pending, original_permissions).unwrap();
        assert!(failed.is_err());
        assert!(pending.join(&prelink_name).exists());
        assert!(pending.join("pending-recovery.json").exists());
        let retried = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
        assert_eq!(retried.pending_recovery(), Some(&expected));
        drop(retried);
    }

    // Crash after directory sync: only the durable manifest remains.
    let mut after = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert_eq!(after.pending_recovery(), Some(&expected));
    assert_eq!(after.recovered_pending().len(), 2);
    assert!(after
        .acknowledge_pending_recovery(&"ff".repeat(32))
        .is_err());
    assert_eq!(after.pending_recovery(), Some(&expected));
    // Crash after acknowledgement unlink but before its directory sync.
    fs::remove_file(pending.join("pending-recovery.json")).unwrap();
    after
        .acknowledge_pending_recovery(&expected.recovery_id)
        .unwrap();
    assert!(after.pending_recovery().is_none());
    drop(after);

    let clean = CheckpointStore::resume(&root, "campaign-1", identity("p1")).unwrap();
    assert!(clean.pending_recovery().is_none());
    assert!(clean.recovered_pending().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_pending_namespace_requires_the_exact_generated_shape() {
    for (index, name) in [
        format!(".{}.json.tmp-1-2", "44".repeat(32)),
        format!(".{}.json.tmp-1-2-3-extra", "44".repeat(32)),
        format!(".{}.json.tmp-1-x-3", "44".repeat(32)),
        format!(".{}.json.tmp-1-2-3", "AA".repeat(32)),
        format!("{}.json.tmp-1-2-3", "44".repeat(32)),
    ]
    .into_iter()
    .enumerate()
    {
        let root = scratch(&format!("pending-name-{index}"));
        let store = CheckpointStore::create_new(&root, "campaign-1", identity("p1")).unwrap();
        drop(store);
        let pending = root.join("pending").join(name);
        fs::write(&pending, b"partial").unwrap();
        assert!(CheckpointStore::resume(&root, "campaign-1", identity("p1")).is_err());
        assert_eq!(fs::read(pending).unwrap(), b"partial");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn atomic_helpers_distinguish_immutable_evidence_from_mutable_pointers() {
    let root = scratch("atomic");
    fs::create_dir(&root).unwrap();
    let path = root.join("record");
    atomic_write_new(&path, b"first").unwrap();
    assert!(atomic_write_new(&path, b"second").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"first");
    atomic_replace(&path, b"second").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    fs::remove_dir_all(root).unwrap();
}
