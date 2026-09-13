//! Preregistered exact acceptance decisions for permanent campaign cells.
//!
//! One [`AcceptancePlan`] fixes the shared global error allocation before any
//! campaign cell is assessed. [`assess_completed_cell`] is the single path for
//! point estimates, Wilson intervals, exact log-scale tests, and verdicts.

use std::fmt;

use serde::{Deserialize, Serialize};

use gf2_stats::binomial::{permanent_zero_floor_test, two_sided_test};

use super::schema::{
    AcceptanceVerdict, CampaignManifest, CellSpec, CellTerminalState, DeterminantCount,
    DeterminantEstimate, DeterminantPlan, Interval, ProportionEstimate, SummaryRow, SCHEMA_VERSION,
};

/// Campaign-wide family-wise false-alarm budget.
pub const GLOBAL_ERROR_BUDGET: f64 = 0.05;
const FAMILY_ERROR_BUDGET: f64 = GLOBAL_ERROR_BUDGET / 2.0;

/// One preregistered family within the shared global budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceFamily {
    /// One-sided proved permanent-floor checks.
    PermanentFloor,
    /// Two-sided finite-size determinant companion checks.
    Determinant,
}

/// A manifest or observation cannot enter the preregistered decision path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceError {
    /// The frozen manifest contains no permanent tests.
    EmptyManifest,
    /// A platform count cannot represent the predeclared multiplicity.
    TestCountOverflow,
    /// A requested family has no predeclared tests.
    EmptyFamily(AcceptanceFamily),
    /// Completed counts differ from the manifested cell contract.
    CountMismatch(&'static str),
    /// Determinant observations differ from the manifested companion plan.
    DeterminantPlanMismatch,
}

impl fmt::Display for AcceptanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyManifest => formatter.write_str("acceptance plan requires manifest cells"),
            Self::TestCountOverflow => {
                formatter.write_str("manifest test count cannot be represented as u64")
            }
            Self::EmptyFamily(family) => {
                write!(
                    formatter,
                    "acceptance family {family:?} has no manifested tests"
                )
            }
            Self::CountMismatch(message) => formatter.write_str(message),
            Self::DeterminantPlanMismatch => formatter
                .write_str("determinant observation does not match the manifested companion plan"),
        }
    }
}

impl std::error::Error for AcceptanceError {}

/// Pre-draw multiplicity and error allocation for both test families.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptancePlan {
    global_error: f64,
    permanent_family_error: f64,
    determinant_family_error: f64,
    permanent_test_count: u64,
    determinant_test_count: u64,
}

impl AcceptancePlan {
    /// Derives fixed family sizes from the complete frozen manifest.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptanceError::EmptyManifest`] for an empty manifest or
    /// [`AcceptanceError::TestCountOverflow`] when a family size exceeds
    /// `u64`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C)` time and `O(1)` extra space for `C` manifest cells.
    pub fn for_manifest(manifest: &CampaignManifest) -> Result<Self, AcceptanceError> {
        let permanent_test_count =
            u64::try_from(manifest.cells.len()).map_err(|_| AcceptanceError::TestCountOverflow)?;
        if permanent_test_count == 0 {
            return Err(AcceptanceError::EmptyManifest);
        }
        let determinant_test_count = manifest
            .cells
            .iter()
            .filter(|cell| cell.determinant_companion == DeterminantPlan::Evaluate)
            .count()
            .try_into()
            .map_err(|_| AcceptanceError::TestCountOverflow)?;
        Ok(Self {
            global_error: GLOBAL_ERROR_BUDGET,
            permanent_family_error: FAMILY_ERROR_BUDGET,
            determinant_family_error: FAMILY_ERROR_BUDGET,
            permanent_test_count,
            determinant_test_count,
        })
    }

    /// Returns the one shared campaign-wide error budget.
    #[must_use]
    pub const fn global_error(self) -> f64 {
        self.global_error
    }

    /// Returns the fixed campaign-wide share for one family.
    #[must_use]
    pub const fn family_budget(self, family: AcceptanceFamily) -> f64 {
        match family {
            AcceptanceFamily::PermanentFloor => self.permanent_family_error,
            AcceptanceFamily::Determinant => self.determinant_family_error,
        }
    }

    /// Returns the predeclared number of tests in one family.
    #[must_use]
    pub const fn family_test_count(self, family: AcceptanceFamily) -> u64 {
        match family {
            AcceptanceFamily::PermanentFloor => self.permanent_test_count,
            AcceptanceFamily::Determinant => self.determinant_test_count,
        }
    }

    /// Returns the fixed per-cell level for one family.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptanceError::EmptyFamily`] when the manifest predeclares
    /// no tests in the requested family.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(1)` time and space.
    pub fn cell_level(self, family: AcceptanceFamily) -> Result<f64, AcceptanceError> {
        let count = self.family_test_count(family);
        if count == 0 {
            return Err(AcceptanceError::EmptyFamily(family));
        }
        Ok(self.family_budget(family) / count as f64)
    }
}

/// Mechanical evidence for one exact binomial decision.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactTestEvidence {
    /// Test family and tail convention.
    pub family: AcceptanceFamily,
    /// Observed event count, the sufficient statistic for the test.
    pub successes: u64,
    /// Fixed sample count.
    pub trials: u64,
    /// Null probability used by the exact test.
    pub null_probability: f64,
    /// Natural logarithm of the exact p-value.
    pub log_p_value: f64,
    /// Fixed per-cell rejection level.
    pub level: f64,
    /// Closed-threshold exact-test verdict.
    pub verdict: AcceptanceVerdict,
}

