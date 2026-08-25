#![cfg(feature = "tuning-profile")]

use gf2_core::tuning::{
    AssemblyProvenance, CanonicalValue, CompiledProfileProvenance, CoreTuning, CoreTuningCodec,
    MeasurementProvenance, PreparedEnvelope, ProfileError, ProfileId, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, SectionCodec, SectionError, SectionId, Sha256, TuningSection,
    PROFILE_FORMAT_VERSION,
};

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
fn duplicate_registry_and_prepared_identities_are_rejected() {
    assert!(ProfileRegistryBuilder::new()
        .register::<Alpha, AlphaCodec>()
        .unwrap()
        .register::<Alpha, AlphaCodec>()
        .is_err());
    assert!(
        PreparedEnvelope::compiled(profile_id(), compiled_provenance())
            .insert(Alpha { value: 1 })
            .unwrap()
            .insert(Alpha { value: 2 })
            .is_err()
    );
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
