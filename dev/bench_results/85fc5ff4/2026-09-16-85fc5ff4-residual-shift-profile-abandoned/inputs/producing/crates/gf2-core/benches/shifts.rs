//! Benchmark and protocol-v4 survey harness for `BitVec` shifts.
//!
//! Ordinary Criterion invocation retains the historical shift suites. Setting
//! `GF2_SHIFT_ARM` makes the executable a canonical A/B-runner child for the
//! residual-shift workload profile, while `--verify` checks both production
//! paths against an independent zero-fill oracle without running a timer.

use criterion::{black_box, criterion_group, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::{CacheState, DecoderCell, FamilyAddendum, RunnerPlan};
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::{execution_windows_fixed_or_calibrated, FIXTURE_BANKS};
use tuning_campaign_support::transport;

const ARM_VAR: &str = "GF2_SHIFT_ARM";
const REQUEST_SCHEMA: &str = "zen3-benchmark-arm-request-v1";
const RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Direction {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArmMode {
    ResidualProduction,
    WordAlignedControl,
}

impl ArmMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "residual-production" => Ok(Self::ResidualProduction),
            "word-aligned-control" => Ok(Self::WordAlignedControl),
            other => Err(format!("unknown shift arm {other:?}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::ResidualProduction => "residual-production",
            Self::WordAlignedControl => "word-aligned-control",
        }
    }

    fn offset(self, case: &ShiftCase) -> usize {
        match self {
            Self::ResidualProduction => case.residual_offset,
            Self::WordAlignedControl => case.control_offset,
        }
    }

    fn selected_path(self, direction: Direction) -> &'static str {
        match (self, direction) {
            (Self::ResidualProduction, Direction::Left) => "bitvec-residual-scalar-left",
            (Self::ResidualProduction, Direction::Right) => "bitvec-residual-scalar-right",
            (Self::WordAlignedControl, Direction::Left) => "bitvec-word-dispatch-left",
            (Self::WordAlignedControl, Direction::Right) => "bitvec-word-dispatch-right",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ShiftCase {
    direction: Direction,
    length_bits: usize,
    residual_offset: usize,
    control_offset: usize,
    seed: u64,
}

impl ShiftCase {
    fn validate(&self) -> Result<(), String> {
        if self.residual_offset.is_multiple_of(64) {
            return Err("residual_offset must exercise the non-word-aligned path".to_owned());
        }
        if self.control_offset == 0 || !self.control_offset.is_multiple_of(64) {
            return Err("control_offset must exercise a non-zero word-aligned path".to_owned());
        }
        if self.residual_offset >= self.length_bits || self.control_offset >= self.length_bits {
            return Err("profile offsets must be smaller than the vector length".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmRequest {
    schema: String,
    cell_id: String,
    arm: String,
    role: String,
    pair: u32,
    case: ShiftCase,
    cache_state: CacheState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cold_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoder: Option<DecoderCell>,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

#[derive(Serialize)]
struct ArmOutput {
    schema: &'static str,
    windows: Vec<WindowRecord>,
    cache_state_applied: CacheState,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<&'static str>,
    conversion: Option<ConversionCosts>,
    quality: Option<Value>,
    calibrated: bool,
}

#[derive(Serialize)]
struct ValidationReport {
    schema: &'static str,
    issue: &'static str,
    passed: bool,
    fixture_cases: usize,
    lengths: &'static [usize],
    base_offsets: &'static [usize],
    seeds: &'static [u64],
    directions: [&'static str; 2],
    in_place_aliasing: bool,
    vector_lane_crossings: bool,
    incomplete_final_words: bool,
    offsets_at_and_beyond_length: bool,
    zero_tail_padding: bool,
    failures: Vec<String>,
}

const VERIFY_LENGTHS: &[usize] = &[
    0, 1, 7, 8, 63, 64, 65, 66, 127, 128, 129, 255, 256, 257, 511, 512, 513,
];
const VERIFY_OFFSETS: &[usize] = &[0, 1, 7, 8, 63, 64, 65];
const VERIFY_SEEDS: &[u64] = &[0, 1, 0x85fc_5ff4_c04d_d4ac];

fn fixture(length_bits: usize, seed: u64) -> BitVec {
    let mut generator = SplitMix64::new(seed);
    let mut words: Vec<u64> = (0..length_bits.div_ceil(64))
        .map(|_| generator.next_u64())
        .collect();
    if !length_bits.is_multiple_of(64) {
        if let Some(last) = words.last_mut() {
            *last &= (1_u64 << (length_bits % 64)) - 1;
        }
    }
    BitVec::from_words(words, length_bits)
}

fn apply_shift(vector: &mut BitVec, direction: Direction, offset: usize) {
    match direction {
        Direction::Left => vector.shift_left(offset),
        Direction::Right => vector.shift_right(offset),
    }
}

fn reference_shift(input: &BitVec, direction: Direction, offset: usize) -> Vec<bool> {
    let bits: Vec<bool> = (0..input.len()).map(|index| input.get(index)).collect();
    (0..bits.len())
        .map(|index| match direction {
            Direction::Left => index.checked_sub(offset).is_some_and(|source| bits[source]),
            Direction::Right => index
                .checked_add(offset)
                .filter(|source| *source < bits.len())
                .is_some_and(|source| bits[source]),
        })
        .collect()
}

fn tail_is_zero(vector: &BitVec) -> bool {
    let used = vector.len() % 64;
    used == 0 || vector.words().last().is_none_or(|last| last >> used == 0)
}

fn verify_one(length: usize, offset: usize, seed: u64, direction: Direction) -> Result<(), String> {
    let mut observed = fixture(length, seed);
    let expected = reference_shift(&observed, direction, offset);
    apply_shift(&mut observed, direction, offset);
    if observed.len() != length {
        return Err(format!(
            "{direction:?} length={length} offset={offset} seed={seed}: length changed"
        ));
    }
    if let Some(index) = (0..length).find(|index| observed.get(*index) != expected[*index]) {
        return Err(format!(
            "{direction:?} length={length} offset={offset} seed={seed}: bit {index} differs"
        ));
    }
    if !tail_is_zero(&observed) {
        return Err(format!(
            "{direction:?} length={length} offset={offset} seed={seed}: tail padding is non-zero"
        ));
    }
    Ok(())
}

fn validation_report() -> ValidationReport {
    let mut failures = Vec::new();
    let mut fixture_cases = 0;
    for &length in VERIFY_LENGTHS {
        let mut offsets = BTreeSet::from_iter(VERIFY_OFFSETS.iter().copied());
        offsets.insert(length);
        offsets.insert(length.saturating_add(1));
        offsets.insert(length.saturating_add(65));
        for offset in offsets {
            for &seed in VERIFY_SEEDS {
                for direction in [Direction::Left, Direction::Right] {
                    fixture_cases += 1;
                    if let Err(error) = verify_one(length, offset, seed, direction) {
                        failures.push(error);
                    }
                }
            }
        }
    }
    ValidationReport {
        schema: "bitvec-shift-profile-validation-v1",
        issue: "85fc5ff4",
        passed: failures.is_empty(),
        fixture_cases,
        lengths: VERIFY_LENGTHS,
        base_offsets: VERIFY_OFFSETS,
        seeds: VERIFY_SEEDS,
        directions: ["left", "right"],
        in_place_aliasing: true,
        vector_lane_crossings: VERIFY_LENGTHS.contains(&257)
            && VERIFY_LENGTHS.contains(&513)
            && VERIFY_OFFSETS.contains(&65),
        incomplete_final_words: VERIFY_LENGTHS
            .iter()
            .any(|length| !length.is_multiple_of(64)),
        offsets_at_and_beyond_length: true,
        zero_tail_padding: failures
            .iter()
            .all(|failure| !failure.contains("tail padding")),
        failures,
    }
}

fn fixture_banks(cache_state: CacheState, case: &ShiftCase) -> Result<Vec<BitVec>, String> {
    let count = match cache_state {
        CacheState::Warm => 1,
        CacheState::Streaming => FIXTURE_BANKS,
        CacheState::Cold => {
            return Err("the residual-shift profile declares no cold cells".to_owned())
        }
    };
    Ok((0..count)
        .map(|bank| fixture(case.length_bits, case.seed ^ bank as u64))
        .collect())
}

fn run_arm(request: ArmRequest, mode: ArmMode) -> Result<ArmOutput, String> {
    if request.schema != REQUEST_SCHEMA {
        return Err(format!(
            "request schema {:?} is not {REQUEST_SCHEMA}",
            request.schema
        ));
    }
    if request.arm != mode.name() {
        return Err(format!(
            "request arm {:?} differs from selected {}",
            request.arm,
            mode.name()
        ));
    }
    if request.role != "exploratory" {
        return Err("the residual-shift profile is exploratory only".to_owned());
    }
    if request.workers_declared != 1 {
        return Err(format!(
            "the shift arm is single-worker; the cell declares {}",
            request.workers_declared
        ));
    }
    if request.decoder.is_some() || request.cold_calls.is_some() {
        return Err("shift cells have neither decoder nor cold-call settings".to_owned());
    }
    request.case.validate()?;
    let offset = mode.offset(&request.case);
    let expected = {
        let initial = fixture(request.case.length_bits, request.case.seed);
        reference_shift(&initial, request.case.direction, offset)
    };
    let mut probe = fixture(request.case.length_bits, request.case.seed);
    apply_shift(&mut probe, request.case.direction, offset);
    if (0..probe.len()).any(|index| probe.get(index) != expected[index]) || !tail_is_zero(&probe) {
        return Err("selected arm failed the independent zero-fill oracle".to_owned());
    }

    let mut fixtures = fixture_banks(request.cache_state, &request.case)?;
    if request.cache_state == CacheState::Warm {
        apply_shift(&mut fixtures[0], request.case.direction, offset);
    }
    let bank_count = fixtures.len();
    let mut body = |bank: usize| {
        let vector = black_box(&mut fixtures[bank % bank_count]);
        apply_shift(vector, request.case.direction, offset);
        black_box(vector.words());
    };
    let samples = execution_windows_fixed_or_calibrated(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        None,
        &mut body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .map_err(|error| format!("cannot observe affinity: {error}"))?;
    Ok(ArmOutput {
        schema: RESULT_SCHEMA,
        windows: samples
            .into_iter()
            .map(|sample| WindowRecord {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(mode.selected_path(request.case.direction)),
        conversion: None,
        quality: None,
        calibrated: true,
    })
}

fn write_pretty(value: &impl Serialize) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value).map_err(|error| error.to_string())?;
    writeln!(stdout).map_err(|error| error.to_string())
}

fn verify_plan(path: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    let plan = RunnerPlan::decode(&bytes)?;
    let addendum_bytes =
        std::fs::read(&plan.addendum).map_err(|error| format!("{}: {error}", plan.addendum))?;
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("addendum invalid: {}", errors.join("; ")))?;
    plan.validate(&addendum)
        .map_err(|errors| format!("plan invalid: {}", errors.join("; ")))?;
    for cell in &plan.cells {
        let case: ShiftCase = serde_json::from_value(cell.case.clone())
            .map_err(|error| format!("cell {}: {error}", cell.cell_id))?;
        case.validate()?;
        for name in [&cell.baseline_arm, &cell.candidate_arm] {
            let mode = ArmMode::parse(name)?;
            let offset = mode.offset(&case);
            let initial = fixture(case.length_bits, case.seed);
            let expected = reference_shift(&initial, case.direction, offset);
            let mut observed = initial;
            apply_shift(&mut observed, case.direction, offset);
            if (0..observed.len()).any(|index| observed.get(index) != expected[index])
                || !tail_is_zero(&observed)
            {
                return Err(format!(
                    "cell {} arm {} failed its oracle",
                    cell.cell_id, name
                ));
            }
        }
    }
    println!(
        "{} cells and {} arms match the frozen addendum and zero-fill oracle",
        plan.cells.len(),
        plan.arms.len()
    );
    Ok(())
}

fn arm_main(mode: ArmMode) -> Result<(), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: ArmRequest = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .map_err(|error| error.to_string())?;
    let output = run_arm(request, mode)?;
    transport::write_result_line(io::stdout().lock(), &output).map_err(|error| error.to_string())
}

/// Helper to create a BitVec with random-ish data
fn create_bitvec(num_bytes: usize) -> BitVec {
    let data: Vec<u8> = (0..num_bytes).map(|i| i as u8).collect();
    BitVec::from_bytes_le(&data)
}

/// Benchmark shift_left with word-aligned shift amount (k % 64 == 0)
fn bench_shift_left_word_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left_word_aligned");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        // Shift by 128 bits (2 words) - word-aligned
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift128", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(128);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_left with bit-level shift amount (k % 64 != 0)
fn bench_shift_left_bit_level(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left_bit_level");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        // Shift by 137 bits (2 words + 9 bits) - requires bit combining
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift137", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(137);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_right with word-aligned shift amount
fn bench_shift_right_word_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right_word_aligned");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift128", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_right(128);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_right with bit-level shift amount
fn bench_shift_right_bit_level(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right_bit_level");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift137", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_right(137);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Comprehensive comparison: word-aligned vs bit-level shifts
fn bench_shift_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_comparison");

    let num_bytes = 64 * 1024; // 64 KB
    group.throughput(Throughput::Bytes(num_bytes as u64));

    let shift_amounts = [
        ("shift_0", 0),
        ("shift_1", 1),
        ("shift_7", 7),
        ("shift_63", 63),
        ("shift_64", 64), // Word boundary
        ("shift_65", 65),
        ("shift_127", 127),
        ("shift_128", 128), // Word boundary
    ];

    for (name, shift) in shift_amounts.iter() {
        group.bench_with_input(BenchmarkId::new("shift_left", name), shift, |b, &k| {
            let mut bv = create_bitvec(num_bytes);
            b.iter(|| {
                bv.shift_left(k);
                black_box(&bv);
            });
        });

        group.bench_with_input(BenchmarkId::new("shift_right", name), shift, |b, &k| {
            let mut bv = create_bitvec(num_bytes);
            b.iter(|| {
                bv.shift_right(k);
                black_box(&bv);
            });
        });
    }

    group.finish();
}

/// Benchmark across multiple buffer sizes to find SIMD crossover point
fn bench_shift_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_sizes");

    // Test small to large buffers
    let sizes = [
        ("64B", 64),
        ("256B", 256),
        ("1KB", 1024),
        ("4KB", 4 * 1024),
        ("16KB", 16 * 1024),
        ("64KB", 64 * 1024),
        ("256KB", 256 * 1024),
        ("1MB", 1024 * 1024),
    ];

    for (name, num_bytes) in sizes.iter() {
        group.throughput(Throughput::Bytes(*num_bytes as u64));

        // Word-aligned shift
        group.bench_with_input(
            BenchmarkId::new("word_aligned", name),
            num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(128); // 2 words
                    black_box(&bv);
                });
            },
        );

        // Bit-level shift
        group.bench_with_input(BenchmarkId::new("bit_level", name), num_bytes, |b, &n| {
            let mut bv = create_bitvec(n);
            b.iter(|| {
                bv.shift_left(137); // 2 words + 9 bits
                black_box(&bv);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_shift_left_word_aligned,
    bench_shift_left_bit_level,
    bench_shift_right_word_aligned,
    bench_shift_right_bit_level,
    bench_shift_comparison,
    bench_shift_sizes,
);

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("--verify") => {
            let report = validation_report();
            let passed = report.passed;
            write_pretty(&report).map(|()| passed)
        }
        Some("--check-plan") => args
            .next()
            .ok_or_else(|| "--check-plan requires a path".to_owned())
            .and_then(|path| verify_plan(&path))
            .map(|()| true),
        Some("--build-identity") => {
            println!(
                "{{\"crate\":\"gf2-core\",\"target\":\"shifts\",\"rust_version\":\"1.95\",\"features\":\"all\"}}"
            );
            Ok(true)
        }
        _ => match std::env::var(ARM_VAR) {
            Ok(name) => ArmMode::parse(&name).and_then(arm_main).map(|()| true),
            Err(_) => {
                benches();
                Criterion::default().configure_from_args().final_summary();
                Ok(true)
            }
        },
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("shifts: {error}");
            ExitCode::from(if error.starts_with("timing failed") {
                1
            } else {
                2
            })
        }
    }
}
