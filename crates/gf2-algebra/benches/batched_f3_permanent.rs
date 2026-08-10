//! Raw timing harness for the four-matrix F_3 permanent AVX2 receipt.
//!
//! Criterion is deliberately not used: the receipt needs retained raw windows,
//! fresh-process execution identifiers, pooled-total rates, and independent
//! within- and across-process dispersion calculations. Matrix construction is
//! complete before a timed window; each invocation evaluates the same four
//! deterministic matrices through exactly one of the three named paths.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::bipedal3::{
    permanent_bipedal3_batch, permanent_bipedal3_singleword, permanent_bipedal3_singleword_simd,
};
use gf2_algebra::testutil::random_matrix;
use gf2_kernels_simd::bipedal::BipedalAvx2Fns;

const SCHEMA_VERSION: &str = "batched-f3-avx2-v1";
const SEED_ROOT: u64 = 0xddd0_c6ee_0000_0000;
const FIXTURE_COUNT: usize = 32;
const BATCH_WIDTH: usize = 4;
const DEFAULT_REPETITIONS: u32 = 5;
const DEFAULT_TARGET_MS: u64 = 250;
const MAX_CALLS: u64 = 1 << 32;

/// The established one-word permanent benchmark group. Keeping the receipt
/// on this exact set makes its per-size statements auditable against the
/// existing `permanent_bipedal3` benchmark protocol.
const CELLS: &[usize] = &[8, 12, 16, 20, 24, 28];

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug)]
enum Backend {
    BatchedAvx2,
    ScalarSingleword,
    SingleMatrixAvx2,
}

impl Backend {
    const ALL: [Self; 3] = [
        Self::BatchedAvx2,
        Self::ScalarSingleword,
        Self::SingleMatrixAvx2,
    ];

    const fn tag(self) -> &'static str {
        match self {
            Self::BatchedAvx2 => "permanent_bipedal3_batch_avx2_four_matrix",
            Self::ScalarSingleword => "permanent_bipedal3_scalar_singleword_four_matrix",
            Self::SingleMatrixAvx2 => "permanent_bipedal3_single_matrix_avx2_four_matrix",
        }
    }
}

