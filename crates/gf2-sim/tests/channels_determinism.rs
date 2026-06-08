//! Byte-identity determinism tests for the AWGN, Rayleigh, and Rician channel
//! stages (issue `db9836e4`).
//!
//! Verifies the [hard] success criterion: the channel output for the same input
//! frame and the same effective §3 seek offset is **bit-identical** (f32
//! bit-level, via `f32::to_bits()`) across worker counts `{1, 4, 24}`.
//!
//! # Mechanism
//!
//! The determinism guarantee rests on the §3 per-frame seek: for each global
//! frame `g`, a [`WorkerCtx`] is seeked to
//! `worker_offset(seed, snr_idx, 0, g)` before the channel's `apply` method
//! is called. Because the noise draw is a pure function of the stream position
//! (and `apply` always starts from that same position), the output for frame
//! `g` is **independent of how many workers are running**. This test validates
//! exactly that property for all three channel types.
//!
//! # Test tier
//!
//! * Fast-tier smoke: `{1, 4}` workers × 5 frames — runs in well under 5 s.
//! * Slow-tier: `{1, 4, 24}` workers × 200 frames — marked `#[ignore]`.
//!   Run explicitly with:
//!
//! ```bash
//! cargo nextest run -p gf2-sim --release --profile slow \
//!     --run-ignored ignored-only -E 'test(channels_determinism)'
//! ```

use std::num::NonZeroUsize;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{worker_offset, WorkerCtx, FRAME_STRIDE};
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

/// Worker counts for the fast-tier smoke (1 and 4).
const FAST_WORKER_COUNTS: [usize; 2] = [1, 4];
/// Worker counts for the slow-tier regression (full set).
const SLOW_WORKER_COUNTS: [usize; 3] = [1, 4, 24];

const SEED: u64 = 0xBEEF_CAFE;
const SNR_IDX: usize = 0;
/// Number of symbols per frame in the test batch.
const SYMS_PER_FRAME: usize = 32;
/// Number of frames per SNR point (fast tier).
const FAST_FRAMES: usize = 5;
/// Number of frames per SNR point (slow tier).
const SLOW_FRAMES: usize = 200;

/// Produce a deterministic test SymbolBatch for global frame `g`.
///
/// Uses a separate RNG seeded from `g` so the transmitted symbols are
/// reproducible across all worker counts without drawing from the channel's
/// RNG stream.
fn make_frame_batch(g: usize, syms: usize) -> SymbolBatch {
    // Deterministic transmitted symbols keyed on `g` only.
    use rand::Rng as _;
    let mut rng = ChaCha20Rng::seed_from_u64(0xDEAD_0000 ^ g as u64);
    let i_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    let q_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    SymbolBatch::new(vec![i_vals], vec![q_vals])
}

/// Simulate one frame through an AWGN channel using the §3 seek path.
///
/// This is the pattern the Phase C executor will use: seek the WorkerCtx's RNG
/// to `worker_offset(seed, snr_idx, 0, g)`, then call `channel.apply`.
fn run_awgn_frame(ch: &Awgn, g: usize) -> SymbolBatch {
    let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
    ctx.reseek_to_frame(g);
    let mut batch = make_frame_batch(g, SYMS_PER_FRAME);
    ch.apply(&mut batch, ctx.rng_mut());
    batch
}

/// Simulate all frames for a set of worker counts using strided distribution.
///
/// Each "run" distributes `n_frames` global frames across `num_workers`
/// workers via strided assignment, then collects per-frame results in global
/// frame order.
fn run_awgn_all_workers(
    ch: &Awgn,
    worker_counts: &[usize],
    n_frames: usize,
) -> Vec<Vec<SymbolBatch>> {
    worker_counts
        .iter()
        .map(|&num_workers| {
            // Collect outputs indexed by global frame g.
            let mut frame_outputs: Vec<Option<SymbolBatch>> = (0..n_frames).map(|_| None).collect();
            for worker_idx in 0..num_workers {
                let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
                let mut g = worker_idx;
                while g < n_frames {
                    ctx.reseek_to_frame(g);
                    let mut batch = make_frame_batch(g, SYMS_PER_FRAME);
                    ch.apply(&mut batch, ctx.rng_mut());
                    frame_outputs[g] = Some(batch);
                    g += num_workers;
                }
            }
            frame_outputs
                .into_iter()
                .map(|b| b.expect("every frame must be processed"))
                .collect()
        })
        .collect()
}

