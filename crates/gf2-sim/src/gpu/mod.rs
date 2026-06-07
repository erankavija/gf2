//! HIP/ROCm host-side GPU dispatch (design doc §5/§6, `feature = "hip"`).
//!
//! Owned by Phase B (`36075e4c`). The HIP host infrastructure itself (stream
//! pool, allocator wrappers, deterministic-launch helpers, multi-arch
//! detection) lives in the `gf2-kernels-hip` kernel crate so that all `unsafe`
//! FFI is isolated there. This module is the `gf2-sim`-side consumer: it owns a
//! `HipDispatcher` (a stream pool plus per-stage scratch) and translates the
//! kernel crate's `HipError` into the pipeline's [`StageError`] hierarchy.
//!
//! The item bodies are gated on `feature = "hip"`; the module home itself is
//! declared unconditionally in `lib.rs` so the crate builds (and documents)
//! cleanly with the feature off. (`HipDispatcher` is a plain code span rather
//! than an intra-doc link because the type exists only under `feature = "hip"`,
//! so the link would be unresolved on the default no-hip documentation build.)
//!
//! [`StageError`]: crate::error::StageError

#[cfg(feature = "hip")]
mod imp {
    use gf2_kernels_hip::host::{HipStreamPool, PinnedHostBuffer};
    use gf2_kernels_hip::HipError;

    use crate::error::{FatalError, RecoverableError, StageError};

    /// Maps a kernel-crate `HipError` to the pipeline's [`StageError`].
    ///
    /// This is the single boundary where the HIP-local error vocabulary becomes
    /// the pipeline vocabulary (design doc §8). The mapping:
    ///
    /// - `HipError::OutOfMemory` → [`RecoverableError::OutOfMemory`] (wrapped in
    ///   [`StageError::Recoverable`]) so the executor can substitute a CPU
    ///   fallback on the offending batch and continue. The `--strict-gpu`
    ///   promotion to [`FatalError::OutOfMemory`] is the executor's job
    ///   (`42eac5cc`), not this function's.
    /// - `HipError::UnsupportedArch` → [`RecoverableError::Transient`] (wrapped
    ///   in [`StageError::Recoverable`]) **after** a `tracing::warn!`, so the
    ///   executor falls back to the CPU-equivalent stage rather than aborting
    ///   (design doc §6: an arch with no kernel blob warns + falls back, the
    ///   same response as OOM). It is **not** a fatal `KernelLaunch`.
    /// - `HipError::NoDevice` → [`FatalError::DeviceUnavailable`] (wrapped in
    ///   [`StageError::Fatal`]) — a host with no GPU aborts construction; the
    ///   user re-runs with `--cpu-only` (design doc §8).
    /// - `HipError::Hip` → [`FatalError::KernelLaunch`] (wrapped in
    ///   [`StageError::Fatal`]) — any other HIP failure aborts the run with
    ///   the raw `hipError_t` code preserved for diagnostics.
    ///
    /// # Arguments
    ///
    /// * `err` - The error returned by a `gf2-kernels-hip` call.
    /// * `kernel` - A static name for the failing operation, recorded in the
    ///   resulting [`FatalError::KernelLaunch`] for generic HIP errors.
    pub fn map_hip_error(err: HipError, kernel: &'static str) -> StageError {
        match err {
            HipError::OutOfMemory {
                device_id,
                bytes_requested,
            } => StageError::Recoverable(RecoverableError::OutOfMemory {
                device_id,
                bytes_requested,
            }),
            HipError::UnsupportedArch { gcn_arch_name } => {
                tracing::warn!(
                    kernel,
                    gcn_arch_name = %gcn_arch_name,
                    "unsupported gfx arch '{gcn_arch_name}'; falling back to CPU stage"
                );
                StageError::Recoverable(RecoverableError::Transient(
                    format!("unsupported gfx arch '{gcn_arch_name}': falling back to CPU stage")
                        .into(),
                ))
            }
            HipError::NoDevice => StageError::Fatal(FatalError::DeviceUnavailable),
            HipError::Hip { code, context } => StageError::Fatal(FatalError::KernelLaunch {
                hip_code: code,
                kernel,
                args: format!("hip context: {context}"),
            }),
        }
    }

