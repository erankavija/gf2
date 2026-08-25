#![cfg(feature = "tuning-profile")]

use gf2_core::tuning::{
    AssemblyProvenance, CanonicalValue, CompiledProfileProvenance, CoreTuning, CoreTuningCodec,
    GitRevision, HarnessSchema, MeasurementProvenance, PreparedEnvelope, ProfileError, ProfileId,
    ProfileRegistryBuilder, RegistryError, RepoRelPath, Rfc3339Utc, SectionCodec, SectionError,
    SectionId, Sha256, TuningSection, PROFILE_FORMAT_VERSION,
};
use sha2::{Digest, Sha256 as Sha256Hasher};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct Alpha {
    value: u64,
}

static ALPHA_CONSERVATIVE: Alpha = Alpha { value: 7 };

impl TuningSection for Alpha {
    const ID: SectionId = SectionId::from_static("test/alpha");
    type Selectors = u64;

    fn conservative() -> &'static Self {
        &ALPHA_CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AlphaAlias(u64);

static ALPHA_ALIAS_CONSERVATIVE: AlphaAlias = AlphaAlias(31);

impl TuningSection for AlphaAlias {
    const ID: SectionId = Alpha::ID;
    type Selectors = u64;

    fn conservative() -> &'static Self {
        &ALPHA_ALIAS_CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.0
    }
}

struct AlphaAliasCodec;

impl SectionCodec<AlphaAlias> for AlphaAliasCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(value: &MeasurementProvenance) -> Result<(), SectionError> {
        match value {
            MeasurementProvenance::Inherited => Ok(()),
            MeasurementProvenance::Calibrated { .. } => {
                Err(SectionError::unsupported_harness("test-alpha-alias-v1"))
            }
        }
    }

    fn decode_body(body: CanonicalValue) -> Result<AlphaAlias, SectionError> {
        body.deserialize().map(AlphaAlias)
    }

    fn encode_body(section: &AlphaAlias) -> Result<CanonicalValue, SectionError> {
        CanonicalValue::serialize(&section.0)
    }
}

struct AlphaCodec;

impl SectionCodec<Alpha> for AlphaCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(value: &MeasurementProvenance) -> Result<(), SectionError> {
        match value {
            MeasurementProvenance::Inherited => Ok(()),
            MeasurementProvenance::Calibrated { .. } => {
                Err(SectionError::unsupported_harness("test-alpha-v1"))
            }
        }
    }

    fn decode_body(body: CanonicalValue) -> Result<Alpha, SectionError> {
        body.deserialize()
    }

    fn encode_body(section: &Alpha) -> Result<CanonicalValue, SectionError> {
        CanonicalValue::serialize(section)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct Beta {
    value: u64,
}

static BETA_CONSERVATIVE: Beta = Beta { value: 11 };

impl TuningSection for Beta {
    const ID: SectionId = SectionId::from_static("test/beta");
    type Selectors = u64;

    fn conservative() -> &'static Self {
        &BETA_CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.value
    }
}

struct BetaCodec;

impl SectionCodec<Beta> for BetaCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(value: &MeasurementProvenance) -> Result<(), SectionError> {
        match value {
            MeasurementProvenance::Inherited => Ok(()),
            MeasurementProvenance::Calibrated { .. } => {
                Err(SectionError::unsupported_harness("test-beta-v1"))
            }
        }
    }

    fn decode_body(body: CanonicalValue) -> Result<Beta, SectionError> {
        body.deserialize()
    }

    fn encode_body(section: &Beta) -> Result<CanonicalValue, SectionError> {
        CanonicalValue::serialize(section)
    }
}

fn profile_id() -> ProfileId {
    ProfileId::parse("format-two-test").unwrap()
}

fn compiled_provenance() -> CompiledProfileProvenance {
    CompiledProfileProvenance {
        artifact_id: profile_id(),
    }
}

fn assembly() -> AssemblyProvenance {
    AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("2026-08-25T19:00:00Z").unwrap(),
        source_revision: gf2_core::tuning::GitRevision::parse(
            "0123456789abcdef0123456789abcdef01234567",
        )
        .unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-core/tests/tuning_envelope_v2.rs").unwrap(),
        tool_sha256: Sha256::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
    }
}

