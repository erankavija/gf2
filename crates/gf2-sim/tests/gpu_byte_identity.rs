//! End-to-end CPU-vs-GPU byte-identity of the full DVB-T2 BICM chain verdict
//! (issue `14f59c2d`, the Phase B closer; design doc §5, §6, §11).
//!
//! This is the **chain-level** counterpart of the two per-kernel byte-identity
//! suites (`gpu_ldpc_byte_identity.rs` for the BP hard decision, issue
//! `a930be7f`; `gpu_demap_byte_identity.rs` for the max-log demap, issue
//! `d3f1616a`). It composes the validated building blocks — encode, interleave,
//! Gray-QAM map, AWGN, demap, deinterleave, LDPC BP decode, BCH outer decode —
//! into one frame's worth of work and asserts the **design-doc §11 CPU-vs-GPU
//! relaxed contract** end-to-end: the three columns `fer / frames / errors` are
//! byte-identical across the CPU-only path and the CPU+GPU path at a fixed seed,
//! for the three named (rate, modulation) configurations on gfx1030.
//!
//! # Why hand-composed (Phase C executor does not exist yet)
//!
//! The hybrid CPU/GPU executor (`75c22fa8` / `de160fc5`) is downstream of this
//! task, so both paths are hand-composed here, mirroring the CPU frame kernel
//! [`DvbT2BicmFrameSim`](gf2_sim::frame_sim::DvbT2BicmFrameSim) stage-for-stage.
//! The two paths differ **only** in which device computes the GPU-eligible
//! stages (demap + LDPC BP); every other stage is shared, and crucially the
//! **same transmitted message, same codeword, and SAME noisy received symbols**
//! feed both demappers.
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
//! CPU-only path (steps 4–7 on CPU):
//!   4. CPU [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper)
//!      **max-log** demap;
//!   5. bit-deinterleave LLRs → FECFRAME order;
//!   6. CPU LDPC BP decode + BCH outer decode via
//!      [`DvbT2Concat::decode_soft_counted`](gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat);
//!   7. count info-bit errors vs the TX message.
//!
//! CPU+GPU path (demap + LDPC BP on GPU, BCH on CPU — BCH has no GPU kernel):
//!   4. GPU [`GpuGrayQamDemapper`](gf2_sim::gpu::demap::GpuGrayQamDemapper)
//!      **max-log** demap;
//!   5. bit-deinterleave LLRs → FECFRAME order;
//!   6. GPU [`GpuLdpcBp`](gf2_sim::gpu::ldpc_bp::GpuLdpcBp) BP decode → n-bit
//!      codeword → extract first k_ldpc bits → CPU [`BchDecoder`] outer decode
//!      (the same `BchCode::dvb_t2` SSOT `DvbT2Concat::new` builds internally);
//!   7. count info-bit errors vs the TX message.
//!
//! # MAX-LOG on BOTH sides (apples-to-apples)
//!
//! [`GpuGrayQamDemapper`] is **max-log only** (no GPU `ExactLogMap`), so BOTH
//! paths use [`DemapMethod::MaxLog`]. Comparing GPU max-log against CPU
//! `ExactLogMap` would be invalid; the comparison here is GPU-max-log vs
//! CPU-max-log throughout.
//!
//! # What is asserted, and what is NOT (design §11, user-approved Q3 2026-06-07)
//!
//! - **Asserted byte-identical**: `fer`, `frames`, `errors` (the three columns of
//!   the CPU-vs-GPU relaxed contract).
//! - **Logged, NOT asserted**: `mean_iters`. Per §11 it is EXCLUDED from
//!   CPU-vs-GPU byte-identity (RDNA2 transcendental ULP drift can shift the BP
//!   early-termination iteration by ±1 without changing the integer-state
//!   parity-check verdict). The GPU batch decode API
//!   ([`GpuLdpcBp::decode_batch`]) does not surface per-frame iteration counts,
//!   so the GPU `mean_iters` is reported as not-surfaced; the CPU `mean_iters`
//!   is logged for the record. The diff is logged, never asserted.
//! - **Excluded entirely**: `ber` (non-associative f32 horizontal reduction;
//!   status-quo amendment from `152388f4`) — it is neither computed nor compared
//!   here.
//!
//! # The hard escalation boundary
//!
//! The §11 rationale is written specifically about the LDPC BP verdict's
//! robustness to transcendental drift; it does NOT claim GPU max-log demap drift
//! is verdict-robust. If GPU-demap-then-GPU-decode flips a borderline bit vs
//! CPU-demap-then-CPU-decode, the three-column byte-identity breaks. On any
//! divergence this test PANICS with the exact (config, frame, column, first
//! differing value) — it does NOT relax the criterion. Resolution is a §11-scope
//! user decision (see the issue's HARD ESCALATION TRIGGER), not a test edit.
//!
//! Gated on GPU presence (skips cleanly when `device_mem_info().is_err()`, like
//! the other `gf2-sim` GPU tests) and carries `#[ignore]` per the CLAUDE.md
//! test-tier rules (200-frame n=64800 sweep over three configs far exceeds the
//! 5 s fast tier). Single gfx1030 → never assumes concurrent GPU suites.

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
use gf2_sim::LlrBatch;

