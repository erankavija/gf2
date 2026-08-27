use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gf2_sim::checkpoint::{CheckpointPayload, CheckpointReader, CheckpointWriter};
use gf2_sim::osd_campaign::{
    accepts_published_value, derive_cell_seed, run_osd_campaign, BinomialConfidenceInterval,
    BinomialIntervalMethod, BinomialIntervalSpec, BlockSampleCounts, ConfidenceIntervalEstimator,
    DigitizationPrecisionUnit, IntervalSamplingUnit, OsdBlockContext, OsdBlockOutcome,
    OsdBlockStream, OsdCampaign, OsdCampaignCheckpoint, OsdCampaignProvenance, OsdCampaignReceipt,
    OsdCampaignTermination, OsdCell, OsdCellId, OsdCellTermination, OsdWorkCounters,
    OSD_CAMPAIGN_SCHEMA_VERSION,
};
use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, GitRevision, Provenance, RngAlgorithm, Sha256Digest,
};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

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

fn interval_spec() -> BinomialIntervalSpec {
    BinomialIntervalSpec::new(BinomialIntervalMethod::NegativeBinomialClopperPearson, 0.95)
        .expect("valid interval")
}

fn campaign_with_target(cells: Vec<OsdCell>, target_block_errors: u64) -> OsdCampaign {
    OsdCampaign::new(
        0x5eed_cafe_1234_5678,
        cells,
        interval_spec(),
        target_block_errors,
        provenance(),
    )
    .expect("valid campaign")
}

fn campaign(cells: Vec<OsdCell>) -> OsdCampaign {
    campaign_with_target(cells, 2)
}

fn work(eliminations: u64, patterns: u64, candidates: u64) -> OsdWorkCounters {
    OsdWorkCounters {
        eliminations,
        generated_patterns: patterns,
        tested_candidates: candidates,
    }
}

/// The single-worker dispatch the worker-count invariance contract is stated
/// against.
fn serial() -> NonZeroUsize {
    NonZeroUsize::new(1).expect("one worker")
}

fn workers(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("positive worker count")
}

/// Scripted per-block error counts with two failing blocks, at indices 2 and 6.
fn scripted_errors(block_index: u64) -> u64 {
    match block_index {
        2 => 3,
        6 => 2,
        _ => 0,
    }
}

/// A block-index-pure pseudo-random process: about one block in sixteen fails,
/// carrying between 1 and 24 of its 64 information bits in error.
///
/// The mixing is SplitMix64 over the global block index alone, so a block's
/// outcome does not depend on which worker evaluates it — the property the
/// protocol's evaluator contract demands.
fn mixed_block_errors(block_index: u64) -> u64 {
    let mut z = block_index.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let mixed = z ^ (z >> 31);
    if mixed.is_multiple_of(16) {
        1 + (mixed >> 8) % 24
    } else {
        0
    }
}

/// The block index carrying the `target`-th error of [`mixed_block_errors`],
/// derived independently of the protocol.
fn mixed_stopping_index(target: u64) -> u64 {
    let mut failures = 0;
    for block_index in 0.. {
        if mixed_block_errors(block_index) != 0 {
            failures += 1;
            if failures == target {
                return block_index;
            }
        }
    }
    unreachable!("the scripted process fails infinitely often")
}

/// Runs the [`mixed_block_errors`] process to completion at `workers`.
fn mixed_run(label: &str, campaign: &OsdCampaign, workers: NonZeroUsize) -> OsdCampaignReceipt {
    let dir = TempDir::new(label);
    run_osd_campaign(
        dir.path().join("checkpoint.json"),
        campaign,
        100_000,
        workers,
        || {
            |context: OsdBlockContext<'_>| OsdBlockOutcome {
                information_bits: 64,
                information_bit_errors: mixed_block_errors(context.block_index),
                work: work(context.block_index + 1, 2, 3),
            }
        },
    )
    .expect("the scripted process reaches the block-error target")
}

fn scripted_outcomes(
    samples: u64,
    information_bits: u64,
    failing_bit_errors: &[u64],
    total_work: OsdWorkCounters,
) -> Vec<OsdBlockOutcome> {
    assert!(samples >= failing_bit_errors.len() as u64);
    assert!(failing_bit_errors.iter().all(|&errors| errors > 0));
    let mut outcomes = vec![
        OsdBlockOutcome {
            information_bits,
            information_bit_errors: 0,
            work: work(0, 0, 0),
        };
        samples as usize
    ];
    outcomes[0].work = total_work;
    let first_failure = outcomes.len() - failing_bit_errors.len();
    for (outcome, &errors) in outcomes[first_failure..].iter_mut().zip(failing_bit_errors) {
        outcome.information_bit_errors = errors;
    }
    outcomes
}

fn read_checkpoint(path: &Path, campaign: &OsdCampaign) -> OsdCampaignCheckpoint {
    CheckpointReader::<OsdCampaignCheckpoint, _>::for_payload(path, campaign.config_hash().unwrap())
        .load_payload()
        .expect("checkpoint envelope is valid")
        .expect("checkpoint exists")
}

