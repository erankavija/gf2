//! Host-side 5G NR flat-layout builder over the standard-agnostic GPU LDPC BP
//! kernel (design doc §6; epic `f9717e7e` Phase E `23d3525f`).
//!
//! This module discharges the per-`i_LS` shift-table consumption deferred from
//! Phase B `a930be7f`: it host-expands a 5G NR base graph + per-`i_LS` lifting
//! shift table into the **flat** [`LdpcGraphLayout`] the existing kernel
//! decodes, then reuses the kernel binary **unchanged**. The same device
//! kernel that decodes DVB-T2 also decodes 5G NR — only the host-built layout
//! differs (design §6 "same kernel parameterises both standards").
//!
//! # No second expansion (SSOT)
//!
//! The base-graph + per-`i_LS` shift → expanded Tanner-graph step lives
//! **entirely** in `gf2-coding`'s
//! [`QuasiCyclicLdpc::nr_5g`](gf2_coding::ldpc::QuasiCyclicLdpc::nr_5g) /
//! [`nr_5g_rate_matched`](gf2_coding::ldpc::QuasiCyclicLdpc::nr_5g_rate_matched):
//! it expands the base matrix with `V mod Z` circulants into a concrete
//! [`LdpcCode`] (the **mother code**). The flat → device
//! [`LdpcGraphLayout`] step is the **one** CSR/CSC flattener already used by
//! the DVB-T2 path inside [`GpuLdpcBp`]. This builder composes those two
//! existing steps; it introduces **no** new base+shift→layout expansion. The
//! per-`i_LS` shift is consumed host-side during the `gf2-coding` mother-code
//! construction, never by the kernel.
//!
//! # Rate matching is host-side LLR mapping (TS 38.212 §5.3.2)
//!
//! 5G NR rate matching is realised by LLR initialisation on the full mother
//! code, not by removing columns from `H` (see the
//! [`gf2-coding` 5G NR docs](gf2_coding::ldpc::nr_5g)). So the GPU path is:
//! map the `target_n` channel LLRs to the `full_n` mother-code LLR vector
//! ([`Nr5gRateMatchedCode::prepare_llrs`] — punctured = 0, filler = strong
//! prior), batch-decode the full mother codeword on the device (the existing
//! flat kernel), then extract the `target_k` message bits in natural column
//! order. Both the LLR prepare and the message extract are deterministic
//! host-side maps; the **device** does exactly the same flooding BP it does for
//! DVB-T2, so the GPU hard decision matches the CPU
//! [`Nr5gRateMatchedDecoder`](gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder)
//! bit-for-bit at a fixed seed (the [hard] byte-identity criterion).
//!
//! The module home is declared unconditionally in [`gpu`](crate::gpu); the
//! items are gated on `feature = "hip"` so the crate builds cleanly with the
//! feature off.

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

    /// A device LDPC BP decoder for a 3GPP TS 38.212 rate-matched 5G NR code.
    ///
    /// Wraps a [`GpuLdpcBp`] built over the 5G NR **mother code** (the full,
    /// already-expanded quasi-cyclic Tanner graph) plus the host-side rate-
    /// matching glue. The expensive base+shift→flat-layout work is done once at
    /// construction by the **existing** [`GpuLdpcBp`] flattener — there is no
    /// second expansion (see the [module docs](self)).
    ///
    /// Reusing one [`GpuNr5gDecoder`] across a batch is the throughput path:
    /// [`build_decoder`](Self::build_decoder) mints a per-worker device decoder
    /// once and [`decode_batch`](Self::decode_batch) drives it for each batch.
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
        /// The rate-matched 5G NR code (owns mother code, encoder, params).
        code: Arc<Nr5gRateMatchedCode>,
        /// GPU LDPC BP stage over the **mother code**; its existing layout
        /// flattener is the single base+shift→flat-layout path (no duplicate).
        gpu: GpuLdpcBp,
        /// The BP decoder configuration (algorithm + early termination),
        /// shared verbatim with the CPU reference for byte-identity.
        config: DecoderConfig,
        /// The BP iteration cap per frame.
        max_iterations: usize,
    }

    impl GpuNr5gDecoder {
        /// Builds a GPU 5G NR decoder over the given rate-matched code.
        ///
        /// The inner [`GpuLdpcBp`] is built over `code.mother_code()`; its
        /// existing CSR/CSC flattener turns the host-expanded mother-code
        /// parity-check matrix into the device [`LdpcGraphLayout`]. No new
        /// expansion is introduced.
        ///
        /// # Arguments
        ///
        /// * `code` — the 3GPP-conformant rate-matched 5G NR LDPC code.
        /// * `config` — the BP algorithm + early-termination configuration.
        ///   Must match the CPU reference for the byte-identity guarantee.
        /// * `max_iterations` — the BP iteration cap (must be `>= 1`).
        ///
        /// # Panics
        ///
        /// Panics if `max_iterations == 0` (via [`GpuLdpcBp::new`]).
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
        /// assert_eq!(dec.target_k(), 8448);
        /// ```
        ///
        /// # Complexity
        ///
        /// O(`edges`) over the full mother code — the one-time host CSR/CSC
        /// flattening inside [`GpuLdpcBp::new`]; no device work.
        ///
        /// [`LdpcGraphLayout`]: gf2_kernels_hip::launch_ldpc_bp::LdpcGraphLayout
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
        /// assert_eq!(dec.target_k(), 8448);
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn target_k(&self) -> usize {
            self.code.params().target_k
        }

        /// The transmitted codeword length `target_n` (the rate-matched `E`).
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
        /// assert_eq!(dec.target_n(), 16896);
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn target_n(&self) -> usize {
            self.code.params().target_n
        }

        /// The full mother-code length `full_n = N_b * Z`.
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
        /// assert_eq!(dec.full_n(), 26112);
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn full_n(&self) -> usize {
            self.code.params().full_n
        }

        /// The BP iteration cap.
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
        /// assert_eq!(dec.max_iterations(), 20);
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn max_iterations(&self) -> usize {
            self.max_iterations
        }

        /// The decoder configuration (algorithm + early termination).
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
        /// let _cfg = dec.config();
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn config(&self) -> DecoderConfig {
            self.config
        }

        /// Borrows the underlying mother-code [`GpuLdpcBp`] stage.
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
        /// let _inner = dec.gpu();
        /// ```
        ///
        /// # Complexity
        ///
        /// O(1).
        #[inline]
        #[must_use]
        pub fn gpu(&self) -> &GpuLdpcBp {
            &self.gpu
        }

        /// Builds a per-worker device decoder sized for up to `max_batch`
        /// frames, delegating to the inner [`GpuLdpcBp::build_decoder`].
        ///
        /// # Arguments
        ///
        /// * `max_batch` — the largest per-call frame count the decoder serves.
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
        ///
        /// # Complexity
        ///
        /// O(`edges + max_batch * full_n`) device allocations + the one-time
        /// graph upload; no per-frame work.
        pub fn build_decoder(&self, max_batch: usize) -> Result<KernelGpuLdpcBp, StageError> {
            self.gpu.build_decoder(max_batch)
        }

        /// Maps one frame's `target_n` channel LLRs to the `full_n` mother-code
        /// LLR vector (TS 38.212 §5.3.2 rate-matching LLR initialisation).
        ///
        /// This is the **same** host-side map the CPU
        /// [`Nr5gRateMatchedDecoder`](gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder)
        /// applies before BP, so the device sees identical inputs.
        ///
        /// # Arguments
        ///
        /// * `channel_llrs` — one frame's `target_n` received LLRs.
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
        ///
        /// # Complexity
        ///
        /// O(`full_n`) — one pass building the mother-code LLR vector.
        #[must_use]
        pub fn prepare_llrs(&self, channel_llrs: &[Llr]) -> Vec<Llr> {
            self.code.prepare_llrs(channel_llrs)
        }

        /// Decodes a batch of `target_n`-length channel-LLR frames to recovered
        /// `target_k`-bit messages on the device.
        ///
        /// Each frame is rate-match-mapped to the `full_n` mother-code LLR
        /// vector ([`prepare_llrs`](Self::prepare_llrs)), the full mother
        /// codeword is decoded on the device via the **existing** flat kernel,
        /// and the `target_k` message bits are extracted in natural column
        /// order — exactly the CPU `decode_iterative` postprocessing, so the
        /// recovered bits are byte-identical to the CPU reference at a fixed
        /// seed.
        ///
        /// # Arguments
        ///
        /// * `input` — the channel-LLR batch (each frame has `target_n` LLRs).
        /// * `decoder` — the per-worker device decoder from
        ///   [`build_decoder`](Self::build_decoder).
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
            // Rate-match-map every frame to the full mother-code LLR length.
            let prepared: Vec<Vec<Llr>> = input
                .frames
                .iter()
                .map(|frame| self.prepare_llrs(frame))
                .collect();
            // Decode the full mother codewords on the device (existing kernel).
            let mother = self.gpu.decode_batch(&LlrBatch::new(prepared), decoder)?;
            // Extract target_k message bits per frame in natural column order.
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
        /// This mirrors the CPU
        /// [`Nr5gRateMatchedDecoder::decode_iterative`](gf2_coding::traits::IterativeSoftDecoder::decode_iterative)
        /// postprocessing exactly: 3GPP TS 38.212 places message bit `i` at
        /// codeword position `i`, so extraction is a prefix slice, **not** an
        /// RREF-systematic gather.
        ///
        /// # Arguments
        ///
        /// * `mother_codeword` — a decoded full mother codeword (length
        ///   `full_n`).
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
        ///
        /// # Complexity
        ///
        /// O(`target_k`) — a prefix copy of the message bits.
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
        /// channel LLRs, via the CPU mother-code
        /// [`LdpcDecoder::decode_to_codeword`] on the **same** prepared LLRs +
        /// the **same** natural-order extraction.
        ///
        /// This is the exact oracle the GPU output is byte-identical to (it
        /// reproduces [`Nr5gRateMatchedDecoder::decode_iterative`] without the
        /// per-call decoder allocation churn). Used by the byte-identity test.
        ///
        /// # Arguments
        ///
        /// * `channel_llrs` — one frame's `target_n` channel LLRs.
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
        /// O(`max_iterations * edges`) CPU BP work over the full mother code
        /// per call (a fresh CPU decoder per invocation; test-oracle use only).
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
            // A small BG2 rate-matched code: fast to build, no GPU needed for
            // the host-side shape tests.
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
            // Set a recognisable prefix pattern in the message region.
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
            // The all-zero codeword: confident LLRs recover the zero message via
            // the CPU reference path (no GPU).
            let code = small_code();
            let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
            let dec = GpuNr5gDecoder::new(code.clone(), config, 25);
            // All-zero transmitted message -> all-zero codeword -> +LLRs.
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
