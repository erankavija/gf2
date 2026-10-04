//! Property tests: the AWGN, Rayleigh and Rician channel output for a given
//! input frame and seed is f32 bit-identical across worker counts `{1, 4, 24}`.
//! Each channel's `apply_for_frame` reseeks the worker RNG to the global frame
//! index, and frames are distributed over workers with the strided assignment
//! `run_snr_point` uses.

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{worker_offset, WorkerCtx, FRAME_STRIDE};
use proptest::prelude::*;
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

const SNR_IDX: usize = 0;

enum Channel {
    Awgn(Awgn),
    Rayleigh(Rayleigh),
    Rician(Rician),
}

impl Channel {
    fn apply_for_frame(&self, batch: &mut SymbolBatch, ctx: &mut WorkerCtx, g: usize) {
        match self {
            Channel::Awgn(c) => c.apply_for_frame(batch, ctx, g),
            Channel::Rayleigh(c) => c.apply_for_frame(batch, ctx, g),
            Channel::Rician(c) => c.apply_for_frame(batch, ctx, g),
        }
    }
}

/// Keyed on `g` through a separate RNG, so the transmitted symbols do not
/// consume the channel's noise stream.
fn make_frame_batch(g: usize, syms: usize) -> SymbolBatch {
    use rand::Rng as _;
    let mut rng = ChaCha20Rng::seed_from_u64(0xDEAD_0000 ^ g as u64);
    let i_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    let q_vals: Vec<f32> = (0..syms).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
    SymbolBatch::new(vec![i_vals], vec![q_vals])
}

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
        // index, so the physical worker only decides which frames it processes.
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

fn channels(es_n0_db: f32) -> [(&'static str, Channel); 3] {
    [
        ("AWGN", Channel::Awgn(Awgn::new(es_n0_db, 4))),
        ("Rayleigh", Channel::Rayleigh(Rayleigh::new(es_n0_db, 4))),
        ("Rician", Channel::Rician(Rician::new(es_n0_db, 4, 2.0))),
    ]
}

fn check_determinism(seed: u64, n_frames: usize, syms: usize, worker_counts: &[usize]) {
    for (label, channel) in channels(6.25) {
        let baseline = run_workers(&channel, seed, worker_counts[0], n_frames, syms);
        for &w in &worker_counts[1..] {
            let run = run_workers(&channel, seed, w, n_frames, syms);
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
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    #[test]
    fn prop_channels_byte_identical_full(
        seed in any::<u64>(),
        n_frames in 24usize..64,
        syms in 8usize..48,
    ) {
        check_determinism(seed, n_frames, syms, &[1, 4, 24]);
    }
}

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

#[test]
fn test_fading_draw_within_frame_budget() {
    let ch = Rayleigh::new(6.25, 4);
    let seed = 0xBEEF_CAFE;
    let g = 0;
    let mut ctx = WorkerCtx::new(seed, SNR_IDX, 0);
    let mut batch = make_frame_batch(g, 1000);
    ctx.reseek_to_frame(g);
    let start = ctx.current_word_pos();
    ch.apply(&mut batch, ctx.rng_mut());
    let drawn = ctx.current_word_pos() - start;
    assert_eq!(drawn, 16_000, "Rayleigh must draw 16 words/symbol");
    assert!(
        drawn <= FRAME_STRIDE - 256,
        "Rayleigh draw {drawn} exceeded FRAME_STRIDE - 256 = {}",
        FRAME_STRIDE - 256
    );
}
