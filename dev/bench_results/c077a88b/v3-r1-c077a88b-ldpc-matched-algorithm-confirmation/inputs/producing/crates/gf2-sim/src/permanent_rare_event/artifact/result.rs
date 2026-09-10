//! Exact reduction of published checkpoints into final result payloads.
//!
//! The reducers here are the producing half of the validators in the parent
//! module: both sides call the same exact histogram, mean, variance, interval,
//! and effective-sample-size routines, so a produced payload and its
//! independent revalidation cannot drift apart.

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::{
    canonical_bytes, exact_mean, exact_sample_variance, interval_contains, merge_histograms,
    parse_canonical_integer, parse_probability, rendered_interval, sha256_hex, validate_histogram,
    validate_relative_path, ArtifactError, CoverageCountV1, CoverageReplicateV1,
    CoverageResultPayloadV1, CoverageVerdictV1, CrossCheckVerdictV1, ExactDecimalV1, ExactValue,
    RareEventDatasetIdentityV1, ScientificIdentityV1, TargetResultPayloadV1,
    ValidatedCheckpointSet, COVERAGE_REPLICATES, COVERAGE_RUNS, COVERAGE_TRAJECTORIES_PER_RUN,
    TARGET_BLOCK_SIZE, TARGET_RUNS, TARGET_TRAJECTORIES_PER_RUN,
};

/// The only accepted exact target-result schema.
pub const EXACT_TARGET_RESULT_SCHEMA_V1: &str = "gf2.rare-event-exact-target-result/v1";

/// Fixed maximum target exponent, one contraction dimension per row.
const TARGET_MAXIMUM_EXPONENT: u32 = 3 * 1_024;
/// Fixed maximum coverage exponent for the three-row compressed state.
const COVERAGE_MAXIMUM_EXPONENT: u32 = 9;
/// Fixed degeneracy threshold on the effective-sample-size fraction.
const DEGENERACY_THRESHOLD_DENOMINATOR: u8 = 100;
/// Fixed per-field coverage adequacy threshold out of 200 replicates.
const COVERAGE_ADEQUACY_THRESHOLD: u16 = 180;

/// The exact deficient-rank count this cross-check is measured against.
///
/// A completing target run reads these bytes from its artifact root and cites
/// them by path and digest in its final receipt, so the stochastic estimate is
/// compared against a preregistered exact answer rather than one chosen after
/// the fact. The issue that executes a target campaign commits them before its
/// first draw.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactTargetResultV1 {
    /// Exact schema identifier.
    pub exact_result_schema: String,
    /// Field order.
    pub q: u8,
    /// Row count.
    pub n: u16,
    /// Column/rank parameter.
    pub k: u8,
    /// Exact deficient-rank count as a canonical decimal integer.
    pub raw_count: String,
    /// Exact total as a canonical decimal integer.
    pub total: String,
}

/// One decoded exact result bound to the exact bytes it was read from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTargetInput {
    path: String,
    sha256: String,
    raw_count: BigUint,
    total: BigUint,
}

