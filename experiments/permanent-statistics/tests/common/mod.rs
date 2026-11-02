#![allow(dead_code)]
use experiments_common::{RunRecord, Sha256Digest};
use permanent_statistics::campaign::{
    dataset,
    runner::{run_cell, Invocation, RunStatus},
    Backend, CampaignConfig,
};
use std::{fs, path::Path};

pub fn config() -> CampaignConfig {
    serde_json::from_str(include_str!("../../config/small-campaign.json")).unwrap()
}
pub fn record(config: &CampaignConfig) -> RunRecord {
    serde_json::from_value(serde_json::json!({
        "identity": {
            "executable_sha256": Sha256Digest::of(b"test executable"),
            "build": {"profile":"ci-test","opt_level":"3","target":"test","rustflags":[]},
            "configuration_sha256": config.digest().unwrap(),"seed":config.root_seed,
            "rng":{"algorithm":"cha_cha20","version":"rand_chacha 0.9.0"},
            "toolchain_version":"rustc 1.95.0",
            "host":{"cpu":{"model":"test CPU","physical_cores":4,"logical_threads":4},"accelerator":{"runtime":null,"model":null}}
        },"source_revision":"1111111111111111111111111111111111111111"
    })).unwrap()
}
pub fn finish(config: &CampaignConfig, record: &RunRecord, path: &Path, invocation: &Invocation) {
    for cell in config.cells() {
        loop {
            let result = run_cell(config, record, path, cell.q, cell.n, invocation).unwrap();
            if result == RunStatus::Terminal {
                break;
            }
        }
    }
    dataset::finalize(path).unwrap();
    dataset::verify(path).unwrap();
}
pub fn shards(path: &Path, q: u8, n: u16) -> Vec<dataset::ShardRecord> {
    fs::read_to_string(path.join(dataset::shard_path(q, n)))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
pub fn events(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path.join("execution.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
pub fn cpu_backends() -> [Backend; 4] {
    [
        Backend::Scalar,
        Backend::BatchParallel,
        Backend::IntraMatrixParallel,
        Backend::GenericRyser,
    ]
}
