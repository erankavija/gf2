//! Exercises launcher recovery using the real neutral discovery command.
//!
//! `dev/scripts/tuning-extent-campaign.sh` resolves a campaign's stage as
//! literally `/tmp/<campaign-id>` and the driver refuses any other path, so
//! these tests stage under the real `/tmp` rather than `std::env::temp_dir()`.
//! [`ScratchPath`] removes the stage when the test ends, whichever way it
//! ends. Everything else the tests write lives under `temp_dir()`.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tuning_campaign_support::campaign::{
    locate_campaign_declaration, CanonicalJson, PreparationStore, SessionChannels, Token,
    DECLARATION_FILE,
};
use tuning_campaign_support::repository::repository_root;
use tuning_campaign_support::scratch::{scratch, ScratchPath};

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {arguments:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

/// Copies the committed campaign launcher to `target`.
fn install_launcher(target: &Path) {
    let source = repository_root()
        .unwrap()
        .join("dev/scripts/tuning-extent-campaign.sh");
    fs::copy(source, target).unwrap();
}

/// An empty git checkout below `root` in which the launcher runs.
fn checkout(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("scripts")).unwrap();
    git(&repo, &["init", "-q"]);
    repo
}

/// Copies the committed campaign declaration of `issue` into the stand-in
/// checkout under `directory`, away from its committed location; the launcher
/// locates it by its issue before selecting an identity.
fn declare(repo: &Path, issue: &str, directory: &str) {
    let committed = repository_root().unwrap();
    let source = committed.join(locate_campaign_declaration(&committed, issue).unwrap());
    let target = repo.join(directory).join(DECLARATION_FILE);
    assert_ne!(
        source.parent().unwrap().strip_prefix(&committed).unwrap(),
        Path::new(directory)
    );
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::copy(source, target).unwrap();
}

fn write_declaration(repo: &Path, directory: &str, content: &str) {
    fs::create_dir_all(repo.join(directory)).unwrap();
    fs::write(repo.join(directory).join(DECLARATION_FILE), content).unwrap();
}

const STAND_IN_DECLARATION: &str =
    "{\"schema\":\"tuning-campaign-declaration-v1\",\"issue\":\"0badbeef\"}";

#[test]
fn byte_identical_declaration_copies_are_one_declaration() {
    let root = scratch("gf2-declaration-copies");
    let repo = checkout(&root);
    for directory in ["live/x", "copies/of/x", "copies/again"] {
        write_declaration(&repo, directory, STAND_IN_DECLARATION);
    }
    let located = locate_campaign_declaration(&repo, "0badbeef").unwrap();
    assert_eq!(located, format!("copies/again/{DECLARATION_FILE}"));
    assert_eq!(
        fs::read_to_string(repo.join(located)).unwrap(),
        STAND_IN_DECLARATION
    );
}

#[test]
fn declarations_of_one_issue_with_different_bytes_are_rejected() {
    let root = scratch("gf2-declaration-conflict");
    let repo = checkout(&root);
    write_declaration(&repo, "first", STAND_IN_DECLARATION);
    write_declaration(&repo, "copy", STAND_IN_DECLARATION);
    write_declaration(&repo, "second", &format!("{STAND_IN_DECLARATION}\n"));
    let error = locate_campaign_declaration(&repo, "0badbeef").unwrap_err();
    assert_eq!(
        error.to_string(),
        "several campaign declarations name issue 0badbeef"
    );
}