type Fixture = [Bipedal3Matrix; BATCH_WIDTH];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    if args.self_check {
        return self_check();
    }

    let Some(fns) = gf2_kernels_simd::bipedal::detect_avx2() else {
        return Err(
            "AVX2 is required: refusing to publish a scalar fallback as an AVX2 measurement".into(),
        );
    };
    let metadata = collect_metadata()?;
    let mut output = open_output(&args.output, args.append)?;
    if !args.append || output.metadata()?.len() == 0 {
        writeln!(
            output,
            "schema_version,execution,repetition,n,backend,seed_root,fixture_count,fixture_start,\
             calls,matrices,elapsed_ns,ns_per_matrix,target_ms,timestamp_unix_s,git_revision,\
             source_dirty,rustc,hostname,cpu_model,kernel,governor"
        )?;
    }

    for &n in CELLS {
        let fixtures = fixtures(n);
        correctness_probe(&fixtures, &fns, n);
        for backend in Backend::ALL {
            let calls = calibrated_calls(args.target, |index| {
                run_backend(backend, &fixtures[index], &fns)
            });
            record_repetitions(
                &args,
                &metadata,
                &mut output,
                n,
                backend,
                calls,
                &fixtures,
                &fns,
            )?;
        }
        output.flush()?;
    }
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut execution = 1;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut target_ms = DEFAULT_TARGET_MS;
    let mut output = None;
    let mut append = false;
    let mut self_check = false;
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--execution" => execution = parse_value(&mut iter, &arg)?,
            "--repetitions" => repetitions = parse_value(&mut iter, &arg)?,
            "--target-ms" => target_ms = parse_value(&mut iter, &arg)?,
            "--output" => output = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--append" => append = true,
            "--self-check" => self_check = true,
            // `cargo bench` adds this compatibility flag to a `harness = false` target.
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
    Ok(Args {
        execution,
        repetitions,
        target: Duration::from_millis(target_ms),
        output: output.unwrap_or_else(|| PathBuf::from("batched-f3-avx2.csv")),
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
    if !append && path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite existing raw receipt: {}",
                path.display()
            ),
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(false)
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

fn cell_seed(n: usize, fixture: usize, lane: usize) -> u64 {
    SEED_ROOT ^ ((n as u64) << 32) ^ ((fixture as u64) << 8) ^ lane as u64
}

fn fixtures(n: usize) -> Vec<Fixture> {
    (0..FIXTURE_COUNT)
        .map(|fixture| {
            std::array::from_fn(|lane| {
                let entries = random_matrix::<3>(n, cell_seed(n, fixture, lane));
                Bipedal3Matrix::from_row_major(&entries, n, n)
            })
        })
        .collect()
}

fn correctness_probe(fixtures: &[Fixture], fns: &BipedalAvx2Fns, n: usize) {
    let matrices = &fixtures[0];
    let scalar: Vec<_> = matrices.iter().map(permanent_bipedal3_singleword).collect();
    let direct: Vec<_> = matrices
        .iter()
        .map(|matrix| permanent_bipedal3_singleword_simd(matrix, fns))
        .collect();
    let batched = permanent_bipedal3_batch(matrices);
    assert_eq!(direct, scalar, "direct AVX2/scalar mismatch at n={n}");
    assert_eq!(batched, scalar, "batched AVX2/scalar mismatch at n={n}");
}

fn run_backend(backend: Backend, fixture: &Fixture, fns: &BipedalAvx2Fns) {
    match backend {
        Backend::BatchedAvx2 => {
            black_box(permanent_bipedal3_batch(black_box(fixture)));
        }
        Backend::ScalarSingleword => {
            for matrix in fixture {
                black_box(permanent_bipedal3_singleword(black_box(matrix)));
            }
        }
        Backend::SingleMatrixAvx2 => {
            for matrix in fixture {
                black_box(permanent_bipedal3_singleword_simd(black_box(matrix), fns));
            }
        }
    }
}

fn run_calls(
    backend: Backend,
    fixtures: &[Fixture],
    fns: &BipedalAvx2Fns,
    calls: u64,
    fixture_start: usize,
) -> Duration {
    let start = Instant::now();
    for call in 0..calls {
        let index = (fixture_start + call as usize) & (FIXTURE_COUNT - 1);
        run_backend(backend, &fixtures[index], fns);
    }
    start.elapsed()
}

fn calibrated_calls(target: Duration, mut call: impl FnMut(usize)) -> u64 {
    let probe_target = target.min(Duration::from_millis(20));
    let mut calls = 1_u64;
    loop {
        let start = Instant::now();
        for index in 0..calls {
            call((index as usize) & (FIXTURE_COUNT - 1));
        }
        let elapsed = start.elapsed();
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return wanted.clamp(1, MAX_CALLS as u128) as u64;
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

#[allow(clippy::too_many_arguments)]
fn record_repetitions(
    args: &Args,
    metadata: &Metadata,
    output: &mut File,
    n: usize,
    backend: Backend,
    calls: u64,
    fixtures: &[Fixture],
    fns: &BipedalAvx2Fns,
) -> io::Result<()> {
    for repetition in 1..=args.repetitions {
        let fixture_start = recorded_fixture_start(args.execution, repetition, args.repetitions);
        let elapsed = run_calls(backend, fixtures, fns, calls, fixture_start);
        let matrices = calls * BATCH_WIDTH as u64;
        let elapsed_ns = elapsed.as_nanos();
        let ns_per_matrix = elapsed_ns as f64 / matrices as f64;
        writeln!(
            output,
            "{},{},{},{},{},{:#018x},{},{},{},{},{},{:.6},{},{},{},{},{},{},{},{},{}",
            SCHEMA_VERSION,
            args.execution,
            repetition,
            n,
            backend.tag(),
            SEED_ROOT,
            FIXTURE_COUNT,
            fixture_start,
            calls,
            matrices,
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

fn recorded_fixture_start(execution: u32, repetition: u32, repetitions: u32) -> usize {
    debug_assert!(execution >= 1);
    debug_assert!((1..=repetitions).contains(&repetition));
    (((execution - 1) as usize * repetitions as usize) + (repetition - 1) as usize)
        & (FIXTURE_COUNT - 1)
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
            "crates/gf2-algebra",
            "crates/gf2-kernels-simd",
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
    assert_eq!(CELLS, &[8, 12, 16, 20, 24, 28]);
    assert!(CELLS.iter().all(|&n| n <= 63));
    assert_eq!(BATCH_WIDTH, 4);
    assert!(FIXTURE_COUNT.is_power_of_two());
    assert_eq!(cell_seed(8, 0, 0), SEED_ROOT ^ (8_u64 << 32));
    assert_ne!(cell_seed(8, 0, 0), cell_seed(8, 0, 1));
    assert_eq!(
        resolve_output_path(Path::new("dev/receipt.csv")),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("dev/receipt.csv")
    );
    let absolute = Path::new("/tmp/batched-f3-avx2-self-check.csv");
    assert_eq!(resolve_output_path(absolute), absolute);
    let mut starts = Vec::new();
    for execution in 1..=5 {
        for repetition in 1..=5 {
            starts.push(recorded_fixture_start(execution, repetition, 5));
        }
    }
    starts.sort_unstable();
    starts.dedup();
    assert_eq!(starts.len(), 25, "canonical starts must be unique");
    eprintln!(
        "self-check PASS: {} one-word cells, {} four-matrix fixtures per cell",
        CELLS.len(),
        FIXTURE_COUNT
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)] // `cargo bench` enables `cfg(test)` without a test harness.
    use super::*;

    #[test]
    fn parser_accepts_recorded_command_shape() {
        let args = parse_args(
            [
                "--execution",
                "3",
                "--repetitions",
                "5",
                "--target-ms",
                "250",
                "--output",
                "dev/benchmarks/permanent_campaign/batched-f3-avx2.csv",
                "--append",
                "--bench",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("recorded command arguments parse");
        assert_eq!(args.execution, 3);
        assert_eq!(args.repetitions, 5);
        assert_eq!(args.target, Duration::from_millis(250));
        assert!(args.append);
    }

    #[test]
    fn parser_rejects_zero_recording_dimensions() {
        assert!(parse_args(["--execution", "0"].into_iter().map(str::to_owned)).is_err());
        assert!(parse_args(["--repetitions", "0"].into_iter().map(str::to_owned)).is_err());
        assert!(parse_args(["--target-ms", "0"].into_iter().map(str::to_owned)).is_err());
    }

    #[test]
    fn raw_windows_have_unique_fixture_starts() {
        let starts: Vec<_> = (1..=5)
            .flat_map(|execution| {
                (1..=5).map(move |repetition| recorded_fixture_start(execution, repetition, 5))
            })
            .collect();
        let mut unique = starts.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), starts.len());
    }
}