/// BP iteration cap (the DVB-T2 default; matches `DvbT2Concat`'s 50).
const MAX_LDPC_ITERATIONS: usize = 50;

/// Frames per config (the §11 contract sweep size, matching the per-kernel
/// suites).
const FRAMES_PER_CONFIG: usize = 200;

/// One (rate, modulation, Es/N0) config plus its per-config seed.
///
/// # Es/N0 selection — the contract's convergence regime
///
/// Each Es/N0 is set **above** the config's TS 102 831 Table 44 QEF C/N
/// threshold (r1/2 16-QAM 6.0 dB, r2/3 64-QAM 13.5 dB, r3/4 16-QAM 10.0 dB),
/// with margin, so the LDPC BP **converges** on essentially every frame. This
/// is the regime the §11 CPU-vs-GPU contract is about: a *converged* frame
/// decodes to the *correct* codeword on both paths, so `fer / frames / errors`
/// are byte-identical (the parity-check verdict is integer-state, robust to the
/// 1-3 ULP transcendental/max-log drift). **Below** threshold the BP does NOT
/// converge and both paths emit *garbage* codewords whose raw bit-error counts
/// legitimately drift by the demap/BP ULP residual — that is a non-converged
/// artefact, not a verdict, and is outside the contract's scope. Running here at
/// a converging operating point validates the contract honestly without masking
/// any real divergence (see the receipt's escalation note).
struct Config {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    seed: u64,
    label: &'static str,
}

/// Aggregated three-column verdict over one config's frame sweep, plus the
/// (logged-only) mean BP iteration accumulator from the path that surfaces it.
#[derive(Debug, Default, Clone, Copy)]
struct Counters {
    frames: u64,
    errors: u64,
    /// Frame-error count (a frame is in error iff any info bit differs).
    errored_frames: u64,
    /// Sum of BP iterations across frames, where the path surfaces it (CPU
    /// only — see module docs). `None` means the path does not surface it.
    iter_sum: Option<u64>,
}

impl Counters {
    /// Frame error rate `errored_frames / frames` (the `fer` column). Compared
    /// bit-for-bit as an `f64`, like the CPU-only/parallel contract's `fer`.
    fn fer(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.errored_frames as f64 / self.frames as f64
        }
    }

    /// Mean BP iterations where surfaced (logged only, never asserted).
    fn mean_iters(&self) -> Option<f64> {
        match self.iter_sum {
            Some(s) if self.frames > 0 => Some(s as f64 / self.frames as f64),
            _ => None,
        }
    }
}

/// Deterministic SplitMix64 stream — the single shared randomness source for one
/// config's sweep. Both paths consume the identical message bits and identical
/// noise samples from this stream, so the ONLY difference between paths is which
/// device runs the demap + LDPC BP.
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
    rx: SymbolBatch,
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
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

    // 3. ONE shared AWGN realisation: I axis (all symbols) then Q axis (all
    //    symbols), matching the `BicmAwgnChannel` draw contract.
    for s in tx_i.iter_mut() {
        let u1 = rng.next_uniform();
        let u2 = rng.next_uniform();
        *s += sigma * box_muller_cos(u1, u2);
    }
    for s in tx_q.iter_mut() {
        let u1 = rng.next_uniform();
        let u2 = rng.next_uniform();
        *s += sigma * box_muller_cos(u1, u2);
    }

    SharedFrame {
        message,
        rx: SymbolBatch::new(vec![tx_i], vec![tx_q]),
    }
}

