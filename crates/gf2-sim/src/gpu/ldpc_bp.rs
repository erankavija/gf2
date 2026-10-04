//! GPU LDPC belief-propagation decode stage (`feature = "hip"`).
//!
//! `GpuLdpcBp` runs the flooding belief-propagation schedule of the CPU
//! [`LdpcDecoder`](gf2_coding::ldpc::LdpcDecoder) on the `gf2-kernels-hip`
//! LDPC BP kernel and emits the `n`-bit hard-decision codeword per frame.
//!
//! The device layout is the canonical
//! [`EdgeLayout`](gf2_coding::ldpc::EdgeLayout) of an expanded
//! [`LdpcCode`](gf2_coding::ldpc::LdpcCode) parity-check matrix, the edge
//! indexing the CPU decoder passes messages over, so both sides gather
//! check-node messages in CSR order and sum variable-node beliefs in CSC
//! order. Per-frame iteration counts are excluded from the CPU-vs-GPU
//! byte-identity contract.

#[cfg(feature = "hip")]
mod imp {
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, EdgeLayout, LdpcCode, LdpcDecoder};
    use gf2_coding::Llr;
    use gf2_core::BitVec;
    use gf2_kernels_hip::host::HipStream;
    use gf2_kernels_hip::launch_ldpc_bp::{GpuBpAlgorithm, LdpcGraphLayout, LdpcStreamScratch};
    use gf2_kernels_hip::GpuLdpcBp as KernelGpuLdpcBp;

    use crate::batch::{HardDecisionBatch, LlrBatch};
    use crate::error::StageError;
    use crate::gpu::map_hip_error;
    use crate::stage::{ExecutionClass, Stage};

    /// Builds the device CSR/CSC [`LdpcGraphLayout`] from the code's canonical
    /// [`EdgeLayout`], widened to the `i32` the kernel consumes.
    fn build_layout(code: &LdpcCode) -> LdpcGraphLayout {
        let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
        let widen = |values: &[u32]| values.iter().map(|&value| value as i32).collect();
        LdpcGraphLayout {
            n: layout.n(),
            m: layout.m(),
            check_row_ptr: widen(layout.check_offsets()),
            check_edge_var: widen(layout.check_edge_var()),
            check_edge_to_var_edge: widen(layout.check_edge_to_var_edge()),
            var_col_ptr: widen(layout.var_offsets()),
            var_edge_to_check_edge: widen(layout.var_edge_to_check_edge()),
        }
    }

    /// CPU LDPC BP decode stage wrapping [`LdpcDecoder`]: the
    /// [`Stage::CpuFallback`](crate::Stage) of [`GpuLdpcBp`].
    ///
    /// The orphan rule forbids implementing [`Stage`] on the `gf2-coding`
    /// `LdpcDecoder`, so this wrapper carries the impl. It emits the `n`-bit
    /// hard-decision codeword of [`LdpcDecoder::decode_to_codeword`].
    pub struct CpuLdpcBp {
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        decoder: std::sync::Mutex<LdpcDecoder>,
    }

    impl CpuLdpcBp {
        /// Builds a CPU LDPC BP stage.
        #[must_use]
        pub fn new(code: LdpcCode, config: DecoderConfig, max_iterations: usize) -> Self {
            let decoder = std::sync::Mutex::new(LdpcDecoder::with_config(code.clone(), config));
            Self {
                code,
                config,
                max_iterations,
                decoder,
            }
        }

        /// The codeword length `n`.
        #[inline]
        #[must_use]
        pub fn n(&self) -> usize {
            self.code.n()
        }

        /// The BP iteration cap.
        #[inline]
        #[must_use]
        pub fn max_iterations(&self) -> usize {
            self.max_iterations
        }

        /// The decoder configuration.
        #[inline]
        #[must_use]
        pub fn config(&self) -> DecoderConfig {
            self.config
        }

        /// Locks and returns the owned [`LdpcDecoder`].
        ///
        /// # Panics
        ///
        /// Panics if the internal mutex is poisoned.
        pub fn decoder(&self) -> std::sync::MutexGuard<'_, LdpcDecoder> {
            self.decoder.lock().expect("CpuLdpcBp decoder mutex")
        }
    }

    impl Stage<LlrBatch, HardDecisionBatch> for CpuLdpcBp {
        type Scratch = ();
        type CpuFallback = Self;

        fn process(
            &self,
            input: &LlrBatch,
            _scratch: &mut (),
        ) -> Result<HardDecisionBatch, StageError> {
            let mut dec = self.decoder.lock().expect("CpuLdpcBp decoder mutex");
            let frames: Vec<BitVec> = input
                .frames
                .iter()
                .map(|llrs| {
                    dec.decode_to_codeword(llrs, self.max_iterations)
                        .decoded_bits
                })
                .collect();
            Ok(HardDecisionBatch::new(frames))
        }

        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }

        fn cpu_fallback(&self) -> Option<&Self> {
            Some(self)
        }
    }

    fn map_algorithm(alg: DecoderAlgorithm) -> GpuBpAlgorithm {
        match alg {
            DecoderAlgorithm::MinSum => GpuBpAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(a) => GpuBpAlgorithm::NormalizedMinSum(a),
            DecoderAlgorithm::OffsetMinSum(b) => GpuBpAlgorithm::OffsetMinSum(b),
            DecoderAlgorithm::SumProduct => GpuBpAlgorithm::SumProduct,
        }
    }

    /// GPU LDPC belief-propagation decode stage: [`LlrBatch`] →
    /// [`HardDecisionBatch`] (full `n`-bit hard-decision codeword per frame).
    ///
    /// Construction touches no device: [`process`](Stage::process) builds a
    /// device decoder per call, and [`decode_batch`](Self::decode_batch) reuses
    /// one from [`build_decoder`](Self::build_decoder).
    pub struct GpuLdpcBp {
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        device_id: i32,
        fallback: CpuLdpcBp,
    }

    impl GpuLdpcBp {
        /// Constructs a GPU LDPC BP decode stage on device 0.
        ///
        /// # Panics
        ///
        /// Panics if `max_iterations == 0`.
        #[must_use]
        pub fn new(code: LdpcCode, config: DecoderConfig, max_iterations: usize) -> Self {
            assert!(max_iterations >= 1, "max_iterations must be >= 1");
            let fallback = CpuLdpcBp::new(code.clone(), config, max_iterations);
            Self {
                code,
                config,
                max_iterations,
                device_id: 0,
                fallback,
            }
        }

        /// Targets a non-default HIP device for the device decoder.
        #[must_use]
        pub fn on_device(mut self, device_id: i32) -> Self {
            self.device_id = device_id;
            self
        }

        /// The codeword length `n`.
        #[inline]
        #[must_use]
        pub fn n(&self) -> usize {
            self.code.n()
        }

        /// The BP iteration cap.
        #[inline]
        #[must_use]
        pub fn max_iterations(&self) -> usize {
            self.max_iterations
        }

        /// The HIP device the decoder targets.
        #[inline]
        #[must_use]
        pub fn device_id(&self) -> i32 {
            self.device_id
        }

        /// The decoder configuration (algorithm + early termination).
        #[inline]
        #[must_use]
        pub fn config(&self) -> DecoderConfig {
            self.config
        }

        /// Builds a per-worker device decoder sized for up to `max_batch` frames.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if the device allocation or graph upload fails.
        pub fn build_decoder(&self, max_batch: usize) -> Result<KernelGpuLdpcBp, StageError> {
            let layout = build_layout(&self.code);
            KernelGpuLdpcBp::new(&layout, max_batch, self.device_id)
                .map_err(|e| map_hip_error(e, "GpuLdpcBp::new"))
        }

        /// Allocates the pinned host staging the stream-ordered decode variants
        /// ([`decode_batch_on_stream`](Self::decode_batch_on_stream) /
        /// [`decode_batch_with_iters_on_stream`](Self::decode_batch_with_iters_on_stream))
        /// require, sized for `decoder`.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if a pinned allocation fails (an OOM is recoverable).
        pub fn build_stream_scratch(
            &self,
            decoder: &KernelGpuLdpcBp,
        ) -> Result<LdpcStreamScratch, StageError> {
            decoder
                .new_stream_scratch()
                .map_err(|e| map_hip_error(e, "GpuLdpcBp::new_stream_scratch"))
        }

        /// Decodes an [`LlrBatch`] to a [`HardDecisionBatch`] using the
        /// caller-owned device decoder `decoder`.
        ///
        /// `decoder` must come from [`build_decoder`](Self::build_decoder) with
        /// `max_batch >= input.frames.len()`.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        ///
        /// # Panics
        ///
        /// Panics if any frame's LLR length != `n`.
        ///
        /// # Complexity
        ///
        /// O(`max_iterations * batch * edges`) device work plus the per-call
        /// H2D / D2H transfers.
        pub fn decode_batch(
            &self,
            input: &LlrBatch,
            decoder: &KernelGpuLdpcBp,
        ) -> Result<HardDecisionBatch, StageError> {
            let (hard, _iters) = self.decode_batch_with_iters(input, decoder)?;
            Ok(hard)
        }

        /// Like [`decode_batch`](Self::decode_batch), but also returns the
        /// per-frame BP iteration count (`iters[f]` for frame `f`).
        ///
        /// The counts follow the convention of
        /// [`KernelGpuLdpcBp::decode_batch_with_iters`](gf2_kernels_hip::GpuLdpcBp::decode_batch_with_iters)
        /// and are excluded from the CPU-vs-GPU byte-identity contract.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        ///
        /// # Panics
        ///
        /// Panics if any frame's LLR length != `n`.
        ///
        /// # Complexity
        ///
        /// Identical to [`decode_batch`](Self::decode_batch).
        pub fn decode_batch_with_iters(
            &self,
            input: &LlrBatch,
            decoder: &KernelGpuLdpcBp,
        ) -> Result<(HardDecisionBatch, Vec<u32>), StageError> {
            let llr_blocks = self.flatten_llrs(input);
            let algorithm = map_algorithm(self.config.algorithm());
            let early = self.config.early_termination();
            let (hard, iters) = decoder
                .decode_batch_with_iters(&llr_blocks, algorithm, self.max_iterations, early)
                .map_err(|e| map_hip_error(e, "GpuLdpcBp::decode_batch_with_iters"))?;
            Ok((self.to_hard_batch(hard), iters))
        }

        /// Like [`decode_batch`](Self::decode_batch), but with every kernel
        /// launch and H2D / D2H transfer enqueued on the caller-owned `stream`
        /// and awaited per stream ([`HipStream::synchronize`]). The output is
        /// byte-identical to [`decode_batch`](Self::decode_batch). `scratch`
        /// comes from [`build_stream_scratch`](Self::build_stream_scratch).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        ///
        /// # Panics
        ///
        /// Panics if any frame's LLR length != `n`, or if `scratch` was sized
        /// for a different decoder.
        ///
        /// # Complexity
        ///
        /// Identical to [`decode_batch`](Self::decode_batch).
        pub fn decode_batch_on_stream(
            &self,
            input: &LlrBatch,
            decoder: &KernelGpuLdpcBp,
            stream: &HipStream,
            scratch: &mut LdpcStreamScratch,
        ) -> Result<HardDecisionBatch, StageError> {
            let (hard, _iters) =
                self.decode_batch_with_iters_on_stream(input, decoder, stream, scratch)?;
            Ok(hard)
        }

        /// Like [`decode_batch_with_iters`](Self::decode_batch_with_iters), but
        /// stream-ordered as in
        /// [`decode_batch_on_stream`](Self::decode_batch_on_stream). The hard
        /// decisions and per-frame iteration counts are byte-identical to the
        /// default-stream variant.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        ///
        /// # Panics
        ///
        /// Same as [`decode_batch_on_stream`](Self::decode_batch_on_stream).
        ///
        /// # Complexity
        ///
        /// Identical to [`decode_batch`](Self::decode_batch).
        pub fn decode_batch_with_iters_on_stream(
            &self,
            input: &LlrBatch,
            decoder: &KernelGpuLdpcBp,
            stream: &HipStream,
            scratch: &mut LdpcStreamScratch,
        ) -> Result<(HardDecisionBatch, Vec<u32>), StageError> {
            let llr_blocks = self.flatten_llrs(input);
            let algorithm = map_algorithm(self.config.algorithm());
            let early = self.config.early_termination();
            let (hard, iters) = decoder
                .decode_batch_with_iters_on_stream(
                    &llr_blocks,
                    algorithm,
                    self.max_iterations,
                    early,
                    stream,
                    scratch,
                )
                .map_err(|e| map_hip_error(e, "GpuLdpcBp::decode_batch_with_iters_on_stream"))?;
            Ok((self.to_hard_batch(hard), iters))
        }

        /// Flattens an [`LlrBatch`] into the per-frame `f32` blocks the kernel
        /// decoder consumes, asserting every frame has length `n`.
        fn flatten_llrs(&self, input: &LlrBatch) -> Vec<Vec<f32>> {
            let n = self.code.n();
            input
                .frames
                .iter()
                .map(|frame| {
                    assert_eq!(
                        frame.len(),
                        n,
                        "GpuLdpcBp::decode_batch: LLR frame length {} != n {}",
                        frame.len(),
                        n
                    );
                    frame.iter().map(|l| l.value()).collect()
                })
                .collect()
        }

        fn to_hard_batch(&self, hard: Vec<Vec<bool>>) -> HardDecisionBatch {
            let n = self.code.n();
            let frames: Vec<BitVec> = hard
                .into_iter()
                .map(|bits| {
                    let mut bv = BitVec::with_capacity(n);
                    for b in bits {
                        bv.push_bit(b);
                    }
                    bv
                })
                .collect();
            HardDecisionBatch::new(frames)
        }

        /// The CPU reference codeword for one frame's LLRs, via
        /// [`LdpcDecoder::decode_to_codeword`] on a fresh decoder.
        ///
        /// # Panics
        ///
        /// Panics if `llrs.len() != n`.
        #[must_use]
        pub fn cpu_reference_codeword(&self, llrs: &[Llr]) -> BitVec {
            let mut dec = LdpcDecoder::with_config(self.code.clone(), self.config);
            dec.decode_to_codeword(llrs, self.max_iterations)
                .decoded_bits
        }
    }

    impl Stage<LlrBatch, HardDecisionBatch> for GpuLdpcBp {
        type Scratch = ();
        type CpuFallback = CpuLdpcBp;

        /// Decodes `input` with a device decoder built per call and sized for
        /// the batch. An empty batch builds no device decoder.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        fn process(
            &self,
            input: &LlrBatch,
            _scratch: &mut (),
        ) -> Result<HardDecisionBatch, StageError> {
            if input.frames.is_empty() {
                return Ok(HardDecisionBatch::new(Vec::new()));
            }
            let decoder = self.build_decoder(input.frames.len())?;
            self.decode_batch(input, &decoder)
        }

        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::GpuOnly
        }

        /// The paired [`CpuLdpcBp`] built from the same code and
        /// [`DecoderConfig`].
        fn cpu_fallback(&self) -> Option<&CpuLdpcBp> {
            Some(&self.fallback)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use gf2_coding::ldpc::DecoderAlgorithm;

        /// An n=6, m=3 LDPC code given as (check, var) edges.
        fn small_code() -> LdpcCode {
            let edges = vec![
                (0, 0),
                (0, 1),
                (0, 3),
                (1, 1),
                (1, 2),
                (1, 4),
                (2, 2),
                (2, 0),
                (2, 5),
            ];
            LdpcCode::from_edges(3, 6, &edges)
        }

        #[test]
        fn test_build_layout_shapes_and_crossmaps() {
            let code = small_code();
            let layout = build_layout(&code);
            assert_eq!(layout.n, 6);
            assert_eq!(layout.m, 3);
            let edges = layout.edges();
            assert_eq!(edges, 9);
            assert_eq!(layout.check_row_ptr.len(), 4);
            assert_eq!(layout.var_col_ptr.len(), 7);
            assert_eq!(layout.check_edge_to_var_edge.len(), edges);
            assert_eq!(layout.var_edge_to_check_edge.len(), edges);

            for e in 0..edges {
                let f = layout.check_edge_to_var_edge[e] as usize;
                assert_eq!(
                    layout.var_edge_to_check_edge[f] as usize, e,
                    "cross-maps must be inverse at check-edge {e}"
                );
            }

            for c in 0..layout.m {
                let cs = layout.check_row_ptr[c] as usize;
                let ce = layout.check_row_ptr[c + 1] as usize;
                for e in cs..ce {
                    let v = layout.check_edge_var[e] as usize;
                    let f = layout.check_edge_to_var_edge[e] as usize;
                    let vs = layout.var_col_ptr[v] as usize;
                    let ve = layout.var_col_ptr[v + 1] as usize;
                    assert!(
                        f >= vs && f < ve,
                        "var-edge {f} must lie in variable {v}'s column [{vs}, {ve})"
                    );
                    let e_back = layout.var_edge_to_check_edge[f] as usize;
                    assert_eq!(
                        layout.check_edge_var[e_back] as usize, v,
                        "inverse cross-map for var-edge {f} must land on a \
                         check-edge of variable {v}"
                    );
                }
            }
        }

        #[test]
        fn test_map_algorithm_covers_all_variants() {
            assert_eq!(
                map_algorithm(DecoderAlgorithm::MinSum),
                GpuBpAlgorithm::MinSum
            );
            assert_eq!(
                map_algorithm(DecoderAlgorithm::NormalizedMinSum(0.75)),
                GpuBpAlgorithm::NormalizedMinSum(0.75)
            );
            assert_eq!(
                map_algorithm(DecoderAlgorithm::OffsetMinSum(0.5)),
                GpuBpAlgorithm::OffsetMinSum(0.5)
            );
            assert_eq!(
                map_algorithm(DecoderAlgorithm::SumProduct),
                GpuBpAlgorithm::SumProduct
            );
        }

        #[test]
        fn test_cpu_fallback_has_same_code_dimensions() {
            let code = small_code();
            let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 50);
            let fb = stage.cpu_fallback().expect("GPU stage has a CPU fallback");
            assert_eq!(fb.n(), 6);
            assert_eq!(fb.max_iterations(), 50);
        }

        #[test]
        fn test_execution_class_is_gpu_only() {
            let stage = GpuLdpcBp::new(small_code(), DecoderConfig::default(), 50);
            assert_eq!(stage.execution_class(), ExecutionClass::GpuOnly);
        }

        #[test]
        fn test_stage_is_send() {
            fn assert_send<T: Send>() {}
            assert_send::<GpuLdpcBp>();
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::{CpuLdpcBp, GpuLdpcBp};
