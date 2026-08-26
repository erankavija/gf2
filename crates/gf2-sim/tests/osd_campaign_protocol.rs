use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gf2_sim::checkpoint::{CheckpointPayload, CheckpointReader};
use gf2_sim::osd_campaign::{
    accepts_published_value, derive_cell_seed, run_osd_campaign, BinomialIntervalMethod,
    BinomialIntervalSpec, ConfidenceIntervalEstimator, DigitizationPrecisionUnit,
    IntervalSamplingUnit, OsdCampaign, OsdCampaignCheckpoint, OsdCampaignProvenance,
    OsdCampaignReceipt, OsdCampaignTermination, OsdCell, OsdCellId, OsdCellRun, OsdCellTermination,
    OsdWorkCounters, OSD_CAMPAIGN_SCHEMA_VERSION,
};
use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, GitRevision, Provenance, RngAlgorithm, Sha256Digest,
};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gf2-osd-campaign-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create temporary campaign directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn artifact(path: &str, digit: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.parse().expect("valid artifact path"),
        sha256: std::iter::repeat_n(digit, 64)
            .collect::<String>()
            .parse::<Sha256Digest>()
            .expect("valid SHA-256 digest"),
    }
}

fn provenance() -> OsdCampaignProvenance {
    OsdCampaignProvenance {
        runtime: Provenance {
            git_revision: "0123456789abcdef0123456789abcdef01234567"
                .parse::<GitRevision>()
                .expect("valid git revision"),
            binary_sha256: Some(
                std::iter::repeat_n('c', 64)
                    .collect::<String>()
                    .parse::<Sha256Digest>()
                    .expect("valid SHA-256 digest"),
            ),
            deps_source_revision: Some(
                "0123456789abcdef0123456789abcdef01234567"
                    .parse::<GitRevision>()
                    .expect("valid git revision"),
            ),
            deps_source_dirty: Some(false),
            compiler_version: "rustc 1.95.0".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "0.9.0".to_owned(),
            invocation: vec!["osd-campaign".to_owned(), "run".to_owned()],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "test cpu".to_owned(),
            cpu_physical_cores: Some(4),
            cpu_logical_threads: Some(8),
            gpu_model: Availability::NotPresent,
        },
        configuration: artifact("dev/campaigns/osd-reference.json", 'a'),
        measurement_behavior: artifact("dev/receipts/osd-behavior.json", 'b'),
    }
}

fn cell(id: &str, eb_n0_db: f64, order: u8) -> OsdCell {
    OsdCell::new(
        id.parse::<OsdCellId>().expect("valid cell id"),
        eb_n0_db,
        order,
        0.002,
    )
    .expect("valid cell")
}

fn campaign(cells: Vec<OsdCell>) -> OsdCampaign {
    OsdCampaign::new(
        0x5eed_cafe_1234_5678,
        cells,
        BinomialIntervalSpec::new(BinomialIntervalMethod::ClopperPearson, 0.95)
            .expect("valid interval"),
        2,
        provenance(),
    )
    .expect("valid campaign")
}

fn work(eliminations: u64, patterns: u64, candidates: u64) -> OsdWorkCounters {
    OsdWorkCounters {
        eliminations,
        generated_patterns: patterns,
        tested_candidates: candidates,
    }
}

fn completed_cell_ids(receipt: &OsdCampaignReceipt) -> Vec<OsdCellId> {
    receipt
        .cell_results
        .iter()
        .filter(|result| !matches!(result.termination, OsdCellTermination::Interrupted))
        .map(|result| result.cell.id.clone())
        .collect()
}

#[test]
fn deterministic_cell_seeds_depend_on_root_and_stable_identity() {
    let alpha = "order-2-point-0".parse::<OsdCellId>().unwrap();
    let beta = "order-2-point-1".parse::<OsdCellId>().unwrap();

    assert_eq!(derive_cell_seed(17, &alpha), derive_cell_seed(17, &alpha));
    assert_ne!(derive_cell_seed(17, &alpha), derive_cell_seed(18, &alpha));
    assert_ne!(derive_cell_seed(17, &alpha), derive_cell_seed(17, &beta));

    let forward = campaign(vec![
        cell("order-2-point-0", 2.0, 2),
        cell("order-2-point-1", 2.5, 2),
    ]);
    let reverse = campaign(vec![
        cell("order-2-point-1", 2.5, 2),
        cell("order-2-point-0", 2.0, 2),
    ]);
    let forward_seeds: BTreeMap<_, _> = forward
        .cells()
        .iter()
        .map(|cell| (cell.id.clone(), forward.cell_seed(&cell.id)))
        .collect();
    let reverse_seeds: BTreeMap<_, _> = reverse
        .cells()
        .iter()
        .map(|cell| (cell.id.clone(), reverse.cell_seed(&cell.id)))
        .collect();
    assert_eq!(forward_seeds, reverse_seeds);
}

