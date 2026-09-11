//! Generated tables of the byte-field survey's receipts (jit:6c6b09b1).
//!
//! Every figure the findings cite is written here from committed receipt
//! bytes, never copied by hand. For each receipt directory the generator
//! renders:
//!
//! * the cell table: pairs, each arm's median nanoseconds per call over its
//!   executions with a distribution-free interval, and the speedup of
//!   medians with the bootstrap interval the acceptance tool evaluated
//!   (re-derived here through the shared `abtest` bootstrap and required to
//!   agree), its relative half-width, the outcome and the flagged windows;
//! * the selected path each arm reported;
//! * the conversion and setup costs each arm reported beside its windows,
//!   as the median over executions of the per-execution probe medians with a
//!   distribution-free interval;
//! * for every whole-consumer cell with a kernel-isolated twin, the ratio of
//!   each arm's whole-consumer median to its kernel-isolated median, with a
//!   percentile bootstrap over both cells' executions.
//!
//! Interval methods:
//!
//! * Median of n per-execution values: the order-statistic interval
//!   `[x(k), x(n+1-k)]` with the largest `k` whose binomial coverage
//!   `1 - 2 P(Bin(n, 1/2) <= k - 1)` is at least 0.95; the coverage is
//!   printed. Fewer than six values admit no such interval and are marked
//!   descriptive.
//! * Speedup of medians: `abtest::paired_bootstrap_speedup` over whole pairs
//!   at the corrected alpha of the acceptance summary, seeded with
//!   `abtest::bootstrap_seed(campaign_seed, cell key)`, the protocol's own
//!   estimator.
//! * Cross-cell ratio of medians: 10000 percentile-bootstrap replicates, each
//!   resampling both cells' per-execution values independently with
//!   replacement, nearest-rank 2.5 % and 97.5 % quantiles. The generator is
//!   the shared `abtest::SplitMix64` seeded with the first eight bytes of
//!   SHA-256 over the two cell identifiers. Descriptive: no decision uses it.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use tuning_campaign_support::abtest::{
    bootstrap_seed, paired_bootstrap_speedup, PairedObservation, SplitMix64,
};
use tuning_campaign_support::protocol::sha256_hex;
use tuning_campaign_support::receipt::{BenchmarkReceipt, CellRecord, CellStatus, ExecutionRecord};

/// Coverage a distribution-free median interval must reach.
const MEDIAN_COVERAGE: f64 = 0.95;
/// Replicates of the cross-cell ratio bootstrap.
const RATIO_RESAMPLES: usize = 10_000;

/// Median of a nonempty sample.
pub fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// `P(Bin(n, 1/2) <= k)`.
fn binomial_half_cdf(n: usize, k: usize) -> f64 {
    let mut term = 0.5f64.powi(n as i32);
    let mut total = term;
    for i in 1..=k.min(n) {
        term *= (n + 1 - i) as f64 / i as f64;
        total += term;
    }
    total
}

/// A distribution-free interval for the median and its exact coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MedianInterval {
    pub median: f64,
    pub lower: f64,
    pub upper: f64,
    pub coverage: f64,
}

/// The order-statistic interval of the module documentation, or `None` when
/// no order statistics reach [`MEDIAN_COVERAGE`].
pub fn median_interval(values: &[f64]) -> Option<MedianInterval> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let mut chosen = None;
    for k in 1..=n / 2 {
        let coverage = 1.0 - 2.0 * binomial_half_cdf(n, k - 1);
        if coverage >= MEDIAN_COVERAGE {
            chosen = Some((k, coverage));
        } else {
            break;
        }
    }
    chosen.map(|(k, coverage)| MedianInterval {
        median: median(&sorted),
        lower: sorted[k - 1],
        upper: sorted[n - k],
        coverage,
    })
}

