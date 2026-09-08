//! Adapter correctness and quality evidence for the LDPC baseline survey
//! (jit:c077a88b).
//!
//! The validator drives the survey's arm executables exactly as the benchmark
//! runner does — one canonical child-v2 request each — but with a large
//! untimed quality pass and per-frame hard decisions written out. It then
//! compares the decisions of every pair of arms on the information window.
//! Two arms that fix the same parity-check matrix, the same recorded LLRs and
//! the same algorithm must agree on almost every frame; the measured
//! agreement is the evidence that an adapter expresses the operation it
//! claims, and a disagreement is reported rather than reworded.
//!
//! # Usage
//!
//! ```bash
//! ldpc-validate --plan <plan.json> --output <validation.json>
//! ```

use ldpc_survey::arm::FRESH_RESULT_PREFIX;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// Schema identifier of a validation plan.
const PLAN_SCHEMA: &str = "ldpc-survey-validation-plan-v1";
/// Schema identifier of a validation report.
const REPORT_SCHEMA: &str = "ldpc-survey-validation-v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    /// Number of frames every arm decodes in its quality pass.
    frames: u32,
    /// Directory the arms write their per-frame decisions into.
    scratch: String,
    groups: Vec<Group>,
}

/// One comparison group: arms that decode the identical bundle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    id: String,
    bundle: String,
    /// Human-readable statement of what the arms of this group share.
    matched_on: String,
    arms: Vec<ArmSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmSpec {
    name: String,
    executable: String,
    /// Case object forwarded to the arm, without `quality_frames` and
    /// `decisions_out`, which the validator fills in.
    case: Value,
    /// Whether this arm claims the matched algorithm of its group.
    matched_algorithm: bool,
    /// Backend selection the arm reads from its environment, exactly as the
    /// benchmark plan supplies it.
    #[serde(default)]
    environment: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct ArmReport {
    name: String,
    executable: String,
    executable_sha256: String,
    matched_algorithm: bool,
    environment: BTreeMap<String, String>,
    case: Value,
    selected_path: Option<String>,
    quality: Value,
}

#[derive(Serialize)]
struct Agreement {
    left: String,
    right: String,
    frames: u64,
    frames_identical: u64,
    bits: u64,
    bits_differing: u64,
    frame_agreement: f64,
    frame_agreement_interval: [f64; 2],
}

#[derive(Serialize)]
struct GroupReport {
    id: String,
    bundle: String,
    matched_on: String,
    manifest: Value,
    arms: Vec<ArmReport>,
    agreement: Vec<Agreement>,
}

#[derive(Serialize)]
struct Report {
    schema: String,
    frames: u32,
    groups: Vec<GroupReport>,
}

/// Runs one arm and returns its result value together with its decisions.
fn run_arm(
    spec: &ArmSpec,
    bundle: &str,
    frames: u32,
    decisions: &Path,
) -> Result<(Value, Vec<u8>), String> {
    let mut case = spec.case.clone();
    let object = case
        .as_object_mut()
        .ok_or_else(|| format!("arm {} case is not an object", spec.name))?;
    object.insert("bundle".into(), Value::String(bundle.to_owned()));
    object.insert("quality_frames".into(), Value::from(frames));
    object.insert(
        "decisions_out".into(),
        Value::String(decisions.to_string_lossy().into_owned()),
    );
    let request: ldpc_survey::arm::Request = serde_json::from_value(serde_json::json!({
        "schema": "zen3-benchmark-arm-request-v1",
        "cell_id": "validation",
        "arm": spec.name,
        "role": "validation",
        "pair": 0,
        "case": case,
        "cache_state": "warm",
        "windows": 1u32,
        "window_target_ms": 1u32,
        "cpus": Vec::<u32>::new(),
        "workers_declared": 1u32,
    }))
    .map_err(|e| e.to_string())?;
    let encoded = serde_json::to_string(&request).map_err(|error| error.to_string())?;
    let mut command = Command::new(&spec.executable);
    command.env("GF2_TUNING_FRESH_CASE", "child-v2");
    for (name, value) in &spec.environment {
        command.env(name, value);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start arm {}: {error}", spec.name))?;
    child
        .stdin
        .take()
        .ok_or("the arm has no stdin")?
        .write_all(encoded.as_bytes())
        .map_err(|error| error.to_string())?;
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("arm {} exited with {}", spec.name, output.status));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| "arm stdout is not UTF-8")?;
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix(FRESH_RESULT_PREFIX))
        .ok_or_else(|| format!("arm {} emitted no result line", spec.name))?;
    let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let bits = fs::read(decisions).map_err(|error| {
        format!(
            "arm {} wrote no decisions to {}: {error}",
            spec.name,
            decisions.display()
        )
    })?;
    Ok((value, bits))
}

