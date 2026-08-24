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
            binary_sha256: emitter_digest().parse().unwrap(),
            deps_source_revision: "95ccd9776376b2b060e0dd40785e2effae29e766"
                .parse::<GitRevision>()
                .unwrap(),
            deps_source_dirty: false,
            compiler_version: "rustc test".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha test".to_owned(),
            invocation: vec!["permanent_campaign".to_owned()],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "test".to_owned(),
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

fn commit_campaign_manifest(checkout: &Path, output: &Path) {
    let relative = output.strip_prefix(checkout).unwrap();
    let added = Command::new("git")
        .args(["-C", checkout.to_str().unwrap(), "add", "--"])
        .arg(relative)
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
    fs::copy(manifest, output.join("manifest.json")).unwrap();
    commit_campaign_manifest(&checkout, &output);
    (checkout, output)
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
