//! Campaign-cell backend nominations and equivalence budgets.
//!
//! The backend selection receipt is the authority for both the campaign cell
//! grid and the configurations checked at each cell. It is embedded into the
//! executable and parsed here; this harness never reconstructs a selection
//! from the premeasurement rates.

use crate::backend::Backend;
use crate::equivalence::reference_backend;

/// Repository path of the authoritative backend-selection receipt.
pub const SELECTION_AUTHORITY_PATH: &str =
    "dev/benchmarks/permanent_campaign/backend-selection-v1.md";

/// SHA-256 of [`SELECTION_AUTHORITY_PATH`] at the harness revision that embeds
/// it. The executable SHA-256 in each receipt pins these embedded bytes.
pub const SELECTION_AUTHORITY_SHA256: &str =
    "fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe";

const SELECTION_AUTHORITY: &str =
    include_str!("../../../benchmarks/permanent_campaign/backend-selection-v1.md");

/// Seconds available to one equivalence cell in the committed budget rule.
pub const EQUIVALENCE_CELL_BUDGET_SECONDS: f64 = 240.0;
/// Smallest matrix corpus allowed by the committed budget rule.
pub const EQUIVALENCE_MIN_MATRICES: usize = 2;

/// Per-order ceilings extended over every exact campaign order.
pub const EQUIVALENCE_ORDER_CEILINGS: [(usize, usize); 25] = [
    (4, 512),
    (5, 512),
    (6, 512),
    (7, 512),
    (8, 512),
    (9, 512),
    (10, 512),
    (11, 512),
    (12, 512),
    (13, 512),
    (14, 512),
    (15, 512),
    (16, 512),
    (17, 512),
    (18, 512),
    (19, 512),
    (20, 512),
    (21, 32),
    (22, 32),
    (23, 32),
    (24, 32),
    (25, 4),
    (26, 4),
    (27, 4),
    (28, 4),
];

/// One arm nominated by the selection receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominatedConfiguration {
    /// Selection-receipt arm label (`A` or `B`).
    pub arm: &'static str,
    /// Backend vocabulary used by the selection receipt.
    pub manifest_backend: &'static str,
    /// Harness backend that implements the nominated arm.
    pub backend: Backend,
}

/// One exact campaign cell and all configurations nominated there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignCell {
    /// Prime-field order.
    pub q: u64,
    /// Square-matrix order.
    pub n: usize,
    /// Arms present in the authoritative cell row, in A/B order.
    pub configurations: Vec<NominatedConfiguration>,
}

/// One committed probe used in a cell's conservative cost projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeUse {
    /// Evaluated backend whose cost this probe represents.
    pub backend: Backend,
    /// Same or higher matrix order where the finite probe was observed.
    pub source_n: usize,
    /// Committed `probe_matrix_s` value.
    pub seconds: f64,
}

/// Result of applying the per-cell equivalence budget.
#[derive(Clone, Debug, PartialEq)]
pub struct CellBudget {
    /// Matrix-count ceiling fixed for this order.
    pub ceiling: usize,
    /// Count selected after halving or applying the missing-cost floor.
    pub matrices: usize,
    /// Sum of the available reference-and-arm probe costs.
    pub probe_sum_seconds: f64,
    /// `Some` only when every evaluated backend has a committed cost proxy.
    pub projected_cell_seconds: Option<f64>,
    /// Complete list of committed probes used in the sum.
    pub probes: Vec<ProbeUse>,
    /// Evaluated backends for which no same-or-higher-order probe exists.
    pub missing_backends: Vec<Backend>,
}

#[derive(Clone, Copy)]
struct CommittedProbe {
    q: u64,
    n: usize,
    backend: Backend,
    seconds: f64,
}

