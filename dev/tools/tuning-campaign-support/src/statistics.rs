//! Empirical summaries and the preregistered extent argmin rule.

use crate::timing::{TimingSample, WINDOWS};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Timing and effective schedules for one ordered candidate.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CandidateSeries<C, S> {
    pub candidate: C,
    /// One effective schedule tuple per stratum.
    pub schedules: Vec<S>,
    /// Positive median ns/call values, indexed `[stratum][execution]`.
    pub strata: Vec<Vec<f64>>,
}

/// Empirical dispersion without a probabilistic interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct EmpiricalSummary {
    pub median: f64,
    pub iqr_low: f64,
    pub iqr_high: f64,
    pub min: f64,
    pub max: f64,
}

/// A candidate's five default-relative aggregate scores.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CandidateScore<C> {
    pub candidate: C,
    pub executions: Vec<f64>,
    pub summary: EmpiricalSummary,
}

/// Existing threshold sweep statistics over all untrimmed windows.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct WindowStatistics {
    pub median: f64,
    /// Nearest-rank IQR divided by the median.
    pub relative_iqr: f64,
    pub samples: Vec<TimingSample>,
}

impl WindowStatistics {
    /// Reproduces the retained threshold harness's exact summary.
    pub fn from_samples(samples: Vec<TimingSample>) -> Result<Self, StatisticsError> {
        for sample in &samples {
            sample
                .validate()
                .map_err(|error| StatisticsError(error.to_string()))?;
        }
        let values: Vec<_> = samples.iter().map(|sample| sample.ns_per_call()).collect();
        let summary = empirical_summary(&values)?;
        Ok(Self {
            median: summary.median,
            relative_iqr: (summary.iqr_high - summary.iqr_low) / summary.median,
            samples,
        })
    }
}

/// One point in the retained two-arm threshold grid.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ThresholdPoint {
    pub size: usize,
    pub conservative: Option<WindowStatistics>,
    pub asymptotic: Option<WindowStatistics>,
}

impl ThresholdPoint {
    /// The noise band and the asymptotic arm's relative margin.
    pub fn comparison(&self) -> Option<(f64, f64)> {
        let conservative = self.conservative.as_ref()?;
        let asymptotic = self.asymptotic.as_ref()?;
        let band = conservative.relative_iqr.max(asymptotic.relative_iqr);
        let margin = (conservative.median - asymptotic.median) / conservative.median;
        Some((band, margin))
    }

    /// Whether the asymptotic arm beats the conservative arm's noise band.
    pub fn asymptotic_wins(&self) -> Option<bool> {
        self.comparison().map(|(band, margin)| margin > band)
    }
}

/// Direction in which a threshold's selected value is interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ThresholdDirection {
    LowerBound,
    UpperBound { floor: usize },
}

/// Retained threshold fallback reason.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ThresholdFallback {
    NoComparableGridPoint,
    NoGridPointWins,
    NonMonotone { first_win: usize, later_loss: usize },
}

/// Retained threshold decision.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ThresholdSelection {
    Crossover {
        value: usize,
        crossover: usize,
        band: f64,
        margin: f64,
    },
    KeptDefault {
        value: usize,
        reason: ThresholdFallback,
    },
}

impl ThresholdSelection {
    /// Selected value, including a retained default.
    pub const fn value(&self) -> usize {
        match self {
            Self::Crossover { value, .. } | Self::KeptDefault { value, .. } => *value,
        }
    }
}

/// One adjacent effective-schedule class.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScheduleClass<C> {
    pub members: Vec<C>,
    pub representative: C,
}

/// Shape of a strict paired comparison curve after unresolved edges are removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CurveShape {
    Unimodal,
    NonMonotone,
}

/// Result of a strict five-execution paired comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StrictComparison {
    LeftWins,
    RightWins,
    Unresolved,
}

