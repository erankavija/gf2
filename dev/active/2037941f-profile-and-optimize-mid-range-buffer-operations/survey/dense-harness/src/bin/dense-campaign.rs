//! Campaign-addendum transcription, verification and plan projection.
//!
//! ```text
//! dense-campaign pins
//! dense-campaign list    --family <id>
//! dense-campaign cells   --family <id> --issue <8-hex> --frozen-utc <t> --output <path>
//! dense-campaign verify  --family <id> --addendum <path>
//! dense-campaign verify-confirmation --family <id> --addendum <path>
//! dense-campaign unavailable --family <id> --addendum <path> --output <path>
//! dense-campaign plan    --family <id> --addendum <path> --campaign-id <id>
//!                        --campaign-seed <n> --lock <absolute> --gf2-executable <path>
//!                        --producing-manifest <path> --output <path>
//!                        [--candidate-executable <path>] [--scalar-executable <path>]
//!                        [--m4ri-executable <path>] [--label pilot|smoke|confirmation]
//!                        [--max-cells-per-session <n>]
//! dense-campaign inputs  --producing-manifest <path> [--also <path>]...
//! dense-campaign profile-request --family <id> --addendum <path>
//!                        --cell <id> --arm <name> --cpu <n>
//! ```
//!
//! `verify` re-derives the transcription of the named family and compares it
//! byte for byte with a candidate campaign addendum, so a campaign JSON that
//! changes a cell, margin, limit or rule fails closed. `verify-confirmation`
//! holds a freezer-derived confirmation addendum to the same transcription and
//! to the family's resolution ceiling.
//!
//! `inputs` refuses unless every path of the producing-input closure, plus each
//! `--also` path, is tracked by git and identical to its committed content.

use dense_parity_harness::campaign::{self, PlanInputs};
use dense_parity_harness::cells::{family_cells, Question, UNAVAILABLE_ROWS};
use dense_parity_harness::inputs;
use dense_parity_harness::wire::Case;
use std::collections::BTreeMap;
use tuning_campaign_support::arm::{ArmRequest, PairPosition};
use tuning_campaign_support::protocol::{
    CellRole, FamilyAddendum, PlanCell, ReceiptLabel, SHARED_SETTINGS,
};
use tuning_campaign_support::transport;

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
        "usage: dense-campaign <pins|list|cells|verify|verify-confirmation|unavailable|plan|inputs|profile-request> --flag value ...",
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
        "verify-confirmation" => verify_confirmation(&arguments),
        "unavailable" => unavailable(&arguments),
        "plan" => project(&arguments),
        "profile-request" => profile_request(&arguments),
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

/// Verifies the frozen prose bytes both projections cite.
fn pinned_addendum_digest() -> Result<String, String> {
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
    Ok(observed)
}

