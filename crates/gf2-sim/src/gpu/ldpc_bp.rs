//! GPU LDPC belief-propagation decode stage (design doc §6 / §10 / §11,
//! `feature = "hip"`).
//!
//! [`GpuLdpcBp`] is the device-accelerated counterpart of the CPU
//! [`LdpcDecoder`](gf2_coding::ldpc::LdpcDecoder). It runs the same
//! flooding belief-propagation schedule (init → alternating check-node and
//! variable-node updates with optional per-iteration syndrome
//! early-termination) on the device LDPC BP kernel
//! (`gf2-kernels-hip`'s [`GpuLdpcBp`](gf2_kernels_hip::GpuLdpcBp)) and emits the
//! hard-decision codeword (all `n` positions) per frame.
//!
//! # Byte-identity (design doc §11)
//!
//! The check-node gather order is the parity-check matrix CSR `row_iter` order
//! and the variable-node belief sum order is the CSC `col_iter` order —
//! **exactly** the CPU [`LdpcDecoder`](gf2_coding::ldpc::LdpcDecoder)'s
//! `check_neighbors` / `var_neighbors` orders — so the device output matches the
//! CPU hard decision bit-for-bit. For MinSum / NormalizedMinSum the check-node
//! rule uses only sign / min / scalar-multiply (order-independent and exactly
//! representable in f32); for SumProduct the `tanh` product is accumulated in
//! the same CSR order. The hard-decision *verdict* is robust to the 1-3 ULP
//! RDNA2 transcendental drift (design §11 rationale), so the 200-frame
//! bit-for-bit criterion holds even though `mean_iters` may differ across paths
//! (which is why `mean_iters` is EXCLUDED from CPU-vs-GPU byte-identity).
//!
//! # CPU fallback (§8)
//!
//! The [`Stage::CpuFallback`](crate::Stage) is the CPU
//! [`LdpcDecoder`](gf2_coding::ldpc::LdpcDecoder):
//! [`cpu_fallback`](GpuLdpcBp::cpu_fallback) returns a decoder built from the
//! *same* code and [`DecoderConfig`], so the Phase C executor can substitute it
//! on a GPU out-of-memory or unsupported-arch fault.
//!
//! # 5G NR seam (design doc §6, Phase E `23d3525f`)
//!
//! The device kernel accepts a per-`i_LS` shift table at launch time (currently
//! unused by the fully-expanded DVB-T2 graph); the same kernel parameterises
//! both standards. This stage builds the DVB-T2 layout today; a Phase E
//! constructor will build the BG1/BG2 layout from the lifting-set shift row.
//!
//! The module home is declared unconditionally in [`gpu`](crate::gpu); the items
//! are gated on `feature = "hip"` so the crate builds cleanly with the feature
//! off.

#[cfg(feature = "hip")]
mod imp {
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
    use gf2_coding::Llr;
    use gf2_core::BitVec;
    use gf2_kernels_hip::launch_ldpc_bp::{GpuBpAlgorithm, LdpcGraphLayout};
    use gf2_kernels_hip::GpuLdpcBp as KernelGpuLdpcBp;

    use crate::batch::{HardDecisionBatch, LlrBatch};
    use crate::error::StageError;
    use crate::gpu::map_hip_error;
    use crate::stage::{ExecutionClass, Stage};

