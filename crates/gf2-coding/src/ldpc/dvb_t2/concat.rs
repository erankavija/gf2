//! DVB-T2 BCH+LDPC concatenated codec.
//!
//! This module implements the full DVB-T2 FECFRAME encoding and decoding chain
//! as specified in ETSI EN 302 755 v1.4.1 §6. A single [`DvbT2Concat`] instance
//! wraps the BCH outer code and LDPC inner code, exposing one encode and one
//! decode entry point.
//!
//! # Encoding chain (§6.1)
//!
//! ```text
//! BBFRAME (k_bch bits)
//!   → BCH encode → LDPC input (k_ldpc = k_bch + parity bits)
//!   → LDPC encode → FECFRAME (N bits, ready for bit interleaver)
//! ```
//!
//! # Decoding chain
//!
//! ```text
//! Received LLRs (N soft values)
//!   → LDPC belief propagation → full N-bit codeword
//!   → extract first k_ldpc bits (BCH codeword)
//!   → BCH hard-decision decode → BBFRAME (k_bch bits)
//! ```
//!
//! # Supported configurations
//!
//! All twelve DVB-T2 configurations (both frame sizes × six rates) are
//! constructible. The three Normal-frame in-scope configurations (1/2, 2/3,
//! 3/4) are fully tested with zero-noise roundtrips.
//!
//! # Example
//!
//! ```
//! use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
//! use gf2_coding::CodeRate;
//!
//! let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2)
//!     .expect("unsupported configuration");
//! assert_eq!(codec.k_bch(), 32208);  // BBFRAME size
//! assert_eq!(codec.k_ldpc(), 32400); // LDPC input = BCH codeword
//! assert_eq!(codec.n_ldpc(), 64800); // FECFRAME size
//! ```

use crate::bch::{BchCode, BchDecoder, BchEncoder};
use crate::ldpc::{DecoderConfig, LdpcCode, LdpcDecoder, LdpcEncoder};
use crate::llr::Llr;
use crate::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;
use once_cell::sync::OnceCell;
use std::sync::Mutex;

use super::FrameSize;
use crate::bch::CodeRate;

// Bring in BCH FrameSize under a distinct alias to avoid ambiguity.
use crate::bch::dvb_t2::FrameSize as BchFrameSize;

/// Error type returned by [`DvbT2Concat::new`] and [`DvbT2Concat::decode_soft`].
///
/// Variants cover the two failure modes: an unsupported (frame_size, code_rate)
/// pair at construction time, and LDPC convergence failure at decode time.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::dvb_t2::{concat::{DvbT2Concat, ConcatError}, FrameSize};
/// use gf2_coding::CodeRate;
///
/// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
/// assert_eq!(codec.k_bch(), 32208);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcatError {
    /// The (frame_size, code_rate) pair is not covered by this implementation.
    ///
    /// # Fields
    ///
    /// * `frame_size` — The requested [`FrameSize`].
    /// * `code_rate`  — The requested [`CodeRate`].
    Unsupported {
        /// Requested frame size.
        frame_size: FrameSize,
        /// Requested code rate.
        code_rate: CodeRate,
    },
    /// LDPC belief propagation did not converge; the BCH codeword may contain
    /// residual errors. The partially-decoded BBFRAME is returned as the
    /// payload.
    ///
    /// # Fields
    ///
    /// * `bbframe`    — Best estimate of the BBFRAME (BCH-corrected where possible).
    /// * `iterations` — Number of BP iterations performed before giving up.
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

