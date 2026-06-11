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
//! # Operating points — the §11 waterfall regime (non-vacuous)
//!
//! Every leg (smoke included) runs at a per-MODCOD **waterfall** Es/N0 (the
//! steep part of the FER curve), calibrated empirically at seed `0xDE16_0FC5`
//! with SumProduct + ExactLogMap so the sweep yields `0 < errors < frames` —
//! a genuine mix of clean and errored frames, **asserted** in every leg. This
//! is the regime where the historical bug class this suite guards (the
//! `81d05bab` preset demapper `noise_var` disconnected from the channel)
//! actually manifests: above threshold, the `errors`/`fer` columns are
//! informationless (0 == 0). Note these points sit below the NMS(0.75)/max-log
//! points in `tests/gpu_byte_identity.rs` — SumProduct + exact log-MAP
//! converges at lower Es/N0, so each point was recalibrated for this
//! decoder/demap pair (32-frame probes, then 200-frame verification).
//!
//! # Graph-construction SSOT
//!
//! The hand-wired chain construction below mirrors the SSOT in
//! `tests/preset_vs_graph.rs` (`build_graph`). That file proves structural +
//! single-frame execution equivalence; this file proves the **run-level**
//! four-column byte-identity over the 50-frame legs. The chain construction
//! is reproduced verbatim here rather than shared via `mod common` because
//! the SSOT helper is private to `preset_vs_graph.rs` and exposing it would
//! require additional public surface.
//!
//! # Frame count — AMENDMENT 2026-06-11
//!
//! The slow legs run **50 frames per MODCOD** per the user-approved
//! AMENDMENT 2026-06-11 on issue `8c8302c8`. The original 200-frame
//! deliverable cannot fit the 120 s slow-tier cap at the waterfall: BP runs
//! near its 50-iteration cap on most frames (`mean_iters` ≈ 44–50) and the
//! stage chain's shared [`DvbT2Concat`] codec serialises decodes on its
//! internal `Mutex` (see the "Throughput caveat" in
//! `gf2-sim/src/executor/topology.rs`) — measured ~1.1–1.5 s per frame
//! regardless of the configured `parallelism`, i.e. 200-frame legs need
//! 171–301 s even with concurrent arms. The one-time 200-frame completion
//! evidence (all six MODCODs byte-identical and non-vacuous at 200 frames)
//! is recorded in the issue. Each leg runs its two arms **concurrently**
//! (each pipeline owns an independent codec, so the two mutexes do not
//! contend); a 50-frame leg fits the unmodified 120 s slow-tier cap and the
//! 2-frame smoke the unmodified 5 s fast-tier cap.
//!
//! # Tiers
//!
//! * **Fast** (`test_preset_vs_graph_smoke_r12_16qam`): 2 frames of r1/2
//!   16-QAM at the 6.0 dB waterfall (the `42eac5cc` smoke precedent point;
//!   1/2 errored — non-vacuous, asserted). ~2–3 s with concurrent arms,
//!   under the unmodified 5 s cap, so the un-ignored smoke genuinely runs
//!   the `[hard]` byte-identity criterion on every green gate.
//! * **Slow** (one `#[ignore = "sim: ..."]` per MODCOD): 50 frames per
//!   MODCOD at its calibrated waterfall point.
//!
//! [`DvbT2Concat`]: gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat

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

/// Base seed for the byte-identity tests (the `de160fc5` waterfall seed; the
/// per-MODCOD Es/N0 points below are calibrated at THIS seed).
const SEED: u64 = 0xDE16_0FC5;

/// Frames for the fast-tier smoke: 2 frames at the 6.0 dB r1/2 16-QAM
/// waterfall — the `42eac5cc` smoke precedent point (frames=2, errors=1 at
/// this exact seed and Es/N0), non-vacuous and within the unmodified 5 s
/// fast-tier cap.
const SMOKE_FRAMES: usize = 2;

/// Frames for the slow-tier legs: 50 per the user-approved AMENDMENT
/// 2026-06-11 on issue `8c8302c8` (the one-time 200-frame completion
/// evidence is recorded in the issue; see the module docs).
const SLOW_FRAMES: usize = 50;

