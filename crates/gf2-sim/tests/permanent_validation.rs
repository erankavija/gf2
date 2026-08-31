//! Behavioral coverage for the frozen pre-draw campaign validation phase.
//!
//! These tests exercise the reusable library computation, the durable
//! no-redraw journal, and the immutable receipt contract on focused anchors.
//! The committed ten-anchor plan is loaded and content-verified here, but its
//! 400,000-draw evidence run belongs to the execution lead, not to this tier.

use std::collections::BTreeMap;
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
    admit_validation_run, evaluate_validation_anchor, is_frozen_validation_toolchain,
    load_frozen_campaign_validation_preregistration, load_validation_continuation_authorization,
    load_validation_preregistration, preflight_required_backends,
    publish_validation_receipt_atomic, read_validation_receipt, run_validation,
    verify_validation_receipt_journal, AnchorReceipt, AnchorSpec, BackendAgreementStatus,
    DecisionRule, FrozenArtifactGuard, FrozenArtifactSnapshot, PhaseStatus, ReplayMode, RetryRule,
    SampleOrigin, ValidationAuthorities, ValidationFailure, ValidationPhase,
    ValidationPreregistration, ValidationProducerSegment, ValidationProtocol, ValidationReceipt,
    ValidationRunMode, ValidationStreamPurpose, ValidationVerdict, CONTINUATION_STATE_FILE,
    FROZEN_TOOLCHAIN_PREFIX, FROZEN_VALIDATION_RECEIPT_PATH, PREREGISTRATION_SCHEMA_VERSION,
    RECEIPT_SCHEMA_VERSION,
};
use gf2_stats::binomial::two_sided_test;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};

/// Committed frozen plan for issue `02b8137c`, verified by this suite before
/// the execution lead consumes it.
const FROZEN_PREREGISTRATION: &str =
    "dev/active/02b8137c/pre-draw-validation-v1-preregistration.json";
const FROZEN_CONTINUATION: &str = "dev/active/02b8137c/pre-draw-validation-v2-continuation.json";
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

fn content_digest(bytes: &[u8]) -> Sha256Digest {
    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes))
        .parse()
        .expect("SHA-256 is canonical")
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
fn fresh_validation_uses_the_single_canonical_schema_v2_state_model() {
    let (receipt, state) = passing_receipt("validation-v2-cutover");
    assert_eq!(receipt.schema_version, 2);
    assert_eq!(receipt.producer_segments.len(), 1);
    assert_eq!(receipt.producer_segments[0].first_anchor_index, 0);
    assert_eq!(receipt.producer_segments[0].end_anchor_index, 2);
    assert_eq!(receipt.anchor_producers.len(), 2);

    for file in [
        "run-state.json",
        "q3-n02-s0.exact.started.json",
        "q3-n02-s0.replay.started.json",
        "q3-n02-s0.sample.started.json",
    ] {
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(state.join(file)).expect("the canonical journal state reads"),
        )
        .expect("the canonical journal state decodes");
        assert_eq!(value["schema_version"], 2, "{file} must use schema v2");
    }
    assert!(!state.join(CONTINUATION_STATE_FILE).exists());
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
    let mut terminal_bytes =
        serde_json::to_vec_pretty(&altered.anchors[0]).expect("the altered terminal encodes");
    terminal_bytes.push(b'\n');
    altered.anchor_producers[0].terminal.sha256 = format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(&terminal_bytes)
    )
    .parse()
    .expect("the altered terminal digest is canonical");
    let error = publish_validation_receipt_atomic(&output, &altered)
        .expect_err("an immutable receipt is never overwritten");
    assert!(error.to_string().contains("incompatible"), "{error}");
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