/// DVB-T2 concatenated BCH + LDPC FEC codec.
///
/// Implements the ETSI EN 302 755 §6 encode/decode chain in a single object.
/// Construct with [`DvbT2Concat::new`]; then call [`DvbT2Concat::encode`] or
/// [`DvbT2Concat::decode_soft`].
///
/// The LDPC encoder is initialised lazily on the first call to
/// [`encode`](Self::encode) (stored in a [`OnceCell`]) to avoid the O(n²/64)
/// preprocessing cost at construction time. The LDPC decoder is wrapped in a
/// [`Mutex`] so that `decode_soft` takes `&self` while still allowing BP
/// scratch-buffer mutation; this also makes `DvbT2Concat` [`Sync`].
///
/// # Arguments to `new`
///
/// * `frame_size` — [`FrameSize::Normal`] (n=64800) or [`FrameSize::Short`] (n=16200).
/// * `code_rate`  — One of the six DVB-T2 rates.
///
/// # Complexity
///
/// - Construction: O(nnz) for decoder graph allocation; O(1) for all other
///   fields.
/// - First call to `encode`: O(n²/64) for LDPC RU preprocessing. Cached
///   inside the encoder for subsequent calls.
/// - Subsequent `encode` calls: O(nnz).
/// - `decode_soft`: O(max_iterations × nnz) + O(k_ldpc) for BCH.
pub struct DvbT2Concat {
    /// BCH encoder.
    bch_encoder: BchEncoder,
    /// BCH decoder.
    bch_decoder: BchDecoder,
    /// LDPC code (held to construct the encoder lazily).
    ldpc_code: LdpcCode,
    /// LDPC encoder (Richardson-Urbanke), initialised on first encode call.
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
    /// Maximum BP iterations for LDPC decoding (50 is the DVB-T2 default).
    max_ldpc_iterations: usize,
}

impl DvbT2Concat {
    /// Construct a DVB-T2 concatenated codec.
    ///
    /// All twelve DVB-T2 configurations (both frame sizes × six rates) are
    /// constructible. The `Err(Unsupported)` variant is reserved for future
    /// use when restricting to a strict subset.
    ///
    /// The LDPC encoder is **not** initialised here; it is created lazily on
    /// the first call to [`encode`](Self::encode).
    ///
    /// # Arguments
    ///
    /// * `frame_size` — [`FrameSize::Normal`] or [`FrameSize::Short`]
    /// * `code_rate`  — DVB-T2 code rate
    ///
    /// # Returns
    ///
    /// `Ok(Self)` for every valid DVB-T2 (frame_size, code_rate) pair.
    /// `Err(ConcatError::Unsupported)` is reserved for future use.
    ///
    /// # Panics
    ///
    /// Panics if the BCH generator polynomial cannot be constructed for the
    /// specified parameters (indicates an invariant violation).
    ///
    /// # Complexity
    ///
    /// O(nnz) for decoder graph allocation; encoder preprocessing deferred
    /// to first [`encode`](Self::encode) call.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2)
    ///     .expect("unsupported configuration");
    /// assert_eq!(codec.n_ldpc(), 64800);
    /// ```
    pub fn new(frame_size: FrameSize, code_rate: CodeRate) -> Result<Self, ConcatError> {
        // Map LDPC FrameSize → BCH FrameSize (same logical enum, separate types).
        let bch_frame_size = match frame_size {
            FrameSize::Short => BchFrameSize::Short,
            FrameSize::Normal => BchFrameSize::Normal,
        };

        let bch_code = BchCode::dvb_t2(bch_frame_size, code_rate);
        let k_bch = bch_code.k();
        let k_ldpc = bch_code.n(); // BCH codeword length = LDPC k

        let ldpc_code = match frame_size {
            FrameSize::Normal => LdpcCode::dvb_t2_normal(code_rate),
            FrameSize::Short => LdpcCode::dvb_t2_short(code_rate),
        };
        let n_ldpc = ldpc_code.n();

        // Sanity-check the BCH/LDPC join point: BCH n == LDPC k.
        debug_assert_eq!(
            k_ldpc,
            ldpc_code.k(),
            "BCH codeword length must equal LDPC k"
        );

        let bch_encoder = BchEncoder::new(bch_code.clone());
        let bch_decoder = BchDecoder::new(bch_code);
        let ldpc_decoder = LdpcDecoder::new(ldpc_code.clone());

        Ok(Self {
            bch_encoder,
            bch_decoder,
            ldpc_code,
            ldpc_encoder: OnceCell::new(),
            ldpc_decoder: Mutex::new(ldpc_decoder),
            k_bch,
            k_ldpc,
            n_ldpc,
            max_ldpc_iterations: 50,
        })
    }

