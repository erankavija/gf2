//! GPU Gray-QAM soft-demap stage (design doc §8 / §11, `feature = "hip"`).
//!
//! `GpuGrayQamDemapper` is the device-accelerated counterpart of the CPU
//! [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper) demap stage.
//! It wraps the existing `gf2-kernels-hip` `GpuGrayQamDemapper` `demap_batch`
//! kernel as a [`Stage<SymbolBatch, LlrBatch>`](crate::Stage), turning a batch
//! of received I/Q symbol frames into soft channel LLRs.
//!
//! # Demap method scoping — GPU is MAX-LOG only
//!
//! The device kernel implements the **max-log** LLR approximation only (its
//! doc: "max-log LLRs", symbol-major, MSB-first).
//! [`DemapMethod`](gf2_coding::modem::DemapMethod) has two variants, `MaxLog`
//! and `ExactLogMap`:
//!
//! - `MaxLog` is served by the GPU kernel.
//! - `ExactLogMap` has **no** GPU exact-log-map path. There is no fabricated
//!   exact-log-map GPU kernel: the erased [`Stage::process`](crate::Stage) path
//!   runs MaxLog on the GPU, and a stage constructed for `ExactLogMap` reports
//!   [`ExecutionClass::CpuOnly`](crate::stage::ExecutionClass) and routes
//!   through the CPU `GpuGrayQamDemapper::cpu_fallback` (the CPU
//!   `FastGrayQamDemapper` computes the exact log-MAP correctly). This keeps
//!   the contract honest — the GPU never claims an exact-log-map it does not
//!   compute.
//!
//! # LLR ordering and sign convention (byte-identity basis)
//!
//! Both the GPU kernel and the CPU
//! [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper) emit LLRs in
//! the **same** symbol-major, MSB-first layout: per symbol, the first `m/2`
//! values are the I-axis Gray-PAM label LLRs (MSB = coarsest level), then the
//! `m/2` Q-axis values. Both consume the same post-normalization
//! [`pam_levels`](gf2_coding::modem::FastGrayQamDemapper::pam_levels) table
//! (the GPU stage builds its device demapper from the CPU demapper's table, so
//! the level set is shared, never re-derived). LLR sign follows
//! [`Llr`](gf2_coding::Llr): positive LLR = bit 0 more likely. The orderings
//! therefore align bit-for-bit; the only residual difference is the SIMT-vs-SIMD
//! `max`/reduction-order ULP drift inherent to max-log (design doc §11), which
//! the byte-identity test bounds to a small ulp tolerance.
//!
//! # Default-stream vs stream-ordered demap (design doc §6)
//!
//! The erased [`Stage::process`](crate::Stage) path and the device demapper's
//! `demap_batch` run on the **default stream**; the additive
//! `demap_batch_on_stream` variant orders the launch and every transfer on a
//! caller-owned HIP stream (pinned staging, per-stream synchronize only) — the
//! route the DAG topology executor (`de160fc5`) takes for this stage's
//! `GpuOnly` dispatch on the worker's owned stream. Both paths emit
//! byte-identical LLRs.
//!
//! # CPU fallback (§8)
//!
//! The [`Stage::CpuFallback`](crate::Stage) is the in-crate `CpuGrayQamDemapper`
//! wrapper (the orphan rule forbids implementing the `gf2-sim`
//! [`Stage`](crate::Stage) trait on the foreign `gf2-coding`
//! `FastGrayQamDemapper`): `GpuGrayQamDemapper::cpu_fallback` returns it. The
//! wrapper delegates to an owned `FastGrayQamDemapper` built from the same
//! modulation + method + `noise_var`, so the Phase C executor can substitute it
//! on a GPU out-of-memory or unsupported-arch fault, and so the `ExactLogMap`
//! path has a correct CPU home.
//!
//! The module home is declared unconditionally in [`gpu`](crate::gpu); the items
//! are gated on `feature = "hip"` so the crate builds cleanly with the feature
//! off. The `GpuGrayQamDemapper`-prefixed code spans above resolve to live
//! intra-doc links only on the `--features hip` documentation build.

#[cfg(feature = "hip")]
mod imp {
    use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    use gf2_coding::modem::{DemapMethod, FastGrayQamDemapper};
    use gf2_coding::Llr;
    use gf2_kernels_hip::host::HipStream;
    use gf2_kernels_hip::{DemapStreamScratch, GpuGrayQamDemapper as KernelGpuDemapper};

