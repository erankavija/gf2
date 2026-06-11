//! GPU AWGN channel stage (design doc §3 / §8 / §11, `feature = "hip"`).
//!
//! [`GpuAwgn`] is the device-accelerated counterpart of the CPU
//! [`channels::Awgn`](crate::channels::Awgn) stage. It adds circularly-symmetric
//! complex Gaussian noise to every I/Q symbol in a
//! [`SymbolBatch`](crate::SymbolBatch), drawing the noise from the device
//! ChaCha20 + Box-Muller kernel (`gf2-kernels-hip`'s
//! [`GpuChaChaAwgn`](gf2_kernels_hip::GpuChaChaAwgn)) so the raw ChaCha word
//! stream is byte-identical to the CPU path at the same per-frame
//! `worker_offset(...)` (criterion 1) and the resulting noise samples agree
//! with the CPU to <= 1 ulp f32 (criterion 2, design doc §11 GPU softmath).
//!
//! # Per-frame seek contract (§3)
//!
//! Each frame `f` in the batch draws its noise from the device kernel seeded to
//! the §3 word offset `worker_offset(seed, snr_idx, worker_idx, f)`, computed by
//! the same [`worker_offset`](crate::parallel::worker_offset) used CPU-side. The
//! kernel emits `2 * num_symbols` standard-normal samples per frame as a flat
//! array, in the exact ChaCha-word order the CPU `draw_standard_normal`
//! consumes (4 words per sample); the host assigns them **planar** — sample `k`
//! is symbol `k`'s I-axis noise, sample `num_symbols + k` is symbol `k`'s
//! Q-axis noise — matching the CPU [`Awgn`](crate::channels::Awgn) stage's SSOT
//! draw order (all I, then all Q; see the CPU stage's module docs). The noise
//! is a pure function of the frame index — byte-identical across worker counts.
//!
//! # CPU fallback (§8)
//!
//! The [`Stage::CpuFallback`](crate::Stage) is the CPU
//! [`Awgn`](crate::channels::Awgn): [`cpu_fallback`](GpuAwgn::cpu_fallback)
//! returns a CPU stage with the *same* `es_n0_db` / `bits_per_symbol`, so the
//! Phase C executor can substitute it on a GPU out-of-memory or unsupported-arch
//! fault. The CPU fallback draws from the same `worker_offset`-seeked
//! `ChaCha20Rng` stream, so a fallback frame is byte-identical (raw words) to
//! what the GPU would have drawn — only the post-Box-Muller sample value differs
//! by <= 1 ulp f32.
//!
//! The module home is declared unconditionally in [`gpu`](crate::gpu); the items
//! are gated on `feature = "hip"` so the crate builds cleanly with the feature
//! off.

#[cfg(feature = "hip")]
mod imp {
    use gf2_kernels_hip::GpuChaChaAwgn;

    use crate::batch::SymbolBatch;
    use crate::channels::Awgn;
    use crate::error::StageError;
    use crate::gpu::map_hip_error;
    use crate::parallel::worker_offset;
    use crate::stage::{ExecutionClass, Stage};

    /// Per-stage scratch for [`GpuAwgn`].
    ///
    /// Holds only a reusable host-side `Vec<f32>` for the D2H noise read-back.
    /// It deliberately does **not** own the device noise generator
    /// ([`GpuChaChaAwgn`], which owns non-`Sync` device buffers): the
    /// [`Stage::Scratch`](crate::Stage) bound requires `Send + Sync`, and the
    /// per-worker-owned device generator is threaded into
    /// [`apply_for_frame`](GpuAwgn::apply_for_frame) by reference instead (the
    /// executor / benchmark owns one generator per worker, never shared by `&`).
    /// The host buffer keeps the read-back allocation amortised across frames.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::gpu::awgn::GpuAwgnScratch;
    ///
    /// let scratch = GpuAwgnScratch::default();
    /// assert!(scratch.host_buf().is_empty());
    /// ```
    #[derive(Default)]
    pub struct GpuAwgnScratch {
        host_buf: Vec<f32>,
    }

