//! Contract checks for the frozen protocol-v4 DVB profile addendum
//! (jit:9fb40c83).

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use survey_gf2_side::CachePolicy;
use tuning_campaign_support::arm::ArmRequest;
use tuning_campaign_support::protocol::{CellRole, FamilyAddendum, MetricKind, RunnerPlan};
use tuning_campaign_support::provenance::ProducingInputs;
use tuning_campaign_support::schema;
use tuning_campaign_support::transport::{encode_case, FRESH_CASE_VALUE, FRESH_CASE_VAR};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../..");
const SCHEMA: &str = "dev/active/f547c394/addendum.schema.json";
const ADDENDUM: &str = "dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json";
const PRODUCING: &str =
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/dvb-producing-inputs.json";
const PROFILE_CASES: &str =
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/profile-cases.txt";

fn read(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(REPO).join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn profile_addendum_is_frozen_valid_and_fully_exploratory() {
    let schema_value: serde_json::Value =
        serde_json::from_slice(&read(SCHEMA)).expect("schema decodes");
    let bytes = read(ADDENDUM);
    let instance: serde_json::Value = serde_json::from_slice(&bytes).expect("addendum decodes");
    let violations = schema::validate(&schema_value, &instance);
    assert!(violations.is_empty(), "schema violations: {violations:#?}");
    let addendum = FamilyAddendum::decode(&bytes).expect("typed addendum decodes");
    addendum
        .validate()
        .unwrap_or_else(|errors| panic!("semantic violations: {errors:#?}"));
    assert!(addendum.frozen.frozen_utc.is_some());
    assert_eq!(addendum.cells.len(), 10);
    assert!(addendum
        .cells
        .iter()
        .all(|cell| cell.role == CellRole::Exploratory));
}

#[test]
fn producing_input_closure_is_valid_and_complete_on_disk() {
    ProducingInputs::read_at(Path::new(REPO), PRODUCING).expect("producing closure validates");
}

#[test]
fn perf_case_declaration_covers_the_dynamic_ladder() {
    let output = Command::new(env!("CARGO_BIN_EXE_dvb-profile"))
        .arg("ladder")
        .output()
        .expect("profile ladder runs");
    assert!(output.status.success());
    let ladder: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("profile ladder decodes");
    let ladder_ids: BTreeSet<_> = ladder
        .iter()
        .map(|case| case["id"].as_str().expect("case has id"))
        .collect();
    let declaration = std::fs::read_to_string(Path::new(REPO).join(PROFILE_CASES))
        .expect("profile case declaration reads");
    let declared: BTreeSet<_> = declaration
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(declared, ladder_ids);
}

#[test]
fn timed_binaries_fail_closed_outside_the_benchmark_window() {
    let output_path = std::env::temp_dir().join(format!(
        "gf2-9fb40c83-forbidden-session-{}.json",
        std::process::id()
    ));
    assert!(!output_path.exists());
    let profile = Command::new(env!("CARGO_BIN_EXE_dvb-profile"))
        .args(["session"])
        .arg(&output_path)
        .env_remove("GF2_BENCH")
        .env_remove("GF2_BENCH_WINDOW")
        .output()
        .expect("profile guard runs");
    assert_eq!(profile.status.code(), Some(2));
    assert!(!output_path.exists());

    let arm = Command::new(env!("CARGO_BIN_EXE_dvb-profile-arm"))
        .env_remove("GF2_BENCH")
        .env_remove("GF2_BENCH_WINDOW")
        .env_remove(FRESH_CASE_VAR)
        .output()
        .expect("arm guard runs");
    assert_eq!(arm.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&arm.stderr).contains("requires GF2_BENCH_WINDOW=1"));
}

/// The exact request `benchmark-ab-runner` writes for a cell of this family.
///
/// The runner serializes its own request type with `serde_json::to_vec`, and
/// `transport::decode_case` accepts only bytes the arm's mirror re-encodes
/// unchanged, so the absent `cold_calls` and `decoder` are part of the wire.
const RUNNER_REQUEST: &str = concat!(
    r#"{"schema":"zen3-benchmark-arm-request-v1","#,
    r#""cell_id":"dvb-t2-qam16-r12-short-warm-isolated-null","#,
    r#""arm":"gf2-direct-a","role":"baseline","pair":0,"#,
    r#""case":{"modcod":"qam16-r12-short","seed":2103},"cache_state":"warm","#,
    r#""windows":5,"window_target_ms":100,"cpus":[0],"workers_declared":2}"#,
);

#[test]
fn a_campaign_child_decodes_the_runner_request_wire() {
    let mut arm = Command::new(env!("CARGO_BIN_EXE_dvb-profile-arm"))
        .env_remove("GF2_BENCH")
        .env_remove("GF2_BENCH_WINDOW")
        .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
        .env("GF2_DVB_PROFILE_ROUTE", "gf2-direct-a")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("arm child spawns");
    arm.stdin
        .take()
        .expect("child stdin is piped")
        .write_all(RUNNER_REQUEST.as_bytes())
        .expect("request writes without a broken pipe");
    let arm = arm.wait_with_output().expect("arm child completes");
    // The request declares two workers, which this profile refuses after it
    // decodes, so the wire contract is observed without timing anything.
    let stderr = String::from_utf8_lossy(&arm.stderr).into_owned();
    assert!(
        stderr.contains("one-worker non-decoder"),
        "the child rejected the runner's wire: {stderr}"
    );
    assert_eq!(arm.status.code(), Some(2));
}