/// Workers on both arms' configs. Identical parallelism + seed on both arms is
/// required; cross-worker-count byte-identity is separately guaranteed by the
/// global-frame keying (§3; `tests/determinism.rs`), so the worker count does
/// not weaken the assertion.
const PARALLELISM: usize = 24;

/// One MODCOD leg: the `(rate, modulation)` pair and its calibrated waterfall
/// Es/N0 (see the module docs; 32-frame probe + 200-frame verification at
/// [`SEED`], SumProduct + ExactLogMap).
struct ModcodPoint {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Waterfall Es/N0 in dB. Calibration evidence — the one-time 200-frame
    /// verification mixes at [`SEED`], recorded in the issue per the
    /// AMENDMENT 2026-06-11: r1/2 16-QAM @6.0 → 58/200; r1/2 64-QAM @10.3 →
    /// 72/200; r2/3 16-QAM @8.8 → 100/200; r2/3 64-QAM @13.8 → 130/200;
    /// r3/4 16-QAM @10.0 → 124/200; r3/4 64-QAM @15.4 → 62/200. The 50-frame
    /// legs observe the prefix of the same global-frame sequence (per-leg
    /// mixes on each test fn below).
    es_n0_db: f32,
    label: &'static str,
}

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
fn build_preset(rate: CodeRate, modulation: DvbT2Modulation, es_n0_db: f32) -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(PARALLELISM).expect("24 is non-zero"))
        .build()
        .expect("in-scope MODCOD builds via the preset")
}

