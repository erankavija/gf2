use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, Backend, CampaignManifest, CellSpec, DeterminantPlan,
    GitRevision, Provenance, RngAlgorithm, ShardSpec, StreamPurpose, SCHEMA_VERSION,
};

fn emitter_digest() -> String {
    let output = Command::new("sha256sum")
        .arg(env!("CARGO_BIN_EXE_permanent_campaign"))
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn manifest() -> CampaignManifest {
    CampaignManifest {
        schema_version: SCHEMA_VERSION,
        campaign_id: "campaign-bin-test".parse().unwrap(),
        root_seed: 0x1234,
        stream_purposes: vec![StreamPurpose {
            name: "campaign-cells".parse().unwrap(),
            tag: 3,
        }],
        cells: vec![CellSpec {
            q: 3,
            n: 2,
            matrix_count: 2,
            shard_size: 2,
            shards: vec![ShardSpec {
                shard_id: 0,
                stream_index: 10,
            }],
            backend: Backend::GenericRyser,
            backend_receipt: ArtifactIdentity {
                path: "dev/benchmarks/permanent/test-receipt.json"
                    .parse()
                    .unwrap(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .parse()
                    .unwrap(),
            },
            determinant_companion: DeterminantPlan::NotEvaluated,
        }],
        provenance: Provenance {
            git_revision: "95ccd9776376b2b060e0dd40785e2effae29e766"
                .parse::<GitRevision>()
                .unwrap(),
            binary_sha256: Some(emitter_digest().parse().unwrap()),
            deps_source_revision: Some(
                "95ccd9776376b2b060e0dd40785e2effae29e766"
                    .parse::<GitRevision>()
                    .unwrap(),
            ),
            deps_source_dirty: Some(false),
            compiler_version: "rustc test".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha test".to_owned(),
            invocation: vec!["permanent_campaign".to_owned()],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "test".to_owned(),
            cpu_physical_cores: None,
            cpu_logical_threads: None,
            gpu_model: Availability::NotPresent,
        },
    }
}

fn unavailable_backend_manifest() -> CampaignManifest {
    let mut campaign = manifest();
    let cell = &mut campaign.cells[0];
    cell.q = 5;
    cell.n = 20;
    cell.backend = Backend::IntraMatrixParallel;
    campaign
}

fn mixed_backend_manifest() -> CampaignManifest {
    let mut campaign = manifest();
    let mut accelerator_cell = campaign.cells[0].clone();
    accelerator_cell.q = 5;
    accelerator_cell.n = 20;
    accelerator_cell.shards[0].stream_index = 11;
    accelerator_cell.backend = Backend::Accelerator;
    campaign.cells.push(accelerator_cell);
    campaign
}

fn launch_cost_manifest() -> CampaignManifest {
    let mut campaign = manifest();
    let mut field_three_accelerator = campaign.cells[0].clone();
    field_three_accelerator.n = 3;
    field_three_accelerator.shards[0].stream_index = 11;
    field_three_accelerator.backend = Backend::Accelerator;
    let mut field_five_accelerator = campaign.cells[0].clone();
    field_five_accelerator.q = 5;
    field_five_accelerator.shards[0].stream_index = 12;
    field_five_accelerator.backend = Backend::Accelerator;
    campaign
        .cells
        .extend([field_three_accelerator, field_five_accelerator]);
    campaign
}

fn exact_selection_manifest() -> CampaignManifest {
    let mut campaign = manifest();
    let first = &mut campaign.cells[0];
    first.q = 7;
    first.n = 4;
    first.shards[0].stream_index = 10;
    let mut target = first.clone();
    target.n = 20;
    target.shards[0].stream_index = 11;
    campaign.cells.push(target);
    campaign
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gf2-permanent-campaign-bin-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn commit_campaign_manifest(checkout: &Path) {
    let added = Command::new("git")
        .args(["-C", checkout.to_str().unwrap(), "add", "--all"])
        .output()
        .unwrap();
    assert!(added.status.success());
    let committed = Command::new("git")
        .args([
            "-C",
            checkout.to_str().unwrap(),
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "--no-gpg-sign",
            "-m",
            "freeze campaign manifest",
        ])
        .output()
        .unwrap();
    assert!(committed.status.success());
}

fn campaign_checkout(parent: &Path, manifest: &Path) -> (PathBuf, PathBuf) {
    let checkout = parent.join("checkout");
    let output = checkout.join("dev/simulation_results/permanent-zero-fraction/campaign-bin-test");
    fs::create_dir_all(&output).unwrap();
    let initialized = Command::new("git")
        .args([
            "-C",
            checkout.to_str().unwrap(),
            "init",
            "--quiet",
            "--initial-branch=main",
        ])
        .output()
        .unwrap();
    assert!(initialized.status.success());
    fs::create_dir_all(checkout.join("crates/gf2-sim")).unwrap();
    fs::write(
        checkout.join("crates/gf2-sim/lib.rs"),
        "pub fn fixture() {}\n",
    )
    .unwrap();
    fs::write(checkout.join("Cargo.lock"), "# fixture lockfile\n").unwrap();
    fs::copy(manifest, output.join("manifest.json")).unwrap();
    commit_campaign_manifest(&checkout);
    (checkout, output)
}

fn files_under(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn run_with_cost_table(label: &str, table: &str) -> std::process::Output {
    let parent = temp_path(label);
    let manifest_path = parent.join("manifest");
    let manifest_file = manifest_path.join("manifest.json");
    let output_path = parent.join("campaign-bin-test");
    fs::create_dir_all(&manifest_path).unwrap();
    fs::create_dir_all(&output_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&launch_cost_manifest()).unwrap(),
    )
    .unwrap();
    let before = files_under(&output_path);
    let table_path = parent.join("accelerator-costs.csv");
    fs::write(&table_path, table).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "5",
            "--accelerator-cost-table",
            table_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        files_under(&output_path),
        before,
        "cost-table preflight must not draw or emit campaign evidence"
    );
    fs::remove_dir_all(parent).unwrap();
    result
}

#[test]
fn print_provenance_reports_the_emitting_binary_and_source_revision() {
    let parent = temp_path("provenance");
    let manifest_path = parent.join("manifest");
    let manifest_file = manifest_path.join("manifest.json");
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&manifest()).unwrap(),
    )
    .unwrap();
    let (_checkout, output_path) = campaign_checkout(&parent, &manifest_file);

    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--print-provenance",
            "--manifest",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let provenance: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(provenance["binary_sha256"], emitter_digest());
    assert_eq!(provenance["deps_source_dirty"], false);
    let revision = provenance["deps_source_revision"].as_str().unwrap();
    assert_eq!(revision.len(), 40);
    assert!(revision
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f')));

    let _ = fs::remove_dir_all(parent);
}

#[test]
fn binary_refuses_emission_before_writing_outside_repository() {
    let parent = temp_path("red");
    let manifest_path = parent.join("manifest");
    let manifest_file = manifest_path.join("manifest.json");
    let output_path = parent.join("campaign-bin-test");
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&manifest()).unwrap(),
    )
    .unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "3",
        ])
        .output()
        .unwrap();

    assert!(
        !result.status.success(),
        "binary must refuse emission; status {:?}\nstderr:\n{}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("emission refused"),
        "stderr must name the emission refusal; stderr:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output_path.join("shards").exists());
    assert!(!output_path.join("summaries").exists());

    let _ = fs::remove_dir_all(parent);
}

