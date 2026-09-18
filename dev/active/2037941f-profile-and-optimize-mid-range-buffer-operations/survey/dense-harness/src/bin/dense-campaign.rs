//! Campaign-addendum transcription, verification and plan projection.
//!
//! ```text
//! dense-campaign pins
//! dense-campaign list    --family <id>
//! dense-campaign cells   --family <id> --issue <8-hex> --frozen-utc <t> --output <path>
//! dense-campaign verify  --family <id> --addendum <path>
//! dense-campaign plan    --family <id> --addendum <path> --campaign-id <id>
//!                        --campaign-seed <n> --lock <absolute> --gf2-executable <path>
//!                        --producing-manifest <path> --output <path>
//!                        [--candidate-executable <path>] [--scalar-executable <path>]
//!                        [--m4ri-executable <path>] [--label pilot|smoke]
//!                        [--max-cells-per-session <n>]
//! dense-campaign smoke   --plan <path> --addendum <path> --stage <dir>
//! dense-campaign inputs  --producing-manifest <path> [--also <path>]...
//! ```
//!
//! `verify` re-derives the transcription of the named family and compares it
//! byte for byte with a candidate campaign addendum, so a campaign JSON that
//! changes a cell, margin, limit or rule fails closed.
//!
//! `smoke` drives one non-timed session of a projected plan: it exits 0 when
//! the plan is complete and 3 when the plan's per-session cell budget pauses
//! it, mirroring the campaign runner's resumable-pause convention.
//!
//! `inputs` refuses unless every path of the producing-input closure, plus each
//! `--also` path, is tracked by git and identical to its committed content.

use dense_parity_harness::campaign::{self, PlanInputs};
use dense_parity_harness::cells::{family_cells, Question, UNAVAILABLE_ROWS};
use dense_parity_harness::{inputs, smoke};
use std::collections::BTreeMap;
use tuning_campaign_support::protocol::{FamilyAddendum, ReceiptLabel, RunnerPlan};

fn main() {
    if let Err(error) = run() {
        eprintln!("dense-campaign: {error}");
        std::process::exit(2);
    }
}

struct Arguments {
    flags: BTreeMap<String, String>,
    repeated: Vec<String>,
}

impl Arguments {
    /// `--also` is the one repeatable flag; every other flag is given once.
    const REPEATABLE: &'static str = "also";

    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut flags = BTreeMap::new();
        let mut repeated = Vec::new();
        let mut index = 0;
        while index < raw.len() {
            let name = raw[index]
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument {:?}", raw[index]))?
                .to_owned();
            let value = raw
                .get(index + 1)
                .ok_or_else(|| format!("--{name} needs a value"))?
                .clone();
            if name == Self::REPEATABLE {
                repeated.push(value);
            } else if flags.insert(name.clone(), value).is_some() {
                return Err(format!("--{name} is given twice"));
            }
            index += 2;
        }
        Ok(Self { flags, repeated })
    }

    fn repeated(&self) -> &[String] {
        &self.repeated
    }

    fn required(&self, name: &str) -> Result<&str, String> {
        self.flags
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("--{name} is required"))
    }

    fn optional(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(String::as_str)
    }

    fn number(&self, name: &str) -> Result<Option<u64>, String> {
        match self.flags.get(name) {
            None => Ok(None),
            Some(value) => value
                .parse()
                .map(Some)
                .map_err(|error| format!("--{name} is not a number: {error}")),
        }
    }
}

