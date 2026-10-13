//! The committed coding owner envelope of the decoder calibration campaign,
//! read through the owner codec.

#![cfg(feature = "tuning-profile")]

use std::collections::BTreeMap;

use gf2_coding::tuning::{CodingTuning, CodingTuningCodec};
use gf2_core::tuning::{ProfileRegistryBuilder, SectionCodec, TuningSection};

const MEASURED_OWNER: &str =
    include_str!("../data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json");

/// The selectors of a section body by `family.field`.
fn fields(selectors: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    selectors
        .as_object()
        .expect("a selector body")
        .iter()
        .flat_map(|(family, fields)| {
            fields
                .as_object()
                .expect("a selector family")
                .iter()
                .map(move |(field, value)| (format!("{family}.{field}"), value.clone()))
        })
        .collect()
}

/// Every selector `section` resolves, whether or not its envelope states it.
fn resolved(section: &CodingTuning) -> BTreeMap<String, serde_json::Value> {
    let complete = CodingTuning::from_selectors(section.selectors().clone());
    let body = CodingTuningCodec::encode_body(&complete).expect("a complete section encodes");
    fields(&serde_json::to_value(body).expect("a canonical value is JSON"))
}

#[test]
fn a_selector_the_measured_owner_omits_keeps_its_conservative_value() {
    let registry = ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
    let prepared = registry.from_json(MEASURED_OWNER).unwrap();
    let section = prepared
        .section::<CodingTuning>()
        .unwrap()
        .expect("the owner holds the coding section")
        .section;
    let document: serde_json::Value = serde_json::from_str(MEASURED_OWNER).unwrap();
    let stated = fields(&document["sections"][CodingTuning::ID.as_str()]["selectors"]);
    let conservative = resolved(&CodingTuning::CONSERVATIVE);

    let resolved = resolved(section);
    assert!(stated.keys().all(|field| resolved.contains_key(field)));
    for (field, value) in &resolved {
        match stated.get(field) {
            // The campaign states a selector only where its decision leaves
            // the conservative value.
            Some(stated) => {
                assert_eq!(value, stated, "{field}");
                assert_ne!(value, &conservative[field], "{field}");
            }
            None => assert_eq!(value, &conservative[field], "{field}"),
        }
    }
}