    use crate::batch::{LlrBatch, SymbolBatch};
    use crate::error::StageError;
    use crate::gpu::map_hip_error;
    use crate::stage::{ExecutionClass, Stage};
    use crate::stages::GrayQamDemapCore;

    /// Bits-per-symbol (`m`) for a [`DvbT2Modulation`].
    fn bits_per_symbol(modulation: DvbT2Modulation) -> usize {
        modulation.bits_per_cell()
    }

    /// CPU Gray-QAM soft-demap stage wrapping
    /// [`FastGrayQamDemapper`] — the registered
    /// [`Stage::CpuFallback`](crate::Stage) for [`GpuGrayQamDemapper`]
    /// (design doc §8).
    ///
    /// The `Stage::CpuFallback` associated type must itself be a
    /// `Stage<SymbolBatch, LlrBatch>`; `FastGrayQamDemapper` lives in
    /// `gf2-coding` and does not (and cannot, by the orphan rule) implement this
    /// `gf2-sim` trait, so this thin wrapper carries the `Stage` impl while
    /// delegating the actual soft-demap to the shared
    /// [`stages`](crate::stages) demap kernel (`GrayQamDemapCore`, the same
    /// core behind the DVB-T2 `GrayQamDemap` and 5G NR `NrGrayQamDemap`
    /// stages). It produces the same symbol-major, MSB-first LLR layout as the
    /// GPU stage, so substituting it on a GPU fault is transparent; it is also
    /// the home of the `ExactLogMap` method (which the GPU kernel does not
    /// compute). [`demapper`](Self::demapper) exposes the underlying demapper.
    pub struct CpuGrayQamDemapper {
        core: GrayQamDemapCore,
    }

    impl CpuGrayQamDemapper {
        /// Builds a CPU Gray-QAM demap stage from the same modulation + method +
        /// `noise_var` as its paired [`GpuGrayQamDemapper`].
        ///
        /// # Arguments
        ///
        /// * `modulation` — DVB-T2 modulation order (16-QAM or 64-QAM).
        /// * `method` — exact log-MAP or max-log demapping.
        /// * `noise_var` — per-symbol total complex AWGN noise variance
        ///   (`N0 = 2 sigma^2`); must be strictly positive and finite.
        ///
        /// # Panics
        ///
        /// Panics if `noise_var` is not finite and strictly positive.
        #[must_use]
        pub fn new(modulation: DvbT2Modulation, method: DemapMethod, noise_var: f32) -> Self {
            Self {
                core: GrayQamDemapCore::new(
                    bits_per_symbol(modulation),
                    method,
                    noise_var,
                    "CpuGrayQamDemapper",
                ),
            }
        }

        /// The bits-per-symbol (`m`) this stage demaps.
        #[inline]
        #[must_use]
        pub fn bits_per_symbol(&self) -> usize {
            self.core.bits_per_symbol()
        }

        /// The demap method (exact log-MAP or max-log).
        #[inline]
        #[must_use]
        pub fn method(&self) -> DemapMethod {
            self.core.method()
        }

        /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`).
        #[inline]
        #[must_use]
        pub fn noise_var(&self) -> f32 {
            self.core.noise_var()
        }

        /// The underlying [`FastGrayQamDemapper`] this stage delegates to.
        #[inline]
        #[must_use]
        pub fn demapper(&self) -> &FastGrayQamDemapper<f32> {
            self.core.demapper()
        }
    }

    impl Stage<SymbolBatch, LlrBatch> for CpuGrayQamDemapper {
        type Scratch = ();
        type CpuFallback = Self;

        fn process(&self, input: &SymbolBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
            Ok(self.core.demap_batch(input))
        }

        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }

        fn cpu_fallback(&self) -> Option<&Self> {
            Some(self)
        }
    }

    /// GPU Gray-QAM soft-demap stage: [`SymbolBatch`] → [`LlrBatch`] (max-log
    /// LLRs in the canonical symbol-major, MSB-first layout).
    ///
    /// Holds the [`DvbT2Modulation`], the [`DemapMethod`], and the per-symbol
    /// noise variance (`N0`). The device demapper is built lazily (per
    /// [`process`](Stage::process) call) so the stage is constructible without a
    /// GPU; the throughput path builds one per-worker device demapper via
    /// [`build_demapper`](Self::build_demapper) and drives it with
    /// [`demap_batch`](Self::demap_batch).
    ///
    /// The GPU kernel computes **max-log** LLRs only. A stage constructed for
    /// [`DemapMethod::ExactLogMap`] reports [`ExecutionClass::CpuOnly`] and is
    /// served entirely by its CPU [`cpu_fallback`](Self::cpu_fallback) (the GPU
    /// has no exact-log-map kernel); a stage constructed for
    /// [`DemapMethod::MaxLog`] reports [`ExecutionClass::GpuOnly`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::gpu::demap::GpuGrayQamDemapper;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// // Constructible without a GPU; the device demapper is built lazily.
    /// let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
    /// assert_eq!(stage.m(), 4);
    /// ```
    pub struct GpuGrayQamDemapper {
        modulation: DvbT2Modulation,
        method: DemapMethod,
        m: u8,
        /// Post-normalization Gray-PAM levels (cast to f32 from the CPU
        /// demapper's `f64` table), uploaded once per device demapper build.
        pam_levels: Vec<f32>,
        noise_var: f32,
        device_id: i32,
        /// The paired CPU fallback (same modulation + method + `noise_var`),
        /// returned by [`cpu_fallback`](Self::cpu_fallback) (design doc §8). It
        /// is also the home of the `ExactLogMap` method.
        fallback: CpuGrayQamDemapper,
    }

