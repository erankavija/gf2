//! End-to-end CPU-vs-GPU byte-identity of the full DVB-T2 BICM chain verdict
//! (issue `14f59c2d`, the Phase B closer; design doc §5, §6, §11).
//!
//! This is the **chain-level** counterpart of the two per-kernel byte-identity
//! suites (`gpu_ldpc_byte_identity.rs` for the BP hard decision, issue
//! `a930be7f`; `gpu_demap_byte_identity.rs` for the max-log demap, issue
//! `d3f1616a`). It composes the validated building blocks — encode, interleave,
//! Gray-QAM map, AWGN, demap, deinterleave, LDPC BP decode, BCH outer decode —
//! and asserts the **design-doc §11 CPU-vs-GPU relaxed contract** end-to-end:
//! the three columns `fer / frames / errors` are byte-identical across the
//! CPU-only path and the CPU+GPU path at a fixed seed, for the three named
//! (rate, modulation) configurations on gfx1030.
//!
//! # The three §11 columns are FRAME-level (not bit-level)
//!
//! The determinism SSOT is [`WorkerCounters`](gf2_sim::parallel::WorkerCounters)
//! (`errors += u64::from(errored)` — one count per *errored frame*) and the
//! comparison helper `tests/common/mod.rs`. The three byte-identical columns are
//! therefore:
//!   - `frames` — number of frames simulated (`u64`),
//!   - `errors` — number of **errored frames** (a frame is errored iff any
//!     information bit differs from the TX message),
//!   - `fer` — `errors / frames` (the frame-error ratio).
//!
//! The **bit-error count** (sum of mismatched info bits) is NOT one of the three
//! columns: `ber = total_bit_errors / total_bits` is **excluded** from
//! byte-identity (non-associative f32 reduction; design §11 "Always-excluded:
//! ber"; status-quo amendment `152388f4`). This suite therefore asserts the
//! FRAME-error columns and only LOGS the bit-error sum (like `mean_iters`),
//! never asserting it.
//!
//! # Waterfall operating point — the regime §11 is about (non-vacuous)
//!
//! Each config runs at a **waterfall** Es/N0 (the steep part of the FER curve,
//! ~0.5-1.5 dB below the TS 102 831 QEF C/N threshold), chosen so the 200-frame
//! sweep yields a non-trivial mix `0 < errored_frames < frames`: some frames
//! decode cleanly, some error. This is exactly the regime §11 names verbatim —
//! "For LDPC BP **near** the convergence threshold, ULP differences ... can
//! change the iteration ... The frame's final verdict ... is robust to that
//! drift; `fer`/`frames`/`errors` remain byte-identical." The suite asserts the
//! sweep is non-vacuous (total errored frames across the suite > 0) so the
//! verdict boundary is genuinely exercised, then asserts the three columns
//! byte-identical there. (Running above threshold, where every frame decodes,
//! would make the asserts 0==0 and exercise no verdict — that is not the
//! contract's regime.)
//!
//! # Why hand-composed (Phase C executor does not exist yet)
//!
//! The hybrid CPU/GPU executor (`75c22fa8` / `de160fc5`) is downstream of this
//! task, so both paths are hand-composed here, mirroring the CPU frame kernel
//! [`DvbT2BicmFrameSim`](gf2_sim::frame_sim::DvbT2BicmFrameSim) stage-for-stage.
//! The two paths differ **only** in which device computes the GPU-eligible
//! stages (demap + LDPC BP); every other stage is shared, and crucially the
//! **same transmitted message, same codeword, and SAME noisy received symbols**
//! feed both demappers. (The *runnable-`Pipeline`*-driven CPU-vs-GPU regression
//! — once the Phase C executor lands — is a separate deliverable owned by D.3
//! `0d9cb8e3`; this hand-composed suite is the Phase B closer.)
//!
//! # Stage coverage of each path
//!
//! Shared per frame (computed once, fed identically to both paths):
//!   1. random k_bch BBFRAME message → BCH+LDPC encode (CPU) → n-bit codeword;
//!   2. bit-interleave → Gray-QAM map (CPU) → tx I/Q symbols;
//!   3. ONE shared AWGN noise realisation (Box-Muller) → noisy rx I/Q symbols.
//!
//! GPU AWGN is **not** used here: its bit-identity to the CPU noise is already
//! proven separately by `f6004add`, and sharing one noise realisation isolates
//! the comparison to the demap→decode verdict — exactly what the §11 contract is
//! about. Feeding the identical `SymbolBatch` to both demappers keeps the
//! comparison valid without re-proving GPU AWGN here.
//!
//! CPU-only path (steps 4–7 on CPU, run across the rayon pool):
//!   4. CPU [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper)
//!      **max-log** demap;
//!   5. bit-deinterleave LLRs → FECFRAME order;
//!   6. CPU LDPC BP decode + BCH outer decode via
//!      [`DvbT2Concat::decode_soft_counted`](gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat);
//!   7. frame-error verdict (any info-bit mismatch) vs the TX message.
//!
//! CPU+GPU path (demap + LDPC BP on GPU in single batched launches; BCH on CPU):
//!   4. GPU [`GpuGrayQamDemapper`](gf2_sim::gpu::demap::GpuGrayQamDemapper)
//!      **max-log** demap;
//!   5. bit-deinterleave LLRs → FECFRAME order;
//!   6. GPU [`GpuLdpcBp`](gf2_sim::gpu::ldpc_bp::GpuLdpcBp) BP decode → n-bit
//!      codeword → extract first k_ldpc bits → CPU [`BchDecoder`] outer decode
//!      (the same `BchCode::dvb_t2` SSOT `DvbT2Concat::new` builds internally);
//!   7. frame-error verdict vs the TX message.
//!
//! # MAX-LOG on BOTH sides (apples-to-apples)
//!
//! [`GpuGrayQamDemapper`] is **max-log only** (no GPU `ExactLogMap`), so BOTH
//! paths use [`DemapMethod::MaxLog`]. Comparing GPU max-log against CPU
//! `ExactLogMap` would be invalid; the comparison here is GPU-max-log vs
//! CPU-max-log throughout.
//!
//! # What is asserted, and what is NOT
//!
//! - **Asserted byte-identical**: `frames`, `errors` (FRAME errors), `fer` (the
//!   three columns of the CPU-vs-GPU relaxed contract).
//! - **Logged, NOT asserted**: `mean_iters` for BOTH paths and the diff (§11
//!   CPU-vs-GPU exclusion — RDNA2 transcendental ULP drift can shift BP
//!   early-termination by ±1). The GPU per-frame iteration counts come from
//!   [`GpuLdpcBp::decode_batch_with_iters`] (the CPU-aligned-convention additive
//!   API), so the logged CPU-vs-GPU `mean_iters` diff is the genuine ±1
//!   near-threshold drift. Also logged: the bit-error sum (the BER numerator).
//! - **Excluded entirely**: `ber` (non-associative f32 reduction; `152388f4`).
//!
//! # The hard escalation boundary
//!
//! At the waterfall, GPU-demap+GPU-BP drift *could* flip a borderline frame's
//! verdict vs CPU-demap+CPU-BP; §11 claims it will NOT. If a frame errors on one
//! path but not the other (the FRAME-error / `fer` columns diverge), that is a
//! genuine §11 violation: this test PANICS with the exact (config, frame, CPU vs
//! GPU verdict) — it does NOT relax the criterion and does NOT move the operating
//! point to dodge it. Resolution is a §11-scope user decision (see the issue's
//! HARD ESCALATION TRIGGER), not a test edit.
//!
//! Gated on GPU presence (skips cleanly when `device_mem_info().is_err()`, like
//! the other `gf2-sim` GPU tests). Split into THREE `#[ignore]` tests (one per
//! config) so each stays under the 120 s slow-tier cap; the CPU path runs across
//! rayon and the GPU demap+decode run as single batched launches. Single gfx1030
//! → never assumes concurrent GPU suites.

