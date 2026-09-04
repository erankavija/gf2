use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const MEASURED_OWNER: &str =
    include_str!("../../data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json");
const MEASURED_PROFILE_ID: &str = "gf2-eaae1b56-20260904-215231-898522";
const MEASURED_RECEIPT: &str = "dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md";

/// Strictly reopens the measured owner and verifies codec-known omitted fields.
pub(crate) fn omitted_fields_section(family: &str, fields: &[&str]) -> CoreTuning {
    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .expect("core codec registers once")
        .build()
        .expect("core registry builds");
    let prepared = registry
        .from_json(MEASURED_OWNER)
        .expect("measured format-2 core owner reopens strictly");
    assert_eq!(prepared.profile_id().as_str(), MEASURED_PROFILE_ID);
    assert_eq!(
        prepared.section_ids().collect::<Vec<_>>(),
        [CoreTuning::ID.as_str()]
    );
    let measured = prepared
        .section::<CoreTuning>()
        .expect("core section has the registered Rust type")
        .expect("measured owner states its core section");
    match measured.measurement {
        MeasurementProvenance::Calibrated {
            harness_schema,
            receipt,
            ..
        } => {
            assert_eq!(harness_schema.as_str(), CoreTuningCodec::HARNESS_SCHEMA);
            assert_eq!(receipt.as_str(), MEASURED_RECEIPT);
        }
        MeasurementProvenance::Inherited => panic!("measured core section is not inherited"),
    }

    let document: serde_json::Value =
        serde_json::from_str(MEASURED_OWNER).expect("measured owner is JSON");
    let family_fields = document["sections"][CoreTuning::ID.as_str()]["selectors"][family]
        .as_object()
        .unwrap_or_else(|| panic!("measured owner lacks selector family {family}"));
    let inventory = CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE)
        .expect("conservative selectors encode");
    let inventory = serde_json::to_value(inventory).expect("codec body is JSON");
    let known_fields = inventory[family]
        .as_object()
        .unwrap_or_else(|| panic!("core codec has no selector family {family}"));
    assert!(!fields.is_empty(), "omission witness must name its fields");
    for field in fields {
        assert!(
            known_fields.contains_key(*field),
            "core codec has no selector {family}.{field}"
        );
        assert!(
            !family_fields.contains_key(*field),
            "measured owner unexpectedly states omitted selector {family}.{field}"
        );
    }
    measured.section.clone()
}
