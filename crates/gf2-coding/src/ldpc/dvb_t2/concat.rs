//! DVB-T2 BCH+LDPC concatenated codec (`@/citation/Etsi2015` §6):
//! [`DvbT2Concat`] wraps the BCH outer code and the LDPC inner code.

use crate::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, DvbT2BchDecoder};
use crate::ldpc::{DecoderConfig, LdpcCode, LdpcDecoder, LdpcEncoder};
use crate::llr::Llr;
use crate::traits::block::{BlockCode, BlockEncoder};
// `LdpcEncoder` implements `traits::BlockEncoder`; the BCH outer code
// implements the `traits::block` trait of the same name.
use crate::traits::BlockEncoder as LdpcBlockEncoder;
use gf2_core::BitVec;
use once_cell::sync::OnceCell;
use std::sync::Mutex;

use super::FrameSize;
use crate::bch::CodeRate;

use crate::bch::dvb_t2::FrameSize as BchFrameSize;

/// Error type returned by [`DvbT2Concat::new`] and [`DvbT2Concat::decode_soft`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcatError {
    /// The (frame_size, code_rate) pair is not covered by this implementation.
    Unsupported {
        /// Requested frame size.
        frame_size: FrameSize,
        /// Requested code rate.
        code_rate: CodeRate,
    },
    /// LDPC belief propagation did not converge; the BCH codeword may contain
    /// residual errors. The partially-decoded BBFRAME is returned as the
    /// payload.
    LdpcDecodeFailed {
        /// Best estimate of the BBFRAME (BCH-corrected where possible).
        bbframe: BitVec,
        /// Number of BP iterations performed.
        iterations: usize,
    },
}

impl std::fmt::Display for ConcatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported {
                frame_size,
                code_rate,
            } => write!(
                f,
                "DVB-T2 concat: unsupported configuration ({:?}, {:?})",
                frame_size, code_rate
            ),
            Self::LdpcDecodeFailed { iterations, .. } => write!(
                f,
                "DVB-T2 concat: LDPC BP did not converge after {} iterations",
                iterations
            ),
        }
    }
}

impl std::error::Error for ConcatError {}

/// DVB-T2 concatenated BCH + LDPC FEC codec (`@/citation/Etsi2015` §6).
///
/// The LDPC encoder is initialised lazily on the first call to
/// [`encode`](Self::encode) (stored in a [`OnceCell`]). For DVB-T2 codes the
/// encoder is the linear-time IRA staircase accumulator, so its construction
/// is O(nnz). The LDPC decoder
/// is wrapped in a [`Mutex`] so that `decode_soft` takes `&self` while still
/// allowing BP scratch-buffer mutation; this also makes `DvbT2Concat` [`Sync`].
pub struct DvbT2Concat {
    /// BCH outer code, the mother code of its frame size shortened to the
    /// standard's `K_bch`.
    bch_code: DvbT2BchCode,
    /// LDPC code (held to construct the encoder lazily).
    ldpc_code: LdpcCode,
    /// LDPC encoder (IRA staircase accumulator for DVB-T2), initialised on
    /// first encode call.
    ldpc_encoder: OnceCell<LdpcEncoder>,
    /// LDPC decoder (belief propagation, with early termination).
    /// Wrapped in a Mutex so decode_soft can take &self.
    ldpc_decoder: Mutex<LdpcDecoder>,
    /// BCH information block size (BBFRAME bits).
    k_bch: usize,
    /// BCH codeword length = LDPC input length.
    k_ldpc: usize,
    /// LDPC codeword length = FECFRAME bits.
    n_ldpc: usize,
    /// Maximum BP iterations for LDPC decoding.
    max_ldpc_iterations: usize,
}

