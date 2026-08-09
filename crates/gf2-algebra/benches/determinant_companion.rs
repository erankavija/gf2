//! Raw timing harness for the permanent campaign's determinant companion.
//!
//! The harness deliberately does not use Criterion: its receipt contract needs
//! every raw repetition, multiple fresh-process execution identifiers, and
//! pooled-total ratios computed downstream. Matrix generation and conversion
//! into determinant/permanent representations happen before timed windows.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::{
    permanent_bipedal3, permanent_bipedal5, permanent_bipedal7, permanent_ryser,
};
use gf2_algebra::testutil::random_matrix;
use gf2_core::field::{matrix::FieldMatrix, FieldVec, FiniteField};
use gf2_core::gfp::Fp;

const SCHEMA_VERSION: &str = "determinant-companion-v2";
const SEED_ROOT: u64 = 0x8cb4_def5_0000_0000;
const FIXTURE_COUNT: usize = 32;
const DEFAULT_REPETITIONS: u32 = 5;
const DEFAULT_TARGET_MS: u64 = 250;
const MAX_CALLS: u64 = 1 << 32;

const CELLS: &[(u64, usize)] = &[
    (3, 4),
    (3, 12),
    (3, 20),
    (3, 28),
    (5, 4),
    (5, 12),
    (5, 20),
    (5, 24),
    (7, 4),
    (7, 12),
    (7, 20),
];

#[derive(Debug)]
struct Args {
    execution: u32,
    repetitions: u32,
    target: Duration,
    output: PathBuf,
    append: bool,
    self_check: bool,
}

#[derive(Debug)]
struct Metadata {
    git_revision: String,
    source_dirty: bool,
    rustc: String,
    hostname: String,
    cpu_model: String,
    kernel: String,
    governor: String,
    timestamp_unix_s: u64,
}

struct F3Fixture {
    dense: FieldMatrix<Fp<3>>,
    packed: Bipedal3Matrix,
    row_major: Vec<Fp<3>>,
}

struct F5Fixture {
    dense: FieldMatrix<Fp<5>>,
    packed: Packed5Matrix,
    row_major: Vec<Fp<5>>,
}

struct F7Fixture {
    dense: FieldMatrix<Fp<7>>,
    packed: Option<Packed7Matrix>,
    row_major: Vec<Fp<7>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    if args.self_check {
        return self_check();
    }

    let metadata = collect_metadata()?;
    let mut output = open_output(&args.output, args.append)?;
    if !args.append || output.metadata()?.len() == 0 {
        writeln!(
            output,
            "schema_version,execution,repetition,q,n,operation,backend,seed_root,\
             fixture_count,fixture_start,calls,elapsed_ns,ns_per_matrix,target_ms,timestamp_unix_s,\
             git_revision,source_dirty,rustc,hostname,cpu_model,kernel,governor"
        )?;
    }

    for &(q, n) in CELLS {
        match q {
            3 => measure_f3(&args, &metadata, &mut output, n)?,
            5 => measure_f5(&args, &metadata, &mut output, n)?,
            7 => measure_f7(&args, &metadata, &mut output, n)?,
            _ => unreachable!("the frozen calibration grid contains only F3/F5/F7"),
        }
        output.flush()?;
    }
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut execution = 1;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut target_ms = DEFAULT_TARGET_MS;
    let mut output = None;
    let mut append = false;
    let mut self_check = false;
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--execution" => execution = parse_value(&mut iter, &arg)?,
            "--repetitions" => repetitions = parse_value(&mut iter, &arg)?,
            "--target-ms" => target_ms = parse_value(&mut iter, &arg)?,
            "--output" => output = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--append" => append = true,
            "--self-check" => self_check = true,
            // `cargo bench` appends this libtest compatibility flag even for
            // a `harness = false` target.
            "--bench" => {}
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if repetitions == 0 {
        return Err("--repetitions must be positive".into());
    }
    if execution == 0 {
        return Err("--execution must be positive for recorded windows".into());
    }
    if target_ms == 0 {
        return Err("--target-ms must be positive".into());
    }
    let output = output.unwrap_or_else(|| PathBuf::from("determinant-cost.csv"));
    Ok(Args {
        execution,
        repetitions,
        target: Duration::from_millis(target_ms),
        output,
        append,
        self_check,
    })
}

