//! CPU-vs-GPU identity of the DVB-T2 BICM chain's frame verdict: at a fixed
//! seed and a waterfall Es/N0, `frames`, `errors` (errored frames) and `fer`
//! are byte-identical between a CPU-only path and a path running the max-log
//! demap and LDPC BP on the GPU, over one shared noise realisation. The
//! bit-error sum and `mean_iters` are logged only. Skips when no GPU is usable.

#![cfg(feature = "hip")]

use gf2_coding::bch::dvb_t2::FrameSize as BchFrameSize;
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchDecoder};
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

const MAX_LDPC_ITERATIONS: usize = 50;
const FRAMES_PER_CONFIG: usize = 200;

/// `es_n0_db` sits in the waterfall, so the sweep mixes clean and errored frames.
struct Config {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    seed: u64,
    label: &'static str,
}

#[derive(Debug, Default, Clone, Copy)]
struct Counters {
    frames: u64,
    /// Frames whose decoded BBFRAME differs from the transmitted message.
    errored_frames: u64,
    /// Mismatched info bits across frames; logged only.
    bit_error_sum: u64,
    /// BP iterations across frames; logged only.
    iter_sum: Option<u64>,
}

impl Counters {
    fn fer(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.errored_frames as f64 / self.frames as f64
        }
    }

    /// `total_bits` is untracked here and maps to 0; the comparator does not read it.
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

/// SplitMix64 stream supplying the message bits and noise samples both paths share.
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

/// Returns `(sigma, N0)` with per-axis `sigma^2 = 1 / (2 * 10^(Es/N0 / 10))` and
/// `N0 = 2 sigma^2`.
fn awgn_params(es_n0_db: f64) -> (f32, f32) {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    let sigma = (sigma_sq as f32).sqrt();
    let noise_var = (2.0 * sigma_sq) as f32;
    (sigma, noise_var)
}

struct SharedFrame {
    message: BitVec,
    rx_i: Vec<f32>,
    rx_q: Vec<f32>,
}

