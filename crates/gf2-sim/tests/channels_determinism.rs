//! Byte-identity determinism property tests for the AWGN, Rayleigh, and Rician
//! channel stages (issue `db9836e4`).
//!
//! Verifies the [hard] success criterion: the channel output for the same input
//! frame and the same effective §3 seek offset is **bit-identical** (f32
//! bit-level, via `f32::to_bits()`) across worker counts `{1, 4, 24}`.
//!
//! These are `proptest` property tests (the project's property-test framework,
//! per CLAUDE.md) over a randomized base seed and batch dimensions.
//!
//! # Mechanism
//!
//! The determinism guarantee rests on the §3 per-frame seek, owned by the
//! channel's own [`Awgn::apply_for_frame`](gf2_sim::channels::Awgn::apply_for_frame)
//! (and the Rayleigh/Rician equivalents): for each global frame `g` the method
//! calls `WorkerCtx::reseek_to_frame(g)` — which internally performs
//! `set_word_pos(worker_offset(seed, snr_idx, 0, g))` — and then draws noise
//! from that position. Because the noise draw is a pure function of the stream
//! position, the output for frame `g` is **independent of how many workers are
//! running**. Frames are distributed across workers via the same strided
//! assignment `run_snr_point` uses (worker `w` owns `w, w+W, w+2W, ...`).
//!
//! # Test tier
//!
//! * Fast-tier `proptest!` (`cases: 16`): `{1, 4}` workers — runs in well under
//!   5 s.
//! * Slow-tier `#[ignore = "sim: byte-identity {1,4,24}"]` `proptest!`: the full
//!   `{1, 4, 24}` set — `{1,4,24}` legitimately must be slow-tier (24-worker
//!   strided runs over many frames exceed the 5 s nextest limit), mirroring
//!   `tests/parallel_determinism.rs`.

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{worker_offset, WorkerCtx, FRAME_STRIDE};
use proptest::prelude::*;
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

/// Fixed SNR-point index for all determinism runs.
const SNR_IDX: usize = 0;

/// A channel whose per-frame application owns the §3 seek.
///
/// Each variant dispatches to its concrete `apply_for_frame`, so the test
/// exercises the channel's own seek-via-`worker_offset` API rather than seeking
/// in the harness.
enum Channel {
    Awgn(Awgn),
    Rayleigh(Rayleigh),
    Rician(Rician),
}

impl Channel {
    /// Seek `ctx` to global frame `g` and apply the channel in-place.
    fn apply_for_frame(&self, batch: &mut SymbolBatch, ctx: &mut WorkerCtx, g: usize) {
        match self {
            Channel::Awgn(c) => c.apply_for_frame(batch, ctx, g),
            Channel::Rayleigh(c) => c.apply_for_frame(batch, ctx, g),
            Channel::Rician(c) => c.apply_for_frame(batch, ctx, g),
        }
    }
}

/// Produce a deterministic transmitted SymbolBatch for global frame `g`.
///
/// Keyed on `g` only (via a separate RNG stream) so the transmitted symbols are
/// reproducible across all worker counts without consuming the channel's noise
/// stream.
fn make_frame_batch(g: usize, syms: usize) -> SymbolBatch {
    use rand::Rng as _;
    let mut rng = ChaCha20Rng::seed_from_u64(0xDEAD_0000 ^ g as u64);
    let i_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    let q_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    SymbolBatch::new(vec![i_vals], vec![q_vals])
}

/// Run all `n_frames` global frames through `channel` distributed across
/// `num_workers` workers (strided assignment), returning outputs indexed by
/// global frame.
///
/// Each frame's noise is seeked via the channel's own `apply_for_frame` keyed on
/// the global frame index, so the result is independent of the worker count.
fn run_workers(
    channel: &Channel,
    seed: u64,
    num_workers: usize,
    n_frames: usize,
    syms: usize,
) -> Vec<SymbolBatch> {
    let mut frame_outputs: Vec<Option<SymbolBatch>> = (0..n_frames).map(|_| None).collect();
    for worker_idx in 0..num_workers {
        // Logical worker 0: the per-frame seek is keyed on the global frame
        // index (design-doc §3); the physical worker only decides which frames
        // it processes, never the RNG stream those frames see.
        let mut ctx = WorkerCtx::new(seed, SNR_IDX, 0);
        let mut g = worker_idx;
        while g < n_frames {
            let mut batch = make_frame_batch(g, syms);
            channel.apply_for_frame(&mut batch, &mut ctx, g);
            frame_outputs[g] = Some(batch);
            g += num_workers;
        }
    }
    frame_outputs
        .into_iter()
        .map(|b| b.expect("every frame must be processed exactly once"))
        .collect()
}

