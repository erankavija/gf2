//! Preset-vs-graph run-level byte-identity (issue `8c8302c8`, wave D.1).
//!
//! For each of the six in-scope DVB-T2 MODCODs
//! (`rate ∈ {1/2, 2/3, 3/4}` × `modulation ∈ {16-QAM, 64-QAM}`), this suite
//! proves the two API layers — the typestate fluent builder
//! ([`Pipeline::dvb_t2`](gf2_sim::Pipeline::dvb_t2)) and a hand-wired
//! [`Chain`](gf2_sim::graph::Chain) — produce **byte-identical** simulation
//! output when driven via the same public stage-driven entrypoint
//! ([`TopologyExecutor::run_dvb_t2_snr_point`](gf2_sim::TopologyExecutor::run_dvb_t2_snr_point))
//! at a fixed seed.
//!
//! # Columns asserted (design doc §11 CPU-only contract)
//!
//! Both arms run on CPU only (same code path), so the full four-column set is
//! asserted:
//!
//! * `frames` (u64, integer-exact)
//! * `errors` (u64, integer-exact — frame errors)
//! * `fer` = `errors / frames` (f64 bit pattern)
//! * `mean_iters` = `total_iterations / frames` (f64 bit pattern)
//!
//! `ber` is **excluded** per the status-quo amendment from issue `152388f4`
//! (non-associative f32 horizontal reduction; see design-doc §11
//! "Always-excluded").
//!
//! # Graph-construction SSOT
//!
//! The hand-wired chain construction below mirrors the SSOT in
//! `tests/preset_vs_graph.rs` (`build_graph`). That file proves structural +
//! single-frame execution equivalence; this file proves the **run-level** four-
//! column byte-identity over 200 frames. The chain construction is shared by
//! copying the exact same wiring pattern to avoid an SSOT violation — a
//! `mod common` import that brings in `build_graph` would require a public
//! API surface that is intentionally kept crate-internal.
//!
//! # Tiers
//!
//! * **Fast** (`test_preset_vs_graph_smoke_r12_16qam`): 2 frames of r1/2
//!   16-QAM well above threshold (20.0 dB), asserting four-column
//!   byte-identity. Completes in < 1 s; un-ignored so the CI gate genuinely
//!   runs the byte-identity criterion.
//! * **Slow** (one `#[ignore = "sim: ..."]` per MODCOD): 200 frames per
//!   MODCOD at 20.0 dB; BP early-terminates after 1–3 iterations so every
//!   leg finishes in < 90 s (the heaviest, r3/4 64-QAM, measured ~85 s on the
//!   development machine — well within the 120 s cap).

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::channels::Awgn;
use gf2_sim::graph::Chain;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::stage::erase;
use gf2_sim::stages::dvb_t2_bicm_stages;
use gf2_sim::{Pipeline, PipelineConfig, Scheduler, TopologyExecutor};

// ──────────────────────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────────────────────

/// Base seed for the byte-identity tests.
const SEED: u64 = 0xDE16_0FC5;

/// Frames for the fast-tier smoke (must complete in < 5 s on any hardware).
/// Two frames; each full n=64800 LDPC decode costs ~0.25 s at 20 dB (1–2
/// BP iterations before early-exit); the combined preset-build + graph-build +
/// 2×2 runs stays well under the 5 s fast-tier cap (measured ~0.5 s).
const SMOKE_FRAMES: usize = 2;

/// Frames for the slow-tier legs.
const SLOW_FRAMES: usize = 200;

/// Es/N0 used for all legs: 20.0 dB is well above the waterfall of every
/// in-scope MODCOD (the hardest in-scope MODCOD, r3/4 64-QAM, requires ~15.5
/// dB for QEF), ensuring BP early-terminates after very few iterations on
/// every frame so 200-frame slow legs finish quickly. Byte-identity is a
/// pure algebraic property of the deterministic pipeline — it holds regardless
/// of the error mix — so no waterfall calibration per-MODCOD is needed here.
const ES_N0_DB: f32 = 20.0;

