#![cfg(feature = "tuning-profile")]

use std::collections::BTreeSet;

use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const CONSERVATIVE_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");
const MEASURED_OWNER: &str =
    include_str!("../data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json");

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
fn committed_measured_owner_is_canonical_and_states_the_declared_campaign() {
    let registry = registry();
    let prepared = registry.from_json(MEASURED_OWNER).unwrap();
    assert_eq!(
        prepared.profile_id().as_str(),
        "gf2-eaae1b56-20260904-215231-898522"
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
                "5578100d50adb58fc2d0d4e70b461efdfc918e98"
            );
            assert!(!source_dirty);
            assert_eq!(
                harness.as_str(),
                "crates/gf2-core/benches/tuning_calibration.rs"
            );
            // The committed owner remains the immutable v3 measurement while
            // the named a83583e0 prepublication codec boundary accepts it.
            // Atomic v4 publication replaces this receipt and assertion.
            assert_eq!(harness_schema.as_str(), "tuning-calibration-v3");
            assert_eq!(
                binary_sha256.as_str(),
                "282c83f10d681c78b2000f0a403ff6b27fe1d08a65712deac514c47aa65ef7bc"
            );
            assert_eq!(
                receipt.as_str(),
                "dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md"
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
    assert_eq!(polynomial.interpolate_fast_min_points(), 2);
    assert_eq!(
        measured.section.bit_matrix().transpose_simple_max_blocks(),
        16
    );
    assert_eq!(measured.section.soa_batch().parallel_min_len(), 16384);
    assert_eq!(measured.section.m4rm().wide_tier_min_stride_words(), 16);
    assert_eq!(measured.section.m4rm().tiled_min_stride_words(), 4);
    assert_eq!(measured.section.dense_inverse().m4ri_min_dim(), 7);
    assert_eq!(measured.section.dense_inverse().blocked_min_dim(), 2);
    assert_eq!(measured.section.triangular().trsm_blocked_min_dim(), 64);
    assert_eq!(measured.section.ple().panel_base_max_cols(), 128);
    assert_eq!(measured.section.ple().blocked_back_sub_min_dim(), 64);
    assert_eq!(measured.section.gemm().axpy_fast_path_min_volume(), 512);

    let inventory = codec_selector_fields();
    let stated = stated_selector_fields(MEASURED_OWNER);
    let expected = BTreeSet::from([
        "bit_backend.simd_min_words".to_owned(),
        "bit_matrix.transpose_simple_max_blocks".to_owned(),
        "dense_inverse.blocked_min_dim".to_owned(),
        "dense_inverse.m4ri_min_dim".to_owned(),
        "gemm.axpy_fast_path_min_volume".to_owned(),
        "m4rm.tiled_min_stride_words".to_owned(),
        "m4rm.wide_tier_min_stride_words".to_owned(),
        "ple.blocked_back_sub_min_dim".to_owned(),
        "ple.panel_base_max_cols".to_owned(),
        "polynomial.div_rem_fast_min_len".to_owned(),
        "polynomial.interpolate_fast_min_points".to_owned(),
        "polynomial.karatsuba_max_out_len".to_owned(),
        "polynomial.karatsuba_min_degree".to_owned(),
        "polynomial.subproduct_min_len".to_owned(),
        "soa_batch.parallel_min_len".to_owned(),
        "triangular.trsm_blocked_min_dim".to_owned(),
    ]);
    assert_eq!(inventory.len(), 37);
    assert_eq!(stated, expected);
    assert!(stated.is_subset(&inventory));
    assert_eq!(inventory.difference(&stated).count(), 21);

    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        registry.to_json(&prepared, &assembly).unwrap(),
        MEASURED_OWNER
    );
}
