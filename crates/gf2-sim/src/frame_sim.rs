//! Deterministic single-frame DVB-T2 BICM-AWGN simulation kernel.
//!
//! This is the reusable per-frame simulation kernel that the within-SNR
//! parallel dispatch ([`run_snr_point`](crate::parallel::run_snr_point))
//! invokes. It composes the **existing, validated** `gf2-coding` BICM building
//! blocks (the [`stages`](crate::stages) wrappers over [`DvbT2Concat`],
//! [`DvbT2BitInterleaver`], `GrayQamMapper`, `FastGrayQamDemapper`) into one
//! frame's worth of work, with the AWGN noise injected by an inline Box-Muller
//! draw that mirrors the legacy baseline harness
//! (`gf2_coding::dvb_t2_bicm_harness::BicmAwgnChannel`). It therefore runs the
//! same per-frame compute as the single-thread baseline measured at 1.6216 fps
//! (`dev/benchmarks/gf2-sim/baseline-single-thread.md`), so the parallel
//! throughput number is comparable to that baseline.
//!
//! # Why it lives here, and why it draws noise inline
//!
//! The Phase A channel stages (`db9836e4`) are not yet on `main`, so the
//! parallel-dispatch task supplies its own frame kernel. The kernel reuses
//! `gf2-coding`'s codec / interleaver / modem math verbatim via the
//! [`stages`](crate::stages) wrappers (no FEC or modem math reimplemented); only
//! the AWGN Box-Muller draw is inline. That draw must come from the per-worker
//! [`ChaCha20Rng`](rand_chacha::ChaCha20Rng) (`rand_chacha 0.9`, the design-doc
//! §5 pin) so the determinism contract is self-contained in `gf2-sim`'s seek
//! scheme — `gf2-coding`'s channel takes a `rand 0.8` RNG, a different version,
//! so calling it directly would couple the contract to a second RNG stream.
//!
//! # Determinism
//!
//! [`DvbT2BicmFrameSim::simulate_frame`] draws **all** randomness (the random
//! transmitted BBFRAME and the AWGN noise) from the supplied
//! [`WorkerCtx`](crate::parallel::WorkerCtx)'s RNG, which the dispatcher has
//! already reseeked to the frame's global-frame-indexed offset. The frame
//! outcome is thus a pure function of the global frame index — the property
//! that makes the aggregate byte-identical across worker counts (design doc §3,
//! §11).

use gf2_coding::dvb_t2_bicm_harness::{ebn0_to_esn0, esn0_to_ebn0, rate_f64};
use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::{ConcatError, DvbT2Concat};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use gf2_coding::simulation::count_bit_errors;
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;

use crate::parallel::{FrameOutcome, WorkerCtx};

/// BP iteration sentinel for a frame that fails to produce any BBFRAME estimate
/// (unreachable in practice; matches the baseline closure's `50` fallback —
/// the default DVB-T2 max BP iteration count).
const DECODE_HARD_FAIL_ITERS: u64 = 50;

