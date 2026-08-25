#![cfg(feature = "tuning-profile")]

use std::collections::BTreeMap;

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec, PermanentSelectors, CHUNK_SUBSETS};
use gf2_core::tuning::{
    AssemblyProvenance, CanonicalValue, CompiledProfileProvenance, GitRevision, PreparedEnvelope,
    ProfileError, ProfileId, ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, SectionCodec,
    SectionError, Sha256,
};

fn profile_id() -> ProfileId {
    ProfileId::parse("algebra-owner-test").unwrap()
}

fn provenance() -> CompiledProfileProvenance {
    CompiledProfileProvenance {
        artifact_id: profile_id(),
    }
}

fn assembly() -> AssemblyProvenance {
    AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("2026-08-25T19:00:00Z").unwrap(),
        source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-algebra/tests/tuning_section.rs").unwrap(),
        tool_sha256: Sha256::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
    }
}

fn registry() -> gf2_core::tuning::ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn body(family: bool, field: Option<usize>) -> CanonicalValue {
    let mut selectors = BTreeMap::<&str, BTreeMap<&str, usize>>::new();
    if family {
        let mut permanent = BTreeMap::new();
        if let Some(value) = field {
            permanent.insert("gray_chunk_subsets", value);
        }
        selectors.insert("permanent", permanent);
    }
    CanonicalValue::serialize(&selectors).unwrap()
}

fn envelope(section: AlgebraTuning) -> PreparedEnvelope {
    PreparedEnvelope::compiled(profile_id(), provenance())
        .insert(section)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn omission_preserves_wire_identity_but_not_dispatch_semantics() {
    let omitted = AlgebraTuningCodec::decode_body(body(false, None)).unwrap();
    let explicit = AlgebraTuningCodec::decode_body(body(true, Some(CHUNK_SUBSETS))).unwrap();

    assert_eq!(omitted.permanent(), explicit.permanent());
    assert_ne!(omitted, explicit);
    assert_ne!(format!("{omitted:?}"), format!("{explicit:?}"));

    let omitted_json = registry().to_json(&envelope(omitted), &assembly()).unwrap();
    let explicit_json = registry()
        .to_json(&envelope(explicit), &assembly())
        .unwrap();
    assert_ne!(omitted_json, explicit_json);
    assert!(!omitted_json.contains("gray_chunk_subsets"));
    assert!(explicit_json.contains("\"gray_chunk_subsets\":65536"));
}

#[test]
fn owner_parse_encode_preserves_omission_and_exact_id_set() {
    let original = registry()
        .to_json(
            &envelope(AlgebraTuningCodec::decode_body(body(true, None)).unwrap()),
            &assembly(),
        )
        .unwrap();
    let reopened = registry().from_json(&original).unwrap();

    assert_eq!(
        reopened.section_ids().collect::<Vec<_>>(),
        ["gf2-algebra/permanent"]
    );
    assert_eq!(
        registry().to_json(&reopened, &assembly()).unwrap(),
        original
    );
}

#[test]
fn programmatic_conservative_is_complete_and_zero_is_rejected() {
    let complete = registry()
        .to_json(&envelope(AlgebraTuning::CONSERVATIVE), &assembly())
        .unwrap();
    assert!(complete.contains("\"permanent\":{\"gray_chunk_subsets\":65536}"));
    assert_eq!(
        PermanentSelectors::try_new(0),
        Err(SectionError::SelectorOutOfRange {
            family: "permanent",
            field: "gray_chunk_subsets",
            value: 0,
        })
    );
    assert!(matches!(
        AlgebraTuningCodec::decode_body(body(true, Some(0))),
        Err(SectionError::SelectorOutOfRange { .. })
    ));
}

#[test]
fn algebra_owner_rejects_unknown_selector_members() {
    let malformed = CanonicalValue::serialize(&BTreeMap::from([(
        "unknown",
        BTreeMap::from([("value", 1_usize)]),
    )]))
    .unwrap();
    assert!(matches!(
        AlgebraTuningCodec::decode_body(malformed),
        Err(SectionError::InvalidBody(_))
    ));

    let owner = registry().to_json(&envelope(AlgebraTuning::CONSERVATIVE), &assembly());
    assert!(!matches!(
        owner,
        Err(ProfileError::UnexpectedSection { .. })
    ));
}
