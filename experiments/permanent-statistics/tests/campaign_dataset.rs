mod common;
use permanent_statistics::campaign::{
    assess_completed_cell,
    dataset::{self, Manifest},
    runner::Invocation,
    AcceptancePlan,
};
use std::fs;

#[test]
fn pooled_summaries_use_the_canonical_assessment_and_csv_round_trips() {
    let mut config = common::config();
    config.cell_groups[1].determinant_companion = false;
    let dir = tempfile::tempdir().unwrap();
    common::finish(
        &config,
        &common::record(&config),
        dir.path(),
        &Invocation::default(),
    );
    let manifest = Manifest::load(dir.path()).unwrap();
    let plan = AcceptancePlan::for_manifest(&manifest).unwrap();
    let csv = fs::read_to_string(dir.path().join("summary.csv")).unwrap();
    for (cell, line) in config.cells().iter().zip(csv.lines().skip(1)) {
        let shards = common::shards(dir.path(), cell.q, cell.n);
        let count = shards.iter().map(|r| r.matrix_count).sum();
        let zeros = shards.iter().map(|r| r.permanent_zero_count).sum();
        let determinant = cell.determinant_companion.then(|| {
            shards
                .iter()
                .map(|r| r.determinant_zero_count.unwrap())
                .sum()
        });
        let expected = assess_completed_cell(&plan, cell, count, zeros, determinant).unwrap();
        let parsed = dataset::parse_summary_row(&line.split(',').collect::<Vec<_>>()).unwrap();
        assert_eq!(parsed, expected);
        dataset::validate_summary_aggregate(&manifest, cell, &shards, &parsed).unwrap();
        assert_eq!(
            dataset::encode_summary_csv(&[parsed]),
            format!("{}\n{line}\n", csv.lines().next().unwrap())
        );
        assert_eq!(shards.len() as u64, cell.shard_count());
        for shard in &shards {
            dataset::validate_shard(cell, shard).unwrap();
        }
    }
}
#[test]
fn verifier_refuses_inconsistent_evidence_even_with_refreshed_digests() {
    let config = common::config();
    let dir = tempfile::tempdir().unwrap();
    common::finish(
        &config,
        &common::record(&config),
        dir.path(),
        &Invocation::default(),
    );
    let path = dir.path().join(dataset::shard_path(3, 2));
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"corrupt").unwrap();
    assert!(dataset::verify(dir.path()).is_err());
    fs::write(&path, original).unwrap();
    let mut records = common::shards(dir.path(), 3, 2);
    records[0].permanent_zero_count += 1;
    let bytes = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect::<String>();
    fs::write(&path, bytes).unwrap();
    let sums = fs::read_to_string(dir.path().join("checksums.sha256")).unwrap();
    let changed = sums
        .lines()
        .map(|l| {
            let (digest, p) = l.split_once("  ").unwrap();
            format!(
                "{}  {p}\n",
                if p == dataset::shard_path(3, 2) {
                    experiments_common::Sha256Digest::of(&fs::read(&path).unwrap()).to_string()
                } else {
                    digest.into()
                }
            )
        })
        .collect::<String>();
    fs::write(dir.path().join("checksums.sha256"), changed).unwrap();
    assert!(dataset::verify(dir.path()).is_err());
    dataset::finalize(dir.path()).unwrap();
    fs::write(dir.path().join("unexpected.json"), b"{}").unwrap();
    assert!(dataset::verify(dir.path()).is_err());
    fs::remove_file(dir.path().join("unexpected.json")).unwrap();
    fs::remove_file(dir.path().join("runs.json")).unwrap();
    assert!(dataset::verify(dir.path()).is_err());
}
#[test]
fn manifest_is_predraw_immutable_and_has_no_shard_inventory() {
    let config = common::config();
    let dir = tempfile::tempdir().unwrap();
    let record = common::record(&config);
    permanent_statistics::campaign::runner::run_cell(
        &config,
        &record,
        dir.path(),
        3,
        2,
        &Invocation {
            max_shards: Some(1),
            ..Invocation::default()
        },
    )
    .unwrap();
    let bytes = fs::read(dir.path().join("manifest.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<Manifest>(&bytes).unwrap(),
        Manifest::from_config(&config).unwrap()
    );
    assert!(dataset::finalize(dir.path()).is_err());
    let mut changed = config.clone();
    changed.root_seed += 1;
    assert!(permanent_statistics::campaign::runner::run_cell(
        &changed,
        &common::record(&changed),
        dir.path(),
        3,
        2,
        &Invocation::default()
    )
    .is_err());
    assert_eq!(fs::read(dir.path().join("manifest.json")).unwrap(), bytes);
}