fn run_rayleigh_all_workers(
    ch: &Rayleigh,
    worker_counts: &[usize],
    n_frames: usize,
) -> Vec<Vec<SymbolBatch>> {
    worker_counts
        .iter()
        .map(|&num_workers| {
            let mut frame_outputs: Vec<Option<SymbolBatch>> = (0..n_frames).map(|_| None).collect();
            for worker_idx in 0..num_workers {
                let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
                let mut g = worker_idx;
                while g < n_frames {
                    ctx.reseek_to_frame(g);
                    let mut batch = make_frame_batch(g, SYMS_PER_FRAME);
                    ch.apply(&mut batch, ctx.rng_mut());
                    frame_outputs[g] = Some(batch);
                    g += num_workers;
                }
            }
            frame_outputs
                .into_iter()
                .map(|b| b.expect("every frame must be processed"))
                .collect()
        })
        .collect()
}

fn run_rician_all_workers(
    ch: &Rician,
    worker_counts: &[usize],
    n_frames: usize,
) -> Vec<Vec<SymbolBatch>> {
    worker_counts
        .iter()
        .map(|&num_workers| {
            let mut frame_outputs: Vec<Option<SymbolBatch>> = (0..n_frames).map(|_| None).collect();
            for worker_idx in 0..num_workers {
                let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
                let mut g = worker_idx;
                while g < n_frames {
                    ctx.reseek_to_frame(g);
                    let mut batch = make_frame_batch(g, SYMS_PER_FRAME);
                    ch.apply(&mut batch, ctx.rng_mut());
                    frame_outputs[g] = Some(batch);
                    g += num_workers;
                }
            }
            frame_outputs
                .into_iter()
                .map(|b| b.expect("every frame must be processed"))
                .collect()
        })
        .collect()
}