#![cfg(feature = "hip")]

use gf2_coding::bch::dvb_t2::FrameSize as BchFrameSize;
use gf2_coding::bch::{BchCode, BchDecoder};
use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;
use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::{ConcatError, DvbT2Concat};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode};
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
};
use gf2_coding::simulation::count_bit_errors;
use gf2_coding::traits::HardDecisionDecoder;
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::batch::SymbolBatch;
use gf2_sim::gpu::demap::GpuGrayQamDemapper;
use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::LlrBatch;
use rayon::prelude::*;

mod common;
use common::assert_three_columns_byte_identical_log_mean_iters;

/// BP iteration cap (the DVB-T2 default; matches `DvbT2Concat`'s 50).
const MAX_LDPC_ITERATIONS: usize = 50;

/// Frames per config (the §11 contract sweep size, matching the per-kernel
/// suites).
const FRAMES_PER_CONFIG: usize = 200;

/// One (rate, modulation, Es/N0) config plus its per-config seed.
///
/// # Es/N0 selection — the §11 waterfall regime (non-vacuous, NOT above-threshold)
///
/// Each Es/N0 sits in the **waterfall** (steep part of the FER curve), ~0.5-1.5
/// dB **below** the config's TS 102 831 Table 44 QEF C/N threshold (r1/2 16-QAM
/// 6.0 dB, r2/3 64-QAM 13.5 dB, r3/4 16-QAM 10.0 dB), calibrated empirically so
/// the 200-frame sweep yields `0 < errored_frames < frames` — a non-trivial mix
/// of clean decodes and errored frames. This is the regime §11 names verbatim:
/// "near the convergence threshold ... the frame's final verdict ... is robust
/// to that drift; `fer`/`frames`/`errors` remain byte-identical." The suite
/// asserts the sweep is non-vacuous, so the verdict boundary — where GPU drift
/// could flip a frame's verdict — is actually exercised.
struct Config {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    seed: u64,
    label: &'static str,
}

