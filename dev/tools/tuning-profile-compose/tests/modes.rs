//! Drives the built composer through its command line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec};
use gf2_coding::tuning::{
    CodingSelectors, CodingTuning, CodingTuningCodec, DecodeSelectors, QcRouteBounds,
};
use gf2_core::tuning::{
    AssemblyProvenance, CanonicalValue, CompiledProfileProvenance, CoreTuning, CoreTuningCodec,
    GitRevision, HarnessSchema, MeasurementProvenance, PreparedEnvelope, ProfileId,
    ProfileRegistry, ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, SectionCodec, Sha256,
    TuningSection,
};

const PROFILE_ID: &str = "campaign";
const ASSEMBLED_AT: &str = "2026-08-25T19:00:00Z";
const SOURCE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const TOOL_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HARNESS: &str = "experiments/tuning-calibration/benches/coding.rs";
const PRODUCER_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// Outputs of the `core-owner`, `algebra-owner` and two-owner `complete`
/// modes, and of `coding-owner` with the QC route bounds fixed on
/// [`partial_producer_envelope`], for the arguments above: the stable external
/// contract of those modes.
const GOLDEN_CORE_OWNER: &str = include_str!("golden/core-owner.json");
const GOLDEN_ALGEBRA_OWNER: &str = include_str!("golden/algebra-owner.json");
const GOLDEN_COMPLETE: &str = include_str!("golden/complete.json");
const GOLDEN_CODING_OWNER_QC: &str = include_str!("golden/coding-owner-qc-route.json");

/// QC route bounds as the fixed-selector input states them.
const QC_ROUTE: &str = r#"{"lane_claim_min_frames":4,"max_workers":12,"min_lifting_size":64}"#;

fn qc_route() -> QcRouteBounds {
    QcRouteBounds::try_new(64, 12, 4).unwrap()
}

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
        receipt: RepoRelPath::parse("target/benchmarks/tuning_profiles/receipt.md").unwrap(),
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

/// A calibrated producer envelope whose section states two decode selectors
/// and omits every other selector, as the decoder calibration campaign does.
fn partial_producer_envelope() -> (String, CodingTuning) {
    let body: CanonicalValue =
        serde_json::from_str(r#"{"decode": {"lane_max_edges": 226799, "lane_min_frames": 2}}"#)
            .unwrap();
    let section = CodingTuningCodec::decode_body(body).unwrap();
    (producer_envelope(section.clone(), calibration()), section)
}

fn compose_fixed(paths: [&Path; 2], selector: &str, value: &str) -> Output {
    let [calibrated, output] = paths;
    Command::new(env!("CARGO_BIN_EXE_tuning-profile-compose"))
        .arg("coding-owner")
        .arg(calibrated)
        .args(["--fixed-selector", selector, value])
        .arg(output)
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

/// The coding section's wrapper in `envelope` with `decode.qc_route`
/// removed, and the family removed when nothing else remains in it.
fn wrapper_without_qc_route(envelope: &str) -> serde_json::Value {
    let document: serde_json::Value = serde_json::from_str(envelope).unwrap();
    let mut wrapper = document["sections"][CodingTuning::ID.as_str()].clone();
    let selectors = wrapper["selectors"].as_object_mut().unwrap();
    if let Some(decode) = selectors.get_mut("decode").and_then(|d| d.as_object_mut()) {
        decode.remove("qc_route");
        if decode.is_empty() {
            selectors.remove("decode");
        }
    }
    wrapper
}

#[test]
fn a_fixed_qc_route_selector_joins_the_calibrated_selectors_and_provenance_unchanged() {
    let scratch = Scratch::new();
    let (producer, section) = partial_producer_envelope();
    let calibrated = written(&scratch, "calibrated.json", &producer);
    let owner = scratch.file("coding-owner.json");

    let output = compose_fixed([&calibrated, &owner], "decode.qc_route", QC_ROUTE);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(&owner).unwrap();
    assert_eq!(text, GOLDEN_CODING_OWNER_QC);

    let registry = coding_registry();
    let reopened = registry.from_json(&text).unwrap();
    assert_eq!(reopened.profile_id().as_str(), PROFILE_ID);
    let composer_assembly = assembly(ASSEMBLED_AT, "dev/tools/tuning-profile-compose");
    assert_eq!(
        reopened.verified_assembly().unwrap().provenance,
        composer_assembly
    );
    assert_eq!(
        registry.to_json(&reopened, &composer_assembly).unwrap(),
        text
    );

    let projection = reopened.section::<CodingTuning>().unwrap().unwrap();
    assert_eq!(projection.measurement, &calibration());
    assert_eq!(projection.section.decode().qc_route(), qc_route());
    assert_eq!(projection.section, &section.with_qc_route(qc_route()));
    // Each other selector keeps its statement or omission, and the
    // measurement its bytes.
    assert_eq!(
        wrapper_without_qc_route(&text),
        wrapper_without_qc_route(&producer)
    );
    let document: serde_json::Value = serde_json::from_str(&text).unwrap();
    let bounds: serde_json::Value = serde_json::from_str(QC_ROUTE).unwrap();
    assert_eq!(
        document["sections"][CodingTuning::ID.as_str()]["selectors"]["decode"]["qc_route"],
        bounds
    );
}

#[test]
fn coding_owner_mode_refuses_a_fixed_selector_at_another_path() {
    let scratch = Scratch::new();
    let calibrated = written(&scratch, "calibrated.json", &partial_producer_envelope().0);
    let owner = scratch.file("coding-owner.json");

    for path in [
        "decode.lane_min_frames",
        "decode.qc_route_cases",
        "encode.qc_route",
        "qc_route",
    ] {
        let output = compose_fixed([&calibrated, &owner], path, "4");
        assert!(!output.status.success(), "{path}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("is no fixed selector"),
            "{path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn coding_owner_mode_refuses_invalid_qc_route_bounds() {
    let scratch = Scratch::new();
    let calibrated = written(&scratch, "calibrated.json", &partial_producer_envelope().0);
    let owner = scratch.file("coding-owner.json");

    for value in [
        r#"{"lane_claim_min_frames":4,"max_workers":0,"min_lifting_size":64}"#,
        r#"{"lane_claim_min_frames":4,"max_workers":12}"#,
        r#"{"lane_claim_min_frames":4,"max_workers":12,"min_lifting_size":64,"extra":1}"#,
        r#"{"lane_claim_min_frames":-1,"max_workers":12,"min_lifting_size":64}"#,
        r#"[{"lane_claim_min_frames":4,"max_workers":12,"min_lifting_size":64}]"#,
        r#"{"lane_claim_min_frames":4,"max_workers":12,"min_lifting_size":64"#,
        "null",
    ] {
        let output = compose_fixed([&calibrated, &owner], "decode.qc_route", value);
        assert!(!output.status.success(), "{value}");
    }
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}
