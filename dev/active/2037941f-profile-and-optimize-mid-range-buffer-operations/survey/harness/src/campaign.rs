//! Transcription of one frozen family into a protocol version-4 campaign
//! addendum, and projection of the runner plan that measures it.
//!
//! The transcription is a pilot: every cell is exploratory, the measurement
//! resolution and its evidence are unresolved, and the family derives its prior
//! comparisons from the authoritative ledger. A confirmation addendum is
//! derived from the committed pilot receipt by the canonical freezer
//! `dev/active/c7113c5a/survey/freeze-confirmation.py`.
//!
//! Every numeric setting here is transcribed from the frozen prose addendum.
//! Nothing in this module invents a margin, limit or rule.

use crate::cells::{family_cells, Cell, Question, Workload};
use crate::routes::Route;
use crate::wire::Case;
use std::collections::BTreeMap;
use tuning_campaign_support::host::CoreArm;
use tuning_campaign_support::protocol::{
    ArmBuilds, BuildIdentity, CacheState, CellDeclaration, CellObjective, CellRole,
    ComplexityBudget, EffectRule, FamilyAddendum, FamilyIdentity, FamilyPurpose, FamilyWise,
    Frozen, Holdout, MetricKind, PlanArm, PlanCell, ProtocolRef, ReceiptLabel, RunnerPlan, Scaling,
    SearchBudget, WorkerDeclaration, ADDENDUM_SCHEMA_ID, PLAN_SCHEMA_ID, PROTOCOL_ID,
    PROTOCOL_VERSION,
};

/// Family-level settings the frozen addendum fixes for one question.
struct FamilyRules {
    purpose: FamilyPurpose,
    description: &'static str,
    worthwhile_speedup: Option<f64>,
    rationale: &'static str,
    equivalence_margin: f64,
    equivalence_rationale: &'static str,
    material_gap_threshold: Option<f64>,
    material_gap_rationale: &'static str,
    max_new_unsafe_kernels: u32,
    max_added_source_lines: u32,
    maintenance_rationale: &'static str,
}

fn rules(question: Question) -> FamilyRules {
    match question {
        Question::IsolatedXor => FamilyRules {
            purpose: FamilyPurpose::KernelFamily,
            description: "Isolated public gf2_core::kernels::ops::xor_inplace at the frozen \
                          anchor and neighbouring word counts, both address layouts and the warm \
                          and streaming cache states. This addendum is the pilot: every cell is \
                          exploratory and no cell decides adoption.",
            worthwhile_speedup: Some(1.05),
            rationale: "Five percent is the smallest isolated gain worth carrying into \
                        consumers; isolated evidence never authorizes adoption by itself.",
            equivalence_margin: 1.03,
            equivalence_rationale: "Anchors immediately outside a selected dispatch band must \
                                    remain not-worse within three percent.",
            material_gap_threshold: None,
            material_gap_rationale: "The isolated family selects a candidate; it declares no \
                                     external comparator gap.",
            max_new_unsafe_kernels: 1,
            max_added_source_lines: 80,
            maintenance_rationale: "At most one new private target-feature XOR function with its \
                                    explicit safety contract, 80 added nonblank production lines, \
                                    one existing unsafe-kernel file, and no allocation, \
                                    dependency, public API or second dispatcher.",
        },
        Question::PublicRowXor => FamilyRules {
            purpose: FamilyPurpose::ConsumerFamily,
            description: "Public gf2_core::BitMatrix::row_xor over a 64-row matrix at the frozen \
                          strides, both column shapes and the warm and streaming cache states. \
                          This addendum is the pilot: every cell is exploratory and no cell \
                          decides adoption.",
            worthwhile_speedup: Some(1.03),
            rationale: "Three percent in the public row operation is the smallest \
                        consumer-visible signal worth confirming.",
            equivalence_margin: 1.02,
            equivalence_rationale: "A row anchor outside a selected dispatch band must remain \
                                    not-worse within two percent.",
            material_gap_threshold: None,
            material_gap_rationale: "The row family selects a candidate; it declares no external \
                                     comparator gap.",
            max_new_unsafe_kernels: 1,
            max_added_source_lines: 80,
            maintenance_rationale: "Shares the isolated candidate's code budget and permits no \
                                    row-layout shortcut or extra production lines.",
        },
        Question::NrConstruction => FamilyRules {
            purpose: FamilyPurpose::ConsumerFamily,
            description: "Whole public QuasiCyclicLdpc::nr_5g_rate_matched construction over the \
                          selected eight-word route and its four non-regression controls. This \
                          addendum is the pilot: every cell is exploratory and no cell decides \
                          adoption.",
            worthwhile_speedup: Some(1.02),
            rationale: "Two percent of the whole constructor is worthwhile because maintenance \
                        is one existing resolver plus one fixed-width loop binding.",
            equivalence_margin: 1.015,
            equivalence_rationale: "The four control constructors must remain not-worse within \
                                    one and a half percent.",
            material_gap_threshold: None,
            material_gap_rationale: "The NR family selects a candidate; it declares no external \
                                     comparator gap.",
            max_new_unsafe_kernels: 0,
            max_added_source_lines: 32,
            maintenance_rationale: "One resolver-hoist candidate, at most 32 added nonblank \
                                    production lines in the existing NR construction module, and \
                                    no unsafe code, dependency, allocation, public API, semantic \
                                    type or alternate matrix abstraction.",
        },
        Question::IsalBaseGap => FamilyRules {
            purpose: FamilyPurpose::KernelFamily,
            description: "Operation-equivalent comparison of public gf2 in-place XOR with ISA-L \
                          xor_gen_base/scalar at the frozen word counts. This addendum is the \
                          pilot: every cell is exploratory and no cell decides adoption.",
            worthwhile_speedup: None,
            rationale: "The scalar comparator gap is not an adoption objective.",
            equivalence_margin: 1.05,
            equivalence_rationale: "A five percent band separates a comparable external arm from \
                                    a recorded difference.",
            material_gap_threshold: Some(1.10),
            material_gap_rationale: "A ten-percent external gap warrants attribution, not \
                                     selection. The scalar result cannot stand in for dispatched \
                                     ISA-L.",
            max_new_unsafe_kernels: 0,
            max_added_source_lines: 0,
            maintenance_rationale: "Zero production lines and zero unsafe kernels; no production \
                                    change is selected from an external comparator.",
        },
    }
}

