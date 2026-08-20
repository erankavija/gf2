use std::fs;
use std::path::PathBuf;
use std::process::Command;

use gf2_sim::permanent_campaign::provenance::build_revision;
use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, Backend, CampaignManifest, CellSpec, DeterminantPlan,
    GitRevision, Provenance, RngAlgorithm, ShardSpec, StreamPurpose, SCHEMA_VERSION,
};

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
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let checkout = parent.join("checkout");
    let output_path =
        checkout.join("dev/simulation_results/permanent-zero-fraction/campaign-bin-test");
    fs::create_dir_all(&parent).unwrap();
    fs::create_dir_all(&manifest_path).unwrap();
    fs::write(
        &manifest_file,
        serde_json::to_vec_pretty(&unavailable_backend_manifest()).unwrap(),
    )
    .unwrap();

    let clone = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--no-local",
            repository.to_str().unwrap(),
            checkout.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        clone.status.success(),
        "test checkout must be clean and usable: {}",
        String::from_utf8_lossy(&clone.stderr)
    );
    let built_revision = build_revision().to_string();
    let checkout_revision = Command::new("git")
        .args([
            "-C",
            checkout.to_str().unwrap(),
            "checkout",
            "--quiet",
            &built_revision,
        ])
        .output()
        .unwrap();
    assert!(
        checkout_revision.status.success(),
        "test checkout must contain the binary's build revision: {}",
        String::from_utf8_lossy(&checkout_revision.stderr)
    );
    fs::create_dir_all(output_path.parent().unwrap()).unwrap();

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
