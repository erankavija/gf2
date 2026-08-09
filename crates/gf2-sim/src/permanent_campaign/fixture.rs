//! Shared campaign-dataset fixtures for this module's test suites.
//!
//! One conforming dataset builder serves both the schema conformance suite and
//! the provenance and integrity suite, so the published shape is described in
//! one place. The generated [`INTEGRITY_FILE`] covers the dataset exactly as
//! written here; a test that mutates a raw file afterwards is exercising
//! [`conform_dataset`](super::schema::conform_dataset), which checks document
//! shape and cross-document counts and never consults the integrity file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::provenance::generate_integrity_file;
use super::schema::{
    encode_summary_csv, field_summary_file, shard_record_file, AcceptanceVerdict, ArtifactIdentity,
    Availability, Backend, CampaignManifest, CellSpec, CellTerminalState, DeterminantCount,
    DeterminantEstimate, DeterminantPlan, FieldSummary, GitRevision, HaltReason, Interval,
    ProportionEstimate, Provenance, RngAlgorithm, ShardRecord, ShardSpec, StreamAddress,
    StreamPurpose, SummaryRow, INTEGRITY_FILE, MANIFEST_FILE, POOLED_SUMMARY_FILE, SCHEMA_VERSION,
    SUMMARY_CSV_FIELDS,
};

/// Directory name every fixture dataset uses, matching its `campaign_id`.
pub(crate) const FIXTURE_CAMPAIGN_ID: &str = "campaign-2026-08-09";

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// An isolated temporary campaign directory removed when the test ends.
pub(crate) struct TestDir {
    root: PathBuf,
    parent: PathBuf,
}

impl TestDir {
    /// Creates `<temp>/<unique>/campaign-2026-08-09`.
    pub(crate) fn new() -> Self {
        let parent = unique_temp_dir("gf2-sim-dataset");
        let root = parent.join(FIXTURE_CAMPAIGN_ID);
        fs::create_dir_all(&root).expect("create canonical campaign directory");
        Self { root, parent }
    }

    /// Returns the campaign-id directory holding the dataset.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.parent);
    }
}

/// Returns a process-unique, unused path under the system temporary directory.
pub(crate) fn unique_temp_dir(prefix: &str) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{id}", std::process::id()))
}

/// Returns the placeholder source revision recorded by the default fixture.
pub(crate) fn fixture_revision() -> GitRevision {
    "95ccd9776376b2b060e0dd40785e2effae29e766"
        .parse()
        .expect("fixture revision is a full object name")
}

/// Returns the single-cell root manifest the default fixture publishes.
pub(crate) fn manifest() -> CampaignManifest {
    manifest_at_revision(&fixture_revision())
}