fn write_checkpoint(path: &Path, campaign: &OsdCampaign, checkpoint: &OsdCampaignCheckpoint) {
    CheckpointWriter::<OsdCampaignCheckpoint, _>::for_payload(
        path,
        campaign.config_hash().unwrap(),
    )
    .expect("checkpoint writer")
    .write_payload(checkpoint)
    .expect("write checkpoint");
}

fn tampered_completed_receipt_error(
    label: &str,
    campaign: &OsdCampaign,
    mutate: impl FnOnce(&mut gf2_sim::osd_campaign::OsdCellReceipt),
) -> String {
    let dir = TempDir::new(label);
    let checkpoint_path = dir.path().join("checkpoint.json");
    let outcomes = scripted_outcomes(10, 64, &[6, 6], work(10, 10, 10));
    let outcomes = &outcomes;
    run_osd_campaign(&checkpoint_path, campaign, 10, serial(), || {
        move |context: OsdBlockContext<'_>| outcomes[context.block_index as usize]
    })
    .expect("create a valid completed receipt");
    let mut checkpoint = read_checkpoint(&checkpoint_path, campaign);
    mutate(&mut checkpoint.cell_results[0]);
    write_checkpoint(&checkpoint_path, campaign, &checkpoint);
    run_osd_campaign(&checkpoint_path, campaign, 1, serial(), || {
        |_: OsdBlockContext<'_>| -> OsdBlockOutcome {
            panic!("an invalid completed receipt must be refused before evaluation")
        }
    })
    .expect_err("tampered completed receipt is invalid")
    .to_string()
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
fn protocol_stops_on_the_block_carrying_the_exact_target_error() {
    let dir = TempDir::new("exact-kth-error");
    let campaign = campaign(vec![cell("scripted", 2.0, 2)]);
    let evaluated = Mutex::new(BTreeSet::new());
    let evaluated_ref = &evaluated;

    let receipt = run_osd_campaign(
        dir.path().join("checkpoint.json"),
        &campaign,
        100,
        serial(),
        || {
            move |context: OsdBlockContext<'_>| {
                evaluated_ref
                    .lock()
                    .expect("scripted evaluator lock")
                    .insert(context.block_index);
                OsdBlockOutcome {
                    information_bits: 64,
                    information_bit_errors: scripted_errors(context.block_index),
                    work: work(1, 2, 3),
                }
            }
        },
    )
    .expect("the second scripted block error completes the cell");

    let evaluated = evaluated.into_inner().expect("scripted evaluator lock");
    assert!((0..7).all(|index| evaluated.contains(&index)));
    assert_eq!(receipt.cell_results[0].samples, 7);
    assert_eq!(receipt.cell_results[0].block_errors, 2);
    assert_eq!(receipt.cell_results[0].bit_errors, 5);
    assert_eq!(receipt.cell_results[0].squared_block_bit_errors, Some(13));
    assert!(matches!(
        receipt.cell_results[0].termination,
        OsdCellTermination::Completed
    ));
}

/// REQ-01: for a fixed campaign and seed the counters, the stopping index, and
/// the whole receipt payload are byte-identical across worker counts, with the
/// single-worker run as the reference.
#[test]
fn multi_worker_runs_reproduce_the_single_worker_reference() {
    let campaign = campaign_with_target(vec![cell("invariance", 2.0, 2)], 25);
    let reference = mixed_run("invariance-1", &campaign, serial());
    let reference_bytes = serde_json::to_vec(&reference).expect("receipt serializes");

    for count in [2, 8, 24] {
        let observed = mixed_run(&format!("invariance-{count}"), &campaign, workers(count));
        assert_eq!(
            serde_json::to_vec(&observed).expect("receipt serializes"),
            reference_bytes,
            "{count} workers must reproduce the single-worker receipt byte for byte"
        );
    }

    // The cell must be long enough that 24 workers span several dispatched
    // waves, otherwise the invariance above is vacuous.
    assert!(reference.cell_results[0].samples > 24 * 16);
}

/// REQ-02: completion falls exactly on the block carrying the `K`-th block
/// error, and the blocks workers evaluate past it contribute to no counter.
#[test]
fn speculative_blocks_past_the_kth_error_contribute_to_no_counter() {
    let dir = TempDir::new("speculative-truncation");
    let campaign = campaign(vec![cell("truncation", 2.0, 2)]);
    let evaluated = Mutex::new(BTreeSet::new());
    let evaluated_ref = &evaluated;

    let receipt = run_osd_campaign(
        dir.path().join("checkpoint.json"),
        &campaign,
        1_000,
        workers(8),
        || {
            move |context: OsdBlockContext<'_>| {
                evaluated_ref
                    .lock()
                    .expect("scripted evaluator lock")
                    .insert(context.block_index);
                OsdBlockOutcome {
                    information_bits: 64,
                    information_bit_errors: scripted_errors(context.block_index),
                    work: work(1, 2, 3),
                }
            }
        },
    )
    .expect("the second scripted block error completes the cell");

    let evaluated = evaluated.into_inner().expect("scripted evaluator lock");
    let result = &receipt.cell_results[0];
    assert_eq!(result.samples, 7, "block 6 carries the second block error");
    assert_eq!(result.block_errors, 2);
    assert_eq!(result.bit_errors, 5);
    assert_eq!(result.squared_block_bit_errors, Some(13));
    assert_eq!(
        result.work,
        work(7, 14, 21),
        "exactly one work delta per committed block"
    );
    assert!(
        evaluated.iter().copied().max().expect("evaluated blocks") > 6,
        "eight workers must have evaluated past the stopping block"
    );
    assert!((0..7).all(|index| evaluated.contains(&index)));
}

