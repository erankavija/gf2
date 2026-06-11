//! OOM auto-fallback dispatch and hard-fail diagnostic dump (Phase C task
//! `42eac5cc`, design doc §8).
//!
//! [`dispatch_with_fallback`] is the single call site wrapping every GPU stage
//! invocation in both the C.1 hybrid scheduler loop
//! (`executor/scheduler.rs::worker_partition_hybrid`) and the topology
//! executor's `GpuOnly` arm (`executor/topology.rs::execute_gpu_stage`). It
//! implements the full §8 failure-mode decision tree:
//!
//! ```text
//! GPU stage call
//!   ├── Ok(output) → return output
//!   ├── Err(Recoverable(OutOfMemory)) + strict_gpu → FatalError::OutOfMemory
//!   ├── Err(Recoverable(OutOfMemory)) + !strict_gpu →
//!   │       cpu_fallback().process(input) →
//!   │           Ok(output) → warn + return output
//!   │           Err(_)     → FatalError::CpuFallbackAlsoFailed
//!   ├── Err(Recoverable(Transient)) → cpu_fallback().process(input)
//!   │       (same branching — Transient is NEVER promoted, even under
//!   │        strict_gpu: the §8 strict row covers OOM only, and §6 pins
//!   │        UnsupportedArch→Transient as a CPU-fallback path, not fatal)
//!   └── Err(Fatal(_)) → write diagnostic dump + propagate
//! ```
//!
//! # Diagnostic dump
//!
//! On every fatal stage error the executor serialises a JSON diagnostic record
//! to the configured `diagnostic_dump_dir` (from [`PipelineConfig`]). The file
//! is named `<timestamp_ns>-<device_id>-<snr_idx>.json` and is written
//! atomically: the payload is written to a sibling `.tmp` file then renamed.
//! Default directory: `dev/benchmarks/gf2-sim/diagnostic-dumps/`.
//!
//! # `strict_gpu` promotion (OOM only)
//!
//! When `PipelineConfig::strict_gpu` is set, a
//! [`RecoverableError::OutOfMemory`] from a GPU stage is promoted to
//! [`FatalError::OutOfMemory`] (no CPU fallback attempted). The diagnostic
//! dump is written in that case too. The promotion is **OOM-specific**:
//! [`RecoverableError::Transient`] (e.g. the §6 `UnsupportedArch` mapping)
//! takes the CPU fallback even under `strict_gpu` — the design §8 strict row
//! names OOM only, and §6 pins the unsupported-arch path as "CPU fallback,
//! not fatal" with no strict-mode carve-out.
//!
//! [`PipelineConfig`]: crate::PipelineConfig

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{FatalError, RecoverableError, StageError};

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic dump schema
// ─────────────────────────────────────────────────────────────────────────────

/// Context passed to [`dispatch_with_fallback`] for tracing and diagnostics.
/// Carries the per-batch identifiers that appear in the `tracing::warn!` /
/// `tracing::error!` events and in the JSON dump.
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::failure::FaultContext;
///
/// let ctx = FaultContext { batch_id: 7, snr_idx: 2, device_id: 0, worker_idx: 3 };
/// assert_eq!(ctx.batch_id, 7);
/// assert_eq!(ctx.snr_idx, 2);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct FaultContext {
    /// The batch identifier (global frame index or batch sequence number).
    pub batch_id: u64,
    /// The SNR-point index keying the §3 RNG seek.
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
    /// Timestamp in nanoseconds since UNIX epoch (monotone across the run).
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

/// Writes a diagnostic dump for a hard-fail fatal error, then returns the error
/// unchanged for propagation.
///
/// The dump is written to `dump_dir/<timestamp_ns>-<device_id>-<snr_idx>.json`
/// via a `.tmp` sibling + atomic rename. If the write fails (permission error,
/// full filesystem), the original `fatal` error is returned unchanged and a
/// `tracing::error!` is emitted for the I/O failure — the run still aborts on
/// the stage error, not on the dump I/O error.
///
/// # Arguments
///
/// * `fatal` — the fatal error to dump and propagate.
/// * `ctx` — per-batch fault context (device_id, snr_idx, batch_id, worker_idx).
/// * `dump_dir` — directory to write the JSON dump into.
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
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::failure::default_dump_dir;
///
/// let dir = default_dump_dir();
/// assert!(dir.is_relative());
/// assert!(dir.to_str().unwrap().contains("diagnostic-dumps"));
/// ```
pub fn default_dump_dir() -> PathBuf {
    PathBuf::from("dev/benchmarks/gf2-sim/diagnostic-dumps")
}

// ─────────────────────────────────────────────────────────────────────────────
// dispatch_with_fallback
// ─────────────────────────────────────────────────────────────────────────────

/// Wraps a single GPU stage call with full §8 failure-mode handling.
///
/// This function implements the OOM auto-fallback and hard-fail paths described
/// in design doc §8 and mandated by issue `42eac5cc`. It is the **single call
/// boundary** wrapping every GPU dispatch in both the C.1 hybrid scheduler loop
/// (`worker_partition_hybrid` in `executor/scheduler.rs`) and the topology
/// executor's `GpuOnly` arm (`execute_gpu_stage` in `executor/topology.rs`).
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
///   │       REGARDLESS of strict_gpu — the §8 strict promotion covers OOM
///   │       only; §6 pins UnsupportedArch→Transient as CPU-fallback-not-fatal
///   │       with no strict-mode carve-out
///   └── Err(Fatal(_)) → write dump, emit tracing::error! → return Err(Fatal(_))
/// ```
///
/// # Arguments
///
/// * `gpu_result` — the `Result` returned by the GPU stage call.
/// * `run_fallback` — closure that runs the CPU fallback stage on the same
///   input. Called only on a recoverable error with `!strict_gpu`.
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

            // Strict mode promotes OOM — and ONLY OOM — to fatal (design §8's
            // strict row; §6's UnsupportedArch→Transient mapping is a
            // CPU-fallback path with no strict-mode carve-out, so Transient
            // falls through to the fallback below even under strict_gpu).
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
                    // The CPU fallback also failed: escalate.
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

    fn dump_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "gf2sim-failure-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        p
    }

    #[test]
    fn test_ok_passes_through() {
        let dir = dump_dir();
        let result: Result<u32, StageError> = Ok(42);
        let out = dispatch_with_fallback(result, || Ok(0), ctx(), false, &dir);
        assert_eq!(out.unwrap(), 42);
        // No dump was written.
        assert!(!dir.exists());
    }

    #[test]
    fn test_oom_non_strict_invokes_fallback() {
        let dir = dump_dir();
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
        let dir = dump_dir();
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
        // A dump file must have been written.
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dump dir must exist after strict OOM")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "at least one dump file must exist");
        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `strict_gpu` promotes OOM ONLY: a `Transient` recoverable error takes
    /// the CPU fallback even under `strict_gpu` (design §8 strict row is
    /// OOM-specific; §6 pins UnsupportedArch→Transient as CPU-fallback,
    /// not fatal). No dump is written when the fallback succeeds.
    #[test]
    fn test_transient_under_strict_gpu_still_falls_back() {
        let dir = dump_dir();
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
        let dir = dump_dir();
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fatal_writes_dump_and_propagates() {
        let dir = dump_dir();
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_type_mismatch_passes_through_without_dump() {
        let dir = dump_dir();
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
}