fn make_shared_frame(
    codec: &DvbT2Concat,
    interleaver: &DvbT2BitInterleaver,
    mapper: &dyn gf2_coding::modem::BatchMapper<f32>,
    bits_per_symbol: usize,
    sigma: f32,
    k_bch: usize,
    rng: &mut SplitMix64,
) -> SharedFrame {
    let message = random_bitvec(k_bch, rng);
    let codeword = codec.encode(&message);
    let n_ldpc = codeword.len();
    let num_symbols = n_ldpc / bits_per_symbol;
    let interleaved = interleaver.interleave(&codeword);
    let interleaved_bits: Vec<bool> = (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
    let mut rx_i = vec![0.0_f32; num_symbols];
    let mut rx_q = vec![0.0_f32; num_symbols];
    mapper.map_bits(&interleaved_bits, &mut rx_i, &mut rx_q);
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

#[derive(Clone, Copy)]
struct Verdict {
    errored: bool,
    bit_errors: u64,
}

/// Returns `(cpu, gpu)` counters; panics on the first per-frame verdict mismatch.
fn run_config(cfg: &Config) -> (Counters, Counters) {
    let (sigma, noise_var) = awgn_params(cfg.es_n0_db);

    let mut codec = DvbT2Concat::new(FrameSize::Normal, cfg.rate)
        .expect("DVB-T2 Normal codec for in-scope rate");
    let decoder_config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    codec.set_decoder_config(decoder_config);
    let k_bch = codec.k_bch();
    let k_ldpc = codec.k_ldpc();

    let bits_per_symbol = cfg.modulation.bits_per_cell();
    let modcod = DvbT2Modcod::new(FrameSize::Normal, cfg.rate, cfg.modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);

    let order = 1usize << bits_per_symbol;
    let spec = ModemSpec::<f32>::gray_square_qam(order);
    let mapper = spec.preferred_mapper();

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

    let rx_i_all: Vec<Vec<f32>> = frames.iter().map(|f| f.rx_i.clone()).collect();
    let rx_q_all: Vec<Vec<f32>> = frames.iter().map(|f| f.rx_q.clone()).collect();
    let demap_out = gpu_demap_stage
        .demap_batch(&SymbolBatch::new(rx_i_all, rx_q_all), &gpu_demapper)
        .expect("gpu demap batch");
    let gpu_llr_frames: Vec<Vec<Llr>> = demap_out
        .frames
        .iter()
        .map(|interleaved| interleaver.deinterleave_llrs(interleaved))
        .collect();
    let (gpu_hard, gpu_iters) = gpu_ldpc_stage
        .decode_batch_with_iters(&LlrBatch::new(gpu_llr_frames), &gpu_ldpc_decoder)
        .expect("gpu ldpc decode batch");
    assert_eq!(gpu_hard.frames.len(), FRAMES_PER_CONFIG);
    assert_eq!(gpu_iters.len(), FRAMES_PER_CONFIG);
    let gpu_iter_sum: u64 = gpu_iters.iter().map(|&i| u64::from(i)).sum();

    let bch_code = dvb_t2_bch_code(BchFrameSize::Normal, cfg.rate)
        .expect("the DVB-T2 Normal outer code for this rate");
    let bch_decoder = DvbT2BchDecoder::new(&bch_code);
    let gpu_results: Vec<Verdict> = gpu_hard
        .frames
        .par_iter()
        .zip(frames.par_iter())
        .map(|(gpu_codeword, frame)| {
            let mut bch_codeword = BitVec::with_capacity(k_ldpc);
            for i in 0..k_ldpc {
                bch_codeword.push_bit(gpu_codeword.get(i));
            }
            let (_outcome, bbframe) = bch_decoder
                .decode(&bch_codeword)
                .expect("a BCH codeword of the extracted length decodes");
            let bit_errors = count_bit_errors(&frame.message, &bbframe) as u64;
            Verdict {
                errored: bit_errors > 0,
                bit_errors,
            }
        })
        .collect();

    let mut cpu = Counters {
        iter_sum: Some(0),
        ..Counters::default()
    };
    let mut gpu = Counters {
        iter_sum: Some(gpu_iter_sum),
        ..Counters::default()
    };

    for (frame_idx, ((cpu_v, cpu_iters), gpu_v)) in
        cpu_results.iter().zip(gpu_results.iter()).enumerate()
    {
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

fn assert_config_byte_identical(cfg: &Config) {
    if device_mem_info().is_err() {
        eprintln!(
            "skipping {} byte-identity: no usable GPU (device_mem_info failed)",
            cfg.label
        );
        return;
    }

    let (cpu, gpu) = run_config(cfg);

    assert!(
        cpu.errored_frames > 0 && cpu.errored_frames < cpu.frames,
        "[{}] VACUOUS sweep: errored_frames={} of {} (need 0 < errored < frames; \
         recalibrate the waterfall Es/N0)",
        cfg.label,
        cpu.errored_frames,
        cpu.frames,
    );

    assert_three_columns_byte_identical_log_mean_iters(
        &gpu.to_worker_counters(),
        &cpu.to_worker_counters(),
        cfg.label,
    );

    // Logged only: on an errored frame the two paths may differ in which bits fail.
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
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 6.4,
        seed: 0x14F5_9C2D_0012_0010,
        label: "r1/2 16-QAM",
    }
}

fn config_r23_64qam() -> Config {
    Config {
        rate: CodeRate::Rate2_3,
        modulation: DvbT2Modulation::Qam64,
        es_n0_db: 14.3,
        seed: 0x14F5_9C2D_0023_0040,
        label: "r2/3 64-QAM",
    }
}

fn config_r34_16qam() -> Config {
    Config {
        rate: CodeRate::Rate3_4,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 10.2,
        seed: 0x14F5_9C2D_0034_0010,
        label: "r3/4 16-QAM",
    }
}

#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r1/2 16-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r12_16qam() {
    assert_config_byte_identical(&config_r12_16qam());
}

#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r2/3 64-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r23_64qam() {
    assert_config_byte_identical(&config_r23_64qam());
}

#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU DVB-T2 BICM chain byte-identity, r3/4 16-QAM waterfall (gfx1030-gated)"]
fn gpu_chain_verdict_byte_identical_r34_16qam() {
    assert_config_byte_identical(&config_r34_16qam());
}
