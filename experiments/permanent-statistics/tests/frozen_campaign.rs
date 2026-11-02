//! Canonical suite for the frozen campaign's stable inputs.
use gf2_algebra::permanent::{
    determinant_singular_probability, try_visit_permanent_anchor_matrices,
};
use permanent_statistics::campaign::{dataset::Manifest, CampaignConfig};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Address namespace of the validation evidence in `@/issue/02b8137c`.
const SEPTEMBER_ROOT_SEED: u64 = 0x4453_4b2f_0000_0001;
const SEPTEMBER_STREAM_INDEX: u64 = 0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preregistration {
    campaign_id: String,
    protocol: String,
    configuration: Pin,
    stream_purpose: String,
    root_seed: u64,
    regeneration_matrix_count: u64,
    sample_matrix_count: u64,
    test: Test,
    retry_rule: String,
    disjoint_from: Prior,
    anchors: Vec<Anchor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Test {
    kind: String,
    level: f64,
    rule: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    issue: String,
    root_seed: u64,
    stream_index: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Anchor {
    q: u8,
    n: u16,
    stream_index: u64,
    expected_matrix_count: u64,
    expected_permanent_zero_count: u64,
    expected_determinant_zero_count: u64,
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn config() -> CampaignConfig {
    CampaignConfig::load(&crate_dir().join("config/campaign.json")).unwrap()
}
fn preregistration() -> Preregistration {
    let bytes = std::fs::read(crate_dir().join("preregistration.json")).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn repository_file(path: &str) -> PathBuf {
    crate_dir().join("../..").join(Path::new(path))
}

#[test]
fn the_frozen_configuration_holds_the_preregistered_cells() {
    let config = config();
    let cells = config.cells();
    let tier = |q: u8, n: u16| match (q, n) {
        (3, 4..=20) => Some((20_000_000, 200_000)),
        (3, 21..=28) => Some((222_223, 50_000)),
        (5, 4..=16) => Some((16_000_000, 200_000)),
        (5, 17..=24) => Some((160_000, 50_000)),
        (7, 4..=16) => Some((12_244_898, 200_000)),
        (7, 17..=20) => Some((122_449, 50_000)),
        _ => None,
    };
    assert_eq!(cells.len(), 25 + 21 + 17);
    for cell in &cells {
        assert_eq!(
            tier(cell.q, cell.n),
            Some((cell.matrix_count, cell.shard_size))
        );
        assert!(cell.determinant_companion);
    }
    assert_eq!(config.execution.first_cell, (7, 20));
    assert_eq!(config.execution.max_shard_attempts, 2);
    assert_eq!(config.acceptance.permanent_family_error, 0.025);
    assert_eq!(config.acceptance.determinant_family_error, 0.025);
    assert_eq!(config.acceptance.wilson_z, 1.959963984540054);
    assert!(cells.iter().map(|c| c.shard_count()).sum::<u64>() <= 4_500);
}

#[test]
fn validation_addresses_differ_from_the_september_ones() {
    let config = config();
    let validation = &config.validation;
    assert_ne!(validation.root_seed, SEPTEMBER_ROOT_SEED);
    assert_ne!(validation.root_seed, config.root_seed);
    let anchors: Vec<_> = validation.anchors.iter().map(|a| (a.q, a.n)).collect();
    let september: Vec<_> = [(3, 1..=4), (5, 1..=3), (7, 1..=3)]
        .into_iter()
        .flat_map(|(q, orders)| orders.map(move |n| (q, n)))
        .collect();
    assert_eq!(anchors, september);
    let mut indices = Vec::new();
    for anchor in &validation.anchors {
        assert_ne!(anchor.stream_index, SEPTEMBER_STREAM_INDEX);
        indices.push(anchor.stream_index);
    }
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(indices.len(), anchors.len());
    let prior = preregistration().disjoint_from;
    assert_eq!(prior.issue, "02b8137c");
    assert_eq!(prior.root_seed, SEPTEMBER_ROOT_SEED);
    assert_eq!(prior.stream_index, SEPTEMBER_STREAM_INDEX);
}

#[test]
fn the_preregistration_agrees_with_the_configuration_the_tool_consumes() {
    let config = config();
    let validation = &config.validation;
    let prereg = preregistration();
    assert_eq!(prereg.campaign_id, config.campaign_id);
    assert!(repository_file(&prereg.protocol).is_file());
    assert_eq!(
        CampaignConfig::load(&repository_file(&prereg.configuration.path)).unwrap(),
        config
    );
    assert_eq!(
        prereg.configuration.sha256,
        config.digest().unwrap().to_string()
    );
    assert_eq!(prereg.stream_purpose, "validation");
    assert_eq!(prereg.root_seed, validation.root_seed);
    assert_eq!(prereg.regeneration_matrix_count, 1024);
    assert_eq!(validation.regeneration_matrix_count, 1024);
    assert_eq!(prereg.sample_matrix_count, 400_000);
    assert_eq!(validation.sample_matrix_count, 400_000);
    assert_eq!(prereg.test.level, 0.001);
    assert_eq!(validation.level, 0.001);
    assert_eq!(
        prereg.test.kind,
        "probability_ordering_exact_two_sided_binomial"
    );
    assert_eq!(prereg.test.rule, "pass_only_when_p_value_exceeds_level");
    assert_eq!(prereg.retry_rule, "no_redraw_after_failed_anchor");
    assert_eq!(prereg.anchors.len(), validation.anchors.len());
    for (a, b) in prereg.anchors.iter().zip(&validation.anchors) {
        assert_eq!(
            (a.q, a.n, a.stream_index, a.expected_permanent_zero_count),
            (b.q, b.n, b.stream_index, b.permanent_zero_count)
        );
        assert_eq!(
            u128::from(a.expected_matrix_count),
            u128::from(a.q).pow(u32::from(a.n).pow(2))
        );
        let singular = determinant_singular_probability(u64::from(a.q), usize::from(a.n));
        let numerator: u128 = singular.zero_count().to_string().parse().unwrap();
        let denominator: u128 = singular.matrix_count().to_string().parse().unwrap();
        assert_eq!(
            u128::from(a.expected_determinant_zero_count) * denominator,
            u128::from(a.expected_matrix_count) * numerator
        );
    }
}

#[test]
fn tractable_preregistered_zero_counts_match_enumeration() {
    fn zeros<const Q: u64>(n: usize) -> String {
        try_visit_permanent_anchor_matrices::<Q, std::convert::Infallible, _>(n, |_, _| Ok(()))
            .unwrap()
            .zero_count()
            .to_string()
    }
    for anchor in preregistration().anchors {
        let count = match (anchor.q, anchor.n) {
            (3, 1..=3) => zeros::<3>(usize::from(anchor.n)),
            (5, 1..=2) => zeros::<5>(usize::from(anchor.n)),
            (7, 1..=2) => zeros::<7>(usize::from(anchor.n)),
            _ => continue,
        };
        assert_eq!(count, anchor.expected_permanent_zero_count.to_string());
    }
}

#[test]
fn the_committed_manifest_is_the_configuration_projection() {
    let config = config();
    let directory = crate_dir().join("config").join(&config.campaign_id);
    assert_eq!(
        Manifest::load(&directory).unwrap(),
        Manifest::from_config(&config).unwrap()
    );
}
