//! Drives the built composer through its command line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec};
use gf2_coding::tuning::{CodingSelectors, CodingTuning, CodingTuningCodec, DecodeSelectors};
use gf2_core::tuning::{
    AssemblyProvenance, CompiledProfileProvenance, CoreTuning, CoreTuningCodec, GitRevision,
    HarnessSchema, MeasurementProvenance, PreparedEnvelope, ProfileId, ProfileRegistry,
    ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, Sha256, TuningSection,
};

const PROFILE_ID: &str = "campaign";
const ASSEMBLED_AT: &str = "2026-08-25T19:00:00Z";
const SOURCE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const TOOL_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HARNESS: &str = "experiments/tuning-calibration/benches/coding.rs";
const PRODUCER_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// Outputs of the `core-owner`, `algebra-owner` and two-owner `complete`
/// modes for the arguments above: the stable external contract of those modes.
const GOLDEN_CORE_OWNER: &str = include_str!("golden/core-owner.json");
const GOLDEN_ALGEBRA_OWNER: &str = include_str!("golden/algebra-owner.json");
const GOLDEN_COMPLETE: &str = include_str!("golden/complete.json");

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "modes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compose(mode: &str, paths: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuning-profile-compose"))
        .arg(mode)
        .args(paths)
        .args([
            PROFILE_ID,
            ASSEMBLED_AT,
            SOURCE_REVISION,
            "false",
            TOOL_SHA256,
        ])
        .output()
        .unwrap()
}

fn composed(mode: &str, paths: &[&Path]) -> String {
    let output = compose(mode, paths);
    assert!(
        output.status.success(),
        "{mode} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(paths.last().unwrap()).unwrap()
}

#[test]
fn two_owner_modes_reproduce_their_pinned_outputs() {
    let scratch = Scratch::new();
    let core = scratch.file("core-owner.json");
    let algebra = scratch.file("algebra-owner.json");
    let complete = scratch.file("complete.json");

    assert_eq!(composed("core-owner", &[&core]), GOLDEN_CORE_OWNER);
    assert_eq!(composed("algebra-owner", &[&algebra]), GOLDEN_ALGEBRA_OWNER);
    assert_eq!(
        composed("complete", &[&core, &algebra, &complete]),
        GOLDEN_COMPLETE
    );
}

fn coding_registry() -> ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn assembly(assembled_at: &str, tool: &str) -> AssemblyProvenance {
    AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse(assembled_at).unwrap(),
        source_revision: GitRevision::parse(SOURCE_REVISION).unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse(tool).unwrap(),
        tool_sha256: Sha256::parse(TOOL_SHA256).unwrap(),
    }
}

fn calibrated_section() -> CodingTuning {
    CodingTuning::from_selectors(CodingSelectors {
        decode: DecodeSelectors::try_new(4, 64_800, 1, 2).unwrap(),
        ..CodingSelectors::CONSERVATIVE
    })
}

fn calibration() -> MeasurementProvenance {
    MeasurementProvenance::Calibrated {
        measured_at: Rfc3339Utc::parse("2026-08-25T18:00:00Z").unwrap(),
        source_revision: GitRevision::parse(SOURCE_REVISION).unwrap(),
        source_dirty: false,
        harness: RepoRelPath::parse(HARNESS).unwrap(),
        harness_schema: HarnessSchema::parse(CodingTuningCodec::HARNESS_SCHEMA).unwrap(),
        binary_sha256: Sha256::parse(PRODUCER_SHA256).unwrap(),
        toolchain: "rustc".to_owned(),
        host: "host".to_owned(),
        cpu_model: "cpu".to_owned(),
        cpu_features: vec!["avx2".to_owned()],
        os_kernel: "kernel".to_owned(),
        governor: "performance".to_owned(),
        receipt: RepoRelPath::parse("dev/benchmarks/tuning_profiles/receipt.md").unwrap(),
    }
}

/// The owner envelope a coding producer publishes: its section under its own
/// profile ID and assembly.
fn producer_envelope(section: CodingTuning, measurement: MeasurementProvenance) -> String {
    let id = ProfileId::parse("producer-campaign").unwrap();
    let prepared =
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert_measured::<CodingTuning, CodingTuningCodec>(section, measurement)
            .unwrap()
            .build()
            .unwrap();
    coding_registry()
        .to_json(&prepared, &assembly("2026-08-25T18:30:00Z", HARNESS))
        .unwrap()
}

