//! The arm binary speaks the runner's canonical child protocol: it answers a
//! request with one result line the runner's strict decoder accepts, honours
//! the cache-state and cold-call declarations, reports conversion costs only
//! for whole-consumer cases, and fails closed on a window or arm it cannot
//! measure.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use tuning_campaign_support::protocol::CacheState;
use tuning_campaign_support::receipt::{ArmQuality, ConversionCosts, WindowRecord};
use tuning_campaign_support::transport;

/// Field-for-field mirror of the runner's private `ArmResult`, decoded through
/// the shared canonical parser the runner uses.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmResult {
    schema: String,
    windows: Vec<WindowRecord>,
    cache_state_applied: CacheState,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<ConversionCosts>,
    quality: Option<ArmQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calibrated: Option<bool>,
}

fn request(case: Value, cache_state: &str, cold_calls: Option<u64>) -> String {
    // Field order and compact encoding follow the runner's ArmRequest.
    let mut fields = vec![
        ("schema", json!("zen3-benchmark-arm-request-v1")),
        ("cell_id", json!("test-cell")),
        ("arm", json!("candidate")),
        ("role", json!("candidate")),
        ("pair", json!(0)),
        ("case", case),
        ("cache_state", json!(cache_state)),
    ];
    if let Some(calls) = cold_calls {
        fields.push(("cold_calls", json!(calls)));
    }
    fields.extend([
        ("windows", json!(5)),
        ("window_target_ms", json!(2)),
        ("cpus", json!([0])),
        ("workers_declared", json!(1)),
    ]);
    let body: Vec<String> = fields
        .into_iter()
        .map(|(key, value)| format!("{}:{}", json!(key), serde_json::to_string(&value).unwrap()))
        .collect();
    format!("{{{}}}", body.join(","))
}

fn run(arm: &str, input: &str) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_popcount-arm"))
        .env("GF2_POPCOUNT_ARM", arm)
        .env("GF2_TUNING_FRESH_CASE", "child-v2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
    )
}

fn result(arm: &str, input: &str) -> Value {
    let (code, stdout) = run(arm, input);
    assert_eq!(code, 0, "{arm} failed");
    let parsed: ArmResult = transport::parse_result(&stdout)
        .unwrap_or_else(|error| panic!("{arm}: runner would reject the result: {error}"));
    assert_eq!(parsed.schema, "zen3-benchmark-arm-result-v1");
    serde_json::to_value(parsed).unwrap()
}

fn popcount_case(words: u64, offset: u64) -> Value {
    json!({"op": "popcount", "pattern": "random", "seed": 3, "word_offset": offset, "words": words})
}

#[test]
fn warm_popcount_cells_calibrate_five_windows() {
    for arm in ["production-dispatch", "libpopcnt", "mula-avx2-harley-seal"] {
        let value = result(arm, &request(popcount_case(64, 0), "warm", None));
        assert_eq!(value["windows"].as_array().unwrap().len(), 5);
        assert_eq!(value["calibrated"], json!(true));
        assert_eq!(value["cache_state_applied"], json!("warm"));
        assert_eq!(value["conversion"], Value::Null);
        assert!(value["selected_path"].as_str().unwrap().len() > 3);
    }
}

#[test]
fn cold_cells_run_exactly_the_frozen_calls() {
    let value = result("nibble-lut", &request(popcount_case(8, 0), "cold", Some(3)));
    assert_eq!(value["calibrated"], json!(false));
    for window in value["windows"].as_array().unwrap() {
        assert_eq!(window["calls"], json!(3));
    }
    let (code, _) = run("nibble-lut", &request(popcount_case(8, 0), "cold", None));
    assert_ne!(code, 0, "a cold cell without frozen calls must fail");
}

#[test]
fn whole_consumer_and_cells_report_conversion_costs() {
    let case = |whole: bool| {
        json!({"op": "and_popcnt", "pattern": "random", "seed_lhs": 1, "seed_rhs": 1001,
               "whole_consumer": whole, "word_offset": 0, "words": 64})
    };
    for arm in ["and-fused", "and-two-pass"] {
        let whole = result(arm, &request(case(true), "streaming", None));
        assert!(whole["conversion"].is_object(), "{arm}");
        let isolated = result(arm, &request(case(false), "warm", None));
        assert_eq!(isolated["conversion"], Value::Null, "{arm}");
    }
    // Only the two-pass route copies its operand, so only it reports a
    // temporary-copy cost; the fused kernel makes no per-call selection.
    let fused = result("and-fused", &request(case(true), "warm", None));
    assert_eq!(fused["conversion"]["pack_ns"], json!(0));
    assert_eq!(fused["conversion"]["dispatch_ns"], json!(0));
    let two_pass = result("and-two-pass", &request(case(true), "warm", None));
    assert!(two_pass["conversion"]["pack_ns"].as_u64().unwrap() > 0);
}

#[test]
fn unmeasurable_requests_fail_closed() {
    let (code, stdout) = run(
        "mula-avx2-harley-seal",
        &request(popcount_case(64, 1), "warm", None),
    );
    assert_ne!(code, 0);
    assert!(stdout.is_empty());
    let (code, _) = run("and-fused", &request(popcount_case(64, 0), "warm", None));
    assert_ne!(code, 0, "an AND arm must refuse a popcount case");
    let (code, _) = run("no-such-arm", &request(popcount_case(64, 0), "warm", None));
    assert_ne!(code, 0);
}