/// Aggregated FRAME-level verdict over one config's sweep, plus logged-only
/// diagnostics (bit-error sum, BP-iteration sum). The three asserted columns are
/// `frames`, `errored_frames` (the §11 `errors`), and `fer`.
#[derive(Debug, Default, Clone, Copy)]
struct Counters {
    /// Number of frames simulated (the §11 `frames` column).
    frames: u64,
    /// Number of **errored frames** (the §11 `errors` column; one per frame
    /// whose decoded BBFRAME differs from the TX message in any info bit).
    errored_frames: u64,
    /// Sum of mismatched info bits across frames (the BER numerator). LOGGED
    /// ONLY — `ber` is excluded from byte-identity (`152388f4`).
    bit_error_sum: u64,
    /// Sum of BP iterations across frames (CPU from `decode_soft_counted`, GPU
    /// from `decode_batch_with_iters`; both CPU-aligned-convention). LOGGED ONLY
    /// (`mean_iters` excluded for CPU-vs-GPU). `None` only when a path does not
    /// supply iteration counts (not used on either arm here).
    iter_sum: Option<u64>,
}

impl Counters {
    /// Frame error rate `errored_frames / frames` (the `fer` column).
    fn fer(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.errored_frames as f64 / self.frames as f64
        }
    }

    /// Adapts this suite-local accumulator to the SSOT
    /// [`WorkerCounters`] so the shared three-column comparator in
    /// `tests/common` stays the single implementation of the §11 relaxed
    /// contract (including the logged-only `mean_iters`, computed there as
    /// `total_iterations / frames`). `total_bits` is not tracked by this
    /// harness (BER is excluded entirely, `152388f4`); it maps to 0 and the
    /// comparator never reads it. `iter_sum` is `Some` on both arms here
    /// (see field docs); `unwrap_or(0)` only guards a hypothetical
    /// iteration-less path.
    fn to_worker_counters(self) -> WorkerCounters {
        WorkerCounters {
            frames: self.frames,
            errors: self.errored_frames,
            total_iterations: self.iter_sum.unwrap_or(0),
            total_bits: 0,
            total_bit_errors: self.bit_error_sum,
        }
    }
}

