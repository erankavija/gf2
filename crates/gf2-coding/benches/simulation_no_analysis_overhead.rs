//! Compares `SimulationRunner::run_uncoded_ber_with_analysis`, with a `None`
//! capture and with an active `PerBitLlrStats`, against
//! `run_uncoded_ber_with_channel`.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gf2_coding::modem::analysis::PerBitLlrStats;
use gf2_coding::modem::{
    AnalysisCapture, DemapMethod, FastGrayQamDemapper, GrayQamMapper, ModemChannelAdapter,
    ModemSpec,
};
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Fixed seed, so every sample does the same work.
const BENCH_SEED: u64 = 0xA71103B7;

const BENCH_FRAMES: usize = 200_000;

/// One SNR point with `min_errors` unreachably high, so the loop exhausts
/// `max_frames`.
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

fn bench_simulation_no_analysis_overhead_qam16(c: &mut Criterion) {
    let config = bench_config();
    let mut group = c.benchmark_group("simulation_no_analysis_overhead_qam16");
    group.throughput(Throughput::Elements(BENCH_FRAMES as u64));

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
            // Matches the adapter's `DemapMethod::MaxLog`.
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