fn objective(cell: &Cell) -> CellObjective {
    match cell.objective() {
        "comparator-gap" => CellObjective::ComparatorGap,
        "non-regression" => CellObjective::NonRegression,
        _ => CellObjective::Improvement,
    }
}

fn cache_state(cell: &Cell) -> CacheState {
    match cell.cache {
        crate::cells::Cache::Warm => CacheState::Warm,
        crate::cells::Cache::Streaming => CacheState::Streaming,
        crate::cells::Cache::Cold => CacheState::Cold,
    }
}

fn size(cell: &Cell) -> BTreeMap<String, u64> {
    let mut size = BTreeMap::new();
    match cell.workload {
        Workload::Xor { words, .. } | Workload::IsalGap { words } => {
            size.insert("words".to_owned(), words as u64);
            size.insert("bits".to_owned(), (words * 64) as u64);
        }
        Workload::RowXor { words, shape } => {
            size.insert("words".to_owned(), words as u64);
            size.insert("rows".to_owned(), crate::cells::ROW_MATRIX_ROWS as u64);
            size.insert("columns".to_owned(), shape.columns(words) as u64);
        }
        Workload::Nr(target) => {
            size.insert("base_graph".to_owned(), u64::from(target.base_graph));
            size.insert("target_n".to_owned(), target.target_n as u64);
            size.insert("target_k".to_owned(), target.target_k as u64);
            size.insert("lifting_factor".to_owned(), target.lifting_factor as u64);
            size.insert("dense_rows".to_owned(), target.dense_rows as u64);
            size.insert("dense_columns".to_owned(), target.dense_cols as u64);
            size.insert("stride_words".to_owned(), target.stride_words as u64);
        }
    }
    size
}

fn builds(question: Question) -> ArmBuilds {
    match question {
        Question::IsalBaseGap => ArmBuilds {
            baseline: BuildIdentity::ConservativePortable,
            candidate: BuildIdentity::External,
        },
        _ => ArmBuilds {
            baseline: BuildIdentity::ConservativePortable,
            candidate: BuildIdentity::ConservativePortable,
        },
    }
}

