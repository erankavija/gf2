//! Two runs of the `dvb_t2_awgn_campaign` binary at the same seed and
//! configuration produce byte-identical `fer`, `frames`, `errors` and
//! `mean_iters` CSV columns; `ber` and `wall_seconds` are not compared.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use gf2_core::test_scratch::{scratch, Scratch};

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dvb_t2_awgn_campaign"))
}

/// The compared CSV columns, keyed by `es_n0_db`.
#[derive(Debug, Clone, PartialEq)]
struct DetRow {
    es_n0_db: String,
    fer: String,
    frames: String,
    errors: String,
    mean_iters: String,
}

fn parse_det_rows(csv: &str) -> Vec<DetRow> {
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            assert_eq!(
                c.len(),
                7,
                "campaign CSV must have 7 columns, got line: {l}"
            );
            DetRow {
                es_n0_db: c[0].to_string(),
                fer: c[1].to_string(),
                frames: c[3].to_string(),
                errors: c[4].to_string(),
                mean_iters: c[5].to_string(),
            }
        })
        .collect()
}

fn out_path(root: &Scratch) -> String {
    root.join("out")
        .to_str()
        .expect("scratch paths are UTF-8")
        .to_owned()
}

fn run_campaign(
    out_dir: String,
    esn0_range: &str,
    max_frames: &str,
    target_errors: &str,
) -> Vec<DetRow> {
    let out_dir = out_dir.as_str();
    let bin = binary_path();
    let status = Command::new(&bin)
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            esn0_range,
            "--max-frames",
            max_frames,
            "--target-errors",
            target_errors,
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            out_dir,
            "--seed",
            "42",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(status.success(), "campaign run must succeed");
    let csv = std::fs::read_to_string(format!("{out_dir}/curve_1_2_16qam.csv"))
        .expect("campaign curve CSV must be written");
    parse_det_rows(&csv)
}

#[test]
#[ignore = "sim: two full-codec subprocess runs for binary two-run byte-identity"]
fn byte_identical_two_runs_smoke() {
    let leg_a = scratch("gf2-byteid-smoke-a");
    let leg_b = scratch("gf2-byteid-smoke-b");
    let a = run_campaign(out_path(&leg_a), "6.25:6.25:0.5", "8", "1000");
    let b = run_campaign(out_path(&leg_b), "6.25:6.25:0.5", "8", "1000");
    assert_eq!(a.len(), 1, "one SNR point");
    assert_eq!(
        a, b,
        "two runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
    assert_eq!(a[0].frames, "8", "max_frames honoured");
}

/// 6.0 dB is the r1/2 16-QAM waterfall point for this decoder, demapper and
/// seed, where some but not all of the 200 frames fail.
#[test]
#[ignore = "sim: 200-frame n=64800 DVB-T2 BICM waterfall two-run byte-identity"]
fn byte_identical_two_runs_waterfall() {
    let leg_a = scratch("gf2-byteid-waterfall-a");
    let leg_b = scratch("gf2-byteid-waterfall-b");
    let a = run_campaign(out_path(&leg_a), "6.0:6.0:0.5", "200", "100000");
    let b = run_campaign(out_path(&leg_b), "6.0:6.0:0.5", "200", "100000");
    assert_eq!(a.len(), 1);

    let frames: u64 = a[0].frames.parse().unwrap();
    let errors: u64 = a[0].errors.parse().unwrap();
    assert_eq!(frames, 200, "the full frame budget ran");
    assert!(
        0 < errors && errors < frames,
        "6.0 dB r1/2 16-QAM must be a non-vacuous waterfall: 0 < errors ({errors}) < frames ({frames})"
    );

    assert_eq!(
        a, b,
        "two 200-frame runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
}