#[test]
fn binary_refuses_an_unavailable_backend_with_cell_and_backend() {
    let parent = temp_path("backend");
    let manifest_path = parent.join("manifest");
    let manifest_file = manifest_path.join("manifest.json");
    fs::create_dir_all(&parent).unwrap();
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&unavailable_backend_manifest()).unwrap(),
    )
    .unwrap();
    let (_checkout, output_path) = campaign_checkout(&parent, &manifest_file);

    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "stderr:\n{stderr}");
    assert!(stderr.contains("q=5"), "stderr:\n{stderr}");
    assert!(stderr.contains("n=20"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("intra_matrix_parallel"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(parent);
}

#[test]
fn binary_requires_accelerator_costs_only_for_the_selected_field() {
    let parent = temp_path("mixed-backend");
    let manifest_path = parent.join("manifest");
    let manifest_file = manifest_path.join("manifest.json");
    fs::create_dir_all(&parent).unwrap();
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&mixed_backend_manifest()).unwrap(),
    )
    .unwrap();
    let (_checkout, output_path) = campaign_checkout(&parent, &manifest_file);

    let processor = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "3",
        ])
        .output()
        .unwrap();
    assert!(
        processor.status.success(),
        "processor-only selected field must run without an accelerator cost table; stderr:\n{}",
        String::from_utf8_lossy(&processor.stderr)
    );

    let accelerator = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "5",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&accelerator.stderr);
    assert!(!accelerator.status.success(), "stderr:\n{stderr}");
    assert!(
        stderr.contains("--accelerator-cost-table is required"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(parent);
}

#[test]
fn binary_accepts_the_complete_exact_accelerator_key_set() {
    let result = run_with_cost_table("complete-cost-table", "q,n,per_matrix_us\n3,3,17\n5,2,19\n");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "the fixture output outside a repository must refuse emission"
    );
    assert!(stderr.contains("emission refused"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("accelerator cost table"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn binary_rejects_invalid_cost_tables_before_draw_or_emission() {
    let cases = [
        ("missing-cost-row", "q,n,per_matrix_us\n3,3,17\n", "missing"),
        (
            "duplicate-cost-row",
            "q,n,per_matrix_us\n3,3,17\n3,3,18\n5,2,19\n",
            "duplicate",
        ),
        (
            "zero-cost-row",
            "q,n,per_matrix_us\n3,3,17\n5,2,0\n",
            "positive",
        ),
        (
            "malformed-cost-row",
            "q,n,per_matrix_us\n3,3,17\n5,2,not-an-integer\n",
            "integer",
        ),
        (
            "processor-only-cost-row",
            "q,n,per_matrix_us\n3,2,13\n3,3,17\n5,2,19\n",
            "processor-backed",
        ),
        (
            "wrong-cell-cost-row",
            "q,n,per_matrix_us\n3,3,17\n5,2,19\n7,2,23\n",
            "not a manifest cell",
        ),
    ];
    for (label, table, expected) in cases {
        let result = run_with_cost_table(label, table);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success(), "{label} unexpectedly passed");
        assert!(
            stderr.contains("accelerator cost table"),
            "{label}: {stderr}"
        );
        assert!(stderr.contains(expected), "{label}: {stderr}");
        assert!(result.stdout.is_empty(), "{label} emitted progress output");
    }
}

#[test]
fn production_table_is_accepted_by_the_production_cli_and_manifest() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let campaign = repository
        .join("dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829");
    let table =
        repository.join("dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv");
    assert!(
        table.is_file(),
        "the frozen production cost table must exist"
    );
    let before = files_under(&campaign);
    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .current_dir(repository)
        .args([
            "--manifest",
            campaign.to_str().unwrap(),
            "--output",
            campaign.to_str().unwrap(),
            "--q",
            "7",
            "--n",
            "20",
            "--workers",
            "1",
            "--accelerator-cost-table",
            table.to_str().unwrap(),
            "--accelerator-launch-cap-ms",
            "500",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "the frozen emitter identity must remain binding"
    );
    assert!(stderr.contains("emission refused"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("accelerator cost table"),
        "stderr:\n{stderr}"
    );
    assert_eq!(
        files_under(&campaign),
        before,
        "production-table preflight must preserve zero-draw dataset bytes"
    );
}

#[test]
fn exact_selector_dry_run_schedules_only_the_requested_cell() {
    let parent = temp_path("exact-selector");
    let manifest_path = parent.join("manifest");
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        manifest_path.join("manifest.json"),
        serde_json::to_vec_pretty(&exact_selection_manifest()).unwrap(),
    )
    .unwrap();
    let output_path = parent.join("campaign-bin-test");

    let result = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .args([
            "--dry-run-schedule",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--q",
            "7",
            "--n",
            "20",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "schedule q=7 n=20 shards=1\n"
    );
    assert!(
        !output_path.exists(),
        "dry scheduling must not open a checkpoint, draw, or emit output"
    );
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn invalid_exact_selector_fails_before_checkpoint_or_output_creation() {
    let parent = temp_path("invalid-exact-selector");
    let manifest_path = parent.join("manifest");
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        manifest_path.join("manifest.json"),
        serde_json::to_vec_pretty(&exact_selection_manifest()).unwrap(),
    )
    .unwrap();

    for arguments in [
        vec!["--q", "7", "--n", "19"],
        vec!["--q", "5", "--n", "20"],
        vec!["--n", "20"],
    ] {
        let output_path = parent.join(format!("output-{}", arguments.join("-")));
        let mut command = Command::new(env!("CARGO_BIN_EXE_permanent_campaign"));
        command.args([
            "--dry-run-schedule",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ]);
        let result = command.args(arguments).output().unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "invalid selection unexpectedly passed"
        );
        assert!(
            stderr.contains("exact cell selector"),
            "selector failure must be explicit; stderr:\n{stderr}"
        );
        assert!(
            !output_path.exists(),
            "invalid selection must fail before checkpoint or output creation"
        );
    }
    fs::remove_dir_all(parent).unwrap();
}
