//! Behavioral contracts of the experiment mechanics.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::thread;

use experiments_common::{
    create_durable, publish_atomic, ArtifactPath, DurabilityUnconfirmed, GitRevision,
    RepositoryRoot, RngAlgorithm, RngIdentity, RunRecord, Sha256Digest,
};

fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

fn checkout_with_one_commit() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "--quiet"]);
    commit(directory.path());
    directory
}

fn commit(directory: &Path) {
    git(
        directory,
        &["commit", "--quiet", "--allow-empty", "--message", "change"],
    );
}

fn rng() -> RngIdentity {
    RngIdentity {
        algorithm: RngAlgorithm::ChaCha20,
        version: "test implementation".to_owned(),
    }
}

fn observed(root: &RepositoryRoot, seed: u64) -> RunRecord {
    RunRecord::observe(root, Sha256Digest::of(b"configuration"), seed, rng()).unwrap()
}

fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn sha256_matches_the_fips_180_4_abc_vector_from_bytes_and_files() {
    let expected: Sha256Digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        .parse()
        .unwrap();
    assert_eq!(Sha256Digest::of(b"abc"), expected);

    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("abc");
    fs::write(&file, b"abc").unwrap();
    assert_eq!(Sha256Digest::of_file(&file).unwrap(), expected);
}

#[test]
fn hexadecimal_identities_accept_only_their_canonical_form() {
    assert!("a".repeat(64).parse::<Sha256Digest>().is_ok());
    assert!("a".repeat(40).parse::<GitRevision>().is_ok());
    for noncanonical in ["", "abc", &"A".repeat(64), &"g".repeat(64), &"a".repeat(65)] {
        assert!(
            noncanonical.parse::<Sha256Digest>().is_err(),
            "{noncanonical}"
        );
    }
    for noncanonical in ["", "abc1234", &"A".repeat(40), &"a".repeat(64)] {
        assert!(
            noncanonical.parse::<GitRevision>().is_err(),
            "{noncanonical}"
        );
    }
}

#[test]
fn identities_reject_noncanonical_text_when_deserialized() {
    let canonical = Sha256Digest::of(b"x");
    let text = serde_json::to_string(&canonical).unwrap();
    assert_eq!(
        serde_json::from_str::<Sha256Digest>(&text).unwrap(),
        canonical
    );
    assert!(serde_json::from_str::<Sha256Digest>(&text.to_uppercase()).is_err());
    assert!(serde_json::from_str::<ArtifactPath>("\"../escape\"").is_err());
}

#[test]
fn artifact_paths_are_normalized_and_repository_relative() {
    for path in ["file", "a/b.json", "..a/.b/c..", "ab:c/d"] {
        assert!(path.parse::<ArtifactPath>().is_ok(), "{path}");
    }
    for path in [
        "", "/a", "a/", "a//b", "./a", "a/./b", "../a", "a/..", "a\\b", "C:/a", "a\nb",
    ] {
        assert!(path.parse::<ArtifactPath>().is_err(), "{path:?}");
    }
}

#[test]
fn the_repository_root_is_independent_of_the_starting_depth() {
    let checkout = checkout_with_one_commit();
    let nested = checkout.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(checkout.path().join("input"), b"bytes").unwrap();

    let from_top = RepositoryRoot::containing(checkout.path()).unwrap();
    let from_depth = RepositoryRoot::containing(&nested).unwrap();
    assert_eq!(from_top, from_depth);

    let input: ArtifactPath = "input".parse().unwrap();
    assert_eq!(fs::read(from_depth.resolve(&input)).unwrap(), b"bytes");
    let identity = from_depth.artifact_identity(&input).unwrap();
    assert_eq!(identity.path, input);
    assert_eq!(identity.sha256, Sha256Digest::of(b"bytes"));
}

#[test]
fn a_new_commit_moves_the_source_pointer_and_preserves_the_identity() {
    let checkout = checkout_with_one_commit();
    let root = RepositoryRoot::containing(checkout.path()).unwrap();
    let before = observed(&root, 7);
    commit(checkout.path());
    let after = observed(&root, 7);
    assert_ne!(before.source_revision, after.source_revision);
    assert_eq!(before.identity, after.identity);
}

#[test]
fn each_caller_input_is_part_of_the_identity() {
    let checkout = checkout_with_one_commit();
    let root = RepositoryRoot::containing(checkout.path()).unwrap();
    let base = observed(&root, 7).identity;
    assert_ne!(base, observed(&root, 8).identity);

    let other_configuration =
        RunRecord::observe(&root, Sha256Digest::of(b"other"), 7, rng()).unwrap();
    assert_ne!(base, other_configuration.identity);

    let other_rng = RngIdentity {
        version: "other implementation".to_owned(),
        ..rng()
    };
    let other_rng =
        RunRecord::observe(&root, Sha256Digest::of(b"configuration"), 7, other_rng).unwrap();
    assert_ne!(base, other_rng.identity);
}