fn state_with_committed_q5_terminal(label: &str) -> (ValidationPreregistration, PathBuf) {
    let state = unique_directory(label);
    let mut bootstrap = focused_plan();
    bootstrap.anchors = vec![focused_anchor(3, 1, 0)];
    run_validation(&bootstrap, focused_identity(), 1, &state)
        .expect("the exact-failing bootstrap records current runtime without sampling");

    let repository = repository();
    let (mut plan, _) = load_frozen_campaign_validation_preregistration(
        &repository,
        Path::new(FROZEN_PREREGISTRATION),
    )
    .expect("the committed frozen plan is valid");
    plan.anchors.retain(|anchor| (anchor.q, anchor.n) == (5, 1));

    let terminal_source =
        repository.join("dev/active/02b8137c/validation-journal/q5-n01-s0.terminal.json");
    let terminal_bytes = fs::read(&terminal_source).expect("the committed q=5 n=1 terminal reads");
    let terminal: AnchorReceipt =
        serde_json::from_slice(&terminal_bytes).expect("the committed terminal decodes");

    let run_state_path = state.join("run-state.json");
    let mut run_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&run_state_path).expect("the bootstrap run state reads"))
            .expect("the bootstrap run state decodes");
    run_state["preregistration"] = serde_json::to_value(&plan).expect("the q=5 n=1 plan encodes");
    run_state["preregistration_identity"] =
        serde_json::to_value(focused_identity()).expect("the focused identity encodes");
    run_state["started_at"] =
        serde_json::to_value(terminal.started_at).expect("the terminal timestamp encodes");
    fs::write(
        run_state_path,
        serde_json::to_vec_pretty(&run_state).expect("the q=5 n=1 run state encodes"),
    )
    .expect("the q=5 n=1 run state is writable");
    fs::write(state.join("q5-n01-s0.terminal.json"), terminal_bytes)
        .expect("the committed terminal fixture is writable");
    (plan, state)
}

fn copy_committed_validation_journal(label: &str) -> PathBuf {
    let destination = unique_directory(label);
    fs::create_dir_all(&destination).expect("the journal fixture directory is creatable");
    let source = repository().join("dev/active/02b8137c/validation-journal");
    for entry in fs::read_dir(source).expect("the committed journal directory reads") {
        let entry = entry.expect("the committed journal entry reads");
        if entry
            .file_type()
            .expect("the journal entry has a type")
            .is_file()
        {
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .expect("the immutable journal fixture copies");
        }
    }
    destination
}

#[test]
fn the_current_journal_admits_an_ordered_continuation_before_q5_n2_starts() {
    let repository = repository();
    let (plan, identity) = load_frozen_campaign_validation_preregistration(
        &repository,
        Path::new(FROZEN_PREREGISTRATION),
    )
    .expect("the committed frozen plan is valid");
    let state = copy_committed_validation_journal("validation-current-continuation");
    let preserved =
        fs::read(state.join("q5-n01-s0.terminal.json")).expect("the last old terminal reads");

    let authorization =
        load_validation_continuation_authorization(&repository, Path::new(FROZEN_CONTINUATION))
            .expect("the committed continuation authorization is valid");
    let admission = admit_validation_run(
        &plan,
        identity,
        24,
        &state,
        ValidationRunMode::ContinueWith(Box::new(authorization)),
    )
    .expect("the exact committed prefix admits its authorized continuation");

    assert_eq!(admission.completed_anchor_count(), 5);
    let next = admission.next_address().expect("five anchors remain");
    assert_eq!((next.q, next.n, next.stream_index), (5, 2, 0));
    assert!(state.join("producer-segment-state-v2.json").is_file());
    assert!(
        !state.join("q5-n02-s0.exact.started.json").exists(),
        "admission publishes the continuation before the next address is opened"
    );
    assert_eq!(
        fs::read(state.join("q5-n01-s0.terminal.json"))
            .expect("the adopted old terminal still reads"),
        preserved,
        "continuation admission never executes or rewrites an old anchor"
    );
    for entry in fs::read_dir(&state).expect("the continued journal reads") {
        let name = entry
            .expect("the continued journal entry reads")
            .file_name()
            .to_string_lossy()
            .into_owned();
        assert!(
            ![
                "shard",
                "coordinator",
                "checkpoint",
                "field-summary",
                "pooled-summary",
                "interpretation",
            ]
            .iter()
            .any(|forbidden| name.contains(forbidden)),
            "continuation published forbidden campaign artifact {name}"
        );
    }
    fs::remove_dir_all(state).expect("the journal fixture is removable");
}

fn current_continuation_inputs(
    label: &str,
) -> (
    ValidationPreregistration,
    ArtifactIdentity,
    PathBuf,
    gf2_sim::permanent_campaign::validation::AuthorizedValidationContinuation,
) {
    let repository = repository();
    let (plan, identity) = load_frozen_campaign_validation_preregistration(
        &repository,
        Path::new(FROZEN_PREREGISTRATION),
    )
    .expect("the frozen plan is valid");
    let authorization =
        load_validation_continuation_authorization(&repository, Path::new(FROZEN_CONTINUATION))
            .expect("the continuation authorization is valid");
    (
        plan,
        identity,
        copy_committed_validation_journal(label),
        authorization,
    )
}

