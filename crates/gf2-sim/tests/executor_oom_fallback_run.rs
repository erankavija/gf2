//! Run-level OOM fallback: a hybrid DVB-T2 run with a recoverable OOM forced
//! into the GPU LDPC stage through `PipelineConfig::inject_gpu_oom_modulus`
//! yields `frames`, `errors` and `fer` byte-identical to a CPU-only run at the
//! same seed, and emits the `dispatch_with_fallback` WARN event. Skips when no
//! GPU is usable.

#![cfg(feature = "hip")]

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_core::test_scratch::scratch;
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

mod common;
use common::{assert_three_columns_byte_identical_log_mean_iters, snr_point_to_counters};

const SEED: u64 = 0xDE16_0FC5;

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

/// The CPU-only reference arm.
fn ssot_counters(es_n0_db: f64, frames: usize, workers: usize) -> WorkerCounters {
    let template = DvbT2BicmFrameSim::new(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        es_n0_db,
        decoder_config(),
        DemapMethod::ExactLogMap,
    );
    run_snr_point(
        SEED,
        0,
        frames,
        NonZeroUsize::new(workers).unwrap(),
        || template.clone(),
        |g, ctx, sim| sim.simulate_frame(g, ctx),
    )
}

struct CapturedEvent {
    fields: HashMap<String, String>,
}

struct WarnCapture {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

struct Visitor<'a>(&'a mut HashMap<String, String>);
impl tracing::field::Visit for Visitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl tracing::Subscriber for WarnCapture {
    fn enabled(&self, meta: &tracing::Metadata<'_>) -> bool {
        *meta.level() == tracing::Level::WARN
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut fields = HashMap::new();
        event.record(&mut Visitor(&mut fields));
        self.events.lock().unwrap().push(CapturedEvent { fields });
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// `tracing::subscriber::set_global_default` installs at most one subscriber
/// per process, so every test in this binary shares this sink.
static CAPTURED_WARNS: OnceLock<Arc<Mutex<Vec<CapturedEvent>>>> = OnceLock::new();

/// Serializes every test in this binary that runs the pipeline while the
/// shared sink is asserted on, so one test's events cannot leak into another's
/// assertion window under multi-threaded bare `cargo test`.
static CAPTURE_GUARD: Mutex<()> = Mutex::new(());

/// Installs on first call and returns the shared sink, cleared. Callers hold
/// [`struct@CAPTURE_GUARD`] across their run and assertions.
fn shared_warn_capture() -> Arc<Mutex<Vec<CapturedEvent>>> {
    let events = CAPTURED_WARNS
        .get_or_init(|| {
            let events = Arc::new(Mutex::new(Vec::new()));
            tracing::subscriber::set_global_default(WarnCapture {
                events: events.clone(),
            })
            .expect("the shared sink is the first and only global subscriber");
            events
        })
        .clone();
    events.lock().unwrap().clear();
    events
}

fn build_gpu_pipeline_with_oom_injection(
    es_n0_db: f32,
    workers: usize,
    oom_modulus: u64,
    dump_dir: &std::path::Path,
) -> Pipeline {
    let mut pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db))
        .parallelism(NonZeroUsize::new(workers).unwrap())
        .seed(SEED)
        .with_gpu(true)
        .build()
        .expect("in-scope MODCOD builds");
    let cfg = pipeline.config_mut();
    cfg.inject_gpu_oom_modulus = Some(oom_modulus);
    cfg.diagnostic_dump_dir = Some(dump_dir.to_path_buf());
    pipeline
}

fn run_and_assert_oom_fallback(frames: usize, workers: usize, label: &str) {
    let es_n0 = 6.0_f32;

    let scratch = scratch("gf2sim-oom-fallback-run");
    let dump_dir = scratch.path().to_path_buf();

    // The fallback warn fires on rayon worker threads, which a thread-local
    // subscriber misses.
    let _capture_serial = CAPTURE_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let events = shared_warn_capture();

    // Modulus 2: OOM on the even global frames, so the run mixes CPU and GPU decodes.
    let pipeline = build_gpu_pipeline_with_oom_injection(es_n0, workers, 2, &dump_dir);
    assert_eq!(
        pipeline.stage_count(),
        8,
        "the GPU chain replaces the combined decode with GpuLdpcBp + BCH tail"
    );
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "GPU host must build an active stream pool"
    );

    let hybrid = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, frames)
        .expect("hybrid OOM-fallback sweep runs");
    let cpu_only = ssot_counters(f64::from(es_n0), frames, workers);

    assert!(
        hybrid.errors > 0 && hybrid.errors < hybrid.frames,
        "{label}: expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames (re-pin SEED if the chain changes)",
        hybrid.errors,
        hybrid.frames
    );

    assert_three_columns_byte_identical_log_mean_iters(
        &hybrid,
        &cpu_only,
        &format!("{label} hybrid(mix)-vs-cpu_only"),
    );

    let captured = events.lock().unwrap();
    let fallback_warn = captured.iter().find(|e| {
        e.fields.contains_key("batch_id")
            && e.fields.contains_key("snr_idx")
            && e.fields.contains_key("device_id")
    });
    assert!(
        fallback_warn.is_some(),
        "{label}: expected a dispatch_with_fallback WARN event with \
         batch_id/snr_idx/device_id; captured {} WARN event(s): {:?}",
        captured.len(),
        captured.iter().map(|e| &e.fields).collect::<Vec<_>>()
    );
}