    impl GpuAwgnScratch {
        /// The reusable host read-back buffer (grown as needed by the erased
        /// [`Stage::process`](crate::Stage) path).
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::awgn::GpuAwgnScratch;
        ///
        /// assert!(GpuAwgnScratch::default().host_buf().is_empty());
        /// ```
        #[must_use]
        pub fn host_buf(&self) -> &[f32] {
            &self.host_buf
        }
    }

    /// GPU AWGN channel stage: adds device-drawn complex Gaussian noise to a
    /// [`SymbolBatch`].
    ///
    /// The per-axis noise standard deviation is
    /// `sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))` (the SSOT
    /// [`es_n0_db_to_sigma`](crate::channels) formula, shared with the CPU
    /// [`Awgn`]). Each frame's noise is drawn from the device ChaCha20 +
    /// Box-Muller kernel seeded to the frame's §3 `worker_offset`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::gpu::awgn::GpuAwgn;
    ///
    /// // Constructible without a GPU; the device generator is built lazily in
    /// // `process` / `apply_for_frame`.
    /// let ch = GpuAwgn::new(6.25, 4);
    /// assert_eq!(ch.bits_per_symbol(), 4);
    /// ```
    #[derive(Debug, Clone)]
    pub struct GpuAwgn {
        es_n0_db: f32,
        bits_per_symbol: usize,
        sigma: f32,
        /// Base seed + (snr_idx, worker_idx) the device kernel seeds from; the
        /// per-frame offset is added on top via `worker_offset`.
        seed: u64,
        snr_idx: usize,
        worker_idx: usize,
        device_id: i32,
        /// The paired CPU fallback (same parameters), returned by
        /// [`cpu_fallback`](Self::cpu_fallback) (design doc §8).
        fallback: Awgn,
    }

    impl GpuAwgn {
        /// Constructs a GPU AWGN stage seeding from `(seed=0, snr_idx=0,
        /// worker_idx=0)` on device 0.
        ///
        /// Use [`with_seek`](Self::with_seek) to set the §3 seek parameters and
        /// [`on_device`](Self::on_device) to target a non-default device. The
        /// device generator is built lazily on first
        /// [`process`](Stage::process) / [`apply_for_frame`](Self::apply_for_frame).
        ///
        /// # Arguments
        ///
        /// * `es_n0_db` — channel Es/N0 in dB.
        /// * `bits_per_symbol` — modulation order in bits/symbol.
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::awgn::GpuAwgn;
        ///
        /// let ch = GpuAwgn::new(10.0, 4);
        /// assert_eq!(ch.bits_per_symbol(), 4);
        /// ```
        #[must_use]
        pub fn new(es_n0_db: f32, bits_per_symbol: usize) -> Self {
            let sigma = crate::channels::es_n0_db_to_sigma(es_n0_db);
            Self {
                es_n0_db,
                bits_per_symbol,
                sigma,
                seed: 0,
                snr_idx: 0,
                worker_idx: 0,
                device_id: 0,
                fallback: Awgn::new(es_n0_db, bits_per_symbol),
            }
        }

        /// Sets the §3 seek parameters `(seed, snr_idx, worker_idx)` the device
        /// kernel seeds each frame's noise from.
        ///
        /// # Arguments
        ///
        /// * `seed` — base RNG seed (selects the ChaCha stream).
        /// * `snr_idx` — zero-based SNR-point index.
        /// * `worker_idx` — zero-based worker partition index (the CPU within-SNR
        ///   path uses `0` and keys per-frame by global frame index, design §3).
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::awgn::GpuAwgn;
        ///
        /// let ch = GpuAwgn::new(6.25, 4).with_seek(42, 1, 0);
        /// assert_eq!(ch.seed(), 42);
        /// ```
        #[must_use]
        pub fn with_seek(mut self, seed: u64, snr_idx: usize, worker_idx: usize) -> Self {
            self.seed = seed;
            self.snr_idx = snr_idx;
            self.worker_idx = worker_idx;
            self
        }