    /// Builds the device CSR/CSC [`LdpcGraphLayout`] from an [`LdpcCode`],
    /// reproducing the CPU decoder's neighbor orders.
    ///
    /// The check-major CSR rows follow `h.row_iter(c)` (the CPU
    /// `check_neighbors[c]` order) and the variable-major CSC columns follow
    /// `h.col_iter(v)` (the CPU `var_neighbors[v]` order). The two cross-maps
    /// link a check-edge to the variable-edge for the same Tanner edge, so the
    /// kernel's per-edge gather visits messages in exactly the CPU order — the
    /// basis of the hard-decision byte-identity.
    fn build_layout(code: &LdpcCode) -> LdpcGraphLayout {
        let n = code.n();
        let m = code.m();
        let h = code.parity_check_matrix();

        // Check-major CSR (row_iter order) and variable-major CSC (col_iter
        // order). Edge indices are assigned in scan order within each view.
        let check_neighbors: Vec<Vec<usize>> = (0..m).map(|c| h.row_iter(c).collect()).collect();
        let var_neighbors: Vec<Vec<usize>> = (0..n).map(|v| h.col_iter(v).collect()).collect();

        let edges: usize = check_neighbors.iter().map(Vec::len).sum();
        debug_assert_eq!(edges, var_neighbors.iter().map(Vec::len).sum::<usize>());

        // CSR row pointers + the variable of each check-edge.
        let mut check_row_ptr = Vec::with_capacity(m + 1);
        let mut check_edge_var = Vec::with_capacity(edges);
        check_row_ptr.push(0i32);
        for neigh in &check_neighbors {
            for &v in neigh {
                check_edge_var.push(v as i32);
            }
            check_row_ptr.push(check_edge_var.len() as i32);
        }

        // CSC column pointers. We also record, per variable, the starting
        // variable-edge index so the cross-maps can resolve a (var, check) pair
        // to its variable-edge slot. (No per-var-edge "check" array is uploaded:
        // the syndrome kernel reads the per-check-edge variable via
        // `check_edge_var`; the var-side check identity is only needed here to
        // build the cross-maps.)
        let mut var_col_ptr = Vec::with_capacity(n + 1);
        let mut var_edge_start = Vec::with_capacity(n);
        var_col_ptr.push(0i32);
        let mut running = 0usize;
        for neigh in &var_neighbors {
            var_edge_start.push(running);
            running += neigh.len();
            var_col_ptr.push(running as i32);
        }

        // For a (var, check) pair, find the variable-edge index = the var's edge
        // start + the position of `check` within `var_neighbors[var]`. Likewise
        // for the check view. Build a position lookup per variable to keep this
        // O(edges) rather than O(edges * degree).
        let mut var_check_pos: Vec<std::collections::HashMap<usize, usize>> = Vec::with_capacity(n);
        for neigh in &var_neighbors {
            let mut map = std::collections::HashMap::with_capacity(neigh.len());
            for (pos, &c) in neigh.iter().enumerate() {
                map.insert(c, pos);
            }
            var_check_pos.push(map);
        }

        // check_edge_to_var_edge: for each check-edge (c, v) the var-edge slot
        // (v, c). var_edge_to_check_edge is its inverse over the same edge set.
        let mut check_edge_to_var_edge = vec![0i32; edges];
        let mut var_edge_to_check_edge = vec![0i32; edges];
        let mut e = 0usize; // check-edge index, in CSR scan order
        for (c, neigh) in check_neighbors.iter().enumerate() {
            for &v in neigh {
                let pos = var_check_pos[v][&c];
                let f = var_edge_start[v] + pos; // variable-edge index
                check_edge_to_var_edge[e] = f as i32;
                var_edge_to_check_edge[f] = e as i32;
                e += 1;
            }
        }

        LdpcGraphLayout {
            n,
            m,
            check_row_ptr,
            check_edge_var,
            check_edge_to_var_edge,
            var_col_ptr,
            var_edge_to_check_edge,
            // DVB-T2 is a fully-expanded graph: no per-`i_LS` 5G NR shift row.
            // The Phase E (`23d3525f`) BG1/BG2 builder will populate this; the
            // kernel signature already carries it (non-breaking).
            shift_table: Vec::new(),
        }
    }

    /// CPU LDPC BP decode stage wrapping [`LdpcDecoder`] — the registered
    /// [`Stage::CpuFallback`](crate::Stage) for [`GpuLdpcBp`] (design doc §8).
    ///
    /// The `Stage::CpuFallback` associated type must itself be a
    /// `Stage<LlrBatch, HardDecisionBatch>`; `LdpcDecoder` lives in `gf2-coding`
    /// and does not (and cannot, by the orphan rule) implement this `gf2-sim`
    /// trait, so this thin wrapper carries the `Stage` impl while delegating the
    /// actual belief-propagation to an owned `LdpcDecoder`. It produces the same
    /// full `n`-bit hard-decision codeword as the GPU stage (via
    /// [`LdpcDecoder::decode_to_codeword`]), so substituting it on a GPU fault is
    /// transparent. [`decoder`](Self::decoder) exposes the underlying decoder.
    pub struct CpuLdpcBp {
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        decoder: std::sync::Mutex<LdpcDecoder>,
    }