#[test]
fn default_resume_refuses_the_original_producer_mismatch_without_opening_q5_n2() {
    let (plan, identity, state, _) = current_continuation_inputs("validation-default-refusal");
    let error = run_validation(&plan, identity, 24, &state)
        .expect_err("default resume cannot weaken exact producer identity");
    assert!(error.to_string().contains("incompatible"), "{error}");
    assert!(!state.join(CONTINUATION_STATE_FILE).exists());
    assert!(!state.join("q5-n02-s0.exact.started.json").exists());
    fs::remove_dir_all(state).expect("the journal fixture is removable");
}

#[test]
fn canonical_v2_resume_refuses_runtime_drift_before_any_journal_change() {
    let state = unique_directory("validation-v2-runtime-mismatch");
    let plan = focused_plan();
    run_validation(&plan, focused_identity(), 1, &state)
        .expect("the canonical schema-v2 journal completes");
    let snapshot = |directory: &Path| -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(directory)
            .expect("the canonical journal reads")
            .map(|entry| {
                let entry = entry.expect("the canonical journal entry reads");
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).expect("the canonical journal bytes read"),
                )
            })
            .collect()
    };
    let before = snapshot(&state);
    let run_state: serde_json::Value = serde_json::from_slice(
        before
            .get("run-state.json")
            .expect("the canonical run state is present"),
    )
    .expect("the canonical run state decodes");
    assert_eq!(run_state["schema_version"], 2);

    let error = run_validation(&plan, focused_identity(), 2, &state)
        .expect_err("worker-count drift changes canonical runtime identity");
    assert!(error.to_string().contains("incompatible"), "{error}");
    assert_eq!(
        snapshot(&state),
        before,
        "runtime mismatch must precede every additional marker, terminal, or rewrite"
    );
    fs::remove_dir_all(state).expect("the canonical journal fixture is removable");
}

#[test]
fn continuation_rejects_changed_prefix_bytes_gaps_and_preopened_suffixes() {
    for (label, alter) in [
        (
            "run-state",
            Box::new(|state: &Path| {
                let path = state.join("run-state.json");
                let mut bytes = fs::read(&path).expect("the run state reads");
                bytes.push(b' ');
                fs::write(path, bytes).expect("the fixture run state changes");
            }) as Box<dyn Fn(&Path)>,
        ),
        (
            "terminal",
            Box::new(|state: &Path| {
                let path = state.join("q3-n03-s0.terminal.json");
                let mut bytes = fs::read(&path).expect("the terminal reads");
                bytes.push(b' ');
                fs::write(path, bytes).expect("the fixture terminal changes");
            }),
        ),
        (
            "started-without-terminal",
            Box::new(|state: &Path| {
                fs::remove_file(state.join("q3-n02-s0.terminal.json"))
                    .expect("the fixture terminal is removable");
            }),
        ),
        (
            "preopened-suffix",
            Box::new(|state: &Path| {
                fs::copy(
                    state.join("q5-n01-s0.exact.started.json"),
                    state.join("q5-n02-s0.exact.started.json"),
                )
                .expect("the suffix fixture is creatable");
            }),
        ),
    ] {
        let (plan, identity, state, authorization) =
            current_continuation_inputs(&format!("validation-reject-{label}"));
        alter(&state);
        let error = admit_validation_run(
            &plan,
            identity,
            24,
            &state,
            ValidationRunMode::ContinueWith(Box::new(authorization)),
        )
        .expect_err("changed or non-prefix journal evidence is never admitted");
        assert!(
            !state.join(CONTINUATION_STATE_FILE).exists(),
            "{label}: {error}"
        );
        assert!(!state.join("q5-n02-s0.sample.started.json").exists());
        fs::remove_dir_all(state).expect("the journal fixture is removable");
    }
}

