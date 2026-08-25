#![cfg(feature = "tuning-profile")]

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec};
use gf2_core::tuning::{
    CompiledProfileProvenance, CoreTuning, CoreTuningCodec, PreparedEnvelope, ProfileRegistry,
    ProfileRegistryBuilder, TuningSection,
};

const CORE_OWNER: &str = include_str!("../../gf2-core/data/tuning-profiles/conservative.json");
const ALGEBRA_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");
const COMPLETE: &str =
    include_str!("../../../dev/reference_data/tuning-profiles/conservative.json");

fn core_registry() -> ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn algebra_registry() -> ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn complete_registry() -> ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn section_wrapper(text: &str, id: &str) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    value["sections"][id].clone()
}

#[test]
fn committed_owner_and_complete_envelopes_reopen_strictly() {
    let core = core_registry().from_json(CORE_OWNER).unwrap();
    let algebra = algebra_registry().from_json(ALGEBRA_OWNER).unwrap();
    let complete = complete_registry().from_json(COMPLETE).unwrap();

    assert_eq!(
        core.section_ids().collect::<Vec<_>>(),
        [CoreTuning::ID.as_str()]
    );
    assert_eq!(
        algebra.section_ids().collect::<Vec<_>>(),
        [AlgebraTuning::ID.as_str()]
    );
    assert_eq!(
        complete.section_ids().collect::<Vec<_>>(),
        [AlgebraTuning::ID.as_str(), CoreTuning::ID.as_str()]
    );
    assert_eq!(
        complete.section::<CoreTuning>().unwrap().unwrap().section,
        &CoreTuning::CONSERVATIVE
    );
    assert_eq!(
        complete
            .section::<AlgebraTuning>()
            .unwrap()
            .unwrap()
            .section,
        &AlgebraTuning::CONSERVATIVE
    );

    assert!(core_registry().from_json(COMPLETE).is_err());
    assert!(algebra_registry().from_json(COMPLETE).is_err());
}

#[test]
fn complete_envelope_is_mechanical_composition_with_identical_wrappers() {
    let core = core_registry().from_json(CORE_OWNER).unwrap();
    let algebra = algebra_registry().from_json(ALGEBRA_OWNER).unwrap();
    let complete = complete_registry().from_json(COMPLETE).unwrap();

    assert_eq!(
        section_wrapper(CORE_OWNER, CoreTuning::ID.as_str()),
        section_wrapper(COMPLETE, CoreTuning::ID.as_str())
    );
    assert_eq!(
        section_wrapper(ALGEBRA_OWNER, AlgebraTuning::ID.as_str()),
        section_wrapper(COMPLETE, AlgebraTuning::ID.as_str())
    );

    let core = core.section::<CoreTuning>().unwrap().unwrap();
    let algebra = algebra.section::<AlgebraTuning>().unwrap().unwrap();
    let id = complete.profile_id().clone();
    let composed =
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert_measured::<CoreTuning, CoreTuningCodec>(
                core.section.clone(),
                core.measurement.clone(),
            )
            .unwrap()
            .insert_measured::<AlgebraTuning, AlgebraTuningCodec>(
                algebra.section.clone(),
                algebra.measurement.clone(),
            )
            .unwrap()
            .build()
            .unwrap();
    let assembly = complete.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        complete_registry().to_json(&composed, &assembly).unwrap(),
        COMPLETE
    );
}

#[test]
fn committed_algebra_owner_is_the_complete_programmatic_conservative_section() {
    let parsed = algebra_registry().from_json(ALGEBRA_OWNER).unwrap();
    let id = parsed.profile_id().clone();
    let prepared =
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert(AlgebraTuning::CONSERVATIVE)
            .unwrap()
            .build()
            .unwrap();
    let assembly = parsed.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        algebra_registry().to_json(&prepared, &assembly).unwrap(),
        ALGEBRA_OWNER
    );
}