    impl GpuGrayQamDemapper {
        /// Constructs a GPU Gray-QAM demap stage on device 0.
        ///
        /// # Arguments
        ///
        /// * `modulation` — DVB-T2 modulation order. Only 16-QAM (`m = 4`) and
        ///   64-QAM (`m = 6`) are supported.
        /// * `method` — exact log-MAP or max-log demapping. `MaxLog` runs on the
        ///   GPU; `ExactLogMap` is served by the CPU fallback (the GPU has no
        ///   exact-log-map kernel).
        /// * `noise_var` — per-symbol total complex AWGN noise variance
        ///   (`N0 = 2 sigma^2`); must be strictly positive and finite.
        ///
        /// # Panics
        ///
        /// Panics if `modulation` is not 16-QAM or 64-QAM, or if `noise_var` is
        /// not finite and strictly positive.
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::demap::GpuGrayQamDemapper;
        /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        /// use gf2_coding::modem::DemapMethod;
        ///
        /// let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.5);
        /// assert_eq!(stage.m(), 6);
        /// ```
        #[must_use]
        pub fn new(modulation: DvbT2Modulation, method: DemapMethod, noise_var: f32) -> Self {
            let m = bits_per_symbol(modulation);
            assert!(
                m == 4 || m == 6,
                "GpuGrayQamDemapper: only 16-QAM (m=4) and 64-QAM (m=6) are supported, got m={m}"
            );
            assert!(
                noise_var.is_finite() && noise_var > 0.0,
                "GpuGrayQamDemapper: noise_var must be finite and > 0, got {noise_var}"
            );
            let fallback = CpuGrayQamDemapper::new(modulation, method, noise_var);
            // Share the CPU demapper's SSOT Gray-PAM level table (cast to f32);
            // the GPU kernel never re-derives the levels.
            let pam_levels: Vec<f32> = fallback
                .demapper()
                .pam_levels()
                .iter()
                .map(|&l| l as f32)
                .collect();
            Self {
                modulation,
                method,
                m: m as u8,
                pam_levels,
                noise_var,
                device_id: 0,
                fallback,
            }
        }

        /// Targets a non-default HIP device for the device demapper.
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::demap::GpuGrayQamDemapper;
        /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        /// use gf2_coding::modem::DemapMethod;
        ///
        /// let stage =
        ///     GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25).on_device(0);
        /// assert_eq!(stage.device_id(), 0);
        /// ```
        #[must_use]
        pub fn on_device(mut self, device_id: i32) -> Self {
            self.device_id = device_id;
            self
        }

        /// The bits-per-symbol (`m`): 4 for 16-QAM, 6 for 64-QAM.
        #[inline]
        #[must_use]
        pub fn m(&self) -> u8 {
            self.m
        }

        /// The DVB-T2 modulation order.
        #[inline]
        #[must_use]
        pub fn modulation(&self) -> DvbT2Modulation {
            self.modulation
        }

        /// The demap method (exact log-MAP or max-log).
        #[inline]
        #[must_use]
        pub fn method(&self) -> DemapMethod {
            self.method
        }