    /// Size of the BBFRAME (BCH information block) in bits.
    ///
    /// This is the expected length of the `bbframe` argument passed to
    /// [`encode`](Self::encode).
    ///
    /// # Arguments
    ///
    /// * `&self` — The codec instance.
    ///
    /// # Returns
    ///
    /// Number of BBFRAME bits (equals `k` of the BCH code for this
    /// configuration).
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Complexity
    ///
    /// O(1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// assert_eq!(codec.k_bch(), 32208);
    /// ```
    pub fn k_bch(&self) -> usize {
        self.k_bch
    }

    /// LDPC input (= BCH codeword) length in bits.
    ///
    /// Equals `k_bch + BCH_parity_bits` (192 for Normal frames, 160 for Short
    /// frames).
    ///
    /// # Arguments
    ///
    /// * `&self` — The codec instance.
    ///
    /// # Returns
    ///
    /// Number of LDPC input bits (equals `n` of the BCH code = `k` of the
    /// LDPC code for this configuration).
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Complexity
    ///
    /// O(1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// assert_eq!(codec.k_ldpc(), 32400);
    /// ```
    pub fn k_ldpc(&self) -> usize {
        self.k_ldpc
    }

    /// FECFRAME length in bits (LDPC codeword length).
    ///
    /// 64800 for Normal frames, 16200 for Short frames.
    ///
    /// # Arguments
    ///
    /// * `&self` — The codec instance.
    ///
    /// # Returns
    ///
    /// Number of FECFRAME bits (equals `n` of the LDPC code for this
    /// configuration).
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Complexity
    ///
    /// O(1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// assert_eq!(codec.n_ldpc(), 64800);
    /// ```
    pub fn n_ldpc(&self) -> usize {
        self.n_ldpc
    }

    /// Set maximum belief-propagation iterations (default 50).
    ///
    /// # Arguments
    ///
    /// * `&mut self`     — The codec instance (mutable because the iteration
    ///   limit is stored inside the struct).
    /// * `max_iterations` — Maximum BP iterations for each call to
    ///   [`decode_soft`](Self::decode_soft). Must be ≥ 1.
    ///
    /// # Panics
    ///
    /// Panics if `max_iterations` is zero.
    ///
    /// # Complexity
    ///
    /// O(1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let mut codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// codec.set_max_ldpc_iterations(100);
    /// ```
    pub fn set_max_ldpc_iterations(&mut self, max_iterations: usize) {
        assert!(max_iterations > 0, "max_iterations must be positive");
        self.max_ldpc_iterations = max_iterations;
    }

    /// Override the LDPC belief-propagation decoder configuration.
    ///
    /// Rebuilds the internal decoder with the supplied [`DecoderConfig`]
    /// (algorithm + early-termination policy). The default decoder is plain
    /// [`DecoderAlgorithm::MinSum`](crate::ldpc::DecoderAlgorithm::MinSum);
    /// selecting `NormalizedMinSum` or `SumProduct` trades decode throughput
    /// for additional coding gain.
    ///
    /// # Arguments
    ///
    /// * `&mut self` — The codec instance (mutable: the decoder is rebuilt).
    /// * `config`    — Replacement [`DecoderConfig`].
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Complexity
    ///
    /// O(nnz) for decoder graph reallocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::CodeRate;
    ///
    /// let mut codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// codec.set_decoder_config(DecoderConfig::new(
    ///     DecoderAlgorithm::NormalizedMinSum(0.75),
    ///     true,
    /// ));
    /// ```
    pub fn set_decoder_config(&mut self, config: DecoderConfig) {
        self.ldpc_decoder = Mutex::new(LdpcDecoder::with_config(self.ldpc_code.clone(), config));
    }

