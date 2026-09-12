//! Shared request and result types for the encoder child arms.
//!
//! Each arm binary includes this module directly, because a `[[bin]]` target
//! cannot depend on a sibling binary and the harness library stays free of
//! the campaign transport types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub cell_id: String,
    pub arm: String,
    pub role: String,
    pub pair: u32,
    pub case: Value,
    pub cache_state: String,
    pub windows: u32,
    pub window_target_ms: u32,
    pub cpus: Vec<u32>,
    pub workers_declared: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub configuration: String,
    pub seed: u64,
}

#[derive(Serialize)]
pub struct Window {
    pub calls: u64,
    pub elapsed_ns: u64,
}

#[derive(Serialize)]
pub struct ConversionCosts {
    pub setup_ns: u64,
    pub pack_ns: u64,
    pub unpack_ns: u64,
    pub batch_fill_ns: u64,
    pub dispatch_ns: u64,
}

#[derive(Serialize)]
pub struct ArmResult {
    pub schema: String,
    pub windows: Vec<Window>,
    pub cache_state_applied: String,
    pub workers_observed: u32,
    pub cpus_observed: Vec<u32>,
    pub selected_path: Option<String>,
    pub conversion: Option<ConversionCosts>,
    pub quality: Option<Value>,
}

pub fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).expect("duration fits in u64")
}
