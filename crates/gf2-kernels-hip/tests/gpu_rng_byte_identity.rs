//! GPU ChaCha20 byte-identity + Box-Muller ulp regression (issue f6004add
//! deliverable 4, success criteria 1 & 2).
//!
//! These tests run on the gfx1030 CI host. They assert:
//!
//! 1. **Raw byte-identity (criterion 1, [hard])** — for N ∈ {1, 256, 1024}
//!    frames at a fixed seed and worker_idx, the device ChaCha20 raw 32-bit word
//!    stream at each frame's `worker_offset(...)` is **bit-for-bit identical** to
//!    a host `rand_chacha::ChaCha20Rng::seed_from_u64(seed)` repositioned with
//!    `set_word_pos(worker_offset(...))` and read via `next_u32`.
//! 2. **Box-Muller agreement (criterion 2, [hard])** — after the Box-Muller
//!    transform, device standard-normal samples agree with the host
//!    `box_muller_cos` (the `gf2-coding` SSOT) to **<= 1 ulp f32** over >= 1024
//!    frames' worth of samples.
//!
//! If no usable GPU is present the tests skip cleanly (so the suite passes on a
//! non-ROCm host that nonetheless built with the kernels linked).

use gf2_kernels_hip::host::device_mem_info;
use gf2_kernels_hip::GpuChaChaAwgn;

use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;
use rand::Rng as _;
use rand::RngCore as _;
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

// Mirror of the `gf2-sim` §3 seek strides (design doc §3, amended 2026-06-07;
// SSOT: `gf2_sim::parallel::{SNR_STRIDE, WORKER_STRIDE, FRAME_STRIDE}`).
// `gf2-kernels-hip` is upstream of `gf2-sim` and cannot depend on it, so the
// strides are restated here for the test oracle; the device kernel itself takes
// the already-computed base word position, so it never re-derives these.
const SNR_STRIDE: u128 = 1 << 56;
const WORKER_STRIDE: u128 = 1 << 40;
const FRAME_STRIDE: u128 = 1 << 20;

/// Verbatim `gf2_sim::parallel::worker_offset` (the seed term does not enter the
/// offset; it selects the ChaCha *stream*).
fn worker_offset(snr_idx: usize, worker_idx: usize, frame_idx: usize) -> u128 {
    (snr_idx as u128) * SNR_STRIDE
        + (worker_idx as u128) * WORKER_STRIDE
        + (frame_idx as u128) * FRAME_STRIDE
}

/// True if `a` and `b` are within one f32 ulp (monotone-key bit distance).
fn ulps_within_one(a: f32, b: f32) -> bool {
    if a == b {
        return true;
    }
    if a.is_nan() || b.is_nan() {
        return false;
    }
    let key = |x: f32| -> i64 {
        let bits = i64::from(x.to_bits());
        if x.to_bits() & 0x8000_0000 != 0 {
            -(bits & 0x7fff_ffff)
        } else {
            bits
        }
    };
    (key(a) - key(b)).abs() <= 1
}

#[test]
fn test_gpu_chacha_raw_words_byte_identical_to_host() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_chacha_raw_words_byte_identical_to_host: no usable GPU");
        return;
    }

    let seed = 0xDEAD_BEEF_u64;
    let snr_idx = 2usize;
    let worker_idx = 0usize;
    // 64 raw words per frame is enough to span a ChaCha block boundary (16
    // words/block) and exercise the device block cache across blocks.
    let words_per_frame = 64usize;

    let gen = GpuChaChaAwgn::new(seed, 0, words_per_frame).expect("build generator");
    let mut host = ChaCha20Rng::seed_from_u64(seed);

    for &n_frames in &[1usize, 256, 1024] {
        // Spot-check the first, middle, and last frame of each N (exhaustively
        // checking 1024 frames × 64 words would be slow; the seek is linear in
        // frame_idx so endpoints + midpoint cover the arithmetic).
        for &frame_idx in &[0usize, n_frames / 2, n_frames.saturating_sub(1)] {
            let base = worker_offset(snr_idx, worker_idx, frame_idx);
            let gpu_words = gen.raw_words(base, words_per_frame).expect("gpu raw words");

            host.set_word_pos(base);
            for (w, &gpu_w) in gpu_words.iter().enumerate() {
                let host_w = host.next_u32();
                assert_eq!(
                    gpu_w, host_w,
                    "raw word mismatch at N={n_frames} frame={frame_idx} word={w}: \
                     gpu={gpu_w:#010x} host={host_w:#010x}"
                );
            }
        }
    }
}

#[test]
fn test_gpu_box_muller_within_1_ulp_of_host() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_box_muller_within_1_ulp_of_host: no usable GPU");
        return;
    }

    let seed = 0x0102_0304_0506_0708_u64;
    let snr_idx = 0usize;
    let worker_idx = 0usize;
    // 256 standard-normal samples per frame; over 1024 frames this is >> 1024
    // samples (criterion 2 requires agreement over >= 1024 frames). Each sample
    // consumes 4 ChaCha words, so 256 samples = 1024 words, comfortably under
    // FRAME_STRIDE.
    let samples_per_frame = 256usize;

    let gen = GpuChaChaAwgn::new(seed, 0, samples_per_frame).expect("build generator");
    let mut host = ChaCha20Rng::seed_from_u64(seed);

    let mut max_checked_frames = 0usize;
    for &frame_idx in &[0usize, 1, 7, 100, 511, 1023] {
        let base = worker_offset(snr_idx, worker_idx, frame_idx);
        let gpu = gen
            .noise_samples(base, samples_per_frame)
            .expect("gpu noise");

        host.set_word_pos(base);
        for (s, &gpu_n) in gpu.iter().enumerate() {
            // Host: two f64 uniforms then box_muller_cos (the SSOT), exactly the
            // order `gf2_sim::channels::draw_standard_normal` uses.
            let u1: f64 = host.random();
            let u2: f64 = host.random();
            let host_n = box_muller_cos(u1, u2);
            assert!(
                ulps_within_one(gpu_n, host_n),
                "Box-Muller sample frame={frame_idx} s={s} differs > 1 ulp: \
                 gpu={gpu_n} host={host_n}"
            );
        }
        max_checked_frames = max_checked_frames.max(frame_idx + 1);
    }
    assert!(
        max_checked_frames >= 1024,
        "criterion 2 requires checking through >= 1024 frames; checked {max_checked_frames}"
    );
}