/// Why the selected output is the reported value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionReason {
    SelectedNonDefault,
    MeasuredDefault,
    NonMonotoneSchedule,
    NonMonotoneCurve,
    UnresolvedMinimum,
    StructuralScheduleTie,
    CrossStratumConflict,
}

impl DecisionReason {
    /// Stable receipt spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelectedNonDefault => "selected-nondefault",
            Self::MeasuredDefault => "measured-default",
            Self::NonMonotoneSchedule => "non-monotone-schedule",
            Self::NonMonotoneCurve => "non-monotone-curve",
            Self::UnresolvedMinimum => "unresolved-minimum",
            Self::StructuralScheduleTie => "structural-schedule-tie",
            Self::CrossStratumConflict => "cross-stratum-conflict",
        }
    }
}

/// Complete one-dimensional extent decision.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ExtentDecision<C> {
    pub selected: C,
    pub reason: DecisionReason,
    pub grid_boundary_limited: bool,
    pub scores: Vec<CandidateScore<C>>,
    pub schedule_classes: Vec<ScheduleClass<C>>,
    pub aggregate_curve: CurveShape,
    pub stratum_curves: Vec<CurveShape>,
}

/// Axis of one required coupled-GEMM unimodality slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GemmSliceAxis {
    FixedRow,
    FixedColumn,
}

/// Recorded aggregate or per-stratum coupled-GEMM slice decision.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GemmSliceCurve {
    pub axis: GemmSliceAxis,
    pub fixed_value: usize,
    /// `None` is the aggregate curve; `Some(i)` is stratum `i`.
    pub stratum: Option<usize>,
    pub shape: CurveShape,
}

/// Exact decision for the coupled 3x3 GEMM tile grid.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GemmDecision {
    pub selected: (usize, usize),
    pub reason: DecisionReason,
    /// Present only for a qualified winner; records each grid boundary.
    pub grid_boundary_limited: Option<GemmGridBoundary>,
    pub scores: Vec<CandidateScore<(usize, usize)>>,
    pub slices: Vec<GemmSliceCurve>,
}

/// Whether a qualified GEMM winner lies on a row or column grid boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GemmGridBoundary {
    pub row: bool,
    pub column: bool,
}

/// Whole-vector M4RM joint-validation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JointVectorReason {
    Accepted,
    ConservativeVector,
    JointValidationDefault,
}

/// Exact two-vector M4RM acceptance evidence.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct JointVectorDecision<C> {
    pub selected: C,
    pub reason: JointVectorReason,
    pub proposed_ratios: Vec<f64>,
    pub summary: EmpiricalSummary,
}

/// Invalid or incomplete empirical evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatisticsError(pub String);

impl fmt::Display for StatisticsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StatisticsError {}

/// Returns the median and nearest-rank quartiles of a positive sample.
pub fn empirical_summary(values: &[f64]) -> Result<EmpiricalSummary, StatisticsError> {
    validate_series(values, "empirical sample")?;
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(EmpiricalSummary {
        median: quantile(&sorted, 0.5),
        iqr_low: quantile(&sorted, 0.25),
        iqr_high: quantile(&sorted, 0.75),
        min: sorted[0],
        max: sorted[sorted.len() - 1],
    })
}

/// Computes one execution's median ns/call from exactly five raw windows.
pub fn execution_median(samples: &[TimingSample], execution: u64) -> Result<f64, StatisticsError> {
    if samples.len() != WINDOWS as usize {
        return Err(StatisticsError(format!(
            "execution {execution} has {} windows rather than {WINDOWS}",
            samples.len()
        )));
    }
    let mut seen = [false; WINDOWS as usize];
    let mut values = Vec::with_capacity(WINDOWS as usize);
    for sample in samples {
        sample
            .validate()
            .map_err(|error| StatisticsError(error.to_string()))?;
        if sample.execution != execution || seen[sample.repetition as usize] {
            return Err(StatisticsError(format!(
                "execution {execution} has a wrong or duplicate window coordinate"
            )));
        }
        seen[sample.repetition as usize] = true;
        values.push(sample.ns_per_call());
    }
    Ok(empirical_summary(&values)?.median)
}

