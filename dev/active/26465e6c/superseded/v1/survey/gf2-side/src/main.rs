use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::execution_windows_configured;
use tuning_campaign_support::transport;

/// Fixture banks a streaming cell rotates through, matching
/// `tuning_campaign_support::timing::FIXTURE_BANKS` and `SURVEY_BANKS` in
/// `wire_common.h`.
const BANKS: usize = 8;
/// Bytes in one AVX2 vector; the alignment the fixture base guarantees.
const VECTOR_BYTES: usize = 32;
/// Target length of one conversion-cost probe.
const PROBE_TARGET: Duration = Duration::from_millis(5);
/// Upper bound on probe iterations, so a slow call cannot run unbounded.
const PROBE_MAX_CALLS: u64 = 1 << 24;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    cell_id: String,
    arm: String,
    role: String,
    pair: u32,
    case: Value,
    cache_state: String,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

#[derive(Clone, Copy, Debug)]
enum Pattern {
    Random,
    AllZero,
    AllOne,
}

impl Pattern {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "random" => Ok(Self::Random),
            "all_zero" => Ok(Self::AllZero),
            "all_one" => Ok(Self::AllOne),
            other => Err(format!("unknown pattern {other:?}")),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PopcountCase {
    op: String,
    words: u64,
    seed: u64,
    pattern: String,
    word_offset: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AndPopcountCase {
    op: String,
    words: u64,
    seed_lhs: u64,
    seed_rhs: u64,
    pattern: String,
    word_offset: u32,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

/// The setup and conversion costs `receipt::ConversionCosts` records. Every
/// field is measured on this host in this process; a step an arm does not
/// perform is reported as zero.
#[derive(Serialize)]
struct Conversion {
    setup_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    batch_fill_ns: u64,
    dispatch_ns: u64,
}

#[derive(Serialize)]
struct Result_ {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: String,
    conversion: Option<Conversion>,
    quality: Option<Value>,
}

fn words_count(words: u64) -> Result<usize, String> {
    usize::try_from(words).map_err(|_| "words does not fit usize".to_owned())
}

fn checked_offset(word_offset: u32) -> Result<usize, String> {
    let offset =
        usize::try_from(word_offset).map_err(|_| "word_offset does not fit usize".to_owned())?;
    if offset > 3 {
        return Err(format!("word_offset {offset} is outside 0..=3"));
    }
    Ok(offset)
}

/// Banks a cell allocates: a streaming cell rotates through all of them so
/// successive calls do not reuse cache-resident data, while a cold or warm
/// cell keeps one working set, matching `ab-smoke-workload` and the `streaming`
/// branch of `survey_run_calls` in `wire_common.h`.
fn bank_count(cache_state: &str) -> usize {
    if cache_state == "streaming" {
        BANKS
    } else {
        1
    }
}

/// One fixture buffer whose logical first word sits `word_offset` words past a
/// 32-byte aligned base.
///
/// `survey_make_words` in `wire_common.h` allocates the `words + 4` word base
/// array through `posix_memalign(32)` and hands out the window at
/// `word_offset`, so `word_offset == 0` is AVX2-vector aligned and 1, 2 or 3
/// break that alignment by 8, 16 or 24 bytes. The global Rust allocator
/// guarantees only `align_of::<u64>()`, so this over-allocates and selects the
/// first 32-byte aligned word inside the allocation. Without it the alignment
/// cell would compare an aligned external buffer against a differently aligned
/// gf2 buffer.
struct Fixture {
    storage: Vec<u64>,
    start: usize,
    len: usize,
}

impl Fixture {
    fn new(words: usize, seed: u64, pattern: Pattern, word_offset: usize) -> Self {
        let base = words + 4;
        let mut storage = vec![0_u64; base + VECTOR_BYTES / size_of::<u64>()];
        let pad = (VECTOR_BYTES - (storage.as_ptr() as usize % VECTOR_BYTES)) % VECTOR_BYTES
            / size_of::<u64>();
        let region = &mut storage[pad..pad + base];
        match pattern {
            Pattern::Random => {
                let mut generator = SplitMix64::new(seed);
                for word in region {
                    *word = generator.next_u64();
                }
            }
            Pattern::AllZero => {}
            Pattern::AllOne => region.fill(u64::MAX),
        }
        let fixture = Self {
            storage,
            start: pad + word_offset,
            len: words,
        };
        assert_eq!(
            fixture.words().as_ptr() as usize % VECTOR_BYTES,
            word_offset * size_of::<u64>() % VECTOR_BYTES,
            "fixture alignment does not match word_offset {word_offset}"
        );
        fixture
    }

    fn words(&self) -> &[u64] {
        &self.storage[self.start..self.start + self.len]
    }
}

fn make_popcount_banks(case: &PopcountCase, banks: usize) -> Result<Vec<Fixture>, String> {
    let words = words_count(case.words)?;
    let offset = checked_offset(case.word_offset)?;
    let pattern = Pattern::parse(&case.pattern)?;
    Ok((0..banks)
        .map(|_| Fixture::new(words, case.seed, pattern, offset))
        .collect())
}

fn make_and_banks(case: &AndPopcountCase, banks: usize) -> Result<Vec<(Fixture, Fixture)>, String> {
    let words = words_count(case.words)?;
    let offset = checked_offset(case.word_offset)?;
    let pattern = Pattern::parse(&case.pattern)?;
    Ok((0..banks)
        .map(|_| {
            (
                Fixture::new(words, case.seed_lhs, pattern, offset),
                Fixture::new(words, case.seed_rhs, pattern, offset),
            )
        })
        .collect())
}

// The shipped gf2-core size-aware production dispatcher.
#[inline(never)]
fn production_dispatch(buffer: &[u64]) -> u64 {
    gf2_core::kernels::ops::popcount(buffer)
}

// The AVX2 nibble-LUT kernel selected directly, bypassing size thresholds.
#[inline(never)]
fn nibble_lut(buffer: &[u64]) -> u64 {
    (gf2_kernels_simd::detect()
        .expect("AVX2 required on this host")
        .popcnt_fn)(buffer)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn scalar_popcnt_instruction_loop_inner(buffer: &[u64]) -> u64 {
    buffer
        .iter()
        .map(|word| core::arch::x86_64::_popcnt64(*word as i64) as u64)
        .sum()
}

// A scalar loop using the POPCNT instruction, guarded by runtime detection.
#[inline(never)]
fn scalar_popcnt_instruction_loop(buffer: &[u64]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        assert!(
            is_x86_feature_detected!("popcnt"),
            "scalar-popcnt requires the x86_64 popcnt feature"
        );
        // SAFETY: the runtime feature check above proves the target feature.
        unsafe { scalar_popcnt_instruction_loop_inner(buffer) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        panic!("scalar-popcnt is only implemented for x86_64")
    }
}

// Portable compiler-generated count_ones control; no target-feature attributes.
#[inline(never)]
fn compiler_count_ones(buffer: &[u64]) -> u64 {
    buffer.iter().map(|word| word.count_ones() as u64).sum()
}

// The AVX2 fused lhs-AND-rhs population count kernel.
#[inline(never)]
fn and_popcnt_fused(lhs: &[u64], rhs: &[u64]) -> u64 {
    (gf2_kernels_simd::detect()
        .expect("AVX2 required on this host")
        .and_popcnt_fn)(lhs, rhs)
}

// A single-pass scalar fused control implementation.
#[inline(never)]
fn and_popcnt_scalar_control(lhs: &[u64], rhs: &[u64]) -> u64 {
    lhs.iter()
        .zip(rhs)
        .map(|(left, right)| (left & right).count_ones() as u64)
        .sum()
}

// The public-API two-pass consumer model; allocation remains inside each call.
#[inline(never)]
fn and_popcnt_two_pass_consumer(lhs: &[u64], rhs: &[u64]) -> u64 {
    let mut temporary = lhs.to_vec();
    gf2_core::kernels::ops::and_inplace(&mut temporary, rhs);
    gf2_core::kernels::ops::popcount(&temporary)
}

fn popcount_operation(arm: &str, buffer: &[u64]) -> Result<u64, String> {
    match arm {
        "production-dispatch" => Ok(production_dispatch(buffer)),
        "nibble-lut" => Ok(nibble_lut(buffer)),
        "scalar-popcnt" => Ok(scalar_popcnt_instruction_loop(buffer)),
        "compiler-count-ones" => Ok(compiler_count_ones(buffer)),
        other => Err(format!("arm {other:?} is not a popcount arm")),
    }
}

fn and_operation(arm: &str, lhs: &[u64], rhs: &[u64]) -> Result<u64, String> {
    match arm {
        "and-popcnt-fused" => Ok(and_popcnt_fused(lhs, rhs)),
        "and-popcnt-scalar-control" => Ok(and_popcnt_scalar_control(lhs, rhs)),
        "and-popcnt-two-pass-consumer" => Ok(and_popcnt_two_pass_consumer(lhs, rhs)),
        other => Err(format!("arm {other:?} is not an and_popcnt arm")),
    }
}

fn path_for_arm(arm: &str) -> Result<&'static str, String> {
    match arm {
        "production-dispatch" => Ok("gf2-production-popcount-dispatch"),
        "nibble-lut" => Ok("gf2-avx2-nibble-lut-direct"),
        "scalar-popcnt" => Ok("scalar-popcnt-instruction-loop"),
        "compiler-count-ones" => Ok("compiler-count-ones-portable"),
        "and-popcnt-fused" => Ok("gf2-avx2-and-popcnt-fused"),
        "and-popcnt-scalar-control" => Ok("and-popcnt-scalar-fused-control"),
        "and-popcnt-two-pass-consumer" => Ok("gf2-and-inplace-then-popcount-two-pass"),
        other => Err(format!("unknown GF2_POPCOUNT_ARM {other:?}")),
    }
}

/// Mean nanoseconds of one `body` call, calibrated to `PROBE_TARGET`.
///
/// Probes run after the timed windows so they cannot perturb the measurement
/// the receipt carries.
fn probe_ns(mut body: impl FnMut()) -> u64 {
    let mut calls = 1_u64;
    loop {
        let start = Instant::now();
        for _ in 0..calls {
            body();
        }
        let elapsed = start.elapsed();
        if elapsed >= PROBE_TARGET || calls >= PROBE_MAX_CALLS {
            return u64::try_from(elapsed.as_nanos() / u128::from(calls)).unwrap_or(u64::MAX);
        }
        calls *= 2;
    }
}

/// Per-call cost of the runtime backend resolution the arm performs inside its
/// timed body. `compiler-count-ones` and the scalar fused control resolve
/// nothing and report zero.
fn dispatch_probe_ns(arm: &str, words: usize) -> u64 {
    match arm {
        "production-dispatch" | "and-popcnt-two-pass-consumer" => {
            let passes = if arm == "production-dispatch" { 1 } else { 2 };
            probe_ns(|| {
                for _ in 0..passes {
                    black_box(
                        gf2_core::kernels::backend::select_backend_for_size(black_box(words))
                            .name(),
                    );
                    black_box(gf2_core::kernels::simd::maybe_simd().is_some());
                }
            })
        }
        "nibble-lut" => probe_ns(|| {
            black_box(
                gf2_kernels_simd::detect()
                    .expect("AVX2 required on this host")
                    .popcnt_fn,
            );
        }),
        "and-popcnt-fused" => probe_ns(|| {
            black_box(
                gf2_kernels_simd::detect()
                    .expect("AVX2 required on this host")
                    .and_popcnt_fn,
            );
        }),
        #[cfg(target_arch = "x86_64")]
        "scalar-popcnt" => probe_ns(|| {
            black_box(is_x86_feature_detected!("popcnt"));
        }),
        _ => 0,
    }
}

/// Per-call cost of the temporary buffer a consumer-equivalent arm materializes
/// before its extra pass. Only the two-pass public-API model pays it; the fused
/// kernel and the single-pass controls report zero.
///
/// The cost is already inside the timed windows: this probe quantifies how much
/// of the arm's time it is, it never subtracts it.
fn pack_probe_ns(arm: &str, lhs: &[u64]) -> u64 {
    if arm == "and-popcnt-two-pass-consumer" {
        probe_ns(|| {
            let temporary = black_box(lhs).to_vec();
            black_box(temporary.as_ptr());
        })
    } else {
        0
    }
}

fn run_transport(request: Request, arm: &str) -> Result<(), String> {
    if request.cache_state != "cold"
        && request.cache_state != "warm"
        && request.cache_state != "streaming"
    {
        return Err(format!("unknown cache_state {:?}", request.cache_state));
    }
    let path = path_for_arm(arm)?;
    let is_and = arm.starts_with("and-popcnt-");
    let banks = bank_count(&request.cache_state);
    let mut sink = 0_u64;
    let (samples, conversion) = if is_and {
        let case: AndPopcountCase = serde_json::from_value(request.case)
            .map_err(|error| format!("case does not decode: {error}"))?;
        if case.op != "and_popcnt" && case.op != "and_popcnt_two_pass" {
            return Err(format!("invalid and case op {:?}", case.op));
        }
        let setup_start = Instant::now();
        let fixtures = make_and_banks(&case, banks)?;
        let setup_ns = u64::try_from(setup_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if request.cache_state == "warm" {
            let (lhs, rhs) = &fixtures[0];
            sink ^= and_operation(arm, black_box(lhs.words()), black_box(rhs.words()))?;
        }
        let mut body = |bank: usize| {
            let (lhs, rhs) = &fixtures[bank % banks];
            let value = and_operation(arm, black_box(lhs.words()), black_box(rhs.words()))
                .expect("validated arm must remain callable");
            sink = black_box(sink ^ black_box(value));
        };
        let samples = execution_windows_configured(
            0,
            u64::from(request.windows),
            Duration::from_millis(u64::from(request.window_target_ms)),
            &mut body,
            |_| Ok(()),
        )
        .map_err(|error| format!("timing failed: {error}"))?;
        // Only the whole-consumer op accounts conversion costs; a
        // kernel-isolated cell reports none, as `ab-smoke-workload` does.
        let conversion = (case.op == "and_popcnt_two_pass").then(|| {
            let words = fixtures[0].0.words().len();
            Conversion {
                setup_ns,
                pack_ns: pack_probe_ns(arm, fixtures[0].0.words()),
                unpack_ns: 0,
                batch_fill_ns: 0,
                dispatch_ns: dispatch_probe_ns(arm, words),
            }
        });
        (samples, conversion)
    } else {
        let case: PopcountCase = serde_json::from_value(request.case)
            .map_err(|error| format!("case does not decode: {error}"))?;
        if case.op != "popcount" {
            return Err(format!("invalid popcount case op {:?}", case.op));
        }
        let fixtures = make_popcount_banks(&case, banks)?;
        if request.cache_state == "warm" {
            sink ^= popcount_operation(arm, black_box(fixtures[0].words()))?;
        }
        let mut body = |bank: usize| {
            let value = popcount_operation(arm, black_box(fixtures[bank % banks].words()))
                .expect("validated arm must remain callable");
            sink = black_box(sink ^ black_box(value));
        };
        let samples = execution_windows_configured(
            0,
            u64::from(request.windows),
            Duration::from_millis(u64::from(request.window_target_ms)),
            &mut body,
            |_| Ok(()),
        )
        .map_err(|error| format!("timing failed: {error}"))?;
        (samples, None)
    };
    black_box(sink);
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = Result_ {
        schema: "zen3-benchmark-arm-result-v1".to_owned(),
        windows: samples
            .into_iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: path.to_owned(),
        conversion,
        quality: None,
    };
    transport::write_result_line(io::stdout().lock(), &result)
        .map_err(|error| format!("cannot write result: {error}"))
}

fn parse_check(args: &[String], arm: &str) -> Result<u64, String> {
    if args.is_empty() {
        return Err("--check requires an operation".to_owned());
    }
    match args[0].as_str() {
        "popcount" if args.len() == 5 => {
            let case = PopcountCase {
                op: args[0].clone(),
                words: args[1].parse().map_err(|_| "invalid words".to_owned())?,
                seed: args[2].parse().map_err(|_| "invalid seed".to_owned())?,
                pattern: args[3].clone(),
                word_offset: args[4]
                    .parse()
                    .map_err(|_| "invalid word_offset".to_owned())?,
            };
            let banks = make_popcount_banks(&case, 1)?;
            popcount_operation(arm, banks[0].words())
        }
        "and_popcnt" | "and_popcnt_two_pass" if args.len() == 6 => {
            let case = AndPopcountCase {
                op: args[0].clone(),
                words: args[1].parse().map_err(|_| "invalid words".to_owned())?,
                seed_lhs: args[2].parse().map_err(|_| "invalid seed_lhs".to_owned())?,
                seed_rhs: args[3].parse().map_err(|_| "invalid seed_rhs".to_owned())?,
                pattern: args[4].clone(),
                word_offset: args[5]
                    .parse()
                    .map_err(|_| "invalid word_offset".to_owned())?,
            };
            let banks = make_and_banks(&case, 1)?;
            and_operation(arm, banks[0].0.words(), banks[0].1.words())
        }
        _ => Err("invalid --check arguments".to_owned()),
    }
}

/// Prints the byte alignment of the fixture window every `word_offset` selects,
/// so the alignment cell's premise is checkable without a debugger.
fn report_alignment() {
    for words in [4_usize, 8, 64, 256] {
        for offset in 0..4_usize {
            let fixture = Fixture::new(words, 1, Pattern::Random, offset);
            println!(
                "words={words} word_offset={offset} address_mod_{VECTOR_BYTES}={}",
                fixture.words().as_ptr() as usize % VECTOR_BYTES
            );
        }
    }
}

/// Prints the arm's per-call runtime backend-resolution cost beside the whole
/// per-call cost of the operation at each surveyed size.
///
/// This is a supporting measurement outside the benchmark protocol: it is not a
/// receipt and no adoption decision rests on it. It exists so the findings can
/// state how much of a kernel-isolated arm's time is dispatch rather than work.
fn report_dispatch(arm: &str) -> Result<(), String> {
    for words in [4_usize, 8, 63, 64, 256, 4096, 16384, 524_288] {
        let fixture = Fixture::new(words, 1, Pattern::Random, 0);
        let buffer = fixture.words();
        let call_ns = probe_ns(|| {
            black_box(popcount_operation(arm, black_box(buffer)).expect("arm must be callable"));
        });
        println!(
            "arm={arm} words={words} call_ns={call_ns} dispatch_ns={}",
            dispatch_probe_ns(arm, words)
        );
    }
    Ok(())
}

fn main() {
    let arm = match std::env::var("GF2_POPCOUNT_ARM") {
        Ok(value) => value,
        Err(_) => {
            eprintln!("gf2-side: GF2_POPCOUNT_ARM is absent");
            std::process::exit(2);
        }
    };
    if let Some(mode) = std::env::args().nth(1) {
        if mode == "--alignment" {
            report_alignment();
            return;
        }
        if mode == "--dispatch-probe" {
            if let Err(error) = report_dispatch(&arm) {
                eprintln!("gf2-side: {error}");
                std::process::exit(2);
            }
            return;
        }
        if mode == "--check" {
            let args: Vec<_> = std::env::args().skip(2).collect();
            match parse_check(&args, &arm) {
                Ok(value) => {
                    println!("{value}");
                    return;
                }
                Err(error) => {
                    eprintln!("gf2-side: {error}");
                    std::process::exit(2);
                }
            }
        }
    }
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("gf2-side: {error}");
                std::process::exit(2);
            }
        };
    if let Err(error) = run_transport(request, &arm) {
        eprintln!("gf2-side: {error}");
        std::process::exit(if error.contains("timing failed") {
            1
        } else {
            2
        });
    }
}
