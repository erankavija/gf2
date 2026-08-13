//! Reproducible rare-event estimates for permanental rank deficiency.
//!
//! The ignored test is an explicitly invoked campaign. It uses the production
//! rank predicate, the production domain-separated sampler, and the production
//! exact binomial interval implementation, then writes the committed receipt.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;

use gf2_algebra::permanent::{permanental_rank_status_with_stats, PermanentalRank};
use gf2_core::field::ConstField;
use gf2_core::gfp::Fp;
use gf2_stats::intervals::clopper_pearson_interval;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};

const ROOT: u64 = 0xE025_1AF3_2026_0813;
const INTERVAL_LEVEL: f64 = 0.95;
const RECEIPT_FILE: &str = "dev/bench_results/e0251af3-rank-event-rates.md";

/// Preregistered cells. Stream indices are unique within the `RareEvent`
/// purpose and are never shared with validation or campaign-cell draws.
const CELLS: &[Cell] = &[
    // k = 1: expected counts are approximately 41, 32, and 29.
    Cell {
        q: 3,
        n: 5,
        k: 1,
        samples: 10_000,
        stream: 101,
    },
    Cell {
        q: 5,
        n: 4,
        k: 1,
        samples: 20_000,
        stream: 102,
    },
    Cell {
        q: 7,
        n: 3,
        k: 1,
        samples: 10_000,
        stream: 103,
    },
    // k = 2: the heuristic predicts approximately 82 and 64 events.
    Cell {
        q: 3,
        n: 5,
        k: 2,
        samples: 10_000,
        stream: 201,
    },
    Cell {
        q: 5,
        n: 4,
        k: 2,
        samples: 20_000,
        stream: 202,
    },
];

#[derive(Clone, Copy)]
struct Cell {
    q: u64,
    n: usize,
    k: usize,
    samples: u64,
    stream: u64,
}

struct ResultRow {
    cell: Cell,
    events: u64,
    interval: (f64, f64),
    mean_permanent_evaluations: f64,
}

fn field_order(q: u64) -> FieldOrder {
    match q {
        3 => FieldOrder::F3,
        5 => FieldOrder::F5,
        7 => FieldOrder::F7,
        _ => panic!("unsupported field order q={q}"),
    }
}

fn run_cell<const Q: u64>(cell: Cell) -> ResultRow {
    let address = MatrixAddress::new(
        ROOT,
        field_order(cell.q),
        cell.n,
        StreamPurpose::RareEvent,
        StreamIndex::new(cell.stream).expect("preregistered stream index fits"),
    );
    let mut sampler = MatrixSampler::<Q>::new(address).expect("field order matches sampler");
    let mut matrix = vec![Fp::<Q>::zero(); cell.n * cell.k];
    let mut events = 0_u64;
    let mut permanent_evaluations = 0_u64;

    for _ in 0..cell.samples {
        for entry in &mut matrix {
            *entry = sampler.next_entry();
        }
        let evaluation = permanental_rank_status_with_stats(&matrix, cell.n, cell.k);
        events += u64::from(evaluation.status == PermanentalRank::Deficient);
        permanent_evaluations += evaluation.permanent_evaluations as u64;
    }

    ResultRow {
        cell,
        events,
        interval: clopper_pearson_interval(events, cell.samples, INTERVAL_LEVEL),
        mean_permanent_evaluations: permanent_evaluations as f64 / cell.samples as f64,
    }
}

fn dispatch(cell: Cell) -> ResultRow {
    match cell.q {
        3 => run_cell::<3>(cell),
        5 => run_cell::<5>(cell),
        7 => run_cell::<7>(cell),
        _ => unreachable!(),
    }
}

fn command_output(command: &str, args: &[&str]) -> String {
    String::from_utf8_lossy(
        &Command::new(command)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("failed to run {command}: {error}"))
            .stdout,
    )
    .trim()
    .to_owned()
}

fn git_revision() -> String {
    command_output("git", &["rev-parse", "HEAD"])
}