/// Reproduces the retained threshold selection rule without owner policy.
pub fn select_threshold(
    points: &[ThresholdPoint],
    default: usize,
    direction: ThresholdDirection,
) -> ThresholdSelection {
    if points.iter().any(|point| point.comparison().is_none()) {
        return ThresholdSelection::KeptDefault {
            value: default,
            reason: ThresholdFallback::NoComparableGridPoint,
        };
    }
    let Some(first) = points
        .iter()
        .position(|point| point.asymptotic_wins() == Some(true))
    else {
        return ThresholdSelection::KeptDefault {
            value: default,
            reason: ThresholdFallback::NoGridPointWins,
        };
    };
    if let Some(later) = points[first + 1..]
        .iter()
        .find(|point| point.asymptotic_wins() != Some(true))
    {
        return ThresholdSelection::KeptDefault {
            value: default,
            reason: ThresholdFallback::NonMonotone {
                first_win: points[first].size,
                later_loss: later.size,
            },
        };
    }
    let (band, margin) = points[first]
        .comparison()
        .expect("a winning grid point has both arms");
    let crossover = points[first].size;
    let value = match direction {
        ThresholdDirection::LowerBound => crossover,
        ThresholdDirection::UpperBound { floor } if first == 0 => floor,
        ThresholdDirection::UpperBound { .. } => points[first - 1].size,
    };
    ThresholdSelection::Crossover {
        value,
        crossover,
        band,
        margin,
    }
}

/// Strictly compares paired positive execution series without epsilon.
pub fn strict_comparison(left: &[f64], right: &[f64]) -> Result<StrictComparison, StatisticsError> {
    if left.len() != right.len() || left.len() != 5 {
        return Err(StatisticsError(format!(
            "strict comparison requires two five-execution series, got {} and {}",
            left.len(),
            right.len()
        )));
    }
    validate_series(left, "left comparison series")?;
    validate_series(right, "right comparison series")?;
    let mut left_wins = true;
    let mut right_wins = true;
    for (left, right) in left.iter().zip(right) {
        let left_ratio = left / right;
        let right_ratio = right / left;
        if !left_ratio.is_finite() || !right_ratio.is_finite() {
            return Err(StatisticsError(
                "paired ratio produced non-finite arithmetic".to_owned(),
            ));
        }
        left_wins &= left_ratio < 1.0;
        right_wins &= right_ratio < 1.0;
    }
    if left_wins {
        Ok(StrictComparison::LeftWins)
    } else if right_wins {
        Ok(StrictComparison::RightWins)
    } else {
        Ok(StrictComparison::Unresolved)
    }
}

/// Classifies an ordered curve using strict adjacent paired comparisons.
pub fn classify_curve(series: &[&[f64]]) -> Result<CurveShape, StatisticsError> {
    let mut saw_increase = false;
    for adjacent in series.windows(2) {
        match strict_comparison(adjacent[0], adjacent[1])? {
            StrictComparison::LeftWins => saw_increase = true,
            StrictComparison::RightWins if saw_increase => return Ok(CurveShape::NonMonotone),
            StrictComparison::RightWins | StrictComparison::Unresolved => {}
        }
    }
    Ok(CurveShape::Unimodal)
}

