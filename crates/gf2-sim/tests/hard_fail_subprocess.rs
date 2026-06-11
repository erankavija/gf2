//! Hard-fail path, proven by a **real subprocess** (issue `42eac5cc` SC3;
//! design doc §8).
//!
//! Criterion (verbatim):
//!
//! > a forced kernel error yields a non-zero exit, a JSON diagnostic dump in
//! > the configured directory, and a `tracing::error!` event with all the
//! > relevant context.
//!
//! The round-1 review FAILED because the criterion was "proven" in-process —
//! asserting an `Err` return with a comment that this *is how* the process would
//! exit non-zero. That reasoning has been rejected twice on this project
//! (`5f12e7ff` and the `42eac5cc` round-1 review): a non-zero **process exit**
//! must be asserted by actually spawning a process and reading its status.
//!
//! This test spawns the `hard_fail_probe` test-only binary (built by cargo and
//! located via `CARGO_BIN_EXE_hard_fail_probe`), which drives the production
//! [`dispatch_with_fallback`](gf2_sim::executor::failure::dispatch_with_fallback)
//! hard-fail boundary with the exact fatal error a `KernelErrorInjector`-wrapped
//! GPU stage produces, then exits `1`. The test asserts:
//!
//! * the child exit status is non-zero;
//! * a JSON diagnostic dump exists in the configured (temp) directory and parses,
//!   carrying the contextual fields (HIP error code, device id, SNR index, batch
//!   id, kernel name);
//! * a `tracing::error!` ERROR event was emitted (the probe prints ERROR events
//!   to stderr as JSON-lines; the test captures the child's stderr and finds
//!   the hard-fail event carrying the same context).
//!
//! No GPU is required — the fatal error is constructed on the host, exactly as
//! `KernelErrorInjector::process` does, so this test is **not** GPU-gated and
//! runs on every fast-tier CI invocation.

use std::path::PathBuf;
use std::process::Command;

/// Returns a fresh temp directory path for one probe invocation (not created;
/// the probe's dump writer creates it on demand).
fn temp_dump_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "gf2sim-hardfail-subproc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

#[test]
fn test_hard_fail_subprocess_nonzero_exit_dump_and_error_event() {
    // The probe binary path, injected by cargo for integration tests.
    let probe = env!("CARGO_BIN_EXE_hard_fail_probe");
    let dump_dir = temp_dump_dir();

    let output = Command::new(probe)
        .arg("--diagnostic-dump-dir")
        .arg(&dump_dir)
        .output()
        .expect("spawn hard_fail_probe");

    // SC3a: the process exits NON-ZERO (a real exit status, not an in-process
    // Err). On a forced kernel hard-fail the probe exits 1.
    assert!(
        !output.status.success(),
        "hard-fail probe must exit non-zero; got status {:?}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "hard-fail probe must exit with status 1"
    );

    // SC3b: a JSON diagnostic dump file exists in the configured directory and
    // parses, carrying the contextual fields.
    let entries: Vec<_> = std::fs::read_dir(&dump_dir)
        .expect("dump dir must exist after the hard-fail")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "exactly one JSON dump file must be written for one hard-fail"
    );
    let dump_path = entries[0].path();
    let content = std::fs::read_to_string(&dump_path).expect("dump file must be readable");
    let dump: serde_json::Value =
        serde_json::from_str(&content).expect("dump must be valid JSON");

    assert_eq!(dump["event"], "hard_fail", "event field must be 'hard_fail'");
    assert_eq!(
        dump["hip_code"], 301_i64,
        "HIP error code must be carried in the dump"
    );
    assert_eq!(dump["device_id"], 0_i64, "device id context must be present");
    assert_eq!(dump["snr_idx"], 3_i64, "SNR index context must be present");
    assert_eq!(dump["batch_id"], 42_i64, "batch id context must be present");
    assert_eq!(
        dump["kernel"], "ldpc_bp",
        "kernel name context must be present"
    );

    // SC3c: a `tracing::error!` ERROR event was emitted. The probe prints ERROR
    // events to stderr as JSON-lines; find the hard-fail abort event and confirm
    // it carries the same context (hip_code + batch/snr/device).
    let stderr = String::from_utf8_lossy(&output.stderr);
    let error_event_line = stderr.lines().find(|line| {
        line.contains("\"level\":\"ERROR\"")
            && line.contains("GPU stage hard-fail")
            && line.contains("\"hip_code\":\"301\"")
            && line.contains("\"batch_id\":\"42\"")
            && line.contains("\"snr_idx\":\"3\"")
            && line.contains("\"device_id\":\"0\"")
    });
    assert!(
        error_event_line.is_some(),
        "expected a tracing::error! hard-fail event with hip_code/batch_id/snr_idx/device_id \
         context on the child's stderr; full stderr:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&dump_dir);
}