fn question(arguments: &Arguments) -> Result<Question, String> {
    let family = arguments.required("family")?;
    Question::from_family_id(family).ok_or_else(|| {
        format!(
            "{family:?} is not a frozen family; the three are {}",
            Question::ALL
                .iter()
                .map(|question| question.family_id())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// Canonical serialization of a campaign addendum: pretty JSON, one newline.
fn render(addendum: &FamilyAddendum) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(addendum)
        .map_err(|error| format!("cannot encode the addendum: {error}"))?;
    text.push('\n');
    Ok(text)
}

fn transcribe(arguments: &Arguments) -> Result<FamilyAddendum, String> {
    let addendum = campaign::addendum(
        question(arguments)?,
        arguments.required("issue")?,
        arguments.required("frozen-utc")?,
    );
    addendum
        .validate()
        .map_err(|errors| format!("the transcription is invalid: {}", errors.join("; ")))?;
    Ok(addendum)
}

fn run() -> Result<(), String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = raw.split_first().ok_or(
        "usage: dense-campaign <pins|list|cells|verify|plan|smoke|inputs> --flag value ...",
    )?;
    let arguments = Arguments::parse(rest)?;
    match command.as_str() {
        "pins" => pins(),
        "list" => {
            for cell in family_cells(question(&arguments)?) {
                println!(
                    "{:3} {} cache={} objective={} reference={} seed={:#018x}",
                    cell.ordinal,
                    cell.cell_id,
                    cell.cache.id(),
                    cell.objective(),
                    cell.scalar_reference,
                    cell.seed
                );
            }
            Ok(())
        }
        "cells" => {
            let addendum = transcribe(&arguments)?;
            let output = arguments.required("output")?;
            std::fs::write(output, render(&addendum)?)
                .map_err(|error| format!("cannot write {output}: {error}"))?;
            println!("{output}: {} cells", addendum.cells.len());
            Ok(())
        }
        "verify" => verify(&arguments),
        "plan" => project(&arguments),
        "smoke" => {
            let stage = std::path::Path::new(arguments.required("stage")?);
            let plan_path = arguments.required("plan")?;
            let addendum_path = arguments.required("addendum")?;
            let plan_bytes = std::fs::read(plan_path)
                .map_err(|error| format!("cannot read {plan_path}: {error}"))?;
            let plan: RunnerPlan = serde_json::from_slice(&plan_bytes)
                .map_err(|error| format!("{plan_path} does not decode: {error}"))?;
            let addendum_bytes = std::fs::read(addendum_path)
                .map_err(|error| format!("cannot read {addendum_path}: {error}"))?;
            let addendum = FamilyAddendum::decode(&addendum_bytes)?;
            addendum
                .validate()
                .map_err(|errors| format!("{addendum_path} is invalid: {}", errors.join("; ")))?;
            let outcome = smoke::session(
                &repository_root()?,
                stage,
                &plan,
                &plan_bytes,
                &addendum,
                &addendum_bytes,
            )
            .map_err(|error| format!("the non-timed smoke failed: {error}"))?;
            println!(
                "{}: {:?} after {} handshake cells, zero timing samples",
                plan.campaign_id, outcome.state, outcome.completed
            );
            std::process::exit(outcome.exit_code());
        }
        "inputs" => {
            let manifest = arguments.required("producing-manifest")?;
            let checked = inputs::check(&repository_root()?, manifest, arguments.repeated())?;
            println!("{manifest}: {} campaign inputs committed and clean", checked.len());
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

/// Prints the frozen input identities a timed run refuses to start without.
fn pins() -> Result<(), String> {
    let path = dense_parity_harness::cells::ADDENDUM_PATH;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read {path} from the repository root: {error}"))?;
    let observed = tuning_campaign_support::protocol::sha256_hex(&bytes);
    if observed != dense_parity_harness::cells::ADDENDUM_SHA256 {
        return Err(format!(
            "{path} sha256 {observed} differs from the pinned {}",
            dense_parity_harness::cells::ADDENDUM_SHA256
        ));
    }
    println!("addendum={path}");
    println!("addendum_sha256={observed}");
    println!("addendum_identity={}", dense_parity_harness::cells::ADDENDUM_IDENTITY);
    println!("addendum_frozen_utc={}", dense_parity_harness::cells::ADDENDUM_FROZEN_UTC);
    println!("comparator={}", dense_parity_harness::cells::COMPARATOR_PATH);
    for question in Question::ALL {
        println!(
            "family={} ledger={} cells={}",
            question.family_id(),
            question.ledger_path(),
            family_cells(question).len()
        );
    }
    for row in UNAVAILABLE_ROWS {
        println!(
            "unavailable_row={} stride={}w reason={}",
            row.row_id, row.stride_words, row.reason
        );
    }
    Ok(())
}

/// Re-derives a family's transcription and compares it with a candidate JSON.
fn verify(arguments: &Arguments) -> Result<(), String> {
    let question = question(arguments)?;
    let path = arguments.required("addendum")?;
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let candidate = FamilyAddendum::decode(&bytes)?;
    candidate
        .validate()
        .map_err(|errors| format!("{path} is invalid: {}", errors.join("; ")))?;
    if candidate.family.id != question.family_id() {
        return Err(format!(
            "{path} declares family {:?} rather than {:?}",
            candidate.family.id,
            question.family_id()
        ));
    }
    let frozen = candidate
        .frozen
        .frozen_utc
        .clone()
        .ok_or("the campaign addendum declares no freeze time")?;
    let derived = campaign::addendum(question, &candidate.family.issue, &frozen);
    if derived != candidate {
        return Err(format!("{path} is not the harness transcription of the frozen addendum"));
    }
    if render(&derived)? != String::from_utf8_lossy(&bytes) {
        return Err(format!("{path} is not canonically rendered"));
    }
    println!(
        "{path}: transcribes {} with {} cells from {} ({})",
        question.family_id(),
        candidate.cells.len(),
        dense_parity_harness::cells::ADDENDUM_PATH,
        dense_parity_harness::cells::ADDENDUM_IDENTITY
    );
    Ok(())
}

/// Projects and writes the runner plan of one campaign addendum.
fn project(arguments: &Arguments) -> Result<(), String> {
    let question = question(arguments)?;
    let path = arguments.required("addendum")?;
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let addendum = FamilyAddendum::decode(&bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("{path} is invalid: {}", errors.join("; ")))?;
    let label = match arguments.optional("label").unwrap_or("pilot") {
        "pilot" => ReceiptLabel::Pilot,
        "smoke" => ReceiptLabel::Smoke,
        other => return Err(format!("label {other:?} is not pilot or smoke")),
    };
    let plan = campaign::plan(
        question,
        &addendum,
        &PlanInputs {
            campaign_id: arguments.required("campaign-id")?,
            campaign_seed: arguments
                .number("campaign-seed")?
                .ok_or("--campaign-seed is required")?,
            label,
            addendum_path: path,
            producing_manifest: arguments.required("producing-manifest")?,
            lock_path: arguments.required("lock")?,
            gf2_executable: arguments.required("gf2-executable")?,
            candidate_executable: arguments.optional("candidate-executable"),
            scalar_executable: arguments.optional("scalar-executable"),
            m4ri_executable: arguments.optional("m4ri-executable"),
            max_cells_per_session: arguments
                .number("max-cells-per-session")?
                .map(|value| value as u32),
        },
    )?;
    plan.validate(&addendum)
        .map_err(|errors| format!("the projected plan is invalid: {}", errors.join("; ")))?;
    let output = arguments.required("output")?;
    let mut text = serde_json::to_string_pretty(&plan)
        .map_err(|error| format!("cannot encode the plan: {error}"))?;
    text.push('\n');
    std::fs::write(output, &text).map_err(|error| format!("cannot write {output}: {error}"))?;
    println!("{output}: {} cells, {} arms", plan.cells.len(), plan.arms.len());
    Ok(())
}

/// The repository root every repository-relative campaign path resolves against.
fn repository_root() -> Result<std::path::PathBuf, String> {
    std::fs::canonicalize(".")
        .map_err(|error| format!("cannot resolve the working directory: {error}"))
}