/// REQ-02 at scale: the protocol's stopping index equals the independently
/// derived index of the target's `K`-th error.
#[test]
fn the_stopping_index_is_the_block_carrying_the_kth_error() {
    let campaign = campaign_with_target(vec![cell("invariance", 2.0, 2)], 25);
    let receipt = mixed_run("stopping-index", &campaign, workers(8));

    assert_eq!(
        receipt.cell_results[0].samples,
        mixed_stopping_index(25) + 1
    );
    assert_eq!(receipt.cell_results[0].block_errors, 25);
}

/// REQ-03: an interrupted multi-worker cell that resumes across several
/// bounded invocations, at differing worker counts, reaches the uninterrupted
/// cumulative evidence.
#[test]
fn interrupted_multi_worker_cell_resumes_to_the_uninterrupted_result() {
    let campaign = campaign_with_target(vec![cell("invariance", 2.0, 2)], 25);
    let uninterrupted = mixed_run("resume-uninterrupted", &campaign, workers(8));

    let dir = TempDir::new("resume-interrupted");
    let checkpoint_path = dir.path().join("checkpoint.json");
    let bounded = |count: usize| {
        run_osd_campaign(&checkpoint_path, &campaign, 40, workers(count), || {
            |context: OsdBlockContext<'_>| OsdBlockOutcome {
                information_bits: 64,
                information_bit_errors: mixed_block_errors(context.block_index),
                work: work(context.block_index + 1, 2, 3),
            }
        })
        .expect("bounded invocation checkpoints its progress")
    };
    for count in [24, 3, 1] {
        let interrupted = bounded(count);
        assert!(matches!(
            interrupted
                .cell_results
                .last()
                .expect("an attempt")
                .termination,
            OsdCellTermination::Interrupted
        ));
    }

    let resumed = mixed_resume(&checkpoint_path, &campaign, workers(2));
    let resumed_cell = resumed.cell_results.last().expect("the completed attempt");
    let uninterrupted_cell = &uninterrupted.cell_results[0];
    let mut comparable = resumed_cell.clone();
    comparable.invocation_index = uninterrupted_cell.invocation_index;
    assert_eq!(
        serde_json::to_vec(&comparable).expect("receipt serializes"),
        serde_json::to_vec(uninterrupted_cell).expect("receipt serializes"),
        "resuming must preserve the byte identity of cumulative cell evidence"
    );
}

/// Resumes the [`mixed_block_errors`] process on an existing checkpoint.
fn mixed_resume(
    checkpoint_path: &Path,
    campaign: &OsdCampaign,
    workers: NonZeroUsize,
) -> OsdCampaignReceipt {
    run_osd_campaign(checkpoint_path, campaign, 100_000, workers, || {
        |context: OsdBlockContext<'_>| OsdBlockOutcome {
            information_bits: 64,
            information_bit_errors: mixed_block_errors(context.block_index),
            work: work(context.block_index + 1, 2, 3),
        }
    })
    .expect("resume reaches the block-error target")
}

/// The per-block stream is positioned by the shared
/// [`gf2_sim::parallel::worker_offset`] seek, so a block's random draw depends
/// on its global index alone.
#[test]
fn block_streams_seek_to_the_shared_worker_offset() {
    let mut stream = OsdBlockStream::new(0x5eed_cafe);
    for block_index in [0_u64, 1, 63, 64, 65, 1 << 20] {
        stream.seek_to_block(block_index);
        assert_eq!(
            stream.current_word_pos(),
            gf2_sim::parallel::worker_offset(
                0x5eed_cafe,
                0,
                0,
                usize::try_from(block_index).expect("host word size")
            )
        );
    }
}