/// Percentile-bootstrap interval of `median(a) / median(b)` for two
/// independent samples, as the module documentation states.
pub fn ratio_interval(a: &[f64], b: &[f64], seed_label: &str) -> (f64, f64, f64) {
    let digest = Sha256::digest(seed_label.as_bytes());
    let mut word = [0u8; 8];
    word.copy_from_slice(&digest[..8]);
    let mut rng = SplitMix64::new(u64::from_le_bytes(word));
    let mut draw = |sample: &[f64], scratch: &mut Vec<f64>| {
        scratch.clear();
        for _ in 0..sample.len() {
            let index = (rng.next_u64() % sample.len() as u64) as usize;
            scratch.push(sample[index]);
        }
        median(scratch)
    };
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut replicates: Vec<f64> = (0..RATIO_RESAMPLES)
        .map(|_| draw(a, &mut left) / draw(b, &mut right))
        .collect();
    replicates.sort_by(f64::total_cmp);
    let rank = |q: f64| {
        let position = (q * RATIO_RESAMPLES as f64).ceil() as usize;
        replicates[position.clamp(1, RATIO_RESAMPLES) - 1]
    };
    (median(a) / median(b), rank(0.025), rank(0.975))
}

fn values(cell: &CellRecord, pick: impl Fn(&ExecutionRecord) -> f64, baseline: bool) -> Vec<f64> {
    cell.pairs
        .iter()
        .map(|pair| {
            pick(if baseline {
                &pair.baseline
            } else {
                &pair.candidate
            })
        })
        .collect()
}

