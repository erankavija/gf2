//! Guarded fresh-process witness that the coding owner codec's canonical
//! format-2 round trip carries the same selection the compiled envelope does.

#![cfg(feature = "tuning-profile")]

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;
use gf2_coding::tuning::{CodingTuning, CodingTuningCodec, EncodeSelectors};
use gf2_core::tuning::{CanonicalValue, SectionCodec, TuningSection};

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
    let section = CodingTuning::from_selectors(
        EncodeSelectors::try_new(32, 16, 64, 8).expect("every selector bound is admissible"),
    );
    let body = CodingTuningCodec::encode_body(&section).expect("a complete section encodes");
    let decoded = CodingTuningCodec::decode_body(body).expect("the canonical body decodes");
    assert_eq!(decoded, section);
    assert_eq!(decoded.encode().table_remainder_min_redundancy(), 32);
    assert_eq!(decoded.encode().table_remainder_min_batch(), 16);
    assert_eq!(decoded.encode().bitslice_interleaved_min_batch(), 64);
    assert_eq!(decoded.encode().clmul_fold_min_batch(), 8);
}

#[test]
fn a_partial_body_defaults_the_absent_selectors() {
    let body: CanonicalValue =
        serde_json::from_str(r#"{"encode": {"table_remainder_min_batch": 4}}"#)
            .expect("a canonical selector body");
    let decoded = CodingTuningCodec::decode_body(body).expect("a partial body decodes");
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
fn an_unknown_selector_is_rejected() {
    let body: CanonicalValue = serde_json::from_str(r#"{"encode": {"no_such_selector": 4}}"#)
        .expect("a canonical selector body");
    assert!(CodingTuningCodec::decode_body(body).is_err());
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