/// Derives the soft demapper's `N0 = 2*sigma^2` from the channel Es/N0, using
/// the same f64-computed, once-rounded arithmetic as the preset's
/// `Channel::demap_noise_var` (the SSOT formula from `frame_sim.rs`).
fn demap_n0(es_n0_db: f32) -> f32 {
    let es_n0_lin = 10.0_f64.powf(f64::from(es_n0_db) / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}

/// Builds the same DVB-T2 BICM chain by hand through the graph [`Chain`] API.
///
/// This mirrors the `build_graph` helper in `tests/preset_vs_graph.rs` (the
/// SSOT for graph-chain structural + single-frame equivalence).
fn build_graph(rate: CodeRate, modulation: DvbT2Modulation, es_n0_db: f32) -> Pipeline {
    let factory = dvb_t2_bicm_stages(
        rate,
        modulation,
        decoder_config(),
        DemapMethod::ExactLogMap,
        demap_n0(es_n0_db),
    );

    let mut chain = Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    // Channel stage: SymbolBatch → SymbolBatch between the forward and inverse
    // halves.
    ids.push(chain.add(erase(Awgn::new(es_n0_db, modulation.bits_per_cell()))));
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
        parallelism: NonZeroUsize::new(PARALLELISM).expect("24 is non-zero"),
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

/// Runs `max_frames` frames of both the preset and graph pipelines at the
/// MODCOD's waterfall point, asserts the sweep is **non-vacuous**
/// (`0 < errors < frames`), then asserts the four byte-identity columns via
/// [`common::assert_four_columns_byte_identical`].
///
/// Both arms are driven through
/// [`TopologyExecutor::run_dvb_t2_snr_point`](gf2_sim::TopologyExecutor::run_dvb_t2_snr_point)
/// at `snr_idx = 0`, so the comparison isolates the API-form difference (the
/// preset's `Pipeline::run()`-vs-stage-driven identity is separately guarded
/// by `tests/stage_driven_byte_identity.rs` and is NOT re-proved here).
///
/// The two arms run **concurrently** (one thread each): each pipeline owns an
/// independent `DvbT2Concat` codec, so their decode mutexes do not contend
/// and the leg wall time halves versus sequential arms. Concurrency cannot
/// affect the outcome — each arm is internally deterministic by the
/// global-frame keying (§3) and shares no state with the other arm.
fn assert_byte_identical(point: &ModcodPoint, max_frames: usize) {
    let ModcodPoint {
        rate,
        modulation,
        es_n0_db,
        label,
    } = *point;

    let (preset_c, graph_c) = std::thread::scope(|s| {
        let preset_arm = s.spawn(move || {
            let pipeline = build_preset(rate, modulation, es_n0_db);
            let scheduler = Scheduler::from_pipeline(&pipeline);
            TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, max_frames)
                .expect("stage-driven preset sweep runs")
        });
        let graph_arm = s.spawn(move || {
            let pipeline = build_graph(rate, modulation, es_n0_db);
            let scheduler = Scheduler::from_pipeline(&pipeline);
            TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, max_frames)
                .expect("stage-driven graph sweep runs")
        });
        let preset_c: WorkerCounters = preset_arm.join().expect("preset arm thread");
        let graph_c: WorkerCounters = graph_arm.join().expect("graph arm thread");
        (preset_c, graph_c)
    });

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

    // Non-vacuous mixed verdict at the waterfall (the §11 regime): without
    // this the `errors`/`fer` columns are informationless (0 == 0).
    assert!(
        preset_c.errors > 0 && preset_c.errors < preset_c.frames,
        "{label}: expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames (re-pin Es/N0 if the chain changes)",
        preset_c.errors,
        preset_c.frames
    );

    common::assert_four_columns_byte_identical(&preset_c, &graph_c, label);

    eprintln!(
        "{label}: frames={} errors={} fer={:.6} mean_iters={:.6} (preset==graph, non-vacuous)",
        preset_c.frames,
        preset_c.errors,
        preset_c.fer(),
        preset_c.mean_iters(),
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Fast-tier smoke (always runs, gates the criterion)
// ──────────────────────────────────────────────────────────────────────────────

/// Fast-tier smoke: 2 frames of r1/2 16-QAM at the 6.0 dB waterfall (the
/// `42eac5cc` smoke precedent point — frames=2, errors=1 at [`SEED`],
/// non-vacuous, asserted).
///
/// Asserts all four byte-identity columns (`frames`, `errors`, `fer`,
/// `mean_iters`) between the typestate-preset form and the hand-wired graph
/// form. Un-ignored so the green `--profile ci` gate genuinely runs the
/// `[hard]` byte-identity criterion at a non-vacuous operating point, within
/// the unmodified 5 s fast-tier cap (the two arms run their 2 waterfall
/// frames concurrently — measured ~3 s).
#[test]
fn test_preset_vs_graph_smoke_r12_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 6.0,
            label: "smoke r1/2 16-QAM @6.0dB",
        },
        SMOKE_FRAMES,
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Slow-tier 50-frame legs (one per MODCOD; AMENDMENT 2026-06-11, module docs)
// ──────────────────────────────────────────────────────────────────────────────

/// Slow: 50 frames, r1/2 16-QAM at the 6.0 dB waterfall (12/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r1/2 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r12_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 6.0,
            label: "50f r1/2 16-QAM @6.0dB",
        },
        SLOW_FRAMES,
    );
}

/// Slow: 50 frames, r1/2 64-QAM at the 10.3 dB waterfall (21/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r1/2 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r12_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 10.3,
            label: "50f r1/2 64-QAM @10.3dB",
        },
        SLOW_FRAMES,
    );
}

/// Slow: 50 frames, r2/3 16-QAM at the 8.8 dB waterfall (27/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r2/3 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r23_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 8.8,
            label: "50f r2/3 16-QAM @8.8dB",
        },
        SLOW_FRAMES,
    );
}

/// Slow: 50 frames, r2/3 64-QAM at the 13.8 dB waterfall (32/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r2/3 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r23_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 13.8,
            label: "50f r2/3 64-QAM @13.8dB",
        },
        SLOW_FRAMES,
    );
}

/// Slow: 50 frames, r3/4 16-QAM at the 10.0 dB waterfall (31/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r3/4 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r34_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 10.0,
            label: "50f r3/4 16-QAM @10.0dB",
        },
        SLOW_FRAMES,
    );
}

/// Slow: 50 frames, r3/4 64-QAM at the 15.4 dB waterfall (17/50 errored).
#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r3/4 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r34_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 15.4,
            label: "50f r3/4 64-QAM @15.4dB",
        },
        SLOW_FRAMES,
    );
}