/// Applies schedule collapse, curve checks, strict argmin qualification, and
/// per-stratum regression checks to an ordered one-dimensional extent grid.
pub fn analyze_extent<C, S>(
    candidates: &[CandidateSeries<C, S>],
    conservative: &C,
) -> Result<ExtentDecision<C>, StatisticsError>
where
    C: Clone + Eq + Ord,
    S: Clone + Eq,
{
    let (strata, default_index) = validate_candidates(candidates, conservative)?;
    let scores = score_candidates(candidates, default_index, strata)?;
    let (class_indices, schedule_classes, nonadjacent_repeat) =
        schedule_classes(candidates, conservative);

    let aggregate_refs: Vec<&[f64]> = class_indices
        .iter()
        .map(|indices| {
            scores[representative_index(candidates, indices, conservative)]
                .executions
                .as_slice()
        })
        .collect();
    let aggregate_curve = classify_curve(&aggregate_refs)?;
    let mut stratum_curves = Vec::with_capacity(strata);
    for stratum in 0..strata {
        let refs: Vec<&[f64]> = class_indices
            .iter()
            .map(|indices| {
                candidates[representative_index(candidates, indices, conservative)].strata[stratum]
                    .as_slice()
            })
            .collect();
        stratum_curves.push(classify_curve(&refs)?);
    }
    if nonadjacent_repeat {
        return Ok(ExtentDecision {
            selected: conservative.clone(),
            reason: DecisionReason::NonMonotoneSchedule,
            grid_boundary_limited: false,
            scores,
            schedule_classes,
            aggregate_curve,
            stratum_curves,
        });
    }
    if aggregate_curve == CurveShape::NonMonotone
        || stratum_curves.contains(&CurveShape::NonMonotone)
    {
        return Ok(ExtentDecision {
            selected: conservative.clone(),
            reason: DecisionReason::NonMonotoneCurve,
            grid_boundary_limited: false,
            scores,
            schedule_classes,
            aggregate_curve,
            stratum_curves,
        });
    }

    let representatives: Vec<usize> = class_indices
        .iter()
        .map(|indices| representative_index(candidates, indices, conservative))
        .collect();
    let provisional_class = representatives
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            scores[**left]
                .summary
                .median
                .total_cmp(&scores[**right].summary.median)
        })
        .map(|(class, _)| class)
        .expect("validated candidates yield a schedule class");
    let winner_index = representatives[provisional_class];
    let qualified = representatives.iter().all(|competitor| {
        *competitor == winner_index
            || strict_comparison(
                &scores[winner_index].executions,
                &scores[*competitor].executions,
            ) == Ok(StrictComparison::LeftWins)
    });
    if !qualified {
        return Ok(ExtentDecision {
            selected: conservative.clone(),
            reason: DecisionReason::UnresolvedMinimum,
            grid_boundary_limited: false,
            scores,
            schedule_classes,
            aggregate_curve,
            stratum_curves,
        });
    }
    if class_indices[provisional_class].len() != 1 {
        return Ok(ExtentDecision {
            selected: conservative.clone(),
            reason: DecisionReason::StructuralScheduleTie,
            grid_boundary_limited: false,
            scores,
            schedule_classes,
            aggregate_curve,
            stratum_curves,
        });
    }

    let selected = &candidates[winner_index];
    let default = &candidates[default_index];
    let regresses = (0..strata).any(|stratum| {
        strict_comparison(&default.strata[stratum], &selected.strata[stratum])
            == Ok(StrictComparison::LeftWins)
    });
    if regresses {
        return Ok(ExtentDecision {
            selected: conservative.clone(),
            reason: DecisionReason::CrossStratumConflict,
            grid_boundary_limited: false,
            scores,
            schedule_classes,
            aggregate_curve,
            stratum_curves,
        });
    }

    let selected_is_default = &selected.candidate == conservative;
    Ok(ExtentDecision {
        selected: selected.candidate.clone(),
        reason: if selected_is_default {
            DecisionReason::MeasuredDefault
        } else {
            DecisionReason::SelectedNonDefault
        },
        grid_boundary_limited: provisional_class == 0
            || provisional_class + 1 == representatives.len(),
        scores,
        schedule_classes,
        aggregate_curve,
        stratum_curves,
    })
}

