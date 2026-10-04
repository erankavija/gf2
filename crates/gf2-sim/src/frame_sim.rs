//! Deterministic single-frame DVB-T2 BICM-AWGN simulation kernel.
//! [`DvbT2BicmFrameSim::simulate_frame`] draws the transmitted BBFRAME and the
//! AWGN noise from the supplied [`WorkerCtx`]'s RNG, which the dispatcher
//! ([`run_snr_point`](crate::parallel::run_snr_point)) has reseeked to the
//! frame's global-index offset, so the outcome is a function of the global
//! frame index.

use rand::Rng as _;

use gf2_coding::dvb_t2_bicm_harness::{box_muller_cos, rate_f64, BicmAwgnChannel};
use gf2_coding::info_theory::{ebn0_to_esn0, esn0_to_ebn0};
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

/// BP iteration count recorded for a frame whose decode returns no BBFRAME
/// estimate.
const DECODE_HARD_FAIL_ITERS: u64 = 50;

/// A DVB-T2 BICM-AWGN single-frame simulator for one MODCOD and Es/N0 point.
///
/// [`DvbT2Concat`] holds its LDPC decoder behind a `Mutex`, so a simulator
/// shared across workers serialises every decode. Give each worker its own
/// clone, as in `make_state = || template.clone()` for
/// [`run_snr_point`](crate::parallel::run_snr_point).
pub struct DvbT2BicmFrameSim {
    // Build parameters retained so [`Clone`] can rebuild the (non-`Clone`)
    // codec for a fresh per-worker instance.
    rate: CodeRate,
    modulation: DvbT2Modulation,
    decoder: DecoderConfig,
    demap: DemapMethod,
    codec: DvbT2Concat,
    /// Transmit and demodulate chain; this kernel supplies the noise draws.
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
    /// Rebuilds an independent codec through [`DvbT2BicmFrameSim::new`];
    /// [`DvbT2Concat`] is not `Clone`.
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
    /// The codec is built for [`FrameSize::Normal`] (n = 64800) with `decoder`
    /// applied.
    ///
    /// # Panics
    ///
    /// Panics if the `(FrameSize::Normal, rate)` codec cannot be constructed.
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
        let channel = BicmAwgnChannel::new(interleaver, bits_per_symbol, demap);

        // Both conversions take the unrounded f64 `es_n0_db`, so sigma and N0
        // correspond to the same SNR.
        let sigma = crate::channels::es_n0_db_to_sigma_f64(es_n0_db);
        let noise_var = crate::channels::es_n0_db_to_n0_f64(es_n0_db);

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

    /// Builds a frame simulator from a channel Eb/N0, converted to Es/N0 by
    /// the BICM offset `10*log10(m * r)`.
    ///
    /// # Panics
    ///
    /// As [`new`](Self::new).
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

    /// The FECFRAME codeword length `n_ldpc` (the LLR count per frame).
    #[inline]
    #[must_use]
    pub fn n_ldpc(&self) -> usize {
        self.codec.n_ldpc()
    }

    /// The DVB-T2 LDPC code this simulator decodes.
    #[inline]
    #[must_use]
    pub fn ldpc_code(&self) -> gf2_coding::ldpc::LdpcCode {
        self.codec.ldpc_code()
    }

    /// The LDPC belief-propagation decoder configuration.
    #[inline]
    #[must_use]
    pub fn decoder_config(&self) -> DecoderConfig {
        self.decoder
    }

    /// The LDPC code rate.
    #[inline]
    #[must_use]
    pub fn rate(&self) -> CodeRate {
        self.rate
    }

    /// The DVB-T2 modulation order.
    #[inline]
    #[must_use]
    pub fn modulation(&self) -> DvbT2Modulation {
        self.modulation
    }

    /// The soft-demap method.
    #[inline]
    #[must_use]
    pub fn demap(&self) -> DemapMethod {
        self.demap
    }