impl DvbT2Concat {
    /// Construct a DVB-T2 concatenated codec.
    ///
    /// Every (frame_size, code_rate) pair is constructible, so the result is
    /// always `Ok`. The LDPC encoder is created lazily on the first call to
    /// [`encode`](Self::encode).
    ///
    /// # Panics
    ///
    /// Panics if the standard's table row for this configuration does not
    /// name a constructible BCH code, which is an invariant violation.
    ///
    /// # Complexity
    ///
    /// O(nnz) for decoder graph allocation plus the BCH mother-code
    /// construction.
    pub fn new(frame_size: FrameSize, code_rate: CodeRate) -> Result<Self, ConcatError> {
        // Map LDPC FrameSize → BCH FrameSize (same logical enum, separate types).
        let bch_frame_size = match frame_size {
            FrameSize::Short => BchFrameSize::Short,
            FrameSize::Normal => BchFrameSize::Normal,
        };

        let bch_code = dvb_t2_bch_code(bch_frame_size, code_rate)
            .expect("every DVB-T2 table row names a constructible BCH code");
        let k_bch = BlockCode::k(&bch_code);
        let k_ldpc = BlockCode::n(&bch_code); // BCH codeword length = LDPC k

        let ldpc_code = match frame_size {
            FrameSize::Normal => LdpcCode::dvb_t2_normal(code_rate),
            FrameSize::Short => LdpcCode::dvb_t2_short(code_rate),
        };
        let n_ldpc = ldpc_code.n();

        debug_assert_eq!(
            k_ldpc,
            ldpc_code.k(),
            "BCH codeword length must equal LDPC k"
        );

        let ldpc_decoder = LdpcDecoder::new(ldpc_code.clone());

        Ok(Self {
            bch_code,
            ldpc_code,
            ldpc_encoder: OnceCell::new(),
            ldpc_decoder: Mutex::new(ldpc_decoder),
            k_bch,
            k_ldpc,
            n_ldpc,
            max_ldpc_iterations: 50,
        })
    }

    /// Size of the BBFRAME (BCH information block) in bits: the length
    /// [`encode`](Self::encode) expects.
    pub fn k_bch(&self) -> usize {
        self.k_bch
    }

    /// LDPC input (= BCH codeword) length in bits.
    pub fn k_ldpc(&self) -> usize {
        self.k_ldpc
    }

    /// FECFRAME length in bits (LDPC codeword length).
    pub fn n_ldpc(&self) -> usize {
        self.n_ldpc
    }

    /// The inner LDPC code (a clone of the configured DVB-T2 LDPC code).
    ///
    /// Exposed so a caller running the LDPC inner decode on a different backend
    /// (e.g. a GPU BP kernel) can build a decoder for the *same* code this codec
    /// encodes with, then finish the concatenated decode via
    /// [`decode_bch_from_ldpc_codeword`](Self::decode_bch_from_ldpc_codeword).
    ///
    /// # Complexity
    ///
    /// O(nnz) for the parity-check-matrix clone.
    #[must_use]
    pub fn ldpc_code(&self) -> LdpcCode {
        self.ldpc_code.clone()
    }

    /// Set maximum belief-propagation iterations (default 50) for each call
    /// to [`decode_soft`](Self::decode_soft).
    ///
    /// # Panics
    ///
    /// Panics if `max_iterations` is zero.
    pub fn set_max_ldpc_iterations(&mut self, max_iterations: usize) {
        assert!(max_iterations > 0, "max_iterations must be positive");
        self.max_ldpc_iterations = max_iterations;
    }

    /// Maximum belief-propagation iterations per [`decode_soft`](Self::decode_soft)
    /// call (default 50; see [`set_max_ldpc_iterations`](Self::set_max_ldpc_iterations)).
    ///
    /// Exposed so an external inner-LDPC decoder (e.g. a GPU LDPC BP stage
    /// paired with [`decode_bch_from_ldpc_codeword`](Self::decode_bch_from_ldpc_codeword))
    /// can run the **same** iteration cap as this codec's own soft decode.
    #[must_use]
    pub fn max_ldpc_iterations(&self) -> usize {
        self.max_ldpc_iterations
    }

    /// Override the LDPC belief-propagation decoder configuration.
    ///
    /// Rebuilds the internal decoder with the supplied [`DecoderConfig`]
    /// in O(nnz). The default decoder is plain
    /// [`DecoderAlgorithm::MinSum`](crate::ldpc::DecoderAlgorithm::MinSum).
    pub fn set_decoder_config(&mut self, config: DecoderConfig) {
        self.ldpc_decoder = Mutex::new(LdpcDecoder::with_config(self.ldpc_code.clone(), config));
    }

