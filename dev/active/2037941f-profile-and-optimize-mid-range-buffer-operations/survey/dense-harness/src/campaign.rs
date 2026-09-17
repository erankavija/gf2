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

/// Name the external M4RI comparator arm carries in a plan.
pub const M4RI_ARM: &str = "m4ri-mzd-mul";

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
        Question::IsolatedFusedParity => FamilyRules {
            purpose: FamilyPurpose::KernelFamily,
            description: "Isolated call to the detected bundle's fused AND-population-count entry \
                          over one row and the vector, at the frozen anchor and neighbouring word \
                          counts in the warm and streaming cache states. This addendum is the \
                          pilot: every cell is exploratory and no cell decides adoption.",
            worthwhile_speedup: Some(1.10),
            rationale: "The fused kernel holds most but not all of the allocated consumer at the \
                        attributed 64-word shape, so an isolated gain no larger than the \
                        allocated bar cannot produce a confirmable allocated gain.",
            equivalence_margin: 1.03,
            equivalence_rationale: "Three percent is the finest non-inferiority statement the \
                                    isolated analogue's resolution supports.",
            material_gap_threshold: None,
            material_gap_rationale: "The isolated family selects a candidate; it holds no \
                                     comparator-gap cell and declares no external gap.",
            max_new_unsafe_kernels: 1,
            max_added_source_lines: 96,
            maintenance_rationale: "One new unsafe kernel function with an explicit safety \
                                    contract in crates/gf2-kernels-simd/src/x86/, 96 added \
                                    nonblank production lines, and no allocation, dependency, \
                                    public API, semantic type or second dispatcher.",
        },
        Question::AllocatedMatvec => FamilyRules {
            purpose: FamilyPurpose::ConsumerFamily,
            description: "Public BitMatrix::matvec including its output allocation and appends, \
                          at the frozen strides, both column shapes and the warm, cold and \
                          streaming cache states, with the scalar four-accumulator reference arm. \
                          This addendum is the pilot: every cell is exploratory and no cell \
                          decides adoption.",
            worthwhile_speedup: Some(1.08),
            rationale: "The protocol rejects a margin at or below one plus the resolution, so \
                        1.035 is this family's floor and 1.08 is the first round value above it \
                        that leaves the interval's lower bound room to reach the margin.",
            equivalence_margin: 1.05,
            equivalence_rationale: "A non-inferiority statement finer than five percent is not \
                                    available at the allocated consumer's resolution.",
            material_gap_threshold: None,
            material_gap_rationale: "The allocated family selects a candidate; it holds no \
                                     comparator-gap cell and declares no external gap.",
            max_new_unsafe_kernels: 1,
            max_added_source_lines: 96,
            maintenance_rationale: "The same single candidate and the same 96-line budget as the \
                                    isolated family; the selector keeps its one threshold \
                                    constant and gains no second arm.",
        },
        Question::MatvecVsM4ri => FamilyRules {
            purpose: FamilyPurpose::ConsumerFamily,
            description: "Whole-consumer comparison of public BitMatrix::matvec with the \
                          qualified external M4RI mzd_mul over the qualified shapes, both arms \
                          charging construction, conversion, execution and output. This addendum \
                          is the pilot: every cell is exploratory and no cell decides adoption.",
            worthwhile_speedup: None,
            rationale: "The comparator family holds no improvement or non-regression cell, so its \
                        worthwhile speedup is unresolved by design.",
            equivalence_margin: 1.05,
            equivalence_rationale: "Five percent separates a comparable external arm from a \
                                    recorded difference at this family's resolution.",
            material_gap_threshold: Some(1.10),
            material_gap_rationale: "A ten-percent external gap warrants attribution, not \
                                     selection; M4RI throughput cannot authorise an incompatible \
                                     production route.",
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
        Workload::AndPopcnt { words } => {
            size.insert("words".to_owned(), words as u64);
            size.insert("bits".to_owned(), (words * 64) as u64);
        }
        Workload::Matvec { words, shape } => {
            size.insert("words".to_owned(), words as u64);
            size.insert("rows".to_owned(), crate::cells::MATVEC_ROWS as u64);
            size.insert("columns".to_owned(), shape.columns(words) as u64);
        }
        Workload::M4riGap { shape, retained } => {
            size.insert("rows".to_owned(), shape.rows as u64);
            size.insert("columns".to_owned(), shape.cols as u64);
            size.insert("words".to_owned(), shape.stride_words() as u64);
            size.insert("retained_state".to_owned(), u64::from(retained));
        }
    }
    size
}