        /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`).
        #[inline]
        #[must_use]
        pub fn noise_var(&self) -> f32 {
            self.noise_var
        }

        /// The HIP device the demapper targets.
        #[inline]
        #[must_use]
        pub fn device_id(&self) -> i32 {
            self.device_id
        }

        /// The post-normalization Gray-PAM level table (f32), shared with the
        /// device demapper. Length `1 << (m / 2)`.
        #[inline]
        #[must_use]
        pub fn pam_levels(&self) -> &[f32] {
            &self.pam_levels
        }

        /// Builds a per-worker device demapper sized for up to `max_batch`
        /// symbols, pre-uploading the shared Gray-PAM level table.
        ///
        /// The executor / benchmark calls this once per worker and threads the
        /// result into [`demap_batch`](Self::demap_batch), keeping the non-`Sync`
        /// device buffers out of the `Sync`-bound scratch.
        ///
        /// # Arguments
        ///
        /// * `max_batch` — the largest per-call symbol count the demapper serves.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if the device allocation or level-table upload fails.
        pub fn build_demapper(&self, max_batch: usize) -> Result<KernelGpuDemapper, StageError> {
            // is_bpsk is always false here: only 16-/64-QAM (m=4,6) are accepted
            // in `new`, so the kernel's `is_bpsk == (m == 1)` invariant holds.
            KernelGpuDemapper::new(&self.pam_levels, self.m, false, max_batch)
                .map_err(|e| map_hip_error(e, "GpuGrayQamDemapper::new"))
        }

        /// Demaps a [`SymbolBatch`] to an [`LlrBatch`] using the caller-owned
        /// device demapper, one device launch per frame.
        ///
        /// Each frame's I/Q symbols are demapped to `num_symbols * m` max-log
        /// LLRs in the canonical symbol-major, MSB-first layout. The `demapper`
        /// must have been built for this stage via
        /// [`build_demapper`](Self::build_demapper) with
        /// `max_batch >=` the largest frame's symbol count. Every frame uses the
        /// stage's constant per-symbol `noise_var` (`N0`), AWGN (no channel
        /// gains).
        ///
        /// # Arguments
        ///
        /// * `input` — the received I/Q symbol batch.
        /// * `demapper` — the per-worker device demapper.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
        ///
        /// # Complexity
        ///
        /// O(total symbols * sqrt(M) * m) device work plus per-frame H2D / D2H
        /// transfers.
        pub fn demap_batch(
            &self,
            input: &SymbolBatch,
            demapper: &KernelGpuDemapper,
        ) -> Result<LlrBatch, StageError> {
            // Not the shared CPU demap kernel (`stages::GrayQamDemapCore`):
            // only the per-frame loop shell matches — the body is the DEVICE
            // demapper (H2D upload, kernel launch, D2H readback), a different
            // compute backend (same for `demap_batch_on_stream` below).
            let m = self.m as usize;
            let mut frames = Vec::with_capacity(input.i.len());
            for (rx_i, rx_q) in input.i.iter().zip(input.q.iter()) {
                let num_symbols = rx_i.len();
                let noise_var = vec![self.noise_var; num_symbols];
                let raw = demapper
                    .demap_batch(rx_i, rx_q, None, None, &noise_var)
                    .map_err(|e| map_hip_error(e, "GpuGrayQamDemapper::demap_batch"))?;
                debug_assert_eq!(raw.len(), num_symbols * m);
                let llrs = raw.into_iter().map(Llr::new).collect();
                frames.push(llrs);
            }
            Ok(LlrBatch::new(frames))
        }

        /// Allocates the pinned host staging the stream-ordered demap variant
        /// ([`demap_batch_on_stream`](Self::demap_batch_on_stream)) requires,
        /// sized for `demapper` (a per-worker demapper from
        /// [`build_demapper`](Self::build_demapper)).
        ///
        /// One scratch per worker, like the demapper itself (both are
        /// `Send`-only, owned per worker, never shared by `&`).
        ///
        /// # Arguments
        ///
        /// * `demapper` — the per-worker device demapper the scratch pairs
        ///   with.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if a pinned allocation fails (an OOM is recoverable).
        pub fn build_stream_scratch(
            &self,
            demapper: &KernelGpuDemapper,
        ) -> Result<DemapStreamScratch, StageError> {
            demapper
                .new_stream_scratch()
                .map_err(|e| map_hip_error(e, "GpuGrayQamDemapper::new_stream_scratch"))
        }

        /// Like [`demap_batch`](Self::demap_batch), but with every kernel
        /// launch and H2D / D2H transfer ordered on the caller-owned `stream`,
        /// awaiting completion per-stream (never device-wide sync). The DAG
        /// topology executor (`de160fc5`) routes this stage's `GpuOnly`
        /// dispatch here on the worker's deterministically owned HIP stream;
        /// the LLRs are **byte-identical** to
        /// [`demap_batch`](Self::demap_batch) (same kernel, same inputs — only
        /// the queue and transfer staging differ).
        ///
        /// # Arguments
        ///
        /// Same as [`demap_batch`](Self::demap_batch), plus:
        ///
        /// * `stream` — the worker's owned HIP stream.
        /// * `scratch` — the worker's pinned staging (from
        ///   [`build_stream_scratch`](Self::build_stream_scratch)).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
        ///
        /// # Panics
        ///
        /// Panics if a frame's symbol count exceeds the demapper's `max_batch`,
        /// or if `scratch` was sized for a different demapper.
        ///
        /// # Complexity
        ///
        /// Identical to [`demap_batch`](Self::demap_batch).
        pub fn demap_batch_on_stream(
            &self,
            input: &SymbolBatch,
            demapper: &KernelGpuDemapper,
            stream: &HipStream,
            scratch: &mut DemapStreamScratch,
        ) -> Result<LlrBatch, StageError> {
            let m = self.m as usize;
            let mut frames = Vec::with_capacity(input.i.len());
            for (rx_i, rx_q) in input.i.iter().zip(input.q.iter()) {
                let num_symbols = rx_i.len();
                let noise_var = vec![self.noise_var; num_symbols];
                let raw = demapper
                    .demap_batch_on_stream(rx_i, rx_q, None, None, &noise_var, stream, scratch)
                    .map_err(|e| map_hip_error(e, "GpuGrayQamDemapper::demap_batch_on_stream"))?;
                debug_assert_eq!(raw.len(), num_symbols * m);
                let llrs = raw.into_iter().map(Llr::new).collect();
                frames.push(llrs);
            }
            Ok(LlrBatch::new(frames))
        }
    }

    impl Stage<SymbolBatch, LlrBatch> for GpuGrayQamDemapper {
        type Scratch = ();
        type CpuFallback = CpuGrayQamDemapper;

        /// Demaps `input` by building a one-shot device demapper sized for the
        /// largest frame and running the max-log kernel per frame. For
        /// [`DemapMethod::ExactLogMap`] (which the GPU kernel does not compute)
        /// this delegates to the CPU [`cpu_fallback`](Self::cpu_fallback). The
        /// throughput path is [`demap_batch`](Self::demap_batch) with a
        /// caller-owned per-worker demapper (the device buffers cannot live in
        /// the `Sync`-bound scratch). An empty batch is a no-op (no device
        /// demapper is built).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
        fn process(&self, input: &SymbolBatch, scratch: &mut ()) -> Result<LlrBatch, StageError> {
            // ExactLogMap has no GPU kernel — serve it on the CPU fallback.
            if self.method == DemapMethod::ExactLogMap {
                return self.fallback.process(input, scratch);
            }
            let max_symbols = input.i.iter().map(Vec::len).max().unwrap_or(0);
            if max_symbols == 0 {
                return Ok(LlrBatch::new(vec![Vec::new(); input.i.len()]));
            }
            let demapper = self.build_demapper(max_symbols)?;
            self.demap_batch(input, &demapper)
        }

        /// 16-/64-QAM `MaxLog` runs on the GPU; `ExactLogMap` is CPU-only (the
        /// GPU kernel computes max-log only).
        fn execution_class(&self) -> ExecutionClass {
            match self.method {
                DemapMethod::MaxLog => ExecutionClass::GpuOnly,
                DemapMethod::ExactLogMap => ExecutionClass::CpuOnly,
            }
        }

        /// The paired CPU [`CpuGrayQamDemapper`] fallback (design doc §8): same
        /// modulation + method + `noise_var`.
        fn cpu_fallback(&self) -> Option<&CpuGrayQamDemapper> {
            Some(&self.fallback)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_new_qam16_m_and_pam_levels() {
            let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
            assert_eq!(stage.m(), 4);
            // 16-QAM has 2 bits/axis → 4 PAM levels.
            assert_eq!(stage.pam_levels().len(), 4);
        }

        #[test]
        fn test_new_qam64_m_and_pam_levels() {
            let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.5);
            assert_eq!(stage.m(), 6);
            // 64-QAM has 3 bits/axis → 8 PAM levels.
            assert_eq!(stage.pam_levels().len(), 8);
        }

        #[test]
        #[should_panic(expected = "only 16-QAM")]
        fn test_new_rejects_qpsk() {
            let _ = GpuGrayQamDemapper::new(DvbT2Modulation::Qpsk, DemapMethod::MaxLog, 0.25);
        }

        #[test]
        #[should_panic(expected = "noise_var must be finite and > 0")]
        fn test_new_rejects_nonpositive_noise_var() {
            let _ = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.0);
        }

        #[test]
        fn test_max_log_is_gpu_only() {
            let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
            assert_eq!(stage.execution_class(), ExecutionClass::GpuOnly);
        }

        #[test]
        fn test_exact_log_map_is_cpu_only() {
            // ExactLogMap has no GPU kernel — the stage reports CpuOnly so the
            // executor never routes it to a non-existent GPU exact path.
            let stage =
                GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::ExactLogMap, 0.25);
            assert_eq!(stage.execution_class(), ExecutionClass::CpuOnly);
        }

        #[test]
        fn test_cpu_fallback_has_same_parameters() {
            let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.7);
            let fb = stage.cpu_fallback().expect("GPU stage has a CPU fallback");
            assert_eq!(fb.bits_per_symbol(), 6);
            assert_eq!(fb.method(), DemapMethod::MaxLog);
            assert_eq!(fb.noise_var(), 0.7);
        }

        /// ExactLogMap on the GPU stage's erased `process` path must produce the
        /// SAME LLRs as the CPU fallback (it delegates), with no GPU required.
        #[test]
        fn test_exact_log_map_process_matches_cpu_fallback_no_gpu() {
            let stage =
                GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::ExactLogMap, 0.3);
            let i = vec![vec![0.4_f32, -0.7, 0.1, 0.9]];
            let q = vec![vec![-0.2_f32, 0.6, -0.5, 0.3]];
            let input = SymbolBatch::new(i, q);

            let via_stage = stage.process(&input, &mut ()).expect("process");
            let via_fallback = stage
                .cpu_fallback()
                .unwrap()
                .process(&input, &mut ())
                .expect("fallback process");
            assert_eq!(via_stage, via_fallback);
        }

        /// The stage and its CPU fallback must be `Send` (per-worker-owned) so
        /// the executor can move them between rayon workers.
        #[test]
        fn test_stage_and_fallback_are_send() {
            fn assert_send<T: Send>() {}
            assert_send::<GpuGrayQamDemapper>();
            assert_send::<CpuGrayQamDemapper>();
        }

        /// The stream-ordered stage path (`demap_batch_on_stream`, the topology
        /// executor's `GpuOnly` route) must emit LLRs **byte-identical** to the
        /// default-stream `demap_batch` path: same kernel, same inputs, only
        /// the queue and transfer staging differ. Skips with no GPU; fast tier.
        #[test]
        fn test_demap_on_stream_matches_default_stream() {
            use gf2_kernels_hip::host::{device_mem_info, HipStream};

            if device_mem_info().is_err() {
                eprintln!("skipping test_demap_on_stream_matches_default_stream: no usable GPU");
                return;
            }

            let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
            let n = 48usize;
            let i: Vec<f32> = (0..n).map(|k| 0.09 * k as f32 - 2.0).collect();
            let q: Vec<f32> = (0..n).map(|k| 1.7 - 0.06 * k as f32).collect();
            // Two frames so the per-frame loop indexes 0 and 1 distinctly.
            let input = SymbolBatch::new(vec![i.clone(), i], vec![q.clone(), q]);

            let demapper = stage.build_demapper(n).expect("device demapper");
            let default = stage
                .demap_batch(&input, &demapper)
                .expect("default-stream demap");

            let stream = HipStream::new().expect("create stream");
            let mut scratch = stage
                .build_stream_scratch(&demapper)
                .expect("pinned staging");
            let streamed = stage
                .demap_batch_on_stream(&input, &demapper, &stream, &mut scratch)
                .expect("stream-ordered demap");

            assert_eq!(default.frames.len(), streamed.frames.len());
            for (f, (df, sf)) in default
                .frames
                .iter()
                .zip(streamed.frames.iter())
                .enumerate()
            {
                assert_eq!(df.len(), sf.len(), "frame {f} LLR count");
                for (b, (d, s)) in df.iter().zip(sf.iter()).enumerate() {
                    // Bit-level comparison (f32 `==` would conflate -0.0 / 0.0).
                    assert_eq!(
                        d.value().to_bits(),
                        s.value().to_bits(),
                        "frame={f} LLR[{b}] differs: default={} stream={}",
                        d.value(),
                        s.value()
                    );
                }
            }
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::{CpuGrayQamDemapper, GpuGrayQamDemapper};
