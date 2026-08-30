//! Behavioral coverage for the frozen pre-draw campaign validation phase.
//!
//! These tests exercise the reusable library computation, the durable
//! no-redraw journal, and the immutable receipt contract on focused anchors.
//! The committed ten-anchor plan is loaded and content-verified here, but its
//! 400,000-draw evidence run belongs to the execution lead, not to this tier.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf2_algebra::permanent::{
    determinant_singular_probability, enumerate_permanent_zero_probability, permanent_ryser,
};
use gf2_core::gfp::Fp;
use gf2_sim::permanent_campaign::provenance::repository_top_level;
use gf2_sim::permanent_campaign::schedule::backend_supports_cell;
use gf2_sim::permanent_campaign::schema::{read_manifest, ArtifactIdentity, Backend, Sha256Digest};
use gf2_sim::permanent_campaign::validation::{
    evaluate_validation_anchor, is_frozen_validation_toolchain,
    load_frozen_campaign_validation_preregistration, load_validation_preregistration,
    publish_validation_receipt_atomic, read_validation_receipt, run_validation, AnchorSpec,
    BackendAgreementStatus, DecisionRule, FrozenArtifactGuard, FrozenArtifactSnapshot, PhaseStatus,
    ReplayMode, RetryRule, SampleOrigin, ValidationAuthorities, ValidationFailure, ValidationPhase,
    ValidationPreregistration, ValidationProtocol, ValidationReceipt, ValidationStreamPurpose,
    ValidationVerdict, FROZEN_TOOLCHAIN_PREFIX, PREREGISTRATION_SCHEMA_VERSION,
    RECEIPT_SCHEMA_VERSION,
};
use gf2_stats::binomial::two_sided_test;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};

/// Committed frozen plan for issue `02b8137c`, verified by this suite before
/// the execution lead consumes it.
const FROZEN_PREREGISTRATION: &str =
    "dev/active/02b8137c/pre-draw-validation-v1-preregistration.json";
