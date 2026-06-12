//! CPU layered-BP convergence measurement for JIT issue 43fb19e2.
//!
//! Ground-truth measurement of iterations-to-BLER<=1e-2 for a row-layered
//! (serial-C) normalized-min-sum schedule vs the PRODUCTION flooding decoder
//! (`Nr5gRateMatchedDecoder`, NMS(0.75)) on the SAME canonical 5G NR mother
//! graph (BG1, i_LS=1, Z=384, r1/2) at the SAME operating point
//! (Es/N0 = -1.4 dB AWGN, per dev/benchmarks/gf2-sim/5g-nr-realtime.md).
//!
//! The layered decoder here is a faithful row-layered NMS implemented on the
//! production mother-code H (`mother_code().parity_check_matrix()`): it reuses
//! the exact same Tanner graph and the exact same NMS(0.75) check rule as the
//! production flooding decoder, differing ONLY in the message schedule (the
//! lever under study). One "layered iteration" = one full sweep over all
//! base-row layers (46 layers of Z=384 rows each), so the iteration unit is
//! directly comparable to one flooding iteration.
//!
//! Channel (verbatim from the receipt): per-bit BPSK-AWGN,
//!   sigma = 1/sqrt(2 * 10^(EsN0_dB/10)),  channel LLR = 2*r/N0,  N0 = 2*sigma^2.
//!
//! Determinism: a fixed-seed ChaCha20 stream (one stream, sequential blocks) so
//! the run is reproducible. We feed the SAME received LLRs to both decoders so
//! the BLER comparison is on identical channel realizations.
//!
//! Run (default 4000 blocks, ~minutes — slow, documented in the design doc):
//!   cargo run --release --bin layered_convergence
//! Env overrides: NR5G_BLOCKS, NR5G_ESN0_DB, NR5G_MAX_ITERS, NR5G_SEED,
//!                NR5G_LAYERS_PER_BASE_ROW (debug), NR5G_SANITY (1 = high-SNR
//!                cross-check layered vs flooding hard outputs, then exit).

use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

const BG: u8 = 1;
const TARGET_N: usize = 16896;
const TARGET_K: usize = 8448;
const Z: usize = 384;
const NMS_ALPHA: f32 = 0.75; // production DEFAULT_NMS_SCALE

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// A precomputed, flattened Tanner graph for the layered decoder.
///
/// Edges are grouped by base-row layer: layer L (0..46) owns check rows
/// [L*Z .. (L+1)*Z). Row-layered BP processes layers in order, and within a
/// layer all Z rows independently (this is exactly the parallelism the QC
/// structure gives — see the design doc's GPU-layering discussion).
struct Graph {
    n: usize,
    m: usize,
    /// For each check row: its variable neighbors (CSR).
    check_vars: Vec<Vec<usize>>,
    /// For each check row: the edge index of each (row, var) pair.
    check_edge_ids: Vec<Vec<usize>>,
    /// Number of edges.
    e: usize,
    /// Base-row layer boundaries: layer L = rows [layer_rows[L] .. layer_rows[L+1]).
    num_layers: usize,
}

impl Graph {
    fn build(h: &SpBitMatrixDual) -> Self {
        let n = h.cols();
        let m = h.rows();
        let mut check_vars = Vec::with_capacity(m);
        let mut check_edge_ids = Vec::with_capacity(m);
        let mut e = 0usize;
        for c in 0..m {
            let vars: Vec<usize> = h.row_iter(c).collect();
            let ids: Vec<usize> = (0..vars.len()).map(|i| e + i).collect();
            e += vars.len();
            check_vars.push(vars);
            check_edge_ids.push(ids);
        }
        let num_layers = m / Z;
        assert_eq!(num_layers * Z, m, "m must be a multiple of Z");
        Self {
            n,
            m,
            check_vars,
            check_edge_ids,
            e,
            num_layers,
        }
    }
}