fn builds(cell: &Cell) -> ArmBuilds {
    match cell.workload {
        Workload::M4riGap { .. } => ArmBuilds {
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
        workers: WorkerDeclaration { declared: 1, nested_pools_allowed: false },
        cache_state: cache_state(cell),
        cold_calls: cell.cache.cold_calls(),
        builds: builds(cell),
        conversion_costs_included: cell.whole_consumer(),
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
        protocol: ProtocolRef { id: PROTOCOL_ID.to_owned(), version: PROTOCOL_VERSION },
        family: FamilyIdentity {
            id: question.family_id().to_owned(),
            issue: issue.to_owned(),
            purpose: rules.purpose,
            description: rules.description.to_owned(),
        },
        frozen: Frozen { frozen_utc: Some(frozen_utc.to_owned()) },
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
        holdout: Holdout { required: false, cells: Vec::new() },
        cells: family_cells(question).iter().map(declaration).collect(),
    }
}

/// Baseline and candidate arm names for one frozen cell.
pub fn cell_arms(cell: &Cell) -> (&'static str, &'static str) {
    match cell.question {
        Question::IsolatedFusedParity => (Route::AndPopcntA.id(), Route::AndPopcntB.id()),
        Question::AllocatedMatvec if cell.scalar_reference => {
            (Route::MatvecScalarReference.id(), Route::MatvecB.id())
        }
        Question::AllocatedMatvec => (Route::MatvecA.id(), Route::MatvecB.id()),
        Question::MatvecVsM4ri => (Route::M4riPeerGf2.id(), M4RI_ARM),
    }
}

fn arm_description(route: Route) -> &'static str {
    match route {
        Route::AndPopcntA => {
            "One call to the detected bundle's and_popcnt_fn over one row and the vector, reached \
             through the bundle function pointer behind an optimisation barrier"
        }
        Route::AndPopcntB => {
            "The same bundle entry and optimisation barrier, independently launched as the \
             pre-candidate identity comparison or carrying the single candidate fusion body"
        }
        Route::MatvecA => {
            "One public BitMatrix::matvec returning a fresh BitVec, with route selection, the \
             per-row bundle call, the parity fold, the output allocation and every output append \
             inside the measured call"
        }
        Route::MatvecB => {
            "The same public route, independently launched as the pre-candidate identity \
             comparison or carrying the candidate kernel in the detected bundle"
        }
        Route::MatvecScalarReference => {
            "The same public route from an executable built without the simd feature, which \
             reaches the four-accumulator scalar row-parity route"
        }
        Route::M4riPeerGf2 => {
            "One public BitMatrix::matvec on a fresh gf2 matrix and vector, with the returned \
             BitVec allocated and released inside the measured call"
        }
    }
}

/// Executables and settings one plan projection needs from its launcher.
pub struct PlanInputs<'a> {
    /// Campaign identifier the runner stages under.
    pub campaign_id: &'a str,
    /// Campaign seed the runner counterbalances with.
    pub campaign_seed: u64,
    /// Receipt label of the run this plan drives.
    pub label: ReceiptLabel,
    /// Repository-relative path of the campaign JSON addendum.
    pub addendum_path: &'a str,
    /// Repository-relative path of the producing-input closure manifest.
    pub producing_manifest: &'a str,
    /// Absolute path of the CCX1 mutex the runner takes.
    pub lock_path: &'a str,
    /// Baseline gf2 executable, built `conservative-portable` with `simd`.
    pub gf2_executable: &'a str,
    /// Compared gf2 executable; the baseline executable for an identity arm.
    pub candidate_executable: Option<&'a str>,
    /// gf2 executable built without `simd`, for the reference arm.
    pub scalar_executable: Option<&'a str>,
    /// External M4RI comparator executable.
    pub m4ri_executable: Option<&'a str>,
    /// Cells one session measures before it pauses.
    pub max_cells_per_session: Option<u32>,
    /// Paired executions each exploratory cell runs.
    pub pilot_pairs: Option<u32>,
}

