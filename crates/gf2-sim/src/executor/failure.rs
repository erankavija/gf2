//! OOM auto-fallback dispatch and hard-fail diagnostic dump.
//!
//! [`dispatch_with_fallback`] wraps every GPU stage invocation in the hybrid
//! scheduler loop and the topology executor's `GpuOnly` arm; its Rustdoc holds
//! the decision tree.
//!
//! # Diagnostic dump
//!
//! On every fatal stage error the executor serialises a JSON diagnostic record
//! to the configured `diagnostic_dump_dir` (from [`PipelineConfig`], default
//! [`default_dump_dir`]). The file is named
//! `<timestamp_ns>-<device_id>-<snr_idx>.json` and is written to a sibling
//! `.tmp` file then renamed.
//!
//! [`PipelineConfig`]: crate::PipelineConfig

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{FatalError, RecoverableError, StageError};

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic dump schema
// ─────────────────────────────────────────────────────────────────────────────

/// Failure-mode parameters threaded through GPU stage dispatch, built from the
/// same [`PipelineConfig`](crate::PipelineConfig) fields by the topology
/// executor's `GpuOnly` arm and the scheduler hybrid loop.
///
/// The fields are only read inside `#[cfg(feature = "hip")]` dispatch arms;
/// the `dead_code` lint would fire on no-hip builds where those arms are
/// elided.
#[allow(dead_code)]
pub(crate) struct FailurePolicy<'p> {
    /// Promote GPU OOM (and only OOM) to fatal instead of CPU-falling-back.
    pub(crate) strict_gpu: bool,
    /// Directory for JSON hard-fail diagnostic dumps.
    pub(crate) dump_dir: &'p std::path::Path,
    /// **Test-only** GPU-OOM injection modulus. When
    /// `Some(m)`, the GPU LDPC dispatch forces a recoverable OOM on every
    /// dispatch whose keying global frame index `g` satisfies `g % m == 0`
    /// (the topology executor keys each one-frame dispatch on its global
    /// frame index; the scheduler hybrid loop keys each batch on the batch's
    /// FIRST global frame index), driving the production
    /// [`dispatch_with_fallback`] path. Mirrors
    /// [`PipelineConfig::inject_gpu_oom_modulus`](crate::PipelineConfig::inject_gpu_oom_modulus).
    pub(crate) inject_gpu_oom_modulus: Option<u64>,
}

impl FailurePolicy<'_> {
    /// Whether the test-only OOM injection fires for the GPU dispatch keyed on
    /// global frame index `g` (see
    /// [`inject_gpu_oom_modulus`](Self::inject_gpu_oom_modulus)).
    #[allow(dead_code)] // read only inside `feature = "hip"` dispatch arms.
    pub(crate) fn injects_oom_at(&self, g: u64) -> bool {
        injects_oom_at(self.inject_gpu_oom_modulus, g)
    }
}

/// Whether the test-only OOM injection modulus fires for the dispatch keyed on
/// global frame index `g` — the free-function form for callers that have no
/// [`FailurePolicy`] (the checkpointed drain hook propagates faults instead of
/// dispatching a fallback, so it carries only the modulus).
#[allow(dead_code)] // read only inside `feature = "hip"` dispatch arms.
pub(crate) fn injects_oom_at(modulus: Option<u64>, g: u64) -> bool {
    modulus.is_some_and(|m| m >= 1 && g.is_multiple_of(m))
}

/// Context passed to [`dispatch_with_fallback`] for tracing and diagnostics.
/// Carries the per-batch identifiers that appear in the `tracing::warn!` /
/// `tracing::error!` events and in the JSON dump.
#[derive(Debug, Clone, Copy)]
pub struct FaultContext {
    /// The batch identifier (global frame index or batch sequence number).
    pub batch_id: u64,
    /// The SNR-point index keying the RNG seek.
    pub snr_idx: usize,
    /// The HIP device the GPU stage ran on (0-indexed).
    pub device_id: i32,
    /// The rayon worker that dispatched the stage.
    pub worker_idx: usize,
}

