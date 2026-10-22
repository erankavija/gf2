//! DVB-T2 concatenated implementer of [`BatchSoftDecoder`]: the LDPC inner
//! code of a batch decodes through [`LdpcBatchDecoder`], the BCH outer code
//! frame by frame.

use super::{bch_decode_ldpc_codeword, DvbT2Concat};
use crate::bch::dvb_t2::DvbT2BchCode;
use crate::ldpc::{DecodedWord, LdpcBatchDecoder};
use crate::llr::Llr;
use crate::traits::block::BlockCode;
use crate::traits::{BatchSoftDecoder, DecodeOutcome};
use crate::tuning::{self, DecodeSelectors};
use gf2_core::BitVec;
use std::num::NonZeroUsize;

/// Batch decoder of the DVB-T2 concatenated BCH and LDPC code
/// (`@/citation/Etsi2015` §6): FECFRAMEs of LLRs in, BBFRAMEs out.
///
/// The decoder takes the codes, the LDPC decoder configuration and the
/// iteration cap of a [`DvbT2Concat`] at construction; a later change of the
/// codec does not reach it. A batch decodes in the waves of an owned
/// [`LdpcBatchDecoder`], routed by its [`DecodeSelectors`], and each LDPC
/// codeword then passes the BCH outer decode of
/// [`DvbT2Concat::decode_bch_from_ldpc_codeword`].
///
/// A frame's BBFRAME and LDPC iteration count equal those of
/// [`DvbT2Concat::decode_soft_counted`] on the same LLRs, in either arm of its
/// result. The frame's [`DecodeOutcome`] is that of the LDPC inner decode:
/// `converged` is false where `decode_soft_counted` returns
/// [`ConcatError::LdpcDecodeFailed`](super::ConcatError::LdpcDecodeFailed),
/// and the decoded word is then that error's BBFRAME estimate. The outcome of
/// the BCH outer decode is not reported.
///
/// The decoder owns the LDPC lane workspace and one codeword per lane and
/// takes no lock. The BCH outer decode allocates per frame as
/// [`DvbT2Concat::decode_bch_from_ldpc_codeword`] does.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::dvb_t2::concat::{DvbT2Concat, DvbT2ConcatBatchDecoder};
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::llr::Llr;
/// use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let codec = DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).unwrap();
/// let bbframes: Vec<BitVec> = (0..3)
///     .map(|seed| BitVec::random_seeded(codec.k_bch(), seed))
///     .collect();
/// let frames: Vec<Vec<Llr>> = bbframes
///     .iter()
///     .map(|bbframe| {
///         let fecframe = codec.encode(bbframe);
///         (0..fecframe.len())
///             .map(|i| Llr::new(if fecframe.get(i) { -4.0 } else { 4.0 }))
///             .collect()
///     })
///     .collect();
///
/// let mut decoder = DvbT2ConcatBatchDecoder::new(&codec);
/// let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
/// let mut decoded = vec![BitVec::new(); views.len()];
/// let mut outcomes = vec![DecodeOutcome::default(); views.len()];
/// decoder.decode_batch(&views, &mut decoded, &mut outcomes);
///
/// assert_eq!(decoded, bbframes);
/// assert!(outcomes.iter().all(|outcome| outcome.converged));
/// ```
pub struct DvbT2ConcatBatchDecoder {
    bch_code: DvbT2BchCode,
    ldpc: LdpcBatchDecoder,
    /// LDPC codewords of one wave.
    codewords: Vec<BitVec>,
}

impl DvbT2ConcatBatchDecoder {
    /// Builds the batch decoder of `codec` with the decode selectors of the
    /// process-wide coding tuning section ([`tuning::active`]), read once
    /// here.
    ///
    /// # Panics
    ///
    /// As [`Self::with_selectors`], and as [`tuning::active`].
    #[must_use]
    pub fn new(codec: &DvbT2Concat) -> Self {
        let selectors = tuning::active().section.decode().clone();
        Self::with_selectors(codec, selectors)
    }

