//! Integration test: argv-level acceptance and rejection of `--decoder` /
//! `--demap` on the DVB-T2 BICM AWGN campaign binary.
//!
//! Spawns `dvb_t2_awgn_campaign` as a subprocess with the new flags set to
//! invalid values and asserts the binary exits non-zero with a parse-error
//! message on stderr. This covers the argv-level CLI path end-to-end (binary
//! → `parse_args` → `parse_decoder`/`parse_demap`) — the in-binary parser
//! unit tests cover the parsing functions in isolation; these tests cover
//! the wiring that routes the argv into them.
//!
//! All four tests fail fast at the parse stage, so no codec / encoder /
//! simulation work runs — they stay well within the fast tier budget.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the binary that cargo built for this integration test.
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dvb_t2_awgn_campaign"))
}

/// Minimal valid-shape argv (rate, modulation, esn0-range, output-dir) so
/// the parser reaches the flag under test instead of erroring on a missing
/// earlier required field.
fn base_args(output_dir: &str) -> Vec<String> {
    vec![
        "--rate".into(),
        "1/2".into(),
        "--modulation".into(),
        "16qam".into(),
        "--esn0-range".into(),
        "5.0:5.0:0.5".into(),
        "--output-dir".into(),
        output_dir.into(),
    ]
}

#[test]
fn cli_rejects_unknown_decoder_algorithm() {
    let bin = binary_path();
    let out = Command::new(&bin)
        .args(base_args("/tmp/dvb_cli_reject_decoder_unknown"))
        .args(["--decoder", "bogusalgo"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "expected non-zero exit on unknown --decoder value"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("decoder") || stderr.contains("Unknown") || stderr.contains("bogusalgo"),
        "stderr should mention the decoder parse error; got: {}",
        stderr
    );
}

#[test]
fn cli_rejects_unknown_demap_method() {
    let bin = binary_path();
    let out = Command::new(&bin)
        .args(base_args("/tmp/dvb_cli_reject_demap_unknown"))
        .args(["--demap", "softoutput"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "expected non-zero exit on unknown --demap value"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("demap") || stderr.contains("Unknown") || stderr.contains("softoutput"),
        "stderr should mention the demap parse error; got: {}",
        stderr
    );
}

#[test]
fn cli_rejects_nms_alpha_out_of_range() {
    let bin = binary_path();
    let out = Command::new(&bin)
        .args(base_args("/tmp/dvb_cli_reject_nms_range"))
        .args(["--decoder", "nms:1.5"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "expected non-zero exit on nms alpha > 1.0 (which would panic in DecoderConfig::new)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("nms") || stderr.contains("alpha"),
        "stderr should mention the alpha-range error; got: {}",
        stderr
    );
}

#[test]
fn cli_rejects_oms_negative_beta() {
    let bin = binary_path();
    let out = Command::new(&bin)
        .args(base_args("/tmp/dvb_cli_reject_oms_neg"))
        .args(["--decoder", "oms:-0.1"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "expected non-zero exit on negative oms beta (which would panic in DecoderConfig::new)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("oms") || stderr.contains("beta"),
        "stderr should mention the beta-range error; got: {}",
        stderr
    );
}
