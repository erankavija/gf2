//! The committed re-measurement addendum is frozen, satisfies the protocol-v3
//! addendum schema and the typed rules the runner applies before measuring,
//! and declares only exploratory cells in cache states the arms implement.

use std::path::Path;
use survey_gf2_side::CachePolicy;
use tuning_campaign_support::protocol::{CellRole, FamilyAddendum};
use tuning_campaign_support::schema;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../..");
const SCHEMA: &str = "dev/active/f547c394/addendum.schema.json";
const ADDENDUM: &str = "dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-remeasure.json";

fn read(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(REPO).join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn remeasure_addendum_is_valid_exploratory_and_implemented() {
    let schema_value: serde_json::Value =
        serde_json::from_slice(&read(SCHEMA)).expect("schema decodes");
    let bytes = read(ADDENDUM);
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