        /// Targets a non-default HIP device for the noise generator.
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_sim::gpu::awgn::GpuAwgn;
        ///
        /// let ch = GpuAwgn::new(6.25, 4).on_device(0);
        /// assert_eq!(ch.device_id(), 0);
        /// ```
        #[must_use]
        pub fn on_device(mut self, device_id: i32) -> Self {
            self.device_id = device_id;
            self
        }

        /// The Es/N0 in dB this channel was constructed with.
        #[inline]
        #[must_use]
        pub fn es_n0_db(&self) -> f32 {
            self.es_n0_db
        }

        /// The modulation order in bits/symbol.
        #[inline]
        #[must_use]
        pub fn bits_per_symbol(&self) -> usize {
            self.bits_per_symbol
        }

        /// Per-axis noise standard deviation `sigma = sqrt(1 / (2 * 10^(Es/N0_dB/10)))`.
        #[inline]
        #[must_use]
        pub fn sigma(&self) -> f32 {
            self.sigma
        }

        /// The base seed the device kernel seeds from.
        #[inline]
        #[must_use]
        pub fn seed(&self) -> u64 {
            self.seed
        }

        /// The HIP device the noise generator targets.
        #[inline]
        #[must_use]
        pub fn device_id(&self) -> i32 {
            self.device_id
        }

        /// Adds GPU-drawn AWGN noise to a frame's symbols in-place using the
        /// caller-owned device generator `gen`, seeking it to frame `frame_idx`'s
        /// §3 `worker_offset` region.
        ///
        /// Draws `2 * num_symbols` standard-normal samples on the device (in the
        /// CPU `draw_standard_normal` word order), scales each by
        /// [`sigma`](Self::sigma), and adds them to the I/Q lanes planar-wise
        /// (sample `k` → I of symbol `k`, sample `num_symbols + k` → Q of
        /// symbol `k`), matching the CPU stage's SSOT draw order.
        /// The `gen` must have been built for the **same** `seed` this stage was
        /// configured with ([`with_seek`](Self::with_seek)); it is owned by the
        /// caller (one per worker) so the non-`Sync` device buffers stay out of
        /// the `Sync`-bound [`Scratch`](Stage::Scratch).
        ///
        /// # Arguments
        ///
        /// * `i_lane` / `q_lane` — the frame's in-phase / quadrature samples
        ///   (equal length; corrupted in place).
        /// * `frame_idx` — the frame index to seek to (the global frame index for
        ///   the within-SNR path).
        /// * `gen` — the per-worker device noise generator (capacity must be
        ///   `>= 2 * i_lane.len()`).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via
        /// [`map_hip_error`](crate::gpu::map_hip_error)) on a device fault — an
        /// OOM or unsupported arch is recoverable (executor substitutes the CPU
        /// fallback), any other HIP failure is fatal.
        ///
        /// # Panics
        ///
        /// Panics if `i_lane.len() != q_lane.len()`, or if `gen`'s capacity is
        /// less than `2 * i_lane.len()`.
        ///
        /// # Complexity
        ///
        /// O(N) host-side over the frame's N symbols, plus one device launch.
        pub fn apply_for_frame(
            &self,
            i_lane: &mut [f32],
            q_lane: &mut [f32],
            frame_idx: usize,
            gen: &GpuChaChaAwgn,
        ) -> Result<(), StageError> {
            assert_eq!(
                i_lane.len(),
                q_lane.len(),
                "GpuAwgn::apply_for_frame: I lane length ({}) != Q lane length ({})",
                i_lane.len(),
                q_lane.len()
            );
            let num_symbols = i_lane.len();
            if num_symbols == 0 {
                return Ok(());
            }
            let n_samples = 2 * num_symbols; // I and Q per symbol.
            let base = worker_offset(self.seed, self.snr_idx, self.worker_idx, frame_idx);

            let sigma = self.sigma;
            let noise = gen
                .noise_samples(base, n_samples)
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::noise_samples"))?;

            // Planar assignment (the CPU stage's SSOT draw order): sample k is
            // symbol k's I-axis noise; sample num_symbols + k is its Q-axis
            // noise.
            for (k, (xi, xq)) in i_lane.iter_mut().zip(q_lane.iter_mut()).enumerate() {
                *xi += noise[k] * sigma;
                *xq += noise[num_symbols + k] * sigma;
            }
            Ok(())
        }

