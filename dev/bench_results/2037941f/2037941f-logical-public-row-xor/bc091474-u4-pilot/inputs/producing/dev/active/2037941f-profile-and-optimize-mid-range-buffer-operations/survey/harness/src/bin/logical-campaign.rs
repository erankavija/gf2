//! Campaign-addendum transcription, verification and plan projection.
//!
//! ```text
//! logical-campaign pins
//! logical-campaign list    --family <id>
//! logical-campaign cells   --family <id> --issue <8-hex> --frozen-utc <t> --output <path>
//! logical-campaign verify  --family <id> --addendum <path>
//! logical-campaign plan    --family <id> --addendum <path> --campaign-id <id>
//!                          --campaign-seed <n> --lock <absolute> --gf2-executable <path>
//!                          --producing-manifest <path> --output <path>
//!                          [--isal-executable <path>] [--label pilot|smoke]
//!                          [--max-cells-per-session <n>] [--pilot-pairs <n>]
//!                          [--isolated-candidate <route>]
//!                          [--candidate-gf2-executable <path>]
//!                          [--candidate-gf2-rustflags <flags>]
//! logical-campaign inputs  --producing-manifest <path> [--also <path>]...
//! ```
//!
//! `verify` re-derives the transcription of the named family and compares it
//! byte for byte with a candidate campaign addendum, so a campaign JSON that
//! changes a cell, margin, limit or rule fails closed.
//!
//! `inputs` refuses unless every path of the producing-input closure, plus each
//! `--also` path, is tracked by git and identical to its committed content.

use logical_buffer_harness::cells::{family_cells, Question};
use logical_buffer_harness::routes::Route;
use logical_buffer_harness::{campaign, inputs};
use std::collections::BTreeMap;
use tuning_campaign_support::protocol::{FamilyAddendum, ReceiptLabel};