/// Assert two per-frame output vectors are f32 bit-identical.
fn assert_runs_bit_identical(baseline: &[SymbolBatch], other: &[SymbolBatch], workers: usize) {
    assert_eq!(
        baseline.len(),
        other.len(),
        "frame-output vector lengths differ: {} vs {}",
        baseline.len(),
        other.len()
    );
    for (g, (b, o)) in baseline.iter().zip(other.iter()).enumerate() {
        for (bf, of) in b.i.iter().zip(o.i.iter()) {
            for (s, (&bv, &ov)) in bf.iter().zip(of.iter()).enumerate() {
                assert_eq!(
                    bv.to_bits(),
                    ov.to_bits(),
                    "I[g={g}][sym={s}] differs at {workers} workers: {bv:?} vs {ov:?}"
                );
            }
        }
        for (bf, of) in b.q.iter().zip(o.q.iter()) {
            for (s, (&bv, &ov)) in bf.iter().zip(of.iter()).enumerate() {
                assert_eq!(
                    bv.to_bits(),
                    ov.to_bits(),
                    "Q[g={g}][sym={s}] differs at {workers} workers: {bv:?} vs {ov:?}"
                );
            }
        }
    }
}

/// Build the three channels at a fixed Es/N0 for a property case.
fn channels(es_n0_db: f32) -> [(&'static str, Channel); 3] {
    [
        ("AWGN", Channel::Awgn(Awgn::new(es_n0_db, 4))),
        ("Rayleigh", Channel::Rayleigh(Rayleigh::new(es_n0_db, 4))),
        ("Rician", Channel::Rician(Rician::new(es_n0_db, 4, 2.0))),
    ]
}

/// Core property: for each channel, the per-frame outputs are bit-identical
/// across every worker count in `worker_counts`.
fn check_determinism(seed: u64, n_frames: usize, syms: usize, worker_counts: &[usize]) {
    for (label, channel) in channels(6.25) {
        let baseline = run_workers(&channel, seed, worker_counts[0], n_frames, syms);
        for &w in &worker_counts[1..] {
            let run = run_workers(&channel, seed, w, n_frames, syms);
            // Annotate failures with the channel label.
            assert_eq!(
                baseline.len(),
                run.len(),
                "{label}: frame count mismatch at {w} workers"
            );
            assert_runs_bit_identical(&baseline, &run, w);
        }
    }
}

proptest! {
    // Fast tier: 16 cases, {1, 4} workers, small frame counts → well under 5 s.
    #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]

    /// Byte-identity across {1, 4} workers for AWGN, Rayleigh, and Rician over a
    /// randomized seed, frame count, and symbol count.
    #[test]
    fn prop_channels_byte_identical_fast(
        seed in any::<u64>(),
        n_frames in 1usize..8,
        syms in 1usize..40,
    ) {
        check_determinism(seed, n_frames, syms, &[1, 4]);
    }
}

proptest! {
    // Slow tier: full {1, 4, 24} worker set over more frames. {1,4,24} must be
    // slow-tier (24-worker strided runs exceed the 5 s fast limit), mirroring
    // tests/parallel_determinism.rs. Kept ignored; run with --profile slow.
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    /// Byte-identity across the full {1, 4, 24} worker set for all three
    /// channels over a randomized seed and dimensions.
    #[test]
    #[ignore = "sim: byte-identity {1,4,24}"]
    fn prop_channels_byte_identical_full(
        seed in any::<u64>(),
        n_frames in 24usize..64,
        syms in 8usize..48,
    ) {
        check_determinism(seed, n_frames, syms, &[1, 4, 24]);
    }
}

// ---------------------------------------------------------------------------
// Targeted unit-style checks for the seek contract (fast, deterministic).
// ---------------------------------------------------------------------------

/// The channel's own `apply_for_frame` seeks via `worker_offset`: two
/// independent contexts seeked to the same global frame produce bit-identical
/// AWGN output.
#[test]
fn test_apply_for_frame_seek_determinism() {
    let ch = Awgn::new(6.25, 4);
    let g = 7;

    let mut ctx_a = WorkerCtx::new(0xBEEF_CAFE, SNR_IDX, 0);
    let mut batch_a = make_frame_batch(g, 32);
    ch.apply_for_frame(&mut batch_a, &mut ctx_a, g);

    let mut ctx_b = WorkerCtx::new(0xBEEF_CAFE, SNR_IDX, 0);
    let mut batch_b = make_frame_batch(g, 32);
    ch.apply_for_frame(&mut batch_b, &mut ctx_b, g);

    for (ia, ib) in batch_a.i[0].iter().zip(batch_b.i[0].iter()) {
        assert_eq!(
            ia.to_bits(),
            ib.to_bits(),
            "I differs across two seeks to frame {g}"
        );
    }
    for (qa, qb) in batch_a.q[0].iter().zip(batch_b.q[0].iter()) {
        assert_eq!(
            qa.to_bits(),
            qb.to_bits(),
            "Q differs across two seeks to frame {g}"
        );
    }
}

/// `apply_for_frame` lands the RNG at exactly the `worker_offset` position
/// before drawing (confirms the seek is the §3 `set_word_pos(worker_offset(..))`).
#[test]
fn test_apply_for_frame_lands_on_worker_offset() {
    // A zero-symbol batch draws nothing, so the post-call word position equals
    // the seek target exactly.
    let ch = Awgn::new(6.25, 4);
    let g = 5;
    let seed = 0xBEEF_CAFE;
    let expected = worker_offset(seed, SNR_IDX, 0, g);

    let mut ctx = WorkerCtx::new(seed, SNR_IDX, 0);
    let mut empty = SymbolBatch::new(vec![vec![]], vec![vec![]]);
    ch.apply_for_frame(&mut empty, &mut ctx, g);
    assert_eq!(
        ctx.current_word_pos(),
        expected,
        "apply_for_frame must seek to worker_offset(seed, snr, 0, {g})"
    );
}

/// Distinct frames produce distinct AWGN output (sanity that the seek varies).
#[test]
fn test_distinct_frames_differ() {
    let ch = Awgn::new(6.25, 4);
    let seed = 0xBEEF_CAFE;

    let mut ctx0 = WorkerCtx::new(seed, SNR_IDX, 0);
    let mut b0 = make_frame_batch(0, 32);
    ch.apply_for_frame(&mut b0, &mut ctx0, 0);

    let mut ctx1 = WorkerCtx::new(seed, SNR_IDX, 0);
    let mut b1 = make_frame_batch(1, 32);
    ch.apply_for_frame(&mut b1, &mut ctx1, 1);

    let any_differ = b0.i[0]
        .iter()
        .zip(b1.i[0].iter())
        .any(|(a, b)| a.to_bits() != b.to_bits());
    assert!(
        any_differ,
        "frames 0 and 1 produced identical I outputs — seek bug"
    );
}

/// The per-frame draw stays within the `FRAME_STRIDE - 256` budget for a fading
/// channel (16 words/symbol) at a large symbol count.
#[test]
fn test_fading_draw_within_frame_budget() {
    let ch = Rayleigh::new(6.25, 4);
    let seed = 0xBEEF_CAFE;
    let g = 0;
    let mut ctx = WorkerCtx::new(seed, SNR_IDX, 0);
    // 1000 symbols * 16 words = 16000 words, well within FRAME_STRIDE.
    let mut batch = make_frame_batch(g, 1000);
    ctx.reseek_to_frame(g);
    let start = ctx.current_word_pos();
    ch.apply(&mut batch, ctx.rng_mut());
    let drawn = ctx.current_word_pos() - start;
    // Rayleigh draws 16 words/symbol => 16000 words for 1000 symbols.
    assert_eq!(drawn, 16_000, "Rayleigh must draw 16 words/symbol");
    assert!(
        drawn <= FRAME_STRIDE - 256,
        "Rayleigh draw {drawn} exceeded FRAME_STRIDE - 256 = {}",
        FRAME_STRIDE - 256
    );
}
