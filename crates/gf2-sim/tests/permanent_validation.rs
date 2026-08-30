use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gf2_sim::permanent_campaign::schema::{ArtifactIdentity, Backend};
use gf2_sim::permanent_campaign::validation::{
    publish_validation_receipt_atomic, read_validation_receipt, run_validation, AnchorSpec,
    BackendAgreementStatus, DecisionRule, ReplayMode, RetryRule, ValidationAuthorities,
    ValidationPreregistration, ValidationProtocol, ValidationStreamPurpose, ValidationVerdict,
    RECEIPT_SCHEMA_VERSION,
};
use gf2_stats::binomial::two_sided_test;

fn identity(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.parse().expect("fixture path is normalized"),
        sha256: byte
            .to_string()
            .repeat(64)
            .parse()
            .expect("fixture digest is lowercase hexadecimal"),
    }
}

fn preregistration(expected_zero_count: u64) -> ValidationPreregistration {
    ValidationPreregistration {
        schema_version: 1,
        protocol: ValidationProtocol {
            root_seed: 0x4453_4B2F_0000_0001,
            stream_purpose: ValidationStreamPurpose::Validation,
            replay_matrix_count: 1_024,
            sample_matrix_count: 64,
            exact_test_level: 0.001,
            decision_rule: DecisionRule::ProbabilityOrderingExactTwoSidedStrictGreater,
            retry_rule: RetryRule::NoRedraw,
            backend_batch_matrix_count: 16,
            sample_backend: Backend::BatchParallel,
            selectable_backends: Backend::campaign_inventory()
                .iter()
                .copied()
                .filter(|backend| !backend.is_accelerator())
                .collect(),
        },
        anchors: vec![AnchorSpec {
            q: 3,
            n: 2,
            stream_index: 0,
            expected_matrix_count: 81,
            expected_permanent_zero_count: expected_zero_count,
            expected_determinant_zero_count: 33,
        }],
        authorities: ValidationAuthorities {
            protocol: identity("protocol.md", 'a'),
            manifest: identity("manifest.json", 'b'),
            exact_anchors: identity("exact-anchors.csv", 'c'),
            backend_equivalence: identity("backend-equivalence.csv", 'd'),
        },
    }
}

fn unique_directory(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    PathBuf::from(format!("target/{label}-{unique}"))
}

#[test]
fn replay_exact_decisions_and_production_paths_are_behavioral() {
    let state = unique_directory("permanent-validation-pass");
    let receipt = run_validation(
        &preregistration(33),
        identity("pre-draw-validation-v1-preregistration.json", 'e'),
        2,
        &state,
    )
    .expect("the focused anchor is valid");

    assert_eq!(receipt.schema_version, RECEIPT_SCHEMA_VERSION);
    assert!(receipt.passed());
    let anchor = &receipt.anchors[0];
    assert_eq!(anchor.address.purpose_tag, 1);

    let exact = anchor.exact.as_ref().expect("exact phase completed");
    assert_eq!(exact.enumerated_matrix_count, 81);
    assert_eq!(exact.oracle_permanent_zero_count, 33);
    assert_eq!(exact.production_determinant_zero_count, 33);
    assert!(exact.backend_agreements.iter().all(|agreement| {
        agreement.status == BackendAgreementStatus::Identical
            && agreement.matrices_compared == 81
            && agreement.mismatch_count == 0
    }));

    let replay = anchor.replay.as_ref().expect("replay phase completed");
    assert_eq!(replay.mode, ReplayMode::TwoFreshSerial);
    assert_eq!(replay.matrix_count, 1_024);
    assert!(replay.identical);

    let sample = anchor.sample.as_ref().expect("sampler phase completed");
    assert_eq!(sample.matrix_count, 64);
    assert_eq!(sample.stream_purpose, ValidationStreamPurpose::Validation);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn equality_fails_and_terminal_failures_are_adopted_without_redraw() {
    let equality = two_sided_test(1, 1, 0.001);
    assert_eq!(equality.log_p_value().exp(), 0.001);
    assert!(equality.rejects_at(0.001));

    let state = unique_directory("permanent-validation-fail");
    let preregistration = preregistration(32);
    let preregistration_identity = identity("pre-draw-validation-v1-preregistration.json", 'e');
    let first = run_validation(
        &preregistration,
        preregistration_identity.clone(),
        1,
        &state,
    )
    .expect("an exact mismatch is preserved as terminal evidence");
    assert!(!first.passed());
    assert_eq!(first.anchors[0].verdict, ValidationVerdict::Failed);

    let adopted = run_validation(
        &preregistration,
        preregistration_identity,
        1,
        &state,
    )
    .expect("the terminal failure is adopted without reopening its address");
    assert_eq!(adopted, first);

    let output = state.join("receipt.json");
    publish_validation_receipt_atomic(&output, &first)
        .expect("the immutable receipt publishes atomically");
    let decoded = read_validation_receipt(&output).expect("receipt round trip validates");
    assert_eq!(decoded, first);
    publish_validation_receipt_atomic(&output, &first)
        .expect("an identical final receipt is adopted rather than overwritten");

    fs::remove_dir_all(state).unwrap();
}
