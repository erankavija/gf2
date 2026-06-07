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

use rand::Rng as _;

use gf2_coding::dvb_t2_bicm_harness::{
    box_muller_cos, ebn0_to_esn0, esn0_to_ebn0, rate_f64, BicmAwgnChannel,
};
use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::{ConcatError, DvbT2Concat};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::DemapMethod;
use gf2_coding::simulation::count_bit_errors;
use gf2_coding::CodeRate;
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
/// `no_run`: a full n=64800 encode+decode is too heavy for an unoptimised
/// doctest build (doctests run without `--release`). The example still compiles,
/// satisfying the public-API example requirement; the executed coverage lives in
/// the `--release` unit/integration tests.
///
/// ```no_run
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
    demap: DemapMethod,
    codec: DvbT2Concat,
    /// The canonical DVB-T2 BICM-AWGN transmit/demod chain (SSOT in
    /// `gf2-coding`); the frame kernel only supplies the noise draws.
    channel: BicmAwgnChannel,
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

        let modcod = DvbT2Modcod::new(FrameSize::Normal, rate, modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);
        // The canonical BICM-AWGN chain (SSOT in gf2-coding); the frame kernel
        // supplies only the per-axis noise draws.
        let channel = BicmAwgnChannel::new(interleaver, bits_per_symbol, demap);

        // AWGN parameters (same formula the channel's eb_n0 path computes):
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
            demap,
            codec,
            channel,
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

        // 3-5. Interleave → QAM-map → per-axis AWGN → soft-demap → deinterleave,
        // delegated to the canonical chain in gf2-coding. The noise draws come
        // from the per-worker rand_chacha 0.9 stream (draw u1 then u2 per sample,
        // all I-axis samples then all Q-axis samples — the chain's draw contract).
        let llrs = self.channel.transmit_and_demodulate_with_noise(
            &codeword,
            self.sigma,
            self.noise_var,
            || {
                let u1 = rng.random::<f64>();
                let u2 = rng.random::<f64>();
                box_muller_cos(u1, u2)
            },
        );

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
