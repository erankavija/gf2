use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tuning_campaign_support::journal::{atomic_write_new, CheckpointStore, ResumeIdentity};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "gf2-checkpoint-provenance-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn digest(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn identity() -> ResumeIdentity {
    ResumeIdentity {
        protocol_digest: digest(b'1'),
        source_revision: "a".repeat(40),
        source_sha256: digest(b'2'),
        ordered_work_manifest_sha256: digest(b'3'),
        process_descriptors_sha256: digest(b'4'),
        executable_sha256: BTreeMap::from([(String::from("producer"), digest(b'5'))]),
        behavior_sha256: BTreeMap::from([(String::from("producer"), digest(b'6'))]),
        lifecycle_schema: String::from("tuning-campaign-lifecycle-v1"),
        lifecycle_behavior_sha256: digest(b'7'),
        feature_contract: String::from("fixture"),
        thread_contract: String::from("one"),
        host_identity: String::from("fixture-host"),
    }
}

fn publish(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if path.try_exists()? {
        if fs::read(path)? == bytes {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "immutable fixture publisher saw different bytes",
            ))
        }
    } else {
        atomic_write_new(path, bytes)
    }
}

#[test]
fn metadata_only_revision_change_allows_resume_and_initialize_replay() {
    let scratch = Scratch::new();
    let root = scratch.0.join("checkpoints");
    let original = identity();
    let store =
        CheckpointStore::initialize_with(&root, "campaign", original.clone(), publish).unwrap();
    drop(store);

    let mut metadata_only = original.clone();
    metadata_only.source_revision = "b".repeat(40);
    CheckpointStore::resume(&root, "campaign", metadata_only.clone()).unwrap();
    CheckpointStore::initialize_with(&root, "campaign", metadata_only, publish).unwrap();

    let mut metadata_only = original.clone();
    metadata_only.source_revision.clear();
    CheckpointStore::resume(&root, "campaign", metadata_only.clone()).unwrap();
    CheckpointStore::initialize_with(&root, "campaign", metadata_only, publish).unwrap();
}

#[test]
fn metadata_only_resume_retains_completed_result_and_original_manifest() {
    let scratch = Scratch::new();
    let root = scratch.0.join("checkpoints");
    let original = identity();
    let mut store =
        CheckpointStore::initialize_with(&root, "campaign", original.clone(), publish).unwrap();
    store
        .accept(
            "producer/unit-0",
            &json!({"input": [1, 2, 3]}),
            &json!({"status": "complete", "value": 42}),
        )
        .unwrap();
    let manifest_before = fs::read(root.join("manifest.json")).unwrap();
    drop(store);

    let mut metadata_only = original.clone();
    metadata_only.source_revision.clear();
    let resumed = CheckpointStore::resume(&root, "campaign", metadata_only).unwrap();
    let (case, result): (serde_json::Value, serde_json::Value) =
        resumed.load("producer/unit-0").unwrap();
    assert_eq!(case, json!({"input": [1, 2, 3]}));
    assert_eq!(result, json!({"status": "complete", "value": 42}));
    assert_eq!(
        fs::read(root.join("manifest.json")).unwrap(),
        manifest_before
    );
    assert_eq!(resumed.identity(), &original);
}

#[test]
fn changed_producing_digest_rejects_before_mutating_checkpoint_evidence() {
    let scratch = Scratch::new();
    let root = scratch.0.join("checkpoints");
    let original = identity();
    let mut store =
        CheckpointStore::initialize_with(&root, "campaign", original.clone(), publish).unwrap();
    store
        .accept(
            "producer/unit-0",
            &json!({"input": 1}),
            &json!({"status": "complete"}),
        )
        .unwrap();
    drop(store);
    let manifest_before = fs::read(root.join("manifest.json")).unwrap();
    let unit_path = fs::read_dir(root.join("units"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let unit_before = fs::read(&unit_path).unwrap();

    let mut changed = original;
    changed.source_sha256 = digest(b'8');
    assert!(CheckpointStore::resume(&root, "campaign", changed).is_err());
    let mut changed = identity();
    changed.source_sha256 = digest(b'8');
    assert!(CheckpointStore::initialize_with(&root, "campaign", changed, publish).is_err());
    assert_eq!(
        fs::read(root.join("manifest.json")).unwrap(),
        manifest_before
    );
    assert_eq!(fs::read(unit_path).unwrap(), unit_before);
}

#[test]
fn inspection_validates_without_recovering_or_creating_files() {
    let scratch = Scratch::new();
    let root = scratch.0.join("checkpoints");
    let mut store = CheckpointStore::create_new(&root, "campaign", identity()).unwrap();
    let unit = store
        .accept("unit", &json!({"input": 1}), &json!({"value": 42}))
        .unwrap();
    let before = fs::read(&unit.path).unwrap();
    drop(store);
    // Committed exports omit empty directories.
    fs::remove_dir(root.join("pending")).unwrap();
    let inspected = CheckpointStore::inspect(&root).unwrap();
    let (_, value): (serde_json::Value, serde_json::Value) = inspected.load("unit").unwrap();
    assert_eq!(value, json!({"value": 42}));
    assert!(!root.join("pending").exists());
    fs::create_dir(root.join("pending")).unwrap();
    let pending = root.join("pending/.interrupted.tmp");
    fs::write(&pending, b"partial evidence").unwrap();
    assert!(CheckpointStore::inspect(&root).is_err());
    assert_eq!(fs::read(&pending).unwrap(), b"partial evidence");
    assert_eq!(fs::read(&unit.path).unwrap(), before);
    fs::remove_file(pending).unwrap();
    // A parseable but noncanonical unit fails through the shared validator.
    let mut corrupt = before;
    corrupt.push(b' ');
    fs::write(&unit.path, &corrupt).unwrap();
    assert!(CheckpointStore::inspect(&root).is_err());
    assert!(CheckpointStore::resume(&root, "campaign", identity()).is_err());
    assert_eq!(fs::read(&unit.path).unwrap(), corrupt);
}