    /// Encode a BBFRAME into a FECFRAME (BCH → LDPC).
    ///
    /// Applies BCH outer encoding followed by LDPC inner encoding, producing
    /// a FECFRAME ready for the bit interleaver and constellation mapper.
    ///
    /// On the first call the LDPC RU encoding matrices are preprocessed and
    /// cached in a [`OnceCell`]; subsequent calls use the cached result without
    /// any synchronisation overhead.
    ///
    /// # Arguments
    ///
    /// * `&self`    — The codec instance (shared reference; interior mutability
    ///   handles lazy encoder initialisation via [`OnceCell`]).
    /// * `bbframe`  — Information bits; must be exactly `k_bch()` bits long.
    ///
    /// # Returns
    ///
    /// FECFRAME codeword (`n_ldpc` bits — 64800 for Normal, 16200 for Short).
    ///
    /// # Panics
    ///
    /// Panics if `bbframe.len() != k_bch()`.
    ///
    /// # Complexity
    ///
    /// First call: O(n²/64) for encoder preprocessing + O(k_bch) BCH +
    /// O(nnz) LDPC.
    /// Subsequent calls: O(k_bch) BCH + O(nnz) LDPC.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::CodeRate;
    /// use gf2_core::BitVec;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// let bbframe = BitVec::zeros(codec.k_bch());
    /// let fecframe = codec.encode(&bbframe);
    /// assert_eq!(fecframe.len(), codec.n_ldpc());
    /// ```
    pub fn encode(&self, bbframe: &BitVec) -> BitVec {
        assert_eq!(
            bbframe.len(),
            self.k_bch,
            "BBFRAME length {} must equal k_bch = {}",
            bbframe.len(),
            self.k_bch
        );

        // Step 1: BCH outer encode — k_bch → k_ldpc bits.
        let bch_codeword = self.bch_encoder.encode(bbframe);
        debug_assert_eq!(bch_codeword.len(), self.k_ldpc);

        // Step 2: LDPC inner encode — k_ldpc → n_ldpc bits.
        // Initialises the encoder lazily on the first call via OnceCell.
        let encoder = self
            .ldpc_encoder
            .get_or_init(|| LdpcEncoder::new(self.ldpc_code.clone()));
        let fecframe = encoder.encode(&bch_codeword);
        debug_assert_eq!(fecframe.len(), self.n_ldpc);

        fecframe
    }