/// Finite `probe_matrix_s` observations from the three superseded grid
/// receipts in the tree of commit `de5f7414`:
///
/// - `dev/studies/047b62ed/*-q3-grid.csv`
/// - `dev/studies/91605d4d/*-q5-grid.csv`
/// - `dev/studies/6c7fcb38/*-q7-grid.csv`
///
/// Only backends evaluated by the reference-plus-nomination schedule are
/// retained. Fixed-batch GPU rows after order 12 carry `NaN`, not a probe, and
/// are deliberately absent.
const COMMITTED_PROBES: &[CommittedProbe] = &[
    CommittedProbe {
        q: 3,
        n: 12,
        backend: Backend::Scalar,
        seconds: 0.000_025,
    },
    CommittedProbe {
        q: 3,
        n: 16,
        backend: Backend::Scalar,
        seconds: 0.000_270,
    },
    CommittedProbe {
        q: 3,
        n: 20,
        backend: Backend::Scalar,
        seconds: 0.004_144,
    },
    CommittedProbe {
        q: 3,
        n: 24,
        backend: Backend::Scalar,
        seconds: 0.067_676,
    },
    CommittedProbe {
        q: 3,
        n: 28,
        backend: Backend::Scalar,
        seconds: 1.065_811,
    },
    CommittedProbe {
        q: 3,
        n: 12,
        backend: Backend::Rayon,
        seconds: 0.000_026,
    },
    CommittedProbe {
        q: 3,
        n: 16,
        backend: Backend::Rayon,
        seconds: 0.000_273,
    },
    CommittedProbe {
        q: 3,
        n: 12,
        backend: Backend::RyserGeneric,
        seconds: 0.000_188,
    },
    CommittedProbe {
        q: 3,
        n: 16,
        backend: Backend::RyserGeneric,
        seconds: 0.003_321,
    },
    CommittedProbe {
        q: 3,
        n: 16,
        backend: Backend::RayonIntra,
        seconds: 0.000_246,
    },
    CommittedProbe {
        q: 3,
        n: 20,
        backend: Backend::RayonIntra,
        seconds: 0.000_470,
    },
    CommittedProbe {
        q: 3,
        n: 24,
        backend: Backend::RayonIntra,
        seconds: 0.003_775,
    },
    CommittedProbe {
        q: 3,
        n: 28,
        backend: Backend::RayonIntra,
        seconds: 0.052_472,
    },
    CommittedProbe {
        q: 5,
        n: 12,
        backend: Backend::Scalar,
        seconds: 0.000_157,
    },
    CommittedProbe {
        q: 5,
        n: 16,
        backend: Backend::Scalar,
        seconds: 0.003_163,
    },
    CommittedProbe {
        q: 5,
        n: 20,
        backend: Backend::Scalar,
        seconds: 0.062_510,
    },
    CommittedProbe {
        q: 5,
        n: 24,
        backend: Backend::Scalar,
        seconds: 1.197_137,
    },
    CommittedProbe {
        q: 5,
        n: 12,
        backend: Backend::Rayon,
        seconds: 0.000_161,
    },
    CommittedProbe {
        q: 5,
        n: 16,
        backend: Backend::Rayon,
        seconds: 0.003_192,
    },
    CommittedProbe {
        q: 5,
        n: 20,
        backend: Backend::Rayon,
        seconds: 0.062_734,
    },
    CommittedProbe {
        q: 5,
        n: 24,
        backend: Backend::Rayon,
        seconds: 1.196_064,
    },
    CommittedProbe {
        q: 5,
        n: 12,
        backend: Backend::Gpu,
        seconds: 0.052_684,
    },
    CommittedProbe {
        q: 7,
        n: 12,
        backend: Backend::Scalar,
        seconds: 0.000_164,
    },
    CommittedProbe {
        q: 7,
        n: 16,
        backend: Backend::Scalar,
        seconds: 0.003_210,
    },
    CommittedProbe {
        q: 7,
        n: 12,
        backend: Backend::Rayon,
        seconds: 0.000_161,
    },
    CommittedProbe {
        q: 7,
        n: 16,
        backend: Backend::Rayon,
        seconds: 0.003_210,
    },
    CommittedProbe {
        q: 7,
        n: 12,
        backend: Backend::Gpu,
        seconds: 0.065_682,
    },
    CommittedProbe {
        q: 7,
        n: 20,
        backend: Backend::RyserGeneric,
        seconds: 0.064_976,
    },
];