/// Established validation namespace recorded in `exact-anchors.csv`.
const VALIDATION_ROOT: u64 = 0x4453_4B2F_0000_0001;
/// Focused draw count: large enough to exercise the exact test, small enough
/// for the fast tier. The protocol's own count is fixed at 400,000.
const FOCUSED_DRAWS: u64 = 4_096;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One labelled single-field mutation of an otherwise valid artifact.
type Mutation<T> = (&'static str, Box<dyn Fn(&mut T)>);

fn unique_directory(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the test host clock follows the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "gf2-{label}-{}-{unique}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn repository() -> PathBuf {
    repository_top_level(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("the test runs inside the repository")
}

fn digest(byte: char) -> Sha256Digest {
    byte.to_string()
        .repeat(64)
        .parse()
        .expect("a repeated hexadecimal digit is a canonical digest")
}

fn identity(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.parse().expect("fixture path is normalized"),
        sha256: digest(byte),
    }
}

fn focused_authorities() -> ValidationAuthorities {
    ValidationAuthorities {
        protocol: identity("fixtures/protocol.md", 'a'),
        manifest: identity("fixtures/manifest.json", 'b'),
        exact_anchors: identity("fixtures/exact-anchors.csv", 'c'),
        backend_equivalence: identity("fixtures/backend-equivalence.csv", 'd'),
    }
}

fn focused_anchor(q: u8, n: u16, expected_permanent_zero_count: u64) -> AnchorSpec {
    let exact = enumerate_permanent_zero_probability(u64::from(q), usize::from(n));
    AnchorSpec {
        q,
        n,
        stream_index: 0,
        expected_matrix_count: exact
            .matrix_count()
            .to_string()
            .parse()
            .expect("a focused anchor universe fits u64"),
        expected_permanent_zero_count,
        expected_determinant_zero_count: determinant_singular_probability(
            u64::from(q),
            usize::from(n),
        )
        .zero_count()
        .to_string()
        .parse()
        .expect("a focused singular count fits u64"),
    }
}

fn focused_protocol() -> ValidationProtocol {
    ValidationProtocol {
        root_seed: VALIDATION_ROOT,
        stream_purpose: ValidationStreamPurpose::Validation,
        replay_matrix_count: 1_024,
        sample_matrix_count: FOCUSED_DRAWS,
        exact_test_level: 0.001,
        decision_rule: DecisionRule::ProbabilityOrderingExactTwoSidedStrictGreater,
        retry_rule: RetryRule::NoRedraw,
        backend_batch_matrix_count: 64,
        sample_backend: Backend::BatchParallel,
        // The accelerator is excluded from the focused inventory because this
        // tier builds without the optional HIP feature. Under the frozen plan
        // an unavailable required backend fails validation; it is never
        // silently skipped.
        selectable_backends: Backend::campaign_inventory()
            .iter()
            .copied()
            .filter(|backend| !backend.is_accelerator())
            .collect(),
    }
}

/// Two cheap anchors, so anchor ordering and coverage are observable.
fn focused_plan() -> ValidationPreregistration {
    ValidationPreregistration {
        schema_version: PREREGISTRATION_SCHEMA_VERSION,
        protocol: focused_protocol(),
        anchors: vec![focused_anchor(3, 2, 33), focused_anchor(5, 2, 145)],
        authorities: focused_authorities(),
    }
}

fn focused_identity() -> ArtifactIdentity {
    identity("fixtures/preregistration.json", 'e')
}

fn passing_receipt(label: &str) -> (ValidationReceipt, PathBuf) {
    let state = unique_directory(label);
    let receipt = run_validation(&focused_plan(), focused_identity(), 2, &state)
        .expect("the focused anchors execute");
    assert!(receipt.passed(), "focused anchors pass: {receipt:?}");
    (receipt, state)
}

/// Regenerates a validation-purpose stream independently of the campaign
/// scheduler and counts zero permanents with the generic Ryser reference.
fn independent_zero_count<const Q: u64>(
    field_order: FieldOrder,
    n: usize,
    purpose: StreamPurpose,
    draws: u64,
) -> u64 {
    let address = MatrixAddress::new(
        VALIDATION_ROOT,
        field_order,
        n,
        purpose,
        StreamIndex::new(0).expect("stream zero is representable"),
    );
    let mut sampler = MatrixSampler::<Q>::new(address).expect("the focused field is supported");
    let mut entries = vec![Fp::<Q>::new(0); n * n];
    let mut zero_count = 0;
    for _ in 0..draws {
        sampler.fill_next_matrix(&mut entries);
        zero_count += u64::from(permanent_ryser(&entries, n).value() == 0);
    }
    zero_count
}

#[test]
fn frozen_preregistration_binds_the_committed_protocol_and_manifest() {
    let repository = repository();
    let (plan, identity) = load_frozen_campaign_validation_preregistration(
        &repository,
        Path::new(FROZEN_PREREGISTRATION),
    )
    .expect("the committed frozen plan is valid and content-bound");

    assert_eq!(identity.path.as_str(), FROZEN_PREREGISTRATION);
    assert_eq!(plan.protocol.root_seed, VALIDATION_ROOT);
    assert_eq!(
        plan.protocol.stream_purpose,
        ValidationStreamPurpose::Validation
    );
    assert_eq!(plan.protocol.replay_matrix_count, 1_024);
    assert_eq!(plan.protocol.sample_matrix_count, 400_000);
    assert_eq!(plan.protocol.exact_test_level, 0.001);
    assert_eq!(
        plan.protocol.decision_rule,
        DecisionRule::ProbabilityOrderingExactTwoSidedStrictGreater
    );
    assert_eq!(plan.protocol.retry_rule, RetryRule::NoRedraw);

    let ordered: Vec<(u8, u16)> = plan
        .anchors
        .iter()
        .map(|anchor| (anchor.q, anchor.n))
        .collect();
    assert_eq!(
        ordered,
        vec![
            (3, 1),
            (3, 2),
            (3, 3),
            (3, 4),
            (5, 1),
            (5, 2),
            (5, 3),
            (7, 1),
            (7, 2),
            (7, 3)
        ],
        "the frozen plan fixes the protocol's ten anchors in address order"
    );

    // The selectable inventory is the frozen manifest's backend union, in
    // schema order, rather than a hand-listed subset.
    let manifest =
        read_manifest(&repository.join(
            "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829",
        ))
        .expect("the frozen manifest reads");
    let union: Vec<Backend> = Backend::campaign_inventory()
        .iter()
        .copied()
        .filter(|backend| manifest.cells.iter().any(|cell| cell.backend == *backend))
        .collect();
    assert_eq!(plan.protocol.selectable_backends, union);
    assert!(union.contains(&Backend::Accelerator));
}

#[test]
fn frozen_preregistration_anchor_counts_match_the_committed_exact_evidence() {
    let repository = repository();
    let (plan, _) = load_frozen_campaign_validation_preregistration(
        &repository,
        Path::new(FROZEN_PREREGISTRATION),
    )
    .expect("the committed frozen plan is valid");
    let evidence =
        fs::read_to_string(repository.join("dev/benchmarks/permanent_campaign/exact-anchors.csv"))
            .expect("the committed exact-anchor evidence reads");

    for anchor in &plan.anchors {
        let prefix = format!(
            "{},{},{},{},",
            anchor.q, anchor.n, anchor.expected_permanent_zero_count, anchor.expected_matrix_count
        );
        assert!(
            evidence.lines().any(|line| line.starts_with(&prefix)),
            "anchor q={} n={} disagrees with the committed exact-anchor row",
            anchor.q,
            anchor.n
        );
        assert_eq!(
            anchor.expected_determinant_zero_count,
            determinant_singular_probability(u64::from(anchor.q), usize::from(anchor.n))
                .zero_count()
                .to_string()
                .parse::<u64>()
                .expect("an anchor singular count fits u64"),
            "anchor q={} n={} disagrees with the finite-n singular formula",
            anchor.q,
            anchor.n
        );
    }
}

#[test]
fn every_required_backend_and_the_determinant_path_reproduce_the_oracle() {
    let (receipt, state) = passing_receipt("validation-pass");
    assert_eq!(receipt.schema_version, RECEIPT_SCHEMA_VERSION);
    assert_eq!(receipt.anchors.len(), 2);

    for (spec, anchor) in focused_plan().anchors.iter().zip(&receipt.anchors) {
        assert_eq!(anchor.address.purpose_tag, StreamPurpose::Validation.tag());
        assert_eq!(anchor.exact_oracle_status, PhaseStatus::Passed);
        assert_eq!(anchor.backend_status, PhaseStatus::Passed);
        assert_eq!(anchor.determinant_status, PhaseStatus::Passed);

        let exact = anchor
            .exact
            .as_ref()
            .expect("the exhaustive phase completed");
        assert_eq!(exact.enumerated_matrix_count, spec.expected_matrix_count);
        assert_eq!(
            exact.oracle_permanent_zero_count,
            spec.expected_permanent_zero_count
        );
        assert_eq!(
            exact.production_determinant_zero_count,
            spec.expected_determinant_zero_count
        );
        assert_eq!(
            exact.backend_agreements.len(),
            focused_protocol().selectable_backends.len()
        );
        let mut compared = 0;
        for agreement in &exact.backend_agreements {
            // A backend is recorded as unsupported only when the one canonical
            // domain rule excludes the cell. Everything else must have been
            // compared per matrix; an unavailable required backend is a
            // failure, never a silent omission.
            if !backend_supports_cell(agreement.backend, spec.q, spec.n) {
                assert_eq!(agreement.status, BackendAgreementStatus::Unsupported);
                assert_eq!(agreement.matrices_compared, 0);
                assert_eq!(agreement.production_permanent_zero_count, None);
                continue;
            }
            compared += 1;
            assert_eq!(
                agreement.status,
                BackendAgreementStatus::Identical,
                "backend {} must agree per matrix with the oracle",
                agreement.backend.name()
            );
            assert_eq!(agreement.matrices_compared, exact.enumerated_matrix_count);
            assert_eq!(agreement.mismatch_count, 0);
            assert_eq!(
                agreement.production_permanent_zero_count,
                Some(exact.oracle_permanent_zero_count),
                "the pooling path must reproduce the oracle zero count"
            );
        }
        assert!(compared > 0, "every anchor compares at least one backend");
    }
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn replay_uses_two_fresh_instances_over_the_first_preregistered_matrices() {
    let (receipt, state) = passing_receipt("validation-replay");
    for anchor in &receipt.anchors {
        let replay = anchor.replay.as_ref().expect("the replay phase completed");
        // `MatrixSampler` exposes no worker-count choice, so the protocol's
        // second fresh serial pass applies and the receipt records it.
        assert_eq!(replay.mode, ReplayMode::TwoFreshSerial);
        assert_eq!(replay.matrix_count, 1_024);
        assert_eq!(replay.mismatch_count, 0);
        assert!(replay.identical);
        assert_eq!(replay.first_sha256, replay.second_sha256);
        assert_eq!(anchor.replay_status, PhaseStatus::Passed);
    }
    assert_ne!(
        receipt.anchors[0].replay.as_ref().unwrap().first_sha256,
        receipt.anchors[1].replay.as_ref().unwrap().first_sha256,
        "distinct anchors address distinct streams"
    );
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn sampled_draws_come_from_the_validation_stream_and_never_the_campaign_stream() {
    let (receipt, state) = passing_receipt("validation-purpose");
    let sample = receipt.anchors[0]
        .sample
        .as_ref()
        .expect("the sampler phase completed");
    assert_eq!(sample.stream_purpose, ValidationStreamPurpose::Validation);
    assert_eq!(sample.origin, SampleOrigin::FreshAddressStart);
    assert_eq!(sample.matrix_count, FOCUSED_DRAWS);

    let from_validation =
        independent_zero_count::<3>(FieldOrder::F3, 2, StreamPurpose::Validation, FOCUSED_DRAWS);
    assert_eq!(
        sample.permanent_zero_count, from_validation,
        "the recorded count must be reproducible from the validation-purpose address"
    );
    let from_campaign = independent_zero_count::<3>(
        FieldOrder::F3,
        2,
        StreamPurpose::CampaignCell,
        FOCUSED_DRAWS,
    );
    assert_ne!(
        from_validation, from_campaign,
        "validation and campaign-cell purposes must address disjoint streams"
    );
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn the_sampled_null_derives_from_the_runtime_enumeration() {
    let protocol = focused_protocol();
    let spec = focused_anchor(3, 2, 33);
    let outcome = evaluate_validation_anchor::<3>(&protocol, &spec, 2)
        .expect("the field-generic computation is public");
    let exact = outcome
        .exact
        .as_ref()
        .expect("the exhaustive phase completed");
    let sample = outcome
        .sample
        .as_ref()
        .expect("the sampler phase completed");

    assert_eq!(
        sample.null_numerator,
        exact.oracle_permanent_zero_count.to_string()
    );
    assert_eq!(
        sample.null_denominator,
        exact.enumerated_matrix_count.to_string()
    );
    let independent = enumerate_permanent_zero_probability(3, 2);
    assert_eq!(sample.null_numerator, independent.zero_count().to_string());
    assert_eq!(
        sample.null_denominator,
        independent.matrix_count().to_string()
    );

    let expected = two_sided_test(
        sample.permanent_zero_count,
        sample.matrix_count,
        exact.oracle_permanent_zero_count as f64 / exact.enumerated_matrix_count as f64,
    );
    assert_eq!(
        sample.log_p_value.to_bits(),
        expected.log_p_value().to_bits()
    );
    assert_eq!(
        sample.verdict,
        if expected.rejects_at(protocol.exact_test_level) {
            ValidationVerdict::Failed
        } else {
            ValidationVerdict::Passed
        }
    );
}

#[test]
fn the_field_generic_computation_covers_every_supported_prime_field() {
    let protocol = focused_protocol();
    for (q, n, zeros) in [(3_u8, 2_u16, 33_u64), (5, 2, 145), (7, 2, 385)] {
        let spec = focused_anchor(q, n, zeros);
        let outcome = match q {
            3 => evaluate_validation_anchor::<3>(&protocol, &spec, 1),
            5 => evaluate_validation_anchor::<5>(&protocol, &spec, 1),
            _ => evaluate_validation_anchor::<7>(&protocol, &spec, 1),
        }
        .expect("each supported field evaluates");
        assert_eq!(outcome.verdict, ValidationVerdict::Passed, "q={q} n={n}");
    }
    // The const parameter must name the anchor's own field.
    assert!(evaluate_validation_anchor::<5>(&protocol, &focused_anchor(3, 2, 33), 1).is_err());
    assert!(evaluate_validation_anchor::<3>(&protocol, &focused_anchor(3, 2, 33), 0).is_err());
}

#[test]
fn the_exact_decision_is_strict_so_equality_at_the_level_fails() {
    // The single attainable outcome has exact mass p0, so this p-value is the
    // level itself.
    let equality = two_sided_test(1, 1, 0.001);
    assert!(
        equality.rejects_at(0.001),
        "a p-value equal to the level rejects, so an anchor at equality fails"
    );
    assert!(
        !equality.rejects_at(0.000_9),
        "the same value passes strictly above the level"
    );
    assert!(
        !two_sided_test(1, 1, 0.002).rejects_at(0.001),
        "a p-value above the level passes"
    );
}

#[test]
fn an_exact_component_failure_is_preserved_and_stops_before_replay() {
    let state = unique_directory("validation-exact-failure");
    let mut plan = focused_plan();
    plan.anchors[0].expected_permanent_zero_count = 32;
    let receipt = run_validation(&plan, focused_identity(), 1, &state)
        .expect("an exact mismatch is preserved as terminal evidence");

    assert!(!receipt.passed());
    let anchor = &receipt.anchors[0];
    assert_eq!(anchor.verdict, ValidationVerdict::Failed);
    assert_eq!(anchor.exact_oracle_status, PhaseStatus::Failed);
    assert_eq!(anchor.replay_status, PhaseStatus::Unexecuted);
    assert_eq!(anchor.statistical_status, PhaseStatus::Unexecuted);
    assert!(anchor.replay.is_none());
    assert!(anchor.sample.is_none(), "a failed anchor draws no sample");
    assert!(
        anchor.exact.is_some(),
        "the contradicting evidence is preserved with the failure"
    );
    // Every anchor still reaches a terminal record, so nothing is lost.
    assert_eq!(receipt.anchors.len(), plan.anchors.len());
    assert_eq!(receipt.anchors[1].verdict, ValidationVerdict::Passed);

    let adopted = run_validation(&plan, focused_identity(), 1, &state)
        .expect("terminal evidence is adopted rather than recomputed");
    assert_eq!(adopted, receipt, "a completed address is never reopened");
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn a_lost_terminal_record_preserves_an_interruption_without_a_redraw() {
    let state = unique_directory("validation-interrupted");
    let plan = focused_plan();
    let first =
        run_validation(&plan, focused_identity(), 1, &state).expect("the first run completes");
    assert!(first.passed());

    // Simulate a crash between the last durable phase marker and the terminal
    // record: the start markers survive, the terminal record does not.
    let terminal = state.join("q3-n02-s0.terminal.json");
    assert!(
        terminal.is_file(),
        "the journal published a terminal record"
    );
    fs::remove_file(&terminal).expect("the terminal record is removable");

    let second = run_validation(&plan, focused_identity(), 1, &state)
        .expect("the interruption is preserved");
    assert!(!second.passed());
    let anchor = &second.anchors[0];
    assert_eq!(anchor.verdict, ValidationVerdict::Failed);
    assert!(
        anchor.sample.is_none(),
        "an interrupted address is not redrawn"
    );
    assert_eq!(
        anchor.failure,
        Some(ValidationFailure::InterruptedAfterStart {
            phase: ValidationPhase::Sample
        }),
        "the receipt names the latest phase that had started"
    );
    assert_eq!(anchor.statistical_status, PhaseStatus::MechanicalFailure);
    assert_eq!(anchor.started_at, first.anchors[0].started_at);

    // The preserved interruption is itself terminal and is adopted unchanged.
    let third = run_validation(&plan, focused_identity(), 1, &state)
        .expect("the interruption record is terminal");
    assert_eq!(third, second);
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn stale_temporary_files_neither_block_publication_nor_are_adopted() {
    let state = unique_directory("validation-stale-temporary");
    fs::create_dir_all(&state).expect("the journal directory is creatable");
    // A crash between creating a temporary and linking it leaves this behind.
    for stale in [
        "run-state.tmp-1-0-0-0",
        "q3-n02-s0.terminal.tmp-1-0-0-0",
        "q3-n02-s0.exact.started.tmp-1-0-0-0",
    ] {
        fs::write(state.join(stale), b"{\"corrupt\": true}\n").expect("the stale file is writable");
    }

    let receipt = run_validation(&focused_plan(), focused_identity(), 1, &state)
        .expect("stale temporaries do not block a fresh run");
    assert!(receipt.passed());
    assert!(
        state.join("q3-n02-s0.terminal.json").is_file(),
        "the terminal record publishes past the stale temporary"
    );
    assert!(
        state.join("run-state.tmp-1-0-0-0").is_file(),
        "an unadopted temporary is left untouched rather than read as state"
    );
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn an_incompatible_journal_refuses_to_adopt_a_different_plan() {
    let state = unique_directory("validation-incompatible-journal");
    let receipt = run_validation(&focused_plan(), focused_identity(), 1, &state)
        .expect("the first run completes");
    assert!(receipt.passed());

    let mut other = focused_plan();
    other.protocol.sample_matrix_count = FOCUSED_DRAWS + 1;
    let error = run_validation(&other, focused_identity(), 1, &state)
        .expect_err("a different plan cannot adopt this journal");
    assert!(error.to_string().contains("incompatible"), "{error}");
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn the_receipt_round_trips_and_republishes_only_identical_evidence() {
    let (receipt, state) = passing_receipt("validation-round-trip");
    let output = state.join("receipt.json");
    publish_validation_receipt_atomic(&output, &receipt).expect("the receipt publishes atomically");
    let decoded = read_validation_receipt(&output).expect("the receipt round trips and validates");
    assert_eq!(decoded, receipt);
    publish_validation_receipt_atomic(&output, &receipt)
        .expect("an identical receipt is adopted rather than overwritten");

    let mut altered = receipt.clone();
    altered.anchors[0].finished_at.nanoseconds ^= 1;
    let error = publish_validation_receipt_atomic(&output, &altered)
        .expect_err("an immutable receipt is never overwritten");
    assert!(error.to_string().contains("incompatible"), "{error}");
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn receipt_validation_rejects_each_mutated_field() {
    let (base, state) = passing_receipt("validation-mutations");
    base.validate().expect("the unmutated receipt validates");

    let snapshot = |byte: char| {
        FrozenArtifactSnapshot {
        root: "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829"
            .parse()
            .expect("the frozen root is a normalized path"),
        artifacts: vec![identity(
            "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json",
            byte,
        )],
    }
    };

    let mutations: Vec<Mutation<ValidationReceipt>> = vec![
        (
            "receipt schema",
            Box::new(|r: &mut ValidationReceipt| r.schema_version += 1),
        ),
        (
            "preregistration schema",
            Box::new(|r: &mut ValidationReceipt| r.preregistration.schema_version += 1),
        ),
        (
            "anchor coverage",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors.pop();
            }),
        ),
        (
            "anchor order",
            Box::new(|r: &mut ValidationReceipt| r.anchors.swap(0, 1)),
        ),
        (
            "anchor stream purpose",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].address.purpose_tag = StreamPurpose::CampaignCell.tag();
            }),
        ),
        (
            "anchor stream index",
            Box::new(|r: &mut ValidationReceipt| r.anchors[0].address.stream_index += 1),
        ),
        (
            "enumerated matrix count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].exact.as_mut().unwrap().enumerated_matrix_count += 1;
            }),
        ),
        (
            "oracle zero count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0]
                    .exact
                    .as_mut()
                    .unwrap()
                    .oracle_permanent_zero_count += 1;
            }),
        ),
        (
            "determinant zero count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0]
                    .exact
                    .as_mut()
                    .unwrap()
                    .production_determinant_zero_count += 1;
            }),
        ),
        (
            "backend coverage",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0]
                    .exact
                    .as_mut()
                    .unwrap()
                    .backend_agreements
                    .pop();
            }),
        ),
        (
            "backend reported unsupported",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].exact.as_mut().unwrap().backend_agreements[0].status =
                    BackendAgreementStatus::Unsupported;
            }),
        ),
        (
            "backend reported unavailable",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].exact.as_mut().unwrap().backend_agreements[0].status =
                    BackendAgreementStatus::Unavailable;
            }),
        ),
        (
            "backend reported mismatched",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].exact.as_mut().unwrap().backend_agreements[0].status =
                    BackendAgreementStatus::Mismatch;
            }),
        ),
        (
            "backend inventory order",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0]
                    .exact
                    .as_mut()
                    .unwrap()
                    .backend_agreements
                    .swap(0, 1);
            }),
        ),
        (
            "backend zero count",
            Box::new(|r: &mut ValidationReceipt| {
                let agreement = &mut r.anchors[0].exact.as_mut().unwrap().backend_agreements[0];
                agreement.production_permanent_zero_count = agreement
                    .production_permanent_zero_count
                    .map(|count| count + 1);
            }),
        ),
        (
            "replay determinism",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].replay.as_mut().unwrap().identical = false;
            }),
        ),
        (
            "replay matrix count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].replay.as_mut().unwrap().matrix_count += 1;
            }),
        ),
        (
            "replay digest",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].replay.as_mut().unwrap().second_sha256 = digest('f');
            }),
        ),
        (
            "sampled zero count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].sample.as_mut().unwrap().permanent_zero_count += 1;
            }),
        ),
        (
            "sampled matrix count",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].sample.as_mut().unwrap().matrix_count += 1;
            }),
        ),
        (
            "null authority",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].sample.as_mut().unwrap().null_numerator = "1".to_owned();
            }),
        ),
        (
            "exact test value",
            Box::new(|r: &mut ValidationReceipt| {
                let sample = r.anchors[0].sample.as_mut().unwrap();
                sample.log_p_value = 0.0;
                sample.p_value = 1.0;
            }),
        ),
        (
            "anchor verdict",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].sample.as_mut().unwrap().verdict = ValidationVerdict::Failed;
            }),
        ),
        (
            "statistical status",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchors[0].statistical_status = PhaseStatus::Failed;
            }),
        ),
        (
            "overall status",
            Box::new(|r: &mut ValidationReceipt| r.overall_verdict = ValidationVerdict::Failed),
        ),
        (
            "worker count",
            Box::new(|r: &mut ValidationReceipt| r.runtime.worker_count = 0),
        ),
        (
            "binary identity",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.binary_sha256 = None),
        ),
        (
            "source closure identity",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.deps_source_revision = None),
        ),
        (
            "source closure state",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.deps_source_dirty = None),
        ),
        (
            "toolchain identity",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.compiler_version.clear()),
        ),
        (
            "rng identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.runtime.provenance.rng_version = "rand_chacha 0.0.0".to_owned();
            }),
        ),
        (
            "hardware identity",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.cpu_model.clear()),
        ),
        (
            "invocation",
            Box::new(|r: &mut ValidationReceipt| r.runtime.provenance.invocation.clear()),
        ),
        (
            "reversed run timestamps",
            Box::new(|r: &mut ValidationReceipt| r.started_at.seconds = r.finished_at.seconds + 1),
        ),
        (
            "anchor start before the run",
            Box::new(|r: &mut ValidationReceipt| r.anchors[0].started_at.seconds -= 1),
        ),
        (
            "anchor finish after the run",
            Box::new(|r: &mut ValidationReceipt| r.anchors[0].finished_at.seconds += 1),
        ),
        (
            "run finish later than every anchor",
            Box::new(|r: &mut ValidationReceipt| r.finished_at.seconds += 1),
        ),
        (
            "frozen guard verdict",
            Box::new(move |r: &mut ValidationReceipt| {
                r.frozen_artifacts = Some(FrozenArtifactGuard {
                    before: snapshot('a'),
                    after: snapshot('b'),
                    status: PhaseStatus::Passed,
                });
            }),
        ),
        (
            "frozen guard propagation",
            Box::new(move |r: &mut ValidationReceipt| {
                r.frozen_artifacts = Some(FrozenArtifactGuard {
                    before: snapshot('a'),
                    after: snapshot('b'),
                    status: PhaseStatus::Failed,
                });
            }),
        ),
    ];

    for (label, mutate) in mutations {
        let mut receipt = base.clone();
        mutate(&mut receipt);
        assert_ne!(receipt, base, "mutation `{label}` changed nothing");
        assert!(
            receipt.validate().is_err(),
            "receipt validation must reject a mutated {label}"
        );
    }
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn preregistration_validation_rejects_malformed_plans() {
    focused_plan()
        .validate()
        .expect("the focused plan is well formed");

    let cases: Vec<Mutation<ValidationPreregistration>> = vec![
        (
            "schema version",
            Box::new(|p: &mut ValidationPreregistration| p.schema_version += 1),
        ),
        (
            "zero replay count",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.replay_matrix_count = 0),
        ),
        (
            "zero sample count",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.sample_matrix_count = 0),
        ),
        (
            "zero batch size",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.backend_batch_matrix_count = 0),
        ),
        (
            "level above one",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.exact_test_level = 1.5),
        ),
        (
            "level at zero",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.exact_test_level = 0.0),
        ),
        (
            "empty backend inventory",
            Box::new(|p: &mut ValidationPreregistration| p.protocol.selectable_backends.clear()),
        ),
        (
            "duplicated backend",
            Box::new(|p: &mut ValidationPreregistration| {
                p.protocol.selectable_backends.push(Backend::GenericRyser);
            }),
        ),
        (
            "sample backend outside the inventory",
            Box::new(|p: &mut ValidationPreregistration| {
                p.protocol
                    .selectable_backends
                    .retain(|backend| *backend != Backend::BatchParallel);
            }),
        ),
        (
            "no anchors",
            Box::new(|p: &mut ValidationPreregistration| p.anchors.clear()),
        ),
        (
            "unsupported anchor",
            Box::new(|p: &mut ValidationPreregistration| p.anchors[0].n = 9),
        ),
        (
            "duplicated anchor address",
            Box::new(|p: &mut ValidationPreregistration| {
                let first = p.anchors[0];
                p.anchors.push(first);
            }),
        ),
        (
            "stream index beyond the low 56 bits",
            Box::new(|p: &mut ValidationPreregistration| p.anchors[0].stream_index = 1 << 56),
        ),
        (
            "inconsistent universe size",
            Box::new(|p: &mut ValidationPreregistration| p.anchors[0].expected_matrix_count += 1),
        ),
        (
            "zero count above the universe",
            Box::new(|p: &mut ValidationPreregistration| {
                p.anchors[0].expected_permanent_zero_count = p.anchors[0].expected_matrix_count + 1;
            }),
        ),
        (
            "singular count off the formula",
            Box::new(|p: &mut ValidationPreregistration| {
                p.anchors[0].expected_determinant_zero_count += 1;
            }),
        ),
        (
            "sample backend outside its kernel domain",
            Box::new(|p: &mut ValidationPreregistration| {
                p.protocol.sample_backend = Backend::IntraMatrixParallel;
                p.protocol.selectable_backends = vec![Backend::IntraMatrixParallel];
                p.anchors = vec![focused_anchor(5, 2, 145)];
            }),
        ),
    ];

    for (label, mutate) in cases {
        let mut plan = focused_plan();
        mutate(&mut plan);
        assert!(
            plan.validate().is_err(),
            "preregistration validation must reject a malformed {label}"
        );
    }
}