fn format_ns(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn format_median(values: &[f64]) -> String {
    match median_interval(values) {
        Some(interval) => format!(
            "{} [{}, {}]",
            format_ns(interval.median),
            format_ns(interval.lower),
            format_ns(interval.upper)
        ),
        None if values.is_empty() => "none".to_owned(),
        None => format!(
            "{} (descriptive, n = {})",
            format_ns(median(values)),
            values.len()
        ),
    }
}

fn coverage_note(values: &[f64]) -> String {
    match median_interval(values) {
        Some(interval) => format!("{:.4}", interval.coverage),
        None => "none".to_owned(),
    }
}

/// Renders one receipt directory.
fn render_receipt(dir: &Path, out: &mut String) -> Result<(), String> {
    let receipt_bytes =
        fs::read(dir.join("receipt.json")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let receipt = BenchmarkReceipt::decode(&receipt_bytes)?;
    let summary_bytes = fs::read(dir.join("acceptance-summary.json"))
        .map_err(|e| format!("{}: acceptance summary: {e}", dir.display()))?;
    let summary: Value =
        serde_json::from_slice(&summary_bytes).map_err(|e| format!("summary: {e}"))?;
    let summary_cells: BTreeMap<String, &Value> = summary["cells"]
        .as_array()
        .ok_or("acceptance summary has no cells")?
        .iter()
        .filter_map(|cell| Some((cell["cell_id"].as_str()?.to_owned(), cell)))
        .collect();

    let _ = writeln!(out, "## `{}`\n", receipt.campaign_id);
    let _ = writeln!(
        out,
        "- Receipt: `{}/receipt.json`, SHA-256 `{}`; acceptance summary SHA-256 `{}`.",
        dir.display(),
        sha256_hex(&receipt_bytes),
        sha256_hex(&summary_bytes)
    );
    let family_id = if receipt.family_id.is_empty() {
        "not recorded by this protocol version".to_owned()
    } else {
        format!("`{}`", receipt.family_id)
    };
    let _ = writeln!(
        out,
        "- Label `{}`, family {family_id}, verdict `{}`, qualifies `{}`, {} finding(s), \
         sessions {}.",
        summary["label"].as_str().unwrap_or("?"),
        summary["verdict"].as_str().unwrap_or("?"),
        summary["qualifies"],
        summary["findings"].as_array().map_or(0, Vec::len),
        summary["sessions"]
    );
    let family = &summary["family"];
    let _ = writeln!(
        out,
        "- Family comparisons m = {}, per-comparison confidence {} (bootstrap resamples {}).",
        family["comparisons"], family["per_comparison_confidence"], family["bootstrap_resamples"]
    );
    let host = &receipt.host;
    let governors: Vec<&String> = host
        .governors
        .values()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let _ = writeln!(
        out,
        "- Host `{}`: {}, kernel `{}`, SMT {:?}, governors {:?}; toolchain `{}`.\n",
        host.hostname,
        host.cpu_model,
        host.os_kernel,
        host.smt_active,
        governors,
        receipt.toolchain
    );

    let _ = writeln!(out, "### Cells\n");
    let _ = writeln!(
        out,
        "Baseline and candidate: median ns/call over the cell's executions [distribution-free \
         interval]. Speedup: median baseline over median candidate, above one favours the \
         candidate, [bootstrap interval at the confidence shown]. Half-width: the larger \
         distance from the estimate to an endpoint, relative to the estimate.\n"
    );
    let _ = writeln!(
        out,
        "| Cell | Pairs | Baseline arm | Baseline ns/call | Candidate arm | Candidate ns/call | \
         Speedup [interval] | Confidence | Half-width | Outcome | Flagged windows |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|");
    let mut median_coverages = BTreeMap::new();
    for cell in &receipt.cells {
        let accepted = summary_cells
            .get(&cell.cell_id)
            .ok_or_else(|| format!("summary lacks cell {}", cell.cell_id))?;
        let outcome = accepted["outcome"].as_str().unwrap_or("?");
        if cell.status == CellStatus::Unavailable {
            let _ = writeln!(
                out,
                "| `{}` | 0 | `{}` | none | `{}` | none | unavailable: {} | | | {outcome} | |",
                cell.cell_id,
                cell.baseline_arm,
                cell.candidate_arm,
                cell.unavailable_reason
                    .as_deref()
                    .unwrap_or("no reason recorded")
            );
            continue;
        }
        let baseline = values(cell, |e| e.ns_per_call, true);
        let candidate = values(cell, |e| e.ns_per_call, false);
        median_coverages.insert(baseline.len(), coverage_note(&baseline));
        let interval = &accepted["interval"];
        let (estimate, lower, upper) = (
            interval["estimate"].as_f64().ok_or("interval estimate")?,
            interval["lower"].as_f64().ok_or("interval lower")?,
            interval["upper"].as_f64().ok_or("interval upper")?,
        );
        if let Some(alpha) = interval["alpha"].as_f64() {
            let pairs: Vec<PairedObservation> = cell
                .pairs
                .iter()
                .map(|pair| PairedObservation {
                    baseline_ns_per_call: pair.baseline.ns_per_call,
                    candidate_ns_per_call: pair.candidate.ns_per_call,
                })
                .collect();
            let recomputed = paired_bootstrap_speedup(
                &pairs,
                receipt.settings.bootstrap_resamples,
                alpha,
                bootstrap_seed(receipt.campaign_seed, &cell.key),
            )
            .map_err(|e| format!("cell {}: {e:?}", cell.cell_id))?;
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
            if !(close(recomputed.estimate, estimate)
                && close(recomputed.lower, lower)
                && close(recomputed.upper, upper))
            {
                return Err(format!(
                    "cell {}: the acceptance interval differs from the shared bootstrap",
                    cell.cell_id
                ));
            }
        }
        let half_width = (estimate - lower).abs().max((upper - estimate).abs()) / estimate;
        let _ = writeln!(
            out,
            "| `{}` | {} | `{}` | {} | `{}` | {} | {:.4} [{:.4}, {:.4}] | {} | {:.4} | {outcome} | {}/{} |",
            cell.cell_id,
            cell.pairs.len(),
            cell.baseline_arm,
            format_median(&baseline),
            cell.candidate_arm,
            format_median(&candidate),
            estimate,
            lower,
            upper,
            interval["confidence"],
            half_width,
            accepted["flagged_windows"],
            accepted["total_windows"]
        );
    }
    let _ = writeln!(
        out,
        "\nMedian interval coverage by execution count: {}.\n",
        median_coverages
            .iter()
            .map(|(n, coverage)| format!("n = {n}: {coverage}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let _ = writeln!(out, "### Selected paths\n");
    let _ = writeln!(out, "| Cell | Baseline path | Candidate path |");
    let _ = writeln!(out, "|---|---|---|");
    for cell in &receipt.cells {
        let paths = |baseline: bool| {
            let mut seen: Vec<String> = cell
                .pairs
                .iter()
                .filter_map(|pair| {
                    if baseline {
                        pair.baseline.selected_path.clone()
                    } else {
                        pair.candidate.selected_path.clone()
                    }
                })
                .collect();
            seen.sort();
            seen.dedup();
            seen.iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            cell.cell_id,
            paths(true),
            paths(false)
        );
    }

    let reports_costs = receipt.cells.iter().any(|cell| {
        cell.pairs
            .iter()
            .any(|pair| pair.baseline.conversion.is_some() || pair.candidate.conversion.is_some())
    });
    if reports_costs {
        let _ = writeln!(out, "\n### Conversion and setup costs\n");
        let _ = writeln!(
            out,
            "Each execution measures every conversion outside its timing windows as the median of \
             repeated probes (`survey/arm-common/src/lib.rs`, `probe_ns`). Entries: median over the \
             cell's executions [distribution-free interval], ns; `0` marks a step the arm does not \
             perform. Kernel-isolated cells exclude these steps from their windows; whole-consumer \
             cells repeat them inside every timed call.\n"
        );
        let _ = writeln!(
            out,
            "| Cell | Arm | Setup | Pack | Unpack | Table/generator preparation | Dispatch |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|");
        for cell in &receipt.cells {
            for (baseline, arm) in [(true, &cell.baseline_arm), (false, &cell.candidate_arm)] {
                let costs: Vec<_> = cell
                    .pairs
                    .iter()
                    .filter_map(|pair| {
                        if baseline {
                            pair.baseline.conversion
                        } else {
                            pair.candidate.conversion
                        }
                    })
                    .collect();
                if costs.is_empty() {
                    continue;
                }
                let column =
                    |pick: fn(&tuning_campaign_support::receipt::ConversionCosts) -> u64| {
                        let sample: Vec<f64> = costs.iter().map(|c| pick(c) as f64).collect();
                        if sample.iter().all(|v| *v == 0.0) {
                            "0".to_owned()
                        } else {
                            format_median(&sample)
                        }
                    };
                let _ = writeln!(
                    out,
                    "| `{}` | `{arm}` | {} | {} | {} | {} | {} |",
                    cell.cell_id,
                    column(|c| c.setup_ns),
                    column(|c| c.pack_ns),
                    column(|c| c.unpack_ns),
                    column(|c| c.batch_fill_ns),
                    column(|c| c.dispatch_ns)
                );
            }
        }
    } else {
        let _ = writeln!(
            out,
            "\nNo execution in this receipt reports conversion costs."
        );
    }

    let twins: Vec<(&CellRecord, &CellRecord)> = receipt
        .cells
        .iter()
        .filter(|cell| cell.cell_id.contains("-whole-"))
        .filter_map(|whole| {
            let isolated = whole.cell_id.replacen("-whole-", "-", 1);
            receipt
                .cells
                .iter()
                .find(|cell| cell.cell_id == isolated)
                .map(|cell| (whole, cell))
        })
        .collect();
    if !twins.is_empty() {
        let _ = writeln!(out, "\n### Whole-consumer over kernel-isolated\n");
        let _ = writeln!(
            out,
            "Ratio of one arm's whole-consumer median to its kernel-isolated median in the twin \
             cell of the same shape [95 % percentile bootstrap over both cells' executions, \
             module documentation of `survey/analysis/src/tables.rs`]. Descriptive: the two \
             cells ran in separate executions and no decision uses the ratio.\n"
        );
        let _ = writeln!(
            out,
            "| Whole-consumer cell | Kernel-isolated cell | Arm | Ratio [interval] | Executions |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|");
        for (whole, isolated) in twins {
            for baseline in [true, false] {
                let arm = if baseline {
                    &whole.baseline_arm
                } else {
                    &whole.candidate_arm
                };
                let a = values(whole, |e| e.ns_per_call, baseline);
                let b = values(isolated, |e| e.ns_per_call, baseline);
                if a.is_empty() || b.is_empty() {
                    continue;
                }
                let label = format!("{}|{}|{arm}", whole.cell_id, isolated.cell_id);
                let (ratio, lower, upper) = ratio_interval(&a, &b, &label);
                let _ = writeln!(
                    out,
                    "| `{}` | `{}` | `{arm}` | {ratio:.3} [{lower:.3}, {upper:.3}] | {} and {} |",
                    whole.cell_id,
                    isolated.cell_id,
                    a.len(),
                    b.len()
                );
            }
        }
    }
    let _ = writeln!(out);
    Ok(())
}

/// Writes the tables of every receipt directory to `output`.
pub fn tables(output: &str, receipts: &[&str]) -> Result<(), String> {
    let mut out = String::new();
    let _ = writeln!(out, "# Byte-field survey tables (jit:6c6b09b1)\n");
    let _ = writeln!(
        out,
        "Generated by `survey-analysis tables` (`dev/active/6c6b09b1/survey/analysis/src/\
         tables.rs`) from the receipt directories below; do not edit by hand. The interval \
         methods are stated in that module's documentation and above each table.\n"
    );
    let _ = writeln!(
        out,
        "Command: `survey-analysis tables {output} {}`\n",
        receipts.join(" ")
    );
    for dir in receipts {
        render_receipt(Path::new(dir), &mut out)?;
    }
    fs::write(output, out).map_err(|e| format!("{output}: {e}"))
}

/// Prints each measured cell's relative bootstrap half-width and the widest
/// one, at the corrected alpha of the pilot's acceptance summary and, when
/// given, at a stricter per-comparison alpha such as a planned
/// confirmation's.
///
/// This is the quantity P-03 recomputes from the pilot's raw pairs: a frozen
/// `effect.measurement_resolution` must be at least the widest half-width,
/// and every margin must strictly exceed one plus the resolution.
pub fn resolution(dir: &str, stricter_alpha: Option<f64>) -> Result<(), String> {
    let receipt = BenchmarkReceipt::decode(
        &fs::read(Path::new(dir).join("receipt.json")).map_err(|e| format!("{dir}: {e}"))?,
    )?;
    let summary: Value = serde_json::from_slice(
        &fs::read(Path::new(dir).join("acceptance-summary.json"))
            .map_err(|e| format!("{dir}: {e}"))?,
    )
    .map_err(|e| format!("summary: {e}"))?;
    let pilot_alpha = 1.0
        - summary["family"]["per_comparison_confidence"]
            .as_f64()
            .ok_or("summary lacks the per-comparison confidence")?;
    let mut alphas = vec![("pilot", pilot_alpha)];
    if let Some(alpha) = stricter_alpha {
        alphas.push(("stricter", alpha));
    }
    for (label, alpha) in alphas {
        let mut widest: Option<(f64, &str)> = None;
        for cell in receipt.cells.iter().filter(|c| !c.pairs.is_empty()) {
            let pairs: Vec<PairedObservation> = cell
                .pairs
                .iter()
                .map(|pair| PairedObservation {
                    baseline_ns_per_call: pair.baseline.ns_per_call,
                    candidate_ns_per_call: pair.candidate.ns_per_call,
                })
                .collect();
            let interval = paired_bootstrap_speedup(
                &pairs,
                receipt.settings.bootstrap_resamples,
                alpha,
                bootstrap_seed(receipt.campaign_seed, &cell.key),
            )
            .map_err(|e| format!("cell {}: {e}", cell.cell_id))?;
            let half_width = (interval.estimate - interval.lower)
                .abs()
                .max((interval.upper - interval.estimate).abs())
                / interval.estimate;
            println!(
                "{label:8} alpha {alpha:.6} {:<44} {half_width:.6}",
                cell.cell_id
            );
            if widest.is_none_or(|(current, _)| half_width > current) {
                widest = Some((half_width, cell.cell_id.as_str()));
            }
        }
        let (width, cell) = widest.ok_or("no measured cell")?;
        println!("{label:8} alpha {alpha:.6} widest {width:.6} in {cell}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_interval_uses_the_widest_order_statistics_that_reach_coverage() {
        let values: Vec<f64> = (1..=12).map(f64::from).collect();
        let interval = median_interval(&values).expect("twelve values admit an interval");
        // k = 3 covers 1 - 2 * 79/4096; k = 4 would cover only 1 - 2 * 299/4096.
        assert_eq!((interval.lower, interval.upper), (3.0, 10.0));
        assert!((interval.coverage - (1.0 - 2.0 * 79.0 / 4096.0)).abs() < 1e-12);
        assert_eq!(interval.median, 6.5);
    }

    #[test]
    fn too_few_values_admit_no_median_interval() {
        assert!(median_interval(&[1.0, 2.0, 3.0, 4.0, 5.0]).is_none());
        assert!(median_interval(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).is_some());
    }

    #[test]
    fn ratio_interval_is_deterministic_and_brackets_the_estimate() {
        let a: Vec<f64> = (0..12).map(|i| 200.0 + f64::from(i)).collect();
        let b: Vec<f64> = (0..12).map(|i| 100.0 + f64::from(i) / 2.0).collect();
        let first = ratio_interval(&a, &b, "whole|isolated|arm");
        assert_eq!(first, ratio_interval(&a, &b, "whole|isolated|arm"));
        assert!(first.1 <= first.0 && first.0 <= first.2);
    }
}
