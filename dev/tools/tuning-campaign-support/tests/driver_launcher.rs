//! Exercises launcher recovery using the real neutral discovery command.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tuning_campaign_support::campaign::{CanonicalJson, PreparationStore, SessionChannels, Token};

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn launcher_replays_preparation(complete_temporary: bool) {
    let root = std::env::temp_dir().join(format!(
        "gf2-launcher-discovery-{}-{complete_temporary}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let campaign = format!(
        "gf2-a83583e0-19700101T00000{}Z-{}{}",
        u8::from(complete_temporary),
        std::process::id(),
        u8::from(complete_temporary)
    );
    let stage = std::env::temp_dir().join(&campaign);
    fs::create_dir(&stage).unwrap();
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
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("scripts")).unwrap();
    executable(
        &repo.join("scripts/cargo-budget.sh"),
        "#!/bin/sh\necho forbidden-build > \"$TEST_BUILD_CAPTURE\"\nexit 91\n",
    );
    let path_bin = root.join("path-bin");
    fs::create_dir(&path_bin).unwrap();
    executable(
        &path_bin.join("git"),
        "#!/bin/sh\nprintf '%s\\n' \"$TEST_REPO\"\n",
    );
    fs::create_dir(stage.join("bin")).unwrap();
    executable(
        &stage.join("bin/driver"),
        "#!/usr/bin/env python3\nimport json,os,sys\nif sys.argv[1]=='discover-preparation': os.execv(os.environ['TEST_REAL_DRIVER'],[os.environ['TEST_REAL_DRIVER']]+sys.argv[1:])\nif sys.argv[1]=='prepare-session':\n    open(os.environ['TEST_PREPARE_CAPTURE'],'w').write(json.dumps(sys.argv[2:]))\n    sys.exit(73)\nsys.exit(92)\n",
    );
    let launcher = root.join("launcher.sh");
    fs::write(
        &launcher,
        include_str!("../../../scripts/tuning-extent-campaign.sh"),
    )
    .unwrap();
    let capture = root.join("prepare.json");
    let build_capture = root.join("build-called");
    let result = Command::new("bash")
        .arg(&launcher)
        .arg(&campaign)
        .env(
            "PATH",
            format!("{}:{}", path_bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("GF2_CCX1_LOCK", root.join("host.lock"))
        .env("TEST_REPO", &repo)
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
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(stage).unwrap();
}

#[test]
fn launcher_discovers_publisher_intent_before_selecting_identity_or_building() {
    launcher_replays_preparation(false);
}

#[test]
fn launcher_discovers_complete_publisher_temporary_before_selecting_identity_or_building() {
    launcher_replays_preparation(true);
}

#[test]
fn launcher_rejects_arbitrary_stage_paths_before_creating_them() {
    let root =
        std::env::temp_dir().join(format!("gf2-launcher-stage-policy-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let launcher = root.join("launcher.sh");
    fs::write(
        &launcher,
        include_str!("../../../scripts/tuning-extent-campaign.sh"),
    )
    .unwrap();
    let forbidden = root.join("repository-adjacent-stage");
    let result = Command::new("bash")
        .arg(&launcher)
        .arg(&forbidden)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(!forbidden.exists());
    fs::remove_dir_all(root).unwrap();
}