    impl CpuLdpcBp {
        /// Builds a CPU LDPC BP stage from the same code + config as its paired
        /// [`GpuLdpcBp`].
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

        /// Locks and returns the owned [`LdpcDecoder`] (the underlying CPU
        /// decoder this stage delegates to).
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

    /// Maps the CPU [`DecoderAlgorithm`] onto the kernel [`GpuBpAlgorithm`].
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
    /// Holds the [`LdpcCode`], the BP [`DecoderConfig`], and the maximum BP
    /// iteration count. The device decoder is built lazily (per
    /// [`process`](Stage::process) call) so the stage is constructible without a
    /// GPU; the throughput path builds one per-worker device decoder via
    /// [`build_decoder`](Self::build_decoder) and drives it with
    /// [`decode_batch`](Self::decode_batch).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode};
    /// use gf2_coding::CodeRate;
    ///
    /// // Requires a real HIP device to decode; constructing the stage does not.
    /// let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    /// let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);
    /// let stage = GpuLdpcBp::new(code, config, 50);
    /// assert_eq!(stage.max_iterations(), 50);
    /// ```
    pub struct GpuLdpcBp {
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        device_id: i32,
        /// The paired CPU fallback stage (same code + config), returned by
        /// [`cpu_fallback`](Self::cpu_fallback) (design doc §8). It wraps an
        /// [`LdpcDecoder`] (the trait bound requires the fallback be a `Stage`,
        /// which `LdpcDecoder` itself is not).
        fallback: CpuLdpcBp,
    }

    impl GpuLdpcBp {
        /// Constructs a GPU LDPC BP decode stage on device 0.
        ///
        /// # Arguments
        ///
        /// * `code` — the LDPC code to decode.
        /// * `config` — the BP algorithm + early-termination configuration.
        /// * `max_iterations` — the BP iteration cap (must be `>= 1`).
        ///
        /// # Panics
        ///
        /// Panics if `max_iterations == 0`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
        /// use gf2_coding::ldpc::{DecoderConfig, LdpcCode};
        /// use gf2_coding::CodeRate;
        ///
        /// let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        /// let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 50);
        /// assert_eq!(stage.max_iterations(), 50);
        /// ```
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
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
        /// use gf2_coding::ldpc::{DecoderConfig, LdpcCode};
        /// use gf2_coding::CodeRate;
        ///
        /// let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        /// let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 50).on_device(0);
        /// assert_eq!(stage.device_id(), 0);
        /// ```
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
        /// The executor / benchmark calls this once per worker and threads the
        /// result into [`decode_batch`](Self::decode_batch), keeping the
        /// non-`Sync` device buffers out of the `Sync`-bound scratch.
        ///
        /// # Arguments
        ///
        /// * `max_batch` — the largest per-call frame count the decoder serves.
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

        /// Decodes an [`LlrBatch`] to a [`HardDecisionBatch`] using the
        /// caller-owned device decoder `decoder`.
        ///
        /// Each frame's channel LLRs are decoded to the full `n`-bit
        /// hard-decision codeword. The `decoder` must have been built for this
        /// stage's code via [`build_decoder`](Self::build_decoder) with a
        /// `max_batch >= input.frames.len()`.
        ///
        /// # Arguments
        ///
        /// * `input` — the channel-LLR batch (each frame has `n` LLRs).
        /// * `decoder` — the per-worker device decoder.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
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
            let n = self.code.n();
            let llr_blocks: Vec<Vec<f32>> = input
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
                .collect();

            let algorithm = map_algorithm(self.config.algorithm());
            let early = self.config.early_termination();
            let hard = decoder
                .decode_batch(&llr_blocks, algorithm, self.max_iterations, early)
                .map_err(|e| map_hip_error(e, "GpuLdpcBp::decode_batch"))?;

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
            Ok(HardDecisionBatch::new(frames))
        }

