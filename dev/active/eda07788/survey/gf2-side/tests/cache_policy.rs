//! Both timed arms refuse a cache state they do not implement before doing
//! any work, so no result line can report it as applied.

use std::io::Write;
use std::process::{Command, Output, Stdio};
use tuning_campaign_support::transport::{FRESH_CASE_VALUE, FRESH_CASE_VAR};

const ARMS: [&str; 2] = [
    env!("CARGO_BIN_EXE_gf2-dvb-t2-candidate"),
    env!("CARGO_BIN_EXE_xdsopl-dvb-t2-baseline"),
];

fn run_arm(executable: &str, cache_state: &str) -> Output {
    // Canonical compact JSON in the arms' request field order.
    let request = format!(
        concat!(
            r#"{{"schema":"zen3-benchmark-arm-request-v1","cell_id":"cache-policy","#,
            r#""arm":"fixture","role":"exploratory","pair":0,"#,
            r#""case":{{"modcod":"qam16-r12-short","seed":1}},"cache_state":"{}","#,
            r#""windows":5,"window_target_ms":100,"cpus":[],"workers_declared":1}}"#
        ),
        cache_state
    );
    let mut child = Command::new(executable)
        .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("arm executable starts");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(request.as_bytes())
        .expect("request is written");
    child.wait_with_output().expect("arm exits")
}

#[test]
fn arms_refuse_unimplemented_cache_states() {
    for executable in ARMS {
        for state in ["cold", "hot"] {
            let output = run_arm(executable, state);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{executable} {state}: {stderr}"
            );
            assert!(output.stdout.is_empty(), "{executable} {state}");
            assert!(
                stderr.contains("is not implemented"),
                "{executable} {state}: {stderr}"
            );
        }
    }
}