/// Parse the exact campaign cells and nominated configurations from the
/// embedded selection receipt.
///
/// # Errors
///
/// Returns an error if a cell row is malformed, names an unknown backend,
/// duplicates a cell, omits a campaign cell, introduces an out-of-grid cell,
/// or selects a backend outside its candidate arms.
pub fn campaign_cells() -> Result<Vec<CampaignCell>, String> {
    let mut cells = Vec::new();
    for line in SELECTION_AUTHORITY.lines() {
        let columns = line.split('|').map(str::trim).collect::<Vec<_>>();
        let Some(q) = columns.get(1).and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        if !matches!(q, 3 | 5 | 7) {
            continue;
        }
        let n = columns
            .get(2)
            .ok_or_else(|| format!("selection row has no n column: {line}"))?
            .parse::<usize>()
            .map_err(|error| format!("invalid selection n in `{line}`: {error}"))?;
        if cells
            .iter()
            .any(|cell: &CampaignCell| cell.q == q && cell.n == n)
        {
            return Err(format!("selection receipt duplicates q={q}, n={n}"));
        }

        let selected = tick_value(
            columns
                .get(3)
                .ok_or_else(|| format!("selection row has no selected backend: {line}"))?,
        )?;
        let mut configurations = vec![configuration(
            "A",
            tick_value(
                columns
                    .get(4)
                    .ok_or_else(|| format!("selection row has no arm A: {line}"))?,
            )?,
        )?];
        let arm_b = columns
            .get(5)
            .ok_or_else(|| format!("selection row has no arm B: {line}"))?;
        if !arm_b.starts_with("No second ") {
            configurations.push(configuration("B", tick_value(arm_b)?)?);
        }
        if !configurations
            .iter()
            .any(|configuration| configuration.manifest_backend == selected)
        {
            return Err(format!(
                "q={q}, n={n} selects `{selected}` outside its candidate arms"
            ));
        }
        cells.push(CampaignCell {
            q,
            n,
            configurations,
        });
    }

    cells.sort_by_key(|cell| (cell.q, cell.n));
    let expected = [(3_u64, 28_usize), (5, 24), (7, 20)];
    for (q, max_n) in expected {
        for n in 4..=max_n {
            if !cells.iter().any(|cell| cell.q == q && cell.n == n) {
                return Err(format!(
                    "selection receipt omits campaign cell q={q}, n={n}"
                ));
            }
        }
    }
    if cells.len() != 63 {
        return Err(format!(
            "selection receipt has {} campaign rows; expected 63",
            cells.len()
        ));
    }
    Ok(cells)
}