#[test]
fn an_immutable_second_segment_refuses_a_third_runtime() {
    let (plan, identity, state, authorization) =
        current_continuation_inputs("validation-third-producer");
    let first = admit_validation_run(
        &plan,
        identity.clone(),
        24,
        &state,
        ValidationRunMode::ContinueWith(Box::new(authorization.clone())),
    )
    .expect("the authorized second producer is admitted");
    drop(first);
    let state_before =
        fs::read(state.join(CONTINUATION_STATE_FILE)).expect("the segment state reads");

    let error = admit_validation_run(
        &plan,
        identity,
        23,
        &state,
        ValidationRunMode::ContinueWith(Box::new(authorization)),
    )
    .expect_err("a different worker configuration is a third producer");
    assert!(error.to_string().contains("incompatible"), "{error}");
    assert_eq!(
        fs::read(state.join(CONTINUATION_STATE_FILE)).expect("the segment state still reads"),
        state_before,
        "third-producer refusal cannot rewrite the admitted segment state"
    );
    assert!(!state.join("q5-n02-s0.exact.started.json").exists());
    fs::remove_dir_all(state).expect("the journal fixture is removable");
}

#[test]
fn receipt_segments_and_terminal_identities_require_exact_ordered_coverage() {
    let (receipt, state) = two_segment_receipt_fixture("validation-segment-receipt");
    receipt
        .validate()
        .expect("two exact contiguous segments validate");

    for (label, alter) in [
        (
            "segment overlap",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].end_anchor_index = 2;
            }) as Box<dyn Fn(&mut ValidationReceipt)>,
        ),
        (
            "segment reorder",
            Box::new(|r: &mut ValidationReceipt| r.producer_segments.swap(0, 1)),
        ),
        (
            "address mapping",
            Box::new(|r: &mut ValidationReceipt| r.anchor_producers[1].address.n = 1),
        ),
        (
            "producer mapping",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchor_producers[1].producer_segment = 0;
            }),
        ),
        (
            "terminal digest",
            Box::new(|r: &mut ValidationReceipt| {
                r.anchor_producers[1].terminal.sha256 = digest('0');
            }),
        ),
        (
            "segment runtime",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[1].runtime.worker_count = 0;
            }),
        ),
    ] {
        let mut altered = receipt.clone();
        alter(&mut altered);
        assert!(altered.validate().is_err(), "receipt accepted {label}");
    }
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

fn two_segment_receipt_fixture(label: &str) -> (ValidationReceipt, PathBuf) {
    let (mut receipt, state) = passing_receipt(label);
    let mut second_runtime = receipt.producer_segments[0].runtime.clone();
    second_runtime.worker_count += 1;
    let segment_state_bytes = b"{\"schema_version\":2,\"fixture\":\"producer-two\"}\n";
    fs::write(state.join(CONTINUATION_STATE_FILE), segment_state_bytes)
        .expect("the fixture segment state is writable");
    receipt.producer_segments[0].end_anchor_index = 1;
    receipt.producer_segments.push(ValidationProducerSegment {
        segment_index: 1,
        first_anchor_index: 1,
        end_anchor_index: 2,
        runtime: second_runtime,
        state: ArtifactIdentity {
            path: CONTINUATION_STATE_FILE
                .parse()
                .expect("the segment-state path is canonical"),
            sha256: content_digest(segment_state_bytes),
        },
    });
    receipt.anchor_producers[1].producer_segment = 1;
    (receipt, state)
}

