use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const MEASURED_OWNER: &str =
    include_str!("../../data/tuning-profiles/gf2-a83583e0-20260930t230000z-2728298.json");
const MEASURED_PROFILE_ID: &str = "gf2-a83583e0-20260930t230000z-2728298";
const MEASURED_RECEIPT: &str =
    "dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md";

/// Strictly reopens the measured owner and verifies which codec-known fields
/// of `family` it states and which it omits.
pub(crate) fn measured_section(family: &str, stated: &[&str], omitted: &[&str]) -> CoreTuning {
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
    assert!(
        !stated.is_empty() || !omitted.is_empty(),
        "measured-owner witness must name its fields"
    );
    for (fields, expect_stated) in [(stated, true), (omitted, false)] {
        for field in fields {
            assert!(
                known_fields.contains_key(*field),
                "core codec has no selector {family}.{field}"
            );
            assert_eq!(
                family_fields.contains_key(*field),
                expect_stated,
                "measured owner statement of {family}.{field}"
            );
        }
    }
    measured.section.clone()
}
