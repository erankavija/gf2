mod common;
use gf2_stats::sampler::StreamPurpose;
use permanent_statistics::campaign::{
    evaluate_shard,
    validation::{run_validation, BackendStatus, Receipt, ValidationStatus},
    Backend, CampaignConfig,
};
use std::fs;

fn record(config: &CampaignConfig) -> experiments_common::RunRecord {
    let mut r = common::record(config);
    r.identity.seed = config.validation.root_seed;
    r
}
#[test]
fn exact_counts_regeneration_and_samples_share_the_production_contract() {
    let config = common::config();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        run_validation(&config, &record(&config), dir.path(), 4, None).unwrap(),
        ValidationStatus::Passed
    );
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(dir.path().join("receipt.json")).unwrap()).unwrap();
    assert!(receipt.verdict);
    for anchor in &receipt.anchors {
        assert_eq!(anchor.oracle_zero_count, 1);
        assert_eq!(anchor.enumerated, u64::from(anchor.q));
        assert_eq!(anchor.determinant_zero_count, anchor.oracle_zero_count);
        assert!(anchor
            .backends
            .iter()
            .all(|b| b.status == BackendStatus::Unsupported
                || (b.status == BackendStatus::Passed
                    && b.compared == anchor.enumerated
                    && b.mismatches == 0)));
        let regeneration = anchor.regeneration.as_ref().unwrap();
        assert!(regeneration.identical);
        assert_eq!(regeneration.sha256[0], regeneration.sha256[1]);
        let registered = config
            .validation
            .anchors
            .iter()
            .find(|a| a.q == anchor.q)
            .unwrap();
        let mut cell = config
            .cells()
            .into_iter()
            .find(|c| c.q == anchor.q)
            .unwrap();
        cell.n = anchor.n;
        cell.matrix_count = config.validation.sample_matrix_count;
        cell.shard_size = cell.matrix_count;
        let spec = permanent_statistics::campaign::ShardSpec {
            shard_id: 0,
            stream_index: registered.stream_index,
            matrix_count: cell.matrix_count,
        };
        macro_rules! draw {
            ($q:literal,$purpose:expr) => {
                evaluate_shard::<$q, _>(
                    config.validation.root_seed,
                    $purpose,
                    &cell,
                    spec,
                    Backend::BatchParallel,
                    2,
                    500,
                    &mut |_, _, _| {},
                )
                .unwrap()
            };
        }
        let sampled = match anchor.q {
            3 => draw!(3, StreamPurpose::Validation),
            5 => draw!(5, StreamPurpose::Validation),
            7 => draw!(7, StreamPurpose::Validation),
            _ => unreachable!(),
        };
        let sample = anchor.sample.as_ref().unwrap();
        assert_eq!(sample.zero_count, sampled.permanent_zero_count);
        let test = gf2_stats::binomial::two_sided_test(
            sample.zero_count,
            sample.matrix_count,
            anchor.oracle_zero_count as f64 / anchor.enumerated as f64,
        );
        assert_eq!(sample.log_p_value, test.log_p_value());
        assert_eq!(anchor.verdict, !test.rejects_at(config.validation.level));
    }
}
#[test]
fn terminal_anchors_are_never_redrawn_and_failed_anchors_stay_failed() {
    let mut config = common::config();
    config.validation.anchors[0].permanent_zero_count = 0;
    let record = record(&config);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        run_validation(&config, &record, dir.path(), 1, Some(1)).unwrap(),
        ValidationStatus::Incomplete
    );
    let first: Receipt =
        serde_json::from_slice(&fs::read(dir.path().join("receipt.json")).unwrap()).unwrap();
    assert!(!first.anchors[0].verdict);
    assert!(first.anchors[0].sample.is_none());
    assert!(first.anchors[0].regeneration.is_none());
    assert_eq!(
        run_validation(&config, &record, dir.path(), 4, None).unwrap(),
        ValidationStatus::Failed
    );
    let bytes = fs::read(dir.path().join("receipt.json")).unwrap();
    let receipt: Receipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(receipt.anchors[0], first.anchors[0]);
    assert!(!receipt.verdict);
    assert_eq!(
        run_validation(&config, &record, dir.path(), 1, None).unwrap(),
        ValidationStatus::Failed
    );
    assert_eq!(fs::read(dir.path().join("receipt.json")).unwrap(), bytes);
    let events: Vec<serde_json::Value> = fs::read_to_string(dir.path().join("execution.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.get("anchor_terminal").is_some())
            .count(),
        config.validation.anchors.len()
    );
}
#[test]
fn applicable_unavailable_backend_fails_preflight_without_output() {
    if permanent_statistics::campaign::backend_available(Backend::Accelerator) {
        return;
    }
    let mut config = common::config();
    config
        .validation
        .selectable_backends
        .push(Backend::Accelerator);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent");
    assert!(run_validation(&config, &record(&config), &path, 2, None).is_err());
    assert!(!path.exists());
}
#[test]
fn validation_only_binds_closed_identity() {
    let config = common::config();
    let mut record = record(&config);
    let dir = tempfile::tempdir().unwrap();
    run_validation(&config, &record, dir.path(), 1, Some(1)).unwrap();
    record.source_revision = "2222222222222222222222222222222222222222".parse().unwrap();
    assert_eq!(
        run_validation(&config, &record, dir.path(), 1, Some(1)).unwrap(),
        ValidationStatus::Incomplete
    );
    record.identity.toolchain_version.push_str("changed");
    assert!(run_validation(&config, &record, dir.path(), 1, None).is_err());
}