fn launcher_replays_preparation(complete_temporary: bool) {
    let root = scratch("gf2-launcher-discovery");
    let campaign = format!(
        "gf2-a83583e0-19700101t00000{}z-{}{}",
        u8::from(complete_temporary),
        std::process::id(),
        u8::from(complete_temporary)
    );
    let stage = ScratchPath::create(Path::new("/tmp"), &campaign);
    let session = "original-session";
    drop(
        PreparationStore::begin(
            SessionChannels::for_stage(&stage).unwrap(),
            Token::new(&campaign).unwrap(),
            Token::new(session).unwrap(),
            root.join("host.lock"),
            CanonicalJson::new("{}".to_owned()).unwrap(),
        )
        .unwrap(),
    );
    let canonical_intent = stage.join("preparations").join(session).join("intent.json");
    let original = fs::read(&canonical_intent).unwrap();
    // Retain only the publisher's durable initiation evidence, matching a
    // crash before the preparation intent or active anchor was published.
    fs::remove_file(&canonical_intent).unwrap();
    fs::remove_file(stage.join("active-preparation.json")).unwrap();
    if complete_temporary {
        let publisher = fs::read_dir(stage.join("artifact-publications"))
            .unwrap()
            .map(|entry| entry.unwrap().path().join("intent.json"))
            .find(|path| {
                let value: serde_json::Value =
                    serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
                value["target"] == canonical_intent.to_str().unwrap()
            })
            .unwrap();
        fs::rename(
            &publisher,
            publisher.with_file_name(".intent.json.tmp-1-2-0"),
        )
        .unwrap();
    }
    let repo = checkout(&root);
    declare(&repo, "a83583e0", "relocated/extent");
    declare(&repo, "a83583e0", "relocated/copy");
    executable(
        &repo.join("scripts/cargo-budget.sh"),
        "#!/bin/sh\necho forbidden-build > \"$TEST_BUILD_CAPTURE\"\nexit 91\n",
    );
    fs::create_dir(stage.join("bin")).unwrap();
    executable(
        &stage.join("bin/driver"),
        "#!/usr/bin/env python3\nimport json,os,sys\nif sys.argv[1]=='discover-preparation': os.execv(os.environ['TEST_REAL_DRIVER'],[os.environ['TEST_REAL_DRIVER']]+sys.argv[1:])\nif sys.argv[1]=='prepare-session':\n    open(os.environ['TEST_PREPARE_CAPTURE'],'w').write(json.dumps(sys.argv[2:]))\n    sys.exit(73)\nsys.exit(92)\n",
    );
    let launcher = root.join("launcher.sh");
    install_launcher(&launcher);
    let capture = root.join("prepare.json");
    let build_capture = root.join("build-called");
    let result = Command::new("bash")
        .arg(&launcher)
        .arg(&campaign)
        .current_dir(&repo)
        .env("GF2_CCX1_LOCK", root.join("host.lock"))
        .env(
            "TEST_REAL_DRIVER",
            env!("CARGO_BIN_EXE_tuning-extent-campaign-driver"),
        )
        .env("TEST_PREPARE_CAPTURE", &capture)
        .env("TEST_BUILD_CAPTURE", &build_capture)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let arguments: Vec<String> = serde_json::from_slice(&fs::read(capture).unwrap()).unwrap();
    assert_eq!(&arguments[1..3], &[campaign.as_str(), session]);
    assert!(!build_capture.exists());
    assert_eq!(fs::read(&canonical_intent).unwrap(), original);
    assert!(!stage.join("execution.log").exists());
    assert_eq!(fs::read_dir(stage.join("preparations")).unwrap().count(), 1);
}

#[test]
fn launcher_discovers_publisher_intent_before_selecting_identity_or_building() {
    launcher_replays_preparation(false);
}

#[test]
fn launcher_discovers_complete_publisher_temporary_before_selecting_identity_or_building() {
    launcher_replays_preparation(true);
}