    /// Encode a BBFRAME into a FECFRAME (BCH → LDPC).
    ///
    /// The inner [`LdpcEncoder`] is constructed on the first call and cached
    /// in a [`OnceCell`].
    ///
    /// # Panics
    ///
    /// Panics if `bbframe.len() != k_bch()`.
    ///
    /// # Complexity
    ///
    /// The BCH outer encode plus O(nnz) for LDPC; the first call adds O(nnz)
    /// for IRA encoder construction.
    pub fn encode(&self, bbframe: &BitVec) -> BitVec {
        assert_eq!(
            bbframe.len(),
            self.k_bch,
            "BBFRAME length {} must equal k_bch = {}",
            bbframe.len(),
            self.k_bch
        );

        let bch_codeword = BlockEncoder::encode(&self.bch_code, bbframe)
            .expect("a BBFRAME of the validated length encodes");
        debug_assert_eq!(bch_codeword.len(), self.k_ldpc);

        let encoder = self
            .ldpc_encoder
            .get_or_init(|| LdpcEncoder::new(self.ldpc_code.clone()));
        let fecframe = encoder.encode(&bch_codeword);
        debug_assert_eq!(fecframe.len(), self.n_ldpc);

        fecframe
    }

    /// Decode a received FECFRAME LLR sequence (LDPC BP → BCH hard-decision).
    ///
    /// `llrs` holds one channel LLR per FECFRAME bit; a positive LLR means bit
    /// 0 is more likely. The first `k_ldpc` bits of the LDPC hard-decision
    /// codeword form the BCH codeword (DVB-T2 LDPC is systematic with
    /// information bits in positions 0..k_ldpc-1), which BCH hard-decision
    /// decoding corrects to the BBFRAME.
    ///
    /// # Errors
    ///
    /// [`ConcatError::LdpcDecodeFailed`] if LDPC did not converge; its
    /// `bbframe` is a best-effort estimate (BCH-corrected) that may contain
    /// uncorrected errors.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n_ldpc()`.
    ///
    /// # Complexity
    ///
    /// O(max_iterations × nnz) for LDPC, plus the BCH outer decode.
    pub fn decode_soft(&self, llrs: &[Llr]) -> Result<BitVec, ConcatError> {
        self.decode_soft_counted(llrs)
            .map(|(bbframe, _iterations)| bbframe)
    }

