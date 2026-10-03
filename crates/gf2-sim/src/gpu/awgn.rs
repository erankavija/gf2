//! GPU AWGN channel stage (`feature = "hip"`).
//!
//! `GpuAwgn` is the device counterpart of the CPU
//! [`channels::Awgn`](crate::channels::Awgn) stage. Frame `f` draws its noise
//! from the device ChaCha20 + Box-Muller kernel (`GpuChaChaAwgn`) at word offset
//! [`worker_offset(seed, snr_idx, worker_idx, f)`](crate::parallel::worker_offset),
//! so the raw ChaCha word stream is byte-identical to the CPU path and the noise
//! samples agree with it to <= 1 ulp f32, independent of worker count.
//!
//! The kernel emits `2 * num_symbols` standard-normal samples per frame in the
//! CPU `draw_standard_normal` word order (4 words per sample); the host assigns
//! them planar, as the CPU stage does: sample `k` is symbol `k`'s I-axis noise
//! and sample `num_symbols + k` its Q-axis noise.

#[cfg(feature = "hip")]
mod imp {
    use gf2_kernels_hip::host::HipStream;
    use gf2_kernels_hip::{AwgnStreamScratch, GpuChaChaAwgn};

    use crate::batch::SymbolBatch;
    use crate::channels::Awgn;
    use crate::error::StageError;
    use crate::gpu::map_hip_error;
    use crate::parallel::worker_offset;
    use crate::stage::{ExecutionClass, Stage};

    /// Per-stage scratch for [`GpuAwgn`]: a reusable host buffer for the D2H
    /// noise read-back.
    ///
    /// The device generator ([`GpuChaChaAwgn`]) owns non-`Sync` device buffers,
    /// so it stays outside the `Send + Sync` [`Stage::Scratch`](crate::Stage)
    /// and is passed to [`apply_for_frame`](GpuAwgn::apply_for_frame) by
    /// reference.
    #[derive(Default)]
    pub struct GpuAwgnScratch {
        host_buf: Vec<f32>,
    }

    impl GpuAwgnScratch {
        /// The host read-back buffer, grown by [`Stage::process`](crate::Stage).
        #[must_use]
        pub fn host_buf(&self) -> &[f32] {
            &self.host_buf
        }
    }

    /// GPU AWGN channel stage: adds device-drawn complex Gaussian noise to a
    /// [`SymbolBatch`].
    ///
    /// The per-axis noise standard deviation is
    /// `sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))`, shared with the CPU
    /// [`Awgn`].
    #[derive(Debug, Clone)]
    pub struct GpuAwgn {
        es_n0_db: f32,
        bits_per_symbol: usize,
        sigma: f32,
        seed: u64,
        snr_idx: usize,
        worker_idx: usize,
        device_id: i32,
        fallback: Awgn,
    }

    impl GpuAwgn {
        /// Constructs a GPU AWGN stage seeding from `(seed=0, snr_idx=0,
        /// worker_idx=0)` on device 0.
        ///
        /// Construction touches no device.
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

        /// Sets the `worker_offset` parameters `(seed, snr_idx, worker_idx)`
        /// each frame's noise is drawn at.
        #[must_use]
        pub fn with_seek(mut self, seed: u64, snr_idx: usize, worker_idx: usize) -> Self {
            self.seed = seed;
            self.snr_idx = snr_idx;
            self.worker_idx = worker_idx;
            self
        }

        /// Targets a non-default HIP device for the noise generator.
        #[must_use]
        pub fn on_device(mut self, device_id: i32) -> Self {
            self.device_id = device_id;
            self
        }

        /// Channel Es/N0 in dB.
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

        /// Adds GPU-drawn AWGN noise to a frame's symbols in place using the
        /// caller-owned device generator `gen`, seeked to frame `frame_idx`'s
        /// `worker_offset`.
        ///
        /// `gen` must have been built for the `seed` this stage is configured
        /// with ([`with_seek`](Self::with_seek)).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via
        /// [`map_hip_error`](crate::gpu::map_hip_error)) on a device fault: an
        /// OOM or unsupported arch is recoverable, any other HIP failure is
        /// fatal.
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
            let n_samples = 2 * num_symbols;
            let base = worker_offset(self.seed, self.snr_idx, self.worker_idx, frame_idx);

