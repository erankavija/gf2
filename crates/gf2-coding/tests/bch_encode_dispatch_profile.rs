//! Guarded fresh-process witness that the coding owner codec's canonical
//! format-2 round trip carries the same selection the compiled envelope does,
//! with the codec's own contract for both selector families.

#![cfg(feature = "tuning-profile")]

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;
use gf2_coding::tuning::{
    CodingSelectors, CodingTuning, CodingTuningCodec, DecodeSelectors, EncodeSelectors,
};
use gf2_core::tuning::{
    CanonicalValue, ProfileRegistryBuilder, SectionCodec, SectionError, TuningSection,
};

/// An owner envelope whose coding section holds the encode family alone; the
/// codec decodes it to the conservative decode family and encodes it back to
/// the same bytes.
const ENCODE_FAMILY_PROFILE: &str = include_str!("data/tuning_profile_encode_family.json");

fn body(text: &str) -> CanonicalValue {
    serde_json::from_str(text).expect("a canonical selector body")
}

/// Decodes `text` and encodes the result, as a JSON value.
fn reencoded(text: &str) -> serde_json::Value {
    let decoded = CodingTuningCodec::decode_body(body(text)).expect("the body decodes");
    let encoded = CodingTuningCodec::encode_body(&decoded).expect("a decoded section encodes");
    serde_json::to_value(encoded).expect("a canonical value is JSON")
}