    /// As [`decode_soft`](Self::decode_soft), with the success arm also
    /// carrying the number of belief-propagation iterations the LDPC inner
    /// decoder ran.
    ///
    /// # Errors
    ///
    /// [`ConcatError::LdpcDecodeFailed`] if LDPC did not converge within
    /// `max_ldpc_iterations`; `iterations` is then the (capped) BP iteration
    /// count.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n_ldpc()`.
    pub fn decode_soft_counted(&self, llrs: &[Llr]) -> Result<(BitVec, usize), ConcatError> {
        assert_eq!(
            llrs.len(),
            self.n_ldpc,
            "LLR length {} must equal n_ldpc = {}",
            llrs.len(),
            self.n_ldpc
        );

        // `decode_to_codeword` returns all n bits, not just the k message
        // bits; the mutex is held for the duration of BP only.
        let ldpc_result = self
            .ldpc_decoder
            .lock()
            .expect("LDPC decoder mutex poisoned")
            .decode_to_codeword(llrs, self.max_ldpc_iterations);

        let full_codeword = ldpc_result.decoded_bits;
        let converged = ldpc_result.converged;
        let iterations = ldpc_result.iterations;

        let bbframe = self.decode_bch_from_ldpc_codeword(&full_codeword);

        if converged {
            Ok((bbframe, iterations))
        } else {
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            })
        }
    }

    /// BCH-decode a BBFRAME from an already-LDPC-decoded FECFRAME hard codeword.
    ///
    /// This is the outer-decode tail of [`decode_soft_counted`](Self::decode_soft_counted),
    /// for a caller that runs the LDPC inner decode elsewhere (e.g. a GPU
    /// LDPC BP kernel that returns the full `n_ldpc`-bit hard codeword).
    ///
    /// The first `k_ldpc` bits of the codeword are the BCH codeword (DVB-T2 LDPC
    /// is systematic with information bits in positions `0..k_ldpc`); BCH
    /// hard-decision decoding extracts and corrects the `k_bch`-bit BBFRAME.
    ///
    /// # Panics
    ///
    /// Panics if `full_codeword.len() != n_ldpc()`.
    ///
    /// # Complexity
    ///
    /// O(`k_ldpc`) for the systematic extraction, plus one
    /// [`DvbT2BchDecoder`] construction and decode over the mother length.
    pub fn decode_bch_from_ldpc_codeword(&self, full_codeword: &BitVec) -> BitVec {
        assert_eq!(
            full_codeword.len(),
            self.n_ldpc,
            "FECFRAME codeword length {} must equal n_ldpc = {}",
            full_codeword.len(),
            self.n_ldpc
        );

        let mut bch_codeword = BitVec::with_capacity(self.k_ldpc);
        for i in 0..self.k_ldpc {
            bch_codeword.push_bit(full_codeword.get(i));
        }

        let decoder = DvbT2BchDecoder::new(&self.bch_code);
        let (_outcome, bbframe) = decoder
            .decode(&bch_codeword)
            .expect("a BCH codeword of the extracted length decodes");
        debug_assert_eq!(bbframe.len(), self.k_bch);
        bbframe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lengths of `@/citation/Etsi2015` Table 6a for Normal 1/2.
    #[test]
    fn test_normal_rate_1_2_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).expect("construction failed");
        assert_eq!(codec.k_bch(), 32208, "k_bch Normal 1/2");
        assert_eq!(codec.k_ldpc(), 32400, "k_ldpc Normal 1/2");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 1/2");
    }

    /// Lengths of `@/citation/Etsi2015` Table 6a for Normal 2/3.
    #[test]
    fn test_normal_rate_2_3_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate2_3).expect("construction failed");
        assert_eq!(codec.k_bch(), 43040, "k_bch Normal 2/3");
        assert_eq!(codec.k_ldpc(), 43200, "k_ldpc Normal 2/3");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 2/3");
    }

    /// Lengths of `@/citation/Etsi2015` Table 6a for Normal 3/4.
    #[test]
    fn test_normal_rate_3_4_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate3_4).expect("construction failed");
        assert_eq!(codec.k_bch(), 48408, "k_bch Normal 3/4");
        assert_eq!(codec.k_ldpc(), 48600, "k_ldpc Normal 3/4");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 3/4");
    }

    #[test]
    fn test_encode_length_all_three_configs() {
        let configs = [
            (FrameSize::Normal, CodeRate::Rate1_2, 32208usize, 64800usize),
            (FrameSize::Normal, CodeRate::Rate2_3, 43040, 64800),
            (FrameSize::Normal, CodeRate::Rate3_4, 48408, 64800),
        ];

        for (frame_size, rate, expected_k_bch, expected_n_ldpc) in configs {
            let codec = DvbT2Concat::new(frame_size, rate).expect("construction failed");
            assert_eq!(
                codec.k_bch(),
                expected_k_bch,
                "k_bch mismatch for {:?}",
                rate
            );
            assert_eq!(
                codec.n_ldpc(),
                expected_n_ldpc,
                "n_ldpc mismatch for {:?}",
                rate
            );
        }
    }

    #[test]
    fn test_set_decoder_config_rebuilds_decoder() {
        use crate::ldpc::DecoderAlgorithm;

        let mut codec =
            DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).expect("construction failed");

        // High-magnitude positive LLRs signal the all-zero codeword (no noise).
        let llrs: Vec<Llr> = vec![Llr::new(10.0); codec.n_ldpc()];
        let zero_bbframe = BitVec::zeros(codec.k_bch());

        let bbframe_default = codec.decode_soft(&llrs).expect("default decode failed");
        assert_eq!(
            bbframe_default, zero_bbframe,
            "default MinSum did not converge to zero codeword on clean LLRs"
        );

        codec.set_decoder_config(DecoderConfig::new(
            DecoderAlgorithm::NormalizedMinSum(0.75),
            true,
        ));
        let bbframe_nms = codec.decode_soft(&llrs).expect("NMS decode failed");
        assert_eq!(
            bbframe_nms, zero_bbframe,
            "NMS(0.75) did not converge to zero codeword after set_decoder_config"
        );

        codec.set_decoder_config(DecoderConfig::new(DecoderAlgorithm::SumProduct, true));
        let bbframe_spa = codec.decode_soft(&llrs).expect("SPA decode failed");
        assert_eq!(
            bbframe_spa, zero_bbframe,
            "SumProduct did not converge to zero codeword after set_decoder_config"
        );
    }

    #[test]
    fn test_roundtrip_normal_rate_1_2() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).expect("construction failed");

        let mut bbframe_in = BitVec::with_capacity(codec.k_bch());
        for i in 0..codec.k_bch() {
            bbframe_in.push_bit(i % 3 == 0);
        }

        let fecframe = codec.encode(&bbframe_in);
        assert_eq!(fecframe.len(), codec.n_ldpc(), "FECFRAME length mismatch");

        // Zero-noise: LLR = +10.0 for 0, -10.0 for 1.
        let llrs: Vec<Llr> = (0..fecframe.len())
            .map(|i| {
                if fecframe.get(i) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let bbframe_out = codec.decode_soft(&llrs).expect("LDPC decode failed");
        assert_eq!(bbframe_out, bbframe_in, "Roundtrip mismatch for Normal 1/2");
    }

    #[test]
    fn test_roundtrip_normal_rate_2_3() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate2_3).expect("construction failed");

        let mut bbframe_in = BitVec::with_capacity(codec.k_bch());
        for i in 0..codec.k_bch() {
            bbframe_in.push_bit(i % 5 == 1);
        }

        let fecframe = codec.encode(&bbframe_in);
        assert_eq!(fecframe.len(), codec.n_ldpc());

        let llrs: Vec<Llr> = (0..fecframe.len())
            .map(|i| {
                if fecframe.get(i) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let bbframe_out = codec.decode_soft(&llrs).expect("LDPC decode failed");
        assert_eq!(bbframe_out, bbframe_in, "Roundtrip mismatch for Normal 2/3");
    }

    #[test]
    fn test_roundtrip_normal_rate_3_4() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate3_4).expect("construction failed");

        let mut bbframe_in = BitVec::with_capacity(codec.k_bch());
        for i in 0..codec.k_bch() {
            bbframe_in.push_bit(i % 7 == 2);
        }

        let fecframe = codec.encode(&bbframe_in);
        assert_eq!(fecframe.len(), codec.n_ldpc());

        let llrs: Vec<Llr> = (0..fecframe.len())
            .map(|i| {
                if fecframe.get(i) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let bbframe_out = codec.decode_soft(&llrs).expect("LDPC decode failed");
        assert_eq!(bbframe_out, bbframe_in, "Roundtrip mismatch for Normal 3/4");
    }

    /// TP04→TP06 chain via concat API with the `@/citation/DvbVerification2010`
    /// vectors.
    ///
    /// Reads VV001-CR35 (Normal, Rate 3/5) TP04 and TP06 vectors and verifies
    /// that [`DvbT2Concat::encode`] reproduces TP06 from TP04 for the first
    /// block of the first frame.
    #[test]
    #[ignore = "slow: reads the VV001-CR35 reference stream from $DVB_TEST_VECTORS_PATH"]
    fn test_tp04_to_tp06_via_concat() {
        use crate::test_support::{dvb_vectors_path, parse_tp_blocks, tp_path};

        let config_dir = dvb_vectors_path().join("VV001-CR35_CSP");
        if !config_dir.exists() {
            eprintln!("Test vectors not found at {:?}, skipping", config_dir);
            return;
        }

        let tp04_blocks = parse_tp_blocks(&tp_path(&config_dir, "04"));
        let tp06_blocks = parse_tp_blocks(&tp_path(&config_dir, "06"));

        assert!(!tp04_blocks.is_empty(), "TP04 parse produced no blocks");
        assert_eq!(
            tp04_blocks.len(),
            tp06_blocks.len(),
            "TP04 and TP06 block counts must match"
        );

        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate3_5).expect("construction failed");

        let tp04_block = &tp04_blocks[0];
        let tp06_block = &tp06_blocks[0];

        assert_eq!(
            tp04_block.len(),
            codec.k_bch(),
            "TP04 block length must equal k_bch"
        );
        assert_eq!(
            tp06_block.len(),
            codec.n_ldpc(),
            "TP06 block length must equal n_ldpc"
        );

        let fecframe = codec.encode(tp04_block);
        assert_eq!(
            fecframe, *tp06_block,
            "TP04 to TP06 encoding mismatch via DvbT2Concat"
        );
    }
}
