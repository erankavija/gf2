//! Raw timing harness for the permanent campaign's determinant companion.
//!
//! The harness deliberately does not use Criterion: its receipt contract needs
//! every raw repetition, multiple fresh-process execution identifiers, and
//! pooled totals computed downstream. Matrix generation and conversion into
//! `FieldMatrix` happen before timed windows.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gf2_algebra::testutil::random_matrix;
use gf2_core::field::{matrix::FieldMatrix, FieldVec, FiniteField};
use gf2_core::gfp::Fp;

const SCHEMA_VERSION: &str = "determinant-companion-v3";
const BACKEND: &str = "fieldmatrix_det_ple";
const SEED_ROOT: u64 = 0xec22_205e_0000_0000;
const FIXTURE_COUNT: usize = 32;
const PROCESS_COUNT: u32 = 5;
const TIMED_REPETITIONS: u32 = 5;
const TARGET_MS: u64 = 250;
const WARMUP_PROBE_MS: u64 = 20;
const WARMUP_POLICY: &str = "doubling-probe-to-min-20ms-target-250ms-max-2^32";
const MAX_CALLS: u64 = 1 << 32;

#[derive(Debug)]
struct Args {
    execution: u32,
    output: PathBuf,
    self_check: bool,
}

#[derive(Debug)]
struct Calibration {
    timed_calls: u64,
    warmup_calls: u64,
    warmup_elapsed: Duration,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    if args.self_check {
        return self_check();
    }

    let mut output = open_output(&args.output)?;
    writeln!(
        output,
        "schema_version,process_index,q,n,backend,seed_root,cell_seed,fixture_count,\
         fixture_starts,warmup_policy,warmup_calls,warmup_elapsed_ns,target_ms,\
         timed_repetitions,calls_per_repetition,repetition_elapsed_ns,sample_count,\
         elapsed_determinant_ns,ns_per_matrix,started_unix_ns,finished_unix_ns"
    )?;