#[test]
fn receipt_and_checkpoint_schemas_round_trip_with_named_block_intervals() {
    let dir = TempDir::new("round-trip");
    let checkpoint_path = dir.path().join("checkpoint.json");
    let campaign = campaign(vec![cell("order-2-point-0", 2.0, 2)]);

    let receipt = run_osd_campaign(&checkpoint_path, &campaign, |context| {
        assert_eq!(context.resume.samples, 0);
        assert_eq!(context.target_block_errors, 2);
        OsdCellRun {
            samples: 100,
            sampled_bits: 6_400,
            bit_errors: 12,
            block_errors: 7,
            work: work(100, 450, 431),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect("campaign completes");

    assert_eq!(receipt.schema_version, OSD_CAMPAIGN_SCHEMA_VERSION);
    assert_eq!(
        <OsdCampaignCheckpoint as CheckpointPayload>::SCHEMA_VERSION,
        OSD_CAMPAIGN_SCHEMA_VERSION
    );
    assert_eq!(receipt.cell_results[0].ber, 12.0 / 6_400.0);
    assert_eq!(receipt.cell_results[0].bler, 0.07);
    assert_eq!(
        receipt.cell_results[0].ber_confidence_interval.estimator,
        ConfidenceIntervalEstimator::HoeffdingBlockMeanSequence
    );
    assert_eq!(
        receipt.cell_results[0]
            .ber_confidence_interval
            .sampling_unit,
        Some(IntervalSamplingUnit::Block)
    );
    assert_eq!(
        receipt.cell_results[0].bler_confidence_interval.estimator,
        ConfidenceIntervalEstimator::ClopperPearson
    );
    assert_eq!(
        receipt.cell_results[0]
            .bler_confidence_interval
            .sampling_unit,
        Some(IntervalSamplingUnit::Block)
    );
    assert_eq!(receipt.cell_results[0].ber_confidence_interval.level, 0.95);
    assert_eq!(receipt.cell_results[0].bler_confidence_interval.level, 0.95);
    assert!(receipt.cell_results[0].ber_confidence_interval.lower <= receipt.cell_results[0].ber);
    assert!(receipt.cell_results[0].ber_confidence_interval.upper >= receipt.cell_results[0].ber);
    assert_eq!(
        (
            receipt.cell_results[0].bler_confidence_interval.lower,
            receipt.cell_results[0].bler_confidence_interval.upper,
        ),
        gf2_stats::intervals::clopper_pearson_interval(7, 100, 0.95)
    );
    assert!(receipt.cell_results[0].accepts_published_value(12.0 / 6_400.0));
    assert!(!accepts_published_value(
        12.0 / 6_400.0,
        &receipt.cell_results[0].bler_confidence_interval,
        receipt.cell_results[0].cell.digitization_precision,
    ));
    assert_eq!(
        receipt.digitization_precision_unit,
        Some(DigitizationPrecisionUnit::Log10Decades)
    );
    assert_eq!(receipt.target_block_errors, Some(2));
    assert_eq!(receipt.invocation_history.len(), 1);
    assert_eq!(receipt.invocation_history[0].first_attempt_index, 0);
    assert_eq!(receipt.invocation_history[0].attempt_count, 1);
    assert_eq!(receipt.cell_results[0].invocation_index, Some(0));
    let receipt_json = serde_json::to_value(&receipt).unwrap();
    assert!(receipt_json.get("completed_cells").is_none());
    assert_eq!(receipt_json["digitization_precision_unit"], "log10_decades");
    assert_eq!(
        receipt_json["cell_results"][0]["ber_confidence_interval"]["sampling_unit"],
        "block"
    );
    assert_eq!(
        receipt_json["cell_results"][0]["ber_confidence_interval"]["estimator"],
        "hoeffding_block_mean_sequence"
    );
    assert_eq!(
        receipt_json["cell_results"][0]["bler_confidence_interval"]["estimator"],
        "clopper_pearson"
    );
    assert_eq!(
        serde_json::from_value::<gf2_sim::osd_campaign::OsdCampaignReceipt>(receipt_json).unwrap(),
        receipt
    );

    let checkpoint = CheckpointReader::<OsdCampaignCheckpoint, _>::for_payload(
        &checkpoint_path,
        campaign.config_hash().unwrap(),
    )
    .load_payload()
    .expect("checkpoint envelope is valid")
    .expect("checkpoint exists");
    let checkpoint_json = serde_json::to_value(&checkpoint).unwrap();
    assert!(checkpoint_json.get("completed_cells").is_none());
    assert_eq!(
        serde_json::from_value::<OsdCampaignCheckpoint>(checkpoint_json).unwrap(),
        checkpoint
    );
    assert_eq!(checkpoint.cell_results, receipt.cell_results);
}

#[test]
fn bursty_fixture_clustered_ber_interval_is_wider_than_bit_independence_interval() {
    let dir = TempDir::new("bursty-interval");
    let campaign = campaign(vec![cell("bursty", 2.0, 2)]);
    let receipt = run_osd_campaign(dir.path().join("checkpoint.json"), &campaign, |_| {
        OsdCellRun {
            samples: 100,
            sampled_bits: 6_400,
            bit_errors: 64,
            block_errors: 2,
            work: work(100, 100, 100),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect("bursty fixture completes");

    let clustered = receipt.cell_results[0].ber_confidence_interval;
    let bit_independence = gf2_stats::intervals::clopper_pearson_interval(64, 6_400, 0.95);
    let clustered_width = clustered.upper - clustered.lower;
    let bit_independence_width = bit_independence.1 - bit_independence.0;

    assert_eq!(clustered.sampling_unit, Some(IntervalSamplingUnit::Block));
    assert_eq!(
        clustered.estimator,
        ConfidenceIntervalEstimator::HoeffdingBlockMeanSequence
    );
    assert!(clustered_width > bit_independence_width);
}

#[test]
fn schema_1_committed_receipt_remains_readable_with_historical_semantics() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.json");
    let receipt: OsdCampaignReceipt =
        serde_json::from_slice(&std::fs::read(path).expect("read committed schema-1 receipt"))
            .expect("schema-1 receipt remains readable");

    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.digitization_precision_unit, None);
    assert_eq!(receipt.target_block_errors, None);
    assert!(receipt.invocation_history.is_empty());
    assert_eq!(receipt.provenance.runtime.cpu_physical_cores, None);
    assert_eq!(receipt.provenance.runtime.cpu_logical_threads, None);
    assert_eq!(receipt.provenance.runtime.cpu_model, "linux-x86_64");
    assert_eq!(
        receipt.provenance.runtime.invocation,
        ["ebch_osd_awgn_campaign"]
    );
    assert!(receipt.cell_results.iter().all(|result| {
        result.invocation_index.is_none()
            && result.ber_confidence_interval.sampling_unit.is_none()
            && result.bler_confidence_interval.sampling_unit.is_none()
            && result.ber_confidence_interval.estimator
                == ConfidenceIntervalEstimator::ClopperPearson
            && result.bler_confidence_interval.estimator
                == ConfidenceIntervalEstimator::ClopperPearson
    }));
}

#[test]
fn resume_skips_completed_cells_and_continues_an_interrupted_cell() {
    let dir = TempDir::new("resume");
    let checkpoint_path = dir.path().join("checkpoint.json");
    let first_id = "order-2-point-0".parse::<OsdCellId>().unwrap();
    let second_id = "order-2-point-1".parse::<OsdCellId>().unwrap();
    let campaign = campaign(vec![
        cell(first_id.as_str(), 2.0, 2),
        cell(second_id.as_str(), 2.5, 2),
    ]);

    let first_receipt = run_osd_campaign(&checkpoint_path, &campaign, |context| {
        if context.cell.id == first_id {
            OsdCellRun {
                samples: 10,
                sampled_bits: 640,
                bit_errors: 3,
                block_errors: 2,
                work: work(10, 40, 38),
                termination: OsdCellTermination::Completed,
            }
        } else {
            assert_eq!(context.cell.id, second_id);
            OsdCellRun {
                samples: 4,
                sampled_bits: 256,
                bit_errors: 2,
                block_errors: 1,
                work: work(4, 17, 15),
                termination: OsdCellTermination::Interrupted,
            }
        }
    })
    .expect("interrupted progress is checkpointed");
    assert_eq!(
        first_receipt.termination,
        OsdCampaignTermination::Interrupted {
            cell_id: second_id.clone()
        }
    );
    assert_eq!(completed_cell_ids(&first_receipt), vec![first_id.clone()]);

    let mut resumed_calls = Vec::new();
    let resumed = run_osd_campaign(&checkpoint_path, &campaign, |context| {
        resumed_calls.push(context.cell.id.clone());
        assert_eq!(context.cell.id, second_id);
        assert_eq!(context.resume.samples, 4);
        assert_eq!(context.resume.sampled_bits, 256);
        assert_eq!(context.resume.bit_errors, 2);
        assert_eq!(context.resume.block_errors, 1);
        assert_eq!(context.resume.work, work(4, 17, 15));
        assert_eq!(context.seed, campaign.cell_seed(&second_id));
        OsdCellRun {
            samples: 10,
            sampled_bits: 640,
            bit_errors: 4,
            block_errors: 2,
            work: work(10, 45, 42),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect("resume completes remaining cell");

    assert_eq!(resumed_calls, vec![second_id.clone()]);
    assert_eq!(resumed.termination, OsdCampaignTermination::Completed);
    assert_eq!(completed_cell_ids(&resumed), vec![first_id, second_id]);
    assert_eq!(resumed.cell_results.len(), 3);
    assert!(matches!(
        &resumed.cell_results[1].termination,
        OsdCellTermination::Interrupted
    ));

    let recovered = run_osd_campaign(&checkpoint_path, &campaign, |_| {
        panic!("a completed campaign must not execute another cell")
    })
    .expect("completed checkpoint is recovered");
    assert_eq!(recovered, resumed);
}

#[test]
fn invocation_history_accumulates_full_arguments_across_resume() {
    let dir = TempDir::new("invocation-history");
    let checkpoint_path = dir.path().join("checkpoint.json");
    let cell = cell("resume", 2.0, 2);
    let mut first_provenance = provenance();
    first_provenance.runtime.invocation = vec![
        "osd-campaign".to_owned(),
        "--max-samples".to_owned(),
        "4".to_owned(),
    ];
    let first_campaign = OsdCampaign::new(
        17,
        vec![cell.clone()],
        BinomialIntervalSpec::new(BinomialIntervalMethod::ClopperPearson, 0.95).unwrap(),
        2,
        first_provenance,
    )
    .unwrap();
    run_osd_campaign(&checkpoint_path, &first_campaign, |_| OsdCellRun {
        samples: 4,
        sampled_bits: 256,
        bit_errors: 8,
        block_errors: 1,
        work: work(4, 4, 4),
        termination: OsdCellTermination::Interrupted,
    })
    .expect("first invocation persists");

    let mut second_provenance = provenance();
    second_provenance.runtime.invocation = vec![
        "osd-campaign".to_owned(),
        "--max-samples".to_owned(),
        "8".to_owned(),
    ];
    let second_campaign = OsdCampaign::new(
        17,
        vec![cell],
        BinomialIntervalSpec::new(BinomialIntervalMethod::ClopperPearson, 0.95).unwrap(),
        2,
        second_provenance,
    )
    .unwrap();
    let receipt = run_osd_campaign(&checkpoint_path, &second_campaign, |context| {
        assert_eq!(context.resume.samples, 4);
        OsdCellRun {
            samples: 12,
            sampled_bits: 768,
            bit_errors: 20,
            block_errors: 2,
            work: work(12, 12, 12),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect("second invocation resumes");

    assert_eq!(receipt.invocation_history.len(), 2);
    assert_eq!(receipt.invocation_history[0].first_attempt_index, 0);
    assert_eq!(receipt.invocation_history[0].attempt_count, 1);
    assert_eq!(receipt.invocation_history[1].first_attempt_index, 1);
    assert_eq!(receipt.invocation_history[1].attempt_count, 1);
    assert_eq!(
        receipt.invocation_history[0].provenance.invocation,
        ["osd-campaign", "--max-samples", "4"]
    );
    assert_eq!(
        receipt.invocation_history[1].provenance.invocation,
        ["osd-campaign", "--max-samples", "8"]
    );
    assert_eq!(receipt.cell_results[0].invocation_index, Some(0));
    assert_eq!(receipt.cell_results[1].invocation_index, Some(1));
    assert_eq!(
        receipt.provenance.runtime.invocation,
        ["osd-campaign", "--max-samples", "8"]
    );
}

#[test]
fn resume_preserves_censored_exhausted_and_contradictory_results() {
    let dir = TempDir::new("terminal-results");
    let checkpoint_path = dir.path().join("checkpoint.json");
    let campaign = campaign(vec![
        cell("censored", 1.0, 1),
        cell("exhausted", 1.5, 2),
        cell("contradictory", 2.0, 2),
    ]);

    let receipt = run_osd_campaign(&checkpoint_path, &campaign, |context| {
        let termination = match context.cell.id.as_str() {
            "censored" => OsdCellTermination::Censored {
                reason: "sample budget reached".to_owned(),
            },
            "exhausted" => OsdCellTermination::Exhausted {
                reason: "candidate budget reached".to_owned(),
            },
            "contradictory" => OsdCellTermination::Contradictory {
                published_value: 0.9,
            },
            other => panic!("unexpected cell {other}"),
        };
        OsdCellRun {
            samples: 20,
            sampled_bits: 1_280,
            bit_errors: 5,
            block_errors: 3,
            work: work(20, 80, 75),
            termination,
        }
    })
    .expect("terminal evidence is checkpointed");

    assert_eq!(completed_cell_ids(&receipt).len(), 3);
    let recovered = run_osd_campaign(&checkpoint_path, &campaign, |_| {
        panic!("preserved terminal cells must not be repeated")
    })
    .expect("terminal evidence recovers");
    assert_eq!(recovered, receipt);
}

#[test]
fn bit_and_block_counts_are_validated_against_their_own_samples() {
    let campaign = campaign(vec![cell("accounting", 2.0, 2)]);

    let bit_dir = TempDir::new("invalid-bit-accounting");
    let bit_error = run_osd_campaign(bit_dir.path().join("checkpoint.json"), &campaign, |_| {
        OsdCellRun {
            samples: 1,
            sampled_bits: 8,
            bit_errors: 9,
            block_errors: 1,
            work: work(1, 1, 1),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect_err("bit errors cannot exceed sampled bits");
    assert!(bit_error
        .to_string()
        .contains("bit errors cannot exceed sampled bits"));

    let block_dir = TempDir::new("invalid-block-accounting");
    let block_error = run_osd_campaign(block_dir.path().join("checkpoint.json"), &campaign, |_| {
        OsdCellRun {
            samples: 1,
            sampled_bits: 8,
            bit_errors: 2,
            block_errors: 2,
            work: work(1, 1, 1),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect_err("block errors cannot exceed sampled blocks");
    assert!(block_error
        .to_string()
        .contains("block errors cannot exceed samples"));
}

#[test]
fn cell_termination_is_bound_to_the_independent_block_error_target() {
    let campaign = campaign(vec![cell("stopping", 2.0, 2)]);
    let early_dir = TempDir::new("early-completion");
    let early = run_osd_campaign(early_dir.path().join("checkpoint.json"), &campaign, |_| {
        OsdCellRun {
            samples: 10,
            sampled_bits: 640,
            bit_errors: 60,
            block_errors: 1,
            work: work(10, 10, 10),
            termination: OsdCellTermination::Completed,
        }
    })
    .expect_err("one block error cannot satisfy the target of two");
    assert!(early
        .to_string()
        .contains("has not reached the target block-error count"));

    let late_dir = TempDir::new("late-interruption");
    let late = run_osd_campaign(late_dir.path().join("checkpoint.json"), &campaign, |_| {
        OsdCellRun {
            samples: 10,
            sampled_bits: 640,
            bit_errors: 60,
            block_errors: 2,
            work: work(10, 10, 10),
            termination: OsdCellTermination::Interrupted,
        }
    })
    .expect_err("reaching two block errors must complete the cell");
    assert!(late
        .to_string()
        .contains("has already reached the target block-error count"));
}

#[test]
fn nonzero_digitization_precision_uses_log10_decades() {
    let interval = gf2_sim::osd_campaign::BinomialConfidenceInterval {
        estimator: ConfidenceIntervalEstimator::ClopperPearson,
        sampling_unit: Some(IntervalSamplingUnit::Block),
        level: 0.95,
        lower: 0.20,
        upper: 0.30,
    };
    let delta = 0.1;
    let widened_lower = interval.lower * 10.0_f64.powf(-delta);
    let widened_upper = interval.upper * 10.0_f64.powf(delta);

    assert!(accepts_published_value(widened_lower, &interval, delta));
    assert!(accepts_published_value(widened_upper, &interval, delta));
    assert!(!accepts_published_value(
        f64::from_bits(widened_lower.to_bits() - 1),
        &interval,
        delta
    ));
    assert!(!accepts_published_value(
        f64::from_bits(widened_upper.to_bits() + 1),
        &interval,
        delta
    ));
}