// ──────────────────────────────────────────────────────────────────────────────
// Shared decoder config
// ──────────────────────────────────────────────────────────────────────────────

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

// ──────────────────────────────────────────────────────────────────────────────
// Pipeline builders
// ──────────────────────────────────────────────────────────────────────────────

/// Builds the preset DVB-T2 pipeline via the typestate builder.
fn build_preset(rate: CodeRate, modulation: DvbT2Modulation) -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0_DB))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(1).expect("1 is non-zero"))
        .build()
        .expect("in-scope MODCOD builds via the preset")
}

/// Derives the soft demapper's `N0 = 2*sigma^2` from `ES_N0_DB`, using the
/// same f64-computed, once-rounded arithmetic as the preset's
/// `Channel::demap_noise_var` (the SSOT formula from `frame_sim.rs`).
fn demap_n0(es_n0_db: f32) -> f32 {
    let es_n0_lin = 10.0_f64.powf(f64::from(es_n0_db) / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}

/// Builds the same DVB-T2 BICM chain by hand through the graph [`Chain`] API.
///
/// This mirrors the `build_graph` helper in `tests/preset_vs_graph.rs` (the
/// SSOT for graph-chain structural + single-frame equivalence). The wiring is
/// reproduced verbatim here rather than shared via `mod common` because the
/// SSOT helper is private to `preset_vs_graph.rs` and exposing it would
/// require additional public surface.
fn build_graph(rate: CodeRate, modulation: DvbT2Modulation) -> Pipeline {
    let n0 = demap_n0(ES_N0_DB);
    let factory = dvb_t2_bicm_stages(
        rate,
        modulation,
        decoder_config(),
        DemapMethod::ExactLogMap,
        n0,
    );

    let mut chain = Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    // Channel stage: SymbolBatch → SymbolBatch between the forward and inverse
    // halves.
    ids.push(chain.add(erase(Awgn::new(ES_N0_DB, modulation.bits_per_cell()))));
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }
    for pair in ids.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("each consecutive BICM hop is type-compatible");
    }

    // Mirror the preset's config so the config comparison is exact.
    let config = PipelineConfig {
        seed: SEED,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(1).expect("1 is non-zero"),
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    };

    chain
        .with_config(config)
        .build()
        .expect("the full BICM chain is a valid DAG")
}

// ──────────────────────────────────────────────────────────────────────────────
// Core assertion helper
// ──────────────────────────────────────────────────────────────────────────────

/// Runs `max_frames` frames of both the preset and graph pipelines for
/// `(rate, modulation)`, then asserts the four byte-identity columns via
/// [`common::assert_four_columns_byte_identical`].
///
/// Both arms are driven through
/// [`TopologyExecutor::run_dvb_t2_snr_point`](gf2_sim::TopologyExecutor::run_dvb_t2_snr_point)
/// at `snr_idx = 0`, so the comparison isolates the API-form difference (the
/// preset's `Pipeline::run()` vs the graph arm is separately guarded by
/// `tests/stage_driven_byte_identity.rs` and is NOT re-proved here).
fn assert_byte_identical(
    rate: CodeRate,
    modulation: DvbT2Modulation,
    max_frames: usize,
    label: &str,
) -> (WorkerCounters, WorkerCounters) {
    let preset = build_preset(rate, modulation);
    let graph = build_graph(rate, modulation);

    let sched_preset = Scheduler::from_pipeline(&preset);
    let sched_graph = Scheduler::from_pipeline(&graph);

    let preset_c = TopologyExecutor::run_dvb_t2_snr_point(&preset, &sched_preset, 0, max_frames)
        .expect("stage-driven preset sweep runs");
    let graph_c = TopologyExecutor::run_dvb_t2_snr_point(&graph, &sched_graph, 0, max_frames)
        .expect("stage-driven graph sweep runs");

    assert_eq!(
        preset_c.frames, max_frames as u64,
        "{label}: preset ran {}/{max_frames} frames",
        preset_c.frames
    );
    assert_eq!(
        graph_c.frames, max_frames as u64,
        "{label}: graph ran {}/{max_frames} frames",
        graph_c.frames
    );

    common::assert_four_columns_byte_identical(&preset_c, &graph_c, label);

    eprintln!(
        "{label}: frames={} errors={} fer={:.6} mean_iters={:.6} (preset==graph)",
        preset_c.frames,
        preset_c.errors,
        preset_c.fer(),
        preset_c.mean_iters(),
    );

    (preset_c, graph_c)
}