/// A self-contained DVB-T2 BICM-AWGN single-frame simulator for one MODCOD and
/// Es/N0 point.
///
/// Holds the concatenated BCH+LDPC codec, the bit interleaver, the Gray-QAM
/// mapper/demapper, and the precomputed AWGN parameters for the configured
/// Es/N0. [`simulate_frame`](Self::simulate_frame) runs one frame and returns a
/// [`FrameOutcome`].
///
/// # Per-worker ownership (no shared decoder)
///
/// `gf2-coding`'s [`DvbT2Concat`] wraps its LDPC decoder in a `Mutex` (so
/// `decode_soft` can take `&self`). Sharing **one** `DvbT2BicmFrameSim` across
/// rayon workers would serialise every decode on that lock and erase the
/// speedup. Instead, give each worker its **own** clone: this type is [`Clone`],
/// so the canonical use is
/// [`run_snr_point`](crate::parallel::run_snr_point) with a
/// `make_state = || template.clone()` factory. Cloning copies the configured
/// codec (a fresh, independent decoder per worker), keeping the per-frame outcome
/// a pure function of the global frame index.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::frame_sim::DvbT2BicmFrameSim;
/// use gf2_sim::parallel::run_snr_point;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
/// use gf2_coding::modem::DemapMethod;
/// use gf2_coding::CodeRate;
///
/// let template = DvbT2BicmFrameSim::new(
///     CodeRate::Rate1_2,
///     DvbT2Modulation::Qam16,
///     9.0, // Es/N0 dB, above threshold
///     DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
///     DemapMethod::ExactLogMap,
/// );
///
/// // Each worker clones its own simulator (own decoder, no lock contention).
/// let counters = run_snr_point(
///     42, 0, 1, NonZeroUsize::new(1).unwrap(),
///     || template.clone(),
///     |g, ctx, sim| sim.simulate_frame(g, ctx),
/// );
/// assert_eq!(counters.frames, 1);
/// ```
pub struct DvbT2BicmFrameSim {
    // Build parameters retained so [`Clone`] can rebuild the (non-`Clone`)
    // codec for a fresh per-worker instance.
    rate: CodeRate,
    modulation: DvbT2Modulation,
    decoder: DecoderConfig,
    codec: DvbT2Concat,
    interleaver: DvbT2BitInterleaver,
    spec: ModemSpec<f32>,
    demap: DemapMethod,
    bits_per_symbol: usize,
    k: usize,
    es_n0_db: f64,
    /// Per-axis noise standard deviation `sigma = sqrt(sigma_sq)`.
    sigma: f32,
    /// Per-symbol total complex noise variance `N0 = 2 * sigma_sq`.
    noise_var: f32,
}

impl Clone for DvbT2BicmFrameSim {
    /// Rebuilds an independent simulator (fresh codec / decoder) from the stored
    /// build parameters.
    ///
    /// [`DvbT2Concat`] is not `Clone` (it holds a `Mutex`/`OnceCell`), so the
    /// clone reconstructs it via [`DvbT2BicmFrameSim::new`]. This is exactly what
    /// the per-worker `make_state` factory needs: each worker gets its own
    /// decoder and never contends on a shared lock.
    fn clone(&self) -> Self {
        Self::new(
            self.rate,
            self.modulation,
            self.es_n0_db,
            self.decoder,
            self.demap,
        )
    }
}

impl DvbT2BicmFrameSim {
    /// Builds a frame simulator for a DVB-T2 MODCOD at a fixed Es/N0 point.
    ///
    /// The codec is constructed for [`FrameSize::Normal`] (n = 64800) — the
    /// in-scope DVB-T2 FECFRAME — and the supplied decoder configuration is
    /// applied to it. The AWGN per-axis variance is derived from `es_n0_db`
    /// using the same `sigma^2 = 1 / (2 * 10^(Es/N0 / 10))` formula as the
    /// legacy baseline harness.
    ///
    /// # Arguments
    ///
    /// * `rate` — DVB-T2 LDPC code rate (1/2, 2/3, 3/4 are in scope).
    /// * `modulation` — DVB-T2 modulation order (16-QAM or 64-QAM in scope).
    /// * `es_n0_db` — channel Es/N0 in dB (e.g. 6.25 for the canonical point).
    /// * `decoder` — LDPC belief-propagation decoder configuration.
    /// * `demap` — soft-demap method ([`DemapMethod::ExactLogMap`] or
    ///   [`DemapMethod::MaxLog`]).
    ///
    /// # Panics
    ///
    /// Panics if the `(FrameSize::Normal, rate)` codec cannot be constructed
    /// (every in-scope DVB-T2 rate constructs successfully).
    #[must_use]
    pub fn new(
        rate: CodeRate,
        modulation: DvbT2Modulation,
        es_n0_db: f64,
        decoder: DecoderConfig,
        demap: DemapMethod,
    ) -> Self {
        let mut codec = DvbT2Concat::new(FrameSize::Normal, rate)
            .expect("DVB-T2 Normal-frame codec construction must succeed for in-scope rates");
        codec.set_decoder_config(decoder);

        let bits_per_symbol = modulation.bits_per_cell();
        let order = 1usize << bits_per_symbol;
        let spec = ModemSpec::<f32>::gray_square_qam(order);

        let modcod = DvbT2Modcod::new(FrameSize::Normal, rate, modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);

        // AWGN parameters (identical formula to BicmAwgnChannel):
        //   sigma_sq = 1 / (2 * 10^(Es/N0 / 10)),  N0 = 2 * sigma_sq.
        let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let sigma = (sigma_sq as f32).sqrt();
        let noise_var = (2.0 * sigma_sq) as f32;

        let k = codec.k_bch();

        Self {
            rate,
            modulation,
            decoder,
            codec,
            interleaver,
            spec,
            demap,
            bits_per_symbol,
            k,
            es_n0_db,
            sigma,
            noise_var,
        }
    }

