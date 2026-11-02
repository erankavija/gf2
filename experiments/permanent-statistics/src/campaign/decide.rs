use super::dataset::{
    AcceptanceVerdict, CellTerminalState, Manifest, ProportionEstimate, SummaryRow, SCHEMA_VERSION,
};
use super::{invalid, CampaignError, Cell};
use gf2_stats::binomial::{lower_tail_test, two_sided_test};

/// Pre-draw per-cell levels and the common report interval convention.
#[derive(Clone, Copy, Debug)]
pub struct AcceptancePlan {
    /// Permanent-floor rejection level.
    pub permanent_level: f64,
    /// Determinant rejection level; zero when no cell has a companion.
    pub determinant_level: f64,
    /// Wilson critical value.
    pub wilson_z: f64,
}
impl AcceptancePlan {
    /// Allocates each family budget across its registered tests.
    pub fn for_manifest(manifest: &Manifest) -> Result<Self, CampaignError> {
        let k = manifest.cells.len() as f64;
        let determinants = manifest
            .cells
            .iter()
            .filter(|cell| cell.determinant_companion)
            .count();
        let a = &manifest.acceptance;
        if k == 0.0
            || !a.wilson_z.is_finite()
            || a.wilson_z <= 0.0
            || !(0.0..1.0).contains(&a.permanent_family_error)
            || a.permanent_family_error == 0.0
            || !(0.0..1.0).contains(&a.determinant_family_error)
            || a.determinant_family_error == 0.0
        {
            return invalid("invalid acceptance allocation");
        }
        Ok(Self {
            permanent_level: a.permanent_family_error / k,
            determinant_level: if determinants == 0 {
                0.0
            } else {
                a.determinant_family_error / determinants as f64
            },
            wilson_z: a.wilson_z,
        })
    }
}
/// Canonical completed-cell assessment, preserving exact log-scale decisions.
/// Refuses incomplete counts or a determinant companion mismatch.
pub fn assess_completed_cell(
    plan: &AcceptancePlan,
    cell: &Cell,
    count: u64,
    zeros: u64,
    determinant: Option<u64>,
) -> Result<SummaryRow, CampaignError> {
    if count == 0
        || count != cell.matrix_count
        || zeros > count
        || determinant.is_some() != cell.determinant_companion
        || determinant.is_some_and(|z| z > count)
    {
        return invalid("completed counts differ from cell");
    }
    let estimate = |z| {
        let (lower, upper) = gf2_stats::intervals::wilson_interval(z, count, plan.wilson_z);
        ProportionEstimate {
            point: z as f64 / count as f64,
            lower,
            upper,
        }
    };
    let verdict = |rejects| {
        if rejects {
            AcceptanceVerdict::Rejected
        } else {
            AcceptanceVerdict::Accepted
        }
    };
    let p = lower_tail_test(zeros, count, 1.0 / f64::from(cell.q));
    let d =
        determinant.map(|z| two_sided_test(z, count, determinant_null_probability(cell.q, cell.n)));
    Ok(SummaryRow {
        schema_version: SCHEMA_VERSION,
        q: cell.q,
        n: cell.n,
        matrix_count: count,
        permanent_zero_count: zeros,
        determinant_zero_count: determinant,
        permanent_estimate: Some(estimate(zeros)),
        determinant_estimate: determinant.map(estimate),
        permanent_verdict: Some(verdict(p.rejects_at(plan.permanent_level))),
        determinant_verdict: d
            .as_ref()
            .map(|t| verdict(t.rejects_at(plan.determinant_level))),
        permanent_log_p_value: Some(p.log_p_value()),
        determinant_log_p_value: d.map(|t| t.log_p_value()),
        terminal_state: CellTerminalState::Completed,
        halt_reason: None,
    })
}
/// Finite-order singular probability for uniformly sampled square matrices.
pub fn determinant_null_probability(q: u8, n: u16) -> f64 {
    1.0 - (1..=n).fold(1.0, |p, i| p * (1.0 - f64::from(q).powi(-i32::from(i))))
}