/// Applies the coupled GEMM rule to an exact row-major 3x3 tile-pair grid.
///
/// Distinct pairs are never schedule-collapsed. All fixed-row and fixed-column
/// slices must be unimodal in aggregate and in every stratum, and the selected
/// pair must strictly beat all eight competitors without a qualified
/// per-stratum regression against the conservative pair.
pub fn analyze_gemm<S>(
    candidates: &[CandidateSeries<(usize, usize), S>],
    conservative: (usize, usize),
) -> Result<GemmDecision, StatisticsError>
where
    S: Clone + Eq,
{
    if candidates.len() != 9 {
        return Err(StatisticsError(format!(
            "coupled GEMM requires nine pairs, got {}",
            candidates.len()
        )));
    }
    let rows = [
        candidates[0].candidate.0,
        candidates[3].candidate.0,
        candidates[6].candidate.0,
    ];
    let cols = [
        candidates[0].candidate.1,
        candidates[1].candidate.1,
        candidates[2].candidate.1,
    ];
    if !rows.windows(2).all(|pair| pair[0] < pair[1])
        || !cols.windows(2).all(|pair| pair[0] < pair[1])
        || candidates
            .iter()
            .enumerate()
            .any(|(index, candidate)| candidate.candidate != (rows[index / 3], cols[index % 3]))
    {
        return Err(StatisticsError(
            "coupled GEMM pairs are not one strictly ordered row-major 3x3 grid".to_owned(),
        ));
    }
    let (strata, default_index) = validate_candidates(candidates, &conservative)?;
    let scores = score_candidates(candidates, default_index, strata)?;
    let mut slices = Vec::new();
    for (axis, fixed_values) in [
        (GemmSliceAxis::FixedRow, rows),
        (GemmSliceAxis::FixedColumn, cols),
    ] {
        for (fixed, fixed_value) in fixed_values.into_iter().enumerate() {
            let indices = |moving: usize| match axis {
                GemmSliceAxis::FixedRow => fixed * 3 + moving,
                GemmSliceAxis::FixedColumn => moving * 3 + fixed,
            };
            let aggregate: Vec<&[f64]> = (0..3)
                .map(|moving| scores[indices(moving)].executions.as_slice())
                .collect();
            slices.push(GemmSliceCurve {
                axis,
                fixed_value,
                stratum: None,
                shape: classify_curve(&aggregate)?,
            });
            for stratum in 0..strata {
                let series: Vec<&[f64]> = (0..3)
                    .map(|moving| candidates[indices(moving)].strata[stratum].as_slice())
                    .collect();
                slices.push(GemmSliceCurve {
                    axis,
                    fixed_value,
                    stratum: Some(stratum),
                    shape: classify_curve(&series)?,
                });
            }
        }
    }
    if slices
        .iter()
        .any(|slice| slice.shape == CurveShape::NonMonotone)
    {
        return Ok(GemmDecision {
            selected: conservative,
            reason: DecisionReason::NonMonotoneCurve,
            grid_boundary_limited: None,
            scores,
            slices,
        });
    }
    let winner = scores
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.summary.median.total_cmp(&right.summary.median))
        .map(|(index, _)| index)
        .expect("nine GEMM scores exist");
    let qualified = scores.iter().enumerate().all(|(competitor, _)| {
        competitor == winner
            || strict_comparison(&scores[winner].executions, &scores[competitor].executions)
                == Ok(StrictComparison::LeftWins)
    });
    if !qualified {
        return Ok(GemmDecision {
            selected: conservative,
            reason: DecisionReason::UnresolvedMinimum,
            grid_boundary_limited: None,
            scores,
            slices,
        });
    }
    if (0..strata).any(|stratum| {
        strict_comparison(
            &candidates[default_index].strata[stratum],
            &candidates[winner].strata[stratum],
        ) == Ok(StrictComparison::LeftWins)
    }) {
        return Ok(GemmDecision {
            selected: conservative,
            reason: DecisionReason::CrossStratumConflict,
            grid_boundary_limited: None,
            scores,
            slices,
        });
    }
    Ok(GemmDecision {
        selected: candidates[winner].candidate,
        reason: if winner == default_index {
            DecisionReason::MeasuredDefault
        } else {
            DecisionReason::SelectedNonDefault
        },
        grid_boundary_limited: Some(GemmGridBoundary {
            row: winner / 3 == 0 || winner / 3 == 2,
            column: winner % 3 == 0 || winner % 3 == 2,
        }),
        scores,
        slices,
    })
}