#[test]
fn bounded_interruption_resumes_to_uninterrupted_totals() {
    let resumed_dir = TempDir::new("bounded-resume");
    let uninterrupted_dir = TempDir::new("uninterrupted");
    let campaign = campaign(vec![cell("scripted", 2.0, 2)]);
    let outcome = |block_index: u64| OsdBlockOutcome {
        information_bits: 64,
        information_bit_errors: match block_index {
            2 => 3,
            6 => 2,
            _ => 0,
        },
        work: work(block_index + 1, 2 * block_index + 1, 3),
    };

    let resumed_indices = Mutex::new(BTreeSet::new());
    let resumed_indices_ref = &resumed_indices;
    let recording = || {
        move |context: OsdBlockContext<'_>| {
            resumed_indices_ref
                .lock()
                .expect("scripted evaluator lock")
                .insert(context.block_index);
            outcome(context.block_index)
        }
    };
    let interrupted = run_osd_campaign(
        resumed_dir.path().join("checkpoint.json"),
        &campaign,
        4,
        serial(),
        recording,
    )
    .expect("the invocation bound interrupts before the second error");
    assert!(matches!(
        interrupted.cell_results[0].termination,
        OsdCellTermination::Interrupted
    ));
    assert_eq!(interrupted.cell_results[0].samples, 4);

    let resumed = run_osd_campaign(
        resumed_dir.path().join("checkpoint.json"),
        &campaign,
        100,
        serial(),
        recording,
    )
    .expect("resume reaches the second error");
    let uninterrupted = run_osd_campaign(
        uninterrupted_dir.path().join("checkpoint.json"),
        &campaign,
        100,
        serial(),
        || |context: OsdBlockContext<'_>| outcome(context.block_index),
    )
    .expect("uninterrupted run reaches the same second error");

    let resumed_indices = resumed_indices
        .into_inner()
        .expect("scripted evaluator lock");
    assert!(
        (0..7).all(|index| resumed_indices.contains(&index)),
        "every committed block is evaluated once across the two invocations"
    );
    let resumed_result = resumed.cell_results.last().unwrap();
    let uninterrupted_result = uninterrupted.cell_results.last().unwrap();
    assert_eq!(resumed_result.samples, uninterrupted_result.samples);
    assert_eq!(
        resumed_result.sampled_bits,
        uninterrupted_result.sampled_bits
    );
    assert_eq!(resumed_result.bit_errors, uninterrupted_result.bit_errors);
    assert_eq!(
        resumed_result.block_errors,
        uninterrupted_result.block_errors
    );
    assert_eq!(
        resumed_result.squared_block_bit_errors,
        uninterrupted_result.squared_block_bit_errors
    );
    assert_eq!(resumed_result.work, uninterrupted_result.work);
    assert_eq!(
        resumed_result.ber_confidence_interval,
        uninterrupted_result.ber_confidence_interval
    );
    assert_eq!(
        resumed_result.bler_confidence_interval,
        uninterrupted_result.bler_confidence_interval
    );
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
    let campaign = campaign_with_target(vec![cell("order-2-point-0", 2.0, 2)], 7);
    let outcomes = scripted_outcomes(100, 64, &[2, 2, 2, 2, 2, 1, 1], work(100, 450, 431));
    let outcomes = &outcomes;
    let campaign_ref = &campaign;

    let receipt = run_osd_campaign(&checkpoint_path, &campaign, 100, serial(), || {
        move |context: OsdBlockContext<'_>| {
            assert_eq!(context.seed, campaign_ref.cell_seed(&context.cell.id));
            outcomes[context.block_index as usize]
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
    assert_eq!(receipt.cell_results[0].squared_block_bit_errors, Some(22));
    let completed = &receipt.cell_results[0];
    let ber_interval = completed
        .ber_confidence_interval
        .as_ref()
        .expect("completed cells record a BER interval");
    let bler_interval = completed
        .bler_confidence_interval
        .as_ref()
        .expect("completed cells record a BLER interval");
    assert_eq!(
        ber_interval.estimator,
        ConfidenceIntervalEstimator::BlockRatioProductInterval
    );
    assert_eq!(
        ber_interval.sampling_unit,
        Some(IntervalSamplingUnit::Block)
    );
    assert_eq!(
        bler_interval.estimator,
        ConfidenceIntervalEstimator::NegativeBinomialClopperPearson
    );
    assert_eq!(
        bler_interval.sampling_unit,
        Some(IntervalSamplingUnit::Block)
    );
    assert_eq!(ber_interval.level, 0.95);
    assert_eq!(bler_interval.level, interval_spec().component_level());
    assert!(ber_interval.lower <= receipt.cell_results[0].ber);
    assert!(ber_interval.upper >= receipt.cell_results[0].ber);
    assert_eq!(
        (bler_interval.lower, bler_interval.upper),
        gf2_stats::intervals::negative_binomial_interval(7, 100, interval_spec().component_level())
    );
    assert!(receipt.cell_results[0].accepts_published_value(12.0 / 6_400.0));
    assert!(!accepts_published_value(
        12.0 / 6_400.0,
        bler_interval,
        receipt.cell_results[0].cell.digitization_precision,
    ));
    assert_eq!(
        receipt.digitization_precision_unit,
        Some(DigitizationPrecisionUnit::Log10Decades)
    );
    assert_eq!(receipt.target_block_errors, Some(7));
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
        "block_ratio_product_interval"
    );
    assert_eq!(
        receipt_json["cell_results"][0]["bler_confidence_interval"]["estimator"],
        "negative_binomial_clopper_pearson"
    );
    assert_eq!(
        receipt_json["cell_results"][0]["squared_block_bit_errors"],
        22
    );
    assert_eq!(
        receipt_json["interval"]["method"],
        "negative_binomial_clopper_pearson"
    );
    assert_eq!(
        serde_json::from_value::<gf2_sim::osd_campaign::OsdCampaignReceipt>(receipt_json).unwrap(),
        receipt
    );

    let checkpoint = read_checkpoint(&checkpoint_path, &campaign);
    let checkpoint_json = serde_json::to_value(&checkpoint).unwrap();
    assert!(checkpoint_json.get("completed_cells").is_none());
    assert_eq!(
        serde_json::from_value::<OsdCampaignCheckpoint>(checkpoint_json).unwrap(),
        checkpoint
    );
    assert_eq!(checkpoint.cell_results, receipt.cell_results);
}