/// Deterministic SplitMix64 stream — the single shared randomness source for one
/// config's sweep. Both paths consume the identical message bits and identical
/// noise samples from this stream, so the ONLY difference between paths is which
/// device runs the demap + LDPC BP.
///
/// NOT a copy of `gf2_sim::testutil::AwgnLlrSource` (review F3): this is a raw
/// u64 word stream feeding the production `dvb_t2_bicm_harness::box_muller_cos`
/// shared-noise-realisation harness with its own §5-pinned draw order — a
/// different generator contract, deliberately not folded.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f64` in `[0, 1)` (top 53 bits / 2^53).
    fn next_uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
}

/// Builds a `len_bits`-bit [`BitVec`] from the shared stream (one `u64` per
/// word, tail-masked per the `gf2-core` tail-masking invariant).
fn random_bitvec(len_bits: usize, rng: &mut SplitMix64) -> BitVec {
    if len_bits == 0 {
        return BitVec::new();
    }
    let num_words = len_bits.div_ceil(64);
    let mut data: Vec<u64> = (0..num_words).map(|_| rng.next_u64()).collect();
    let tail = len_bits & 63;
    if tail != 0 {
        let mask = (1u64 << tail) - 1;
        let last = num_words - 1;
        data[last] &= mask;
    }
    BitVec::from_words(data, len_bits)
}

/// AWGN per-axis sigma and per-symbol N0 from an Es/N0 (dB), using the SAME
/// `sigma^2 = 1 / (2 * 10^(Es/N0 / 10))`, `N0 = 2 sigma^2` formula as
/// [`DvbT2BicmFrameSim`](gf2_sim::frame_sim::DvbT2BicmFrameSim).
fn awgn_params(es_n0_db: f64) -> (f32, f32) {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    let sigma = (sigma_sq as f32).sqrt();
    let noise_var = (2.0 * sigma_sq) as f32;
    (sigma, noise_var)
}

/// The shared per-frame state both paths consume identically: the transmitted
/// info-bit message and the ONE noisy received-symbol realisation.
struct SharedFrame {
    message: BitVec,
    rx_i: Vec<f32>,
    rx_q: Vec<f32>,
}

/// Encodes a random message and produces the single noisy received-symbol
/// realisation shared by both paths.
///
/// Uses the SSOT building blocks verbatim — [`DvbT2Concat::encode`],
/// [`DvbT2BitInterleaver::interleave`], the `ModemSpec::gray_square_qam`
/// preferred mapper, and [`box_muller_cos`] — exactly as the production frame
/// kernel composes them; no FEC / modem math is reimplemented. The noise draw
/// contract matches `BicmAwgnChannel`: all I-axis samples first, then all Q-axis
/// samples.
fn make_shared_frame(
    codec: &DvbT2Concat,
    interleaver: &DvbT2BitInterleaver,
    mapper: &dyn gf2_coding::modem::BatchMapper<f32>,
    bits_per_symbol: usize,
    sigma: f32,
    k_bch: usize,
    rng: &mut SplitMix64,
) -> SharedFrame {
    // 1. Random BBFRAME → BCH+LDPC encode → n-bit FECFRAME codeword.
    let message = random_bitvec(k_bch, rng);
    let codeword = codec.encode(&message);
    let n_ldpc = codeword.len();
    let num_symbols = n_ldpc / bits_per_symbol;

    // 2. Bit-interleave → Gray-QAM map → tx I/Q symbols.
    let interleaved = interleaver.interleave(&codeword);
    let interleaved_bits: Vec<bool> = (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
    let mut rx_i = vec![0.0_f32; num_symbols];
    let mut rx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut rx_i, &mut rx_q);

    // 3. ONE shared AWGN realisation: I axis (all symbols) then Q axis (all
    //    symbols), matching the `BicmAwgnChannel` draw contract.
    for s in rx_i.iter_mut() {
        let u1 = rng.next_uniform();
        let u2 = rng.next_uniform();
        *s += sigma * box_muller_cos(u1, u2);
    }
    for s in rx_q.iter_mut() {
        let u1 = rng.next_uniform();
        let u2 = rng.next_uniform();
        *s += sigma * box_muller_cos(u1, u2);
    }

    SharedFrame {
        message,
        rx_i,
        rx_q,
    }
}

