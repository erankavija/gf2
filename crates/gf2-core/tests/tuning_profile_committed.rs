#![cfg(feature = "tuning-profile")]

use std::collections::BTreeSet;

use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const CONSERVATIVE_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");
const MEASURED_OWNER: &str =
    include_str!("../data/tuning-profiles/gf2-a83583e0-20260930t230000z-2728298.json");

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
        "gf2-a83583e0-20260930t230000z-2728298"
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
                "21790510c0aac0b8459abe75825b4deb61e479e2"
            );
            assert!(!source_dirty);
            assert_eq!(
                harness.as_str(),
                "crates/gf2-core/benches/tuning_calibration.rs"
            );
            assert_eq!(harness_schema.as_str(), CoreTuningCodec::HARNESS_SCHEMA);
            assert_eq!(
                binary_sha256.as_str(),
                "5d2357fe37d3917bb0ebee966ccb17bfa004ef49d816b93b7a1cd1f4d407ddda"
            );
            assert_eq!(
                receipt.as_str(),
                "dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md"
            );
        }
        MeasurementProvenance::Inherited => panic!("measured owner is not inherited"),
    }

    let section = measured.section;
    assert_eq!(section.bit_backend().simd_min_words(), 4);
    let polynomial = section.polynomial();
    assert_eq!(polynomial.karatsuba_min_degree(), 31);
    assert_eq!(polynomial.karatsuba_max_out_len(), 128);
    assert_eq!(polynomial.div_rem_fast_min_len(), 1024);
    assert_eq!(polynomial.subproduct_min_len(), 512);
    assert_eq!(polynomial.interpolate_fast_min_points(), 2);
    assert_eq!(section.bit_matrix().transpose_simple_max_blocks(), 16);
    assert_eq!(section.bit_matrix().transpose_macro_tile_blocks(), 8);
    assert_eq!(section.soa_batch().parallel_min_len(), 65536);
    assert_eq!(section.soa_batch().parallel_chunk_len(), 16384);
    let m4rm = section.m4rm();
    assert_eq!(m4rm.wide_tier_min_stride_words(), 16);
    assert_eq!(m4rm.tiled_min_stride_words(), 4);
    assert_eq!(m4rm.default_table_bytes(), 65536);
    assert_eq!(m4rm.mid_table_bytes(), 131072);
    assert_eq!(m4rm.wide_table_bytes(), 262144);
    assert_eq!(m4rm.wide_max_k(), 9);
    assert_eq!(m4rm.small_n_max_k(), 8);
    assert_eq!(section.dense_inverse().m4ri_min_dim(), 7);
    assert_eq!(section.dense_inverse().blocked_min_dim(), 2);
    assert_eq!(section.triangular().trsm_blocked_min_dim(), 64);
    assert_eq!(section.triangular().trsm_panel_rows(), 64);
    assert_eq!(section.ple().panel_base_max_cols(), 128);
    assert_eq!(section.ple().blocked_back_sub_min_dim(), 64);
    assert_eq!(section.gemm().axpy_fast_path_min_volume(), 512);
    assert_eq!(
        (section.gemm().row_tile(), section.gemm().col_tile()),
        (32, 64)
    );
    assert_eq!(section.field_vec().dot_chunk_len(), 256);

    let inventory = codec_selector_fields();
    let stated = stated_selector_fields(MEASURED_OWNER);
    let omitted = BTreeSet::from([
        "bit_matrix.matvec_simd_min_words".to_owned(),
        "charpoly.keller_gehrig_min_dim".to_owned(),
        "gemm.winograd_min_dim".to_owned(),
        "ple.panel_byte_lane_max_cols".to_owned(),
        "ple.panel_u16_lane_max_cols".to_owned(),
        "ple.scalar_base_max_cols".to_owned(),
        "prime_route.f32_min_cols".to_owned(),
        "prime_route.f32_min_prime".to_owned(),
        "prime_route.f64_min_cols".to_owned(),
        "triangular.base_case_max_dim".to_owned(),
    ]);
    assert!(stated.is_subset(&inventory));
    assert_eq!(
        inventory
            .difference(&stated)
            .cloned()
            .collect::<BTreeSet<_>>(),
        omitted
    );

    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        registry.to_json(&prepared, &assembly).unwrap(),
        MEASURED_OWNER
    );
}
