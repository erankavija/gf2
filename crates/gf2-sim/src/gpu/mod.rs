//! HIP/ROCm host-side GPU dispatch (`feature = "hip"`).
//!
//! The `unsafe` HIP FFI lives in the `gf2-kernels-hip` kernel crate. This
//! module owns a `HipDispatcher` (a stream pool plus per-stage scratch) and
//! translates the kernel crate's `HipError` into the pipeline's
//! [`StageError`] hierarchy.
//!
//! [`StageError`]: crate::error::StageError

pub mod awgn;
pub mod demap;
pub mod ldpc_bp;
pub mod nr_5g_ldpc;

#[cfg(feature = "hip")]
mod imp {
    use gf2_kernels_hip::host::{GfxTarget, HipStreamPool, PinnedHostBuffer};
    use gf2_kernels_hip::HipError;

    use crate::error::{FatalError, RecoverableError, StageError};

    /// Maps a kernel-crate `HipError` to the pipeline's [`StageError`].
    ///
    /// - `HipError::OutOfMemory` → [`RecoverableError::OutOfMemory`] (wrapped in
    ///   [`StageError::Recoverable`]).
    /// - `HipError::UnsupportedArch` → [`RecoverableError::Transient`] (wrapped
    ///   in [`StageError::Recoverable`]) after a `tracing::warn!`.
    /// - `HipError::NoDevice` → [`FatalError::DeviceUnavailable`] (wrapped in
    ///   [`StageError::Fatal`]).
    /// - `HipError::BlobLoad` → [`FatalError::KernelLaunch`] (wrapped in
    ///   [`StageError::Fatal`]) carrying `HipError::code()` and the blob path.
    /// - `HipError::Hip` → [`FatalError::KernelLaunch`] (wrapped in
    ///   [`StageError::Fatal`]) carrying the raw `hipError_t` code.
    ///
    /// `kernel` names the failing operation in the resulting
    /// [`FatalError::KernelLaunch`].
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
            ref e @ HipError::BlobLoad {
                ref path,
                ref source,
            } => StageError::Fatal(FatalError::KernelLaunch {
                // `code()` is the hipErrorFileNotFound sentinel (301) for a BlobLoad.
                hip_code: e.code(),
                kernel,
                args: format!("blob load failed for '{}': {source}", path.display()),
            }),
            HipError::Hip { code, context } => StageError::Fatal(FatalError::KernelLaunch {
                hip_code: code,
                kernel,
                args: format!("hip context: {context}"),
            }),
        }
    }

    /// Per-stage staging scratch held by the dispatcher.
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
    /// A `HipDispatcher` holds one `HipStreamPool` bound to a single device and
    /// the per-stage [`StageScratch`].
    ///
    /// The dispatcher is owned by the orchestrator thread: its
    /// [`StageScratch`] embeds a `Send`-only `PinnedHostBuffer`, so
    /// `HipDispatcher` is `Send` but not `Sync`. The stream pool is `Sync`; the
    /// orchestrator borrows it via [`streams`](HipDispatcher::streams) and
    /// workers call `acquire` / `acquire_idle` on the shared `&HipStreamPool`.
    pub struct HipDispatcher {
        device_id: i32,
        /// The gfx target detected at construction.
        target: GfxTarget,
        streams: HipStreamPool,
        scratch: Vec<StageScratch>,
    }

    impl HipDispatcher {
        /// Builds a dispatcher with `n_streams` streams on `device_id`.
        ///
        /// Detects the device's gfx target via [`GfxTarget::detect_device`]
        /// before creating the stream pool.
        ///
        /// # Panics
        ///
        /// Panics if `n_streams` is zero.
        ///
        /// # Errors
        ///
        /// Returns a [`StageError`] if arch detection fails (recoverable for an
        /// unsupported arch, fatal for an absent device) or the stream pool
        /// cannot be created. An OOM is surfaced as recoverable; any other HIP
        /// failure as fatal.
        pub fn new(device_id: i32, n_streams: usize) -> Result<Self, StageError> {
            let target = GfxTarget::detect_device(device_id)
                .map_err(|e| map_hip_error(e, "GfxTarget::detect_device"))?;
            let streams = HipStreamPool::new(device_id, n_streams)
                .map_err(|e| map_hip_error(e, "HipStreamPool::new"))?;
            Ok(Self {
                device_id,
                target,
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

        /// The gfx target detected for this dispatcher's device at construction.
        pub fn target(&self) -> GfxTarget {
            self.target
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

    /// Compile-time check of the concurrency contract documented on
    /// [`HipDispatcher`].
    #[cfg(test)]
    mod sync_contract {
        use super::*;
        use gf2_kernels_hip::host::HipStreamPool;

        const fn _assert_send<T: Send>() {}
        const fn _assert_sync<T: Sync>() {}

        const _: () = {
            _assert_send::<HipStreamPool>();
            _assert_sync::<HipStreamPool>();
            _assert_send::<HipDispatcher>();
        };
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

        /// The typed error is constructed directly because detection on a
        /// supported device never produces `UnsupportedArch`.
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

        #[test]
        fn test_map_blob_load_is_fatal_kernel_launch() {
            let err = HipError::BlobLoad {
                path: std::path::PathBuf::from("/kernels/gfx1030/bcjr.co"),
                source: "No such file or directory (os error 2)".to_string(),
            };
            match map_hip_error(err, "load") {
                StageError::Fatal(FatalError::KernelLaunch {
                    hip_code,
                    kernel,
                    args,
                }) => {
                    assert_ne!(hip_code, 0, "must not report hipSuccess for an I/O failure");
                    assert_eq!(hip_code, 301);
                    assert_eq!(kernel, "load");
                    assert!(
                        args.contains("bcjr.co"),
                        "diagnostic args should carry the offending blob path, got: {args}"
                    );
                }
                other => panic!("expected fatal KernelLaunch for blob load, got {other:?}"),
            }
        }

        /// This boundary and `HipError::is_recoverable` state the same split of
        /// `@/inv/accelerator-safe-fallback` in two vocabularies — the
        /// pipeline's `StageError` here, the kernel crate's predicate for
        /// callers outside the pipeline, such as `gf2-coding`'s GPU-assisted
        /// BCH decoding. They answer alike for every variant.
        #[test]
        fn test_stage_error_split_matches_the_kernel_predicate() {
            let errors = [
                HipError::OutOfMemory {
                    device_id: 0,
                    bytes_requested: 1 << 40,
                },
                HipError::UnsupportedArch {
                    gcn_arch_name: "gfx908".to_string(),
                },
                HipError::NoDevice,
                HipError::BlobLoad {
                    path: std::path::PathBuf::from("/kernels/gfx1030/bcjr.co"),
                    source: "No such file or directory (os error 2)".to_string(),
                },
                HipError::Hip {
                    code: 7,
                    context: "hipMalloc",
                },
            ];
            for err in errors {
                let recoverable = err.is_recoverable();
                let mapped = map_hip_error(err.clone(), "split");
                assert_eq!(
                    matches!(mapped, StageError::Recoverable(_)),
                    recoverable,
                    "{err:?} maps to {mapped:?} but reports is_recoverable() == {recoverable}"
                );
            }
        }

        #[test]
        fn test_map_no_device_is_device_unavailable() {
            match map_hip_error(HipError::NoDevice, "detect") {
                StageError::Fatal(FatalError::DeviceUnavailable) => {}
                other => panic!("expected fatal DeviceUnavailable, got {other:?}"),
            }
        }

        #[test]
        fn test_dispatcher_acquires_stream_and_allocates() {
            use gf2_kernels_hip::host::DeviceBuffer;

            let target = match GfxTarget::detect_device(0) {
                Ok(target) => target,
                Err(error) if error.code() == HipError::NoDevice.code() => {
                    eprintln!("Skipping device allocation: no HIP device is visible");
                    return;
                }
                Err(error) => panic!("detect HIP device: {error:?}"),
            };
            let mut disp = HipDispatcher::new(0, 4).expect("build dispatcher on detected device");
            assert_eq!(disp.device_id(), 0);
            assert_eq!(disp.target(), target);
            assert_eq!(disp.streams().len(), 4);

            let _stream = disp.streams().acquire();
            let idx = disp.add_stage_scratch(128).expect("pinned scratch");
            assert_eq!(disp.scratch(idx).staging.len(), 128);

            let buf = DeviceBuffer::<f32>::new(256, disp.device_id()).expect("device alloc");
            assert_eq!(buf.len(), 256);
        }

        /// Asserts the mapping arms `HipDispatcher::new` routes detection
        /// errors through; a bad device cannot be forced on a healthy host.
        #[test]
        fn test_dispatcher_new_detect_error_mapping() {
            let unsupported = HipError::UnsupportedArch {
                gcn_arch_name: "gfx908".to_string(),
            };
            match map_hip_error(unsupported, "GfxTarget::detect_device") {
                StageError::Recoverable(RecoverableError::Transient(cause)) => {
                    assert!(
                        cause.to_string().contains("gfx908"),
                        "fallback cause should name the unsupported arch, got: {cause}"
                    );
                }
                other => panic!("expected recoverable fallback from detect, got {other:?}"),
            }

            match map_hip_error(HipError::NoDevice, "GfxTarget::detect_device") {
                StageError::Fatal(FatalError::DeviceUnavailable) => {}
                other => panic!("expected fatal DeviceUnavailable from detect, got {other:?}"),
            }
        }

        #[test]
        fn test_forced_oom_returns_recoverable_not_panic() {
            use gf2_kernels_hip::host::{device_mem_info, DeviceBuffer};

            // `new_with_fallback` guarantees a structured OOM only when it can
            // read total device memory via `device_mem_info`; without a usable
            // GPU the request falls through to `hipMalloc` and a non-OOM error.
            if device_mem_info().is_err() {
                eprintln!(
                    "skipping test_forced_oom_returns_recoverable_not_panic: \
                     no usable GPU (device_mem_info failed)"
                );
                return;
            }

            // 256 GiB of u8 exceeds total device memory.
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