fn recompute_content_digest(json: &str) -> String {
    let mut document = json.trim_end_matches('\n').to_owned();
    let assembly = document
        .find(",\"assembly\":")
        .expect("the registry emits assembly before sections");
    let sections = document
        .find(",\"sections\":")
        .expect("the registry emits a sections object");
    let content = format!("{},{}", &document[..assembly], &document[sections + 1..]);
    let digest = format!("{:x}", Sha256Hasher::digest(content.as_bytes()));
    let digest_marker = "\"content_sha256\":\"";
    let digest_start = document
        .find(digest_marker)
        .expect("the registry emits an assembly digest")
        + digest_marker.len();
    document.replace_range(digest_start..digest_start + 64, &digest);
    document.push('\n');
    document
}

fn calibrated_core_measurement() -> MeasurementProvenance {
    MeasurementProvenance::Calibrated {
        measured_at: Rfc3339Utc::parse("2026-08-25T18:00:00Z").unwrap(),
        source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap(),
        source_dirty: false,
        harness: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs").unwrap(),
        harness_schema: HarnessSchema::parse(CoreTuningCodec::HARNESS_SCHEMA).unwrap(),
        binary_sha256: Sha256::parse(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap(),
        toolchain: "rustc 1.95.0".to_owned(),
        host: "test-host".to_owned(),
        cpu_model: "test-cpu".to_owned(),
        cpu_features: vec!["avx2".to_owned()],
        os_kernel: "Linux 6.test".to_owned(),
        governor: "performance".to_owned(),
        receipt: RepoRelPath::parse("dev/benchmarks/tuning_profiles/test.md").unwrap(),
    }
}

#[test]
fn registration_and_insertion_order_do_not_change_canonical_bytes() {
    let alpha_beta = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .register::<Beta, BetaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let beta_alpha = ProfileRegistryBuilder::new()
        .register::<Beta, BetaCodec>()
        .unwrap()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();

    let first = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Beta { value: 13 })
        .unwrap()
        .insert(Alpha { value: 17 })
        .unwrap()
        .build()
        .unwrap();
    let second = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 17 })
        .unwrap()
        .insert(Beta { value: 13 })
        .unwrap()
        .build()
        .unwrap();

    let first_json = alpha_beta.to_json(&first, &assembly()).unwrap();
    let second_json = beta_alpha.to_json(&second, &assembly()).unwrap();
    assert_eq!(first_json, second_json);
    assert!(first_json.ends_with('\n'));
    assert!(first_json.starts_with(&format!(
        "{{\"profile_format_version\":{PROFILE_FORMAT_VERSION},\"profile_id\":"
    )));

    let reopened = alpha_beta.from_json(&first_json).unwrap();
    assert_eq!(
        reopened.section::<Alpha>().unwrap().unwrap().section.value,
        17
    );
    assert_eq!(
        reopened.section::<Beta>().unwrap().unwrap().section.value,
        13
    );
}

#[test]
fn present_sections_are_strict_and_registered_but_absent_sections_are_valid() {
    let complete_registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .register::<Beta, BetaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let alpha_registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();

    let owner = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 19 })
        .unwrap()
        .build()
        .unwrap();
    let owner_json = alpha_registry.to_json(&owner, &assembly()).unwrap();
    assert_eq!(
        complete_registry.to_json(&owner, &assembly()).unwrap(),
        owner_json,
        "an extra registered codec does not emit an absent section"
    );
    let reopened = complete_registry.from_json(&owner_json).unwrap();
    assert!(reopened.section::<Beta>().unwrap().is_none());

    let complete = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 19 })
        .unwrap()
        .insert(Beta { value: 23 })
        .unwrap()
        .build()
        .unwrap();
    let complete_json = complete_registry.to_json(&complete, &assembly()).unwrap();
    assert!(matches!(
        alpha_registry.from_json(&complete_json),
        Err(ProfileError::UnexpectedSection { .. })
    ));
}