    /// Builds the batch decoder of `codec` with the given decode selectors.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    ///
    /// Construction clones both codes and allocates the workspace of an
    /// [`LdpcBatchDecoder`] and one FECFRAME-length codeword per lane.
    ///
    /// # Panics
    ///
    /// As [`LdpcBatchDecoder::with_selectors`] for
    /// [`DecodedWord::Codeword`].
    #[must_use]
    pub fn with_selectors(codec: &DvbT2Concat, selectors: DecodeSelectors) -> Self {
        let ldpc = LdpcBatchDecoder::with_selectors(
            codec.ldpc_code.clone(),
            codec.decoder_config,
            codec.max_ldpc_iterations,
            DecodedWord::Codeword,
            selectors,
        );
        let codewords = vec![BitVec::with_capacity(codec.n_ldpc); ldpc.batch_granularity()];
        Self {
            bch_code: codec.bch_code.clone(),
            ldpc,
            codewords,
        }
    }

    /// Holds the lane route of the LDPC inner decoder on the portable
    /// backend, as [`LdpcBatchDecoder::with_portable_lanes`] does.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    #[must_use]
    pub fn with_portable_lanes(mut self) -> Self {
        self.ldpc = self.ldpc.with_portable_lanes();
        self
    }

    /// The LDPC inner batch decoder, for its selectors, iteration cap and
    /// [`batch_route`](LdpcBatchDecoder::batch_route).
    #[must_use]
    pub fn ldpc(&self) -> &LdpcBatchDecoder {
        &self.ldpc
    }
}

impl BatchSoftDecoder for DvbT2ConcatBatchDecoder {
    /// The FECFRAME length, [`DvbT2Concat::n_ldpc`].
    fn frame_len(&self) -> usize {
        self.ldpc.frame_len()
    }

    /// The BBFRAME length, [`DvbT2Concat::k_bch`].
    fn decoded_len(&self) -> usize {
        BlockCode::k(&self.bch_code)
    }

    /// The granularity of [`LdpcBatchDecoder`], the frames of one wave.
    fn batch_granularity(&self) -> usize {
        self.ldpc.batch_granularity()
    }

    /// Decodes consecutive waves of
    /// [`batch_granularity`](Self::batch_granularity) frames: the LDPC inner
    /// code of a wave in one [`LdpcBatchDecoder`] batch, then the BCH outer
    /// code of each of its frames.
    ///
    /// # Panics
    ///
    /// Panics as [`BatchSoftDecoder::decode_batch`] does, before any frame
    /// decodes.
    fn decode_batch(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
    ) {
        self.decode_batch_with_workers(frames, decoded, outcomes, NonZeroUsize::MIN);
    }

    fn decode_batch_with_workers(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        workers: NonZeroUsize,
    ) {
        let n = self.frame_len();
        assert_eq!(
            decoded.len(),
            frames.len(),
            "a batch needs one decoded word per frame"
        );
        assert_eq!(
            outcomes.len(),
            frames.len(),
            "a batch needs one outcome per frame"
        );
        for (index, llrs) in frames.iter().enumerate() {
            assert_eq!(
                llrs.len(),
                n,
                "frame {index} has {} LLRs, the decoder takes {n}",
                llrs.len()
            );
        }

        // Waves cut here are the waves `LdpcBatchDecoder` cuts from the whole
        // batch, so each takes the same route.
        let lanes = self.codewords.len();
        for ((wave, decoded), outcomes) in frames
            .chunks(lanes)
            .zip(decoded.chunks_mut(lanes))
            .zip(outcomes.chunks_mut(lanes))
        {
            let codewords = &mut self.codewords[..wave.len()];
            self.ldpc
                .decode_batch_with_workers(wave, codewords, outcomes, workers);
            for (bbframe, codeword) in decoded.iter_mut().zip(codewords.iter()) {
                *bbframe = bch_decode_ldpc_codeword(&self.bch_code, codeword);
            }
        }
    }
}