/// Per-frame verdict: `(errored, bit_errors)`. `errored` is the §11 frame-error
/// flag (any info-bit mismatch); `bit_errors` is the logged-only bit count.
#[derive(Clone, Copy)]
struct Verdict {
    errored: bool,
    bit_errors: u64,
}

/// Runs both paths over one config's frame sweep and returns
/// `(cpu_counters, gpu_counters)`. Panics with the precise divergence detail on
/// the FIRST per-frame FRAME-verdict mismatch (the escalation contract).
fn run_config(cfg: &Config) -> (Counters, Counters) {
    let (sigma, noise_var) = awgn_params(cfg.es_n0_db);

    // Shared / CPU back-end: the production DVB-T2 codec.
    let mut codec = DvbT2Concat::new(FrameSize::Normal, cfg.rate)
        .expect("DVB-T2 Normal codec for in-scope rate");
    let decoder_config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    codec.set_decoder_config(decoder_config);
    let k_bch = codec.k_bch();
    let k_ldpc = codec.k_ldpc();

    let bits_per_symbol = cfg.modulation.bits_per_cell();
    let modcod = DvbT2Modcod::new(FrameSize::Normal, cfg.rate, cfg.modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);

    // Mapper from the SAME `gray_square_qam(order)` SSOT the production chain
    // (`BicmAwgnChannel`) builds.
    let order = 1usize << bits_per_symbol;
    let spec = ModemSpec::<f32>::gray_square_qam(order);
    let mapper = spec.preferred_mapper();

    // ---- Generate all shared frames up front (encode + map + shared noise) ----
    let mut rng = SplitMix64::new(cfg.seed);
    let frames: Vec<SharedFrame> = (0..FRAMES_PER_CONFIG)
        .map(|_| {
            make_shared_frame(
                &codec,
                &interleaver,
                mapper.as_ref(),
                bits_per_symbol,
                sigma,
                k_bch,
                &mut rng,
            )
        })
        .collect();

    let ldpc_code = LdpcCode::dvb_t2_normal(cfg.rate);
    let n_ldpc = ldpc_code.n();

    // ---- CPU-only path: demap + deinterleave + LDPC+BCH per frame, across the
    // rayon pool. Each frame's outcome is a pure function of its shared inputs
    // (own one-shot codec per frame), so the result is thread-independent. ----
    let cpu_results: Vec<(Verdict, u64)> = frames
        .par_iter()
        .map(|frame| {
            let num_symbols = frame.rx_i.len();
            let cpu_demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order));
            let nv = vec![noise_var; num_symbols];
            let mut interleaved_llrs = vec![Llr::zero(); n_ldpc];
            cpu_demapper.demap_llrs(
                DemapInput {
                    rx_i: &frame.rx_i,
                    rx_q: &frame.rx_q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &nv,
                    method: DemapMethod::MaxLog,
                },
                &mut interleaved_llrs,
            );
            let llrs = interleaver.deinterleave_llrs(&interleaved_llrs);
            // Fresh per-frame codec so the rayon map is data-race-free and the
            // outcome is thread-independent.
            let mut frame_codec = DvbT2Concat::new(FrameSize::Normal, cfg.rate)
                .expect("DVB-T2 Normal codec for in-scope rate");
            frame_codec.set_decoder_config(decoder_config);
            let (bbframe, iters) = match frame_codec.decode_soft_counted(&llrs) {
                Ok((bb, it)) => (bb, it as u64),
                Err(ConcatError::LdpcDecodeFailed {
                    bbframe,
                    iterations,
                }) => (bbframe, iterations as u64),
                Err(_) => (BitVec::with_capacity(k_bch), MAX_LDPC_ITERATIONS as u64),
            };
            let bit_errors = count_bit_errors(&frame.message, &bbframe) as u64;
            (
                Verdict {
                    errored: bit_errors > 0,
                    bit_errors,
                },
                iters,
            )
        })
        .collect();

    // ---- CPU+GPU path: ONE batched GPU demap + ONE batched GPU LDPC decode over
    // the whole sweep, then CPU BCH per frame (across rayon). ----
    let gpu_demap_stage = GpuGrayQamDemapper::new(cfg.modulation, DemapMethod::MaxLog, noise_var);
    // demapper `max_batch` is in *symbols* (one frame's worth).
    let symbols_per_frame = n_ldpc / bits_per_symbol;
    let gpu_demapper = gpu_demap_stage
        .build_demapper(symbols_per_frame)
        .expect("build GPU demapper on gfx1030");
    let gpu_ldpc_stage = GpuLdpcBp::new(ldpc_code, decoder_config, MAX_LDPC_ITERATIONS);
    // decoder `max_batch` is in *frames*.
    let gpu_ldpc_decoder = gpu_ldpc_stage
        .build_decoder(FRAMES_PER_CONFIG)
        .expect("build GPU LDPC decoder on gfx1030");

    // Batched GPU max-log demap over all frames (one launch per frame inside).
    let rx_i_all: Vec<Vec<f32>> = frames.iter().map(|f| f.rx_i.clone()).collect();
    let rx_q_all: Vec<Vec<f32>> = frames.iter().map(|f| f.rx_q.clone()).collect();
    let demap_out = gpu_demap_stage
        .demap_batch(&SymbolBatch::new(rx_i_all, rx_q_all), &gpu_demapper)
        .expect("gpu demap batch");
    // Deinterleave each frame's interleaved LLRs → FECFRAME order.
    let gpu_llr_frames: Vec<Vec<Llr>> = demap_out
        .frames
        .iter()
        .map(|interleaved| interleaver.deinterleave_llrs(interleaved))
        .collect();
    // ONE batched GPU LDPC BP decode over all frames → n-bit codewords + the
    // per-frame BP iteration counts (CPU-aligned convention) for the logged-only
    // GPU mean_iters.
    let (gpu_hard, gpu_iters) = gpu_ldpc_stage
        .decode_batch_with_iters(&LlrBatch::new(gpu_llr_frames), &gpu_ldpc_decoder)
        .expect("gpu ldpc decode batch");
    assert_eq!(gpu_hard.frames.len(), FRAMES_PER_CONFIG);
    assert_eq!(gpu_iters.len(), FRAMES_PER_CONFIG);
    let gpu_iter_sum: u64 = gpu_iters.iter().map(|&i| u64::from(i)).sum();

    // BCH outer decode (CPU) per frame, across rayon. Uses the SAME
    // `BchCode::dvb_t2` SSOT `DvbT2Concat::new` constructs internally (Normal
    // frame, same rate) — the identical public building block, not a
    // reimplementation; BCH has no GPU kernel so it is CPU on both arms.
    let gpu_results: Vec<Verdict> = gpu_hard
        .frames
        .par_iter()
        .zip(frames.par_iter())
        .map(|(gpu_codeword, frame)| {
            let bch_decoder = BchDecoder::new(BchCode::dvb_t2(BchFrameSize::Normal, cfg.rate));
            // Extract systematic BCH codeword (positions 0..k_ldpc), same
            // convention as `DvbT2Concat::decode_soft_counted`.
            let mut bch_codeword = BitVec::with_capacity(k_ldpc);
            for i in 0..k_ldpc {
                bch_codeword.push_bit(gpu_codeword.get(i));
            }
            let bbframe = bch_decoder.decode(&bch_codeword);
            let bit_errors = count_bit_errors(&frame.message, &bbframe) as u64;
            Verdict {
                errored: bit_errors > 0,
                bit_errors,
            }
        })
        .collect();

    // ---- Per-frame FRAME-verdict comparison + aggregation ----
    let mut cpu = Counters {
        iter_sum: Some(0),
        ..Counters::default()
    };
    let mut gpu = Counters {
        // GPU per-frame iters now surfaced via `decode_batch_with_iters`
        // (CPU-aligned convention). LOGGED only — §11 excludes mean_iters from
        // CPU-vs-GPU byte-identity.
        iter_sum: Some(gpu_iter_sum),
        ..Counters::default()
    };

    for (frame_idx, ((cpu_v, cpu_iters), gpu_v)) in
        cpu_results.iter().zip(gpu_results.iter()).enumerate()
    {
        // The §11 contract is on the FRAME verdict. A per-frame verdict mismatch
        // (frame errors on one path but not the other) is a genuine §11
        // violation -> PANIC (never relax, never move the operating point). The
        // bit-error count is NOT compared (excluded BER numerator).
        if cpu_v.errored != gpu_v.errored {
            panic!(
                "BYTE-IDENTITY VIOLATION [{}] frame={frame_idx}: FRAME verdict diverged \
                 (CPU errored={}, GPU errored={}; CPU bit_errors={}, GPU bit_errors={}). \
                 ESCALATE per the §11 HARD trigger — do NOT relax the criterion, do NOT \
                 move the operating point.",
                cfg.label, cpu_v.errored, gpu_v.errored, cpu_v.bit_errors, gpu_v.bit_errors,
            );
        }

        cpu.frames += 1;
        cpu.errored_frames += u64::from(cpu_v.errored);
        cpu.bit_error_sum += cpu_v.bit_errors;
        cpu.iter_sum = cpu.iter_sum.map(|s| s + cpu_iters);

        gpu.frames += 1;
        gpu.errored_frames += u64::from(gpu_v.errored);
        gpu.bit_error_sum += gpu_v.bit_errors;
    }

    (cpu, gpu)
}