        /// Builds a per-worker device noise generator sized for `max_symbols`
        /// symbols (`2 * max_symbols` samples), seeded from this stage's `seed`.
        ///
        /// The executor / benchmark calls this once per worker and threads the
        /// result into [`apply_for_frame`](Self::apply_for_frame), keeping the
        /// non-`Sync` device buffers out of the `Sync`-bound scratch.
        ///
        /// # Arguments
        ///
        /// * `max_symbols` — the largest per-frame symbol count the generator
        ///   must serve (sizes the device output buffer).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if the device allocation or key upload fails.
        pub fn build_generator(&self, max_symbols: usize) -> Result<GpuChaChaAwgn, StageError> {
            GpuChaChaAwgn::new(self.seed, self.device_id, 2 * max_symbols)
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::new"))
        }

        /// Adds GPU-drawn AWGN noise to every frame in `batch` using the
        /// caller-owned generator `gen`, seeking each frame `f` to its §3
        /// `worker_offset(.., f)` region.
        ///
        /// # Arguments
        ///
        /// * `batch` — the IQ symbol batch to corrupt in-place.
        /// * `gen` — the per-worker device noise generator (capacity must cover
        ///   `2 *` the largest frame's symbol count).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on any per-frame device fault (see
        /// [`apply_for_frame`](Self::apply_for_frame)).
        ///
        /// # Complexity
        ///
        /// O(total symbols) host-side plus one device launch per frame.
        pub fn apply(
            &self,
            batch: &mut SymbolBatch,
            gen: &GpuChaChaAwgn,
        ) -> Result<(), StageError> {
            for (f, (i_frame, q_frame)) in batch.i.iter_mut().zip(batch.q.iter_mut()).enumerate() {
                self.apply_for_frame(i_frame, q_frame, f, gen)?;
            }
            Ok(())
        }

        /// Like [`apply_for_frame`](Self::apply_for_frame) but reads the device
        /// noise back into the caller-provided `host_buf` (resized as needed),
        /// avoiding a per-frame allocation. Used by [`process`](Stage::process)
        /// to make the [`Scratch`](Stage::Scratch) read-back buffer functional.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (see
        /// [`apply_for_frame`](Self::apply_for_frame)).
        ///
        /// # Panics
        ///
        /// Panics if `i_lane.len() != q_lane.len()`.
        fn apply_for_frame_with_buf(
            &self,
            i_lane: &mut [f32],
            q_lane: &mut [f32],
            frame_idx: usize,
            gen: &GpuChaChaAwgn,
            host_buf: &mut Vec<f32>,
        ) -> Result<(), StageError> {
            assert_eq!(
                i_lane.len(),
                q_lane.len(),
                "GpuAwgn::apply_for_frame_with_buf: I lane length ({}) != Q lane length ({})",
                i_lane.len(),
                q_lane.len()
            );
            let num_symbols = i_lane.len();
            if num_symbols == 0 {
                return Ok(());
            }
            let n_samples = 2 * num_symbols;
            if host_buf.len() < n_samples {
                host_buf.resize(n_samples, 0.0);
            }
            let base = worker_offset(self.seed, self.snr_idx, self.worker_idx, frame_idx);
            gen.noise_samples_into(base, &mut host_buf[..n_samples])
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::noise_samples_into"))?;
            let sigma = self.sigma;
            // Planar assignment, matching `apply_for_frame`.
            for (k, (xi, xq)) in i_lane.iter_mut().zip(q_lane.iter_mut()).enumerate() {
                *xi += host_buf[k] * sigma;
                *xq += host_buf[num_symbols + k] * sigma;
            }
            Ok(())
        }
    }