#[test]
fn equality_at_the_exact_sample_threshold_fails_the_anchor() {
    use gf2_core::gfp::Fp;
    use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex};
    let mut config = common::config();
    config.validation.anchors.truncate(1);
    config.validation.sample_matrix_count = 1;
    config.validation.level = 1.0 / 3.0;
    let seed = (0..100)
        .find(|&seed| {
            let address = MatrixAddress::new(
                seed,
                FieldOrder::F3,
                1,
                StreamPurpose::Validation,
                StreamIndex::new(0).unwrap(),
            );
            let mut sampler = MatrixSampler::<3>::new(address).unwrap();
            let mut matrix = [Fp::new(0)];
            sampler.fill_next_matrix(&mut matrix);
            matrix[0].value() == 0
        })
        .unwrap();
    config.validation.root_seed = seed;
    let test = gf2_stats::binomial::two_sided_test(1, 1, 1.0 / 3.0);
    assert_eq!(test.log_p_value(), config.validation.level.ln());
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        run_validation(&config, &record(&config), dir.path(), 1, None).unwrap(),
        ValidationStatus::Failed
    );
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(dir.path().join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt.anchors[0].sample.as_ref().unwrap().zero_count, 1);
    assert!(!receipt.anchors[0].verdict);
}
#[test]
fn interrupted_anchor_restarts_at_its_registered_address() {
    let config = common::config();
    let record = record(&config);
    let dir = tempfile::tempdir().unwrap();
    run_validation(&config, &record, dir.path(), 1, None).unwrap();
    let receipt = fs::read(dir.path().join("receipt.json")).unwrap();
    let path = dir.path().join("execution.jsonl");
    let text = fs::read_to_string(&path).unwrap();
    let start = text.lines().find(|l| l.contains("\"start\"")).unwrap();
    fs::write(&path, format!("{start}\n{{\"anchor_terminal\":")).unwrap();
    assert_eq!(
        run_validation(&config, &record, dir.path(), 1, None).unwrap(),
        ValidationStatus::Passed
    );
    assert_eq!(fs::read(dir.path().join("receipt.json")).unwrap(), receipt);
}

#[test]
fn preflight_rejects_an_invalid_plan_before_probing_or_drawing() {
    let mut config = common::config();
    config.validation.anchors[0].q = 2;
    assert!(
        permanent_statistics::campaign::validation::preflight_required_backends(&config, 1)
            .is_err()
    );
}