fn declaration(cell: &Cell) -> CellDeclaration {
    CellDeclaration {
        cell_id: cell.cell_id.clone(),
        objective: objective(cell),
        role: CellRole::Exploratory,
        workload: tuning_campaign_support::protocol::Workload {
            identity: cell.workload_identity(),
            size: size(cell),
            seed: cell.seed,
        },
        metric_kind: if cell.whole_consumer() {
            MetricKind::WholeConsumer
        } else {
            MetricKind::KernelIsolated
        },
        scaling: Scaling::SingleCoreLatency,
        core_arm: CoreArm::SingleCore,
        workers: WorkerDeclaration {
            declared: 1,
            nested_pools_allowed: false,
        },
        cache_state: cache_state(cell),
        cold_calls: cell.cache.cold_calls(),
        builds: builds(cell.question),
        conversion_costs_included: cell.whole_consumer()
            || matches!(cell.workload, Workload::IsalGap { .. }),
        decoder: None,
    }
}

/// Transcribes one frozen family into a version-4 pilot campaign addendum.
///
/// `issue` is the eight-hex short ID of the leaf that owns the campaign and
/// `frozen_utc` is the whole-second UTC time at which it freezes the
/// transcription; both are supplied by that leaf, so this function reads no
/// clock.
pub fn addendum(question: Question, issue: &str, frozen_utc: &str) -> FamilyAddendum {
    let rules = rules(question);
    FamilyAddendum {
        schema: ADDENDUM_SCHEMA_ID.to_owned(),
        protocol: ProtocolRef {
            id: PROTOCOL_ID.to_owned(),
            version: PROTOCOL_VERSION,
        },
        family: FamilyIdentity {
            id: question.family_id().to_owned(),
            issue: issue.to_owned(),
            purpose: rules.purpose,
            description: rules.description.to_owned(),
        },
        frozen: Frozen {
            frozen_utc: Some(frozen_utc.to_owned()),
        },
        effect: EffectRule {
            worthwhile_speedup: rules.worthwhile_speedup,
            rationale: rules.rationale.to_owned(),
            measurement_resolution: None,
            resolution_evidence: None,
            equivalence_margin: Some(rules.equivalence_margin),
            equivalence_rationale: rules.equivalence_rationale.to_owned(),
            material_gap_threshold: rules.material_gap_threshold,
            material_gap_rationale: rules.material_gap_rationale.to_owned(),
        },
        complexity_budget: ComplexityBudget {
            max_new_unsafe_kernels: rules.max_new_unsafe_kernels,
            max_added_source_lines: Some(rules.max_added_source_lines),
            maintenance_rationale: rules.maintenance_rationale.to_owned(),
        },
        family_wise: FamilyWise {
            alpha: tuning_campaign_support::protocol::SHARED_SETTINGS.family_alpha,
            prior_confirmatory_trials: 0,
            prior_trials: Vec::new(),
            ledger_path: Some(question.ledger_path().to_owned()),
        },
        search_budget: SearchBudget {
            // The frozen addendum admits at most four exploratory pilot trials
            // per cell and exactly one confirmatory attempt per identity.
            max_pilot_trials_per_cell: 4,
            max_confirmatory_attempts_per_candidate: 1,
        },
        holdout: Holdout {
            required: false,
            cells: Vec::new(),
        },
        cells: family_cells(question).iter().map(declaration).collect(),
    }
}

/// Arm pairing the plan projects for one question.
pub fn arm_pair(question: Question, isolated_candidate: Route) -> (Route, Route) {
    match question {
        Question::IsolatedXor => (Route::PublicXorA, isolated_candidate),
        Question::PublicRowXor => (Route::RowXorA, Route::RowXorB),
        Question::NrConstruction => (Route::NrConstructA, Route::NrConstructB),
        Question::IsalBaseGap => (Route::IsalPeerGf2, Route::IsalPeerGf2),
    }
}

/// Name the ISA-L comparator arm carries in a plan.
pub const ISAL_ARM: &str = "isal-xor-gen-base";

fn arm_description(route: Route) -> &'static str {
    match route {
        Route::PublicXorA => {
            "Public gf2_core::kernels::ops::xor_inplace, one call per logical operation, with \
             per-call dispatch inside the measured region"
        }
        Route::PublicXorB => {
            "Byte-identical executable and public in-place XOR route, independently launched as \
             the pre-candidate identity comparison"
        }
        Route::ResolvedXor => {
            "Exploratory attribution: one resolve_xor_inplace pointer resolved outside timing and \
             invoked once per logical operation"
        }
        Route::RowXorA => {
            "Public gf2_core::BitMatrix::row_xor, one public row operation per call, cycling the \
             frozen eight directed row pairs"
        }
        Route::RowXorB => {
            "Byte-identical executable and public row-XOR route, independently launched as the \
             pre-candidate identity comparison"
        }
        Route::NrConstructA => {
            "Whole public QuasiCyclicLdpc::nr_5g_rate_matched constructor, including lifting \
             selection, sparse creation, dense conversion, allocation and RREF row operations"
        }
        Route::NrConstructB => {
            "Byte-identical executable constructing the same public code, as the pre-candidate \
             identity comparison"
        }
        Route::IsalPeerGf2 => {
            "Operation-equivalent gf2 arm: fresh aligned destination, copy of source zero, public \
             in-place XOR with source one and output observation, all inside the call"
        }
    }
}