/// Assert all runs produce bit-identical f32 outputs for every frame.
fn assert_bit_identical(all_runs: &[Vec<SymbolBatch>], label: &str, worker_counts: &[usize]) {
    let baseline = &all_runs[0];
    for (run_idx, run) in all_runs.iter().enumerate().skip(1) {
        let workers = worker_counts[run_idx];
        for (g, (base_batch, run_batch)) in baseline.iter().zip(run.iter()).enumerate() {
            for (frame_idx, (base_frame_i, run_frame_i)) in
                base_batch.i.iter().zip(run_batch.i.iter()).enumerate()
            {
                for (sym_idx, (&base_v, &run_v)) in
                    base_frame_i.iter().zip(run_frame_i.iter()).enumerate()
                {
                    assert_eq!(
                        base_v.to_bits(),
                        run_v.to_bits(),
                        "{label}: I[g={g}][frame={frame_idx}][sym={sym_idx}] differs at \
                         {workers} workers: {:?} vs {:?}",
                        base_v,
                        run_v
                    );
                }
            }
            for (frame_idx, (base_frame_q, run_frame_q)) in
                base_batch.q.iter().zip(run_batch.q.iter()).enumerate()
            {
                for (sym_idx, (&base_v, &run_v)) in
                    base_frame_q.iter().zip(run_frame_q.iter()).enumerate()
                {
                    assert_eq!(
                        base_v.to_bits(),
                        run_v.to_bits(),
                        "{label}: Q[g={g}][frame={frame_idx}][sym={sym_idx}] differs at \
                         {workers} workers: {:?} vs {:?}",
                        base_v,
                        run_v
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fast-tier smoke tests ({1, 4} workers × 5 frames)
// ---------------------------------------------------------------------------

/// Fast smoke: AWGN output is bit-identical across {1, 4} workers.
#[test]
fn test_awgn_determinism_fast_smoke() {
    let ch = Awgn::new(6.25, 4);
    let runs = run_awgn_all_workers(&ch, &FAST_WORKER_COUNTS, FAST_FRAMES);
    assert_bit_identical(&runs, "AWGN (fast)", &FAST_WORKER_COUNTS);
}

/// Fast smoke: Rayleigh output is bit-identical across {1, 4} workers.
#[test]
fn test_rayleigh_determinism_fast_smoke() {
    let ch = Rayleigh::new(6.25, 4);
    let runs = run_rayleigh_all_workers(&ch, &FAST_WORKER_COUNTS, FAST_FRAMES);
    assert_bit_identical(&runs, "Rayleigh (fast)", &FAST_WORKER_COUNTS);
}

/// Fast smoke: Rician (K=2) output is bit-identical across {1, 4} workers.
#[test]
fn test_rician_determinism_fast_smoke() {
    let ch = Rician::new(6.25, 4, 2.0);
    let runs = run_rician_all_workers(&ch, &FAST_WORKER_COUNTS, FAST_FRAMES);
    assert_bit_identical(&runs, "Rician (fast)", &FAST_WORKER_COUNTS);
}

/// Fast: verify the §3 seek path — two independent WorkerCtxs seeked to the
/// same (seed, snr_idx, 0, g) produce bit-identical AWGN output.
#[test]
fn test_awgn_seek_determinism() {
    let ch = Awgn::new(6.25, 4);
    let g = 7;

    let out_a = run_awgn_frame(&ch, g);
    let out_b = run_awgn_frame(&ch, g);

    for (ia, ib) in out_a.i[0].iter().zip(out_b.i[0].iter()) {
        assert_eq!(
            ia.to_bits(),
            ib.to_bits(),
            "AWGN I component differs between two independent seeks to frame {g}"
        );
    }
    for (qa, qb) in out_a.q[0].iter().zip(out_b.q[0].iter()) {
        assert_eq!(
            qa.to_bits(),
            qb.to_bits(),
            "AWGN Q component differs between two independent seeks to frame {g}"
        );
    }
}

/// Fast: verify different frames produce different outputs.
#[test]
fn test_awgn_distinct_frames_differ() {
    let ch = Awgn::new(6.25, 4);
    let out_0 = run_awgn_frame(&ch, 0);
    let out_1 = run_awgn_frame(&ch, 1);

    // It is astronomically unlikely for frames 0 and 1 to produce
    // identical noise — assert at least one sample differs.
    let any_differ = out_0.i[0]
        .iter()
        .zip(out_1.i[0].iter())
        .any(|(a, b)| a.to_bits() != b.to_bits());
    assert!(
        any_differ,
        "frames 0 and 1 produced identical I outputs — suggests a seek bug"
    );
}

/// Fast: verify debug_assert budget — draw count must be within FRAME_STRIDE - 256.
///
/// We verify indirectly: simulate one frame and check the word position
/// advanced by less than FRAME_STRIDE words from the seek point.
#[test]
fn test_awgn_draw_within_frame_budget() {
    let ch = Awgn::new(6.25, 4);
    let g = 0;
    let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
    ctx.reseek_to_frame(g);
    let start = ctx.current_word_pos();
    let mut batch = make_frame_batch(g, 1000); // 1000 symbols
    ch.apply(&mut batch, ctx.rng_mut());
    let drawn = ctx.current_word_pos() - start;
    assert!(
        drawn <= FRAME_STRIDE - 256,
        "AWGN draw {drawn} exceeded FRAME_STRIDE - 256 = {}",
        FRAME_STRIDE - 256
    );
}

// ---------------------------------------------------------------------------
// Slow-tier regression ({1, 4, 24} workers × 200 frames)
// ---------------------------------------------------------------------------

/// Slow regression: AWGN byte-identical across {1, 4, 24} workers × 200 frames.
#[test]
#[ignore = "sim: AWGN byte-identity across worker counts {1,4,24}"]
fn test_awgn_determinism_full() {
    let ch = Awgn::new(6.25, 4);
    let runs = run_awgn_all_workers(&ch, &SLOW_WORKER_COUNTS, SLOW_FRAMES);
    assert_bit_identical(&runs, "AWGN (full)", &SLOW_WORKER_COUNTS);
}

/// Slow regression: Rayleigh byte-identical across {1, 4, 24} workers × 200 frames.
#[test]
#[ignore = "sim: Rayleigh byte-identity across worker counts {1,4,24}"]
fn test_rayleigh_determinism_full() {
    let ch = Rayleigh::new(6.25, 4);
    let runs = run_rayleigh_all_workers(&ch, &SLOW_WORKER_COUNTS, SLOW_FRAMES);
    assert_bit_identical(&runs, "Rayleigh (full)", &SLOW_WORKER_COUNTS);
}

/// Slow regression: Rician (K=2) byte-identical across {1, 4, 24} workers × 200 frames.
#[test]
#[ignore = "sim: byte-identity across worker counts {1,4,24}"]
fn test_rician_determinism_full() {
    let ch = Rician::new(6.25, 4, 2.0);
    let runs = run_rician_all_workers(&ch, &SLOW_WORKER_COUNTS, SLOW_FRAMES);
    assert_bit_identical(&runs, "Rician (full)", &SLOW_WORKER_COUNTS);
}

// Keep the `worker_offset` import live — used in the fast seek test below
// to confirm the arithmetic matches WorkerCtx.
#[test]
fn test_worker_offset_seek_matches_ctx() {
    let g = 5;
    let expected_pos = worker_offset(SEED, SNR_IDX, 0, g);

    let mut ctx = WorkerCtx::new(SEED, SNR_IDX, 0);
    ctx.reseek_to_frame(g);
    assert_eq!(
        ctx.current_word_pos(),
        expected_pos,
        "WorkerCtx seek for frame {g} must match worker_offset arithmetic"
    );
}

// Make `NonZeroUsize` used (suppress unused-import warning).
const _: () = {
    let _ = NonZeroUsize::new(1);
};
