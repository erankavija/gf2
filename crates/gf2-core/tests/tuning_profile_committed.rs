#![cfg(feature = "tuning-profile")]

use std::collections::BTreeSet;

use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const CONSERVATIVE_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");
const MEASURED_OWNER: &str =
    include_str!("../data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json");

fn registry() -> gf2_core::tuning::ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

fn selector_fields(selectors: &serde_json::Value) -> BTreeSet<String> {
    selectors
        .as_object()
        .unwrap()
        .iter()
        .flat_map(|(family, fields)| {
            fields
                .as_object()
                .unwrap()
                .keys()
                .map(move |field| format!("{family}.{field}"))
        })
        .collect()
}

fn stated_selector_fields(text: &str) -> BTreeSet<String> {
    let document: serde_json::Value = serde_json::from_str(text).unwrap();
    selector_fields(&document["sections"][CoreTuning::ID.as_str()]["selectors"])
}

fn codec_selector_fields() -> BTreeSet<String> {
    let encoded = CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE).unwrap();
    selector_fields(&serde_json::to_value(encoded).unwrap())
}

#[test]
fn committed_core_owner_envelope_is_strict_canonical_and_complete() {
    let registry = registry();

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

#[test]
fn committed_measured_owner_is_canonical_and_states_exactly_the_five_field_pilot() {
    let registry = registry();
    let prepared = registry.from_json(MEASURED_OWNER).unwrap();
    assert_eq!(
        prepared.profile_id().as_str(),
        "gf2-389aa4de-20260901-040229-2742533"
    );
    assert_eq!(
        prepared.section_ids().collect::<Vec<_>>(),
        [CoreTuning::ID.as_str()]
    );
    let measured = prepared.section::<CoreTuning>().unwrap().unwrap();
    match measured.measurement {
        MeasurementProvenance::Calibrated {
            source_revision,
            source_dirty,
            harness,
            harness_schema,
            binary_sha256,
            receipt,
            ..
        } => {
            assert_eq!(
                source_revision.as_str(),
                "501fe8b28f351ac05e52cccd96ce6f0ae9898cdf"
            );
            assert!(!source_dirty);
            assert_eq!(
                harness.as_str(),
                "crates/gf2-core/benches/tuning_calibration.rs"
            );
            assert_eq!(
                harness_schema.as_str(),
                "tuning-calibration-v2",
                "the live 389 owner remains readable during the named eaae1b56 prepublication migration"
            );
            assert_eq!(
                binary_sha256.as_str(),
                "d1582f69ee17c379e47060f3f8b2b832061832de1fd4303376d75cc4a04eaca2"
            );
            assert_eq!(
                receipt.as_str(),
                "dev/benchmarks/tuning_profiles/2026-09-01-389aa4de.md"
            );
        }
        MeasurementProvenance::Inherited => panic!("measured owner is not inherited"),
    }

    assert_eq!(measured.section.bit_backend().simd_min_words(), 4);
    let polynomial = measured.section.polynomial();
    assert_eq!(polynomial.karatsuba_min_degree(), 31);
    assert_eq!(polynomial.karatsuba_max_out_len(), 383);
    assert_eq!(polynomial.div_rem_fast_min_len(), 1024);
    assert_eq!(polynomial.subproduct_min_len(), 512);

    let inventory = codec_selector_fields();
    let stated = stated_selector_fields(MEASURED_OWNER);
    let expected = BTreeSet::from([
        "bit_backend.simd_min_words".to_owned(),
        "polynomial.div_rem_fast_min_len".to_owned(),
        "polynomial.karatsuba_max_out_len".to_owned(),
        "polynomial.karatsuba_min_degree".to_owned(),
        "polynomial.subproduct_min_len".to_owned(),
    ]);
    assert_eq!(inventory.len(), 37);
    assert_eq!(stated, expected);
    assert!(stated.is_subset(&inventory));
    assert_eq!(inventory.difference(&stated).count(), 32);

    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        registry.to_json(&prepared, &assembly).unwrap(),
        MEASURED_OWNER
    );
}