fn tick_value(value: &'static str) -> Result<&'static str, String> {
    let start = value
        .find('`')
        .ok_or_else(|| format!("selection value has no opening backtick: {value}"))?
        + 1;
    let rest = &value[start..];
    let end = rest
        .find('`')
        .ok_or_else(|| format!("selection value has no closing backtick: {value}"))?;
    Ok(&rest[..end])
}

fn configuration(
    arm: &'static str,
    manifest_backend: &'static str,
) -> Result<NominatedConfiguration, String> {
    let backend = match manifest_backend {
        "batch_parallel" => Backend::Rayon,
        "generic_ryser" => Backend::RyserGeneric,
        "accelerator" => Backend::Gpu,
        "intra_matrix_parallel" => Backend::RayonIntra,
        _ => return Err(format!("unknown nominated backend `{manifest_backend}`")),
    };
    Ok(NominatedConfiguration {
        arm,
        manifest_backend,
        backend,
    })
}

fn order_ceiling(n: usize) -> Result<usize, String> {
    EQUIVALENCE_ORDER_CEILINGS
        .iter()
        .find_map(|(order, ceiling)| (*order == n).then_some(*ceiling))
        .ok_or_else(|| format!("equivalence order {n} is outside the committed order table"))
}

fn conservative_probe(q: u64, n: usize, backend: Backend) -> Option<ProbeUse> {
    COMMITTED_PROBES
        .iter()
        .filter(|probe| probe.q == q && probe.backend == backend && probe.n >= n)
        .min_by_key(|probe| probe.n)
        .map(|probe| ProbeUse {
            backend,
            source_n: probe.n,
            seconds: probe.seconds,
        })
}

/// Halve `ceiling`, flooring at two, until `probe_sum_seconds * matrices`
/// fits the 240-second cell budget.
///
/// # Panics
///
/// Panics if `ceiling` is below two or if `probe_sum_seconds` is negative or
/// non-finite.
#[must_use]
pub fn derive_matrix_count(ceiling: usize, probe_sum_seconds: f64) -> usize {
    assert!(
        ceiling >= EQUIVALENCE_MIN_MATRICES,
        "equivalence ceiling must be at least two"
    );
    assert!(
        probe_sum_seconds.is_finite() && probe_sum_seconds >= 0.0,
        "equivalence probe sum must be finite and nonnegative"
    );
    let mut matrices = ceiling;
    while matrices > EQUIVALENCE_MIN_MATRICES
        && probe_sum_seconds * matrices as f64 > EQUIVALENCE_CELL_BUDGET_SECONDS
    {
        matrices = (matrices / 2).max(EQUIVALENCE_MIN_MATRICES);
    }
    matrices
}

/// Apply the committed per-cell budget to one authoritative campaign cell.
///
/// The evaluated backend set is the reference plus the nominated arms, with a
/// nominated reference counted once. An absent exact-order cost uses the
/// nearest finite observation at the same or a higher order. This is a
/// conservative substitution and never extrapolates from a cheaper lower
/// order. If no such observation exists, the cell takes the two-matrix floor;
/// the harness records the missing backend rather than inventing a latency.
///
/// # Errors
///
/// Returns an error if the cell order has no committed ceiling or if no
/// reference implementation supports the cell.
pub fn cell_budget(cell: &CampaignCell) -> Result<CellBudget, String> {
    let ceiling = order_ceiling(cell.n)?;
    let reference = reference_backend(cell.q, cell.n)
        .ok_or_else(|| format!("q={}, n={} has no reference backend", cell.q, cell.n))?;
    let mut backends = vec![reference];
    for configuration in &cell.configurations {
        if !backends.contains(&configuration.backend) {
            backends.push(configuration.backend);
        }
    }

    let mut probes = Vec::new();
    let mut missing_backends = Vec::new();
    for backend in backends {
        match conservative_probe(cell.q, cell.n, backend) {
            Some(probe) => probes.push(probe),
            None => missing_backends.push(backend),
        }
    }
    let probe_sum_seconds = probes.iter().map(|probe| probe.seconds).sum::<f64>();
    let (matrices, projected_cell_seconds) = if missing_backends.is_empty() {
        let matrices = derive_matrix_count(ceiling, probe_sum_seconds);
        (matrices, Some(probe_sum_seconds * matrices as f64))
    } else {
        (EQUIVALENCE_MIN_MATRICES, None)
    };
    Ok(CellBudget {
        ceiling,
        matrices,
        probe_sum_seconds,
        projected_cell_seconds,
        probes,
        missing_backends,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_table_has_every_cell_and_124_configurations() {
        let cells = campaign_cells().expect("valid embedded selection receipt");
        assert_eq!(cells.len(), 63);
        for (q, cells_expected, configurations_expected) in [(3, 25, 50), (5, 21, 41), (7, 17, 33)]
        {
            assert_eq!(
                cells.iter().filter(|cell| cell.q == q).count(),
                cells_expected
            );
            assert_eq!(
                cells
                    .iter()
                    .filter(|cell| cell.q == q)
                    .map(|cell| cell.configurations.len())
                    .sum::<usize>(),
                configurations_expected
            );
        }
        assert_eq!(
            cells
                .iter()
                .map(|cell| cell.configurations.len())
                .sum::<usize>(),
            124
        );
    }

    #[test]
    fn frontier_cells_preserve_the_single_nominated_arm() {
        let cells = campaign_cells().expect("valid embedded selection receipt");
        for (q, n, expected) in [
            (5, 24, ("A", "batch_parallel", Backend::Rayon)),
            (7, 20, ("A", "accelerator", Backend::Gpu)),
        ] {
            let cell = cells
                .iter()
                .find(|cell| cell.q == q && cell.n == n)
                .expect("frontier cell");
            assert_eq!(cell.configurations.len(), 1);
            assert_eq!(
                (
                    cell.configurations[0].arm,
                    cell.configurations[0].manifest_backend,
                    cell.configurations[0].backend,
                ),
                expected
            );
        }
    }

    #[test]
    fn budget_halving_reproduces_the_de5f7414_expensive_cell() {
        let probe_sum = 22.054_209 + 22.269_200 + 24.030_546;
        assert_eq!(derive_matrix_count(4, probe_sum), 2);
        assert!(probe_sum * 4.0 > EQUIVALENCE_CELL_BUDGET_SECONDS);
        assert!(probe_sum * 2.0 <= EQUIVALENCE_CELL_BUDGET_SECONDS);
    }

    #[test]
    fn campaign_budget_uses_higher_order_probes_and_floor_for_missing_gpu_costs() {
        let cells = campaign_cells().expect("valid embedded selection receipt");
        let q3_n13 = cells
            .iter()
            .find(|cell| cell.q == 3 && cell.n == 13)
            .expect("q3 n13");
        let budget = cell_budget(q3_n13).expect("budget");
        assert_eq!(budget.matrices, 512);
        assert!(budget.missing_backends.is_empty());
        assert!(budget.probes.iter().all(|probe| probe.source_n == 16));

        let q5_n13 = cells
            .iter()
            .find(|cell| cell.q == 5 && cell.n == 13)
            .expect("q5 n13");
        let budget = cell_budget(q5_n13).expect("budget");
        assert_eq!(budget.matrices, EQUIVALENCE_MIN_MATRICES);
        assert_eq!(budget.missing_backends, vec![Backend::Gpu]);
        assert_eq!(budget.projected_cell_seconds, None);
    }

    #[test]
    fn every_campaign_cell_has_the_expected_deterministic_budget() {
        let cells = campaign_cells().expect("valid embedded selection receipt");
        for cell in &cells {
            let expected = match (cell.q, cell.n) {
                (3, 4..=15) | (5, 4..=12) | (7, 4..=12) => 512,
                (5, 24) => 32,
                _ => 2,
            };
            assert_eq!(
                cell_budget(cell).expect("budget").matrices,
                expected,
                "q={} n={}",
                cell.q,
                cell.n
            );
        }
    }
}
