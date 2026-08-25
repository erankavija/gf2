#![cfg(feature = "tuning-profile")]

use gf2_core::tuning::{CoreTuning, CoreTuningCodec, ProfileRegistryBuilder, TuningSection};

const CONSERVATIVE_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");

#[test]
fn committed_core_owner_envelope_is_strict_canonical_and_complete() {
    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .build()
        .unwrap();

    let prepared = registry.from_json(CONSERVATIVE_OWNER).unwrap();
    assert_eq!(
        prepared.section_ids().collect::<Vec<_>>(),
        [CoreTuning::ID.as_str()]
    );
    assert_eq!(
        prepared.section::<CoreTuning>().unwrap().unwrap().section,
        &CoreTuning::CONSERVATIVE
    );

    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        registry.to_json(&prepared, &assembly).unwrap(),
        CONSERVATIVE_OWNER
    );
}
