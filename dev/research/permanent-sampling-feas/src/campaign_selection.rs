//! Campaign-cell backend nominations and equivalence budgets.
//!
//! The backend selection receipt is the authority for both the campaign cell
//! grid and the configurations checked at each cell. It is embedded into the
//! executable and parsed here; this harness never reconstructs a selection
//! from the premeasurement rates.
//!
//! The probe-cost derivation record is the authority for the single-matrix
//! latencies that size each cell's matrix corpus. It is embedded and parsed
//! the same way; no probe figure is written into this source.

use std::sync::OnceLock;

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

/// Repository path of the committed probe-cost derivation record.
pub const PROBE_COSTS_PATH: &str = "dev/benchmarks/permanent_campaign/probe-costs-de5f7414.csv";

/// SHA-256 of [`PROBE_COSTS_PATH`] at the harness revision that embeds it. The
/// executable SHA-256 in each receipt pins these embedded bytes.
pub const PROBE_COSTS_SHA256: &str =
    "525100d986b8c2632b7c378c9059da8709f962c0d93d7b560b3393200debc3f0";

const PROBE_COSTS: &str =
    include_str!("../../../benchmarks/permanent_campaign/probe-costs-de5f7414.csv");

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

/// One probe observation parsed from the committed derivation record.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CommittedProbe {
    q: u64,
    n: usize,
    backend: Backend,
    seconds: f64,
}

/// Header line separating the derivation record's preamble from its rows.
const PROBE_COSTS_HEADER: &str = "q,n,backend,probe_matrix_s";

/// Parse every probe row of a derivation record in [`PROBE_COSTS_PATH`] form.
///
/// Comment lines and blank lines are skipped and the single header line is
/// required. Every other line must be a four-column row naming a builtin
/// backend and a finite, nonnegative latency.
///
/// # Errors
///
/// Returns an error if the header is absent, repeated, or preceded by a row;
/// if a row has the wrong column count, an unparsable number, a non-finite or
/// negative latency, or an unknown backend name; if a `(q, n, backend)` key
/// repeats; or if the record carries no row at all.
fn parse_probe_costs(record: &str) -> Result<Vec<CommittedProbe>, String> {
    let mut probes: Vec<CommittedProbe> = Vec::new();
    let mut header_seen = false;
    for line in record.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line == PROBE_COSTS_HEADER {
            if header_seen {
                return Err("probe-cost record repeats its header".to_string());
            }
            header_seen = true;
            continue;
        }
        if !header_seen {
            return Err(format!("probe-cost row precedes the header: {line}"));
        }
        let columns = line.split(',').collect::<Vec<_>>();
        if columns.len() != 4 {
            return Err(format!(
                "probe-cost row has {} columns; expected 4: {line}",
                columns.len()
            ));
        }
        let q = columns[0]
            .parse::<u64>()
            .map_err(|error| format!("invalid probe-cost q in `{line}`: {error}"))?;
        let n = columns[1]
            .parse::<usize>()
            .map_err(|error| format!("invalid probe-cost n in `{line}`: {error}"))?;
        let backend = probe_backend(columns[2])?;
        let seconds = columns[3]
            .parse::<f64>()
            .map_err(|error| format!("invalid probe-cost seconds in `{line}`: {error}"))?;
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(format!(
                "probe-cost seconds must be finite and nonnegative: {line}"
            ));
        }
        if probes
            .iter()
            .any(|probe| probe.q == q && probe.n == n && probe.backend == backend)
        {
            return Err(format!(
                "probe-cost record duplicates q={q}, n={n}, backend `{}`",
                columns[2]
            ));
        }
        probes.push(CommittedProbe {
            q,
            n,
            backend,
            seconds,
        });
    }
    if !header_seen {
        return Err(format!(
            "probe-cost record has no `{PROBE_COSTS_HEADER}` header"
        ));
    }
    if probes.is_empty() {
        return Err("probe-cost record has no probe rows".to_string());
    }
    Ok(probes)
}

/// Resolve a derivation-record backend name against [`Backend::BUILTIN`].
///
/// The record uses the measurement vocabulary of [`Backend::name`], so the
/// builtin list is the only name table; this function never adds a second one.
fn probe_backend(name: &str) -> Result<Backend, String> {
    Backend::BUILTIN
        .iter()
        .copied()
        .find(|backend| backend.name() == name)
        .ok_or_else(|| format!("unknown probe backend `{name}`"))
}

/// The probe observations of the embedded derivation record, parsed once.
fn committed_probes() -> Result<&'static [CommittedProbe], String> {
    static PROBES: OnceLock<Result<Vec<CommittedProbe>, String>> = OnceLock::new();
    PROBES
        .get_or_init(|| parse_probe_costs(PROBE_COSTS))
        .as_deref()
        .map_err(Clone::clone)
}

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