    for (q, n) in campaign_cells() {
        match q {
            3 => measure_cell::<3>(&args, &mut output, n)?,
            5 => measure_cell::<5>(&args, &mut output, n)?,
            7 => measure_cell::<7>(&args, &mut output, n)?,
            _ => unreachable!("the frozen campaign grid contains only F3/F5/F7"),
        }
        output.flush()?;
    }
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut execution = None;
    let mut output = None;
    let mut self_check = false;
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--execution" => execution = Some(parse_value(&mut iter, &arg)?),
            "--output" => output = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--self-check" => self_check = true,
            // `cargo bench` appends this libtest compatibility flag even for
            // a `harness = false` target.
            "--bench" => {}
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if self_check {
        return Ok(Args {
            execution: 1,
            output: PathBuf::new(),
            self_check,
        });
    }
    let execution = execution.ok_or("--execution is required")?;
    if !(1..=PROCESS_COUNT).contains(&execution) {
        return Err(format!("--execution must be between 1 and {PROCESS_COUNT}"));
    }
    let output = output.ok_or("--output is required")?;
    Ok(Args {
        execution,
        output,
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

fn open_output(path: &Path) -> io::Result<File> {
    let path = resolve_output_path(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    OpenOptions::new().create_new(true).write(true).open(path)
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

fn campaign_cells() -> Vec<(u64, usize)> {
    [(3, 4..=28), (5, 4..=24), (7, 4..=20)]
        .into_iter()
        .flat_map(|(q, sizes)| sizes.map(move |n| (q, n)))
        .collect()
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

fn fixtures<const P: u64>(n: usize) -> Vec<FieldMatrix<Fp<P>>>
where
    Fp<P>: FiniteField,
{
    let seed = cell_seed(P, n);
    (0..FIXTURE_COUNT)
        .map(|index| {
            let row_major = random_matrix::<P>(n, seed.wrapping_add(index as u64));
            dense_matrix(&row_major, n)
        })
        .collect()
}

fn run_calls_from<T>(
    mut call: impl FnMut(usize) -> T,
    calls: u64,
    fixture_start: usize,
) -> Duration {
    let start = Instant::now();
    for index in 0..calls {
        black_box(call((fixture_start + index as usize) & (FIXTURE_COUNT - 1)));
    }
    start.elapsed()
}

fn recorded_fixture_start(execution: u32, repetition: u32) -> usize {
    debug_assert!((1..=PROCESS_COUNT).contains(&execution));
    debug_assert!((1..=TIMED_REPETITIONS).contains(&repetition));
    ((execution - 1) * TIMED_REPETITIONS + (repetition - 1)) as usize
}

fn calibrate<T>(target: Duration, mut call: impl FnMut(usize) -> T) -> Calibration {
    let probe_target = target.min(Duration::from_millis(WARMUP_PROBE_MS));
    let mut calls = 1_u64;
    let mut warmup_calls = 0_u64;
    let mut warmup_elapsed = Duration::ZERO;
    loop {
        let elapsed = run_calls_from(&mut call, calls, 0);
        warmup_calls = warmup_calls.saturating_add(calls);
        warmup_elapsed = warmup_elapsed.saturating_add(elapsed);
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return Calibration {
                timed_calls: wanted.clamp(1, MAX_CALLS as u128) as u64,
                warmup_calls,
                warmup_elapsed,
            };
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

fn unix_time_ns() -> io::Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(io::Error::other)
}

fn measure_cell<const P: u64>(args: &Args, output: &mut File, n: usize) -> io::Result<()>
where
    Fp<P>: FiniteField,
{
    let matrices = fixtures::<P>(n);
    let started_unix_ns = unix_time_ns()?;
    let target = Duration::from_millis(TARGET_MS);
    let calibration = calibrate(target, |index| matrices[index].det());
    let mut repetition_elapsed = Vec::with_capacity(TIMED_REPETITIONS as usize);
    let mut starts = Vec::with_capacity(TIMED_REPETITIONS as usize);
    for repetition in 1..=TIMED_REPETITIONS {
        let fixture_start = recorded_fixture_start(args.execution, repetition);
        starts.push(fixture_start);
        repetition_elapsed.push(run_calls_from(
            |index| matrices[index].det(),
            calibration.timed_calls,
            fixture_start,
        ));
    }
    let finished_unix_ns = unix_time_ns()?;
    let elapsed_ns: u128 = repetition_elapsed.iter().map(Duration::as_nanos).sum();
    let sample_count = calibration.timed_calls * u64::from(TIMED_REPETITIONS);
    let ns_per_matrix = elapsed_ns as f64 / sample_count as f64;
    let start_list = starts
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(";");
    let elapsed_list = repetition_elapsed
        .iter()
        .map(|duration| duration.as_nanos().to_string())
        .collect::<Vec<_>>()
        .join(";");
    writeln!(
        output,
        "{SCHEMA_VERSION},{},{P},{n},{BACKEND},{SEED_ROOT:#018x},{:#018x},{FIXTURE_COUNT},\
         {start_list},{WARMUP_POLICY},{},{},{TARGET_MS},{TIMED_REPETITIONS},{},\
         {elapsed_list},{sample_count},{elapsed_ns},{ns_per_matrix:.9},{started_unix_ns},\
         {finished_unix_ns}",
        args.execution,
        cell_seed(P, n),
        calibration.warmup_calls,
        calibration.warmup_elapsed.as_nanos(),
        calibration.timed_calls,
    )
}

fn self_check() -> Result<(), Box<dyn std::error::Error>> {
    let cells = campaign_cells();
    let mut unique = cells.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 63, "campaign grid must contain 63 cells");
    assert_eq!(unique.len(), cells.len(), "campaign cells must be unique");
    assert_eq!(cells.first(), Some(&(3, 4)));
    assert_eq!(cells.last(), Some(&(7, 20)));

    let mut starts = Vec::new();
    for execution in 1..=PROCESS_COUNT {
        for repetition in 1..=TIMED_REPETITIONS {
            starts.push(recorded_fixture_start(execution, repetition));
        }
    }
    let mut unique_starts = starts.clone();
    unique_starts.sort_unstable();
    unique_starts.dedup();
    assert_eq!(starts, (0..25).collect::<Vec<_>>());
    assert_eq!(unique_starts.len(), 25);

    let f3 = fixtures::<3>(4);
    let f5 = fixtures::<5>(4);
    let f7 = fixtures::<7>(4);
    assert_eq!(f3.len(), FIXTURE_COUNT);
    assert_eq!(f5.len(), FIXTURE_COUNT);
    assert_eq!(f7.len(), FIXTURE_COUNT);
    black_box(f3[0].det());
    black_box(f5[0].det());
    black_box(f7[0].det());
    eprintln!(
        "self-check PASS: {} unique cells, {} unique recorded starts per cell",
        cells.len(),
        unique_starts.len()
    );
    Ok(())
}