    /// The per-axis AWGN noise standard deviation `sigma`.
    #[inline]
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma
    }

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`).
    #[inline]
    #[must_use]
    pub fn noise_var(&self) -> f32 {
        self.noise_var
    }

    /// The Es/N0 (dB) this simulator runs at.
    #[inline]
    #[must_use]
    pub fn es_n0_db(&self) -> f64 {
        self.es_n0_db
    }

    /// The Eb/N0 (dB) equivalent of this simulator's Es/N0 at code rate `rate`.
    #[must_use]
    pub fn eb_n0_db(&self, rate: CodeRate) -> f64 {
        esn0_to_ebn0(self.es_n0_db, self.bits_per_symbol, rate_f64(rate))
    }

    /// Simulates one frame: random `k`-bit BBFRAME, BCH+LDPC encode,
    /// interleave, Gray-QAM map, per-axis Box-Muller AWGN, soft demap with the
    /// channel `N0`, deinterleave, soft decode, information-bit error count.
    ///
    /// All randomness comes from `ctx`'s RNG; `_global_frame_idx` is unused
    /// because the dispatcher has already positioned that RNG. A frame is
    /// errored iff any information bit differs. A non-converged LDPC decode
    /// keeps its best-effort BBFRAME estimate, and `iterations` is the BP
    /// iteration count on both the converged and the non-converged arm.
    ///
    /// # Complexity
    ///
    /// Dominated by one LDPC belief-propagation decode (`O(iters · edges)`).
    pub fn simulate_frame(&self, _global_frame_idx: usize, ctx: &mut WorkerCtx) -> FrameOutcome {
        let rng = ctx.rng_mut();

        let message = random_bitvec(self.k, rng);
        let codeword = self.codec.encode(&message);

        // Draw order: u1 then u2 per sample, all I-axis samples then all
        // Q-axis samples.
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

        let (decoded, iterations) = match self.codec.decode_soft_counted(&llrs) {
            Ok((bbframe, iters)) => (bbframe, iters as u64),
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            }) => (bbframe, iterations as u64),
            Err(_) => (BitVec::with_capacity(self.k), DECODE_HARD_FAIL_ITERS),
        };

        let bit_errors = count_bit_errors(&message, &decoded) as u64;
        FrameOutcome {
            errored: bit_errors > 0,
            iterations,
            info_bits: self.k as u64,
            bit_errors,
        }
    }

    /// CPU batch-prep half of one frame: draws the BBFRAME and the AWGN
    /// realisation and returns the transmitted message with the channel LLRs,
    /// omitting the decode of [`simulate_frame`](Self::simulate_frame).
    ///
    /// All randomness comes from `ctx`'s RNG; `_global_frame_idx` is unused.
    ///
    /// # Complexity
    ///
    /// O(`n_ldpc`) encode + interleave + map + demap (no BP decode).
    pub fn prepare_frame(&self, _global_frame_idx: usize, ctx: &mut WorkerCtx) -> FramePrep {
        let rng = ctx.rng_mut();
        let message = random_bitvec(self.k, rng);
        let codeword = self.codec.encode(&message);
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
        FramePrep { message, llrs }
    }

    /// CPU decode tail of the hybrid path: BCH-decodes the GPU LDPC
    /// hard-decision codeword and counts information-bit errors against
    /// `message`, reporting the supplied `iterations`.
    ///
    /// # Panics
    ///
    /// Panics if `ldpc_codeword.len() != n_ldpc()`.
    ///
    /// # Complexity
    ///
    /// O(`k_ldpc`) BCH decode.
    #[must_use]
    pub fn decode_codeword_to_outcome(
        &self,
        message: &BitVec,
        ldpc_codeword: &BitVec,
        iterations: u64,
    ) -> FrameOutcome {
        let decoded = self.codec.decode_bch_from_ldpc_codeword(ldpc_codeword);
        let bit_errors = count_bit_errors(message, &decoded) as u64;
        FrameOutcome {
            errored: bit_errors > 0,
            iterations,
            info_bits: self.k as u64,
            bit_errors,
        }
    }
}

/// Output of [`DvbT2BicmFrameSim::prepare_frame`]: the transmitted message and
/// the channel LLRs the device LDPC decoder consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct FramePrep {
    /// The transmitted BBFRAME (`k` information bits).
    pub message: BitVec,
    /// The channel LLRs (`n_ldpc` values), one per FECFRAME bit.
    pub llrs: Vec<gf2_coding::Llr>,
}

/// Builds a `len_bits`-bit [`BitVec`] from a `rand 0.9` RNG, one `u64` draw
/// per word, with the padding bits beyond `len_bits` zeroed.
///
/// `gf2_core::BitVec::random` takes a `rand 0.8` RNG. The topology executor
/// draws its BBFRAME with this helper, so its RNG stream position matches
/// [`DvbT2BicmFrameSim::simulate_frame`]'s.
#[inline]
pub(crate) fn random_bitvec<R: rand::Rng>(len_bits: usize, rng: &mut R) -> BitVec {
    if len_bits == 0 {
        return BitVec::new();
    }
    let num_words = len_bits.div_ceil(64);
    let mut data: Vec<u64> = (0..num_words).map(|_| rng.random::<u64>()).collect();
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

    #[test]
    fn test_from_eb_n0_constructs_sim() {
        let sim = DvbT2BicmFrameSim::from_eb_n0(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            5.0,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
        );
        assert!(sim.k() > 0);
        assert!(sim.n_ldpc() > sim.k());
        let roundtrip = sim.eb_n0_db(CodeRate::Rate1_2);
        assert!(
            (roundtrip - 5.0).abs() < 0.5,
            "eb_n0 roundtrip: {roundtrip}"
        );
    }

    #[test]
    fn test_all_accessor_methods() {
        let sim = DvbT2BicmFrameSim::new(
            CodeRate::Rate2_3,
            DvbT2Modulation::Qam64,
            7.0,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, false),
            DemapMethod::ExactLogMap,
        );
        assert!(sim.k() > 0, "k must be positive");
        assert!(sim.n_ldpc() > sim.k(), "n_ldpc must exceed k");
        let _code = sim.ldpc_code();
        let dc = sim.decoder_config();
        assert_eq!(dc.algorithm(), DecoderAlgorithm::SumProduct);
        assert_eq!(sim.rate(), CodeRate::Rate2_3);
        assert_eq!(sim.modulation(), DvbT2Modulation::Qam64);
        assert_eq!(sim.demap(), DemapMethod::ExactLogMap);
        assert!(sim.sigma() > 0.0, "sigma must be positive");
        assert!(
            (sim.noise_var() - 2.0 * sim.sigma() * sim.sigma()).abs() < 1e-4,
            "noise_var must equal 2*sigma^2"
        );
        assert!((sim.es_n0_db() - 7.0).abs() < 1e-9, "es_n0_db roundtrip");
        let eb = sim.eb_n0_db(CodeRate::Rate2_3);
        assert!(
            eb.is_finite() && eb > 0.0,
            "eb_n0_db must be finite positive"
        );
    }

    #[test]
    fn test_random_bitvec_zero_length_returns_empty() {
        use rand::SeedableRng;
        use rand_chacha::ChaCha20Rng;
        let mut rng = ChaCha20Rng::seed_from_u64(0);
        let bv = random_bitvec(0, &mut rng);
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_prepare_frame_and_decode_codeword_to_outcome() {
        let sim = DvbT2BicmFrameSim::new(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            9.0,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
        );
        let mut ctx = crate::parallel::WorkerCtx::new(42, 0, 0);
        let prep = sim.prepare_frame(0, &mut ctx);
        assert_eq!(prep.message.len(), sim.k(), "message has k bits");
        assert_eq!(prep.llrs.len(), sim.n_ldpc(), "llrs has n_ldpc entries");

        let codeword = gf2_core::BitVec::zeros(sim.n_ldpc());
        let outcome = sim.decode_codeword_to_outcome(&prep.message, &codeword, 10);
        assert_eq!(outcome.info_bits, sim.k() as u64);
        assert_eq!(outcome.iterations, 10);
    }
}
