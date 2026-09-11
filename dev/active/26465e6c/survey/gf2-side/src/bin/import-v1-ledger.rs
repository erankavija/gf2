//! Opens the protocol-v3 family ledgers of jit:26465e6c.
//!
//! The protocol-v1 confirmation `popcount-26465e6c-20260908` reserved nine
//! confirmatory comparisons of the `popcount-baselines` question. Its v3
//! ledger therefore opens with that reservation, computed by the runner's own
//! `trial_ledger::candidate_ids` from the committed receipt, saved plan and
//! receipt-local addendum snapshot. The `and-popcnt-baselines` question was
//! never confirmed, so its ledger opens empty. The derivation record states
//! both, with every source digest.
//!
//! Usage: `import-v1-ledger` from the repository root. It refuses to touch a
//! ledger that already exists, so a chain in use is never rewritten.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use tuning_campaign_support::protocol::{sha256_hex, CellRole, FamilyAddendum, RunnerPlan};
use tuning_campaign_support::receipt::BenchmarkReceipt;
use tuning_campaign_support::trial_ledger::{self, Reservation};

const RESULTS: &str = "dev/bench_results/26465e6c";
const V1_CONFIRMATION: &str = "dev/bench_results/26465e6c/2026-09-08-26465e6c-popcount";
const POPCOUNT_LEDGER: &str = "dev/bench_results/26465e6c/v3-popcount-family-ledger.jsonl";
const AND_LEDGER: &str = "dev/bench_results/26465e6c/v3-and-popcnt-family-ledger.jsonl";
const RECORD: &str = "dev/active/26465e6c/ledger-genesis.json";
const AND_V1_ADDENDUM: &str = "dev/active/26465e6c/superseded/v1/addendum-and-popcnt.json";

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))
}

fn import() -> Result<Value, String> {
    let receipt_path = format!("{V1_CONFIRMATION}/receipt.json");
    let plan_path = format!("{V1_CONFIRMATION}/plan.json");
    let addendum_path = format!("{V1_CONFIRMATION}/inputs/family-addendum.json");
    let receipt_bytes = read(&receipt_path)?;
    let plan_bytes = read(&plan_path)?;
    let addendum_bytes = read(&addendum_path)?;
    let receipt = BenchmarkReceipt::decode(&receipt_bytes)?;
    let plan = RunnerPlan::decode(&plan_bytes)?;
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    let addendum_sha = sha256_hex(&addendum_bytes);
    if receipt.addendum.sha256 != addendum_sha {
        return Err("the v1 receipt does not pin its own addendum snapshot".into());
    }
    if plan.campaign_id != receipt.campaign_id {
        return Err("the saved plan belongs to another campaign".into());
    }
    let candidates = trial_ledger::candidate_ids(&addendum, &plan, &receipt.arms)
        .map_err(|error| error.to_string())?;
    let mut ids_by_arm = BTreeMap::new();
    for cell in &plan.cells {
        if addendum
            .cell(&cell.cell_id)
            .is_some_and(|declared| declared.role != CellRole::Exploratory)
        {
            let single = BTreeMap::from([(
                cell.candidate_arm.clone(),
                receipt.arms[&cell.candidate_arm].clone(),
            )]);
            let lone = RunnerPlan {
                cells: vec![cell.clone()],
                ..plan.clone()
            };
            let id = trial_ledger::candidate_ids(&addendum, &lone, &single)
                .map_err(|error| error.to_string())?;
            ids_by_arm.insert(cell.candidate_arm.clone(), id[0].clone());
        }
    }
    let comparisons = addendum
        .cells
        .iter()
        .filter(|cell| cell.role != CellRole::Exploratory)
        .count() as u32;
    let entry = Reservation {
        sequence: 0,
        predecessor: "0".repeat(64),
        family: addendum.family.id.clone(),
        campaign: receipt.campaign_id.clone(),
        addendum_sha256: addendum_sha.clone(),
        comparisons,
        protocol_version: addendum.protocol.version,
        candidates,
    };
    let mut line = serde_json::to_vec(&entry).map_err(|error| error.to_string())?;
    line.push(b'\n');
    trial_ledger::decode(&line, &addendum.family.id).map_err(|error| error.to_string())?;
    write_new(POPCOUNT_LEDGER, &line)?;
    write_new(AND_LEDGER, b"")?;
    let receipts: Vec<String> = std::fs::read_dir(RESULTS)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("receipt.json").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(json!({
        "schema": "popcount-v3-ledger-genesis-v1",
        "issue": "26465e6c",
        "purpose": "Carry the protocol-v1 confirmatory reservation of the popcount-baselines question into its v3 ledger, and open the never-confirmed and-popcnt-baselines ledger empty. The imported line is retrospective accounting, not a premeasurement reservation.",
        "popcount_baselines": {
            "ledger_path": POPCOUNT_LEDGER,
            "line_sha256": sha256_hex(&line),
            "source": {
                "receipt": receipt_path,
                "receipt_sha256": sha256_hex(&receipt_bytes),
                "plan": plan_path,
                "plan_sha256": sha256_hex(&plan_bytes),
                "addendum_snapshot": addendum_path,
                "addendum_sha256": addendum_sha,
            },
            "source_fields": {
                "campaign": "receipt.campaign_id",
                "family": "decoded receipt-local addendum family.id",
                "protocol_version": "decoded receipt-local addendum protocol.version",
                "comparisons": "count of decoded addendum cells whose role is not exploratory",
                "candidates": "tuning_campaign_support::trial_ledger::candidate_ids over the addendum, saved plan and receipt.arms",
            },
            "candidate_ids_by_arm": ids_by_arm,
            "entry": entry,
        },
        "and_popcnt_baselines": {
            "ledger_path": AND_LEDGER,
            "genesis": "empty",
            "unrun_v1_confirmation_addendum": {
                "path": AND_V1_ADDENDUM,
                "sha256": sha256_hex(&read(AND_V1_ADDENDUM)?),
            },
            "reason": "The protocol-v1 confirmatory addendum was frozen but never run: no receipt directory, execution log or checkpoint exists for it, so it reserved nothing. The v1 pilot 2026-09-08-26465e6c-and-popcnt-pilot is exploratory and spends zero comparisons.",
        },
        "prior_campaign_search": {
            "receipt_directories": receipts,
            "confirmations": [V1_CONFIRMATION],
        },
    }))
}

fn write_new(path: &str, bytes: &[u8]) -> Result<(), String> {
    if Path::new(path).exists() {
        return Err(format!(
            "{path} already exists; refusing to rewrite a ledger"
        ));
    }
    std::fs::write(path, bytes).map_err(|error| format!("cannot write {path}: {error}"))
}

fn main() -> ExitCode {
    match import() {
        Ok(record) => {
            let mut text = serde_json::to_string_pretty(&record).expect("record encodes");
            text.push('\n');
            if let Err(error) = std::fs::write(RECORD, text) {
                eprintln!("import-v1-ledger: cannot write {RECORD}: {error}");
                return ExitCode::FAILURE;
            }
            println!("{POPCOUNT_LEDGER}: imported v1 reservation; {AND_LEDGER}: empty genesis");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("import-v1-ledger: {error}");
            ExitCode::FAILURE
        }
    }
}
