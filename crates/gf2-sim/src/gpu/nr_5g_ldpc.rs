//! 5G NR rate-matched decoding on the GPU LDPC BP kernel (`feature = "hip"`).
//!
//! The mother code is the `gf2-coding` expansion
//! ([`QuasiCyclicLdpc::nr_5g_rate_matched`](gf2_coding::ldpc::QuasiCyclicLdpc::nr_5g_rate_matched))
//! of the base graph and per-`i_LS` shift table, flattened by `GpuLdpcBp` as
//! for any [`LdpcCode`](gf2_coding::ldpc::LdpcCode). Rate matching
//! (`@/citation/ThreeGpp2017` Section 5.3.2) is a host-side map: the `target_n`
//! channel LLRs become the `full_n` mother-code LLR vector
//! ([`Nr5gRateMatchedCode::prepare_llrs`](gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode::prepare_llrs)),
//! the device decodes the full mother codeword, and the `target_k` message bits
//! are extracted in natural column order.

#[cfg(feature = "hip")]
mod imp {
    use std::sync::Arc;

    use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
    use gf2_coding::ldpc::{DecoderConfig, LdpcDecoder};
    use gf2_coding::Llr;
    use gf2_core::BitVec;
    use gf2_kernels_hip::GpuLdpcBp as KernelGpuLdpcBp;

    use crate::batch::{HardDecisionBatch, LlrBatch};
    use crate::error::StageError;
    use crate::gpu::ldpc_bp::GpuLdpcBp;

    /// A device LDPC BP decoder for a rate-matched 5G NR code: a [`GpuLdpcBp`]
    /// over the mother code plus the host-side rate-matching maps.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, QuasiCyclicLdpc};
    ///
    /// // BG1, i_LS = 1 (Z = 384), rate 1/2 — the headline configuration.
    /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
    /// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    /// // Constructing the wrapper does not touch the GPU; decoding does.
    /// let dec = GpuNr5gDecoder::new(code, config, 25);
    /// assert_eq!(dec.target_k(), 8448);
    /// ```
    pub struct GpuNr5gDecoder {
        code: Arc<Nr5gRateMatchedCode>,
        /// GPU LDPC BP stage over the mother code.
        gpu: GpuLdpcBp,
        config: DecoderConfig,
        max_iterations: usize,
    }

    impl GpuNr5gDecoder {
        /// Builds a GPU 5G NR decoder over the given rate-matched code.
        ///
        /// # Panics
        ///
        /// Panics if `max_iterations == 0`.
        #[must_use]
        pub fn new(
            code: Arc<Nr5gRateMatchedCode>,
            config: DecoderConfig,
            max_iterations: usize,
        ) -> Self {
            let gpu = GpuLdpcBp::new(code.mother_code().clone(), config, max_iterations);
            Self {
                code,
                gpu,
                config,
                max_iterations,
            }
        }

        /// The recovered message length `target_k`.
        #[inline]
        #[must_use]
        pub fn target_k(&self) -> usize {
            self.code.params().target_k
        }

        /// The transmitted codeword length `target_n` (the rate-matched `E`).
        #[inline]
        #[must_use]
        pub fn target_n(&self) -> usize {
            self.code.params().target_n
        }

        /// The full mother-code length `full_n = N_b * Z`.
        #[inline]
        #[must_use]
        pub fn full_n(&self) -> usize {
            self.code.params().full_n
        }

        /// The BP iteration cap.
        #[inline]
        #[must_use]
        pub fn max_iterations(&self) -> usize {
            self.max_iterations
        }

        /// The decoder configuration (algorithm + early termination).
        #[inline]
        #[must_use]
        pub fn config(&self) -> DecoderConfig {
            self.config
        }

        /// Borrows the underlying mother-code [`GpuLdpcBp`] stage.
        #[inline]
        #[must_use]
        pub fn gpu(&self) -> &GpuLdpcBp {
            &self.gpu
        }

