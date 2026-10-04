//! Pins the request the arm decodes against the runner's own request
//! declaration, over a cell the frozen family declares.

use byte_field_arm_common::smoke::{decode_guarded, top_level_keys};
use byte_field_arm_common::{runner_request_fields, validation_request};
use serde_json::Value;
use std::path::Path;
use tuning_campaign_support::protocol::{CellDeclaration, FamilyAddendum, SHARED_SETTINGS};
use tuning_campaign_support::transport;

/// The family's whole-consumer declaration of the frozen pilot, so the mirror
/// is pinned against a declaration the campaigns measure rather than a fixture
/// written here. The family directory holds this crate's survey directory.
fn whole_consumer_declaration() -> CellDeclaration {
    let family = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the arm workspace lies in the family's survey directory");
    let addendum = std::fs::read(family.join("addendum-v4-dense-product-pilot.json"))
        .expect("the frozen pilot addendum is readable");
    FamilyAddendum::decode(&addendum)
        .expect("the frozen addendum decodes")
        .cells
        .into_iter()
        .find(|cell| cell.conversion_costs_included)
        .expect("the family declares a whole-consumer cell")
}

/// The case `make-plan.py` projects for one declaration.
fn case_for(declaration: &CellDeclaration) -> Value {
    serde_json::json!({
        "operation": "matmul",
        "bytes": 0,
        "n": declaration.workload.size["n"],
        "k": 0,
        "rows": 0,
        "poly": 285,
        "metric": "whole-consumer",
        "seed": declaration.workload.seed,
        "workers": declaration.workers.declared,
    })
}

/// Runner request fields no cell of this family carries, and why.
const ABSENT_FIELDS: [(&str, &str); 2] = [
    (
        "cold_calls",
        "no cell is cold, and the arm refuses a cache state whose contract forbids the \
         untimed calls its allocation probe makes",
    ),
    (
        "decoder",
        "no cell declares a decoder, and the arm's guarded decoder rejects a request that \
         carries one",
    ),
];

/// A validation request for a family cell carries the runner's declared fields
/// in the runner's declaration order, the arm's own guarded decoder accepts it,
/// and its re-encoding is byte-identical, so a field added, dropped or reordered
/// on either side fails here rather than inside a benchmark window.
#[test]
fn the_request_mirror_is_the_runners_request() {
    let declaration = whole_consumer_declaration();
    let request = validation_request(
        &declaration,
        "table-element",
        case_for(&declaration),
        vec![0],
        &SHARED_SETTINGS,
    );
    let encoded = transport::encode_case(&request).expect("the runner's encoder");

    let expected: Vec<&str> = runner_request_fields()
        .into_iter()
        .filter(|field| !ABSENT_FIELDS.iter().any(|(absent, _)| absent == field))
        .collect();
    assert_eq!(top_level_keys(&encoded), expected);
    for (field, reason) in ABSENT_FIELDS {
        assert!(
            runner_request_fields().contains(&field),
            "{field} is no longer a runner request field ({reason})"
        );
    }

    let decoded = decode_guarded(&encoded).expect("the arm's decoder");
    assert_eq!(
        transport::encode_case(&decoded).expect("re-encodes"),
        encoded
    );
    assert_eq!(decoded.cache_state, declaration.cache_state);
    assert_eq!(decoded.cold_calls, declaration.cold_calls);
    assert_eq!(decoded.windows, SHARED_SETTINGS.windows_per_execution);
    assert_eq!(decoded.window_target_ms, SHARED_SETTINGS.window_target_ms);
    assert_eq!(decoded.workers_declared, declaration.workers.declared);
}