            let sigma = self.sigma;
            let noise = gen
                .noise_samples(base, n_samples)
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::noise_samples"))?;

            // Planar: sample k is symbol k's I noise, num_symbols + k its Q noise.
            for (k, (xi, xq)) in i_lane.iter_mut().zip(q_lane.iter_mut()).enumerate() {
                *xi += noise[k] * sigma;
                *xq += noise[num_symbols + k] * sigma;
            }
            Ok(())
        }

        /// Builds a per-worker device noise generator sized for `max_symbols`
        /// symbols (`2 * max_symbols` samples), seeded from this stage's `seed`.
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
        /// caller-owned generator `gen`, seeking each frame `f` to its
        /// `worker_offset(.., f)`.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on any per-frame device fault (see
        /// [`apply_for_frame`](Self::apply_for_frame)).
        ///
        /// # Complexity
        ///
        /// O(total symbols) host-side plus one device launch per frame.
        ///
        /// # Panics
        ///
        /// Panics if `gen`'s capacity is less than twice the largest frame's
        /// symbol count.
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

        /// Allocates the pinned host staging the stream-ordered noise variants
        /// ([`apply_for_frame_on_stream`](Self::apply_for_frame_on_stream) /
        /// [`apply_on_stream`](Self::apply_on_stream)) require, sized for
        /// `gen`.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`](crate::gpu::map_hip_error))
        /// if the pinned allocation fails (an OOM is recoverable).
        pub fn build_stream_scratch(
            &self,
            gen: &GpuChaChaAwgn,
        ) -> Result<AwgnStreamScratch, StageError> {
            gen.new_stream_scratch()
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::new_stream_scratch"))
        }

        /// Like [`apply_for_frame`](Self::apply_for_frame), but with the noise
        /// launch and read-back ordered on the caller-owned `stream` and
        /// synchronized per stream. The added noise is byte-identical to
        /// [`apply_for_frame`](Self::apply_for_frame). `scratch` comes from
        /// [`build_stream_scratch`](Self::build_stream_scratch).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
        ///
        /// # Panics
        ///
        /// Panics if `i_lane.len() != q_lane.len()`, if `gen`'s capacity is
        /// less than `2 * i_lane.len()`, or if `scratch` was sized for a
        /// different generator.
        ///
        /// # Complexity
        ///
        /// Identical to [`apply_for_frame`](Self::apply_for_frame).
        pub fn apply_for_frame_on_stream(
            &self,
            i_lane: &mut [f32],
            q_lane: &mut [f32],
            frame_idx: usize,
            gen: &GpuChaChaAwgn,
            stream: &HipStream,
            scratch: &mut AwgnStreamScratch,
        ) -> Result<(), StageError> {
            assert_eq!(
                i_lane.len(),
                q_lane.len(),
                "GpuAwgn::apply_for_frame_on_stream: I lane length ({}) != Q lane length ({})",
                i_lane.len(),
                q_lane.len()
            );
            let num_symbols = i_lane.len();
            if num_symbols == 0 {
                return Ok(());
            }
            let n_samples = 2 * num_symbols;
            let base = worker_offset(self.seed, self.snr_idx, self.worker_idx, frame_idx);

            let mut noise = vec![0.0f32; n_samples];
            gen.noise_samples_into_on_stream(base, &mut noise, stream, scratch)
                .map_err(|e| map_hip_error(e, "GpuChaChaAwgn::noise_samples_into_on_stream"))?;

            let sigma = self.sigma;
            for (k, (xi, xq)) in i_lane.iter_mut().zip(q_lane.iter_mut()).enumerate() {
                *xi += noise[k] * sigma;
                *xq += noise[num_symbols + k] * sigma;
            }
            Ok(())
        }

        /// Like [`apply`](Self::apply), but stream-ordered: every frame's noise
        /// launch and read-back run on the caller-owned `stream` (see
        /// [`apply_for_frame_on_stream`](Self::apply_for_frame_on_stream)).
        /// The corrupted batch is byte-identical to [`apply`](Self::apply).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on any per-frame device fault (see
        /// [`apply_for_frame_on_stream`](Self::apply_for_frame_on_stream)).
        ///
        /// # Complexity
        ///
        /// Identical to [`apply`](Self::apply).
        pub fn apply_on_stream(
            &self,
            batch: &mut SymbolBatch,
            gen: &GpuChaChaAwgn,
            stream: &HipStream,
            scratch: &mut AwgnStreamScratch,
        ) -> Result<(), StageError> {
            for (f, (i_frame, q_frame)) in batch.i.iter_mut().zip(batch.q.iter_mut()).enumerate() {
                self.apply_for_frame_on_stream(i_frame, q_frame, f, gen, stream, scratch)?;
            }
            Ok(())
        }