    impl Stage<SymbolBatch, SymbolBatch> for GpuAwgn {
        type Scratch = GpuAwgnScratch;
        type CpuFallback = Awgn;

        /// Adds GPU AWGN noise to a copy of `input`, drawing from a freshly-built
        /// device generator; each frame is seeked to its §3 `worker_offset`
        /// region. The per-frame device noise is read back into
        /// `scratch.host_buf` (reused across frames and calls).
        ///
        /// This erased-`Stage` path builds a generator per call (the device
        /// buffers cannot live in the `Sync`-bound scratch). The throughput path
        /// is [`apply`](Self::apply) with a caller-owned per-worker generator. An
        /// empty batch is a no-op (no device generator is built).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch so the executor substitutes
        /// [`cpu_fallback`](Self::cpu_fallback); fatal otherwise).
        fn process(
            &self,
            input: &SymbolBatch,
            scratch: &mut GpuAwgnScratch,
        ) -> Result<SymbolBatch, StageError> {
            let max_symbols = input.i.iter().map(Vec::len).max().unwrap_or(0);
            if max_symbols == 0 {
                return Ok(input.clone());
            }
            let gen = self.build_generator(max_symbols)?;
            let mut out = input.clone();
            for (f, (i_frame, q_frame)) in out.i.iter_mut().zip(out.q.iter_mut()).enumerate() {
                self.apply_for_frame_with_buf(i_frame, q_frame, f, &gen, &mut scratch.host_buf)?;
            }
            Ok(out)
        }

        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::GpuOnly
        }