#[test]
fn only_completed_attempts_serialize_confidence_intervals() {
    let dir = TempDir::new("optional-intervals");
    let campaign = campaign(vec![cell("completed", 2.0, 2), cell("interrupted", 2.5, 2)]);
    let receipt = run_osd_campaign(
        dir.path().join("checkpoint.json"),
        &campaign,
        4,
        workers(4),
        || {
            |context: OsdBlockContext<'_>| {
                let errors = match (context.cell.id.as_str(), context.block_index) {
                    ("completed", 1) => 2,
                    ("completed", 3) => 1,
                    ("interrupted", 3) => 2,
                    _ => 0,
                };
                OsdBlockOutcome {
                    information_bits: 64,
                    information_bit_errors: errors,
                    work: work(1, 1, 1),
                }
            }
        },
    )
    .expect("interrupted progress is checkpointed");

    assert!(receipt.cell_results[0].ber_confidence_interval.is_some());
    assert!(receipt.cell_results[0].bler_confidence_interval.is_some());
    assert!(receipt.cell_results[1].ber_confidence_interval.is_none());
    assert!(receipt.cell_results[1].bler_confidence_interval.is_none());
    assert!(!receipt.cell_results[1].accepts_published_value(0.0));

    let json = serde_json::to_value(&receipt).expect("receipt serializes");
    assert!(json["cell_results"][0]
        .get("ber_confidence_interval")
        .is_some());
    assert!(json["cell_results"][0]
        .get("bler_confidence_interval")
        .is_some());
    assert!(json["cell_results"][1]
        .get("ber_confidence_interval")
        .is_none());
    assert!(json["cell_results"][1]
        .get("bler_confidence_interval")
        .is_none());
}

#[test]
fn bursty_fixture_clustered_ber_interval_is_wider_than_bit_independence_interval() {
    let dir = TempDir::new("bursty-interval");
    let campaign = campaign(vec![cell("bursty", 2.0, 2)]);
    let outcomes = scripted_outcomes(100, 64, &[32, 32], work(100, 100, 100));
    let outcomes = &outcomes;
    let receipt = run_osd_campaign(
        dir.path().join("checkpoint.json"),
        &campaign,
        100,
        serial(),
        || move |context: OsdBlockContext<'_>| outcomes[context.block_index as usize],
    )
    .expect("bursty fixture completes");

    let clustered = receipt.cell_results[0]
        .ber_confidence_interval
        .expect("completed cells record a BER interval");
    let bit_independence = gf2_stats::intervals::clopper_pearson_interval(64, 6_400, 0.95);
    let clustered_width = clustered.upper - clustered.lower;
    let bit_independence_width = bit_independence.1 - bit_independence.0;

    assert_eq!(clustered.sampling_unit, Some(IntervalSamplingUnit::Block));
    assert_eq!(
        clustered.estimator,
        ConfidenceIntervalEstimator::BlockRatioProductInterval
    );
    assert!(clustered_width > bit_independence_width);
}

/// The BER interval is the recorded block-error interval rescaled by the
/// bounded mean failing-block error fraction, so dividing the two recorded
/// intervals recovers a factor inside that fraction's domain.
#[test]
fn recorded_ber_interval_rescales_the_recorded_block_error_interval() {
    let dir = TempDir::new("product-structure");
    let campaign = campaign_with_target(vec![cell("product", 2.0, 2)], 100);
    let mut failing_errors = vec![13; 98];
    failing_errors.extend([14, 14]);
    let outcomes = scripted_outcomes(4_000, 64, &failing_errors, work(4_000, 4_000, 4_000));
    let outcomes = &outcomes;
    let receipt = run_osd_campaign(
        dir.path().join("checkpoint.json"),
        &campaign,
        4_000,
        workers(4),
        || move |context: OsdBlockContext<'_>| outcomes[context.block_index as usize],
    )
    .expect("product fixture completes");

    let ber = receipt.cell_results[0]
        .ber_confidence_interval
        .expect("completed cells record a BER interval");
    let bler = receipt.cell_results[0]
        .bler_confidence_interval
        .expect("completed cells record a BLER interval");
    let fraction_lower = ber.lower / bler.lower;
    let fraction_upper = ber.upper / bler.upper;
    let smallest_fraction = 1.0 / 64.0;

    assert!(smallest_fraction <= fraction_lower && fraction_lower <= fraction_upper);
    assert!(fraction_upper <= 1.0);
    assert!(fraction_lower <= 1_302.0 / 6_400.0 && 1_302.0 / 6_400.0 <= fraction_upper);
}