fn main() {
    if let Err(error) = run() {
        eprintln!("logical-campaign: {error}");
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
            "{family:?} is not a frozen family; the four are {}",
            Question::ALL
                .iter()
                .map(|q| q.family_id())
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

fn transcribe(arguments: &Arguments) -> Result<(Question, FamilyAddendum), String> {
    let question = question(arguments)?;
    let issue = arguments.required("issue")?;
    let frozen = arguments.required("frozen-utc")?;
    let addendum = campaign::addendum(question, issue, frozen);
    addendum
        .validate()
        .map_err(|errors| format!("the transcription is invalid: {}", errors.join("; ")))?;
    Ok((question, addendum))
}

fn run() -> Result<(), String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = raw
        .split_first()
        .ok_or("usage: logical-campaign <pins|list|cells|verify|plan|inputs> --flag value ...")?;
    let arguments = Arguments::parse(rest)?;
    match command.as_str() {
        "pins" => {
            // The frozen prose addendum is an input identity, not a setting:
            // a timed run refuses to start when its bytes differ from the pin
            // this harness carries.
            let path = logical_buffer_harness::cells::ADDENDUM_PATH;
            let bytes = std::fs::read(path)
                .map_err(|error| format!("cannot read {path} from the repository root: {error}"))?;
            let observed = tuning_campaign_support::protocol::sha256_hex(&bytes);
            if observed != logical_buffer_harness::cells::ADDENDUM_SHA256 {
                return Err(format!(
                    "{path} sha256 {observed} differs from the pinned {}",
                    logical_buffer_harness::cells::ADDENDUM_SHA256
                ));
            }
            println!("addendum={path}");
            println!("addendum_sha256={observed}");
            println!(
                "addendum_identity={}",
                logical_buffer_harness::cells::ADDENDUM_IDENTITY
            );
            println!(
                "addendum_frozen_utc={}",
                logical_buffer_harness::cells::ADDENDUM_FROZEN_UTC
            );
            println!(
                "comparator={}",
                logical_buffer_harness::cells::COMPARATOR_PATH
            );
            for question in Question::ALL {
                println!(
                    "family={} ledger={} cells={}",
                    question.family_id(),
                    question.ledger_path(),
                    family_cells(question).len()
                );
            }
            Ok(())
        }
        "list" => {
            let question = question(&arguments)?;
            for cell in family_cells(question) {
                println!(
                    "{:3} {} cache={} objective={} seed={:#018x}",
                    cell.ordinal,
                    cell.cell_id,
                    cell.cache.id(),
                    cell.objective(),
                    cell.seed
                );
            }
            Ok(())
        }
        "cells" => {
            let (_, addendum) = transcribe(&arguments)?;
            let output = arguments.required("output")?;
            let text = render(&addendum)?;
            std::fs::write(output, &text)
                .map_err(|error| format!("cannot write {output}: {error}"))?;
            println!("{output}: {} cells", addendum.cells.len());
            Ok(())
        }
        "verify" => {
            let question = question(&arguments)?;
            let path = arguments.required("addendum")?;
            let bytes =
                std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
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
                return Err(format!(
                    "{path} is not the harness transcription of the frozen addendum"
                ));
            }
            if render(&derived)? != String::from_utf8_lossy(&bytes) {
                return Err(format!("{path} is not canonically rendered"));
            }
            println!(
                "{path}: transcribes {} with {} cells from {} ({})",
                question.family_id(),
                candidate.cells.len(),
                logical_buffer_harness::cells::ADDENDUM_PATH,
                logical_buffer_harness::cells::ADDENDUM_IDENTITY
            );
            Ok(())
        }
        "plan" => {
            let question = question(&arguments)?;
            let path = arguments.required("addendum")?;
            let bytes =
                std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
            let addendum = FamilyAddendum::decode(&bytes)?;
            addendum
                .validate()
                .map_err(|errors| format!("{path} is invalid: {}", errors.join("; ")))?;
            let label = match arguments.optional("label").unwrap_or("pilot") {
                "pilot" => ReceiptLabel::Pilot,
                "smoke" => ReceiptLabel::Smoke,
                other => return Err(format!("label {other:?} is not pilot or smoke")),
            };
            let candidate = match arguments.optional("isolated-candidate") {
                None => Route::PublicXorB,
                Some(name) => Route::parse(name)?,
            };
            let seed = arguments
                .number("campaign-seed")?
                .ok_or("--campaign-seed is required")?;
            let candidate_gf2 = match (
                arguments.optional("candidate-gf2-executable"),
                arguments.optional("candidate-gf2-rustflags"),
            ) {
                (None, None) => None,
                (Some(executable), Some(rustflags)) => Some((executable, rustflags)),
                _ => {
                    return Err(
                        "candidate gf2 executable and rustflags must be given together".to_owned(),
                    )
                }
            };
            let plan = campaign::plan(
                question,
                &addendum,
                arguments.required("campaign-id")?,
                seed,
                label,
                path,
                arguments.required("producing-manifest")?,
                arguments.required("lock")?,
                arguments.required("gf2-executable")?,
                candidate_gf2,
                arguments.optional("isal-executable"),
                candidate,
                arguments
                    .number("max-cells-per-session")?
                    .map(|value| value as u32),
                arguments.number("pilot-pairs")?.map(|value| value as u32),
            )?;
            plan.validate(&addendum).map_err(|errors| {
                format!("the projected plan is invalid: {}", errors.join("; "))
            })?;
            let output = arguments.required("output")?;
            let mut text = serde_json::to_string_pretty(&plan)
                .map_err(|error| format!("cannot encode the plan: {error}"))?;
            text.push('\n');
            std::fs::write(output, &text)
                .map_err(|error| format!("cannot write {output}: {error}"))?;
            println!(
                "{output}: {} cells, {} arms",
                plan.cells.len(),
                plan.arms.len()
            );
            Ok(())
        }
        "inputs" => {
            let manifest = arguments.required("producing-manifest")?;
            let checked = inputs::check(&repository_root()?, manifest, arguments.repeated())?;
            println!(
                "{manifest}: {} campaign inputs committed and clean",
                checked.len()
            );
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

/// The repository root every repository-relative campaign path resolves against.
fn repository_root() -> Result<std::path::PathBuf, String> {
    std::fs::canonicalize(".")
        .map_err(|error| format!("cannot resolve the working directory: {error}"))
}
