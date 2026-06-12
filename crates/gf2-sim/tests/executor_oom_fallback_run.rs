//! Run-level OOM auto-fallback byte-identity (issue `42eac5cc` SC1; design
//! doc §8 + §11 CPU-vs-GPU relaxed contract).
//!
//! This is the **run-level** proof of the SC1 criterion (the function-level
//! `dispatch_with_fallback` unit checks in `executor_failure_modes.rs` prove the
//! decision tree in isolation; this file proves the criterion *as a run*):
//!
//! > a forced OOM during a hybrid run yields the same `fer / frames / errors`
//! > (the design §11 CPU-vs-GPU three-column contract; `mean_iters` logged not
//! > asserted, since the OOM-fallback run is mixed CPU+GPU) as a CPU-only
//! > reference run at the same seed.
//!
//! It drives the **production** stage-driven hybrid path
//! ([`TopologyExecutor::run_dvb_t2_snr_point`]) with the real GPU LDPC BP stage
//! active (`with_gpu(true)`), forcing a recoverable OOM into that stage via the
//! `42eac5cc` test-only injection hook
//! ([`PipelineConfig::inject_gpu_oom_modulus`]). The forced OOM flows through
//! the production `dispatch_with_fallback` boundary, which substitutes the CPU
//! LDPC fallback — exactly the path a genuine device OOM takes.
//!
//! # Operating point (the de160fc5 GPU smoke precedent)
//!
//! Seed `0xDE16_0FC5`, r1/2 16-QAM, 2 frames at the 6.0 dB waterfall — the same
//! pinned seed and Es/N0 as
//! `stage_driven_byte_identity.rs::test_stage_driven_gpu_smoke_matches_ssot_3_columns`
//! (2 frames keeps the fast-tier leg well under the 5 s cap even under
//! contention; the slow leg `test_oom_fallback_run_waterfall_matches_cpu_only`
//! sweeps 32 frames). `0 < errors < frames` is asserted, so the verdict
//! boundary §11 is about is genuinely exercised.
//!
//! # Injection modulus 2 (a genuine mixed run)
//!
//! `inject_gpu_oom_modulus = Some(2)` forces OOM on the even global frames only
//! (`g % 2 == 0`): those decode via the CPU fallback, the odd frames decode on
//! the GPU. So the run is a true CPU+GPU mix, not an all-fallback degenerate
//! (at 2 frames: frame 0 falls back to CPU, frame 1 runs on the GPU). The three
//! §11 columns must still be byte-identical to a CPU-only reference at the same
//! seed (the fallback runs the same CPU LDPC logic, and the GPU frames are
//! byte-identical on the verdict per the §11 relaxed contract).
//!
//! # `tracing::warn!` attestation
//!
//! Each injected-OOM frame emits the `dispatch_with_fallback` recoverable-error
//! `tracing::warn!` carrying `batch_id`, `snr_idx`, and `device_id`. A capturing
//! global subscriber records every WARN event; the test asserts at least one
//! such event fired with those three fields present.
//!
//! # Tier
//!
//! The whole binary is `#[cfg(feature = "hip")]` (it needs the GPU LDPC stage);
//! without `hip` it compiles to an empty test binary. With `hip` but no device
//! it skips cleanly.
//!
//! [`TopologyExecutor::run_dvb_t2_snr_point`]: gf2_sim::TopologyExecutor::run_dvb_t2_snr_point
//! [`PipelineConfig::inject_gpu_oom_modulus`]: gf2_sim::PipelineConfig::inject_gpu_oom_modulus

#![cfg(feature = "hip")]

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

mod common;
use common::{assert_three_columns_byte_identical_log_mean_iters, snr_point_to_counters};

/// The de160fc5 GPU-smoke pinned seed (shared with
/// `stage_driven_byte_identity.rs`).
const SEED: u64 = 0xDE16_0FC5;

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