#[test]
fn journal_verifier_rehashes_each_segment_state_and_terminal_on_disk() {
    let (receipt, state) = two_segment_receipt_fixture("validation-journal-rehash");
    verify_validation_receipt_journal(&receipt, &state)
        .expect("the unmodified two-segment journal verifies");

    for file in [
        "run-state.json",
        CONTINUATION_STATE_FILE,
        "q3-n02-s0.terminal.json",
        "q5-n02-s0.terminal.json",
    ] {
        let path = state.join(file);
        let original = fs::read(&path).expect("the immutable fixture artifact reads");
        let mut altered = original.clone();
        altered.push(b' ');
        fs::write(&path, altered).expect("the temporary fixture artifact changes");
        let error = verify_validation_receipt_journal(&receipt, &state)
            .expect_err("on-disk identity drift must fail journal verification");
        assert!(
            error.to_string().contains("identity mismatch"),
            "{file}: {error}"
        );
        fs::write(&path, original).expect("the temporary fixture artifact restores");
        verify_validation_receipt_journal(&receipt, &state)
            .expect("the restored journal verifies before the next mutation");
    }
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn q5_exact_test_values_survive_json_publication_bit_exact() {
    let recomputed = two_sided_test(80_029, 400_000, 0.2);
    let values = (recomputed.log_p_value(), recomputed.log_p_value().exp());
    let encoded = serde_json::to_vec(&values).expect("the exact-test values encode");
    let decoded: (f64, f64) =
        serde_json::from_slice(&encoded).expect("the exact-test values decode");

    assert_eq!(values.0.to_bits(), 0xbfb8_7fcf_b6c9_e338);
    assert_eq!(values.1.to_bits(), 0x3fed_145e_4cad_104e);
    assert_eq!(
        (decoded.0.to_bits(), decoded.1.to_bits()),
        (values.0.to_bits(), values.1.to_bits()),
        "publication must preserve the exact values used by bitwise receipt validation"
    );
}

#[test]
fn the_committed_q5_terminal_is_adopted_without_float_drift_or_redraw() {
    let (plan, state) = state_with_committed_q5_terminal("validation-q5-adoption");
    let terminal_path = state.join("q5-n01-s0.terminal.json");
    let before = fs::read(&terminal_path).expect("the terminal fixture reads");
    let terminal: AnchorReceipt =
        serde_json::from_slice(&before).expect("the terminal fixture decodes");
    let sample = terminal
        .sample
        .as_ref()
        .expect("the terminal has sample evidence");
    let recomputed = two_sided_test(80_029, 400_000, 0.2);

    assert_eq!(recomputed.log_p_value().to_bits(), 0xbfb8_7fcf_b6c9_e338);
    assert_eq!(
        recomputed.log_p_value().exp().to_bits(),
        0x3fed_145e_4cad_104e
    );
    assert_eq!(
        sample.log_p_value.to_bits(),
        recomputed.log_p_value().to_bits()
    );
    assert_eq!(
        sample.p_value.to_bits(),
        recomputed.log_p_value().exp().to_bits()
    );

    let receipt = run_validation(&plan, focused_identity(), 1, &state)
        .expect("the valid committed terminal is adopted without recomputation");
    assert_eq!(receipt.anchors, vec![terminal]);
    assert_eq!(
        fs::read(&terminal_path).expect("the adopted terminal still reads"),
        before,
        "adoption never rewrites immutable journal bytes"
    );
    fs::remove_dir_all(state).expect("the journal directory is removable");
}

#[test]
fn a_genuinely_altered_committed_terminal_is_rejected_without_overwrite() {
    let (plan, state) = state_with_committed_q5_terminal("validation-q5-altered");
    let terminal_path = state.join("q5-n01-s0.terminal.json");
    let mut altered: serde_json::Value =
        serde_json::from_slice(&fs::read(&terminal_path).expect("the terminal fixture reads"))
            .expect("the terminal fixture decodes");
    altered["sample"]["permanent_zero_count"] = serde_json::json!(80_030);
    let altered_bytes = serde_json::to_vec_pretty(&altered).expect("altered evidence encodes");
    fs::write(&terminal_path, &altered_bytes).expect("the altered fixture is writable");

    let error = run_validation(&plan, focused_identity(), 1, &state)
        .expect_err("different scientific evidence is rejected without sampling");
    assert!(
        error
            .to_string()
            .contains("sample p-value or strict decision is inconsistent"),
        "{error}"
    );
    assert_eq!(
        fs::read(&terminal_path).expect("the rejected terminal still reads"),
        altered_bytes,
        "rejection never overwrites journal bytes"
    );
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
            Box::new(|r: &mut ValidationReceipt| r.producer_segments[0].runtime.worker_count = 0),
        ),
        (
            "binary identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].runtime.provenance.binary_sha256 = None;
            }),
        ),
        (
            "source closure identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0]
                    .runtime
                    .provenance
                    .deps_source_revision = None;
            }),
        ),
        (
            "source closure state",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].runtime.provenance.deps_source_dirty = None;
            }),
        ),
        (
            "toolchain identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0]
                    .runtime
                    .provenance
                    .compiler_version
                    .clear();
            }),
        ),
        (
            "rng identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].runtime.provenance.rng_version =
                    "rand_chacha 0.0.0".to_owned();
            }),
        ),
        (
            "hardware identity",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].runtime.provenance.cpu_model.clear();
            }),
        ),
        (
            "invocation",
            Box::new(|r: &mut ValidationReceipt| {
                r.producer_segments[0].runtime.provenance.invocation.clear();
            }),
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
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--continue-producer-segment"),
        "runner help exposes the explicit continuation authority flag"
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
fn the_runner_refuses_the_superseded_v1_receipt_path_before_journal_creation() {
    let state = unique_directory("validation-stale-receipt-path");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_permanent_validation"))
        .current_dir(repository())
        .args([
            "--preregistration",
            FROZEN_PREREGISTRATION,
            "--state-dir",
            state.to_str().expect("the temporary journal path is UTF-8"),
            "--receipt",
            "dev/active/02b8137c/pre-draw-validation-v1-receipt.json",
        ])
        .output()
        .expect("the runner executes");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("canonical schema-v2"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !state.exists(),
        "a stale receipt destination is rejected before journal admission"
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

#[test]
fn the_preflight_admits_every_backend_the_focused_plan_requires() {
    let plan = focused_plan();
    preflight_required_backends(&plan, 2).expect("every required CPU backend executes");

    // A backend outside a cell's kernel domain is not required there, so its
    // absence from that anchor is not a refusal.
    let mut narrow = plan.clone();
    narrow.protocol.sample_backend = Backend::GenericRyser;
    narrow.protocol.selectable_backends = vec![Backend::IntraMatrixParallel, Backend::GenericRyser];
    narrow.anchors = vec![focused_anchor(5, 2, 145)];
    assert!(!backend_supports_cell(Backend::IntraMatrixParallel, 5, 2));
    preflight_required_backends(&narrow, 1)
        .expect("an unsupported cell is skipped rather than refused");

    let error = preflight_required_backends(&plan, 0)
        .expect_err("a zero worker count cannot execute any backend");
    assert!(error.to_string().contains("worker count"), "{error}");
}

/// Without the accelerator build every anchor's required accelerator is
/// unavailable, so the preflight must refuse rather than let the run open an
/// address it could never redraw. This build has no HIP support, so the refusal
/// comes from the kernel inventory and touches no device.
#[cfg(not(feature = "hip"))]
#[test]
fn a_required_but_unavailable_backend_is_refused_before_any_address() {
    let mut plan = focused_plan();
    plan.protocol.selectable_backends = vec![Backend::BatchParallel, Backend::Accelerator];
    assert!(
        backend_supports_cell(Backend::Accelerator, 3, 2),
        "the accelerator's kernel domain covers this anchor, so it is required here"
    );

    let error = preflight_required_backends(&plan, 1)
        .expect_err("a required backend that cannot execute is refused");
    let message = error.to_string();
    assert!(message.contains("accelerator"), "{message}");
    assert!(message.contains("cannot execute"), "{message}");
}

/// The frozen runner must refuse a wrong producing toolchain before it creates
/// the journal, because an address opened under a refused build could never be
/// redrawn.
///
/// A 1.95.0 build would pass that check and begin the lead's evidence run, so
/// this test disables itself there rather than drawing 400,000 matrices.
#[test]
fn the_frozen_runner_refuses_a_wrong_toolchain_before_creating_the_journal() {
    if is_frozen_validation_toolchain(env!("GF2_BUILD_RUSTC_VERSION")) {
        eprintln!("skipping: a 1.95.0 build would start the frozen evidence run");
        return;
    }
    let state = unique_directory("validation-frozen-refusal");
    let receipt = repository().join(FROZEN_VALIDATION_RECEIPT_PATH);
    let receipt_before = fs::read(&receipt).ok();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_permanent_validation"))
        .current_dir(repository())
        .args([
            "--preregistration",
            FROZEN_PREREGISTRATION,
            "--state-dir",
            state.to_str().expect("the journal path is UTF-8"),
            "--receipt",
            FROZEN_VALIDATION_RECEIPT_PATH,
        ])
        .output()
        .expect("the runner executes");

    assert_eq!(output.status.code(), Some(1), "a refusal is not a verdict");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(FROZEN_TOOLCHAIN_PREFIX), "{stderr}");
    assert!(
        !state.exists(),
        "the runner must refuse before it creates the journal"
    );
    assert_eq!(
        fs::read(receipt).ok(),
        receipt_before,
        "a refused run leaves the canonical receipt unchanged"
    );
}