#[test]
fn a_preregistration_authority_digest_must_match_the_committed_bytes() {
    let root = unique_directory("validation-authorities");
    fs::create_dir_all(root.join("fixtures")).expect("the fixture directory is creatable");
    let mut plan = focused_plan();
    let mut authorities = Vec::new();
    for (name, contents) in [
        ("protocol.md", "protocol"),
        ("manifest.json", "manifest"),
        ("exact-anchors.csv", "anchors"),
        ("backend-equivalence.csv", "equivalence"),
    ] {
        let relative = format!("fixtures/{name}");
        fs::write(root.join(&relative), contents).expect("the fixture is writable");
        authorities.push(ArtifactIdentity {
            path: relative.parse().expect("the fixture path is normalized"),
            sha256: format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(contents))
                .parse()
                .expect("SHA-256 is canonical"),
        });
    }
    plan.authorities = ValidationAuthorities {
        protocol: authorities[0].clone(),
        manifest: authorities[1].clone(),
        exact_anchors: authorities[2].clone(),
        backend_equivalence: authorities[3].clone(),
    };
    let relative = Path::new("fixtures/preregistration.json");
    fs::write(
        root.join(relative),
        serde_json::to_vec_pretty(&plan).expect("the plan serializes"),
    )
    .expect("the plan is writable");

    let (loaded, identity) =
        load_validation_preregistration(&root, relative).expect("a content-bound plan loads");
    assert_eq!(loaded, plan);
    assert_eq!(identity.path.as_str(), "fixtures/preregistration.json");

    plan.authorities.exact_anchors.sha256 = digest('0');
    fs::write(
        root.join(relative),
        serde_json::to_vec_pretty(&plan).expect("the plan serializes"),
    )
    .expect("the plan is writable");
    let error = load_validation_preregistration(&root, relative)
        .expect_err("a mismatched authority digest is rejected");
    assert!(
        error.to_string().contains("artifact identity mismatch"),
        "{error}"
    );

    assert!(
        load_validation_preregistration(&root, &root.join(relative)).is_err(),
        "an absolute preregistration path is rejected"
    );
    fs::remove_dir_all(root).expect("the fixture directory is removable");
}