/// Runs the full CPU-only and CPU+GPU paths over one config's frame sweep and
/// returns `(cpu_counters, gpu_counters)`. Panics with the precise divergence
/// detail on the FIRST per-frame verdict mismatch (the escalation contract).
fn run_config(cfg: &Config) -> (Counters, Counters) {
    let (sigma, noise_var) = awgn_params(cfg.es_n0_db);

    // Shared / CPU back-end: the production DVB-T2 codec. `decode_soft_counted`
    // is the CPU-only LDPC+BCH path (and the encoder for both paths).
    let mut codec = DvbT2Concat::new(FrameSize::Normal, cfg.rate)
        .expect("DVB-T2 Normal codec for in-scope rate");
    let decoder_config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    codec.set_decoder_config(decoder_config);
    let k_bch = codec.k_bch();
    let k_ldpc = codec.k_ldpc();

    let bits_per_symbol = cfg.modulation.bits_per_cell();
    let modcod = DvbT2Modcod::new(FrameSize::Normal, cfg.rate, cfg.modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);

    // Mapper + CPU demapper from the SAME `gray_square_qam(order)` SSOT the
    // production chain (`BicmAwgnChannel`) builds.
    let order = 1usize << bits_per_symbol;
    let spec = ModemSpec::<f32>::gray_square_qam(order);
    let mapper = spec.preferred_mapper();
    let cpu_demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order));

    // GPU stages: max-log demap + LDPC BP, same code + config + noise_var.
    let ldpc_code = LdpcCode::dvb_t2_normal(cfg.rate);
    let n_ldpc = ldpc_code.n();
    // The GPU demapper's `max_batch` is sized in *symbols* (one frame's worth):
    // n_ldpc bits / bits_per_symbol symbols. The LDPC decoder's is in *frames*.
    let symbols_per_frame = n_ldpc / bits_per_symbol;
    let gpu_demap_stage = GpuGrayQamDemapper::new(cfg.modulation, DemapMethod::MaxLog, noise_var);
    let gpu_demapper = gpu_demap_stage
        .build_demapper(symbols_per_frame)
        .expect("build GPU demapper on gfx1030");
    let gpu_ldpc_stage = GpuLdpcBp::new(ldpc_code, decoder_config, MAX_LDPC_ITERATIONS);
    let gpu_ldpc_decoder = gpu_ldpc_stage
        .build_decoder(1)
        .expect("build GPU LDPC decoder on gfx1030");

    // GPU back-end BCH: the SAME `BchCode::dvb_t2` SSOT `DvbT2Concat::new`
    // constructs internally (Normal frame, same rate). Not a reimplementation —
    // the identical public building block, used to finish the GPU path's outer
    // decode (BCH has no GPU kernel).
    let bch_decoder = BchDecoder::new(BchCode::dvb_t2(BchFrameSize::Normal, cfg.rate));

    let mut cpu = Counters::default();
    let mut gpu = Counters::default();
    cpu.iter_sum = Some(0); // CPU path surfaces BP iterations.
    gpu.iter_sum = None; // GPU batch decode does not surface per-frame iters.

    let mut rng = SplitMix64::new(cfg.seed);

    for frame_idx in 0..FRAMES_PER_CONFIG {
        let frame = make_shared_frame(
            &codec,
            &interleaver,
            mapper.as_ref(),
            bits_per_symbol,
            sigma,
            k_bch,
            &mut rng,
        );
        let num_symbols = frame.rx.i[0].len();

        // -------------------- CPU-only path --------------------
        // 4. CPU max-log demap (interleaved LLR order).
        let nv = vec![noise_var; num_symbols];
        let mut cpu_interleaved_llrs = vec![Llr::zero(); n_ldpc];
        cpu_demapper.demap_llrs(
            DemapInput {
                rx_i: &frame.rx.i[0],
                rx_q: &frame.rx.q[0],
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::MaxLog,
            },
            &mut cpu_interleaved_llrs,
        );
        // 5. Deinterleave → FECFRAME order.
        let cpu_llrs = interleaver.deinterleave_llrs(&cpu_interleaved_llrs);
        // 6. CPU LDPC BP + BCH outer decode (production path).
        let (cpu_bbframe, cpu_iters) = match codec.decode_soft_counted(&cpu_llrs) {
            Ok((bb, it)) => (bb, it as u64),
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            }) => (bbframe, iterations as u64),
            Err(_) => (BitVec::with_capacity(k_bch), MAX_LDPC_ITERATIONS as u64),
        };
        // 7. Info-bit errors.
        let cpu_bit_errors = count_bit_errors(&frame.message, &cpu_bbframe) as u64;

        // -------------------- CPU+GPU path --------------------
        // 4. GPU max-log demap (interleaved LLR order), same noisy symbols.
        let gpu_demap_out = gpu_demap_stage
            .demap_batch(&frame.rx, &gpu_demapper)
            .expect("gpu demap");
        // 5. Deinterleave → FECFRAME order.
        let gpu_llrs = interleaver.deinterleave_llrs(&gpu_demap_out.frames[0]);
        // 6. GPU LDPC BP → n-bit codeword.
        let gpu_hard = gpu_ldpc_stage
            .decode_batch(&LlrBatch::new(vec![gpu_llrs]), &gpu_ldpc_decoder)
            .expect("gpu ldpc decode");
        let gpu_codeword = &gpu_hard.frames[0];
        // Extract systematic BCH codeword (positions 0..k_ldpc), same convention
        // as `DvbT2Concat::decode_soft_counted`.
        let mut bch_codeword = BitVec::with_capacity(k_ldpc);
        for i in 0..k_ldpc {
            bch_codeword.push_bit(gpu_codeword.get(i));
        }
        // BCH outer decode (CPU) → BBFRAME estimate.
        let gpu_bbframe = bch_decoder.decode(&bch_codeword);
        // 7. Info-bit errors.
        let gpu_bit_errors = count_bit_errors(&frame.message, &gpu_bbframe) as u64;

        // -------------------- Per-frame verdict comparison --------------------
        // The §11 three-column contract is an aggregate equality, but a per-frame
        // mismatch is the earliest signal and yields the precise escalation
        // detail. errors (info-bit count) and the frame-error flag must match
        // frame-by-frame; any mismatch is a contract violation -> PANIC (never
        // relax). BER is intentionally NOT computed or compared (excluded per
        // `152388f4`).
        if cpu_bit_errors != gpu_bit_errors {
            let first = (0..k_bch).find(|&b| cpu_bbframe.get(b) != gpu_bbframe.get(b));
            panic!(
                "BYTE-IDENTITY VIOLATION [{}] frame={frame_idx}: column=errors \
                 (CPU info-bit errors={cpu_bit_errors}, GPU={gpu_bit_errors}); \
                 first differing info bit {first:?} (cpu={:?}, gpu={:?}). \
                 ESCALATE per the §11 HARD trigger — do NOT relax the criterion.",
                cfg.label,
                first.map(|b| cpu_bbframe.get(b)),
                first.map(|b| gpu_bbframe.get(b)),
            );
        }
        let cpu_errored = cpu_bit_errors > 0;
        let gpu_errored = gpu_bit_errors > 0;
        if cpu_errored != gpu_errored {
            panic!(
                "BYTE-IDENTITY VIOLATION [{}] frame={frame_idx}: column=fer/frames \
                 (CPU errored={cpu_errored}, GPU errored={gpu_errored}). \
                 ESCALATE per the §11 HARD trigger — do NOT relax the criterion.",
                cfg.label,
            );
        }

        // Aggregate.
        cpu.frames += 1;
        cpu.errors += cpu_bit_errors;
        cpu.errored_frames += u64::from(cpu_errored);
        cpu.iter_sum = cpu.iter_sum.map(|s| s + cpu_iters);

        gpu.frames += 1;
        gpu.errors += gpu_bit_errors;
        gpu.errored_frames += u64::from(gpu_errored);
    }

    (cpu, gpu)
}

