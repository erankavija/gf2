use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, TuningSection,
};

const MEASURED_OWNER: &str =
    include_str!("../../data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json");
const MEASURED_PROFILE_ID: &str = "gf2-389aa4de-20260901-040229-2742533";
const MEASURED_RECEIPT: &str = "dev/benchmarks/tuning_profiles/2026-09-01-389aa4de.md";

/// Strictly reopens the current measured owner and verifies one omitted family.
pub(crate) fn omitted_family_section(family: &str) -> CoreTuning {
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
            assert_eq!(
                harness_schema.as_str(),
                "tuning-calibration-v2",
                "the live 389 owner remains readable until the eaae1b56 publication cutover"
            );
            assert_eq!(receipt.as_str(), MEASURED_RECEIPT);
        }
        MeasurementProvenance::Inherited => panic!("measured core section is not inherited"),
    }

    let document: serde_json::Value =
        serde_json::from_str(MEASURED_OWNER).expect("measured owner is JSON");
    let family_fields = document["sections"][CoreTuning::ID.as_str()]["selectors"][family]
        .as_object()
        .unwrap_or_else(|| panic!("measured owner lacks selector family {family}"));
    assert!(
        family_fields.is_empty(),
        "measured owner unexpectedly states a value in omitted family {family}"
    );
    measured.section.clone()
}
