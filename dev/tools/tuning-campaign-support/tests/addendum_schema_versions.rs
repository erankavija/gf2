//! Cross-checks that every protocol version's own committed schema file
//! validates a real frozen family addendum that names that version.
//!
//! `dev/active/f547c394/addendum.schema.json` is the shared path and always
//! describes the current protocol version; each earlier version has its own
//! committed sibling file (`addendum-v1.schema.json`, `addendum-v2.schema.json`,
//! `addendum-v3.schema.json`, ...), named by the `addendum-v1.schema.json`
//! pattern the version-1 archive established. Version 1 has two such files,
//! `addendum-v1-initial.schema.json` and `addendum-v1.schema.json`, since a
//! wording-only edit to the `frozen.description` annotation split the very
//! first pilot and confirmation receipts from every later version-1 receipt
//! (`amendment-v2.md`); both are exercised below. A frozen family addendum
//! names its own protocol version through its `schema` field, so this test
//! resolves each fixture's schema file by that identity and fails loudly when
//! the version a fixture names has no committed schema file to read.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tuning_campaign_support::schema;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

/// One frozen family addendum known to name a given protocol version, paired
/// with the committed schema file that version's `schema` identity resolves
/// to. The shared path stands for the current version; earlier versions each
/// name their own sibling file.
const VERSIONED_FIXTURES: &[(&str, &str)] = &[
    (
        "dev/active/f547c394/addendum-v1-initial.schema.json",
        "dev/active/f547c394/addendum-protocol-smoke-pilot.json",
    ),
    (
        "dev/active/f547c394/addendum-v1.schema.json",
        "dev/active/26465e6c/superseded/v1/addendum-popcount.json",
    ),
    (
        "dev/active/f547c394/addendum-v2.schema.json",
        "dev/active/f547c394/addendum-smoke-v2-pilot.json",
    ),
    (
        "dev/active/f547c394/addendum-v3.schema.json",
        "dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-confirmation.json",
    ),
    (
        "dev/active/f547c394/addendum.schema.json",
        "dev/active/5cbb6545/addendum-popcount-v4-confirmation.json",
    ),
];

#[test]
fn each_protocol_version_validates_against_its_own_committed_schema() {
    let root = repo_root();
    for (schema_path, addendum_path) in VERSIONED_FIXTURES {
        let schema_bytes = fs::read(root.join(schema_path))
            .unwrap_or_else(|error| panic!("{schema_path} is not committed: {error}"));
        let schema_value: Value = serde_json::from_slice(&schema_bytes)
            .unwrap_or_else(|error| panic!("{schema_path} does not decode as JSON: {error}"));
        let addendum_bytes = fs::read(root.join(addendum_path))
            .unwrap_or_else(|error| panic!("{addendum_path} is not committed: {error}"));
        let instance: Value = serde_json::from_slice(&addendum_bytes)
            .unwrap_or_else(|error| panic!("{addendum_path} does not decode as JSON: {error}"));
        let named_schema_id = instance
            .get("schema")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{addendum_path} names no schema identity"));
        assert_eq!(
            schema_value.get("$id").and_then(Value::as_str),
            Some(named_schema_id),
            "{schema_path} identity does not match the schema {addendum_path} names",
        );
        let violations = schema::validate(&schema_value, &instance);
        assert!(
            violations.is_empty(),
            "{addendum_path} does not validate against {schema_path}: {violations:?}",
        );
    }
}