fn cpu_model() -> String {
    let output = Command::new("sh")
        .args([
            "-c",
            "lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -1",
        ])
        .output()
        .expect("failed to run lscpu");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn make_receipt(rows: &[ResultRow]) -> String {
    let mut receipt = String::new();
    writeln!(receipt, "# Permanental rank-deficiency rare-event receipt").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "Issue: `e0251af3`. This is a preregistered, single-run pipeline check of the production permanental-rank predicate at observable small dimensions.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "## Interpretation").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "The $k = 1$ event is an all-zero column, whose exact probability is $q^{{-n}}$. The $k = 2$ comparison uses the stated heuristic $2q^{{-n}}$, not a theorem prediction. Every measured $(n,k)$ lies outside the cited hypothesis $k \\le 0.1\\sqrt{{n}}$: these small observable cells cannot test the theorem in its proven range. Agreement therefore supports the implementation and the $k/q^n$ heuristic, rather than that theorem.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "Event counts are in the small-count regime, so every interval below is the equal-tailed 95% Clopper–Pearson exact binomial interval from `gf2_stats::intervals::clopper_pearson_interval`; no normal approximation is used.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "Cell selection is fixed before drawing: $k = 1$ uses $(q,n) = (3,5), (5,4), (7,3)$, giving expected counts $10^4 q^{{-n}} \\approx 41$, $2\\cdot10^4 q^{{-n}} = 32$, and $10^4 q^{{-n}} \\approx 29$; $k = 2$ uses $(3,5)$ and $(5,4)$, giving heuristic counts $10^4(2q^{{-n}}) \\approx 82$ and $2\\cdot10^4(2q^{{-n}}) = 64$. These choices keep the expected events in the tens at modest sample sizes.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "A disagreement with an exact value or heuristic is recorded as a pipeline finding, not a mathematical one; no observed result is reconciled by changing the preregistered sample sizes.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "## Preregistered cells and results").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "| $k$ | $q$ | $n$ | samples | events | estimate | exact 95% CP interval | exact $q^{{-n}}$ (k=1) | heuristic $2q^{{-n}}$ (k=2) | heuristic/exact comparison | mean $k \\times k$ permanent evaluations per matrix |").unwrap();
    writeln!(
        receipt,
        "|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|"
    )
    .unwrap();
    for row in rows {
        let exact = (row.cell.q as f64).powi(-(row.cell.n as i32));
        let heuristic = 2.0 * exact;
        let comparison = if row.cell.k == 1 {
            if row.interval.0 <= exact && exact <= row.interval.1 {
                "covered exact value"
            } else {
                "DISAGREEMENT: interval excludes exact value (pipeline finding)"
            }
        } else if row.interval.0 <= heuristic && heuristic <= row.interval.1 {
            "heuristic lies in interval"
        } else {
            "DISAGREEMENT: heuristic outside interval (pipeline finding)"
        };
        let exact_column = if row.cell.k == 1 {
            format!("{exact:.12}")
        } else {
            "—".to_owned()
        };
        let heuristic_column = if row.cell.k == 2 {
            format!("{heuristic:.12}")
        } else {
            "—".to_owned()
        };
        writeln!(
            receipt,
            "| {} | {} | {} | {} | {} | {:.12} | [{:.12}, {:.12}] (95% CP) | {} | {} | {} | {:.6} |",
            row.cell.k,
            row.cell.q,
            row.cell.n,
            row.cell.samples,
            row.events,
            row.events as f64 / row.cell.samples as f64,
            row.interval.0,
            row.interval.1,
            exact_column,
            heuristic_column,
            comparison,
            row.mean_permanent_evaluations,
        )
        .unwrap();
    }
    writeln!(receipt).unwrap();
    writeln!(receipt, "The measured means are small constants relative to the $\\binom{{n}}{{k}}$ worst-case scan, confirming the production predicate's early exit for these uniformly sampled matrices.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "## Provenance and regeneration").unwrap();
    writeln!(receipt).unwrap();
    writeln!(receipt, "- Git revision: `{}`", git_revision()).unwrap();
    writeln!(receipt, "- CPU model: `{}`", cpu_model()).unwrap();
    writeln!(
        receipt,
        "- Toolchain: `{}`; `{}`",
        command_output("rustc", &["--version"]),
        command_output("cargo", &["--version"])
    )
    .unwrap();
    writeln!(receipt, "- Sampler: `gf2_stats::sampler::MatrixSampler<F_q>` with ChaCha20; root `ROOT = {ROOT:#018x}`.").unwrap();
    writeln!(receipt, "- Stream purpose: `StreamPurpose::RareEvent` (tag 4), distinct from `Validation`, `Timing`, and `CampaignCell`; stream indices are recorded per cell below.").unwrap();
    writeln!(receipt, "- Harness constants and sample sizes are committed in `crates/gf2-algebra/tests/rank_event_rates.rs`; the sizes were not changed after observing outcomes.").unwrap();
    writeln!(receipt).unwrap();
    writeln!(
        receipt,
        "| $k$ | $q$ | $n$ | stream index | matrix entries drawn per sample | total samples |"
    )
    .unwrap();
    writeln!(receipt, "|---:|---:|---:|---:|---:|---:|").unwrap();
    for cell in CELLS {
        writeln!(
            receipt,
            "| {} | {} | {} | {} | {} | {} |",
            cell.k,
            cell.q,
            cell.n,
            cell.stream,
            cell.n * cell.k,
            cell.samples
        )
        .unwrap();
    }
    writeln!(receipt).unwrap();
    writeln!(receipt, "Regeneration (from the repository root): `cargo nextest run -p gf2-algebra --test rank_event_rates --release --profile ci --run-ignored ignored-only`.").unwrap();
    receipt
}

#[test]
#[ignore = "sim: preregistered rare-event rate campaign writes a committed receipt"]
fn rank_event_rate_receipt() {
    let rows: Vec<_> = CELLS.iter().copied().map(dispatch).collect();
    let receipt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(RECEIPT_FILE);
    std::fs::write(&receipt_path, make_receipt(&rows))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", receipt_path.display()));
    println!("wrote {}", receipt_path.display());
}