    /// Decode a received FECFRAME LLR sequence (LDPC BP → BCH hard-decision).
    ///
    /// LDPC belief propagation runs first (soft input), decoding the full
    /// FECFRAME codeword. The first `k_ldpc` bits of the hard-decided codeword
    /// form the BCH codeword (DVB-T2 LDPC is systematic with information bits
    /// in positions 0..k_ldpc-1). BCH hard-decision decoding then extracts and
    /// corrects the BBFRAME.
    ///
    /// The LDPC decoder is wrapped in a [`Mutex`] so this method takes a shared
    /// reference; the lock is held only for the duration of the BP iterations.
    ///
    /// # Arguments
    ///
    /// * `&self` — The codec instance (shared reference).
    /// * `llrs`  — Channel LLRs, one per FECFRAME bit (`n_ldpc` values).
    ///   Positive LLR → more likely 0; negative LLR → more likely 1.
    ///
    /// # Returns
    ///
    /// * `Ok(bbframe)` — BBFRAME (`k_bch` bits) when LDPC converged.
    /// * `Err(ConcatError::LdpcDecodeFailed { bbframe, iterations })` — LDPC
    ///   did not converge; the returned `bbframe` is a best-effort estimate
    ///   (BCH-corrected) but may contain uncorrected errors.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n_ldpc()`.
    ///
    /// # Complexity
    ///
    /// O(max_iterations × nnz) for LDPC + O(k_ldpc) for BCH.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::CodeRate;
    /// use gf2_core::BitVec;
    ///
    /// let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
    /// // Zero-noise LLRs for the all-zeros FECFRAME:
    /// let llrs: Vec<Llr> = vec![Llr::new(10.0); codec.n_ldpc()];
    /// let bbframe = codec.decode_soft(&llrs).unwrap();
    /// assert_eq!(bbframe.len(), codec.k_bch());
    /// ```
    pub fn decode_soft(&self, llrs: &[Llr]) -> Result<BitVec, ConcatError> {
        assert_eq!(
            llrs.len(),
            self.n_ldpc,
            "LLR length {} must equal n_ldpc = {}",
            llrs.len(),
            self.n_ldpc
        );

        // Step 1: LDPC inner decode — produce the full n_ldpc-bit codeword.
        // `decode_to_codeword` runs BP and returns all n bits (not just the
        // k message bits), so we can extract the BCH codeword directly.
        // Acquire the mutex for the duration of BP only.
        let ldpc_result = self
            .ldpc_decoder
            .lock()
            .expect("LDPC decoder mutex poisoned")
            .decode_to_codeword(llrs, self.max_ldpc_iterations);

        let full_codeword = ldpc_result.decoded_bits;
        let converged = ldpc_result.converged;
        let iterations = ldpc_result.iterations;

        // Step 2: Extract BCH codeword from systematic positions 0..k_ldpc-1.
        // DVB-T2 LDPC uses the natural systematic convention: information bits
        // occupy codeword positions [0, k_ldpc), parity in [k_ldpc, n_ldpc).
        let mut bch_codeword = BitVec::with_capacity(self.k_ldpc);
        for i in 0..self.k_ldpc {
            bch_codeword.push_bit(full_codeword.get(i));
        }

        // Step 3: BCH outer decode — k_ldpc → k_bch bits.
        let bbframe = self.bch_decoder.decode(&bch_codeword);
        debug_assert_eq!(bbframe.len(), self.k_bch);

        if converged {
            Ok(bbframe)
        } else {
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Dimension tests — do NOT call encode() or decode_soft(); they only
    // verify parameter tables via DvbT2Concat::new(), which is O(nnz) for
    // the decoder graph and O(1) for everything else. These run in the fast
    // CI tier (well under 5 s).
    // -----------------------------------------------------------------------

    /// Verify lengths match EN 302 755 Table 6a for Normal 1/2.
    #[test]
    fn test_normal_rate_1_2_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).expect("construction failed");
        assert_eq!(codec.k_bch(), 32208, "k_bch Normal 1/2");
        assert_eq!(codec.k_ldpc(), 32400, "k_ldpc Normal 1/2");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 1/2");
    }