    /// Per-stage staging scratch held by the dispatcher.
    ///
    /// Phase B GPU stages stage their H2D / D2H transfers through pinned host
    /// buffers for overlap (design doc §6). The dispatcher owns one staging area
    /// per stage so the kernel stages (`ed575f15` and the next-wave kernel
    /// owners) borrow it rather than re-allocating per batch. This is a minimal
    /// v1 holder; the kernel stages extend it with their concrete typed buffers.
    pub struct StageScratch {
        /// Pinned host staging buffer for LLR / symbol payloads (f32 lanes).
        pub staging: PinnedHostBuffer<f32>,
    }

    impl StageScratch {
        /// Allocates a staging area sized for `capacity` f32 lanes on
        /// `device_id`.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] (via [`map_hip_error`]) if the pinned
        /// allocation fails — an OOM here is recoverable, any other HIP failure
        /// is fatal.
        pub fn new(capacity: usize, device_id: i32) -> Result<Self, StageError> {
            let staging = PinnedHostBuffer::<f32>::new(capacity, device_id)
                .map_err(|e| map_hip_error(e, "StageScratch::new"))?;
            Ok(Self { staging })
        }
    }

    /// Owns the HIP host resources a pipeline run shares across its GPU stages.
    ///
    /// A `HipDispatcher` holds the `HipStreamPool` (one stream per worker, handed
    /// out round-robin / oldest-idle) and the per-stage [`StageScratch`]. v1 is
    /// single-device; the design doc §7 multi-GPU seam replaces the single pool
    /// with a per-device map without changing this type's stage-facing API.
    pub struct HipDispatcher {
        device_id: i32,
        streams: HipStreamPool,
        scratch: Vec<StageScratch>,
    }

    impl HipDispatcher {
        /// Builds a dispatcher with `n_streams` streams on `device_id`.
        ///
        /// # Arguments
        ///
        /// * `device_id` - The HIP device to bind the stream pool to.
        /// * `n_streams` - Number of streams (typically the worker count). Must
        ///   be non-zero (delegated to `HipStreamPool::new`).
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] if the stream pool cannot be created. An OOM
        /// is surfaced as recoverable; any other HIP failure as fatal.
        pub fn new(device_id: i32, n_streams: usize) -> Result<Self, StageError> {
            let streams = HipStreamPool::new(device_id, n_streams)
                .map_err(|e| map_hip_error(e, "HipStreamPool::new"))?;
            Ok(Self {
                device_id,
                streams,
                scratch: Vec::new(),
            })
        }