fn run() -> Result<(), String> {
    let mut plan_path = None;
    let mut output = None;
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        match flag.as_str() {
            "--plan" => plan_path = argv.next().map(PathBuf::from),
            "--output" => output = argv.next().map(PathBuf::from),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let plan_path = plan_path.ok_or("--plan is required")?;
    let output = output.ok_or("--output is required")?;
    let plan: Plan = serde_json::from_slice(&fs::read(&plan_path).map_err(|e| e.to_string())?)
        .map_err(|error| format!("plan does not decode: {error}"))?;
    if plan.schema != PLAN_SCHEMA {
        return Err(format!(
            "plan schema {:?} is not {PLAN_SCHEMA}",
            plan.schema
        ));
    }
    let scratch = PathBuf::from(&plan.scratch);
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;

    let mut groups = Vec::new();
    for group in &plan.groups {
        let bundle_dir = PathBuf::from(&group.bundle);
        let manifest = ldpc_survey::load_manifest(&bundle_dir).map_err(|e| e.to_string())?;
        ldpc_survey::verify_digests(&bundle_dir, &manifest).map_err(|e| e.to_string())?;
        let k = manifest.k;
        let mut reports = Vec::new();
        let mut decisions: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for spec in &group.arms {
            let path = scratch.join(format!("{}-{}.bits", group.id, spec.name));
            let (value, bits) = run_arm(spec, &group.bundle, plan.frames, &path)?;
            let expected = plan.frames as usize * k;
            if bits.len() != expected {
                return Err(format!(
                    "arm {} wrote {} decision bytes, expected {expected}",
                    spec.name,
                    bits.len()
                ));
            }
            reports.push(ArmReport {
                name: spec.name.clone(),
                executable: spec.executable.clone(),
                executable_sha256: ldpc_survey::sha256_file(Path::new(&spec.executable))
                    .map_err(|e| e.to_string())?,
                matched_algorithm: spec.matched_algorithm,
                environment: spec.environment.clone(),
                case: spec.case.clone(),
                selected_path: value
                    .get("selected_path")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                quality: value.get("quality").cloned().unwrap_or(Value::Null),
            });
            decisions.insert(spec.name.clone(), bits);
        }
        let names: Vec<&String> = decisions.keys().collect();
        let mut agreement = Vec::new();
        for left in 0..names.len() {
            for right in (left + 1)..names.len() {
                let a = &decisions[names[left]];
                let b = &decisions[names[right]];
                let mut identical = 0u64;
                let mut differing = 0u64;
                for frame in 0..plan.frames as usize {
                    let range = frame * k..(frame + 1) * k;
                    let frame_diff = a[range.clone()]
                        .iter()
                        .zip(&b[range])
                        .filter(|(x, y)| x != y)
                        .count() as u64;
                    differing += frame_diff;
                    identical += u64::from(frame_diff == 0);
                }
                let frames = u64::from(plan.frames);
                agreement.push(Agreement {
                    left: names[left].clone(),
                    right: names[right].clone(),
                    frames,
                    frames_identical: identical,
                    bits: frames * k as u64,
                    bits_differing: differing,
                    frame_agreement: identical as f64 / frames as f64,
                    frame_agreement_interval: ldpc_survey::arm::wilson_interval_95(
                        identical, frames,
                    ),
                });
            }
        }
        groups.push(GroupReport {
            id: group.id.clone(),
            bundle: group.bundle.clone(),
            matched_on: group.matched_on.clone(),
            manifest: serde_json::to_value(&manifest).map_err(|e| e.to_string())?,
            arms: reports,
            agreement,
        });
    }

    let report = Report {
        schema: REPORT_SCHEMA.to_owned(),
        frames: plan.frames,
        groups,
    };
    let mut encoded = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    encoded.push(b'\n');
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, &encoded).map_err(|e| e.to_string())?;
    println!("{}", output.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-validate: {error}");
            ExitCode::FAILURE
        }
    }
}