        /// Builds a per-worker device decoder sized for up to `max_batch`
        /// frames, delegating to the inner [`GpuLdpcBp::build_decoder`].
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] if the device allocation or graph upload
        /// fails.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use std::sync::Arc;
        /// use gf2_coding::ldpc::{DecoderConfig, DecoderAlgorithm, QuasiCyclicLdpc};
        /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
        ///
        /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
        /// let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        /// let dec = GpuNr5gDecoder::new(code, cfg, 20);
        /// let device = dec.build_decoder(128)?;
        /// # Ok::<(), gf2_sim::error::StageError>(())
        /// ```
        pub fn build_decoder(&self, max_batch: usize) -> Result<KernelGpuLdpcBp, StageError> {
            self.gpu.build_decoder(max_batch)
        }

        /// Maps one frame's `target_n` channel LLRs to the `full_n` mother-code
        /// LLR vector via [`Nr5gRateMatchedCode::prepare_llrs`].
        ///
        /// # Panics
        ///
        /// Panics if `channel_llrs.len() != target_n`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use std::sync::Arc;
        /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
        /// use gf2_coding::ldpc::{DecoderConfig, DecoderAlgorithm, QuasiCyclicLdpc};
        /// use gf2_coding::llr::Llr;
        /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
        ///
        /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
        /// let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        /// let dec = GpuNr5gDecoder::new(code, cfg, 20);
        /// let channel = vec![Llr::new(4.0); 16896];
        /// let full = dec.prepare_llrs(&channel);
        /// assert_eq!(full.len(), dec.full_n());
        /// ```
        #[must_use]
        pub fn prepare_llrs(&self, channel_llrs: &[Llr]) -> Vec<Llr> {
            self.code.prepare_llrs(channel_llrs)
        }

        /// Decodes a batch of `target_n`-length channel-LLR frames to recovered
        /// `target_k`-bit messages on the device.
        ///
        /// Composes [`prepare_llrs`](Self::prepare_llrs), the device decode of
        /// the full mother codeword, and
        /// [`extract_message`](Self::extract_message). `decoder` comes from
        /// [`build_decoder`](Self::build_decoder).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault.
        ///
        /// # Panics
        ///
        /// Panics if any frame's LLR length != `target_n`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use std::sync::Arc;
        /// use gf2_coding::ldpc::{DecoderConfig, DecoderAlgorithm, QuasiCyclicLdpc};
        /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
        ///
        /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
        /// let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        /// let dec = GpuNr5gDecoder::new(code, cfg, 20);
        /// # use gf2_coding::llr::Llr;
        /// # use gf2_sim::LlrBatch;
        /// let device = dec.build_decoder(128)?;
        /// let batch = LlrBatch::new(vec![vec![Llr::new(4.0); 16896]; 8]);
        /// let recovered = dec.decode_batch(&batch, &device)?;
        /// assert_eq!(recovered.frames.len(), 8);
        /// # Ok::<(), gf2_sim::error::StageError>(())
        /// ```
        ///
        /// # Complexity
        ///
        /// O(`max_iterations * batch * edges`) device work plus the per-call
        /// H2D / D2H transfers, where `edges` is over the full mother code.
        pub fn decode_batch(
            &self,
            input: &LlrBatch,
            decoder: &KernelGpuLdpcBp,
        ) -> Result<HardDecisionBatch, StageError> {
            let prepared: Vec<Vec<Llr>> = input
                .frames
                .iter()
                .map(|frame| self.prepare_llrs(frame))
                .collect();
            let mother = self.gpu.decode_batch(&LlrBatch::new(prepared), decoder)?;
            let frames: Vec<BitVec> = mother
                .frames
                .iter()
                .map(|cw| self.extract_message(cw))
                .collect();
            Ok(HardDecisionBatch::new(frames))
        }

        /// Extracts the `target_k` message bits from a full mother codeword in
        /// natural column order (positions `0..target_k`).
        ///
        /// `@/citation/ThreeGpp2017` places message bit `i` at codeword position
        /// `i`, so extraction is a prefix slice.
        ///
        /// # Panics
        ///
        /// Panics if `mother_codeword.len() < target_k`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use std::sync::Arc;
        /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
        /// use gf2_coding::ldpc::{DecoderConfig, DecoderAlgorithm, QuasiCyclicLdpc};
        /// use gf2_coding::llr::Llr;
        /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
        ///
        /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
        /// let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        /// let dec = GpuNr5gDecoder::new(code, cfg, 20);
        /// # use gf2_core::BitVec;
        /// let mother = BitVec::zeros(dec.full_n());
        /// let msg = dec.extract_message(&mother);
        /// assert_eq!(msg.len(), dec.target_k());
        /// ```
        #[must_use]
        pub fn extract_message(&self, mother_codeword: &BitVec) -> BitVec {
            let target_k = self.code.params().target_k;
            let mut msg = BitVec::with_capacity(target_k);
            for i in 0..target_k {
                msg.push_bit(mother_codeword.get(i));
            }
            msg
        }

