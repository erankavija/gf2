//! A forced kernel error in the `hard_fail_probe` subprocess yields a non-zero
//! process exit, a JSON diagnostic dump in the configured directory, and a
//! `tracing::error!` event carrying the HIP error code, device id, SNR index
//! and batch id. The probe constructs the fatal error on the host, so the test
//! needs no GPU.

use std::path::PathBuf;
use std::process::Command;

/// The dump directory is not created here; the probe's dump writer creates it.
fn temp_dump_dir() -> (gf2_core::test_scratch::Scratch, PathBuf) {
    let scratch = gf2_core::test_scratch::scratch("gf2sim-hardfail-subproc");
    let dump_dir = scratch.path().join("dump");
    (scratch, dump_dir)
}

#[test]
fn test_hard_fail_subprocess_nonzero_exit_dump_and_error_event() {
    let probe = env!("CARGO_BIN_EXE_hard_fail_probe");
    let (_scratch, dump_dir) = temp_dump_dir();

    let output = Command::new(probe)
        .arg("--diagnostic-dump-dir")
        .arg(&dump_dir)
        .output()
        .expect("spawn hard_fail_probe");

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
    let dump: serde_json::Value = serde_json::from_str(&content).expect("dump must be valid JSON");

    assert_eq!(
        dump["event"], "hard_fail",
        "event field must be 'hard_fail'"
    );
    assert_eq!(
        dump["hip_code"], 301_i64,
        "HIP error code must be carried in the dump"
    );
    assert_eq!(
        dump["device_id"], 0_i64,
        "device id context must be present"
    );
    assert_eq!(dump["snr_idx"], 3_i64, "SNR index context must be present");
    assert_eq!(dump["batch_id"], 42_i64, "batch id context must be present");
    assert_eq!(
        dump["kernel"], "ldpc_bp",
        "kernel name context must be present"
    );

    // The probe prints ERROR events to stderr as JSON lines.
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