fn conservative_probe(
    committed: &[CommittedProbe],
    q: u64,
    n: usize,
    backend: Backend,
) -> Option<ProbeUse> {
    committed
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
/// Returns an error if the embedded probe-cost record is malformed, if the
/// cell order has no committed ceiling, or if no reference implementation
/// supports the cell.
pub fn cell_budget(cell: &CampaignCell) -> Result<CellBudget, String> {
    let committed = committed_probes()?;
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
        match conservative_probe(committed, cell.q, cell.n, backend) {
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
        assert_eq!(cells.len(), 63);
        let mut at_ceiling = 0usize;
        let mut at_floor = 0usize;
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
            at_ceiling += usize::from(expected == 512);
            at_floor += usize::from(expected == EQUIVALENCE_MIN_MATRICES);
        }
        assert_eq!(at_ceiling, 30);
        assert_eq!(at_floor, 32);

        // The floor cells are exactly q=3 n=16..28, q=5 n=13..23, q=7 n=13..20.
        for (q, floor_orders) in [(3_u64, 16..=28), (5, 13..=23), (7, 13..=20)] {
            for cell in cells.iter().filter(|cell| cell.q == q) {
                let budget = cell_budget(cell).expect("budget");
                assert_eq!(
                    budget.matrices == EQUIVALENCE_MIN_MATRICES,
                    floor_orders.contains(&cell.n),
                    "q={q} n={} took {} matrices",
                    cell.n,
                    budget.matrices
                );
            }
        }
    }

    #[test]
    fn embedded_probe_costs_match_the_committed_sha256() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("sha256sum is the digest tool this harness already uses for provenance");
        child
            .stdin
            .take()
            .expect("sha256sum stdin")
            .write_all(PROBE_COSTS.as_bytes())
            .expect("pipe the embedded record to sha256sum");
        let output = child.wait_with_output().expect("sha256sum output");
        assert!(
            output.status.success(),
            "sha256sum exited {}",
            output.status
        );
        let printed = String::from_utf8(output.stdout).expect("sha256sum prints UTF-8");
        let digest = printed
            .split_whitespace()
            .next()
            .expect("sha256sum prints a digest");
        assert_eq!(
            digest, PROBE_COSTS_SHA256,
            "{PROBE_COSTS_PATH} changed without its embedded SHA-256"
        );
    }

    #[test]
    fn the_derivation_record_parses_to_the_committed_probe_set() {
        let probes = committed_probes().expect("valid embedded probe-cost record");
        assert_eq!(probes.len(), 28);
        for (q, expected) in [(3_u64, 13_usize), (5, 9), (7, 6)] {
            assert_eq!(probes.iter().filter(|probe| probe.q == q).count(), expected);
        }
        for (q, n, backend, seconds) in [
            (3_u64, 12_usize, Backend::Scalar, 0.000_025),
            (3, 28, Backend::RayonIntra, 0.052_472),
            (5, 12, Backend::Gpu, 0.052_684),
            (5, 24, Backend::Rayon, 1.196_064),
            (7, 20, Backend::RyserGeneric, 0.064_976),
        ] {
            let probe = probes
                .iter()
                .find(|probe| probe.q == q && probe.n == n && probe.backend == backend)
                .unwrap_or_else(|| panic!("q={q}, n={n} probe for {}", backend.name()));
            assert_eq!(probe.seconds, seconds);
        }

        // Fixed-batch GPU rows after order 12 carry NaN, not a probe, so the
        // record holds no GPU observation above that order.
        assert!(probes
            .iter()
            .all(|probe| probe.backend != Backend::Gpu || probe.n == 12));
    }

    #[test]
    fn the_probe_cost_parser_rejects_malformed_records() {
        let header = PROBE_COSTS_HEADER;
        for (label, record) in [
            ("short row", format!("{header}\n3,12,cpu_scalar\n")),
            (
                "long row",
                format!("{header}\n3,12,cpu_scalar,0.000025,7\n"),
            ),
            (
                "non-numeric n",
                format!("{header}\n3,twelve,cpu_scalar,0.000025\n"),
            ),
            (
                "non-numeric seconds",
                format!("{header}\n3,12,cpu_scalar,fast\n"),
            ),
            (
                "non-finite seconds",
                format!("{header}\n3,12,cpu_scalar,NaN\n"),
            ),
            (
                "negative seconds",
                format!("{header}\n3,12,cpu_scalar,-0.1\n"),
            ),
            (
                "duplicate key",
                format!("{header}\n3,12,cpu_scalar,0.000025\n3,12,cpu_scalar,0.000026\n"),
            ),
            (
                "row before header",
                format!("3,12,cpu_scalar,0.000025\n{header}\n"),
            ),
            ("no header", "3,12,cpu_scalar,0.000025\n".to_string()),
            ("no rows", format!("# preamble only\n{header}\n")),
        ] {
            assert!(
                parse_probe_costs(&record).is_err(),
                "parser accepted a record with a {label}"
            );
        }
    }

    #[test]
    fn the_probe_cost_parser_rejects_an_unknown_backend() {
        let record = format!("{PROBE_COSTS_HEADER}\n3,12,cpu_quantum,0.000025\n");
        let error = parse_probe_costs(&record).expect_err("unknown backend must be rejected");
        assert!(
            error.contains("cpu_quantum"),
            "error must name the rejected backend: {error}"
        );
    }
}