        /// Like [`apply_for_frame`](Self::apply_for_frame) but reads the device
        /// noise back into the caller-provided `host_buf` (resized as needed),
        /// avoiding a per-frame allocation.
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

        /// Adds GPU AWGN noise to a copy of `input`, building a device
        /// generator per call (the device buffers cannot live in the
        /// `Sync`-bound scratch); [`apply`](Self::apply) reuses a caller-owned
        /// generator. An empty batch builds no generator.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] on a device fault (recoverable for OOM /
        /// unsupported arch; fatal otherwise).
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

        /// The paired CPU [`Awgn`] with the same `es_n0_db` / `bits_per_symbol`.
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

        #[test]
        fn test_stage_and_scratch_are_send() {
            fn assert_send<T: Send>() {}
            assert_send::<GpuAwgn>();
            assert_send::<GpuAwgnScratch>();
        }

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
            // 64 samples = 256 words per frame.
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
                    // The order `gf2_sim::channels::draw_standard_normal` uses.
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
            let gpu = GpuAwgn::new(6.5, 4).with_seek(seed, snr_idx, 0);
            let gen = gpu.build_generator(num_symbols).expect("build generator");

            for frame_idx in 0..n_frames {
                let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
                ctx.reseek_to_frame(frame_idx);
                let mut cpu_batch = SymbolBatch::new(vec![i0.clone()], vec![q0.clone()]);
                cpu.apply(&mut cpu_batch, ctx.rng_mut());
                let cpu_i = &cpu_batch.i[0];
                let cpu_q = &cpu_batch.q[0];

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

            let mut scratch = GpuAwgnScratch::default();
            let via_process = gpu.process(&input, &mut scratch).expect("process");

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

        #[test]
        fn test_apply_on_stream_matches_default_stream() {
            use gf2_kernels_hip::host::{device_mem_info, HipStream};

            if device_mem_info().is_err() {
                eprintln!("skipping test_apply_on_stream_matches_default_stream: no usable GPU");
                return;
            }

            let seed = 0xDE16_0FC5_u64;
            let num_symbols = 300usize;
            let i0: Vec<f32> = (0..num_symbols).map(|k| (k as f32) * 0.01 - 1.5).collect();
            let q0: Vec<f32> = (0..num_symbols)
                .map(|k| 0.75 - (k as f32) * 0.004)
                .collect();
            // Two frames so the loops index frame 0 and 1 distinctly.
            let input =
                SymbolBatch::new(vec![i0.clone(), i0.clone()], vec![q0.clone(), q0.clone()]);

            let gpu = GpuAwgn::new(6.0, 4).with_seek(seed, 1, 0);
            let gen = gpu.build_generator(num_symbols).expect("generator");

            let mut default = input.clone();
            gpu.apply(&mut default, &gen).expect("default-stream apply");

            let stream = HipStream::new().expect("create stream");
            let mut scratch = gpu.build_stream_scratch(&gen).expect("pinned staging");
            let mut streamed = input.clone();
            gpu.apply_on_stream(&mut streamed, &gen, &stream, &mut scratch)
                .expect("stream-ordered apply");

            // Bit-level comparison (f32 `==` would conflate -0.0 with 0.0).
            for f in 0..2 {
                for k in 0..num_symbols {
                    assert_eq!(
                        default.i[f][k].to_bits(),
                        streamed.i[f][k].to_bits(),
                        "frame={f} I[{k}] differs: default={} stream={}",
                        default.i[f][k],
                        streamed.i[f][k]
                    );
                    assert_eq!(
                        default.q[f][k].to_bits(),
                        streamed.q[f][k].to_bits(),
                        "frame={f} Q[{k}] differs: default={} stream={}",
                        default.q[f][k],
                        streamed.q[f][k]
                    );
                }
            }
        }

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