/// Returns the default root manifest with `revision` recorded as its source.
pub(crate) fn manifest_at_revision(revision: &GitRevision) -> CampaignManifest {
    CampaignManifest {
        schema_version: SCHEMA_VERSION,
        campaign_id: FIXTURE_CAMPAIGN_ID.parse().unwrap(),
        root_seed: 0x5758_790d,
        stream_purposes: vec![StreamPurpose {
            name: "campaign-cells".parse().unwrap(),
            tag: 3,
        }],
        cells: vec![CellSpec {
            q: 3,
            n: 4,
            matrix_count: 20,
            shard_size: 10,
            shards: vec![
                ShardSpec {
                    shard_id: 0,
                    stream_index: 100,
                },
                ShardSpec {
                    shard_id: 1,
                    stream_index: 101,
                },
            ],
            backend: Backend::Scalar,
            backend_receipt: ArtifactIdentity {
                path: "dev/benchmarks/permanent-backend-selection/test-receipt.json"
                    .parse()
                    .unwrap(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .parse()
                    .unwrap(),
            },
            determinant_companion: DeterminantPlan::NotEvaluated,
        }],
        provenance: Provenance {
            git_revision: revision.clone(),
            compiler_version: "rustc 1.95.0".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha 0.9.0".to_owned(),
            invocation: vec![
                "gf2-permanent-campaign".to_owned(),
                "--manifest".to_owned(),
                "manifest.json".to_owned(),
            ],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "Test CPU".to_owned(),
            gpu_model: Availability::NotPresent,
        },
    }
}

/// Returns one $q = 3$, $n = 4$ shard record of ten matrices.
pub(crate) fn shard(shard_id: u64, stream_index: u64) -> ShardRecord {
    ShardRecord {
        schema_version: SCHEMA_VERSION,
        shard_id,
        stream_address: StreamAddress {
            root_seed: 0x5758_790d,
            q: 3,
            n: 4,
            purpose_tag: 3,
            stream_index,
        },
        matrix_count: 10,
        permanent_zero_count: 3,
        permanent_histogram: vec![3, 4, 3],
        determinant: DeterminantCount::NotEvaluated,
    }
}

/// Returns the completed summary row pooling both fixture shards.
pub(crate) fn summary_row() -> SummaryRow {
    SummaryRow {
        schema_version: SCHEMA_VERSION,
        q: 3,
        n: 4,
        matrix_count: 20,
        permanent_zero_count: 6,
        determinant: DeterminantCount::NotEvaluated,
        terminal_state: CellTerminalState::Completed {
            permanent_estimate: ProportionEstimate {
                point: 0.3,
                interval: Interval {
                    lower: 0.145,
                    upper: 0.519,
                },
            },
            permanent_verdict: AcceptanceVerdict::Accepted,
            determinant_estimate: DeterminantEstimate::NotEvaluated,
        },
    }
}

/// Writes a complete conforming single-field dataset into `root`.
pub(crate) fn write_fixture(root: &Path) {
    write_fixture_at_revision(root, &fixture_revision());
}

/// Writes the single-field dataset with `revision` recorded as its source.
pub(crate) fn write_fixture_at_revision(root: &Path, revision: &GitRevision) {
    let campaign = manifest_at_revision(revision);
    fs::write(
        root.join(MANIFEST_FILE),
        serde_json::to_vec_pretty(&campaign).unwrap(),
    )
    .unwrap();

    let shard_dir = root.join("shards/q3/n04");
    fs::create_dir_all(&shard_dir).unwrap();
    for (id, stream) in [(0, 100), (1, 101)] {
        fs::write(
            shard_dir.join(format!("shard-{id:06}.json")),
            serde_json::to_vec_pretty(&shard(id, stream)).unwrap(),
        )
        .unwrap();
    }

    let summary_dir = root.join("summaries");
    fs::create_dir_all(&summary_dir).unwrap();
    let field_summary = FieldSummary {
        schema_version: SCHEMA_VERSION,
        q: 3,
        rows: vec![summary_row()],
    };
    fs::write(
        summary_dir.join("q3.json"),
        serde_json::to_vec_pretty(&field_summary).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join(POOLED_SUMMARY_FILE),
        encode_summary_csv(&field_summary.rows),
    )
    .unwrap();

    write_integrity_file(root, &campaign);
}

/// Extends the single-field fixture with a second field arm.
pub(crate) fn write_multifield_fixture(root: &Path) {
    write_fixture(root);

    let mut campaign = manifest();
    let backend_receipt = campaign.cells[0].backend_receipt.clone();
    campaign.cells.push(CellSpec {
        q: 5,
        n: 4,
        matrix_count: 10,
        shard_size: 10,
        shards: vec![ShardSpec {
            shard_id: 0,
            stream_index: 200,
        }],
        backend: Backend::Scalar,
        backend_receipt,
        determinant_companion: DeterminantPlan::NotEvaluated,
    });
    fs::write(
        root.join(MANIFEST_FILE),
        serde_json::to_vec_pretty(&campaign).unwrap(),
    )
    .unwrap();

    let mut q5_shard = shard(0, 200);
    q5_shard.stream_address.q = 5;
    q5_shard.permanent_zero_count = 2;
    q5_shard.permanent_histogram = vec![2; 5];
    let shard_dir = root.join("shards/q5/n04");
    fs::create_dir_all(&shard_dir).unwrap();
    fs::write(
        shard_dir.join("shard-000000.json"),
        serde_json::to_vec_pretty(&q5_shard).unwrap(),
    )
    .unwrap();

    let mut q5_row = summary_row();
    q5_row.q = 5;
    q5_row.matrix_count = 10;
    q5_row.permanent_zero_count = 2;
    q5_row.terminal_state = CellTerminalState::Completed {
        permanent_estimate: ProportionEstimate {
            point: 0.2,
            interval: Interval {
                lower: 0.05,
                upper: 0.45,
            },
        },
        permanent_verdict: AcceptanceVerdict::Accepted,
        determinant_estimate: DeterminantEstimate::NotEvaluated,
    };
    let q5_summary = FieldSummary {
        schema_version: SCHEMA_VERSION,
        q: 5,
        rows: vec![q5_row.clone()],
    };
    fs::write(
        root.join("summaries/q5.json"),
        serde_json::to_vec_pretty(&q5_summary).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join(POOLED_SUMMARY_FILE),
        encode_summary_csv(&[summary_row(), q5_row]),
    )
    .unwrap();

    write_integrity_file(root, &campaign);
}

/// Rewrites the field and pooled summaries as one halted cell.
///
/// The counts are supplied rather than derived because a halted cell pools
/// exactly the shard subset it executed, which the caller chooses by removing
/// shard files. This does not regenerate the integrity file: schema
/// conformance never consults it, and a caller that needs it consistent
/// finishes with [`write_integrity_file`].
pub(crate) fn write_halted_summary(
    root: &Path,
    matrix_count: u64,
    permanent_zero_count: u64,
    determinant: DeterminantCount,
    reason: HaltReason,
) {
    let path = root.join(field_summary_file(3));
    let mut summary: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let row = summary["rows"][0].as_object_mut().unwrap();
    row.insert("matrix_count".to_owned(), json!(matrix_count));
    row.insert(
        "permanent_zero_count".to_owned(),
        json!(permanent_zero_count),
    );
    row.remove("permanent_estimate");
    row.remove("permanent_verdict");
    row.insert(
        "determinant".to_owned(),
        serde_json::to_value(&determinant).unwrap(),
    );
    row.insert(
        "terminal_state".to_owned(),
        json!({"state": "halted", "reason": halt_reason_token(reason)}),
    );
    fs::write(&path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();

    let (determinant_state, determinant_sample_count, determinant_zero_count) = match determinant {
        DeterminantCount::NotEvaluated => {
            ("not_evaluated".to_owned(), String::new(), String::new())
        }
        DeterminantCount::Evaluated {
            sample_count,
            zero_count,
        } => (
            "evaluated".to_owned(),
            sample_count.to_string(),
            zero_count.to_string(),
        ),
    };
    let csv_fields = [
        SCHEMA_VERSION.to_string(),
        "3".to_owned(),
        "4".to_owned(),
        matrix_count.to_string(),
        permanent_zero_count.to_string(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        determinant_state,
        determinant_sample_count,
        determinant_zero_count,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        "halted".to_owned(),
        halt_reason_token(reason).to_owned(),
    ];
    fs::write(
        root.join(POOLED_SUMMARY_FILE),
        format!(
            "{}\n{}\n",
            SUMMARY_CSV_FIELDS.join(","),
            csv_fields.join(",")
        ),
    )
    .unwrap();
}

/// Returns the pooled-CSV token for a halt reason, via its own serialization.
fn halt_reason_token(reason: HaltReason) -> &'static str {
    match reason {
        HaltReason::AcceptanceFailure => "acceptance_failure",
        HaltReason::BackendUnavailable => "backend_unavailable",
        HaltReason::ExecutionFailure => "execution_failure",
    }
}

/// Writes a dataset whose only cell halted after one of its two shards.
///
/// The second shard is legitimately absent: the cell never executed it. This
/// is the case that must stay coverable, so the integrity file it leaves
/// behind omits that shard and still verifies.
pub(crate) fn write_halted_fixture(root: &Path, revision: &GitRevision) {
    write_fixture_at_revision(root, revision);
    fs::remove_file(root.join(shard_record_file(3, 4, 1))).unwrap();
    write_halted_summary(
        root,
        10,
        3,
        DeterminantCount::NotEvaluated,
        HaltReason::ExecutionFailure,
    );
    write_integrity_file(root, &manifest_at_revision(revision));
}

/// Regenerates the integrity file covering the dataset currently in `root`.
pub(crate) fn write_integrity_file(root: &Path, campaign: &CampaignManifest) {
    let integrity =
        generate_integrity_file(root, campaign).expect("fixture raw files must be hashable");
    fs::write(root.join(INTEGRITY_FILE), integrity).unwrap();
}
