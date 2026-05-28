//! Integration test: SIGINT + resume byte-identity.
//!
//! Spawns `dvb_t2_awgn_campaign` as a subprocess, runs 3 SNR points, sends
//! SIGINT after the first SNR point completes (detected via the tracing JSONL
//! log), re-invokes with `--resume`, and asserts the final CSV is byte-identical
//! to a same-seed uninterrupted reference run.
//!
//! The test is marked `#[ignore]` because it requires SIGINT timing that is
//! inherently slow: the runner needs to complete at least one full SNR point
//! before SIGINT is sent, and the per-SNR-point runtime at minimum frame
//! budgets is 2–10 s on release builds (the first call initialises the
//! Richardson-Urbanke LDPC encoder, which takes that long for Normal frames).
//!
//! Run manually when validating resumability:
//!
//! ```bash
//! cargo nextest run -p gf2-coding --release \
//!     --run-ignored ignored-only \
//!     -E 'test(resume_byte_identity)'
//! ```

#[ignore = "slow: subprocess SIGINT timing exceeds fast tier"]
#[test]
fn resume_byte_identity() {
    use std::io::BufRead;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    let binary = {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Navigate up to workspace root, then down to release binary.
        p.pop(); // gf2-coding
        p.pop(); // crates
        p.pop(); // workspace root
        p.join("target")
            .join("release")
            .join("dvb_t2_awgn_campaign")
    };

    if !binary.exists() {
        panic!(
            "Binary not found at {}. Build with: cargo build -p gf2-coding --release \
             --bin dvb_t2_awgn_campaign",
            binary.display()
        );
    }

    // Use a temp directory that survives between interrupted and resumed runs.
    let out_dir = std::env::temp_dir().join(format!(
        "dvb_t2_resume_test_{}_{}",
        std::process::id(),
        // Use nanos to make the path unique across parallel test processes.
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0),
    ));
    let ref_dir = std::env::temp_dir().join(format!(
        "dvb_t2_resume_ref_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0),
    ));
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&ref_dir);

    let common_args = [
        "--rate",
        "1/2",
        "--modulation",
        "16qam",
        "--esn0-range",
        "5.5:6.5:0.5", // 3 points: 5.5, 6.0, 6.5
        "--max-frames",
        "200", // small but deterministic
        "--target-errors",
        "3",
        "--seed",
        "0xDEADBEEF",
    ];

    // -----------------------------------------------------------------------
    // Step 1: Interrupted run — start and SIGINT after first SNR completes.
    // -----------------------------------------------------------------------
    let tracing_jsonl = out_dir.join("tracing.jsonl");

    let mut child = std::process::Command::new(&binary)
        .args(common_args)
        .args(["--output-dir", out_dir.to_str().expect("utf8 path")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("Failed to spawn dvb_t2_awgn_campaign for interrupted run");

    // Wait for first SNR-completed event in the JSONL log (up to 120 s).
    let timeout = Duration::from_secs(120);
    let started = Instant::now();
    let mut first_snr_done = false;
    while started.elapsed() < timeout {
        if tracing_jsonl.exists() {
            if let Ok(file) = std::fs::File::open(&tracing_jsonl) {
                let reader = std::io::BufReader::new(file);
                let snr_events = reader
                    .lines()
                    .map_while(Result::ok)
                    .filter(|l| l.contains("\"snr_completed\""))
                    .count();
                if snr_events >= 1 {
                    first_snr_done = true;
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    if !first_snr_done {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&out_dir);
        let _ = std::fs::remove_dir_all(&ref_dir);
        panic!(
            "Timed out waiting for first SNR point to complete in interrupted run \
             (checked {})",
            tracing_jsonl.display()
        );
    }

    // Send SIGINT to the process via `kill -INT <pid>`.
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status();
    }
    #[cfg(not(unix))]
    {
        // On non-Unix we just kill the process (no SIGINT support).
        let _ = child.kill();
    }

    let status = child.wait().expect("Failed to wait on interrupted run");
    // The process exits non-zero after SIGINT — that's expected.
    let _ = status;

    // -----------------------------------------------------------------------
    // Step 2: Resumed run.
    // -----------------------------------------------------------------------
    let status = std::process::Command::new(&binary)
        .args(common_args)
        .args([
            "--output-dir",
            out_dir.to_str().expect("utf8 path"),
            "--resume",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to spawn dvb_t2_awgn_campaign for resumed run");
    assert!(status.success(), "Resumed run exited with status {status}");

    // -----------------------------------------------------------------------
    // Step 3: Uninterrupted reference run.
    // -----------------------------------------------------------------------
    let status = std::process::Command::new(&binary)
        .args(common_args)
        .args(["--output-dir", ref_dir.to_str().expect("utf8 path")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to spawn dvb_t2_awgn_campaign for reference run");
    assert!(
        status.success(),
        "Reference run exited with status {status}"
    );

    // -----------------------------------------------------------------------
    // Step 4: Compare CSVs.
    // -----------------------------------------------------------------------
    let csv_name = "curve_1_2_16qam.csv";
    let resumed_csv = out_dir.join(csv_name);
    let ref_csv = ref_dir.join(csv_name);

    let resumed = std::fs::read_to_string(&resumed_csv)
        .unwrap_or_else(|e| panic!("Cannot read resumed CSV {}: {e}", resumed_csv.display()));
    let reference = std::fs::read_to_string(&ref_csv)
        .unwrap_or_else(|e| panic!("Cannot read reference CSV {}: {e}", ref_csv.display()));

    assert_eq!(
        resumed,
        reference,
        "Resumed CSV differs from uninterrupted reference CSV.\n\
         Resumed ({}):\n{resumed}\n\
         Reference ({}):\n{reference}",
        resumed_csv.display(),
        ref_csv.display(),
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&ref_dir);
}