impl PlanInputs<'_> {
    fn executable(&self, arm: &str) -> Result<String, String> {
        let candidate = self.candidate_executable.unwrap_or(self.gf2_executable);
        let resolved = match arm {
            name if name == Route::AndPopcntB.id() || name == Route::MatvecB.id() => candidate,
            name if name == Route::MatvecScalarReference.id() => self
                .scalar_executable
                .ok_or("the scalar-reference cells need the executable built without simd")?,
            M4RI_ARM => self
                .m4ri_executable
                .ok_or("the M4RI family needs the external comparator executable")?,
            _ => self.gf2_executable,
        };
        Ok(resolved.to_owned())
    }

    fn arm(&self, name: &str) -> Result<PlanArm, String> {
        let executable = self.executable(name)?;
        if name == M4RI_ARM {
            return Ok(PlanArm {
                build: BuildIdentity::External,
                description: "External M4RI mzd_init for A, x and y, bit-wise packing through \
                              mzd_write_bit, mzd_mul(y, A, x, 0), mzd_read_bit unpacking into a \
                              gf2 BitVec, and disposal of every owned mzd_t"
                    .to_owned(),
                executable,
                arguments: Vec::new(),
                environment: BTreeMap::new(),
                rustflags: Some(
                    "C -O3 -march=native -fPIC library build; Rust glue without a target-cpu \
                     override"
                        .to_owned(),
                ),
                tuning_profile: None,
            });
        }
        let route = Route::parse(name)?;
        Ok(PlanArm {
            build: BuildIdentity::ConservativePortable,
            description: arm_description(route).to_owned(),
            executable,
            arguments: Vec::new(),
            environment: BTreeMap::from([(
                crate::routes::ROUTE_VAR.to_owned(),
                route.id().to_owned(),
            )]),
            rustflags: Some(String::new()),
            tuning_profile: None,
        })
    }
}

/// Projects the runner plan for one transcribed family.
///
/// The plan follows the campaign addendum, and every cell it declares must be a
/// frozen cell of this family at the frozen seed. A campaign addendum that
/// renames, reseeds or invents a cell therefore projects no plan.
pub fn plan(
    question: Question,
    addendum: &FamilyAddendum,
    inputs: &PlanInputs<'_>,
) -> Result<RunnerPlan, String> {
    let frozen: Vec<Cell> = family_cells(question);
    let mut arms: BTreeMap<String, PlanArm> = BTreeMap::new();
    let mut cells = Vec::with_capacity(addendum.cells.len());
    for declared in &addendum.cells {
        let cell = frozen
            .iter()
            .find(|cell| cell.cell_id == declared.cell_id)
            .ok_or_else(|| {
                format!("{} is not a frozen cell of {}", declared.cell_id, question.family_id())
            })?;
        if declared.workload.seed != cell.seed {
            return Err(format!(
                "{} declares seed {} rather than the frozen {}",
                cell.cell_id, declared.workload.seed, cell.seed
            ));
        }
        let (baseline, candidate) = cell_arms(cell);
        for name in [baseline, candidate] {
            if !arms.contains_key(name) {
                arms.insert(name.to_owned(), inputs.arm(name)?);
            }
        }
        cells.push(PlanCell {
            cell_id: cell.cell_id.clone(),
            baseline_arm: baseline.to_owned(),
            candidate_arm: candidate.to_owned(),
            case: serde_json::to_value(Case::of(cell))
                .map_err(|error| format!("cannot encode case: {error}"))?,
            pilot_pairs: inputs.pilot_pairs,
        });
    }
    Ok(RunnerPlan {
        schema: PLAN_SCHEMA_ID.to_owned(),
        campaign_id: inputs.campaign_id.to_owned(),
        issue: addendum.family.issue.clone(),
        label: inputs.label,
        campaign_seed: inputs.campaign_seed,
        addendum: inputs.addendum_path.to_owned(),
        producing_manifest: Some(
            tuning_campaign_support::provenance::ProducingManifestPath::try_from(
                inputs.producing_manifest.to_owned(),
            )
            .map_err(|error| format!("producing manifest: {error}"))?,
        ),
        lock_path: inputs.lock_path.to_owned(),
        wrapper: "dev/scripts/ccx1-bench-flock.sh --full-host".to_owned(),
        timing_override: None,
        arms,
        cells,
        max_cells_per_session: inputs.max_cells_per_session,
    })
}