        /// The paired CPU [`Awgn`] fallback (design doc §8): same `es_n0_db` /
        /// `bits_per_symbol`, drawing from the same `worker_offset`-seeked
        /// stream so a substituted frame's raw words are byte-identical.
        fn cpu_fallback(&self) -> Option<&Awgn> {
            Some(&self.fallback)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_new_sigma_matches_cpu_formula() {
            let gpu = GpuAwgn::new(6.25, 4);
            let cpu = Awgn::new(6.25, 4);
            assert_eq!(gpu.sigma(), cpu.sigma(), "GPU/CPU sigma must use the SSOT");
        }

        #[test]
        fn test_cpu_fallback_has_same_parameters() {
            let gpu = GpuAwgn::new(7.5, 6);
            let fb = gpu.cpu_fallback().expect("GPU stage has a CPU fallback");
            assert_eq!(fb.es_n0_db(), 7.5);
            assert_eq!(fb.bits_per_symbol(), 6);
            assert_eq!(fb.sigma(), gpu.sigma());
        }

        #[test]
        fn test_execution_class_is_gpu_only() {
            assert_eq!(
                GpuAwgn::new(6.0, 4).execution_class(),
                ExecutionClass::GpuOnly
            );
        }

        #[test]
        fn test_with_seek_and_on_device_setters() {
            let gpu = GpuAwgn::new(6.0, 4).with_seek(99, 3, 2).on_device(0);
            assert_eq!(gpu.seed(), 99);
            assert_eq!(gpu.device_id(), 0);
        }

        /// The stage and its scratch must be `Send` (per-worker-owned) so the
        /// executor can move them between rayon workers; the scratch is NOT
        /// required to be `Sync` (it owns device buffers).
        #[test]
        fn test_stage_and_scratch_are_send() {
            fn assert_send<T: Send>() {}
            assert_send::<GpuAwgn>();
            assert_send::<GpuAwgnScratch>();
        }

        /// **Criterion 1 / deliverable 4 (full-range raw byte-identity).** For
        /// N ∈ {1, 256, 1024} frames the device ChaCha20 raw 32-bit word stream
        /// at **every** frame's `worker_offset(...)` must be bit-for-bit
        /// identical to a host `ChaCha20Rng::seed_from_u64(seed)` repositioned
        /// with `set_word_pos(worker_offset(...))` and read via `next_u32`. This
        /// checks the **entire** word range of **every** frame (not first/mid/
        /// last spot-checks), so a seek-arithmetic bug inside a range cannot
        /// hide.
        ///
        /// Uses the **real** [`gf2_sim::parallel::worker_offset`](crate::parallel::worker_offset)
        /// — no duplicated seek scheme (this test lives in `gf2-sim`, which
        /// depends on both the seek SSOT and the GPU launch wrappers). Skips
        /// cleanly with no GPU.
        ///
        /// Timing: 1 + 256 + 1024 = 1281 small launches (32 words each ≈ two
        /// ChaCha blocks), well under the 5 s fast-tier limit (measured ~0.1 s).
        #[test]
        fn test_gpu_chacha_raw_words_full_range_byte_identical() {
            use crate::parallel::worker_offset;
            use gf2_kernels_hip::host::device_mem_info;
            use gf2_kernels_hip::GpuChaChaAwgn;
            use rand::RngCore as _;
            use rand::SeedableRng as _;
            use rand_chacha::ChaCha20Rng;

            if device_mem_info().is_err() {
                eprintln!(
                    "skipping test_gpu_chacha_raw_words_full_range_byte_identical: no usable GPU"
                );
                return;
            }

            let seed = 0xDEAD_BEEF_u64;
            let snr_idx = 2usize;
            let worker_idx = 0usize;
            // 32 words/frame spans two ChaCha blocks (16 words/block), exercising
            // the device block cache across a block boundary every frame.
            let words_per_frame = 32usize;

            let gen = GpuChaChaAwgn::new(seed, 0, words_per_frame).expect("build generator");
            let mut host = ChaCha20Rng::seed_from_u64(seed);

            for &n_frames in &[1usize, 256, 1024] {
                // EVERY frame in 0..n_frames, EVERY word in the frame.
                for frame_idx in 0..n_frames {
                    let base = worker_offset(seed, snr_idx, worker_idx, frame_idx);
                    let gpu_words = gen.raw_words(base, words_per_frame).expect("gpu raw words");

                    host.set_word_pos(base);
                    for (w, &gpu_w) in gpu_words.iter().enumerate() {
                        let host_w = host.next_u32();
                        assert_eq!(
                            gpu_w, host_w,
                            "raw word mismatch at N={n_frames} frame={frame_idx} word={w}: \
                             gpu={gpu_w:#010x} host={host_w:#010x}"
                        );
                    }
                }
            }
        }

        /// **Criterion 2 (≤ 1 ulp over ≥ 1024 frames).** For **every** frame in
        /// `0..1024`, every device Box-Muller standard-normal sample must agree
        /// with the host `box_muller_cos` (the `gf2-coding` SSOT, fed the same
        /// two `f64` uniforms in the same `draw_standard_normal` order) to
        /// **≤ 1 ulp f32**. This genuinely covers ≥ 1024 frames of samples (a
        /// dense per-frame set), not a handful of spot frames.
        ///
        /// Uses the **real** [`gf2_sim::parallel::worker_offset`](crate::parallel::worker_offset).
        /// Skips cleanly with no GPU.
        ///
        /// Timing: 1024 small launches (64 samples = 256 words each), well under
        /// the 5 s fast-tier limit (measured ~0.1 s).
        #[test]
        fn test_gpu_box_muller_within_1_ulp_over_1024_frames() {
            use crate::parallel::worker_offset;
            use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;
            use gf2_kernels_hip::host::device_mem_info;
            use gf2_kernels_hip::GpuChaChaAwgn;
            use rand::Rng as _;
            use rand::SeedableRng as _;
            use rand_chacha::ChaCha20Rng;

            if device_mem_info().is_err() {
                eprintln!(
                    "skipping test_gpu_box_muller_within_1_ulp_over_1024_frames: no usable GPU"
                );
                return;
            }

            let seed = 0x0102_0304_0506_0708_u64;
            let snr_idx = 0usize;
            let worker_idx = 0usize;
            // 64 standard-normal samples/frame (256 words, well under FRAME_STRIDE);
            // a dense representative set per frame.
            let samples_per_frame = 64usize;
            let n_frames = 1024usize;

            let gen = GpuChaChaAwgn::new(seed, 0, samples_per_frame).expect("build generator");
            let mut host = ChaCha20Rng::seed_from_u64(seed);

            for frame_idx in 0..n_frames {
                let base = worker_offset(seed, snr_idx, worker_idx, frame_idx);
                let gpu = gen
                    .noise_samples(base, samples_per_frame)
                    .expect("gpu noise");

                host.set_word_pos(base);
                for (s, &gpu_n) in gpu.iter().enumerate() {
                    // Host: two f64 uniforms then box_muller_cos (the SSOT), the
                    // exact order `gf2_sim::channels::draw_standard_normal` uses.
                    let u1: f64 = host.random();
                    let u2: f64 = host.random();
                    let host_n = box_muller_cos(u1, u2);
                    assert!(
                        ulps_within_one(gpu_n, host_n),
                        "Box-Muller sample frame={frame_idx} s={s} differs > 1 ulp: \
                         gpu={gpu_n} host={host_n}"
                    );
                }
            }
        }

        /// End-to-end on the gfx1030 host: the `GpuAwgn` stage noise must match
        /// the CPU `channels::Awgn` applied to the same input, frame-seeked to
        /// the same `worker_offset`, to ≤ 1 ulp f32 per sample (criterion 2),
        /// across a **multi-frame span** (every frame in `0..256`). This is the
        /// stage-level end-to-end check; the dense ≥ 1024-frame raw-stream and
        /// Box-Muller regressions are the dedicated tests above. Skips cleanly
        /// if no usable GPU is present.
        ///
        /// Timing: 256 CPU+GPU frame pairs of 512 symbols, well under the 5 s
        /// fast-tier limit.
        #[test]
        fn test_gpu_awgn_matches_cpu_within_1_ulp() {
            use crate::parallel::WorkerCtx;
            use gf2_kernels_hip::host::device_mem_info;

            if device_mem_info().is_err() {
                eprintln!("skipping test_gpu_awgn_matches_cpu_within_1_ulp: no usable GPU");
                return;
            }

            let seed = 0xC0FFEE_u64;
            let snr_idx = 1usize;
            let num_symbols = 512usize;
            let n_frames = 256usize;

            let i0: Vec<f32> = (0..num_symbols).map(|k| (k as f32) * 0.01 - 2.0).collect();
            let q0: Vec<f32> = (0..num_symbols).map(|k| 1.0 - (k as f32) * 0.005).collect();

            let cpu = Awgn::new(6.5, 4);
            // GPU path: one per-worker generator reused across frames.
            let gpu = GpuAwgn::new(6.5, 4).with_seek(seed, snr_idx, 0);
            let gen = gpu.build_generator(num_symbols).expect("build generator");

            for frame_idx in 0..n_frames {
                // CPU reference: seek a WorkerCtx to this frame and apply CPU Awgn.
                let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
                ctx.reseek_to_frame(frame_idx);
                let mut cpu_batch = SymbolBatch::new(vec![i0.clone()], vec![q0.clone()]);
                cpu.apply(&mut cpu_batch, ctx.rng_mut());
                let cpu_i = &cpu_batch.i[0];
                let cpu_q = &cpu_batch.q[0];

                // GPU: same seek parameters, apply to a copy.
                let mut gpu_i = i0.clone();
                let mut gpu_q = q0.clone();
                gpu.apply_for_frame(&mut gpu_i, &mut gpu_q, frame_idx, &gen)
                    .expect("gpu awgn frame");

                for k in 0..num_symbols {
                    assert!(
                        ulps_within_one(cpu_i[k], gpu_i[k]),
                        "frame={frame_idx} I[{k}] CPU={} GPU={} differ by > 1 ulp",
                        cpu_i[k],
                        gpu_i[k]
                    );
                    assert!(
                        ulps_within_one(cpu_q[k], gpu_q[k]),
                        "frame={frame_idx} Q[{k}] CPU={} GPU={} differ by > 1 ulp",
                        cpu_q[k],
                        gpu_q[k]
                    );
                }
            }
        }

        /// The erased `Stage::process` path (which reads back through
        /// `scratch.host_buf`) must produce the same per-frame noise as the
        /// per-worker `apply_for_frame` path, frame-for-frame. Confirms the
        /// scratch read-back buffer is wired correctly. Skips with no GPU.
        #[test]
        fn test_process_matches_apply_for_frame() {
            use crate::stage::Stage;
            use gf2_kernels_hip::host::device_mem_info;

            if device_mem_info().is_err() {
                eprintln!("skipping test_process_matches_apply_for_frame: no usable GPU");
                return;
            }

            let seed = 0xABCD_1234_u64;
            let num_symbols = 300usize;
            let i0: Vec<f32> = vec![0.5; num_symbols];
            let q0: Vec<f32> = vec![-0.25; num_symbols];
            // Two frames so the process loop indexes frame 0 and 1 distinctly.
            let input =
                SymbolBatch::new(vec![i0.clone(), i0.clone()], vec![q0.clone(), q0.clone()]);

            let gpu = GpuAwgn::new(6.5, 4).with_seek(seed, 0, 0);

            // process() path (reads back via scratch.host_buf).
            let mut scratch = GpuAwgnScratch::default();
            let via_process = gpu.process(&input, &mut scratch).expect("process");

            // apply_for_frame() path (per-worker generator, fresh Vec read-back).
            let gen = gpu.build_generator(num_symbols).expect("generator");
            let mut ref_i0 = i0.clone();
            let mut ref_q0 = q0.clone();
            gpu.apply_for_frame(&mut ref_i0, &mut ref_q0, 0, &gen)
                .expect("frame 0");
            let mut ref_i1 = i0.clone();
            let mut ref_q1 = q0.clone();
            gpu.apply_for_frame(&mut ref_i1, &mut ref_q1, 1, &gen)
                .expect("frame 1");

            assert_eq!(via_process.i[0], ref_i0, "frame 0 I lane mismatch");
            assert_eq!(via_process.q[0], ref_q0, "frame 0 Q lane mismatch");
            assert_eq!(via_process.i[1], ref_i1, "frame 1 I lane mismatch");
            assert_eq!(via_process.q[1], ref_q1, "frame 1 Q lane mismatch");
            assert!(scratch.host_buf().len() >= 2 * num_symbols);
        }

        /// True if `a` and `b` are within one f32 ulp.
        ///
        /// Maps each float to a monotone `i64` ordering key (sign-magnitude →
        /// two's-complement-like, so adjacent representable floats differ by 1)
        /// and checks the keys differ by at most 1.
        fn ulps_within_one(a: f32, b: f32) -> bool {
            if a == b {
                return true;
            }
            if a.is_nan() || b.is_nan() {
                return false;
            }
            // Monotone key: non-negative floats keep their bit pattern; negative
            // floats are flipped to order below zero. Adjacent floats → keys ±1.
            let key = |x: f32| -> i64 {
                let bits = i64::from(x.to_bits());
                if x.to_bits() & 0x8000_0000 != 0 {
                    // Negative: map to a descending range below 0.
                    -(bits & 0x7fff_ffff)
                } else {
                    bits
                }
            };
            (key(a) - key(b)).abs() <= 1
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::{GpuAwgn, GpuAwgnScratch};
