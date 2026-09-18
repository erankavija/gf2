//! The arm dispatch the non-timed smoke reaches its arms through.
//!
//! Its own test binary: `tuning_campaign_support::process` makes this process a
//! child subreaper and reaps any adopted child, so a dispatch here would take
//! the exit status of a child another test in the same process spawned.

use dense_parity_harness::campaign::{self, PlanInputs};
use dense_parity_harness::cells::{Question, ADDENDUM_FROZEN_UTC};
use tuning_campaign_support::arm::{
    validate_arm, ArmResult, PairPosition, ARM_RESULT_SCHEMA,
};
use tuning_campaign_support::protocol::{CellDeclaration, FamilyAddendum, ReceiptLabel, RunnerPlan};
use tuning_campaign_support::receipt::WindowRecord;
use tuning_campaign_support::transport;

const ISSUE: &str = "e1f9a78f";

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("the harness crate sits five directories below the repository root")
        .canonicalize()
        .expect("the repository root resolves")
}

/// One family's projected plan, with `gf2` as the executable of its gf2 arms.
fn projected(question: Question, addendum: &FamilyAddendum, gf2: &str) -> RunnerPlan {
    let plan = campaign::plan(
        question,
        addendum,
        &PlanInputs {
            campaign_id: "e1f9a78f-smoke-dispatch",
            campaign_seed: 1,
            label: ReceiptLabel::Smoke,
            addendum_path: "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaign.json",
            producing_manifest: "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-producing-inputs.json",
            lock_path: "/tmp/gf2-contract.lock",
            gf2_executable: gf2,
            candidate_executable: None,
            scalar_executable: Some("/nonexistent/dense-arm-scalar"),
            m4ri_executable: Some("/nonexistent/dense-m4ri-arm"),
            max_cells_per_session: None,
        },
    )
    .expect("the plan projects");
    plan.validate(addendum)
        .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
    plan
}

/// A fake arm executable answering one request with `windows` timing windows, so
/// the dispatch is exercised from an arm's own result line.
fn scripted_arm(
    name: &str,
    declared: &CellDeclaration,
    windows: Vec<WindowRecord>,
) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows,
        cache_state_applied: declared.cache_state,
        workers_observed: 1,
        cpus_observed: vec![0],
        selected_path: Some("scripted-arm".to_owned()),
        conversion: None,
        quality: None,
        calibrated: Some(false),
    };
    let line = transport::encode_result_line(&result).expect("the canonical result line");
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("the scratch tree");
    let path = root.join("arm.sh");
    std::fs::write(
        &path,
        format!("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{line}'\n"),
    )
    .expect("the scripted arm is written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("the scripted arm is executable");
    path
}

/// The smoke reaches its arms through the shared dispatch, which completes the
/// validation position on an untimed answer and fails the arm that reports a
/// timing window.
#[test]
fn the_smoke_dispatch_fails_an_arm_that_reports_a_timing_window() {
    let root = repository_root();
    let addendum = campaign::addendum(Question::IsolatedFusedParity, ISSUE, ADDENDUM_FROZEN_UTC);
    let declared = &addendum.cells[0];

    let untimed = scripted_arm("smoke-untimed-arm", declared, Vec::new());
    let plan = projected(
        Question::IsolatedFusedParity,
        &addendum,
        untimed.to_str().expect("a UTF-8 path"),
    );
    let cell = &plan.cells[0];
    assert_eq!(cell.cell_id, declared.cell_id);
    let validated = validate_arm(
        &root,
        &plan,
        cell,
        declared,
        &cell.baseline_arm,
        dense_parity_harness::smoke::CHILD_TIMEOUT,
        &mut (),
    )
    .expect("an untimed answer completes the validation position");
    assert_eq!(validated.windows, 0);
    assert_eq!(validated.role, PairPosition::Validation);
    assert_eq!(validated.cache_state_applied, declared.cache_state);

    let reporting = scripted_arm(
        "smoke-timed-arm",
        declared,
        vec![WindowRecord { calls: 1, elapsed_ns: 1 }],
    );
    let plan = projected(
        Question::IsolatedFusedParity,
        &addendum,
        reporting.to_str().expect("a UTF-8 path"),
    );
    let cell = &plan.cells[0];
    let refusal = validate_arm(
        &root,
        &plan,
        cell,
        declared,
        &cell.baseline_arm,
        dense_parity_harness::smoke::CHILD_TIMEOUT,
        &mut (),
    )
    .expect_err("an arm that reports a timing window fails the smoke");
    assert!(refusal.to_string().contains("timing window"), "{refusal}");
}