#[test]
fn schema_1_committed_receipt_remains_readable_with_historical_semantics() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("dev/simulation_results/osd-ebch-128-64/schema1/ebch_osd_awgn.json");
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
            && result.ber_confidence_interval.is_some()
            && result.bler_confidence_interval.is_some()
            && result
                .ber_confidence_interval
                .expect("schema-1 BER interval")
                .sampling_unit
                .is_none()
            && result
                .bler_confidence_interval
                .expect("schema-1 BLER interval")
                .sampling_unit
                .is_none()
            && result
                .ber_confidence_interval
                .expect("schema-1 BER interval")
                .estimator
                == ConfidenceIntervalEstimator::ClopperPearson
            && result
                .bler_confidence_interval
                .expect("schema-1 BLER interval")
                .estimator
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

    let first_id_ref = &first_id;
    let second_id_ref = &second_id;
    let first_receipt = run_osd_campaign(&checkpoint_path, &campaign, 4, serial(), || {
        move |context: OsdBlockContext<'_>| {
            let errors = if &context.cell.id == first_id_ref {
                match context.block_index {
                    1 => 1,
                    3 => 2,
                    _ => 0,
                }
            } else {
                assert_eq!(&context.cell.id, second_id_ref);
                u64::from(context.block_index == 3) * 2
            };
            OsdBlockOutcome {
                information_bits: 64,
                information_bit_errors: errors,
                work: work(1, 1, 1),
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

    let resumed_calls = Mutex::new(BTreeSet::new());
    let resumed_calls_ref = &resumed_calls;
    let campaign_ref = &campaign;
    let resumed = run_osd_campaign(&checkpoint_path, &campaign, 6, workers(3), || {
        move |context: OsdBlockContext<'_>| {
            resumed_calls_ref
                .lock()
                .expect("scripted evaluator lock")
                .insert((context.cell.id.clone(), context.block_index, context.seed));
            assert_eq!(&context.cell.id, second_id_ref);
            assert_eq!(context.seed, campaign_ref.cell_seed(second_id_ref));
            OsdBlockOutcome {
                information_bits: 64,
                information_bit_errors: u64::from(context.block_index == 9) * 2,
                work: work(1, 1, 1),
            }
        }
    })
    .expect("resume completes remaining cell");

    assert_eq!(
        resumed_calls.into_inner().expect("scripted evaluator lock"),
        (4..10)
            .map(|index| (second_id.clone(), index, campaign.cell_seed(&second_id)))
            .collect::<BTreeSet<_>>(),
        "the invocation bound caps the dispatched blocks at the remaining budget"
    );
    assert_eq!(resumed.termination, OsdCampaignTermination::Completed);
    assert_eq!(completed_cell_ids(&resumed), vec![first_id, second_id]);
    assert_eq!(resumed.cell_results.len(), 3);
    assert!(matches!(
        &resumed.cell_results[1].termination,
        OsdCellTermination::Interrupted
    ));

    let recovered = run_osd_campaign(&checkpoint_path, &campaign, 1, serial(), || {
        |_: OsdBlockContext<'_>| -> OsdBlockOutcome {
            panic!("a completed campaign must not execute another block")
        }
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
    let first_campaign =
        OsdCampaign::new(17, vec![cell.clone()], interval_spec(), 2, first_provenance).unwrap();
    run_osd_campaign(&checkpoint_path, &first_campaign, 4, workers(2), || {
        |context: OsdBlockContext<'_>| OsdBlockOutcome {
            information_bits: 64,
            information_bit_errors: u64::from(context.block_index == 3) * 8,
            work: work(1, 1, 1),
        }
    })
    .expect("first invocation persists");

    let mut second_provenance = provenance();
    second_provenance.runtime.invocation = vec![
        "osd-campaign".to_owned(),
        "--max-samples".to_owned(),
        "8".to_owned(),
    ];
    let second_campaign =
        OsdCampaign::new(17, vec![cell], interval_spec(), 2, second_provenance).unwrap();
    let receipt = run_osd_campaign(&checkpoint_path, &second_campaign, 8, workers(4), || {
        |context: OsdBlockContext<'_>| {
            assert!((4..12).contains(&context.block_index));
            OsdBlockOutcome {
                information_bits: 64,
                information_bit_errors: u64::from(context.block_index == 11) * 12,
                work: work(1, 1, 1),
            }
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

    run_osd_campaign(&checkpoint_path, &campaign, 2, workers(2), || {
        |_: OsdBlockContext<'_>| OsdBlockOutcome {
            information_bits: 64,
            information_bit_errors: 1,
            work: work(1, 1, 1),
        }
    })
    .expect("create structurally valid receipt-level evidence");
    let mut checkpoint = read_checkpoint(&checkpoint_path, &campaign);
    for result in &mut checkpoint.cell_results {
        result.termination = match result.cell.id.as_str() {
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
        result.ber_confidence_interval = None;
        result.bler_confidence_interval = None;
    }
    write_checkpoint(&checkpoint_path, &campaign, &checkpoint);

    let receipt = run_osd_campaign(&checkpoint_path, &campaign, 1, serial(), || {
        |_: OsdBlockContext<'_>| -> OsdBlockOutcome {
            panic!("preserved terminal cells must not be repeated")
        }
    })
    .expect("terminal evidence recovers");

    assert_eq!(completed_cell_ids(&receipt).len(), 3);
    assert!(receipt.cell_results.iter().all(|result| {
        result.ber_confidence_interval.is_none() && result.bler_confidence_interval.is_none()
    }));
    let recovered = run_osd_campaign(&checkpoint_path, &campaign, 1, serial(), || {
        |_: OsdBlockContext<'_>| -> OsdBlockOutcome {
            panic!("preserved terminal cells must not be repeated")
        }
    })
    .expect("terminal evidence recovers again");
    assert_eq!(recovered, receipt);
}

#[test]
fn per_block_outcomes_validate_bit_counts_and_fixed_block_length() {
    let campaign = campaign(vec![cell("accounting", 2.0, 2)]);

    let bit_dir = TempDir::new("invalid-bit-accounting");
    let bit_error = run_osd_campaign(
        bit_dir.path().join("checkpoint.json"),
        &campaign,
        1,
        serial(),
        || {
            |_: OsdBlockContext<'_>| OsdBlockOutcome {
                information_bits: 8,
                information_bit_errors: 9,
                work: work(1, 1, 1),
            }
        },
    )
    .expect_err("bit errors cannot exceed sampled bits");
    assert!(bit_error
        .to_string()
        .contains("information-bit errors cannot exceed the sampled block length"));

    let width_dir = TempDir::new("changing-block-length");
    let width_error = run_osd_campaign(
        width_dir.path().join("checkpoint.json"),
        &campaign,
        2,
        serial(),
        || {
            |context: OsdBlockContext<'_>| OsdBlockOutcome {
                information_bits: if context.block_index == 0 { 8 } else { 7 },
                information_bit_errors: 0,
                work: work(1, 1, 1),
            }
        },
    )
    .expect_err("information-block length cannot vary");
    assert!(width_error
        .to_string()
        .contains("information-block length cannot change within a cell"));
}