        /// The CPU reference recovered message for one frame's `target_n`
        /// channel LLRs: the CPU mother-code
        /// [`LdpcDecoder::decode_to_codeword`] on the prepared LLRs, followed
        /// by [`extract_message`](Self::extract_message).
        ///
        /// # Panics
        ///
        /// Panics if `channel_llrs.len() != target_n`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use std::sync::Arc;
        /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
        /// use gf2_coding::ldpc::{DecoderConfig, DecoderAlgorithm, QuasiCyclicLdpc};
        /// use gf2_coding::llr::Llr;
        /// use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
        ///
        /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448));
        /// let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        /// let dec = GpuNr5gDecoder::new(code, cfg, 20);
        /// let channel = vec![Llr::new(4.0); 16896];
        /// let oracle = dec.cpu_reference_message(&channel);
        /// assert_eq!(oracle.len(), dec.target_k());
        /// ```
        ///
        /// # Complexity
        ///
        /// O(`max_iterations * edges`) CPU BP work over the full mother code;
        /// each call builds a CPU decoder.
        #[must_use]
        pub fn cpu_reference_message(&self, channel_llrs: &[Llr]) -> BitVec {
            let full_llrs = self.prepare_llrs(channel_llrs);
            let mut dec = LdpcDecoder::with_config(self.code.mother_code().clone(), self.config);
            let codeword = dec
                .decode_to_codeword(&full_llrs, self.max_iterations)
                .decoded_bits;
            self.extract_message(&codeword)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};

        fn small_code() -> Arc<Nr5gRateMatchedCode> {
            Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121))
        }

        #[test]
        fn test_dimensions_match_code() {
            let code = small_code();
            let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
            let dec = GpuNr5gDecoder::new(code.clone(), config, 25);
            assert_eq!(dec.target_k(), 121);
            assert_eq!(dec.target_n(), 256);
            assert_eq!(dec.full_n(), code.params().full_n);
            assert_eq!(dec.max_iterations(), 25);
        }

        #[test]
        fn test_prepare_llrs_has_full_n_length() {
            let code = small_code();
            let config = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
            let dec = GpuNr5gDecoder::new(code.clone(), config, 10);
            let channel: Vec<Llr> = vec![Llr::new(2.0); dec.target_n()];
            let prepared = dec.prepare_llrs(&channel);
            assert_eq!(prepared.len(), dec.full_n());
        }

        #[test]
        fn test_extract_message_is_natural_prefix() {
            let code = small_code();
            let config = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
            let dec = GpuNr5gDecoder::new(code.clone(), config, 10);
            let mut cw = BitVec::zeros(dec.full_n());
            for i in 0..dec.target_k() {
                cw.set(i, i % 3 == 0);
            }
            let msg = dec.extract_message(&cw);
            assert_eq!(msg.len(), dec.target_k());
            for i in 0..dec.target_k() {
                assert_eq!(msg.get(i), i % 3 == 0, "bit {i} must be the prefix");
            }
        }

        #[test]
        fn test_cpu_reference_recovers_zero_message() {
            let code = small_code();
            let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
            let dec = GpuNr5gDecoder::new(code.clone(), config, 25);
            // All-zero codeword: every LLR is positive.
            let channel: Vec<Llr> = vec![Llr::new(8.0); dec.target_n()];
            let msg = dec.cpu_reference_message(&channel);
            assert_eq!(msg.len(), dec.target_k());
            assert!(
                (0..dec.target_k()).all(|i| !msg.get(i)),
                "confident-zero LLRs recover the all-zero message"
            );
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::GpuNr5gDecoder;