/// Prints the frozen input identities a timed run refuses to start without.
fn pins() -> Result<(), String> {
    let path = dense_parity_harness::cells::ADDENDUM_PATH;
    let observed = pinned_addendum_digest()?;
    println!("addendum={path}");
    println!("addendum_sha256={observed}");
    println!(
        "addendum_identity={}",
        dense_parity_harness::cells::ADDENDUM_IDENTITY
    );
    println!(
        "addendum_frozen_utc={}",
        dense_parity_harness::cells::ADDENDUM_FROZEN_UTC
    );
    println!(
        "comparator={}",
        dense_parity_harness::cells::COMPARATOR_PATH
    );
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

/// Projects the comparator shapes the frozen qualification does not admit.
fn unavailable(arguments: &Arguments) -> Result<(), String> {
    let (question, addendum) = family_addendum(arguments)?;
    if question != Question::MatvecVsM4ri {
        return Err("unavailable rows belong only to the M4RI comparator".into());
    }
    for row in UNAVAILABLE_ROWS {
        if addendum.cell(row.row_id).is_some() {
            return Err(format!("{} is also a measured cell", row.row_id));
        }
    }
    let digest = pinned_addendum_digest()?;
    let mut table = format!(
        "# addendum={} sha256={}\n# family={}\ncell_id\tstride_words\tstatus\tsamples\tcomparisons\treason\n",
        dense_parity_harness::cells::ADDENDUM_PATH,
        digest,
        question.family_id(),
    );
    for row in UNAVAILABLE_ROWS {
        if row.reason.contains('\t') || row.reason.contains('\n') {
            return Err(format!("{} has an invalid unavailable reason", row.row_id));
        }
        table.push_str(&format!(
            "{}\t{}\tunavailable\t0\t0\t{}\n",
            row.row_id, row.stride_words, row.reason
        ));
    }
    let output = arguments.required("output")?;
    std::fs::write(output, table).map_err(|error| format!("cannot write {output}: {error}"))?;
    println!("{output}: unavailable M4RI shapes from the frozen addendum");
    Ok(())
}

/// Decodes a schema-valid family addendum, including a derived confirmation.
fn family_addendum(arguments: &Arguments) -> Result<(Question, FamilyAddendum), String> {
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
    Ok((question, candidate))
}

/// Re-derives a pilot transcription and compares it with committed JSON.
fn verified_addendum(arguments: &Arguments) -> Result<(Question, FamilyAddendum), String> {
    let (question, candidate) = family_addendum(arguments)?;
    let path = arguments.required("addendum")?;
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
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
    Ok((question, candidate))
}

fn verify(arguments: &Arguments) -> Result<(), String> {
    let (question, candidate) = verified_addendum(arguments)?;
    let path = arguments.required("addendum")?;
    println!(
        "{path}: transcribes {} with {} cells from {} ({})",
        question.family_id(),
        candidate.cells.len(),
        dense_parity_harness::cells::ADDENDUM_PATH,
        dense_parity_harness::cells::ADDENDUM_IDENTITY
    );
    Ok(())
}

/// Largest pilot-derived resolution the frozen addendum lets a family confirm
/// at (§ Effect, resolution, and complexity rules).
fn resolution_ceiling(question: Question) -> f64 {
    match question {
        Question::IsolatedFusedParity => 0.020,
        Question::AllocatedMatvec => 0.035,
        Question::MatvecVsM4ri => 0.040,
    }
}

/// Checks that `candidate` is the confirmation the frozen addendum admits.
///
/// The canonical freezer restates the family description and freeze time, pins
/// the pilot-derived resolution with its evidence, and gives the family's
/// confirmatory cells the confirmatory role. Every other field must equal the
/// pilot transcription, and a resolution above the family ceiling is the
/// addendum's `resolution-insufficient` outcome, which runs no confirmation.
fn check_confirmation(question: Question, candidate: &FamilyAddendum) -> Result<(), String> {
    let frozen = candidate
        .frozen
        .frozen_utc
        .as_deref()
        .ok_or("the confirmation addendum declares no freeze time")?;
    let mut expected = campaign::addendum(question, &candidate.family.issue, frozen);
    let anchors: Vec<String> = family_cells(question)
        .into_iter()
        .filter(|cell| cell.anchor)
        .map(|cell| cell.cell_id)
        .collect();
    expected
        .cells
        .retain(|cell| anchors.contains(&cell.cell_id));
    for cell in &mut expected.cells {
        cell.role = CellRole::Confirmatory;
    }
    expected
        .family
        .description
        .clone_from(&candidate.family.description);
    expected.effect.measurement_resolution = candidate.effect.measurement_resolution;
    expected
        .effect
        .resolution_evidence
        .clone_from(&candidate.effect.resolution_evidence);
    if expected != *candidate {
        return Err(format!(
            "the addendum is not the confirmation of {}: it changes a cell, margin, limit or \
             rule of the frozen addendum",
            question.family_id()
        ));
    }
    let resolution = candidate
        .effect
        .measurement_resolution
        .ok_or("the confirmation addendum pins no pilot-derived resolution")?;
    let ceiling = resolution_ceiling(question);
    if resolution > ceiling {
        return Err(format!(
            "resolution-insufficient: {resolution} exceeds the family ceiling {ceiling}"
        ));
    }
    Ok(())
}

fn verify_confirmation(arguments: &Arguments) -> Result<(), String> {
    let (question, candidate) = family_addendum(arguments)?;
    let path = arguments.required("addendum")?;
    check_confirmation(question, &candidate).map_err(|error| format!("{path}: {error}"))?;
    println!(
        "{path}: confirms {} cells of {} from {} ({})",
        candidate.cells.len(),
        question.family_id(),
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
        "confirmation" => ReceiptLabel::Confirmation,
        other => {
            return Err(format!(
                "label {other:?} is not pilot, smoke or confirmation"
            ))
        }
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
    println!(
        "{output}: {} cells, {} arms",
        plan.cells.len(),
        plan.arms.len()
    );
    Ok(())
}

/// The repository root every repository-relative campaign path resolves against.
fn repository_root() -> Result<std::path::PathBuf, String> {
    std::fs::canonicalize(".")
        .map_err(|error| format!("cannot resolve the working directory: {error}"))
}

/// Emits one canonical timed request for profiler attribution of a frozen cell.
///
/// The profiler invokes the existing arm and is not a campaign or a source of
/// acceptance samples. The request uses the protocol's builder and frozen
/// window settings, so the arm follows its measured cost boundary.
fn profile_request(arguments: &Arguments) -> Result<(), String> {
    let (question, addendum) = verified_addendum(arguments)?;
    if question == Question::MatvecVsM4ri {
        return Err("the M4RI comparison is a separate family".into());
    }
    let cell_id = arguments.required("cell")?;
    let cell = family_cells(question)
        .into_iter()
        .find(|cell| cell.cell_id == cell_id)
        .ok_or_else(|| format!("{cell_id} is not a frozen cell of {}", question.family_id()))?;
    let (baseline, candidate) = campaign::cell_arms(&cell);
    let arm = arguments.required("arm")?;
    if arm != baseline && arm != candidate {
        return Err(format!("{arm} is not an arm of {cell_id}"));
    }
    let declared = addendum
        .cell(cell_id)
        .ok_or("the cell is absent from the addendum")?;
    let plan_cell = PlanCell {
        cell_id: cell.cell_id.clone(),
        baseline_arm: baseline.to_owned(),
        candidate_arm: candidate.to_owned(),
        case: serde_json::to_value(Case::of(&cell))
            .map_err(|error| format!("cannot encode case: {error}"))?,
        pilot_pairs: Some(campaign::PILOT_PAIRS),
    };
    let cpu = arguments
        .number("cpu")?
        .ok_or("--cpu is required")?
        .try_into()
        .map_err(|_| "--cpu exceeds the supported CPU range")?;
    let role = if arm == baseline {
        PairPosition::Baseline
    } else {
        PairPosition::Candidate
    };
    let request = ArmRequest::timed(
        &plan_cell,
        declared,
        arm,
        role,
        0,
        vec![cpu],
        &SHARED_SETTINGS,
    )?;
    print!("{}", transport::encode_case(&request)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dense_parity_harness::cells::ADDENDUM_FROZEN_UTC;
    use tuning_campaign_support::protocol::ResolutionEvidence;

    /// The confirmation the canonical freezer derives from one family's pilot.
    fn confirmation(question: Question, resolution: f64) -> FamilyAddendum {
        let mut derived = campaign::addendum(question, "e1f9a78f", ADDENDUM_FROZEN_UTC);
        let anchors: Vec<String> = family_cells(question)
            .into_iter()
            .filter(|cell| cell.anchor)
            .map(|cell| cell.cell_id)
            .collect();
        derived.cells.retain(|cell| anchors.contains(&cell.cell_id));
        for cell in &mut derived.cells {
            cell.role = CellRole::Confirmatory;
        }
        derived.family.description = "Confirmatory stage.".to_owned();
        derived.effect.measurement_resolution = Some(resolution);
        derived.effect.resolution_evidence = Some(ResolutionEvidence {
            receipt: "dev/bench_results/2037941f/pilot/receipt.json".to_owned(),
            sha256: "0".repeat(64),
        });
        derived
    }

    #[test]
    fn a_confirmation_of_the_anchor_cells_within_the_ceiling_verifies() {
        for question in Question::ALL {
            let derived = confirmation(question, 0.02);
            derived
                .validate()
                .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
            check_confirmation(question, &derived)
                .unwrap_or_else(|error| panic!("{}: {error}", question.family_id()));
        }
        let comparator = confirmation(Question::MatvecVsM4ri, 0.04);
        let cells: Vec<&str> = comparator
            .cells
            .iter()
            .map(|cell| cell.cell_id.as_str())
            .collect();
        assert_eq!(cells, ["m4ri-gap-65x512-warm", "m4ri-gap-65x4096-warm"]);
        check_confirmation(Question::MatvecVsM4ri, &comparator).expect("at the ceiling");
    }

    #[test]
    fn a_confirmation_that_departs_from_the_frozen_addendum_is_refused() {
        let question = Question::MatvecVsM4ri;
        let derived = confirmation(question, 0.02);

        let insufficient = confirmation(question, 0.041);
        let refusal = check_confirmation(question, &insufficient).unwrap_err();
        assert!(refusal.starts_with("resolution-insufficient"), "{refusal}");

        let mut widened = derived.clone();
        widened.effect.material_gap_threshold = Some(1.2);
        assert!(check_confirmation(question, &widened).is_err());

        let mut narrowed = derived.clone();
        narrowed.cells.pop();
        assert!(check_confirmation(question, &narrowed).is_err());

        let pilot = campaign::addendum(question, "e1f9a78f", ADDENDUM_FROZEN_UTC);
        let mut promoted = derived.clone();
        promoted
            .cells
            .push(pilot.cells.last().expect("a retained-state cell").clone());
        promoted.cells.last_mut().expect("the pushed cell").role = CellRole::Confirmatory;
        assert!(check_confirmation(question, &promoted).is_err());

        let mut unresolved = derived;
        unresolved.effect.measurement_resolution = None;
        unresolved.effect.resolution_evidence = None;
        assert!(check_confirmation(question, &unresolved).is_err());
    }
}