#[test]
fn unsupported_envelope_versions_duplicate_keys_and_floats_are_rejected() {
    let registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 29 })
        .unwrap()
        .build()
        .unwrap();
    let json = registry.to_json(&prepared, &assembly()).unwrap();

    for version in [1, 3, u32::MAX] {
        let changed = json.replacen(
            &format!("\"profile_format_version\":{PROFILE_FORMAT_VERSION}"),
            &format!("\"profile_format_version\":{version}"),
            1,
        );
        assert!(matches!(
            registry.from_json(&changed),
            Err(ProfileError::UnsupportedProfileFormatVersion { found, .. }) if found == version
        ));
    }

    let duplicate = json.replacen(
        "\"profile_id\":",
        "\"profile_id\":\"duplicate\",\"profile_id\":",
        1,
    );
    assert!(matches!(
        registry.from_json(&duplicate),
        Err(ProfileError::Malformed)
    ));

    let float = json.replacen("\"value\":29", "\"value\":29.0", 1);
    assert!(matches!(
        registry.from_json(&float),
        Err(ProfileError::Malformed)
    ));
}

#[test]
fn section_versions_are_checked_after_a_valid_content_digest() {
    let registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 29 })
        .unwrap()
        .build()
        .unwrap();
    let json = registry.to_json(&prepared, &assembly()).unwrap();

    for version in [0, 2, u32::MAX] {
        let changed = json.replacen(
            &format!("\"schema_version\":{}", AlphaCodec::SCHEMA_VERSION),
            &format!("\"schema_version\":{version}"),
            1,
        );
        let changed = recompute_content_digest(&changed);
        assert!(matches!(
            registry.from_json(&changed),
            Err(ProfileError::UnsupportedSectionSchemaVersion {
                id,
                found,
                supported: AlphaCodec::SCHEMA_VERSION,
            }) if id == Alpha::ID.as_str() && found == version
        ));
    }
}

#[test]
fn selector_changes_without_reassembly_fail_the_content_digest() {
    let registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 29 })
        .unwrap()
        .build()
        .unwrap();
    let json = registry.to_json(&prepared, &assembly()).unwrap();
    let changed = json.replacen("\"value\":29", "\"value\":30", 1);

    assert!(matches!(
        registry.from_json(&changed),
        Err(ProfileError::ContentDigestMismatch { .. })
    ));
}

#[test]
fn calibrated_core_measurement_rejects_an_unknown_harness_token() {
    let registry = core_registry();
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert_measured::<CoreTuning, CoreTuningCodec>(
            CoreTuning::CONSERVATIVE,
            calibrated_core_measurement(),
        )
        .unwrap()
        .build()
        .unwrap();
    let json = registry.to_json(&prepared, &assembly()).unwrap();
    let unknown = "unknown-core-harness-v9";
    let changed = json.replacen(CoreTuningCodec::HARNESS_SCHEMA, unknown, 1);
    let changed = recompute_content_digest(&changed);

    assert!(matches!(
        registry.from_json(&changed),
        Err(ProfileError::UnsupportedHarnessSchema {
            id,
            found,
            supported: CoreTuningCodec::HARNESS_SCHEMA,
        }) if id == CoreTuning::ID.as_str() && found == unknown
    ));
}

#[test]
fn duplicate_registry_and_prepared_identities_are_rejected() {
    assert!(ProfileRegistryBuilder::new().build().is_err());
    assert!(matches!(
        ProfileRegistryBuilder::new()
            .register::<Alpha, AlphaCodec>()
            .unwrap()
            .register::<Alpha, AlphaCodec>(),
        Err(RegistryError::DuplicateSectionType(_))
    ));
    assert!(matches!(
        ProfileRegistryBuilder::new()
            .register::<Alpha, AlphaCodec>()
            .unwrap()
            .register::<AlphaAlias, AlphaAliasCodec>(),
        Err(RegistryError::DuplicateSectionId(id)) if id == Alpha::ID.as_str()
    ));
    assert!(matches!(
        PreparedEnvelope::compiled(profile_id(), compiled_provenance())
            .insert(Alpha { value: 1 })
            .unwrap()
            .insert(Alpha { value: 2 }),
        Err(ProfileError::DuplicateSectionType { .. })
    ));
    assert!(matches!(
        PreparedEnvelope::compiled(profile_id(), compiled_provenance())
            .insert(Alpha { value: 1 })
            .unwrap()
            .insert(AlphaAlias(2)),
        Err(ProfileError::DuplicateSectionId { id }) if id == Alpha::ID.as_str()
    ));
}