/// The SSOT CPU-only reference arm: `run_snr_point` over the
/// `DvbT2BicmFrameSim` kernel (the byte-identity baseline).
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

/// One captured WARN event: its recorded fields rendered via `Debug`.
struct CapturedEvent {
    fields: HashMap<String, String>,
}

/// A minimal capturing `tracing::Subscriber` recording every WARN-level
/// event's fields. Span machinery is stubbed (this test only inspects events).
/// Shared behind an `Arc<Mutex<…>>` so the rayon worker threads that emit the
/// events write into the same buffer.
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

/// The process-wide WARN-capture sink. `tracing::subscriber::set_global_default`
/// installs at most ONE subscriber per process, so every test in this binary
/// must share the same sink: the first caller installs it, later callers reuse
/// it. Under nextest (process-per-test) each test is alone anyway; under bare
/// `cargo test` all tests of this binary share the process, and a per-test
/// `Arc` + ignored `set_global_default` would leave later tests asserting
/// against an empty buffer while events flow to the first test's sink (the
/// process-global test-isolation class).
static CAPTURED_WARNS: OnceLock<Arc<Mutex<Vec<CapturedEvent>>>> = OnceLock::new();

/// Serializes every test in this binary that runs the pipeline while the
/// shared sink is asserted on, so one test's events cannot leak into another's
/// assertion window under multi-threaded bare `cargo test`.
static CAPTURE_GUARD: Mutex<()> = Mutex::new(());

/// Installs (first call) and returns the shared WARN-capture sink, cleared of
/// any events from earlier tests in this process. Callers must hold
/// [`struct@CAPTURE_GUARD`] for the duration of their run + assertion.
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

/// Builds the preset DVB-T2 r1/2 16-QAM GPU chain at `es_n0_db` with the
/// `42eac5cc` OOM injection modulus and a temp diagnostic-dump dir set.
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