/// Applies the mandatory M4RM proposed-vector versus conservative-vector gate.
pub fn analyze_joint_vector<C, S>(
    conservative: &CandidateSeries<C, S>,
    proposed: &CandidateSeries<C, S>,
) -> Result<JointVectorDecision<C>, StatisticsError>
where
    C: Clone + Eq,
    S: Eq,
{
    if conservative.strata.is_empty()
        || conservative.strata.len() != proposed.strata.len()
        || conservative.schedules.len() != conservative.strata.len()
        || proposed.schedules.len() != proposed.strata.len()
    {
        return Err(StatisticsError(
            "joint vectors have missing or mismatched strata and schedules".to_owned(),
        ));
    }
    let mut log_ratios = [0.0_f64; 5];
    let mut regression = false;
    for stratum in 0..conservative.strata.len() {
        let default = &conservative.strata[stratum];
        let trial = &proposed.strata[stratum];
        if default.len() != 5 || trial.len() != 5 {
            return Err(StatisticsError(
                "joint-vector strata require five executions".to_owned(),
            ));
        }
        validate_series(default, "conservative joint stratum")?;
        validate_series(trial, "proposed joint stratum")?;
        for execution in 0..5 {
            let ratio = trial[execution] / default[execution];
            if !ratio.is_finite() || ratio <= 0.0 {
                return Err(StatisticsError(
                    "joint-vector ratio produced invalid arithmetic".to_owned(),
                ));
            }
            log_ratios[execution] += ratio.ln();
            regression |= ratio > 1.0;
        }
    }
    let ratios: Vec<_> = log_ratios
        .into_iter()
        .map(|sum| (sum / conservative.strata.len() as f64).exp())
        .collect();
    let summary = empirical_summary(&ratios)?;
    let (selected, reason) = if conservative.candidate == proposed.candidate {
        (
            conservative.candidate.clone(),
            JointVectorReason::ConservativeVector,
        )
    } else if ratios.iter().all(|ratio| *ratio < 1.0) && !regression {
        (proposed.candidate.clone(), JointVectorReason::Accepted)
    } else {
        (
            conservative.candidate.clone(),
            JointVectorReason::JointValidationDefault,
        )
    };
    Ok(JointVectorDecision {
        selected,
        reason,
        proposed_ratios: ratios,
        summary,
    })
}

fn validate_candidates<C, S>(
    candidates: &[CandidateSeries<C, S>],
    conservative: &C,
) -> Result<(usize, usize), StatisticsError>
where
    C: Eq + Ord,
{
    let Some(first) = candidates.first() else {
        return Err(StatisticsError("extent grid is empty".to_owned()));
    };
    let strata = first.strata.len();
    if strata == 0 {
        return Err(StatisticsError("extent grid has no strata".to_owned()));
    }
    let mut default = None;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.schedules.len() != strata || candidate.strata.len() != strata {
            return Err(StatisticsError(format!(
                "candidate {index} does not have exactly {strata} schedules and strata"
            )));
        }
        for values in &candidate.strata {
            if values.len() != 5 {
                return Err(StatisticsError(format!(
                    "candidate {index} has {} executions rather than five",
                    values.len()
                )));
            }
            validate_series(values, "candidate stratum")?;
        }
        if &candidate.candidate == conservative && default.replace(index).is_some() {
            return Err(StatisticsError(
                "conservative candidate occurs more than once".to_owned(),
            ));
        }
        if candidates[..index]
            .iter()
            .any(|prior| prior.candidate == candidate.candidate)
        {
            return Err(StatisticsError(
                "candidate occurs more than once".to_owned(),
            ));
        }
        if index > 0 && candidates[index - 1].candidate >= candidate.candidate {
            return Err(StatisticsError(
                "extent candidates are not in strictly increasing protocol order".to_owned(),
            ));
        }
    }
    let default = default.ok_or_else(|| {
        StatisticsError("conservative candidate is absent from the extent grid".to_owned())
    })?;
    Ok((strata, default))
}