        /// Reserves one [`StageScratch`] of `capacity` f32 lanes and returns its
        /// index for later borrowing via [`HipDispatcher::scratch`].
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] if the pinned staging allocation fails.
        pub fn add_stage_scratch(&mut self, capacity: usize) -> Result<usize, StageError> {
            let s = StageScratch::new(capacity, self.device_id)?;
            self.scratch.push(s);
            Ok(self.scratch.len() - 1)
        }

        /// The device this dispatcher's resources are bound to.
        pub fn device_id(&self) -> i32 {
            self.device_id
        }

        /// Borrows the shared stream pool.
        pub fn streams(&self) -> &HipStreamPool {
            &self.streams
        }

        /// Borrows the scratch reserved at `index` by
        /// [`HipDispatcher::add_stage_scratch`].
        ///
        /// # Panics
        ///
        /// Panics if `index` is out of range.
        pub fn scratch(&self, index: usize) -> &StageScratch {
            &self.scratch[index]
        }

        /// Mutably borrows the scratch reserved at `index`.
        ///
        /// # Panics
        ///
        /// Panics if `index` is out of range.
        pub fn scratch_mut(&mut self, index: usize) -> &mut StageScratch {
            &mut self.scratch[index]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_map_oom_is_recoverable() {
            let err = HipError::OutOfMemory {
                device_id: 0,
                bytes_requested: 1 << 40,
            };
            match map_hip_error(err, "test") {
                StageError::Recoverable(RecoverableError::OutOfMemory {
                    device_id,
                    bytes_requested,
                }) => {
                    assert_eq!(device_id, 0);
                    assert_eq!(bytes_requested, 1 << 40);
                }
                other => panic!("expected recoverable OOM, got {other:?}"),
            }
        }

        #[test]
        fn test_map_generic_hip_is_fatal() {
            let err = HipError::Hip {
                code: 7,
                context: "hipMalloc",
            };
            match map_hip_error(err, "kern") {
                StageError::Fatal(FatalError::KernelLaunch {
                    hip_code, kernel, ..
                }) => {
                    assert_eq!(hip_code, 7);
                    assert_eq!(kernel, "kern");
                }
                other => panic!("expected fatal KernelLaunch, got {other:?}"),
            }
        }

        /// Finding 1: an unsupported arch must NOT abort as fatal. It maps to a
        /// recoverable error (warn + CPU fallback per design §6), so the
        /// end-to-end "warn + fall back" path is expressible at this boundary.
        /// Simulated by constructing the typed error directly (this host is a
        /// gfx1030, so real detection never produces UnsupportedArch here).
        #[test]
        fn test_map_unsupported_arch_is_recoverable_not_fatal() {
            let err = HipError::UnsupportedArch {
                gcn_arch_name: "gfx908".to_string(),
            };
            match map_hip_error(err, "detect") {
                StageError::Recoverable(RecoverableError::Transient(cause)) => {
                    assert!(
                        cause.to_string().contains("gfx908"),
                        "transient cause should name the offending arch, got: {cause}"
                    );
                }
                other => panic!("expected recoverable Transient fallback, got {other:?}"),
            }
        }

        /// Finding 1: a host with no GPU maps to the dedicated
        /// `DeviceUnavailable` fatal, not a generic `KernelLaunch`.
        #[test]
        fn test_map_no_device_is_device_unavailable() {
            match map_hip_error(HipError::NoDevice, "detect") {
                StageError::Fatal(FatalError::DeviceUnavailable) => {}
                other => panic!("expected fatal DeviceUnavailable, got {other:?}"),
            }
        }

        /// Finding 6: exercise `HipDispatcher` end-to-end on the gfx1030 host —
        /// build it, acquire a stream from its pool, and allocate a small
        /// `DeviceBuffer` — so it is covered rather than dead code. Phase B
        /// kernel stages (`ed575f15` and the next-wave kernel owners) and the
        /// Phase C executor (`42eac5cc`) are the production consumers.
        #[test]
        fn test_dispatcher_acquires_stream_and_allocates() {
            use gf2_kernels_hip::host::DeviceBuffer;

            let mut disp = HipDispatcher::new(0, 4).expect("build dispatcher on gfx1030");
            assert_eq!(disp.device_id(), 0);
            assert_eq!(disp.streams().len(), 4);

            // Acquire a stream from the shared pool (round-robin path).
            let _stream = disp.streams().acquire();

            // Reserve per-stage scratch and allocate a small device buffer.
            let idx = disp.add_stage_scratch(128).expect("pinned scratch");
            assert_eq!(disp.scratch(idx).staging.len(), 128);

            let buf = DeviceBuffer::<f32>::new(256, disp.device_id()).expect("device alloc");
            assert_eq!(buf.len(), 256);
        }

        /// End-to-end OOM path on the real GPU: a `DeviceBuffer` request larger
        /// than device memory must return a recoverable OOM error, not panic.
        #[test]
        fn test_forced_oom_returns_recoverable_not_panic() {
            use gf2_kernels_hip::host::DeviceBuffer;

            // Request far more than any GPU has (256 GiB of u8).
            // `new_with_fallback` pre-flights against total device memory and
            // returns OOM.
            let huge: usize = 256 * 1024 * 1024 * 1024;
            let result = DeviceBuffer::<u8>::new_with_fallback(huge, 0);
            let err = result.err().expect("256 GiB alloc must fail");
            match map_hip_error(err, "oom-test") {
                StageError::Recoverable(RecoverableError::OutOfMemory {
                    bytes_requested, ..
                }) => {
                    assert_eq!(bytes_requested, huge);
                }
                other => panic!("expected recoverable OOM, got {other:?}"),
            }
        }
    }
}

#[cfg(feature = "hip")]
pub use imp::{map_hip_error, HipDispatcher, StageScratch};
