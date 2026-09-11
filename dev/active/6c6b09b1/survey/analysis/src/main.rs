//! Post-processing for the byte-field survey (jit:6c6b09b1).
//!
//! Usage:
//!
//! ```text
//! survey-analysis ledger-genesis FAMILY LEDGER V1_RECEIPT_DIR
//! survey-analysis ledger-check FAMILY LEDGER
//! survey-analysis plan-check ADDENDUM [PLAN]
//! survey-analysis tables OUTPUT RECEIPT_DIR...
//! survey-analysis resolution PILOT_RECEIPT_DIR [STRICTER_ALPHA]
//! ```
//!
//! `ledger-genesis` creates a protocol-v3 family ledger whose only line
//! restates the completed protocol-v1 exploratory pilot of this issue as a
//! zero-comparison reservation, encoded with the shared
//! `trial_ledger::Reservation` and decoded again before it is kept. The line
//! is retrospective accounting: protocol v1 took no reservations. The v1
//! pilot pooled every operation of this survey in one exploratory family, so
//! the same line opens each v3 family ledger.
//!
//! `ledger-check` decodes a ledger with the shared chain rules and prints
//! each reservation and the comparison count the chain implies.
//!
//! `plan-check` decodes a family addendum with the shared strict decoder and
//! runs its semantic validation, then does the same for a runner plan
//! against that addendum when one is given, so a frozen addendum or a
//! projected plan that the runner would refuse fails here, before anything
//! is queued for a benchmark window.
//!
//! `tables` writes the generated tables of the given receipt directories
//! (see [`tables`] for what they contain and how every interval is formed).
//! `resolution` prints the relative bootstrap half-width of every cell of a
//! pilot receipt and the widest one, the input of a confirmation's frozen
//! measurement resolution.

mod tables;

use std::fs;
use std::path::Path;
use std::process::ExitCode;
use tuning_campaign_support::protocol::{sha256_hex, FamilyAddendum, RunnerPlan};
use tuning_campaign_support::receipt::BenchmarkReceipt;
use tuning_campaign_support::trial_ledger::{self, Reservation};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["ledger-genesis", family, ledger, receipt_dir] => {
            ledger_genesis(family, ledger, receipt_dir)
        }
        ["ledger-check", family, ledger] => ledger_check(family, ledger),
        ["plan-check", addendum] => plan_check(addendum, None),
        ["plan-check", addendum, plan] => plan_check(addendum, Some(plan)),
        ["tables", output, receipts @ ..] if !receipts.is_empty() => {
            tables::tables(output, receipts)
        }
        ["resolution", pilot] => tables::resolution(pilot, None),
        ["resolution", pilot, alpha] => alpha
            .parse::<f64>()
            .map_err(|e| format!("alpha {alpha}: {e}"))
            .and_then(|alpha| tables::resolution(pilot, Some(alpha))),
        _ => Err(
            "usage: survey-analysis ledger-genesis FAMILY LEDGER V1_RECEIPT_DIR | \
                  ledger-check FAMILY LEDGER | plan-check ADDENDUM [PLAN] | \
                  tables OUTPUT RECEIPT_DIR... | resolution PILOT_RECEIPT_DIR [ALPHA]"
                .to_owned(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("survey-analysis: {message}");
            ExitCode::FAILURE
        }
    }
}

fn ledger_genesis(family: &str, ledger: &str, receipt_dir: &str) -> Result<(), String> {
    if Path::new(ledger).exists() {
        return Err(format!("{ledger} exists; a family ledger is created once"));
    }
    let dir = Path::new(receipt_dir);
    let bytes = fs::read(dir.join("receipt.json")).map_err(|e| format!("receipt: {e}"))?;
    let receipt = BenchmarkReceipt::decode(&bytes)?;
    let addendum = fs::read(dir.join(&receipt.addendum.snapshot))
        .map_err(|e| format!("addendum snapshot: {e}"))?;
    if sha256_hex(&addendum) != receipt.addendum.sha256 {
        return Err("the v1 addendum snapshot differs from its receipt pin".to_owned());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&addendum).map_err(|e| format!("addendum: {e}"))?;
    let version = value["protocol"]["version"]
        .as_u64()
        .ok_or("addendum protocol version")?;
    let confirmatory = value["cells"]
        .as_array()
        .ok_or("addendum cells")?
        .iter()
        .filter(|cell| cell["role"] != "exploratory")
        .count();
    if confirmatory != 0 {
        return Err(format!(
            "the v1 receipt declares {confirmatory} non-exploratory cells; this genesis \
             imports only a zero-comparison pilot"
        ));
    }
    let entry = Reservation {
        sequence: 0,
        predecessor: "0".repeat(64),
        family: family.to_owned(),
        campaign: receipt.campaign_id.clone(),
        addendum_sha256: receipt.addendum.sha256.clone(),
        comparisons: 0,
        protocol_version: u32::try_from(version).map_err(|e| e.to_string())?,
        candidates: Vec::new(),
    };
    let mut line = serde_json::to_vec(&entry).map_err(|e| e.to_string())?;
    line.push(b'\n');
    trial_ledger::decode(&line, family).map_err(|e| format!("genesis line: {e}"))?;
    fs::write(ledger, &line).map_err(|e| format!("{ledger}: {e}"))?;
    println!("ledger    {ledger}");
    println!("line      {}", String::from_utf8_lossy(&line).trim_end());
    println!("sha256    {}", sha256_hex(&line));
    Ok(())
}

fn ledger_check(family: &str, ledger: &str) -> Result<(), String> {
    let bytes = fs::read(ledger).map_err(|e| format!("{ledger}: {e}"))?;
    let entries = trial_ledger::decode(&bytes, family).map_err(|e| e.to_string())?;
    for entry in &entries {
        println!(
            "{:>3} {} protocol-v{} comparisons={} candidates={}",
            entry.sequence,
            entry.campaign,
            entry.protocol_version,
            entry.comparisons,
            entry.candidates.len()
        );
    }
    let attempts = entries.iter().filter(|entry| entry.comparisons > 0).count();
    println!(
        "family {family}: {} reservations, m = {}, confirmatory attempts so far = {attempts}",
        entries.len(),
        trial_ledger::comparisons(&entries).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn plan_check(addendum_path: &str, plan_path: Option<&str>) -> Result<(), String> {
    let bytes = fs::read(addendum_path).map_err(|e| format!("{addendum_path}: {e}"))?;
    let addendum = FamilyAddendum::decode(&bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("{addendum_path}: {}", errors.join("; ")))?;
    println!(
        "addendum  {addendum_path} sha256 {} family {} cells {}",
        sha256_hex(&bytes),
        addendum.family.id,
        addendum.cells.len()
    );
    if let Some(plan_path) = plan_path {
        let bytes = fs::read(plan_path).map_err(|e| format!("{plan_path}: {e}"))?;
        let plan = RunnerPlan::decode(&bytes)?;
        plan.validate(&addendum)
            .map_err(|errors| format!("{plan_path}: {}", errors.join("; ")))?;
        if plan.addendum != addendum_path {
            return Err(format!(
                "{plan_path} names addendum {}, not {addendum_path}",
                plan.addendum
            ));
        }
        println!(
            "plan      {plan_path} sha256 {} campaign {} cells {} arms {}",
            sha256_hex(&bytes),
            plan.campaign_id,
            plan.cells.len(),
            plan.arms.len()
        );
    }
    Ok(())
}