// ──────────────────────────────────────────────────────────────────────────────
// Fast-tier smoke (always runs, gates the criterion)
// ──────────────────────────────────────────────────────────────────────────────

/// Fast-tier smoke: 2 frames of r1/2 16-QAM at 20.0 dB (well above threshold).
///
/// Asserts all four byte-identity columns (`frames`, `errors`, `fer`,
/// `mean_iters`) between the typestate-preset form and the hand-wired graph
/// form. Both arms run through
/// [`TopologyExecutor::run_dvb_t2_snr_point`] at `snr_idx = 0`, `seed =
/// 0xDE16_0FC5`. This un-ignored test is the `[hard]` byte-identity gate
/// criterion's CI-facing guard — the slow 200-frame legs below are
/// supplementary evidence.
#[test]
fn test_preset_vs_graph_smoke_r12_16qam() {
    assert_byte_identical(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SMOKE_FRAMES,
        "smoke r1/2 16-QAM @20dB",
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Slow-tier 200-frame legs (one per MODCOD, each under the 120 s cap)
// ──────────────────────────────────────────────────────────────────────────────

/// Slow: 200 frames, r1/2 16-QAM at 20.0 dB.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r1/2 16-QAM"]
fn test_preset_vs_graph_200f_r12_16qam() {
    assert_byte_identical(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SLOW_FRAMES,
        "200f r1/2 16-QAM @20dB",
    );
}

/// Slow: 200 frames, r1/2 64-QAM at 20.0 dB.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r1/2 64-QAM"]
fn test_preset_vs_graph_200f_r12_64qam() {
    assert_byte_identical(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam64,
        SLOW_FRAMES,
        "200f r1/2 64-QAM @20dB",
    );
}

/// Slow: 200 frames, r2/3 16-QAM at 20.0 dB.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r2/3 16-QAM"]
fn test_preset_vs_graph_200f_r23_16qam() {
    assert_byte_identical(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam16,
        SLOW_FRAMES,
        "200f r2/3 16-QAM @20dB",
    );
}

/// Slow: 200 frames, r2/3 64-QAM at 20.0 dB.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r2/3 64-QAM"]
fn test_preset_vs_graph_200f_r23_64qam() {
    assert_byte_identical(
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam64,
        SLOW_FRAMES,
        "200f r2/3 64-QAM @20dB",
    );
}

/// Slow: 200 frames, r3/4 16-QAM at 20.0 dB.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r3/4 16-QAM"]
fn test_preset_vs_graph_200f_r34_16qam() {
    assert_byte_identical(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam16,
        SLOW_FRAMES,
        "200f r3/4 16-QAM @20dB",
    );
}

/// Slow: 200 frames, r3/4 64-QAM at 20.0 dB.
///
/// This is the slowest config (highest code rate + 64-QAM LDPC decode);
/// measured ~85 s on the development machine, well within the 120 s cap.
#[test]
#[ignore = "sim: 200-frame preset-vs-graph byte-identity, r3/4 64-QAM"]
fn test_preset_vs_graph_200f_r34_64qam() {
    assert_byte_identical(
        CodeRate::Rate3_4,
        DvbT2Modulation::Qam64,
        SLOW_FRAMES,
        "200f r3/4 64-QAM @20dB",
    );
}