/// Asserts the three §11 columns (`frames`, `errors`=errored frames, `fer`)
/// byte-identical CPU-vs-GPU, asserts the sweep is non-vacuous, and logs the
/// excluded quantities (`mean_iters`, bit-error sum). Shared by the three
/// per-config tests.
fn assert_config_byte_identical(cfg: &Config) {
    if device_mem_info().is_err() {
        eprintln!(
            "skipping {} byte-identity: no usable GPU (device_mem_info failed)",
            cfg.label
        );
        return;
    }

    let (cpu, gpu) = run_config(cfg);

    // Non-vacuity: the waterfall sweep MUST exercise the verdict boundary —
    // some frames error, some decode cleanly. (0 errored = above threshold /
    // vacuous; all errored = below the waterfall / also uninformative.)
    assert!(
        cpu.errored_frames > 0 && cpu.errored_frames < cpu.frames,
        "[{}] VACUOUS sweep: errored_frames={} of {} (need 0 < errored < frames; \
         recalibrate the waterfall Es/N0)",
        cfg.label,
        cpu.errored_frames,
        cpu.frames,
    );

    // The three §11 CPU-vs-GPU columns, byte-identical, via the shared SSOT
    // comparator. mean_iters for BOTH paths + the diff is LOGGED there, NOT
    // asserted (design §11 CPU-vs-GPU exclusion); the GPU per-frame iteration
    // counts come from `decode_batch_with_iters` (CPU-aligned convention), so
    // the logged diff is the genuine ±1 near-threshold drift §11 describes.
    assert_three_columns_byte_identical_log_mean_iters(
        &gpu.to_worker_counters(),
        &cpu.to_worker_counters(),
        cfg.label,
    );

    // bit-error sum (BER numerator): LOGGED, NOT asserted (`ber` excluded,
    // `152388f4`). It legitimately differs CPU-vs-GPU on errored frames (demap
    // ULP drift changes garbage bits within a failed frame); only the FRAME
    // verdict is contractual.
    println!(
        "[{}] bit-error sum (LOGGED, NOT asserted — `ber` excluded): CPU {}, GPU {}, \
         diff {}",
        cfg.label,
        cpu.bit_error_sum,
        gpu.bit_error_sum,
        gpu.bit_error_sum as i64 - cpu.bit_error_sum as i64,
    );

    println!(
        "[{}] PASS @ Es/N0={} dB: frames={} errored_frames={} fer={:.6} \
         (CPU == GPU; three columns frames/errors/fer byte-identical; non-vacuous)",
        cfg.label,
        cfg.es_n0_db,
        cpu.frames,
        cpu.errored_frames,
        cpu.fer(),
    );
}