    /// Builds a frame simulator from a channel Eb/N0 instead of Es/N0.
    ///
    /// Converts Eb/N0 → Es/N0 via the BICM offset `10*log10(m * r)` (the same
    /// conversion the baseline harness applies) and delegates to [`new`].
    ///
    /// # Arguments
    ///
    /// Same as [`new`], except `eb_n0_db` is the per-information-bit SNR.
    ///
    /// [`new`]: Self::new
    #[must_use]
    pub fn from_eb_n0(
        rate: CodeRate,
        modulation: DvbT2Modulation,
        eb_n0_db: f64,
        decoder: DecoderConfig,
        demap: DemapMethod,
    ) -> Self {
        let es_n0_db = ebn0_to_esn0(eb_n0_db, modulation.bits_per_cell(), rate_f64(rate));
        Self::new(rate, modulation, es_n0_db, decoder, demap)
    }

    /// The information-bit count `k` per frame (BBFRAME size).
    #[inline]
    #[must_use]
    pub fn k(&self) -> usize {
        self.k
    }

    /// The Es/N0 (dB) this simulator runs at.
    #[inline]
    #[must_use]
    pub fn es_n0_db(&self) -> f64 {
        self.es_n0_db
    }

    /// The Eb/N0 (dB) equivalent of this simulator's Es/N0 for the MODCOD.
    #[must_use]
    pub fn eb_n0_db(&self, rate: CodeRate) -> f64 {
        esn0_to_ebn0(self.es_n0_db, self.bits_per_symbol, rate_f64(rate))
    }

    /// Simulates one frame and returns its [`FrameOutcome`].
    ///
    /// Per-frame sequence (all randomness from `ctx`'s RNG):
    ///
    /// 1. Draw a random BBFRAME `message` of `k` bits.
    /// 2. BCH+LDPC encode → `n_ldpc` FECFRAME codeword.
    /// 3. Bit-interleave → Gray-QAM map to I/Q symbols.
    /// 4. Add independent Box-Muller AWGN on the I and Q axes.
    /// 5. Soft-demap (with the true channel `N0`) → bit-deinterleave LLRs.
    /// 6. LDPC+BCH soft-decode back to a BBFRAME estimate.
    /// 7. Count information-bit errors vs the transmitted `message`.
    ///
    /// A frame is "in error" iff any information bit differs. Non-converged LDPC
    /// decodes keep their best-effort BBFRAME estimate (matching the baseline's
    /// `LdpcDecodeFailed` handling).
    ///
    /// # Arguments
    ///
    /// * `_global_frame_idx` — the frame's global index (unused directly; the
    ///   dispatcher has already reseeked `ctx`'s RNG to this frame's region).
    /// * `ctx` — the worker context whose RNG supplies all randomness.
    ///
    /// # Returns
    ///
    /// The [`FrameOutcome`] for this frame.
    ///
    /// # Complexity
    ///
    /// Dominated by one LDPC belief-propagation decode (`O(iters · edges)`).
    pub fn simulate_frame(&self, _global_frame_idx: usize, ctx: &mut WorkerCtx) -> FrameOutcome {
        let rng = ctx.rng_mut();

        // 1. Random BBFRAME (filled from the rand_chacha 0.9 stream; see
        //    `random_bitvec` for why we do not call `BitVec::random`).
        let message = random_bitvec(self.k, rng);
        // 2. Encode.
        let codeword = self.codec.encode(&message);
        let n_ldpc = codeword.len();
        let num_symbols = n_ldpc / self.bits_per_symbol;

        // 3. Bit-interleave then Gray-QAM map.
        let interleaved = self.interleaver.interleave(&codeword);
        let interleaved_bits: Vec<bool> =
            (0..interleaved.len()).map(|b| interleaved.get(b)).collect();
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        let mapper = self.spec.preferred_mapper();
        mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

        // 4. AWGN: independent Box-Muller noise on each axis (same draw order as
        // the baseline: all I-axis samples first, then all Q-axis samples).
        for s in tx_i.iter_mut() {
            *s += self.sigma * box_muller(rng);
        }
        for s in tx_q.iter_mut() {
            *s += self.sigma * box_muller(rng);
        }

        // 5. Soft-demap with the true channel N0, then deinterleave.
        let noise_var_buf = vec![self.noise_var; num_symbols];
        let mut interleaved_llrs = vec![Llr::new(0.0); n_ldpc];
        let demapper = self.spec.preferred_soft_demapper();
        demapper.demap_llrs(
            DemapInput {
                rx_i: &tx_i,
                rx_q: &tx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &noise_var_buf,
                method: self.demap,
            },
            &mut interleaved_llrs,
        );
        let llrs = self.interleaver.deinterleave_llrs(&interleaved_llrs);

        // 6. Decode (mirror the baseline closure; sentinel iteration 1 on
        // convergence, the real iteration count on non-convergence).
        let (decoded, iterations) = match self.codec.decode_soft(&llrs) {
            Ok(bbframe) => (bbframe, 1u64),
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            }) => (bbframe, iterations as u64),
            Err(_) => (BitVec::with_capacity(self.k), DECODE_HARD_FAIL_ITERS),
        };

        // 7. Information-bit error count.
        let bit_errors = count_bit_errors(&message, &decoded) as u64;
        FrameOutcome {
            errored: bit_errors > 0,
            iterations,
            info_bits: self.k as u64,
            bit_errors,
        }
    }
}