    /// Verify lengths match EN 302 755 Table 6a for Normal 2/3.
    #[test]
    fn test_normal_rate_2_3_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate2_3).expect("construction failed");
        assert_eq!(codec.k_bch(), 43040, "k_bch Normal 2/3");
        assert_eq!(codec.k_ldpc(), 43200, "k_ldpc Normal 2/3");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 2/3");
    }

    /// Verify lengths match EN 302 755 Table 6a for Normal 3/4.
    #[test]
    fn test_normal_rate_3_4_lengths() {
        let codec =
            DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate3_4).expect("construction failed");
        assert_eq!(codec.k_bch(), 48408, "k_bch Normal 3/4");
        assert_eq!(codec.k_ldpc(), 48600, "k_ldpc Normal 3/4");
        assert_eq!(codec.n_ldpc(), 64800, "n_ldpc Normal 3/4");
    }

    /// Verify FECFRAME lengths for all three in-scope configurations
    /// (EN 302 755 Table 6a assertions; no encode/decode performed).
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

    /// Verify [`DvbT2Concat::set_decoder_config`] rebuilds the internal LDPC
    /// belief-propagation decoder with the supplied algorithm and that
    /// decoding still recovers a zero-noise codeword afterward.
    ///
    /// Uses Short frame 1/2 with manually-constructed clean LLRs to stay in
    /// the fast tier — no encode() call, so no RREF preprocessing runs.
    #[test]
    fn test_set_decoder_config_rebuilds_decoder() {
        use crate::ldpc::DecoderAlgorithm;

        let mut codec =
            DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).expect("construction failed");

        // High-magnitude positive LLRs signal the all-zero codeword (no noise).
        let llrs: Vec<Llr> = vec![Llr::new(10.0); codec.n_ldpc()];
        let zero_bbframe = BitVec::zeros(codec.k_bch());

        // Default decoder (MinSum) converges to the all-zero BBFRAME.
        let bbframe_default = codec.decode_soft(&llrs).expect("default decode failed");
        assert_eq!(
            bbframe_default, zero_bbframe,
            "default MinSum did not converge to zero codeword on clean LLRs"
        );

        // Swap in normalized min-sum and re-decode.
        codec.set_decoder_config(DecoderConfig::new(
            DecoderAlgorithm::NormalizedMinSum(0.75),
            true,
        ));
        let bbframe_nms = codec.decode_soft(&llrs).expect("NMS decode failed");
        assert_eq!(
            bbframe_nms, zero_bbframe,
            "NMS(0.75) did not converge to zero codeword after set_decoder_config"
        );

        // Swap to sum-product and verify the decoder still functions.
        codec.set_decoder_config(DecoderConfig::new(DecoderAlgorithm::SumProduct, true));
        let bbframe_spa = codec.decode_soft(&llrs).expect("SPA decode failed");
        assert_eq!(
            bbframe_spa, zero_bbframe,
            "SumProduct did not converge to zero codeword after set_decoder_config"
        );
    }

    // -----------------------------------------------------------------------
    // Roundtrip tests — call encode() + decode_soft(); the first encode()
    // triggers LdpcEncoder::new() which preprocesses the RU encoding matrices
    // (O(n²/64) ≈ 2-10 s for DVB-T2 Normal frame). Marked slow accordingly.
    // -----------------------------------------------------------------------

    /// Zero-noise roundtrip: Normal frame 1/2, pseudo-random payload.
    #[test]
    #[ignore = "slow: LdpcEncoder::new for Normal frame takes 2-10 s"]
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

    /// Zero-noise roundtrip: Normal frame 2/3, pseudo-random payload.
    #[test]
    #[ignore = "slow: LdpcEncoder::new for Normal frame takes 2-10 s"]
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

    /// Zero-noise roundtrip: Normal frame 3/4, pseudo-random payload.
    #[test]
    #[ignore = "slow: LdpcEncoder::new for Normal frame takes 2-10 s"]
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

    /// TP04→TP06 chain via concat API with external test vectors.
    ///
    /// Reads VV001-CR35 (Normal, Rate 3/5) TP04 and TP06 vectors and verifies
    /// that [`DvbT2Concat::encode`] reproduces TP06 from TP04 for the first
    /// block of the first frame.
    #[test]
    #[ignore = "external: requires DVB-T2 test vectors at $DVB_TEST_VECTORS_PATH or ~/dvb_test_vectors"]
    fn test_tp04_to_tp06_via_concat() {
        use std::path::PathBuf;

        // Locate test vectors.
        let base_path = std::env::var("DVB_TEST_VECTORS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").expect("HOME not set")).join("dvb_test_vectors")
            });

        let config_dir = base_path.join("VV001-CR35_CSP");
        if !config_dir.exists() {
            eprintln!("Test vectors not found at {:?}, skipping", config_dir);
            return;
        }

        use crate::test_support::{parse_tp_blocks, tp_path};

        let tp04_blocks = parse_tp_blocks(&tp_path(&config_dir, "04"));
        let tp06_blocks = parse_tp_blocks(&tp_path(&config_dir, "06"));

        assert!(!tp04_blocks.is_empty(), "TP04 parse produced no blocks");
        assert_eq!(
            tp04_blocks.len(),
            tp06_blocks.len(),
            "TP04 and TP06 block counts must match"
        );

        // VV001-CR35 = Normal frame, Rate 3/5.
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