fn config_r12_16qam() -> Config {
    Config {
        // Waterfall midpoint for NMS(0.75) max-log at this seed: 6.4 dB →
        // ≈105/200 errored frames (calibrated empirically; the FER curve is
        // steep — 6.2 → 200/200, 6.6 → 3/200).
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 6.4,
        seed: 0x14F5_9C2D_0012_0010,
        label: "r1/2 16-QAM",
    }
}

fn config_r23_64qam() -> Config {
    Config {
        // Waterfall point for NMS(0.75) max-log at this seed: 14.3 dB →
        // ≈33/200 errored frames (14.0 → 200/200, 14.5 → 0/200).
        rate: CodeRate::Rate2_3,
        modulation: DvbT2Modulation::Qam64,
        es_n0_db: 14.3,
        seed: 0x14F5_9C2D_0023_0040,
        label: "r2/3 64-QAM",
    }
}

fn config_r34_16qam() -> Config {
    Config {
        // Waterfall point for NMS(0.75) max-log at this seed: 10.2 dB →
        // ≈70/200 errored frames (10.1 → 185/200, 10.3 → 3/200).
        rate: CodeRate::Rate3_4,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 10.2,
        seed: 0x14F5_9C2D_0034_0010,
        label: "r3/4 16-QAM",
    }
}

// One #[ignore] test per config so each stays under the 120 s slow-tier cap.

/// Phase B close (issue `14f59c2d`), r1/2 16-QAM: the three §11 columns
/// `frames / errors / fer` are byte-identical CPU-vs-GPU at a waterfall Es/N0
/// with a non-vacuous frame-error mix. `mean_iters` + bit-error sum logged
/// (not asserted); `ber` excluded. Skips cleanly with no GPU.
#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r1/2 16-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r12_16qam() {
    assert_config_byte_identical(&config_r12_16qam());
}

/// Phase B close (issue `14f59c2d`), r2/3 64-QAM: see
/// [`gpu_chain_verdict_byte_identical_r12_16qam`].
#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r2/3 64-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r23_64qam() {
    assert_config_byte_identical(&config_r23_64qam());
}

/// Phase B close (issue `14f59c2d`), r3/4 16-QAM: see
/// [`gpu_chain_verdict_byte_identical_r12_16qam`].
#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r3/4 16-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r34_16qam() {
    assert_config_byte_identical(&config_r34_16qam());
}