/// Builds a `len_bits`-bit [`BitVec`] filled from a `rand_chacha 0.9` RNG.
///
/// `gf2_core::BitVec::random` requires a `rand 0.8` `Rng`, but the per-worker
/// stream is `rand_chacha 0.9` (the design-doc §5 pin), so we fill the word
/// storage directly and zero the padding bits beyond `len_bits` to uphold the
/// tail-masking invariant (`gf2-core` design invariant 1). Each word is drawn
/// as one `u64` so the draw count per frame is deterministic.
#[inline]
fn random_bitvec<R: rand::Rng>(len_bits: usize, rng: &mut R) -> BitVec {
    if len_bits == 0 {
        return BitVec::new();
    }
    let num_words = len_bits.div_ceil(64);
    let mut data: Vec<u64> = (0..num_words).map(|_| rng.random::<u64>()).collect();
    // Tail-mask the final word so padding bits beyond `len_bits` are zero.
    let tail = len_bits & 63;
    if tail != 0 {
        let mask = (1u64 << tail) - 1;
        let last = num_words - 1;
        data[last] &= mask;
    }
    BitVec::from_words(data, len_bits)
}

/// One standard-normal sample via the Box-Muller transform.
///
/// Draws two uniforms from `rng` and returns one Gaussian sample (the cosine
/// branch), matching the legacy baseline harness's per-axis noise generation
/// exactly. Draws `u1` then `u2`; clamps `u1` away from zero to avoid `ln(0)`.
#[inline]
fn box_muller<R: rand::Rng>(rng: &mut R) -> f32 {
    let u1: f64 = rng.random::<f64>().max(1e-15);
    let u2: f64 = rng.random::<f64>();
    ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::DecoderAlgorithm;
    use std::num::NonZeroUsize;

    #[test]
    fn test_frame_sim_constructs_normal_frame() {
        let sim = DvbT2BicmFrameSim::new(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            6.25,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
        );
        assert!(sim.k() > 0);
        assert!((sim.es_n0_db() - 6.25).abs() < 1e-9);
    }

    #[test]
    fn test_single_frame_above_threshold_decodes() {
        use crate::parallel::run_snr_point;
        // One frame at a high Es/N0 (well above the r1/2 16-QAM waterfall) must
        // decode without error. Single frame keeps the fast tier under 5 s.
        let sim = DvbT2BicmFrameSim::new(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            9.0,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
        );
        let counters = run_snr_point(
            7,
            0,
            1,
            NonZeroUsize::new(1).unwrap(),
            || sim.clone(),
            |g, ctx, s| s.simulate_frame(g, ctx),
        );
        assert_eq!(counters.frames, 1);
        assert_eq!(counters.errors, 0, "frame above threshold must decode");
    }
}