#[test]
fn an_encoded_profile_selects_the_table_family() {
    let result =
        fresh::fresh_tuning_process(fresh::FreshProcessCase::TableRemainderEncoded).unwrap();
    assert_eq!(result["family"], EncodeFamily::TableRemainder.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
}

#[test]
fn an_encoded_profile_selects_the_bit_sliced_family() {
    let result =
        fresh::fresh_tuning_process(fresh::FreshProcessCase::BitsliceInterleavedEncoded).unwrap();
    assert_eq!(result["family"], EncodeFamily::BitsliceInterleaved.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
}

#[test]
fn an_encoded_profile_selects_the_fold_family() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ClmulFoldEncoded).unwrap();
    assert_eq!(result["family"], EncodeFamily::ClmulFold.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
}

#[test]
fn the_owner_codec_round_trips_its_selectors() {
    let section = CodingTuning::from_selectors(CodingSelectors {
        encode: EncodeSelectors::try_new(32, 16, 64, 8)
            .expect("every selector bound is admissible"),
        decode: DecodeSelectors::try_new(4, 4096, 2, 3).expect("a positive group count"),
    });
    let body = CodingTuningCodec::encode_body(&section).expect("a complete section encodes");
    let decoded = CodingTuningCodec::decode_body(body).expect("the canonical body decodes");
    assert_eq!(decoded, section);
    assert_eq!(decoded.encode().table_remainder_min_redundancy(), 32);
    assert_eq!(decoded.encode().table_remainder_min_batch(), 16);
    assert_eq!(decoded.encode().bitslice_interleaved_min_batch(), 64);
    assert_eq!(decoded.encode().clmul_fold_min_batch(), 8);
    assert_eq!(decoded.decode().lane_min_frames(), 4);
    assert_eq!(decoded.decode().lane_max_edges(), 4096);
    assert_eq!(decoded.decode().lane_max_workers().get(), 2);
    assert_eq!(decoded.decode().granules_per_task().get(), 3);
    assert_eq!(decoded.decode().qc_route(), None);
}

#[test]
fn a_partial_body_defaults_the_absent_selectors() {
    let decoded =
        CodingTuningCodec::decode_body(body(r#"{"encode": {"table_remainder_min_batch": 4}}"#))
            .expect("a partial body decodes");
    let conservative = CodingTuning::CONSERVATIVE;
    assert_eq!(
        decoded.encode().table_remainder_min_redundancy(),
        conservative.encode().table_remainder_min_redundancy()
    );
    assert_eq!(decoded.encode().table_remainder_min_batch(), 4);
    assert_eq!(
        decoded.encode().bitslice_interleaved_min_batch(),
        conservative.encode().bitslice_interleaved_min_batch()
    );
    assert_eq!(
        decoded.encode().clmul_fold_min_batch(),
        conservative.encode().clmul_fold_min_batch()
    );
}

#[test]
fn a_partial_decode_family_defaults_the_absent_selectors() {
    let conservative = DecodeSelectors::CONSERVATIVE;
    let decoded = CodingTuningCodec::decode_body(body(r#"{"decode": {"lane_max_edges": 4096}}"#))
        .expect("a partial body decodes");
    assert_eq!(
        decoded.decode().lane_min_frames(),
        conservative.lane_min_frames()
    );
    assert_eq!(
        decoded.decode().lane_max_workers(),
        conservative.lane_max_workers()
    );
    assert_eq!(decoded.decode().lane_max_edges(), 4096);
    assert_eq!(
        decoded.decode().granules_per_task(),
        conservative.granules_per_task()
    );
    assert_eq!(decoded.encode(), CodingTuning::CONSERVATIVE.encode());
}

#[test]
fn the_codec_encodes_the_fields_a_body_states_and_no_others() {
    for text in [
        r#"{}"#,
        r#"{"decode": {}}"#,
        r#"{"decode": {"lane_min_frames": 2}}"#,
        r#"{"decode": {"lane_max_edges": 4096}}"#,
        r#"{"decode": {"granules_per_task": 5}}"#,
        r#"{"decode": {"lane_max_workers": 6}}"#,
        r#"{"decode": {"granules_per_task": 5, "lane_min_frames": 2}, "encode": {}}"#,
        r#"{"encode": {"clmul_fold_min_batch": 8}}"#,
    ] {
        let stated: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(reencoded(text), stated, "{text}");
    }
}

#[test]
fn a_body_without_the_decode_family_reads_as_the_conservative_family() {
    for text in [r#"{}"#, r#"{"encode": {"table_remainder_min_batch": 4}}"#] {
        let decoded = CodingTuningCodec::decode_body(body(text)).expect("the body decodes");
        assert_eq!(decoded.decode(), &DecodeSelectors::CONSERVATIVE, "{text}");
    }
}

#[test]
fn the_conservative_decode_family_holds_the_extreme_of_each_domain() {
    let conservative = CodingTuning::CONSERVATIVE;
    let decode = conservative.decode();
    assert_eq!(decode, &DecodeSelectors::CONSERVATIVE);
    assert_eq!(decode.lane_min_frames(), usize::MAX);
    assert_eq!(decode.lane_max_edges(), 0);
    assert_eq!(decode.granules_per_task().get(), 1);
    // QC route bounds are no selector; the route takes its structural bounds.
    assert_eq!(decode.qc_route(), None);
}

#[test]
fn an_unknown_selector_is_rejected() {
    for text in [
        r#"{"encode": {"no_such_selector": 4}}"#,
        r#"{"decode": {"no_such_selector": 4}}"#,
        r#"{"no_such_family": {}}"#,
    ] {
        assert!(
            CodingTuningCodec::decode_body(body(text)).is_err(),
            "{text}"
        );
    }
}

#[test]
fn a_null_family_or_selector_is_rejected() {
    for text in [
        r#"{"decode": null}"#,
        r#"{"decode": {"lane_min_frames": null}}"#,
        r#"{"decode": {"lane_max_edges": null}}"#,
        r#"{"decode": {"lane_max_workers": null}}"#,
        r#"{"decode": {"granules_per_task": null}}"#,
        r#"{"encode": null}"#,
        r#"{"encode": {"clmul_fold_min_batch": null}}"#,
    ] {
        assert!(
            matches!(
                CodingTuningCodec::decode_body(body(text)),
                Err(SectionError::InvalidBody(_))
            ),
            "{text}"
        );
    }
}

/// QC route bounds are a caller's parameter of the route, never a profile
/// selector, so a body stating them is refused.
#[test]
fn a_body_stating_a_fold_run_length_is_rejected() {
    for text in [
        r#"{"encode": {"clmul_fold_run_blocks": 64}}"#,
        r#"{"encode": {"clmul_fold_run_blocks": null}}"#,
    ] {
        assert!(
            matches!(
                CodingTuningCodec::decode_body(body(text)),
                Err(SectionError::InvalidBody(_))
            ),
            "{text}"
        );
    }
}

#[test]
fn a_body_stating_qc_route_bounds_is_rejected() {
    for text in [
        r#"{"decode": {"qc_route": {"lane_claim_max_lifting_size": 24, "lane_claim_min_frames": 8, "max_workers": 12, "min_lifting_size": 0}}}"#,
        r#"{"decode": {"qc_route": null}}"#,
        r#"{"decode": {"lane_min_frames": 2, "qc_route": {}}}"#,
    ] {
        assert!(
            matches!(
                CodingTuningCodec::decode_body(body(text)),
                Err(SectionError::InvalidBody(_))
            ),
            "{text}"
        );
    }
}

#[test]
fn a_zero_group_count_is_rejected() {
    let rejected = SectionError::SelectorOutOfRange {
        family: "decode",
        field: "granules_per_task",
        value: 0,
    };
    assert_eq!(
        CodingTuningCodec::decode_body(body(r#"{"decode": {"granules_per_task": 0}}"#)),
        Err(rejected.clone())
    );
    assert_eq!(DecodeSelectors::try_new(8, 4096, 1, 0), Err(rejected));
}

#[test]
fn a_committed_encode_only_profile_decodes_to_the_same_section() {
    let registry = ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = registry.from_json(ENCODE_FAMILY_PROFILE).unwrap();
    let section = prepared
        .section::<CodingTuning>()
        .unwrap()
        .expect("the profile holds the coding section")
        .section;
    assert_eq!(
        section.encode(),
        &EncodeSelectors::try_new(32, 16, 64, 8).unwrap()
    );
    assert_eq!(section.decode(), &DecodeSelectors::CONSERVATIVE);

    // The same bytes come back, so the section states the encode family alone.
    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        registry.to_json(&prepared, &assembly).unwrap(),
        ENCODE_FAMILY_PROFILE
    );
}

#[test]
fn an_uninstalled_process_resolves_the_conservative_decode_family() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ConservativeDefault).unwrap();
    assert_eq!(
        result["decode"],
        fresh::decode_report(&DecodeSelectors::CONSERVATIVE)
    );
    assert_eq!(result["resolution"], "frozen-before-install");
}

#[test]
fn an_installed_profile_resolves_both_families_through_one_section() {
    let (lane_min_frames, lane_max_edges, granules_per_task) = fresh::INSTALLED_DECODE;
    let installed = fresh::decode_report(
        &DecodeSelectors::try_new(lane_min_frames, lane_max_edges, 1, granules_per_task)
            .expect("a positive group count"),
    );
    assert_ne!(
        installed,
        fresh::decode_report(&DecodeSelectors::CONSERVATIVE)
    );
    for case in [
        fresh::FreshProcessCase::DecodeFamilyCompiled,
        fresh::FreshProcessCase::DecodeFamilyEncoded,
    ] {
        let result = fresh::fresh_tuning_process(case).unwrap();
        assert_eq!(result["decode"], installed, "{case:?}");
        assert_eq!(result["family"], EncodeFamily::TableRemainder.name());
        assert_eq!(result["agrees_with_reference"], true);
        assert_eq!(result["resolution"], "installed");
    }
}

#[test]
fn the_section_carries_its_own_stable_identity() {
    assert_eq!(CodingTuning::ID.as_str(), "gf2-coding/encode");
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}

#[test]
fn zero_lane_worker_limit_is_rejected() {
    let rejected = SectionError::SelectorOutOfRange {
        family: "decode",
        field: "lane_max_workers",
        value: 0,
    };
    assert_eq!(
        DecodeSelectors::try_new(1, 4096, 0, 1),
        Err(rejected.clone())
    );
    assert_eq!(
        CodingTuningCodec::decode_body(body(r#"{"decode":{"lane_max_workers":0}}"#)),
        Err(rejected)
    );
}