impl ExactTargetInput {
    /// Returns the repository-relative path these exact bytes were read from.
    ///
    /// The final receipt cites this path, so it names the artifact a reader
    /// must fetch to recheck the comparison.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the SHA-256 of the exact bytes this result was decoded from.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Decodes one exact target result and binds it to its path and digest.
///
/// # Errors
///
/// Refuses a non-canonical encoding, an unknown schema, an allocation that
/// differs from the target identity, and a count exceeding its total.
pub fn decode_exact_target_result(
    identity: &RareEventDatasetIdentityV1,
    path: &str,
    bytes: &[u8],
) -> Result<ExactTargetInput, ArtifactError> {
    validate_relative_path(path)?;
    let decoded: ExactTargetResultV1 = serde_json::from_slice(bytes)?;
    if canonical_bytes(&decoded)? != bytes {
        return Err(ArtifactError::Schema(
            "exact target result JSON is not canonical".into(),
        ));
    }
    if decoded.exact_result_schema != EXACT_TARGET_RESULT_SCHEMA_V1 {
        return Err(ArtifactError::Schema(
            "unknown exact target result schema".into(),
        ));
    }
    let ScientificIdentityV1::Target { q, n, k, .. } = identity.scientific else {
        return Err(ArtifactError::Identity(
            "exact target result needs a target dataset identity".into(),
        ));
    };
    if (decoded.q, decoded.n, decoded.k) != (q, n, k) {
        return Err(ArtifactError::Identity(
            "exact target result addresses another allocation".into(),
        ));
    }
    let raw_count = parse_canonical_integer(&decoded.raw_count, "exact raw count")?;
    let total = parse_canonical_integer(&decoded.total, "exact total")?;
    if raw_count > total || total != BigUint::from(q).pow(u32::from(n) * u32::from(k)) {
        return Err(ArtifactError::Schema(
            "exact target counts leave the closed allocation".into(),
        ));
    }
    Ok(ExactTargetInput {
        path: path.to_owned(),
        sha256: sha256_hex(bytes),
        raw_count,
        total,
    })
}

/// Renders one exact value as its canonical reduced decimal pair.
fn decimal(value: &ExactValue) -> ExactDecimalV1 {
    ExactDecimalV1::new(value.numerator.to_string(), value.denominator.to_string())
}

/// Returns the fixed degeneracy threshold on the ESS fraction.
fn degeneracy_threshold() -> ExactValue {
    ExactValue::new(
        BigUint::from(1_u8),
        BigUint::from(DEGENERACY_THRESHOLD_DENOMINATOR),
    )
    .expect("the fixed degeneracy denominator is positive")
}

/// Reduces the complete target checkpoint set into its final result payload.
///
/// # Errors
///
/// Refuses a set that is not the closed target partition, a histogram outside
/// its fixed range, and a run-mean sum of zero.
pub fn target_result_payload(
    checkpoints: &ValidatedCheckpointSet,
    exact: &ExactTargetInput,
) -> Result<TargetResultPayloadV1, ArtifactError> {
    let runs = usize::from(TARGET_RUNS);
    let blocks_per_run = (TARGET_TRAJECTORIES_PER_RUN / TARGET_BLOCK_SIZE) as usize;
    let run_trajectories = u64::from(TARGET_TRAJECTORIES_PER_RUN);
    let total_trajectories = run_trajectories * runs as u64;
    if checkpoints.checkpoints.len() != runs * blocks_per_run {
        return Err(ArtifactError::AddressSet(
            "target reducer needs every published checkpoint".into(),
        ));
    }

    let mut run_means = Vec::with_capacity(runs);
    let mut per_run_ess = Vec::with_capacity(runs);
    let mut extinction_reasons = Vec::new();
    for run in 0..runs {
        let blocks = &checkpoints.checkpoints[run * blocks_per_run..(run + 1) * blocks_per_run];
        let pooled = merge_histograms(
            blocks
                .iter()
                .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
            TARGET_MAXIMUM_EXPONENT,
        )?;
        let (ess, _, mean) =
            validate_histogram(&pooled, 3, TARGET_MAXIMUM_EXPONENT, run_trajectories)?;
        if mean == ExactValue::zero() {
            extinction_reasons.push(format!("target run {run:02} produced zero total weight"));
        }
        run_means.push(mean);
        per_run_ess.push(ess);
    }

    let exponent_histogram = merge_histograms(
        checkpoints
            .checkpoints
            .iter()
            .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
        TARGET_MAXIMUM_EXPONENT,
    )?;
    let (final_weight_ess, largest_weight_share, histogram_mean) = validate_histogram(
        &exponent_histogram,
        3,
        TARGET_MAXIMUM_EXPONENT,
        total_trajectories,
    )?;
    let estimate = exact_mean(&run_means);
    if estimate != histogram_mean {
        return Err(ArtifactError::Schema(
            "reduced run means and pooled histogram disagree".into(),
        ));
    }
    let independent_run_variance = exact_sample_variance(&run_means);
    let minimum_exponent = exponent_histogram
        .first()
        .expect("a validated histogram is nonempty")
        .exponent;
    let maximum_exponent = exponent_histogram
        .last()
        .expect("a validated histogram is nonempty")
        .exponent;
    let (interval_lower, interval_upper) =
        rendered_interval(&estimate, &independent_run_variance, 3, minimum_exponent)?;
    let ess_fraction = final_weight_ess.divide(&ExactValue::from_integer(total_trajectories))?;
    let run_sum = run_means
        .iter()
        .fold(ExactValue::zero(), |sum, value| sum.add(value));
    if run_sum == ExactValue::zero() {
        return Err(ArtifactError::Schema("target run-mean sum is zero".into()));
    }
    let largest_run_mean_share = run_means
        .iter()
        .max()
        .expect("the target allocation has runs")
        .divide(&run_sum)?;
    let exact_probability = ExactValue::new(exact.raw_count.clone(), exact.total.clone())?;
    let degeneracy = ess_fraction < degeneracy_threshold();
    let verdict = if !extinction_reasons.is_empty() || degeneracy {
        CrossCheckVerdictV1::Unusable
    } else if interval_contains(&exact_probability, &estimate, &independent_run_variance)? {
        CrossCheckVerdictV1::Agreement
    } else {
        CrossCheckVerdictV1::Contradiction
    };

    Ok(TargetResultPayloadV1 {
        expected_trajectory_count: total_trajectories,
        exact_result_path: exact.path.clone(),
        exact_result_sha256: exact.sha256.clone(),
        exact_raw_count: exact.raw_count.to_string(),
        exact_total: exact.total.to_string(),
        exact_probability: decimal(&exact_probability),
        cross_check_estimate: decimal(&estimate),
        independent_run_variance: decimal(&independent_run_variance),
        interval_lower,
        interval_upper,
        final_weight_ess: decimal(&final_weight_ess),
        per_run_ess: per_run_ess.iter().map(decimal).collect(),
        ess_fraction: decimal(&ess_fraction),
        largest_weight_share: decimal(&largest_weight_share),
        largest_run_mean_share: decimal(&largest_run_mean_share),
        exponent_histogram,
        minimum_exponent,
        maximum_exponent,
        run_means: run_means.iter().map(decimal).collect(),
        extinction_reasons,
        degeneracy,
        verdict,
    })
}

/// Reduces the complete coverage checkpoint set into its final result payload.
///
/// # Errors
///
/// Refuses a set that is not the closed coverage partition, an identity that
/// is not a coverage allocation, and a histogram outside its fixed range.
pub fn coverage_result_payload(
    identity: &RareEventDatasetIdentityV1,
    checkpoints: &ValidatedCheckpointSet,
) -> Result<CoverageResultPayloadV1, ArtifactError> {
    let ScientificIdentityV1::Coverage { cases, .. } = &identity.scientific else {
        return Err(ArtifactError::Identity(
            "coverage reducer needs a coverage dataset identity".into(),
        ));
    };
    let replicates_per_case = usize::from(COVERAGE_REPLICATES);
    let runs_per_replicate = usize::from(COVERAGE_RUNS);
    let replicate_trajectories =
        u64::from(COVERAGE_TRAJECTORIES_PER_RUN) * runs_per_replicate as u64;
    let expected_checkpoints = cases.len() * replicates_per_case * runs_per_replicate;
    if checkpoints.checkpoints.len() != expected_checkpoints {
        return Err(ArtifactError::AddressSet(
            "coverage reducer needs every published checkpoint".into(),
        ));
    }

    let mut replicate_results = Vec::with_capacity(cases.len() * replicates_per_case);
    let mut coverage_counts = Vec::with_capacity(cases.len());
    let mut has_extinction = false;
    for (case_slot, case) in cases.iter().enumerate() {
        let anchor = parse_probability(&case.exact_anchor, "coverage exact anchor")?;
        let mut containing = 0_u16;
        for replicate in 0..replicates_per_case {
            let offset = (case_slot * replicates_per_case + replicate) * runs_per_replicate;
            let runs = &checkpoints.checkpoints[offset..offset + runs_per_replicate];
            let mut run_means = Vec::with_capacity(runs_per_replicate);
            let mut extinction_reasons = Vec::new();
            for (run, checkpoint) in runs.iter().enumerate() {
                let (_, _, mean) = validate_histogram(
                    &checkpoint.exponent_histogram,
                    case.q,
                    COVERAGE_MAXIMUM_EXPONENT,
                    u64::from(COVERAGE_TRAJECTORIES_PER_RUN),
                )?;
                if mean == ExactValue::zero() {
                    let q = case.q;
                    extinction_reasons.push(format!(
                        "coverage q{q} replicate {replicate:03} run {run:02} produced zero total weight"
                    ));
                }
                run_means.push(mean);
            }
            let exponent_histogram = merge_histograms(
                runs.iter()
                    .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
                COVERAGE_MAXIMUM_EXPONENT,
            )?;
            let (final_weight_ess, _, histogram_mean) = validate_histogram(
                &exponent_histogram,
                case.q,
                COVERAGE_MAXIMUM_EXPONENT,
                replicate_trajectories,
            )?;
            let estimate = exact_mean(&run_means);
            if estimate != histogram_mean {
                return Err(ArtifactError::Schema(
                    "reduced run means and pooled histogram disagree".into(),
                ));
            }
            let independent_run_variance = exact_sample_variance(&run_means);
            let (interval_lower, interval_upper) = rendered_interval(
                &estimate,
                &independent_run_variance,
                u32::from(case.q),
                exponent_histogram
                    .first()
                    .expect("a validated histogram is nonempty")
                    .exponent,
            )?;
            let contains_anchor = interval_contains(&anchor, &estimate, &independent_run_variance)?;
            containing += u16::from(contains_anchor);
            let ess_fraction =
                final_weight_ess.divide(&ExactValue::from_integer(replicate_trajectories))?;
            let degeneracy = ess_fraction < degeneracy_threshold();
            has_extinction |= !extinction_reasons.is_empty();
            replicate_results.push(CoverageReplicateV1 {
                q: case.q,
                replicate: replicate as u16,
                exact_anchor: case.exact_anchor.clone(),
                estimate: decimal(&estimate),
                independent_run_variance: decimal(&independent_run_variance),
                interval_lower,
                interval_upper,
                contains_anchor,
                ess_fraction: decimal(&ess_fraction),
                final_weight_ess: decimal(&final_weight_ess),
                exponent_histogram,
                run_means: run_means.iter().map(decimal).collect(),
                extinction_reasons,
                degeneracy,
            });
        }
        coverage_counts.push(CoverageCountV1 {
            q: case.q,
            count: containing,
        });
    }

    let adequate = !has_extinction
        && coverage_counts
            .iter()
            .all(|count| count.count >= COVERAGE_ADEQUACY_THRESHOLD);
    Ok(CoverageResultPayloadV1 {
        expected_trajectory_count: replicate_trajectories
            * (cases.len() * replicates_per_case) as u64,
        replicates: replicate_results,
        coverage_counts,
        verdict: if adequate {
            CoverageVerdictV1::Adequate
        } else {
            CoverageVerdictV1::Unusable
        },
    })
}