/// JSON record written per hard-fail event into `diagnostic_dump_dir`.
#[derive(Debug, Serialize)]
struct DiagnosticDump {
    /// Event kind: always `"hard_fail"` for this record.
    event: &'static str,
    /// Timestamp in nanoseconds since UNIX epoch.
    timestamp_ns: u128,
    /// The HIP device that faulted.
    device_id: i32,
    /// The rayon worker that encountered the fault.
    worker_idx: usize,
    /// The SNR-point index.
    snr_idx: usize,
    /// The batch identifier.
    batch_id: u64,
    /// The HIP error code (from [`FatalError::KernelLaunch`]).
    hip_code: i32,
    /// The kernel name that faulted.
    kernel: &'static str,
    /// The launch args / context string.
    args: String,
}

/// Writes the hard-fail diagnostic dump for `fatal` to
/// `dump_dir/<timestamp_ns>-<device_id>-<snr_idx>.json` via a `.tmp` sibling
/// and rename. A failed write (permission error, full filesystem) is logged
/// with `tracing::error!` and otherwise ignored: the run aborts on the stage
/// error, not on the dump I/O error.
fn write_diagnostic_dump(fatal: &FatalError, ctx: FaultContext, dump_dir: &std::path::Path) {
    let timestamp_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let (hip_code, kernel, args) = match fatal {
        FatalError::KernelLaunch {
            hip_code,
            kernel,
            args,
        } => (*hip_code, *kernel, args.clone()),
        FatalError::OutOfMemory {
            device_id,
            bytes_requested,
        } => (
            -1,
            "OOM",
            format!("device {device_id}: {bytes_requested} bytes requested"),
        ),
        FatalError::DeviceUnavailable => (-1, "DeviceUnavailable", String::new()),
        FatalError::BuildError(_) => (-1, "BuildError", format!("{fatal:?}")),
        FatalError::CpuFallbackAlsoFailed { original } => {
            (-1, "CpuFallbackAlsoFailed", format!("{original:?}"))
        }
    };

    let record = DiagnosticDump {
        event: "hard_fail",
        timestamp_ns,
        device_id: ctx.device_id,
        worker_idx: ctx.worker_idx,
        snr_idx: ctx.snr_idx,
        batch_id: ctx.batch_id,
        hip_code,
        kernel,
        args,
    };

    if let Ok(payload) = serde_json::to_string_pretty(&record) {
        let file_name = format!("{timestamp_ns}-{}-{}.json", ctx.device_id, ctx.snr_idx);
        let canonical = dump_dir.join(&file_name);
        let tmp = dump_dir.join(format!("{file_name}.tmp"));

        if let Err(e) = std::fs::create_dir_all(dump_dir) {
            tracing::error!(
                error = %e,
                dump_dir = %dump_dir.display(),
                "failed to create diagnostic dump directory"
            );
            return;
        }
        if let Err(e) = std::fs::write(&tmp, &payload) {
            tracing::error!(
                error = %e,
                path = %tmp.display(),
                "failed to write diagnostic dump tmp file"
            );
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &canonical) {
            tracing::error!(
                error = %e,
                tmp = %tmp.display(),
                canonical = %canonical.display(),
                "failed to rename diagnostic dump into place"
            );
            return;
        }
        tracing::error!(
            dump_path = %canonical.display(),
            hip_code,
            kernel,
            snr_idx = ctx.snr_idx,
            batch_id = ctx.batch_id,
            device_id = ctx.device_id,
            worker_idx = ctx.worker_idx,
            "GPU stage hard-fail: diagnostic dump written"
        );
    } else {
        tracing::error!(
            snr_idx = ctx.snr_idx,
            batch_id = ctx.batch_id,
            device_id = ctx.device_id,
            "GPU stage hard-fail: could not serialise diagnostic record"
        );
    }
}