fn run_and_assert_scheduler_oom_fallback(frames: usize, workers: usize, label: &str) {
    let es_n0 = 6.0_f32;

    let scratch = scratch("gf2sim-sched-oom");
    let dump_dir = scratch.path().to_path_buf();

    let _capture_serial = CAPTURE_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let events = shared_warn_capture();

    let mut pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0))
        .parallelism(NonZeroUsize::new(workers).unwrap())
        .seed(SEED)
        .with_gpu(true)
        .build()
        .expect("in-scope MODCOD builds");
    {
        let cfg = pipeline.config_mut();
        // A batch whose first global frame is even injects OOM and decodes on the CPU.
        cfg.inject_gpu_oom_modulus = Some(2);
        cfg.diagnostic_dump_dir = Some(dump_dir.clone());
        cfg.esn0_db_points = vec![f64::from(es_n0)];
        cfg.max_frames = frames as u64;
    }

    let results = pipeline
        .run()
        .expect("scheduler OOM-fallback run must succeed");
    assert_eq!(results.per_point.len(), 1, "{label}: one SNR point");
    let pt = &results.per_point[0];

    let cpu_only = ssot_counters(f64::from(es_n0), frames, workers);

    assert_three_columns_byte_identical_log_mean_iters(
        &snr_point_to_counters(pt),
        &cpu_only,
        &format!("{label} sched(mix)-vs-cpu_only"),
    );

    let captured = events.lock().unwrap();
    let fallback_warn = captured.iter().find(|e| {
        e.fields.contains_key("batch_id")
            && e.fields.contains_key("snr_idx")
            && e.fields.contains_key("device_id")
    });
    assert!(
        fallback_warn.is_some(),
        "{label}: expected a dispatch_with_fallback WARN event with \
         batch_id/snr_idx/device_id from the scheduler hybrid loop; \
         captured {} WARN event(s): {:?}",
        captured.len(),
        captured.iter().map(|e| &e.fields).collect::<Vec<_>>()
    );
}

#[test]
fn test_scheduler_oom_injection_matches_cpu_only_3_columns() {
    if !gpu_present() {
        eprintln!(
            "skipping test_scheduler_oom_injection_matches_cpu_only_3_columns: no usable GPU"
        );
        return;
    }
    run_and_assert_scheduler_oom_fallback(2, 2, "scheduler OOM-fallback smoke @6dB (modulus=2)");
}

#[test]
#[ignore = "sim: GPU-gated 32-frame scheduler OOM-fallback waterfall sweep"]
fn test_scheduler_oom_injection_waterfall_matches_cpu_only() {
    if !gpu_present() {
        eprintln!(
            "skipping test_scheduler_oom_injection_waterfall_matches_cpu_only: no usable GPU"
        );
        return;
    }
    run_and_assert_scheduler_oom_fallback(
        32,
        4,
        "scheduler OOM-fallback waterfall @6dB (modulus=2)",
    );
}

#[test]
fn test_oom_fallback_run_matches_cpu_only_3_columns() {
    if !gpu_present() {
        eprintln!("skipping test_oom_fallback_run_matches_cpu_only_3_columns: no usable GPU");
        return;
    }
    run_and_assert_oom_fallback(2, 2, "OOM-fallback smoke @6dB (modulus=2)");
}

#[test]
#[ignore = "sim: GPU-gated 32-frame OOM-fallback waterfall sweep"]
fn test_oom_fallback_run_waterfall_matches_cpu_only() {
    if !gpu_present() {
        eprintln!("skipping test_oom_fallback_run_waterfall_matches_cpu_only: no usable GPU");
        return;
    }
    run_and_assert_oom_fallback(32, 4, "OOM-fallback waterfall @6dB (modulus=2)");
}

#[test]
fn test_strict_gpu_config_promotes_oom_to_fatal_via_topology() {
    if !gpu_present() {
        eprintln!(
            "skipping test_strict_gpu_config_promotes_oom_to_fatal_via_topology: no usable GPU"
        );
        return;
    }

    use gf2_sim::error::{FatalError, StageError};

    // The run emits dispatch events; the guard keeps them out of a sibling test's
    // capture.
    let _capture_serial = CAPTURE_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let scratch = scratch("gf2sim-sc4-strict");
    let dump_dir = scratch.path().to_path_buf();

    let mut pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(6.0_f32))
        .parallelism(NonZeroUsize::new(1).unwrap())
        .seed(SEED)
        .with_gpu(true)
        .build()
        .expect("in-scope MODCOD builds");
    {
        let cfg = pipeline.config_mut();
        cfg.strict_gpu = true;
        cfg.inject_gpu_oom_modulus = Some(1); // inject on every frame
        cfg.diagnostic_dump_dir = Some(dump_dir.clone());
    }

    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "GPU host must build an active stream pool for SC4 test"
    );

    let result = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, 4);
    match result {
        Err(StageError::Fatal(FatalError::OutOfMemory { .. })) => {}
        Ok(c) => panic!(
            "SC4: expected Fatal::OutOfMemory from config-driven strict_gpu + modulus=1, \
             got Ok (frames={} errors={})",
            c.frames, c.errors
        ),
        Err(other) => panic!(
            "SC4: expected Fatal::OutOfMemory from config-driven strict_gpu + modulus=1, \
             got {other:?}"
        ),
    }

    let entries: Vec<_> = std::fs::read_dir(&dump_dir)
        .expect("dump dir must exist after strict OOM promotion")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    assert!(
        !entries.is_empty(),
        "SC4: at least one JSON dump file must be written on strict_gpu OOM promotion"
    );
}