fn written(scratch: &Scratch, name: &str, text: &str) -> PathBuf {
    let file = scratch.file(name);
    fs::write(&file, text).unwrap();
    file
}

#[test]
fn coding_owner_mode_reassembles_a_calibrated_section_under_the_callers_identity() {
    let scratch = Scratch::new();
    let calibrated = written(
        &scratch,
        "calibrated.json",
        &producer_envelope(calibrated_section(), calibration()),
    );
    let owner = scratch.file("coding-owner.json");

    let text = composed("coding-owner", &[&calibrated, &owner]);

    let registry = coding_registry();
    let reopened = registry.from_json(&text).unwrap();
    assert_eq!(
        reopened.section_ids().collect::<Vec<_>>(),
        [CodingTuning::ID.as_str()]
    );
    assert_eq!(reopened.profile_id().as_str(), PROFILE_ID);
    let composer_assembly = assembly(ASSEMBLED_AT, "dev/tools/tuning-profile-compose");
    assert_eq!(
        reopened.verified_assembly().unwrap().provenance,
        composer_assembly
    );
    let projection = reopened.section::<CodingTuning>().unwrap().unwrap();
    assert_eq!(projection.section, &calibrated_section());
    assert_eq!(projection.measurement, &calibration());
    assert_eq!(
        registry.to_json(&reopened, &composer_assembly).unwrap(),
        text
    );
}

#[test]
fn coding_owner_mode_rejects_an_inherited_section_and_publishes_nothing() {
    let scratch = Scratch::new();
    let inherited = written(
        &scratch,
        "inherited.json",
        &producer_envelope(CodingTuning::CONSERVATIVE, MeasurementProvenance::Inherited),
    );
    let owner = scratch.file("coding-owner.json");

    let output = compose("coding-owner", &[&inherited, &owner]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a calibrated one"));
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn complete_mode_joins_the_coding_owner_as_a_third_section() {
    let scratch = Scratch::new();
    let calibrated = written(
        &scratch,
        "calibrated.json",
        &producer_envelope(calibrated_section(), calibration()),
    );
    let core = scratch.file("core-owner.json");
    let algebra = scratch.file("algebra-owner.json");
    let coding = scratch.file("coding-owner.json");
    let complete = scratch.file("complete.json");
    composed("core-owner", &[&core]);
    composed("algebra-owner", &[&algebra]);
    composed("coding-owner", &[&calibrated, &coding]);

    let text = composed("complete", &[&core, &algebra, &coding, &complete]);

    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .unwrap()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
    let reopened = registry.from_json(&text).unwrap();
    let mut ids = [
        CoreTuning::ID.as_str(),
        AlgebraTuning::ID.as_str(),
        CodingTuning::ID.as_str(),
    ];
    ids.sort_unstable();
    assert_eq!(reopened.section_ids().collect::<Vec<_>>(), ids);
    assert_eq!(reopened.profile_id().as_str(), PROFILE_ID);
    let composer_assembly = assembly(ASSEMBLED_AT, "dev/tools/tuning-profile-compose");
    assert_eq!(
        registry.to_json(&reopened, &composer_assembly).unwrap(),
        text
    );

    let core_section = reopened.section::<CoreTuning>().unwrap().unwrap();
    assert_eq!(core_section.section, &CoreTuning::CONSERVATIVE);
    assert_eq!(core_section.measurement, &MeasurementProvenance::Inherited);
    let algebra_section = reopened.section::<AlgebraTuning>().unwrap().unwrap();
    assert_eq!(algebra_section.section, &AlgebraTuning::CONSERVATIVE);
    assert_eq!(
        algebra_section.measurement,
        &MeasurementProvenance::Inherited
    );
    let coding_section = reopened.section::<CodingTuning>().unwrap().unwrap();
    assert_eq!(coding_section.section, &calibrated_section());
    assert_eq!(coding_section.measurement, &calibration());

    assert_eq!(gf2_core::tuning::install(reopened), Ok(()));
    assert_eq!(gf2_coding::tuning::active().section, &calibrated_section());
}