fn parse_value<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    next_value(iter, flag)?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn open_output(path: &Path, append: bool) -> io::Result<File> {
    let path = resolve_output_path(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(path)
}

fn resolve_output_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

fn cell_seed(q: u64, n: usize) -> u64 {
    SEED_ROOT ^ (q << 48) ^ ((n as u64) << 32)
}

fn dense_matrix<F: FiniteField>(row_major: &[F], n: usize) -> FieldMatrix<F> {
    let rows = row_major
        .chunks_exact(n)
        .map(|row| FieldVec::from(row.to_vec()))
        .collect();
    FieldMatrix::from_rows(rows)
}

fn fixtures_f3(n: usize) -> Vec<F3Fixture> {
    let seed = cell_seed(3, n);
    (0..FIXTURE_COUNT)
        .map(|index| {
            let row_major = random_matrix::<3>(n, seed.wrapping_add(index as u64));
            let dense = dense_matrix(&row_major, n);
            let packed = Bipedal3Matrix::from_row_major(&row_major, n, n);
            F3Fixture {
                dense,
                packed,
                row_major,
            }
        })
        .collect()
}

fn fixtures_f5(n: usize) -> Vec<F5Fixture> {
    let seed = cell_seed(5, n);
    (0..FIXTURE_COUNT)
        .map(|index| {
            let row_major = random_matrix::<5>(n, seed.wrapping_add(index as u64));
            let dense = dense_matrix(&row_major, n);
            let packed = Packed5Matrix::from_row_major(&row_major, n, n);
            F5Fixture {
                dense,
                packed,
                row_major,
            }
        })
        .collect()
}

fn fixtures_f7(n: usize) -> Vec<F7Fixture> {
    let seed = cell_seed(7, n);
    (0..FIXTURE_COUNT)
        .map(|index| {
            let row_major = random_matrix::<7>(n, seed.wrapping_add(index as u64));
            let dense = dense_matrix(&row_major, n);
            let packed = (n <= 16).then(|| Packed7Matrix::from_row_major(&row_major, n, n));
            F7Fixture {
                dense,
                packed,
                row_major,
            }
        })
        .collect()
}

fn run_calls_from(mut call: impl FnMut(usize), calls: u64, fixture_start: usize) -> Duration {
    let start = Instant::now();
    for index in 0..calls {
        call((fixture_start + index as usize) & (FIXTURE_COUNT - 1));
    }
    start.elapsed()
}

fn run_calls(call: impl FnMut(usize), calls: u64) -> Duration {
    run_calls_from(call, calls, 0)
}

fn recorded_fixture_start(execution: u32, repetition: u32, repetitions: u32) -> usize {
    debug_assert!(execution >= 1);
    debug_assert!((1..=repetitions).contains(&repetition));
    (((execution - 1) as usize * repetitions as usize) + (repetition - 1) as usize)
        & (FIXTURE_COUNT - 1)
}

fn calibrated_calls(target: Duration, mut call: impl FnMut(usize)) -> u64 {
    // Calibration is not receipt evidence: it only selects a call count for
    // the recorded windows. Starting at fixture zero and cycling the pool is
    // deterministic and does not consume a recorded execution/repetition
    // address.
    let probe_target = target.min(Duration::from_millis(20));
    let mut calls = 1_u64;
    loop {
        let elapsed = run_calls(&mut call, calls);
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return wanted.clamp(1, MAX_CALLS as u128) as u64;
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

fn measure_f3(args: &Args, metadata: &Metadata, output: &mut File, n: usize) -> io::Result<()> {
    let fixtures = fixtures_f3(n);
    let det_calls = calibrated_calls(args.target, |index| {
        black_box(fixtures[index].dense.det());
    });
    let permanent_calls = calibrated_calls(args.target, |index| {
        black_box(permanent_bipedal3(black_box(&fixtures[index].packed)));
    });
    record_repetitions(
        args,
        metadata,
        output,
        3,
        n,
        "determinant",
        "fieldmatrix_det_ple",
        det_calls,
        |index| black_box(fixtures[index].dense.det()),
    )?;
    record_repetitions(
        args,
        metadata,
        output,
        3,
        n,
        "permanent",
        "permanent_bipedal3_public_scalar_single_matrix",
        permanent_calls,
        |index| black_box(permanent_bipedal3(black_box(&fixtures[index].packed))),
    )
}

fn measure_f5(args: &Args, metadata: &Metadata, output: &mut File, n: usize) -> io::Result<()> {
    let fixtures = fixtures_f5(n);
    let det_calls = calibrated_calls(args.target, |index| {
        black_box(fixtures[index].dense.det());
    });
    let permanent_calls = calibrated_calls(args.target, |index| {
        black_box(permanent_bipedal5(black_box(&fixtures[index].packed)));
    });
    record_repetitions(
        args,
        metadata,
        output,
        5,
        n,
        "determinant",
        "fieldmatrix_det_ple",
        det_calls,
        |index| black_box(fixtures[index].dense.det()),
    )?;
    record_repetitions(
        args,
        metadata,
        output,
        5,
        n,
        "permanent",
        "permanent_bipedal5_public_packed_scalar",
        permanent_calls,
        |index| black_box(permanent_bipedal5(black_box(&fixtures[index].packed))),
    )
}

fn measure_f7(args: &Args, metadata: &Metadata, output: &mut File, n: usize) -> io::Result<()> {
    let fixtures = fixtures_f7(n);
    let det_calls = calibrated_calls(args.target, |index| {
        black_box(fixtures[index].dense.det());
    });
    let (backend, permanent_calls) = if n <= 16 {
        let calls = calibrated_calls(args.target, |index| {
            black_box(permanent_bipedal7(black_box(
                fixtures[index].packed.as_ref().expect("n <= 16 is packed"),
            )));
        });
        ("permanent_bipedal7_public_packed_scalar", calls)
    } else {
        let calls = calibrated_calls(args.target, |index| {
            black_box(permanent_ryser::<Fp<7>>(
                black_box(&fixtures[index].row_major),
                black_box(n),
            ));
        });
        ("permanent_ryser_f7_generic_cpu", calls)
    };
    record_repetitions(
        args,
        metadata,
        output,
        7,
        n,
        "determinant",
        "fieldmatrix_det_ple",
        det_calls,
        |index| black_box(fixtures[index].dense.det()),
    )?;
    if n <= 16 {
        record_repetitions(
            args,
            metadata,
            output,
            7,
            n,
            "permanent",
            backend,
            permanent_calls,
            |index| {
                black_box(permanent_bipedal7(black_box(
                    fixtures[index].packed.as_ref().expect("n <= 16 is packed"),
                )))
            },
        )
    } else {
        record_repetitions(
            args,
            metadata,
            output,
            7,
            n,
            "permanent",
            backend,
            permanent_calls,
            |index| {
                black_box(permanent_ryser::<Fp<7>>(
                    black_box(&fixtures[index].row_major),
                    black_box(n),
                ))
            },
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn record_repetitions<T>(
    args: &Args,
    metadata: &Metadata,
    output: &mut File,
    q: u64,
    n: usize,
    operation: &str,
    backend: &str,
    calls: u64,
    mut call: impl FnMut(usize) -> T,
) -> io::Result<()> {
    for repetition in 1..=args.repetitions {
        let fixture_start =
            recorded_fixture_start(args.execution, repetition, args.repetitions);
        let elapsed = run_calls_from(
            |index| {
                black_box(call(index));
            },
            calls,
            fixture_start,
        );
        let elapsed_ns = elapsed.as_nanos();
        let ns_per_matrix = elapsed_ns as f64 / calls as f64;
        writeln!(
            output,
            "{},{},{},{},{},{},{},{:#018x},{},{},{},{},{:.6},{},{},{},{},{},{},{},{},{},{}",
            SCHEMA_VERSION,
            args.execution,
            repetition,
            q,
            n,
            operation,
            backend,
            SEED_ROOT,
            FIXTURE_COUNT,
            fixture_start,
            calls,
            elapsed_ns,
            ns_per_matrix,
            args.target.as_millis(),
            metadata.timestamp_unix_s,
            csv_field(&metadata.git_revision),
            metadata.source_dirty,
            csv_field(&metadata.rustc),
            csv_field(&metadata.hostname),
            csv_field(&metadata.cpu_model),
            csv_field(&metadata.kernel),
            csv_field(&metadata.governor),
        )?;
    }
    Ok(())
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn command_output(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn collect_metadata() -> io::Result<Metadata> {
    let git_revision = command_output("git", &["rev-parse", "HEAD"])?;
    let status = command_output(
        "git",
        &[
            "status",
            "--porcelain",
            "--untracked-files=no",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "crates/gf2-core",
            "crates/gf2-algebra",
        ],
    )?;
    let rustc = command_output("rustc", &["+1.95.0", "--version"])?;
    let hostname = command_output("hostname", &[])?;
    let kernel = command_output("uname", &["-srvmo"])?;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let cpu_model = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .unwrap_or("unknown")
        .to_owned();
    let governor = fs::read_to_string("/sys/devices/system/cpu/cpu6/cpufreq/scaling_governor")
        .unwrap_or_else(|_| "unknown".to_owned())
        .trim()
        .to_owned();
    let timestamp_unix_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    Ok(Metadata {
        git_revision,
        source_dirty: !status.is_empty(),
        rustc,
        hostname,
        cpu_model,
        kernel,
        governor,
        timestamp_unix_s,
    })
}

fn self_check() -> Result<(), Box<dyn std::error::Error>> {
    let mut unique = CELLS.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        CELLS.len(),
        "calibration cells must be unique"
    );
    assert!(CELLS.contains(&(3, 28)));
    assert!(CELLS.contains(&(5, 24)));
    assert!(CELLS.contains(&(7, 20)));
    assert_eq!(
        resolve_output_path(Path::new("dev/receipt.csv")),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("dev/receipt.csv"),
        "relative receipt paths must resolve from the workspace root"
    );
    let absolute = Path::new("/tmp/determinant-companion-self-check.csv");
    assert_eq!(resolve_output_path(absolute), absolute);
    let mut starts = Vec::new();
    for execution in 1..=5 {
        for repetition in 1..=5 {
            let start = recorded_fixture_start(execution, repetition, 5);
            assert_eq!(start, starts.len());
            starts.push(start);
        }
    }
    starts.sort_unstable();
    starts.dedup();
    assert_eq!(starts.len(), 25, "canonical frontier starts must be unique");

    let f3 = fixtures_f3(4);
    let f5 = fixtures_f5(4);
    let f7 = fixtures_f7(4);
    for fixture in &f3 {
        assert_eq!(fixture.dense.get(2, 3), fixture.row_major[11]);
        assert_eq!(fixture.packed.get(2, 3), fixture.row_major[11]);
        assert_eq!(
            permanent_bipedal3(&fixture.packed),
            permanent_ryser(&fixture.row_major, 4)
        );
    }
    for fixture in &f5 {
        assert_eq!(fixture.dense.get(2, 3), fixture.row_major[11]);
        assert_eq!(fixture.packed.get(2, 3), fixture.row_major[11]);
        assert_eq!(
            permanent_bipedal5(&fixture.packed),
            permanent_ryser(&fixture.row_major, 4)
        );
    }
    for fixture in &f7 {
        let packed = fixture.packed.as_ref().expect("n=4 has packed F7 form");
        assert_eq!(fixture.dense.get(2, 3), fixture.row_major[11]);
        assert_eq!(packed.get(2, 3), fixture.row_major[11]);
        assert_eq!(
            permanent_bipedal7(packed),
            permanent_ryser(&fixture.row_major, 4)
        );
    }
    eprintln!(
        "self-check PASS: {} unique cells, {} paired fixtures per cell",
        CELLS.len(),
        FIXTURE_COUNT
    );
    Ok(())
}