/// The failing-block decomposition needs every bit error to live inside a
/// failing block, and needs the squared per-block sum to be consistent with the
/// bit-error total it was accumulated from.
#[test]
fn failing_block_counters_are_validated_against_the_bit_error_total() {
    let campaign = campaign(vec![cell("failing-blocks", 2.0, 2)]);
    let outside = tampered_completed_receipt_error(
        "bit-errors-outside-failing-blocks",
        &campaign,
        |receipt| {
            receipt.bit_errors = 100;
            receipt.block_errors = 1;
            receipt.squared_block_bit_errors = Some(10_000);
        },
    );
    assert!(outside.contains("bit errors must fit inside the failing blocks"));

    let too_long = tampered_completed_receipt_error(
        "block-longer-than-information-block",
        &campaign,
        |receipt| receipt.squared_block_bit_errors = Some(800),
    );
    assert!(too_long.contains("more bit errors than its information-block length"));

    let inconsistent_squares =
        tampered_completed_receipt_error("squares-below-their-floor", &campaign, |receipt| {
            receipt.squared_block_bit_errors = Some(60)
        });
    assert!(
        inconsistent_squares.contains("squared block bit errors contradict the bit-error total")
    );
}

#[test]
fn receipt_validation_binds_termination_to_the_block_error_target() {
    let campaign = campaign(vec![cell("stopping", 2.0, 2)]);

    let early = tampered_completed_receipt_error("early-completion", &campaign, |receipt| {
        receipt.bit_errors = 6;
        receipt.block_errors = 1;
        receipt.squared_block_bit_errors = Some(36);
    });
    assert!(early
        .to_string()
        .contains("has not reached the target block-error count"));

    let overshoot =
        tampered_completed_receipt_error("overshoot-completion", &campaign, |receipt| {
            receipt.bit_errors = 18;
            receipt.block_errors = 3;
            receipt.squared_block_bit_errors = Some(108);
        });
    assert!(overshoot.to_string().contains(
        "must stop exactly at the target block-error count; the inverse-binomial interval assumes the last sampled block is the target's K-th error"
    ));

    let late = tampered_completed_receipt_error("late-interruption", &campaign, |receipt| {
        receipt.termination = OsdCellTermination::Interrupted;
        receipt.ber_confidence_interval = None;
        receipt.bler_confidence_interval = None;
    });
    assert!(late
        .to_string()
        .contains("has already reached the target block-error count"));
}

#[test]
#[should_panic(expected = "BER interval requires at least one sampled block")]
fn ber_interval_panics_for_zero_samples() {
    let _ = interval_spec().ber_interval(BlockSampleCounts {
        samples: 0,
        sampled_bits: 0,
        bit_errors: 0,
        block_errors: 0,
        squared_block_bit_errors: 0,
    });
}

/// The pinned eBCH(128,64) grid's four representative operating points, each
/// stopped at 100 block errors with the campaign's 64-bit information blocks.
/// The per-failing-block error fractions average about 0.2 with a standard
/// deviation near 0.03, so `squared_block_bit_errors` is the sum of squares
/// those two moments imply.
const OPERATING_POINTS: [(&str, u64, u64, u64); 4] = [
    ("1.55 dB", 195, 1_373, 19_216),
    ("3.47 dB", 3_700, 1_302, 17_317),
    ("4.56 dB", 140_000, 1_523, 23_560),
    ("5.23 dB", 630_000, 1_371, 19_162),
];