#[test]
fn a_run_identifies_its_own_executable_and_build() {
    let checkout = checkout_with_one_commit();
    let root = RepositoryRoot::containing(checkout.path()).unwrap();
    let identity = observed(&root, 7).identity;
    let executable = std::env::current_exe().unwrap();

    assert_eq!(
        identity.executable_sha256,
        Sha256Digest::of_file(&executable).unwrap()
    );
    assert!(
        executable
            .iter()
            .any(|component| component == identity.build.profile.as_str()),
        "{} ran from {}",
        identity.build.profile,
        executable.display()
    );
    assert!(identity.toolchain_version.starts_with("rustc "));
    assert!(identity.host.cpu.physical_cores > 0);
    assert!(identity.host.cpu.logical_threads >= identity.host.cpu.physical_cores);
}

#[test]
fn a_run_record_survives_serialization_and_refuses_unknown_fields() {
    let checkout = checkout_with_one_commit();
    let root = RepositoryRoot::containing(checkout.path()).unwrap();
    let record = observed(&root, u64::MAX);
    let mut value = serde_json::to_value(&record).unwrap();
    assert_eq!(
        serde_json::from_value::<RunRecord>(value.clone()).unwrap(),
        record
    );
    value["identity"]["unrecognized"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RunRecord>(value).is_err());
}

#[test]
fn publish_creates_parents_replaces_the_target_and_leaves_only_the_target() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("a/b");
    let target = parent.join("receipt.json");
    publish_atomic(&target, b"a longer first version").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"a longer first version");
    publish_atomic(&target, b"second").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"second");
    assert_eq!(entries(&parent), ["receipt.json"]);
}

#[test]
fn concurrent_publishes_leave_one_complete_version() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("state");
    let versions: Vec<Vec<u8>> = (0..8u8).map(|writer| vec![writer; 4096]).collect();
    thread::scope(|scope| {
        for version in &versions {
            let target = &target;
            scope.spawn(move || publish_atomic(target, version).unwrap());
        }
    });
    assert!(versions.contains(&fs::read(&target).unwrap()));
    assert_eq!(entries(directory.path()), ["state"]);
}

#[test]
fn a_failed_publish_keeps_the_target_and_removes_its_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let occupied = directory.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("kept"), b"kept").unwrap();

    assert!(publish_atomic(&occupied, b"bytes").is_err());
    assert_eq!(entries(directory.path()), ["occupied"]);
    assert_eq!(entries(&occupied), ["kept"]);

    assert_eq!(
        publish_atomic(Path::new(".."), b"bytes")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}

/// Makes `directory` searchable and writable but unreadable for `action`, or
/// skips it for root, which ignores the mode.
fn while_unreadable<T>(directory: &Path, action: impl FnOnce() -> T) -> Option<T> {
    fs::set_permissions(directory, fs::Permissions::from_mode(0o300)).unwrap();
    let result = fs::read_dir(directory).is_err().then(action);
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    result
}

#[test]
fn an_unreadable_parent_refuses_before_replacing_the_target() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("parent");
    let target = parent.join("state");
    publish_atomic(&target, b"first").unwrap();

    if let Some(result) = while_unreadable(&parent, || publish_atomic(&target, b"second")) {
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    assert_eq!(fs::read(&target).unwrap(), b"first");
    assert_eq!(entries(&parent), ["state"]);
}

#[test]
fn an_unreadable_intermediate_ancestor_refuses_a_new_chain() {
    let directory = tempfile::tempdir().unwrap();
    let ancestor = directory.path().join("a");
    fs::create_dir(&ancestor).unwrap();
    let target = ancestor.join("b/state");

    if let Some(result) = while_unreadable(&ancestor, || publish_atomic(&target, b"bytes")) {
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(!target.exists());
    }
}

#[test]
fn a_failure_before_the_rename_is_not_a_durability_failure() {
    let directory = tempfile::tempdir().unwrap();
    let occupied = directory.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    let error = publish_atomic(&occupied, b"bytes").unwrap_err();
    assert!(!error
        .get_ref()
        .is_some_and(|source| source.is::<DurabilityUnconfirmed>()));
}

#[test]
fn durable_creation_tolerates_racing_creators_and_existing_chains() {
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("a/b/c");
    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| create_durable(&nested).unwrap());
        }
    });
    assert!(nested.is_dir());
    create_durable(&nested).unwrap();
}

#[test]
fn durable_creation_refuses_a_file_in_the_chain() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file");
    fs::write(&file, b"bytes").unwrap();
    assert!(create_durable(&file).is_err());
    assert!(create_durable(&file.join("below")).is_err());
}

#[test]
fn concurrent_publishes_create_a_shared_new_chain() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("a/b");
    thread::scope(|scope| {
        for writer in 0..8 {
            let parent = &parent;
            scope.spawn(move || {
                publish_atomic(&parent.join(format!("state{writer}")), b"bytes").unwrap();
            });
        }
    });
    assert_eq!(entries(&parent).len(), 8);
}