#[test]
fn prepared_projection_checks_the_stable_id_and_rust_type_without_resolving() {
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(Alpha { value: 37 })
        .unwrap()
        .build()
        .unwrap();

    assert!(matches!(
        prepared.section::<AlphaAlias>(),
        Err(ProfileError::RegistryInvariant {
            id,
            expected_type: _,
            stored_type: _,
        }) if id == Alpha::ID.as_str()
    ));
    assert_eq!(
        prepared.section::<Alpha>().unwrap().unwrap().section.value,
        37
    );
}

#[test]
fn registry_encoding_reports_an_id_type_mismatch_without_panicking() {
    let registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(AlphaAlias(41))
        .unwrap()
        .build()
        .unwrap();

    assert!(matches!(
        registry.to_json(&prepared, &assembly()),
        Err(ProfileError::RegistryInvariant {
            id,
            expected_type: _,
            stored_type: _,
        }) if id == Alpha::ID.as_str()
    ));
}

#[test]
fn a_strict_empty_envelope_is_valid_and_projects_every_type_as_absent() {
    let registry = ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .build()
        .unwrap();
    let empty = PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .build()
        .unwrap();
    let json = registry.to_json(&empty, &assembly()).unwrap();
    let reopened = registry.from_json(&json).unwrap();

    assert_eq!(reopened.section_ids().count(), 0);
    assert!(reopened.section::<Alpha>().unwrap().is_none());
}

fn core_registry() -> gf2_core::tuning::ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn core_envelope(section: CoreTuning) -> PreparedEnvelope {
    PreparedEnvelope::compiled(profile_id(), compiled_provenance())
        .insert(section)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn core_omission_is_encoding_state_but_not_dispatch_state() {
    let omitted = CoreTuningCodec::decode_body(
        CanonicalValue::serialize(&std::collections::BTreeMap::<String, u64>::new()).unwrap(),
    )
    .unwrap();
    let explicit_default: CanonicalValue = serde_json::from_str(&format!(
        "{{\"bit_backend\":{{\"simd_min_words\":{}}}}}",
        CoreTuning::CONSERVATIVE.bit_backend().simd_min_words()
    ))
    .unwrap();
    let explicit = CoreTuningCodec::decode_body(explicit_default).unwrap();

    assert_eq!(omitted.selectors(), explicit.selectors());
    assert_ne!(omitted, explicit);
    assert_ne!(format!("{omitted:?}"), format!("{explicit:?}"));

    let omitted_json = core_registry()
        .to_json(&core_envelope(omitted), &assembly())
        .unwrap();
    let explicit_json = core_registry()
        .to_json(&core_envelope(explicit), &assembly())
        .unwrap();
    assert_ne!(omitted_json, explicit_json);
    assert!(omitted_json.contains("\"selectors\":{}"));
    assert!(explicit_json.contains("\"bit_backend\":{\"simd_min_words\":"));

    let reopened = core_registry().from_json(&omitted_json).unwrap();
    assert_eq!(
        core_registry().to_json(&reopened, &assembly()).unwrap(),
        omitted_json
    );
}

#[test]
fn programmatic_core_conservative_encoding_declares_the_canonical_inventory() {
    let json = core_registry()
        .to_json(&core_envelope(CoreTuning::CONSERVATIVE), &assembly())
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    let sections = wire["sections"].as_object().unwrap();
    assert_eq!(sections.len(), 1);
    let selectors = sections["gf2-core/selectors"]["selectors"]
        .as_object()
        .unwrap();
    assert_eq!(selectors.len(), 12);
    assert_eq!(
        selectors
            .values()
            .map(|family| family.as_object().unwrap().len())
            .sum::<usize>(),
        37
    );
}

#[test]
fn core_codec_rejects_null_instead_of_treating_it_as_omission() {
    for malformed in [
        r#"{"bit_backend":null}"#,
        r#"{"bit_backend":{"simd_min_words":null}}"#,
    ] {
        let value: CanonicalValue = serde_json::from_str(malformed).unwrap();
        assert!(matches!(
            CoreTuningCodec::decode_body(value),
            Err(SectionError::InvalidBody(_))
        ));
    }
}