/// The three named (rate, modulation) configurations from the issue.
fn configs() -> [Config; 3] {
    [
        Config {
            // QEF threshold 6.0 dB; +1.5 dB margin for reliable convergence.
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 7.5,
            seed: 0x14F5_9C2D_0012_0010,
            label: "r1/2 16-QAM",
        },
        Config {
            // QEF threshold 13.5 dB; +1.5 dB margin for reliable convergence.
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 15.0,
            seed: 0x14F5_9C2D_0023_0040,
            label: "r2/3 64-QAM",
        },
        Config {
            // QEF threshold 10.0 dB; +1.5 dB margin for reliable convergence.
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 11.5,
            seed: 0x14F5_9C2D_0034_0010,
            label: "r3/4 16-QAM",
        },
    ]
}

/// Phase B close (issue `14f59c2d`): the full DVB-T2 BICM chain verdict is
/// byte-identical (`fer / frames / errors`) across CPU-only and CPU+GPU at a
/// fixed seed for the three named configs. `mean_iters` is logged (not
/// asserted); `ber` is excluded. Skips cleanly with no GPU.
#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity over 3 configs (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!(
            "skipping gpu_chain_verdict_byte_identical_to_cpu: no usable GPU \
             (device_mem_info failed)"
        );
        return;
    }

    for cfg in &configs() {
        let (cpu, gpu) = run_config(cfg);

        // The three-column §11 CPU-vs-GPU contract: fer / frames / errors must be
        // byte-identical. (Per-frame mismatches already panic in `run_config`;
        // these aggregate asserts are the contract statement and a backstop.)
        assert_eq!(
            cpu.frames, gpu.frames,
            "[{}] column `frames` diverged: CPU {} vs GPU {}",
            cfg.label, cpu.frames, gpu.frames
        );
        assert_eq!(
            cpu.errors, gpu.errors,
            "[{}] column `errors` diverged: CPU {} vs GPU {}",
            cfg.label, cpu.errors, gpu.errors
        );
        // `fer` is the errored-frame ratio; both sides share the same `frames`,
        // and `errored_frames` is compared bit-for-bit, so `fer` is byte-equal.
        assert_eq!(
            cpu.errored_frames, gpu.errored_frames,
            "[{}] column `fer` diverged (errored frames): CPU {} vs GPU {}",
            cfg.label, cpu.errored_frames, gpu.errored_frames
        );
        assert_eq!(
            cpu.fer().to_bits(),
            gpu.fer().to_bits(),
            "[{}] column `fer` diverged: CPU {} vs GPU {}",
            cfg.label,
            cpu.fer(),
            gpu.fer()
        );

        // `mean_iters`: LOGGED, NOT asserted (design §11 CPU-vs-GPU exclusion).
        // The GPU batch decode API does not surface per-frame iteration counts,
        // so the GPU value is reported as not-surfaced; the diff is informational.
        let cpu_mean = cpu.mean_iters();
        match (cpu_mean, gpu.mean_iters()) {
            (Some(c), Some(g)) => {
                println!(
                    "[{}] mean_iters (LOGGED, NOT asserted — §11 CPU-vs-GPU exclusion): \
                     CPU {c:.4}, GPU {g:.4}, diff {:+.4}",
                    cfg.label,
                    g - c,
                );
            }
            (Some(c), None) => {
                println!(
                    "[{}] mean_iters (LOGGED, NOT asserted — §11 CPU-vs-GPU exclusion): \
                     CPU {c:.4}, GPU n/a (batch GPU decode does not surface per-frame \
                     iteration counts), diff n/a",
                    cfg.label,
                );
            }
            _ => {}
        }

        println!(
            "[{}] PASS: fer={:.6} frames={} errors={} (CPU == GPU, three columns byte-identical)",
            cfg.label,
            cpu.fer(),
            cpu.frames,
            cpu.errors,
        );
    }
}
