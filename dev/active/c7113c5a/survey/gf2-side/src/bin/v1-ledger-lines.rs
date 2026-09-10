//! Derives the retrospective family-ledger lines of completed protocol-v1
//! campaigns (jit:c7113c5a).
//!
//! Usage: `v1-ledger-lines <family-id> <receipt-dir>...`
//!
//! Protocol v1 had no family ledger. Protocol v3 carries every earlier
//! reservation of the same family question forward, so this binary restates
//! each listed v1 campaign as the reservation the v3 runner would have
//! appended before measuring it: the receipt's campaign ID and addendum digest,
//! the count of non-exploratory cells, and the candidate identities computed by
//! the shared `trial_ledger::candidate_ids` from the saved plan and the
//! receipt's arm records. Lines are chained in argument order and written to
//! stdout exactly as the shared `Reservation` encoding serializes them; the
//! concatenation is decoded with the shared `trial_ledger::decode` before it is
//! printed.

use std::path::Path;
use std::process::ExitCode;
use tuning_campaign_support::protocol::{sha256_hex, CellRole, FamilyAddendum, RunnerPlan};
use tuning_campaign_support::receipt::BenchmarkReceipt;
use tuning_campaign_support::trial_ledger::{candidate_ids, decode, Reservation};

fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(dir.join(name)).map_err(|error| format!("{}: {error}", dir.join(name).display()))
}

fn reservation(
    family: &str,
    dir: &Path,
    sequence: u64,
    predecessor: String,
) -> Result<Reservation, String> {
    let receipt = BenchmarkReceipt::decode(&read(dir, "receipt.json")?)?;
    let plan = RunnerPlan::decode(&read(dir, "plan.json")?)?;
    let addendum_bytes = read(dir, "inputs/family-addendum.json")?;
    if sha256_hex(&addendum_bytes) != receipt.addendum.sha256 {
        return Err(format!("{}: addendum snapshot differs from the receipt pin", dir.display()));
    }
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    if plan.campaign_id != receipt.campaign_id {
        return Err(format!("{}: plan and receipt name different campaigns", dir.display()));
    }
    let comparisons = addendum
        .cells
        .iter()
        .filter(|cell| cell.role != CellRole::Exploratory)
        .count() as u32;
    let candidates = if comparisons == 0 {
        Vec::new()
    } else {
        candidate_ids(&addendum, &plan, &receipt.arms).map_err(|error| error.to_string())?
    };
    Ok(Reservation {
        sequence,
        predecessor,
        family: family.to_owned(),
        campaign: receipt.campaign_id,
        addendum_sha256: receipt.addendum.sha256,
        comparisons,
        protocol_version: addendum.protocol.version,
        candidates,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((family, dirs)) = args.split_first() else {
        eprintln!("usage: v1-ledger-lines <family-id> <receipt-dir>...");
        return ExitCode::from(2);
    };
    let mut ledger = Vec::new();
    let mut predecessor = "0".repeat(64);
    for (sequence, dir) in dirs.iter().enumerate() {
        match reservation(family, Path::new(dir), sequence as u64, predecessor.clone()) {
            Ok(entry) => {
                let mut line = serde_json::to_vec(&entry).expect("a reservation serializes");
                line.push(b'\n');
                predecessor = sha256_hex(&line);
                ledger.extend(line);
            }
            Err(error) => {
                eprintln!("v1-ledger-lines: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(error) = decode(&ledger, family) {
        eprintln!("v1-ledger-lines: derived chain does not decode: {error}");
        return ExitCode::FAILURE;
    }
    print!("{}", String::from_utf8(ledger).expect("JSON lines are UTF-8"));
    ExitCode::SUCCESS
}