fn score_candidates<C, S>(
    candidates: &[CandidateSeries<C, S>],
    default: usize,
    strata: usize,
) -> Result<Vec<CandidateScore<C>>, StatisticsError>
where
    C: Clone,
{
    candidates
        .iter()
        .map(|candidate| {
            let executions: Vec<f64> = (0..5)
                .map(|execution| {
                    let mean_log = (0..strata)
                        .map(|stratum| {
                            let ratio = candidate.strata[stratum][execution]
                                / candidates[default].strata[stratum][execution];
                            if !ratio.is_finite() || ratio <= 0.0 {
                                return f64::NAN;
                            }
                            ratio.ln()
                        })
                        .sum::<f64>()
                        / strata as f64;
                    mean_log.exp()
                })
                .collect();
            validate_series(&executions, "aggregate candidate score")?;
            Ok(CandidateScore {
                candidate: candidate.candidate.clone(),
                summary: empirical_summary(&executions)?,
                executions,
            })
        })
        .collect()
}

fn schedule_classes<C, S>(
    candidates: &[CandidateSeries<C, S>],
    conservative: &C,
) -> (Vec<Vec<usize>>, Vec<ScheduleClass<C>>, bool)
where
    C: Clone + Eq + Ord,
    S: Eq,
{
    let nonadjacent_repeat = (0..candidates.len()).any(|left| {
        (left + 2..candidates.len()).any(|right| {
            candidates[left].schedules == candidates[right].schedules
                && candidates[left + 1..right]
                    .iter()
                    .any(|middle| middle.schedules != candidates[left].schedules)
        })
    });
    let mut indices: Vec<Vec<usize>> = Vec::new();
    for index in 0..candidates.len() {
        let joins_current = indices
            .last()
            .is_some_and(|current| candidates[current[0]].schedules == candidates[index].schedules);
        if joins_current {
            indices
                .last_mut()
                .expect("a joining schedule class exists")
                .push(index);
        } else {
            indices.push(vec![index]);
        }
    }
    let classes = indices
        .iter()
        .map(|members| {
            let representative = representative_index(candidates, members, conservative);
            ScheduleClass {
                members: members
                    .iter()
                    .map(|index| candidates[*index].candidate.clone())
                    .collect(),
                representative: candidates[representative].candidate.clone(),
            }
        })
        .collect();
    (indices, classes, nonadjacent_repeat)
}

fn representative_index<C, S>(
    candidates: &[CandidateSeries<C, S>],
    members: &[usize],
    conservative: &C,
) -> usize
where
    C: Eq + Ord,
{
    members
        .iter()
        .copied()
        .find(|index| &candidates[*index].candidate == conservative)
        .unwrap_or_else(|| {
            *members
                .iter()
                .min_by_key(|index| &candidates[**index].candidate)
                .expect("schedule classes are nonempty")
        })
}

fn validate_series(values: &[f64], name: &str) -> Result<(), StatisticsError> {
    if values.is_empty() {
        return Err(StatisticsError(format!("{name} is empty")));
    }
    if let Some(value) = values
        .iter()
        .find(|value| !value.is_finite() || **value <= 0.0)
    {
        return Err(StatisticsError(format!(
            "{name} contains non-positive or non-finite value {value}"
        )));
    }
    Ok(())
}

fn quantile(sorted: &[f64], fraction: f64) -> f64 {
    if fraction == 0.5 && sorted.len().is_multiple_of(2) {
        let upper = sorted.len() / 2;
        return (sorted[upper - 1] + sorted[upper]) / 2.0;
    }
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
