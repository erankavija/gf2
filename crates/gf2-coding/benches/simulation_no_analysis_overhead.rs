//! Zero-overhead lock for the opt-in analysis capture in
//! `SimulationRunner::run_uncoded_ber_with_analysis` (JIT `80f218ca`).
//!
//! The contract the integration must honour: passing
//! `None` for the `AnalysisCapture` is bit-identical to calling the
//! unaugmented `run_uncoded_ber_with_channel` runner in terms of
//! behaviour, and matches it to within measurement noise in
//! throughput. This bench makes both requirements machine-checkable.
//!
//! Three benchmarks live in the same group so they share warmup /
//! sampling and are directly comparable inside Criterion's report:
//!
//! 1. `baseline_run_uncoded_ber_with_channel` — the original runner,
//!    unchanged public API.
//! 2. `analysis_none` — the new runner called with `None` as the
//!    capture. Must be within 1-2% of (1).
//! 3. `analysis_enabled` — the new runner with an active
//!    `PerBitLlrStats`; reference point for the enabled cost.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gf2_coding::modem::analysis::PerBitLlrStats;
use gf2_coding::modem::{
    AnalysisCapture, DemapMethod, FastGrayQamDemapper, GrayQamMapper, ModemChannelAdapter,
    ModemSpec,
};
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Fixed deterministic seed for every sample. Keeps the frame count
/// (and therefore the work) identical across bench iterations so the
/// comparison is apples-to-apples.
const BENCH_SEED: u64 = 0xA71103B7;

/// Number of bits per sample. Sized so the Monte Carlo loop runs for
/// many batches (>> the 960-bit internal batch size) but each sample
/// finishes well under Criterion's default measurement window.
const BENCH_FRAMES: usize = 200_000;

/// Builds a locked `SimulationConfig` that runs the full frame budget
/// (no early termination) at a single SNR point. `min_errors` is set
/// unreachably high so the loop exhausts `max_frames`, giving the
/// bench a stable per-sample work quantum.
fn bench_config() -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: vec![6.0],
        min_errors: usize::MAX,
        max_frames: BENCH_FRAMES,
        max_decoder_iterations: 1,
        rng_seed: Some(BENCH_SEED),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    }
}

fn bench_simulation_no_analysis_overhead(c: &mut Criterion) {
    let config = bench_config();
    let mut group = c.benchmark_group("simulation_no_analysis_overhead");
    group.throughput(Throughput::Elements(BENCH_FRAMES as u64));

    group.bench_function("baseline_run_uncoded_ber_with_channel", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let channel = BpskAwgnChannel;
            let r = SimulationRunner::run_uncoded_ber_with_channel(
                black_box(&channel),
                black_box(&config),
                &mut rng,
            );
            black_box(r);
        });
    });

    group.bench_function("analysis_none", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let channel = BpskAwgnChannel;
            let r = SimulationRunner::run_uncoded_ber_with_analysis(
                black_box(&channel),
                black_box(&config),
                None,
                &mut rng,
            );
            black_box(r);
        });
    });

    group.bench_function("analysis_enabled", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let channel = BpskAwgnChannel;
            let mut stats = PerBitLlrStats::new(1);
            let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
            let r = SimulationRunner::run_uncoded_ber_with_analysis(
                black_box(&channel),
                black_box(&config),
                Some(&mut capture),
                &mut rng,
            );
            black_box(r);
            black_box(stats);
        });
    });

    group.finish();
}

/// Companion bench over a 16-QAM `ModemChannelAdapter` fast path so the
/// zero-overhead claim is tested against the shared modem framework
/// (Gray-QAM fast kernel dispatch, `c5cee991`-prerequisite path), not
/// just the BPSK compatibility surface.
fn bench_simulation_no_analysis_overhead_qam16(c: &mut Criterion) {
    let config = bench_config();
    let mut group = c.benchmark_group("simulation_no_analysis_overhead_qam16");
    group.throughput(Throughput::Elements(BENCH_FRAMES as u64));

    // 16-QAM, m = 4, so the runner aligns to 4 bits per batch.
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    let demapper = FastGrayQamDemapper::<f32>::new(spec);
    let channel = ModemChannelAdapter::new(mapper, demapper, DemapMethod::MaxLog);

    group.bench_function("baseline_run_uncoded_ber_with_channel", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let r = SimulationRunner::run_uncoded_ber_with_channel(
                black_box(&channel),
                black_box(&config),
                &mut rng,
            );
            black_box(r);
        });
    });

    group.bench_function("analysis_none", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let r = SimulationRunner::run_uncoded_ber_with_analysis(
                black_box(&channel),
                black_box(&config),
                None,
                &mut rng,
            );
            black_box(r);
        });
    });

    group.bench_function("analysis_enabled", |b| {
        b.iter(|| {
            let mut rng = StdRng::seed_from_u64(BENCH_SEED);
            let mut stats = PerBitLlrStats::new(4);
            // QAM16 adapter was built with DemapMethod::MaxLog above
            // (see the `channel` binding in this function's scope).
            let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::MaxLog);
            let r = SimulationRunner::run_uncoded_ber_with_analysis(
                black_box(&channel),
                black_box(&config),
                Some(&mut capture),
                &mut rng,
            );
            black_box(r);
            black_box(stats);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simulation_no_analysis_overhead,
    bench_simulation_no_analysis_overhead_qam16
);
criterion_main!(benches);
