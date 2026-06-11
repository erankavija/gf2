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
use std::sync::{Arc, Mutex};

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

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
    // threads, so a thread-local subscriber would miss it). nextest runs each
    // test in its own process, so a global default is safe here.
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(WarnCapture {
        events: events.clone(),
    })
    .expect("first and only global subscriber in this process");

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

    // The three §11 CPU-vs-GPU columns, byte-identical hybrid-vs-CPU-only.
    assert_eq!(hybrid.frames, cpu_only.frames, "{label}: frames");
    assert_eq!(
        hybrid.errors, cpu_only.errors,
        "{label}: errors (frame errors) must match the CPU-only reference"
    );
    assert_eq!(
        hybrid.fer().to_bits(),
        cpu_only.fer().to_bits(),
        "{label}: fer bit pattern must match the CPU-only reference"
    );

    // mean_iters: LOGGED, never asserted (§11 CPU-vs-GPU exclusion; the
    // OOM-fallback run is a CPU+GPU mix).
    eprintln!(
        "{label}: frames={} errors={} fer={:.6} | \
         mean_iters hybrid(mix)={:.6} cpu_only={:.6} diff={:+.6} (logged only, §11)",
        hybrid.frames,
        hybrid.errors,
        hybrid.fer(),
        hybrid.mean_iters(),
        cpu_only.mean_iters(),
        hybrid.mean_iters() - cpu_only.mean_iters()
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