/// A new campaign names its issue; the launcher refuses an issue without
/// exactly one declaration content before it creates a stage or builds
/// anything.
#[test]
fn launcher_requires_the_named_issue_declaration_before_creating_a_stage() {
    let root = scratch("gf2-launcher-declaration");
    let launcher = root.join("launcher.sh");
    install_launcher(&launcher);
    let repo = checkout(&root);
    write_declaration(&repo, "first", STAND_IN_DECLARATION);
    write_declaration(&repo, "second", &format!("{STAND_IN_DECLARATION}\n"));
    let build_capture = root.join("build-called");
    executable(
        &repo.join("scripts/cargo-budget.sh"),
        "#!/bin/sh\necho forbidden-build > \"$TEST_BUILD_CAPTURE\"\nexit 91\n",
    );
    for argument in [
        "0badc0de",
        "gf2-0badc0de-19700101t000000z-1",
        "0badbeef",
        "gf2-0badbeef-19700101t000000z-1",
    ] {
        let result = Command::new("bash")
            .arg(&launcher)
            .arg(argument)
            .current_dir(&repo)
            .env("GF2_CCX1_LOCK", root.join("host.lock"))
            .env("TEST_BUILD_CAPTURE", &build_capture)
            .output()
            .unwrap();
        assert!(!result.status.success(), "{argument}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("campaign declarations name issue"),
            "{argument}"
        );
        assert!(!build_capture.exists());
        assert!(!Path::new("/tmp/gf2-0badc0de-19700101t000000z-1").exists());
        assert!(!Path::new("/tmp/gf2-0badbeef-19700101t000000z-1").exists());
    }
    for argument in ["", "DBD8787D", "gf2-dbd8787d-19700101T000000Z-1", "a b"] {
        let result = Command::new("bash")
            .arg(&launcher)
            .arg(argument)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{argument:?}");
    }
}

#[test]
fn launcher_rejects_arbitrary_stage_paths_before_creating_them() {
    let root = scratch("gf2-launcher-stage-policy");
    let launcher = root.join("launcher.sh");
    install_launcher(&launcher);
    let forbidden = root.join("repository-adjacent-stage");
    let result = Command::new("bash")
        .arg(&launcher)
        .arg(&forbidden)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(!forbidden.exists());
}

/// Relaunching an existing stage first asks the driver to publish it: a
/// complete campaign (exit 0) or a publication failure ends the launch, and
/// only an incomplete campaign (exit 3) proceeds to session preparation.
#[test]
fn launcher_resumes_publication_before_preparing_another_session() {
    for (publish_exit, launcher_exit, prepared) in [(0, 0, false), (1, 1, false), (3, 73, true)] {
        let root = scratch("gf2-launcher-publication");
        let campaign = format!(
            "gf2-a83583e0-19700101t00000{publish_exit}z-{}",
            std::process::id()
        );
        let stage = ScratchPath::create(Path::new("/tmp"), &campaign);
        fs::write(
            stage.join("campaign.json"),
            format!("{{\"campaign_id\":\"{campaign}\"}}"),
        )
        .unwrap();
        fs::create_dir(stage.join("bin")).unwrap();
        executable(
            &stage.join("bin/driver"),
            "#!/usr/bin/env python3\nimport os,sys\nif sys.argv[1]=='publish-campaign':\n    open(os.environ['TEST_PUBLISH_CAPTURE'],'w').write(sys.argv[2])\n    sys.exit(int(os.environ['TEST_PUBLISH_EXIT']))\nif sys.argv[1]=='discover-preparation':\n    print('null'); sys.exit(0)\nif sys.argv[1]=='prepare-session':\n    open(os.environ['TEST_PREPARE_CAPTURE'],'w').write('prepared')\n    sys.exit(73)\nsys.exit(92)\n",
        );
        let repo = checkout(&root);
        declare(&repo, "a83583e0", "relocated/extent");
        let launcher = root.join("launcher.sh");
        install_launcher(&launcher);
        let publish_capture = root.join("published");
        let prepare_capture = root.join("prepared");
        let result = Command::new("bash")
            .arg(&launcher)
            .arg(&campaign)
            .current_dir(&repo)
            .env("GF2_CCX1_LOCK", root.join("host.lock"))
            .env("TEST_PUBLISH_EXIT", publish_exit.to_string())
            .env("TEST_PUBLISH_CAPTURE", &publish_capture)
            .env("TEST_PREPARE_CAPTURE", &prepare_capture)
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(launcher_exit),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(&publish_capture).unwrap(),
            stage.to_str().unwrap()
        );
        assert_eq!(prepare_capture.exists(), prepared);
    }
}