/// Projects the runner plan for one transcribed family.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    question: Question,
    addendum: &FamilyAddendum,
    campaign_id: &str,
    campaign_seed: u64,
    label: ReceiptLabel,
    addendum_path: &str,
    producing_manifest: &str,
    lock_path: &str,
    gf2_executable: &str,
    isal_executable: Option<&str>,
    isolated_candidate: Route,
    max_cells_per_session: Option<u32>,
    pilot_pairs: Option<u32>,
) -> Result<RunnerPlan, String> {
    let (baseline, candidate) = arm_pair(question, isolated_candidate);
    let mut arms: BTreeMap<String, PlanArm> = BTreeMap::new();
    let gf2_arm = |route: Route| PlanArm {
        build: BuildIdentity::ConservativePortable,
        description: arm_description(route).to_owned(),
        executable: gf2_executable.to_owned(),
        arguments: Vec::new(),
        environment: BTreeMap::from([(crate::routes::ROUTE_VAR.to_owned(), route.id().to_owned())]),
        rustflags: Some(String::new()),
        tuning_profile: None,
    };
    arms.insert(baseline.id().to_owned(), gf2_arm(baseline));
    let candidate_name = if question == Question::IsalBaseGap {
        let executable = isal_executable
            .ok_or("the ISA-L family needs the external comparator executable")?
            .to_owned();
        arms.insert(
            ISAL_ARM.to_owned(),
            PlanArm {
                build: BuildIdentity::External,
                description: "ISA-L xor_gen_base/scalar with its fresh aligned destination, its \
                              void * pointer array built inside the call and output observation"
                    .to_owned(),
                executable,
                arguments: Vec::new(),
                environment: BTreeMap::new(),
                rustflags: Some(
                    "C -std=c11 -O3 -march=native -Wall -Wextra -Werror; Rust glue without a \
                     target-cpu override"
                        .to_owned(),
                ),
                tuning_profile: None,
            },
        );
        ISAL_ARM.to_owned()
    } else {
        arms.insert(candidate.id().to_owned(), gf2_arm(candidate));
        candidate.id().to_owned()
    };

    // The plan follows the campaign addendum, and every cell it declares must
    // be a frozen cell of this family at the frozen seed. A campaign addendum
    // that renames, reseeds or invents a cell therefore projects no plan.
    let frozen: Vec<Cell> = family_cells(question);
    let mut cells = Vec::with_capacity(addendum.cells.len());
    for declared in &addendum.cells {
        let cell = frozen
            .iter()
            .find(|cell| cell.cell_id == declared.cell_id)
            .ok_or_else(|| {
                format!(
                    "{} is not a frozen cell of {}",
                    declared.cell_id,
                    question.family_id()
                )
            })?;
        if declared.workload.seed != cell.seed {
            return Err(format!(
                "{} declares seed {} rather than the frozen {}",
                cell.cell_id, declared.workload.seed, cell.seed
            ));
        }
        cells.push(PlanCell {
            cell_id: cell.cell_id.clone(),
            baseline_arm: baseline.id().to_owned(),
            candidate_arm: candidate_name.clone(),
            case: serde_json::to_value(Case::of(cell))
                .map_err(|error| format!("cannot encode case: {error}"))?,
            pilot_pairs,
        });
    }
    Ok(RunnerPlan {
        schema: PLAN_SCHEMA_ID.to_owned(),
        campaign_id: campaign_id.to_owned(),
        issue: addendum.family.issue.clone(),
        label,
        campaign_seed,
        addendum: addendum_path.to_owned(),
        producing_manifest: Some(
            tuning_campaign_support::provenance::ProducingManifestPath::try_from(
                producing_manifest.to_owned(),
            )
            .map_err(|error| format!("producing manifest: {error}"))?,
        ),
        lock_path: lock_path.to_owned(),
        wrapper: "dev/scripts/ccx1-bench-flock.sh --full-host".to_owned(),
        timing_override: None,
        arms,
        cells,
        max_cells_per_session,
    })
}