/// Row-layered (serial-C) normalized-min-sum decoder.
///
/// State: posterior beliefs `belief[v]` (running total APP) and per-edge
/// check-to-variable messages `c2v[e]`. The layered update for each check row:
///
/// 1. for each edge e=(c,v): tmp = belief[v] - c2v[e]   (extrinsic v->c)
/// 2. NMS box-plus over the row's tmp values -> new c2v[e]
/// 3. belief[v] += (new c2v[e] - old c2v[e])             (immediate propagation)
///
/// This immediate write-back of updated beliefs within the same iteration is
/// what makes layered BP converge faster than flooding.
struct LayeredDecoder {
    g: Graph,
    alpha: f32,
}

impl LayeredDecoder {
    fn new(h: &SpBitMatrixDual, alpha: f32) -> Self {
        Self {
            g: Graph::build(h),
            alpha,
        }
    }

    /// Decodes one frame. Returns (hard_bits_full_n, iterations_used, converged).
    /// `converged` = a full-syndrome-pass was reached at or before `max_iters`.
    fn decode(
        &self,
        channel_llrs: &[f32],
        max_iters: usize,
        h: &SpBitMatrixDual,
    ) -> (Vec<u8>, usize, bool) {
        let n = self.g.n;
        let mut belief: Vec<f32> = channel_llrs.to_vec();
        let mut c2v: Vec<f32> = vec![0.0; self.g.e];
        let mut iters = 0usize;
        let mut converged = false;

        for it in 0..max_iters {
            iters = it + 1;
            // One layered sweep over all base-row layers.
            for layer in 0..self.g.num_layers {
                let row0 = layer * Z;
                let row1 = row0 + Z;
                for c in row0..row1 {
                    let vars = &self.g.check_vars[c];
                    let ids = &self.g.check_edge_ids[c];
                    let deg = vars.len();
                    if deg == 0 {
                        continue;
                    }
                    // Extrinsic var->check messages for this row.
                    // NMS: msg_e = alpha * sign_product(others) * min_mag(others).
                    // Compute via running min1/min2 + sign parity, matching the
                    // production min-sum semantics (sign keyed on sign bit).
                    let mut sign_parity = 0u32; // count of negative incoming
                    let mut min1 = f32::INFINITY;
                    let mut min2 = f32::INFINITY;
                    let mut argmin = usize::MAX;
                    // First pass: tmp values and running mins.
                    let mut tmp: [f32; 32] = [0.0; 32]; // max check degree is 19 for BG1
                    for (j, (&v, &eid)) in vars.iter().zip(ids.iter()).enumerate() {
                        let t = belief[v] - c2v[eid];
                        tmp[j] = t;
                        if t.is_sign_negative() {
                            sign_parity ^= 1;
                        }
                        let mag = t.abs();
                        if mag < min1 {
                            min2 = min1;
                            min1 = mag;
                            argmin = j;
                        } else if mag < min2 {
                            min2 = mag;
                        }
                    }
                    // Second pass: write c2v and propagate belief deltas.
                    for j in 0..deg {
                        let v = vars[j];
                        let eid = ids[j];
                        let t = tmp[j];
                        // sign of message = product of OTHER signs = total parity
                        // XOR this edge's sign.
                        let this_neg = (t.is_sign_negative()) as u32;
                        let other_neg_parity = sign_parity ^ this_neg;
                        let mag = if j == argmin { min2 } else { min1 };
                        let mut msg = self.alpha * mag;
                        if other_neg_parity == 1 {
                            msg = -msg;
                        }
                        let old = c2v[eid];
                        c2v[eid] = msg;
                        belief[v] += msg - old; // immediate propagation
                    }
                }
            }
            // Syndrome check on current beliefs (hard decision belief<0 -> 1).
            if self.syndrome_passes(&belief, h) {
                converged = true;
                break;
            }
        }
        let hard: Vec<u8> = belief.iter().map(|&b| (b < 0.0) as u8).collect();
        let _ = n;
        (hard, iters, converged)
    }

