#![cfg(feature = "tuning-profile")]

use std::collections::BTreeSet;

use gf2_core::tuning::{
    CoreTuning, CoreTuningCodec, MeasurementProvenance, ProfileRegistryBuilder, SectionCodec,
    TuningSection,
};

const CONSERVATIVE_OWNER: &str = include_str!("../data/tuning-profiles/conservative.json");
const MEASURED_OWNER: &str =
    include_str!("../data/tuning-profiles/gf2-63bad95d-20261006t230547z-2073085.json");

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
        "gf2-63bad95d-20261006t230547z-2073085"
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
                "65f178adc91add5c63b8ffe12dc7d4123dea0b5d"
            );
            assert!(!source_dirty);
            assert_eq!(
                harness.as_str(),
                "crates/gf2-core/benches/tuning_calibration.rs"
            );
            assert_eq!(harness_schema.as_str(), CoreTuningCodec::HARNESS_SCHEMA);
            assert_eq!(
                binary_sha256.as_str(),
                "d1dbabe8f4ea42e3b5b4cdbb54642addc12a1602e93233f859706a1d8e49f256"
            );
            assert_eq!(
                receipt.as_str(),
                "dev/benchmarks/tuning_profiles/gf2-63bad95d-20261006t230547z-2073085.md"
            );
        }
        MeasurementProvenance::Inherited => panic!("measured owner is not inherited"),
    }

    let section = measured.section;
    assert_eq!(section.bit_backend().simd_min_words(), 4);
    assert_eq!(section.bit_matrix().matvec_simd_min_words(), 8);
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
    assert_eq!(section.triangular().base_case_max_dim(), 12);
    assert_eq!(section.ple().panel_base_max_cols(), 128);
    assert_eq!(section.ple().blocked_back_sub_min_dim(), 64);
    assert_eq!(section.ple().scalar_base_max_cols(), 1);
    assert_eq!(section.gemm().axpy_fast_path_min_volume(), 512);
    assert_eq!(section.gemm().winograd_min_dim(), 128);
    assert_eq!(
        (section.gemm().row_tile(), section.gemm().col_tile()),
        (32, 64)
    );
    assert_eq!(section.field_vec().dot_chunk_len(), 256);

    let inventory = codec_selector_fields();
    let stated = stated_selector_fields(MEASURED_OWNER);
    let omitted = BTreeSet::from([
        "charpoly.keller_gehrig_min_dim".to_owned(),
        "ple.panel_byte_lane_max_cols".to_owned(),
        "ple.panel_u16_lane_max_cols".to_owned(),
        "prime_route.f32_k_chunk_cap".to_owned(),
        "prime_route.f32_min_cols".to_owned(),
        "prime_route.f32_min_prime".to_owned(),
        "prime_route.f64_k_chunk_cap".to_owned(),
        "prime_route.f64_min_cols".to_owned(),
        "sparse.block_csr_default_rows".to_owned(),
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

/// The selectors `@/issue/817fc47d` brings under the core section, with the
/// values of the constants they take over.
const CONVERGED_SELECTORS: [(&str, usize); 3] = [
    ("prime_route.f32_k_chunk_cap", 1024),
    ("prime_route.f64_k_chunk_cap", 4096),
    ("sparse.block_csr_default_rows", 32),
];

fn converged_values(section: &CoreTuning) -> [usize; 3] {
    [
        section.prime_route().f32_k_chunk_cap(),
        section.prime_route().f64_k_chunk_cap(),
        section.sparse().block_csr_default_rows(),
    ]
}

/// Every committed core owner: the conservative projection first, then the
/// measured owners.
const COMMITTED_OWNERS: [(&str, &str); 6] = [
    ("conservative", CONSERVATIVE_OWNER),
    (
        "gf2-389aa4de-20260901-040229-2742533",
        include_str!("../data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json"),
    ),
    (
        "gf2-eaae1b56-20260904-215231-898522",
        include_str!("../data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json"),
    ),
    (
        "gf2-a83583e0-20260930t230000z-2728298",
        include_str!("../data/tuning-profiles/gf2-a83583e0-20260930t230000z-2728298.json"),
    ),
    (
        "gf2-dbd8787d-20261001t230000z-2601601",
        include_str!("../data/tuning-profiles/gf2-dbd8787d-20261001t230000z-2601601.json"),
    ),
    ("gf2-63bad95d-20261006t230547z-2073085", MEASURED_OWNER),
];

#[test]
fn committed_owners_resolve_the_converged_selectors_to_their_conservative_values() {
    let expected = CONVERGED_SELECTORS.map(|(_, value)| value);
    assert_eq!(converged_values(&CoreTuning::CONSERVATIVE), expected);

    let registry = registry();
    let mut reopened = BTreeSet::new();
    for (name, text) in COMMITTED_OWNERS {
        let stated = stated_selector_fields(text);
        for (selector, _) in CONVERGED_SELECTORS {
            assert_eq!(
                stated.contains(selector),
                name == "conservative",
                "{name} and {selector}"
            );
        }
        // The codec refuses an owner of an earlier harness schema whole; the
        // owners it reopens carry the conservative values for what they omit.
        if let Ok(prepared) = registry.from_json(text) {
            let section = prepared.section::<CoreTuning>().unwrap().unwrap().section;
            assert_eq!(converged_values(section), expected, "{name}");
            reopened.insert(name);
        }
    }
    assert!(reopened.contains("conservative"));
    assert!(reopened.contains("gf2-63bad95d-20261006t230547z-2073085"));
}