/// Drives the production hybrid OOM-fallback sweep over `frames` global frames
/// (modulus-2 injection: even frames fall back to CPU, odd frames run on the
/// GPU), then asserts the three §11 columns byte-identical to the CPU-only
/// reference and that the `dispatch_with_fallback` WARN event fired with
/// `batch_id`/`snr_idx`/`device_id`. Returns nothing; panics on any mismatch.
///
/// Shared by the fast 2-frame smoke and the slow 32-frame waterfall leg.
fn run_and_assert_oom_fallback(frames: usize, workers: usize, label: &str) {
    let es_n0 = 6.0_f32;

    let dump_dir = std::env::temp_dir().join(format!(
        "gf2sim-oom-fallback-run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    // Capture WARN events globally (the fallback warn fires on rayon worker
    // threads, so a thread-local subscriber would miss it). The sink is shared
    // process-wide; the guard serializes the capture-asserting tests.
    let _capture_serial = CAPTURE_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let events = shared_warn_capture();

    // Inject OOM on the even global frames → a genuine CPU+GPU mix.
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

    // Non-vacuity: a genuine mixed decode verdict at the waterfall (§11).
    assert!(
        hybrid.errors > 0 && hybrid.errors < hybrid.frames,
        "{label}: expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames (re-pin SEED if the chain changes)",
        hybrid.errors,
        hybrid.frames
    );

    // The three §11 CPU-vs-GPU columns, byte-identical hybrid-vs-CPU-only,
    // via the shared SSOT comparator (mean_iters logged there, never
    // asserted — the OOM-fallback run is a CPU+GPU mix).
    assert_three_columns_byte_identical_log_mean_iters(
        &hybrid,
        &cpu_only,
        &format!("{label} hybrid(mix)-vs-cpu_only"),
    );

    // tracing::warn! attestation: at least one recoverable-fallback WARN event
    // fired carrying batch_id, snr_idx, and device_id.
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

    let _ = std::fs::remove_dir_all(&dump_dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// MEDIUM-2: scheduler hybrid loop OOM injection (Pipeline::run path)
// ─────────────────────────────────────────────────────────────────────────────

/// Drives `Pipeline::run()` (the scheduler hybrid loop path) with modulus-2 OOM
/// injection and asserts the three §11 columns byte-identical to a CPU-only
/// reference, plus the `dispatch_with_fallback` WARN event with
/// `batch_id`/`snr_idx`/`device_id`. Shared by the fast smoke and slow waterfall
/// leg below.
fn run_and_assert_scheduler_oom_fallback(frames: usize, workers: usize, label: &str) {
    let es_n0 = 6.0_f32;

    let dump_dir = std::env::temp_dir().join(format!(
        "gf2sim-sched-oom-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    // Capture WARN events for the fallback warn from the hybrid loop. Shared
    // process-wide sink; the guard serializes the capture-asserting tests.
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
        // modulus=2: worker 0's batches (first frame 0, 2, ...) inject OOM →
        // CPU fallback; worker 1's batches (first frame 1, 3, ...) run GPU.
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

    // The three §11 CPU-vs-GPU columns, byte-identical scheduler-vs-CPU-only,
    // via the shared SSOT comparator (mean_iters logged there, never asserted).
    assert_three_columns_byte_identical_log_mean_iters(
        &snr_point_to_counters(pt),
        &cpu_only,
        &format!("{label} sched(mix)-vs-cpu_only"),
    );

    // tracing::warn! attestation: the hybrid loop's dispatch_with_fallback must
    // have fired the recoverable-error WARN with batch_id/snr_idx/device_id.
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

    let _ = std::fs::remove_dir_all(&dump_dir);
}

/// **MEDIUM-2 fast smoke: scheduler hybrid loop OOM injection (fast tier,
/// GPU-gated, NOT ignored).**
///
/// Drives `Pipeline::run()` (the C.1 scheduler hybrid loop,
/// `worker_partition_hybrid`) with `inject_gpu_oom_modulus = Some(2)` wired via
/// `PipelineConfig` — NOT a literal arg to `dispatch_with_fallback`. Worker 0's
/// batch (first global frame 0, `0 % 2 == 0`) injects OOM → CPU fallback; worker
/// 1's batch (first frame 1, `1 % 2 != 0`) runs on the GPU. The three §11
/// columns must be byte-identical to the CPU-only reference.
///
/// Timing: 2 total frames across 2 workers (1 per-worker batch of ≤BATCH_FRAMES),
/// measured well under 5 s on the gfx1030 host. Skips cleanly with no GPU.
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

/// **MEDIUM-2 slow leg: scheduler hybrid loop OOM injection (slow tier,
/// GPU-gated).** The 32-frame counterpart of the fast smoke above.
/// `#[ignore]`d (32 GPU/CPU-mix frames via the scheduler path exceed the 5 s cap).
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

// ─────────────────────────────────────────────────────────────────────────────
// SC1: topology executor OOM auto-fallback byte-identity
// ─────────────────────────────────────────────────────────────────────────────

/// **SC1 run-level proof (fast tier, GPU-gated, NOT ignored).**
///
/// A forced OOM during the production hybrid run yields the same three §11
/// columns (`fer`/`frames`/`errors`) as a CPU-only reference run at the same
/// seed. `mean_iters` is logged, never asserted (§11 CPU-vs-GPU exclusion, and
/// the OOM-fallback run is a CPU+GPU mix). The `dispatch_with_fallback`
/// recoverable-error `tracing::warn!` is asserted via a capturing subscriber.
///
/// Timing: 2 staged GPU/CPU-fallback frames + 2 SSOT CPU frames at the de160fc5
/// smoke point, measured ~2.9 s on the gfx1030 host even under heavy load
/// (loadavg ~26), inside the 5 s fast-tier cap. Skips cleanly with no GPU.
#[test]
fn test_oom_fallback_run_matches_cpu_only_3_columns() {
    if !gpu_present() {
        eprintln!("skipping test_oom_fallback_run_matches_cpu_only_3_columns: no usable GPU");
        return;
    }
    run_and_assert_oom_fallback(2, 2, "OOM-fallback smoke @6dB (modulus=2)");
}

/// **SC1 run-level proof (slow tier, GPU-gated).** The deeper 32-frame waterfall
/// mix the fast smoke is a miniature of: every even frame falls back to the CPU
/// LDPC stage, every odd frame runs on the GPU, and the three §11 columns must
/// stay byte-identical to the CPU-only reference across the whole non-vacuous
/// sweep. `#[ignore]`d under the slow-tier rules (32 GPU/CPU-mix decodes plus 32
/// SSOT CPU decodes exceed the 5 s fast cap).
#[test]
#[ignore = "sim: GPU-gated 32-frame OOM-fallback waterfall sweep"]
fn test_oom_fallback_run_waterfall_matches_cpu_only() {
    if !gpu_present() {
        eprintln!("skipping test_oom_fallback_run_waterfall_matches_cpu_only: no usable GPU");
        return;
    }
    run_and_assert_oom_fallback(32, 4, "OOM-fallback waterfall @6dB (modulus=2)");
}

/// **SC4: config-driven `strict_gpu` promotion (fast tier, GPU-gated, NOT
/// ignored).**
///
/// Sets `PipelineConfig::strict_gpu = true` AND
/// `PipelineConfig::inject_gpu_oom_modulus = Some(1)` (OOM on every frame) and
/// drives `TopologyExecutor::run_dvb_t2_snr_point` — the **config wiring**
/// path, not a literal function-argument invocation.  The first injected frame's
/// recoverable OOM must be promoted to `FatalError::OutOfMemory` (no CPU
/// fallback attempted) and the run must return that error; a JSON diagnostic
/// dump must have been written to the configured directory.
///
/// This closes the BLOCKING-1 finding: pre-snapshot SC4 tests passed `strict_gpu`
/// as a literal arg to `dispatch_with_fallback`, bypassing the config-to-policy
/// wiring that both the topology executor (`run_dvb_t2_snr_point`) and the
/// scheduler hybrid loop consume.
///
/// Timing: fails on the FIRST injected frame (modulus=1), so the run is as
/// short as a single GPU dispatch — measured well under 5 s on the gfx1030
/// host. Skips cleanly with no GPU.
#[test]
fn test_strict_gpu_config_promotes_oom_to_fatal_via_topology() {
    if !gpu_present() {
        eprintln!(
            "skipping test_strict_gpu_config_promotes_oom_to_fatal_via_topology: no usable GPU"
        );
        return;
    }

    use gf2_sim::error::{FatalError, StageError};

    // This test asserts no captured events, but it RUNS the pipeline (which
    // emits dispatch events) — hold the guard so its events cannot leak into
    // a concurrently-asserting sibling test under bare `cargo test`.
    let _capture_serial = CAPTURE_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let dump_dir = std::env::temp_dir().join(format!(
        "gf2sim-sc4-strict-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    // Build the GPU preset and wire BOTH strict_gpu and inject_gpu_oom_modulus
    // through the PipelineConfig — this is the config-driven path under test.
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

    // The first frame injects OOM; strict_gpu promotes it to fatal. The run
    // must return Err(Fatal(OutOfMemory)).
    let result = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, 4);
    match result {
        Err(StageError::Fatal(FatalError::OutOfMemory { .. })) => {
            // Correct: strict_gpu promoted the config-injected OOM to fatal.
        }
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

    // A JSON diagnostic dump must have been written (strict OOM triggers the
    // dump in dispatch_with_fallback before promoting the error).
    let entries: Vec<_> = std::fs::read_dir(&dump_dir)
        .expect("dump dir must exist after strict OOM promotion")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    assert!(
        !entries.is_empty(),
        "SC4: at least one JSON dump file must be written on strict_gpu OOM promotion"
    );
    let _ = std::fs::remove_dir_all(&dump_dir);
}