#[test]
fn validation_writes_no_campaign_artifact_and_leaves_the_frozen_payload_intact() {
    let repository = repository();
    let frozen = repository
        .join("dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829");
    let inventory = |root: &Path| -> Vec<(String, String)> {
        let mut entries: Vec<(String, String)> = fs::read_dir(root)
            .expect("the frozen campaign directory reads")
            .map(|entry| {
                let entry = entry.expect("the directory entry reads");
                let bytes = fs::read(entry.path()).expect("the frozen artifact reads");
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&bytes)),
                )
            })
            .collect();
        entries.sort();
        entries
    };
    let before = inventory(&frozen);

    let state = unique_directory("validation-no-campaign-artifact");
    let receipt = run_validation(&focused_plan(), focused_identity(), 1, &state)
        .expect("the focused anchors execute");
    assert!(receipt.passed());
    assert!(
        receipt.frozen_artifacts.is_none(),
        "the generic runner carries no repository-specific guard"
    );

    assert_eq!(
        before,
        inventory(&frozen),
        "validation must not change the frozen campaign payload or its inventory"
    );

    // The journal holds run state, phase markers, and terminal records only.
    let mut published: Vec<String> = fs::read_dir(&state)
        .expect("the journal directory reads")
        .map(|entry| {
            entry
                .expect("the directory entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    published.sort();
    assert_eq!(
        published,
        vec![
            "q3-n02-s0.exact.started.json".to_owned(),
            "q3-n02-s0.replay.started.json".to_owned(),
            "q3-n02-s0.sample.started.json".to_owned(),
            "q3-n02-s0.terminal.json".to_owned(),
            "q5-n02-s0.exact.started.json".to_owned(),
            "q5-n02-s0.replay.started.json".to_owned(),
            "q5-n02-s0.sample.started.json".to_owned(),
            "q5-n02-s0.terminal.json".to_owned(),
            "run-state.json".to_owned(),
        ],
        "validation publishes no shard, checkpoint, summary, or interpretation artifact"
    );
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn the_runner_refuses_an_incomplete_invocation_and_a_missing_receipt() {
    let binary = env!("CARGO_BIN_EXE_permanent_validation");
    let repository = repository();

    let output = std::process::Command::new(binary)
        .current_dir(&repository)
        .output()
        .expect("the runner executes");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--preregistration"),
        "an incomplete invocation names the required options"
    );

    let output = std::process::Command::new(binary)
        .current_dir(&repository)
        .args([
            "--verify-receipt",
            "dev/active/02b8137c/absent-receipt.json",
        ])
        .output()
        .expect("the runner executes");
    assert_eq!(
        output.status.code(),
        Some(1),
        "a missing receipt is a runtime error, not a verdict"
    );

    let output = std::process::Command::new(binary)
        .current_dir(&repository)
        .args([
            "--verify-receipt",
            "dev/active/02b8137c/pre-draw-validation-v1-preregistration.json",
        ])
        .output()
        .expect("the runner executes");
    assert_eq!(
        output.status.code(),
        Some(1),
        "a preregistration is not a receipt"
    );
}

#[test]
fn frozen_validation_evidence_is_pinned_to_its_producing_toolchain() {
    assert_eq!(FROZEN_TOOLCHAIN_PREFIX, "rustc 1.95.0 ");
    assert!(is_frozen_validation_toolchain(
        "rustc 1.95.0 (59807616e 2026-04-14)"
    ));
    assert!(!is_frozen_validation_toolchain(
        "rustc 1.97.0 (2d8144b78 2026-07-07)"
    ));
    assert!(
        !is_frozen_validation_toolchain("rustc 1.95.0"),
        "the prefix ends at the separator, so 1.95.01 cannot match"
    );
    assert!(!is_frozen_validation_toolchain(""));
}
