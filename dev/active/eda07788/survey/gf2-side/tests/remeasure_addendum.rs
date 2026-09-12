//! The committed re-measurement addendum is the one its campaign measured, is
//! frozen, satisfies the addendum schema and the typed rules the runner applies
//! before measuring, and declares only exploratory cells in cache states the
//! arms implement.
//!
//! The campaign is finished under protocol version 3, so the schema it must
//! satisfy is the version-3 snapshot its receipt pins, not the shared file,
//! which now carries version 4. The receipt's digests bind both snapshot and
//! addendum, so neither can drift away from what was measured.

use std::path::Path;
use survey_gf2_side::CachePolicy;
use tuning_campaign_support::protocol::{sha256_hex, CellRole, FamilyAddendum};
use tuning_campaign_support::schema;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../..");
const RECEIPT: &str = "dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/receipt.json";
const SCHEMA: &str =
    "dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/inputs/addendum.schema.json";
const ADDENDUM: &str = "dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-remeasure.json";

fn read(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(REPO).join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn pinned_sha256<'a>(receipt: &'a serde_json::Value, field: &str) -> &'a str {
    receipt[field]["sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("receipt pins no {field} digest"))
}

#[test]
fn remeasure_addendum_is_valid_exploratory_and_implemented() {
    let receipt: serde_json::Value =
        serde_json::from_slice(&read(RECEIPT)).expect("receipt decodes");
    let schema_bytes = read(SCHEMA);
    assert_eq!(
        sha256_hex(&schema_bytes),
        pinned_sha256(&receipt, "addendum_schema"),
        "{SCHEMA} is not the schema snapshot the receipt pins"
    );
    let bytes = read(ADDENDUM);
    assert_eq!(
        sha256_hex(&bytes),
        pinned_sha256(&receipt, "addendum"),
        "{ADDENDUM} is not the addendum the campaign measured"
    );

    let schema_value: serde_json::Value =
        serde_json::from_slice(&schema_bytes).expect("schema decodes");
    let instance: serde_json::Value = serde_json::from_slice(&bytes).expect("addendum decodes");
    let violations = schema::validate(&schema_value, &instance);
    assert!(violations.is_empty(), "schema violations: {violations:#?}");
    let addendum = FamilyAddendum::decode(&bytes).expect("typed addendum decodes");
    addendum
        .validate()
        .unwrap_or_else(|errors| panic!("semantic violations: {errors:#?}"));
    assert!(
        addendum.frozen.frozen_utc.is_some(),
        "addendum is not frozen"
    );
    assert!(!addendum.cells.is_empty());
    for cell in &addendum.cells {
        assert_eq!(cell.role, CellRole::Exploratory, "{}", cell.cell_id);
        let state = serde_json::to_value(cell.cache_state).expect("cache state encodes");
        let state = state.as_str().expect("cache state is a string");
        assert!(
            CachePolicy::from_request(state).is_ok(),
            "{}: {state}",
            cell.cell_id
        );
    }
}