    fn syndrome_passes(&self, belief: &[f32], h: &SpBitMatrixDual) -> bool {
        for c in 0..self.g.m {
            let mut p = 0u32;
            for v in h.row_iter(c) {
                p ^= (belief[v] < 0.0) as u32;
            }
            if p & 1 == 1 {
                return false;
            }
        }
        true
    }
}

/// BPSK-AWGN channel matching the receipt: maps a transmitted codeword bit
/// (0/1) to +/-1, adds N(0, sigma^2), returns channel LLR = 2*r/N0.
fn awgn_llrs(codeword: &BitVec, sigma: f32, rng: &mut ChaCha20Rng) -> Vec<f32> {
    let n0 = 2.0 * sigma * sigma;
    let mut out = Vec::with_capacity(codeword.len());
    for i in 0..codeword.len() {
        let bit = codeword.get(i);
        let s = if bit { -1.0f32 } else { 1.0f32 }; // 0 -> +1, 1 -> -1
                                                    // Box-Muller for one Gaussian sample.
        let u1: f32 = rng.gen_range(1e-12f32..1.0);
        let u2: f32 = rng.gen_range(0.0f32..1.0);
        let noise = (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos();
        let r = s + sigma * noise;
        out.push(2.0 * r / n0);
    }
    out
}

fn main() {
    let blocks = env_usize("NR5G_BLOCKS", 4000);
    let esn0_db = env_f64("NR5G_ESN0_DB", -1.4);
    let max_iters = env_usize("NR5G_MAX_ITERS", 40);
    let seed = env_usize("NR5G_SEED", 0xC0FFEE) as u64;
    let sanity = env_usize("NR5G_SANITY", 0) == 1;

    let sigma = (1.0 / (2.0 * 10f64.powf(esn0_db / 10.0))).sqrt() as f32;

    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(BG, TARGET_N, TARGET_K);
    let mother = rm.mother_code();
    let h = mother.parity_check_matrix().clone();
    let layered = LayeredDecoder::new(&h, NMS_ALPHA);

    println!("# Layered vs flooding convergence — BG{BG} i_LS=1 Z={Z} r1/2");
    println!(
        "blocks={blocks} esn0_db={esn0_db} sigma={sigma:.5} max_iters={max_iters} seed={seed:#x}"
    );
    println!(
        "mother: n={} m={} E(nnz)={} layers={}",
        layered.g.n, layered.g.m, layered.g.e, layered.g.num_layers
    );
    println!();

    let mut rng = ChaCha20Rng::seed_from_u64(seed);

    if sanity {
        // High-SNR cross-check: at a clean channel both schedules must recover
        // the transmitted message bit-for-bit. Confirms the layered decoder is
        // wired to the same graph/semantics as production before we trust its
        // convergence numbers.
        let sane_sigma = (1.0 / (2.0 * 10f64.powf(2.0 / 10.0))).sqrt() as f32; // Es/N0=+2 dB
        let mut ok = 0usize;
        let n_sanity = 50usize;
        let mut flood = gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder::with_algorithm(
            rm.clone(),
            DecoderAlgorithm::NormalizedMinSum(NMS_ALPHA),
        );
        for _ in 0..n_sanity {
            let msg = random_msg(&mut rng);
            let cw = rm.encode(&msg);
            let chan = awgn_llrs(&cw, sane_sigma, &mut rng);
            let full = rm.prepare_llrs(&to_llr(&chan));
            let (hard, _it, conv) = layered.decode(&to_f32_full(&full), max_iters, &h);
            // Production flooding message bits.
            let fres = flood.decode_iterative(&to_llr(&chan), max_iters);
            // Compare first target_k bits of layered hard output to message.
            let layered_ok = conv && !(0..TARGET_K).any(|i| (hard[i] == 1) != msg.get(i));
            let flood_ok =
                fres.converged && !(0..TARGET_K).any(|i| fres.decoded_bits.get(i) != msg.get(i));
            if layered_ok && flood_ok {
                ok += 1;
            }
        }
        println!(
            "SANITY (Es/N0=+2dB, {n_sanity} blocks): both recovered message in {ok}/{n_sanity}"
        );
        return;
    }

    // ---- Main sweep: measure BLER + mean iterations for both schedules ----
    let mut flood = gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder::with_algorithm(
        rm.clone(),
        DecoderAlgorithm::NormalizedMinSum(NMS_ALPHA),
    );

    let mut lay_block_errs = 0usize;
    let mut flood_block_errs = 0usize;
    let mut lay_iter_sum = 0u64;
    let mut flood_iter_sum = 0u64;
    let mut lay_iter_sum_ok = 0u64; // iters on CONVERGED blocks only
    let mut lay_ok_count = 0u64;
    let mut flood_iter_sum_ok = 0u64;
    let mut flood_ok_count = 0u64;

    for _ in 0..blocks {
        let msg = random_msg(&mut rng);
        let cw = rm.encode(&msg);
        let chan = awgn_llrs(&cw, sigma, &mut rng);
        let chan_llr = to_llr(&chan);

        // Layered on the full mother LLRs.
        let full = rm.prepare_llrs(&chan_llr);
        let (hard, lit, lconv) = layered.decode(&to_f32_full(&full), max_iters, &h);
        lay_iter_sum += lit as u64;
        let lay_err = !lconv || (0..TARGET_K).any(|i| (hard[i] == 1) != msg.get(i));
        if lay_err {
            lay_block_errs += 1;
        } else {
            lay_iter_sum_ok += lit as u64;
            lay_ok_count += 1;
        }

        // Production flooding.
        let fres = flood.decode_iterative(&chan_llr, max_iters);
        flood_iter_sum += fres.iterations as u64;
        let flood_err =
            !fres.converged || (0..TARGET_K).any(|i| fres.decoded_bits.get(i) != msg.get(i));
        if flood_err {
            flood_block_errs += 1;
        } else {
            flood_iter_sum_ok += fres.iterations as u64;
            flood_ok_count += 1;
        }
    }

    let lay_bler = lay_block_errs as f64 / blocks as f64;
    let flood_bler = flood_block_errs as f64 / blocks as f64;
    println!("## Results ({blocks} blocks, max_iters={max_iters})");
    println!("layered  : BLER={lay_bler:.5} ({lay_block_errs} errs)  mean_iters(all)={:.3}  mean_iters(converged)={:.3}",
        lay_iter_sum as f64 / blocks as f64,
        if lay_ok_count > 0 { lay_iter_sum_ok as f64 / lay_ok_count as f64 } else { f64::NAN });
    println!("flooding : BLER={flood_bler:.5} ({flood_block_errs} errs)  mean_iters(all)={:.3}  mean_iters(converged)={:.3}",
        flood_iter_sum as f64 / blocks as f64,
        if flood_ok_count > 0 { flood_iter_sum_ok as f64 / flood_ok_count as f64 } else { f64::NAN });
    println!();
    if lay_ok_count > 0 && flood_ok_count > 0 {
        let ratio = (flood_iter_sum_ok as f64 / flood_ok_count as f64)
            / (lay_iter_sum_ok as f64 / lay_ok_count as f64);
        println!("iteration-reduction factor (flooding/layered, converged-only) = {ratio:.3}x");
    }
}

fn random_msg(rng: &mut ChaCha20Rng) -> BitVec {
    let mut m = BitVec::with_capacity(TARGET_K);
    for _ in 0..TARGET_K {
        m.push_bit(rng.gen::<bool>());
    }
    m
}

fn to_llr(v: &[f32]) -> Vec<Llr> {
    v.iter().map(|&x| Llr::new(x)).collect()
}

fn to_f32_full(v: &[Llr]) -> Vec<f32> {
    v.iter().map(|x| x.value()).collect()
}