/// The cell the non-timed arm smoke validates first for the Short frame.
const VALIDATION_CELL: &str = "dvb-t2-qam16-r12-short-warm-isolated-null";

#[test]
fn the_validation_role_dispatches_once_and_reports_no_window() {
    // The request is the shared smoke's own: `ArmRequest::validation` over the
    // projected plan cell and the frozen declaration, encoded by the transport
    // the runner uses, so the fixture cannot drift from the wire it mirrors.
    let addendum = FamilyAddendum::decode(&read(ADDENDUM)).expect("typed addendum decodes");
    let plan = projected_plan();
    let cell = plan
        .cells
        .iter()
        .find(|cell| cell.cell_id == VALIDATION_CELL)
        .expect("the projected plan carries the validated cell");
    let declared = addendum
        .cell(&cell.cell_id)
        .expect("the addendum declares the validated cell");
    let declared_arm = plan
        .arms
        .get(&cell.baseline_arm)
        .expect("the projected plan declares the cell's baseline arm");
    let request = ArmRequest::validation(cell, declared, &cell.baseline_arm);
    let encoded = encode_case(&request).expect("the runner's encoder accepts its own request");

    let mut child = Command::new(env!("CARGO_BIN_EXE_dvb-profile-arm"));
    child
        .env_remove("GF2_BENCH")
        .env_remove("GF2_BENCH_WINDOW")
        .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in &declared_arm.environment {
        child.env(name, value);
    }
    let mut arm = child.spawn().expect("arm child spawns");
    arm.stdin
        .take()
        .expect("child stdin is piped")
        .write_all(encoded.as_bytes())
        .expect("request writes without a broken pipe");
    let arm = arm.wait_with_output().expect("arm child completes");
    let stdout = String::from_utf8_lossy(&arm.stdout).into_owned();
    assert_eq!(arm.status.code(), Some(0), "{stdout}");
    assert!(arm.stderr.is_empty());
    assert!(stdout.contains(r#""windows":[]"#), "{stdout}");
    assert!(stdout.contains(r#""conversion":null"#), "{stdout}");
    assert!(
        stdout.contains("DvbT2BitInterleaver::interleave/scalar-bit-scatter/qam16-r12-short"),
        "{stdout}"
    );
}

#[test]
fn profile_matrix_covers_modcod_boundaries_and_cache_states() {
    let addendum = FamilyAddendum::decode(&read(ADDENDUM)).expect("typed addendum decodes");
    let identities: BTreeSet<_> = addendum
        .cells
        .iter()
        .map(|cell| cell.workload.identity.as_str())
        .collect();
    for modcod in [
        "qam16-r12-normal",
        "qam64-r12-normal",
        "qam16-r12-short",
        "qam64-r12-short",
    ] {
        assert!(identities.contains(format!("dvb-t2-bit-interleave-{modcod}-isolated").as_str()));
        assert!(identities.contains(format!("dvb-t2-bit-interleave-{modcod}-sim-stage").as_str()));
    }
    let mut isolated = 0;
    let mut whole = 0;
    let mut streaming = 0;
    for cell in &addendum.cells {
        let cache = serde_json::to_value(cell.cache_state).expect("cache encodes");
        let cache = cache.as_str().expect("cache is a string");
        assert!(
            CachePolicy::from_request(cache).is_ok(),
            "{}: {cache}",
            cell.cell_id
        );
        streaming += usize::from(cache == "streaming");
        match cell.metric_kind {
            MetricKind::KernelIsolated => {
                isolated += 1;
                assert!(!cell.conversion_costs_included);
            }
            MetricKind::WholeConsumer => {
                whole += 1;
                assert!(cell.conversion_costs_included);
            }
        }
    }
    assert_eq!((isolated, whole, streaming), (5, 5, 2));
}

/// The runner plan of this campaign, projected by the committed projector.
fn projected_plan() -> RunnerPlan {
    let plan_path = std::env::temp_dir().join(format!(
        "gf2-9fb40c83-plan-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    let executable = std::env::current_exe().expect("test executable resolves");
    let status = Command::new("python3")
        .current_dir(REPO)
        .args([
            "-B",
            "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-dvb-plan.py",
            "--addendum",
            ADDENDUM,
            "--campaign-id",
            "v4-r2-9fb40c83-dvb-interleave-profile",
            "--campaign-seed",
            "20260915",
            "--lock",
            "/tmp/gf2-ccx1.lock",
            "--executable",
        ])
        .arg(executable)
        .args(["--producing-manifest", PRODUCING, "--output"])
        .arg(&plan_path)
        .args(["--max-cells-per-session", "2", "--pilot-pairs", "6"])
        .status()
        .expect("plan projection runs");
    assert!(status.success());
    let plan = RunnerPlan::decode(&std::fs::read(&plan_path).expect("plan reads"))
        .expect("typed plan decodes");
    std::fs::remove_file(plan_path).expect("temporary plan removes");
    plan
}

#[test]
fn projected_plan_is_valid_for_the_frozen_addendum() {
    let addendum = FamilyAddendum::decode(&read(ADDENDUM)).expect("typed addendum decodes");
    projected_plan()
        .validate(&addendum)
        .unwrap_or_else(|errors| panic!("plan violations: {errors:#?}"));
}