/// The default diagnostic dump directory.
///
/// The returned path is **relative, resolved against the process's current
/// working directory** at dump time (for `cargo run` from the repo root that
/// is the workspace root; from anywhere else it is wherever the process was
/// started). Campaigns that need a stable location should set
/// [`PipelineConfig::diagnostic_dump_dir`](crate::PipelineConfig::diagnostic_dump_dir)
/// to an absolute path.
pub fn default_dump_dir() -> PathBuf {
    PathBuf::from("dev/benchmarks/gf2-sim/diagnostic-dumps")
}

// ─────────────────────────────────────────────────────────────────────────────
// dispatch_with_fallback
// ─────────────────────────────────────────────────────────────────────────────

/// Wraps a single GPU stage call with the failure-mode handling below; the
/// single call boundary for every GPU dispatch in the hybrid scheduler loop
/// and the topology executor's `GpuOnly` arm.
///
/// # Decision tree
///
/// ```text
/// gpu_result
///   ├── Ok(output) → return Ok(output)
///   ├── Err(Recoverable(OutOfMemory)) + strict_gpu:
///   │       emit tracing::error!, write dump → return Err(Fatal::OutOfMemory)
///   ├── Err(Recoverable(OutOfMemory)) + !strict_gpu:
///   │       emit tracing::warn!(batch_id, snr_idx, device_id)
///   │       fallback.process(input) →
///   │           Ok(o)  → return Ok(o)
///   │           Err(e) → return Err(Fatal::CpuFallbackAlsoFailed { original })
///   ├── Err(Recoverable(Transient)) → CPU fallback path (same branching),
///   │       regardless of strict_gpu: the strict promotion covers OOM only
///   └── Err(Fatal(_)) → write dump, emit tracing::error! → return Err(Fatal(_))
/// ```
///
/// # Arguments
///
/// * `gpu_result` — the `Result` returned by the GPU stage call.
/// * `run_fallback` — closure that runs the CPU fallback stage on the same
///   input. Called only on a recoverable error that is not promoted to fatal.
/// * `ctx` — per-batch context for tracing events and the diagnostic dump.
/// * `strict_gpu` — whether **OOM** is promoted to fatal (no CPU fallback).
///   Transient errors are never promoted (see the decision tree above).
/// * `dump_dir` — directory for JSON diagnostic dumps on hard-fail.
///
/// # Errors
///
/// Returns the original fatal error (or `FatalError::OutOfMemory` on strict-gpu
/// OOM, or `FatalError::CpuFallbackAlsoFailed` when the fallback also fails).
///
/// # Panics
///
/// Never panics; all I/O errors are logged via `tracing::error!` and the
/// original stage error is returned unchanged.
///
/// # Complexity
///
/// `O(1)` bookkeeping plus the fallback stage call when invoked.
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::failure::{dispatch_with_fallback, FaultContext, default_dump_dir};
/// use gf2_sim::error::{FatalError, RecoverableError, StageError};
///
/// let gpu_result: Result<u32, StageError> = Err(StageError::Recoverable(
///     RecoverableError::OutOfMemory { device_id: 0, bytes_requested: 1024 }
/// ));
/// let ctx = FaultContext { batch_id: 0, snr_idx: 0, device_id: 0, worker_idx: 0 };
/// // Non-strict: the fallback is invoked and succeeds.
/// let out = dispatch_with_fallback(
///     gpu_result,
///     || Ok::<u32, StageError>(42_u32),
///     ctx,
///     false,
///     &std::env::temp_dir(),
/// );
/// assert_eq!(out.unwrap(), 42_u32);
/// ```
pub fn dispatch_with_fallback<T, F>(
    gpu_result: Result<T, StageError>,
    run_fallback: F,
    ctx: FaultContext,
    strict_gpu: bool,
    dump_dir: &std::path::Path,
) -> Result<T, StageError>
where
    F: FnOnce() -> Result<T, StageError>,
{
    match gpu_result {
        Ok(output) => Ok(output),

        Err(StageError::Recoverable(recoverable)) => {
            // Extract device_id for the OOM warn event (may not be present for
            // Transient, so fall back to ctx.device_id).
            let device_id = match &recoverable {
                RecoverableError::OutOfMemory { device_id, .. } => *device_id,
                RecoverableError::Transient(_) => ctx.device_id,
            };

            // Strict mode promotes OOM, and only OOM, to fatal; Transient
            // falls through to the fallback below even under strict_gpu.
            if strict_gpu {
                if let RecoverableError::OutOfMemory {
                    device_id,
                    bytes_requested,
                } = &recoverable
                {
                    let fatal = FatalError::OutOfMemory {
                        device_id: *device_id,
                        bytes_requested: *bytes_requested,
                    };
                    write_diagnostic_dump(&fatal, ctx, dump_dir);
                    tracing::error!(
                        batch_id = ctx.batch_id,
                        snr_idx = ctx.snr_idx,
                        device_id = *device_id,
                        worker_idx = ctx.worker_idx,
                        "GPU stage OOM with strict_gpu: promoting to fatal"
                    );
                    return Err(StageError::Fatal(fatal));
                }
            }

            // Non-strict OOM, or Transient (any mode): attempt the CPU fallback.
            tracing::warn!(
                batch_id = ctx.batch_id,
                snr_idx = ctx.snr_idx,
                device_id,
                worker_idx = ctx.worker_idx,
                "GPU stage recoverable error; substituting CPU fallback"
            );

            match run_fallback() {
                Ok(output) => Ok(output),
                Err(fallback_err) => {
                    let fatal = FatalError::CpuFallbackAlsoFailed {
                        original: Box::new(recoverable),
                    };
                    write_diagnostic_dump(&fatal, ctx, dump_dir);
                    tracing::error!(
                        batch_id = ctx.batch_id,
                        snr_idx = ctx.snr_idx,
                        device_id,
                        worker_idx = ctx.worker_idx,
                        fallback_error = ?fallback_err,
                        "CPU fallback also failed after GPU recoverable error"
                    );
                    Err(StageError::Fatal(fatal))
                }
            }
        }

        Err(StageError::Fatal(fatal)) => {
            write_diagnostic_dump(&fatal, ctx, dump_dir);
            tracing::error!(
                batch_id = ctx.batch_id,
                snr_idx = ctx.snr_idx,
                device_id = ctx.device_id,
                worker_idx = ctx.worker_idx,
                error = ?fatal,
                "GPU stage hard-fail: aborting run"
            );
            Err(StageError::Fatal(fatal))
        }

        Err(other) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{FatalError, RecoverableError, StageError};

    fn ctx() -> FaultContext {
        FaultContext {
            batch_id: 1,
            snr_idx: 2,
            device_id: 0,
            worker_idx: 0,
        }
    }

    fn dump_dir() -> (gf2_core::test_scratch::Scratch, std::path::PathBuf) {
        let scratch = gf2_core::test_scratch::scratch("gf2sim-failure-test");
        let dump_dir = scratch.path().join("dump");
        (scratch, dump_dir)
    }

    #[test]
    fn test_ok_passes_through() {
        let (_scratch, dir) = dump_dir();
        let result: Result<u32, StageError> = Ok(42);
        let out = dispatch_with_fallback(result, || Ok(0), ctx(), false, &dir);
        assert_eq!(out.unwrap(), 42);
        assert!(!dir.exists());
    }

    #[test]
    fn test_oom_non_strict_invokes_fallback() {
        let (_scratch, dir) = dump_dir();
        let result: Result<u32, StageError> =
            Err(StageError::Recoverable(RecoverableError::OutOfMemory {
                device_id: 0,
                bytes_requested: 1024,
            }));
        let out = dispatch_with_fallback(result, || Ok(99_u32), ctx(), false, &dir);
        assert_eq!(out.unwrap(), 99, "fallback value must be returned");
        // Non-strict OOM + successful fallback produces no dump.
        assert!(!dir.exists());
    }

    #[test]
    fn test_oom_strict_promotes_to_fatal_and_writes_dump() {
        let (_scratch, dir) = dump_dir();
        let result: Result<u32, StageError> =
            Err(StageError::Recoverable(RecoverableError::OutOfMemory {
                device_id: 1,
                bytes_requested: 2048,
            }));
        let err = dispatch_with_fallback(result, || Ok(0_u32), ctx(), true, &dir)
            .expect_err("strict OOM must be fatal");
        assert!(
            matches!(err, StageError::Fatal(FatalError::OutOfMemory { .. })),
            "expected Fatal::OutOfMemory, got {err:?}"
        );
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dump dir must exist after strict OOM")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "at least one dump file must exist");
    }

    /// `strict_gpu` promotes OOM only: a `Transient` recoverable error takes
    /// the CPU fallback even under `strict_gpu`, and no dump is written when
    /// the fallback succeeds.
    #[test]
    fn test_transient_under_strict_gpu_still_falls_back() {
        let (_scratch, dir) = dump_dir();
        let result: Result<u32, StageError> = Err(StageError::Recoverable(
            RecoverableError::Transient("unsupported arch gfx9999".into()),
        ));
        let fallback_called = std::cell::Cell::new(false);
        let out = dispatch_with_fallback(
            result,
            || {
                fallback_called.set(true);
                Ok(7_u32)
            },
            ctx(),
            true, // strict_gpu — must NOT promote Transient
            &dir,
        );
        assert_eq!(
            out.unwrap(),
            7,
            "Transient under strict_gpu must take the CPU fallback"
        );
        assert!(
            fallback_called.get(),
            "the fallback must actually run for Transient under strict_gpu"
        );
        assert!(
            !dir.exists(),
            "no dump for a Transient that fell back successfully"
        );
    }

    #[test]
    fn test_oom_non_strict_fallback_also_fails() {
        let (_scratch, dir) = dump_dir();
        let result: Result<u32, StageError> =
            Err(StageError::Recoverable(RecoverableError::OutOfMemory {
                device_id: 0,
                bytes_requested: 512,
            }));
        let fallback_err = StageError::Fatal(FatalError::KernelLaunch {
            hip_code: 7,
            kernel: "fallback",
            args: "also failed".to_string(),
        });
        let err = dispatch_with_fallback(result, || Err(fallback_err), ctx(), false, &dir)
            .expect_err("both failed must be fatal");
        assert!(
            matches!(
                err,
                StageError::Fatal(FatalError::CpuFallbackAlsoFailed { .. })
            ),
            "expected CpuFallbackAlsoFailed, got {err:?}"
        );
    }

    #[test]
    fn test_fatal_writes_dump_and_propagates() {
        let (_scratch, dir) = dump_dir();
        let fatal = StageError::Fatal(FatalError::KernelLaunch {
            hip_code: 7,
            kernel: "bcjr",
            args: "something went wrong".to_string(),
        });
        let err = dispatch_with_fallback::<u32, _>(Err(fatal), || Ok(0), ctx(), false, &dir)
            .expect_err("fatal must propagate");
        assert!(
            matches!(err, StageError::Fatal(FatalError::KernelLaunch { .. })),
            "fatal must be the original KernelLaunch, got {err:?}"
        );
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dump dir must exist after fatal")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "fatal must write a dump file");
    }

    #[test]
    fn test_type_mismatch_passes_through_without_dump() {
        let (_scratch, dir) = dump_dir();
        let err = StageError::TypeMismatch {
            expected: std::any::TypeId::of::<u32>(),
            actual: std::any::TypeId::of::<u8>(),
        };
        let out = dispatch_with_fallback::<u32, _>(Err(err), || Ok(0), ctx(), false, &dir)
            .expect_err("TypeMismatch must propagate");
        assert!(
            matches!(out, StageError::TypeMismatch { .. }),
            "TypeMismatch must pass through unchanged"
        );
        // TypeMismatch is not a GPU failure; no dump.
        assert!(!dir.exists());
    }

    // ── injects_oom_at (free fn and FailurePolicy method) ──────────────────

    #[test]
    fn test_injects_oom_at_none_never_fires() {
        assert!(!injects_oom_at(None, 0));
        assert!(!injects_oom_at(None, 100));
    }

    /// Modulus 0 is treated as "inactive" (the `m >= 1` guard).
    #[test]
    fn test_injects_oom_at_zero_modulus_never_fires() {
        assert!(!injects_oom_at(Some(0), 0));
        assert!(!injects_oom_at(Some(0), 6));
    }

    #[test]
    fn test_injects_oom_at_modulus_logic() {
        assert!(injects_oom_at(Some(1), 0));
        assert!(injects_oom_at(Some(1), 7));
        assert!(injects_oom_at(Some(3), 0));
        assert!(injects_oom_at(Some(3), 3));
        assert!(injects_oom_at(Some(3), 6));
        assert!(!injects_oom_at(Some(3), 1));
        assert!(!injects_oom_at(Some(3), 2));
        assert!(!injects_oom_at(Some(3), 5));
    }

    #[test]
    fn test_failure_policy_injects_oom_at_delegates() {
        let tmp = std::path::Path::new("/tmp");
        let policy = FailurePolicy {
            strict_gpu: false,
            dump_dir: tmp,
            inject_gpu_oom_modulus: Some(4),
        };
        assert!(policy.injects_oom_at(0));
        assert!(policy.injects_oom_at(4));
        assert!(!policy.injects_oom_at(1));
        let silent = FailurePolicy {
            strict_gpu: false,
            dump_dir: tmp,
            inject_gpu_oom_modulus: None,
        };
        assert!(!silent.injects_oom_at(0));
    }

    // ── Fatal variants in write_diagnostic_dump ─────────────────────────────

    #[test]
    fn test_fatal_device_unavailable_propagates_and_writes_dump() {
        let (_scratch, dir) = dump_dir();
        let fatal = StageError::Fatal(FatalError::DeviceUnavailable);
        let err = dispatch_with_fallback::<u32, _>(Err(fatal), || Ok(0), ctx(), false, &dir)
            .expect_err("DeviceUnavailable must propagate");
        assert!(
            matches!(err, StageError::Fatal(FatalError::DeviceUnavailable)),
            "got {err:?}"
        );
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dump dir must exist after DeviceUnavailable")
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            !entries.is_empty(),
            "DeviceUnavailable fatal must write a dump file"
        );
    }

    #[test]
    fn test_fatal_build_error_propagates_and_writes_dump() {
        use crate::error::BuildError;
        let (_scratch, dir) = dump_dir();
        let fatal = StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
            reason: "unit test build error".to_string(),
        }));
        let err = dispatch_with_fallback::<u32, _>(Err(fatal), || Ok(0), ctx(), false, &dir)
            .expect_err("BuildError must propagate");
        assert!(
            matches!(err, StageError::Fatal(FatalError::BuildError(_))),
            "got {err:?}"
        );
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dump dir must exist after BuildError fatal")
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            !entries.is_empty(),
            "BuildError fatal must write a dump file"
        );
    }

    /// A `dump_dir` that cannot be created is logged and skipped, and
    /// `dispatch_with_fallback` still propagates the original fatal error.
    #[test]
    fn test_write_dump_silently_skips_on_unwritable_dir() {
        // /dev/null is a character device — create_dir_all("/dev/null/…")
        // fails with ENOTDIR on Linux, exercising the create_dir_all error arm.
        let impossible = std::path::Path::new("/dev/null/gf2sim-impossible-dump-test");
        let fatal = StageError::Fatal(FatalError::DeviceUnavailable);
        let err = dispatch_with_fallback::<u32, _>(Err(fatal), || Ok(0), ctx(), false, impossible)
            .expect_err("DeviceUnavailable must propagate even when dump dir fails");
        assert!(
            matches!(err, StageError::Fatal(FatalError::DeviceUnavailable)),
            "original fatal must be returned unchanged: {err:?}"
        );
        assert!(!impossible.exists());
    }
}