/// A scale-oblivious estimator produces endpoints orders of magnitude away from
/// the point estimate at these operating points, which makes any downstream
/// acceptance rule accept every published value. This is the regression guard
/// against that: the interval must exclude zero and span at most one decade.
#[test]
fn ber_interval_stays_informative_at_the_pinned_grid_operating_points() {
    let spec = interval_spec();
    for (label, samples, bit_errors, squared_block_bit_errors) in OPERATING_POINTS {
        let counts = BlockSampleCounts {
            samples,
            sampled_bits: samples * 64,
            bit_errors,
            block_errors: 100,
            squared_block_bit_errors,
        };
        let interval = spec.ber_interval(counts);
        let estimate = bit_errors as f64 / (samples * 64) as f64;

        println!(
            "{label}: n={samples} BER={estimate:.3e} interval=[{:.3e}, {:.3e}] ratio={:.2} width/estimate={:.2}",
            interval.lower,
            interval.upper,
            interval.upper / interval.lower,
            (interval.upper - interval.lower) / estimate,
        );
        assert!(interval.lower > 0.0, "{label} lower endpoint is vacuous");
        assert!(
            interval.upper / interval.lower <= 10.0,
            "{label} spans more than one decade"
        );
        assert!(interval.lower <= estimate && estimate <= interval.upper);
    }
}

/// A seeded bursty process with a known BER: a block fails with probability
/// `1 / 4`, and a failing block carries a uniform 8 to 15 of its 64
/// information bits in error. Sampling stops at 100 block errors, matching the
/// design the interval claims coverage for.
fn bursty_stopped_sample(rng: &mut ChaCha20Rng, block_errors: u64) -> BlockSampleCounts {
    let mut counts = BlockSampleCounts {
        samples: 0,
        sampled_bits: 0,
        bit_errors: 0,
        block_errors: 0,
        squared_block_bit_errors: 0,
    };
    while counts.block_errors < block_errors {
        counts.samples += 1;
        counts.sampled_bits += 64;
        if rng.next_u64().is_multiple_of(4) {
            let errors = 8 + rng.next_u64() % 8;
            counts.block_errors += 1;
            counts.bit_errors += errors;
            counts.squared_block_bit_errors += errors * errors;
        }
    }
    counts
}

/// Empirical coverage over independent replications of that process must reach
/// the nominal level. The tolerance is 0.05 below nominal: at 200 replications
/// the Monte Carlo standard error of a coverage estimate near 0.95 is about
/// 0.015, so the tolerance is roughly three standard errors and the seeded
/// draw makes the outcome deterministic.
#[test]
fn ber_interval_covers_a_seeded_bursty_process_at_its_nominal_level() {
    const REPLICATIONS: u32 = 200;
    const TOLERANCE: f64 = 0.05;

    let spec = interval_spec();
    let true_ber = 0.25 * 11.5 / 64.0;
    let mut rng = ChaCha20Rng::seed_from_u64(0x0bad_5eed_1234_5678);
    let mut covered = 0_u32;
    for _ in 0..REPLICATIONS {
        let interval = spec.ber_interval(bursty_stopped_sample(&mut rng, 100));
        if interval.lower <= true_ber && true_ber <= interval.upper {
            covered += 1;
        }
    }

    let coverage = f64::from(covered) / f64::from(REPLICATIONS);
    assert!(
        coverage >= spec.level - TOLERANCE,
        "empirical coverage {coverage} fell below the nominal level {}",
        spec.level
    );
}

/// The stopping design fixes which exact inversion is valid, so the campaign
/// refuses the fixed-trial method. The method vocabulary still dispatches what
/// it names, which is what keeps a schema 1 receipt's estimator meaningful.
#[test]
fn a_block_error_stopping_campaign_refuses_the_fixed_trial_interval_method() {
    let spec = BinomialIntervalSpec::new(BinomialIntervalMethod::ClopperPearson, 0.95)
        .expect("valid interval");
    let error = OsdCampaign::new(17, vec![cell("fixed-trial", 2.0, 2)], spec, 2, provenance())
        .expect_err("a stopping-time block count refuses the fixed-trial inversion");
    assert!(error
        .to_string()
        .contains("inverse-binomial exact interval"));

    let interval = spec.bler_interval(7, 100);
    assert_eq!(
        interval.estimator,
        ConfidenceIntervalEstimator::ClopperPearson
    );
    assert_eq!(
        (interval.lower, interval.upper),
        gf2_stats::intervals::clopper_pearson_interval(7, 100, spec.component_level())
    );
}

/// A digitization precision large enough to overflow its decade scale must
/// still produce a decision rather than an indeterminate comparison.
#[test]
fn a_precision_beyond_the_decade_range_keeps_the_predicate_total() {
    let zero_width = BinomialConfidenceInterval {
        estimator: ConfidenceIntervalEstimator::BlockRatioProductInterval,
        sampling_unit: Some(IntervalSamplingUnit::Block),
        level: 0.95,
        lower: 0.0,
        upper: 0.0,
    };
    let positive = BinomialConfidenceInterval {
        lower: 0.2,
        upper: 0.3,
        ..zero_width
    };

    for delta in [400.0, 1e6, f64::MAX] {
        assert!(accepts_published_value(0.0, &zero_width, delta));
        assert!(!accepts_published_value(1e-300, &zero_width, delta));
        assert!(accepts_published_value(0.0, &positive, delta));
        assert!(accepts_published_value(f64::MAX, &positive, delta));
        assert!(!accepts_published_value(f64::INFINITY, &positive, delta));
    }
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