/// Estimate, interval, exact test, and verdict from one canonical path.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessedProportion {
    /// Point estimate and 95% Wilson interval.
    pub estimate: ProportionEstimate,
    /// Exact acceptance evidence over the same counts.
    pub test: ExactTestEvidence,
}

/// Complete assessment of one executed-to-completion campaign cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedCellAssessment {
    /// Canonical raw summary row containing estimates and verdicts.
    pub summary: SummaryRow,
    /// Permanent-floor assessment.
    pub permanent: AssessedProportion,
    /// Determinant assessment, absent exactly for `not_evaluated` cells.
    pub determinant: Option<AssessedProportion>,
}

impl CompletedCellAssessment {
    /// Returns whether either checked family rejects this completed cell.
    #[must_use]
    pub fn rejected(&self) -> bool {
        self.permanent.test.verdict == AcceptanceVerdict::Rejected
            || self
                .determinant
                .is_some_and(|value| value.test.verdict == AcceptanceVerdict::Rejected)
    }
}

/// Assesses manifested completed counts through the canonical decision path.
///
/// The permanent test is the one-sided exact floor test at `p = 1/q`. The
/// determinant test, when manifested, is two-sided at the exact finite-`n`
/// singular probability. Decisions use exact-test comparisons on log scale,
/// including cases whose ordinary `f64` p-value underflows.
///
/// # Errors
///
/// Returns [`AcceptanceError::CountMismatch`] unless the observation contains
/// exactly the manifested matrix count and valid event counts. Returns
/// [`AcceptanceError::DeterminantPlanMismatch`] unless determinant presence and
/// sample count exactly match [`CellSpec::determinant_companion`].
///
/// # Panics
///
/// Does not panic.
///
/// # Complexity
///
/// `O(n + log N)` time and `O(1)` extra space for matrix order `n` and fixed
/// sample count `N`.
pub fn assess_completed_cell(
    plan: &AcceptancePlan,
    cell: &CellSpec,
    matrix_count: u64,
    permanent_zero_count: u64,
    determinant: DeterminantCount,
) -> Result<CompletedCellAssessment, AcceptanceError> {
    if matrix_count == 0 || matrix_count != cell.matrix_count {
        return Err(AcceptanceError::CountMismatch(
            "completed matrix count differs from the manifested cell",
        ));
    }
    if permanent_zero_count > matrix_count {
        return Err(AcceptanceError::CountMismatch(
            "permanent zero count exceeds matrix count",
        ));
    }
    let permanent_level = plan.cell_level(AcceptanceFamily::PermanentFloor)?;
    let permanent_test =
        permanent_zero_floor_test(permanent_zero_count, matrix_count, u64::from(cell.q));
    let permanent_verdict = verdict(permanent_test.rejects_at(permanent_level));
    let permanent = AssessedProportion {
        estimate: estimate(permanent_zero_count, matrix_count),
        test: ExactTestEvidence {
            family: AcceptanceFamily::PermanentFloor,
            successes: permanent_zero_count,
            trials: matrix_count,
            null_probability: 1.0 / f64::from(cell.q),
            log_p_value: permanent_test.log_p_value(),
            level: permanent_level,
            verdict: permanent_verdict,
        },
    };

    let (determinant_assessment, determinant_estimate) =
        match (cell.determinant_companion, determinant.clone()) {
            (DeterminantPlan::NotEvaluated, DeterminantCount::NotEvaluated) => {
                (None, DeterminantEstimate::NotEvaluated)
            }
            (
                DeterminantPlan::Evaluate,
                DeterminantCount::Evaluated {
                    sample_count,
                    zero_count,
                },
            ) if sample_count == matrix_count && zero_count <= sample_count => {
                let level = plan.cell_level(AcceptanceFamily::Determinant)?;
                let null_probability = determinant_null_probability(cell.q, cell.n);
                let test = two_sided_test(zero_count, sample_count, null_probability);
                let determinant_verdict = verdict(test.rejects_at(level));
                let assessed = AssessedProportion {
                    estimate: estimate(zero_count, sample_count),
                    test: ExactTestEvidence {
                        family: AcceptanceFamily::Determinant,
                        successes: zero_count,
                        trials: sample_count,
                        null_probability,
                        log_p_value: test.log_p_value(),
                        level,
                        verdict: determinant_verdict,
                    },
                };
                (
                    Some(assessed),
                    DeterminantEstimate::Evaluated {
                        estimate: assessed.estimate,
                        verdict: determinant_verdict,
                    },
                )
            }
            _ => return Err(AcceptanceError::DeterminantPlanMismatch),
        };
    Ok(CompletedCellAssessment {
        summary: SummaryRow {
            schema_version: SCHEMA_VERSION,
            q: cell.q,
            n: cell.n,
            matrix_count,
            permanent_zero_count,
            determinant,
            terminal_state: CellTerminalState::Completed {
                permanent_estimate: permanent.estimate,
                permanent_verdict,
                determinant_estimate,
            },
        },
        permanent,
        determinant: determinant_assessment,
    })
}

fn determinant_null_probability(q: u8, n: u16) -> f64 {
    let q = f64::from(q);
    1.0 - (1..=n).fold(1.0, |product, i| product * (1.0 - q.powi(-i32::from(i))))
}

fn verdict(rejected: bool) -> AcceptanceVerdict {
    if rejected {
        AcceptanceVerdict::Rejected
    } else {
        AcceptanceVerdict::Accepted
    }
}

fn estimate(successes: u64, trials: u64) -> ProportionEstimate {
    let (lower, upper) =
        gf2_stats::intervals::wilson_interval(successes, trials, gf2_stats::intervals::Z_95);
    ProportionEstimate {
        point: successes as f64 / trials as f64,
        interval: Interval { lower, upper },
    }
}