        /// The CPU reference codeword for one frame's LLRs, via
        /// [`LdpcDecoder::decode_to_codeword`] on a fresh decoder (the exact
        /// hard-decision oracle the GPU output is byte-identical to).
        ///
        /// Used by the byte-identity test and the throughput benchmark's CPU
        /// comparator. Builds a one-shot decoder so it is stateless per call.
        ///
        /// # Arguments
        ///
        /// * `llrs` — one frame's channel LLRs (length `n`).
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

        /// Decodes `input` by building a one-shot device decoder sized for the
        /// batch and running the BP schedule. The throughput path is
        /// [`decode_batch`](Self::decode_batch) with a caller-owned per-worker
        /// decoder (the device buffers cannot live in the `Sync`-bound scratch).
        /// An empty batch is a no-op (no device decoder is built).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
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

        /// The paired CPU [`CpuLdpcBp`] fallback (design doc §8): wraps an
        /// [`LdpcDecoder`] built from the same code + [`DecoderConfig`].
        fn cpu_fallback(&self) -> Option<&CpuLdpcBp> {
            Some(&self.fallback)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use gf2_coding::ldpc::DecoderAlgorithm;

        /// A tiny [n=6, m=3] LDPC code with degree-2 checks and degree-1/2 vars,
        /// for layout-shape unit tests that need no GPU.
        fn small_code() -> LdpcCode {
            // Edges (check, var): a sparse but valid Tanner graph.
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
            // 9 edges in the COO list.
            assert_eq!(edges, 9);
            assert_eq!(layout.check_row_ptr.len(), 4);
            assert_eq!(layout.var_col_ptr.len(), 7);
            assert_eq!(layout.check_edge_to_var_edge.len(), edges);
            assert_eq!(layout.var_edge_to_check_edge.len(), edges);

            // The cross-maps must be exact inverses over the edge set.
            for e in 0..edges {
                let f = layout.check_edge_to_var_edge[e] as usize;
                assert_eq!(
                    layout.var_edge_to_check_edge[f] as usize, e,
                    "cross-maps must be inverse at check-edge {e}"
                );
            }

            // A check-edge `e` and its mapped var-edge `f` must reference the
            // SAME Tanner edge: f lies within variable `check_edge_var[e]`'s CSC
            // column, and the inverse map lands back on a check-edge whose
            // variable is the same `v`. (No `var_edge_check` array is uploaded;
            // the check identity of a var-edge is recovered through the inverse
            // cross-map + the row that owns the check-edge, which the syndrome
            // kernel uses via `check_edge_var`.)
            for c in 0..layout.m {
                let cs = layout.check_row_ptr[c] as usize;
                let ce = layout.check_row_ptr[c + 1] as usize;
                for e in cs..ce {
                    let v = layout.check_edge_var[e] as usize;
                    let f = layout.check_edge_to_var_edge[e] as usize;
                    // f must lie within variable v's CSC column.
                    let vs = layout.var_col_ptr[v] as usize;
                    let ve = layout.var_col_ptr[v + 1] as usize;
                    assert!(
                        f >= vs && f < ve,
                        "var-edge {f} must lie in variable {v}'s column [{vs}, {ve})"
                    );
                    // The inverse map returns to a check-edge whose variable is v.
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
            // The fallback stage reports the same code dimensions + iteration cap.
            assert_eq!(fb.n(), 6);
            assert_eq!(fb.max_iterations(), 50);
        }

        #[test]
        fn test_execution_class_is_gpu_only() {
            let stage = GpuLdpcBp::new(small_code(), DecoderConfig::default(), 50);
            assert_eq!(stage.execution_class(), ExecutionClass::GpuOnly);
        }

        /// The stage must be `Send` (per-worker-owned) so the executor can move
        /// it between rayon workers.
        #[test]
        fn test_stage_is_send() {
            fn assert_send<T: Send>() {}
            assert_send::<GpuLdpcBp>();
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::{CpuLdpcBp, GpuLdpcBp};
