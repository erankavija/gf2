//! Cross-checks that every protocol version's own committed schema file
//! validates a real frozen family addendum that names that version.
//!
//! Each protocol version keeps its own committed schema file. Version 1 has
//! two, since a wording-only edit to the `frozen.description` annotation split
//! the very first pilot and confirmation receipts from every later version-1
//! receipt (`amendment-v2.md`); both are exercised below. Schemas and fixtures
//! are frozen, so the test locates each by its SHA-256 among the addendum
//! files git lists in the checkout.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use tuning_campaign_support::campaign::Sha256Digest;
use tuning_campaign_support::repository::repository_root;
use tuning_campaign_support::schema;

/// SHA-256 of one committed schema file and of one frozen family addendum
/// whose `schema` field names that schema's `$id`, in protocol-version order.
const VERSIONED_FIXTURES: &[(&str, &str)] = &[
    (
        "5eab7106aaae753555a21177e93be8368bdb463eb638b06d288c4471a3e8d240",
        "6f762913988f8cb517f5e17c2832fff06d22359a7f1eb8c59d2f47a7ffe0feb4",
    ),
    (
        "44dd132a6203c5d04676bbc9d596915a0aeb4c250fb88975ab930a81ffe6aca1",
        "5ff212e57d0e7cd23b40d6007668cb7a51a1e96aa1a1d4d389e1b66f3d2290c3",
    ),
    (
        "4900f412729a76cd6ecf94de21b1f3fb36239f977975490ab8e182397a6fadef",
        "0b92af3dfd9e0902aac4650eb63e74f81034bd02cb3f75fc7691e72ca123092c",
    ),
    (
        "513f37627dd82dbce95f49ff3f714af83131320fdbd0fe704baffc18a57412cd",
        "42850bc46ce792bea49d2017158d8b500c35c30bb21d724dc767637899818281",
    ),
    (
        "ebd969ef6ae666d0c85f05f96a6d07ebbbf74a9cf69d88d474d2e39d66a2dc47",
        "019f7444d5fd5e9a2ef4dc2aba733997fa2dd0350b5710cd4aa0d69b3f9f8b90",
    ),
];

/// Every `addendum*.json` file git reports below `root` as tracked or
/// untracked and not ignored, keyed by SHA-256; byte-identical copies share
/// one entry.
fn addendum_files(root: &Path) -> BTreeMap<String, (String, Vec<u8>)> {
    let listing = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ":(glob)**/addendum*.json",
        ])
        .output()
        .unwrap();
    assert!(listing.status.success(), "git cannot list addendum files");
    let mut files = BTreeMap::new();
    for path in listing.stdout.split(|&b| b == 0).filter(|p| !p.is_empty()) {
        let path = String::from_utf8(path.to_vec()).unwrap();
        let bytes = match fs::read(root.join(&path)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            other => other.unwrap(),
        };
        files
            .entry(Sha256Digest::of(&bytes).as_str().to_owned())
            .or_insert((path, bytes));
    }
    files
}

#[test]
fn each_protocol_version_validates_against_its_own_committed_schema() {
    let files = addendum_files(&repository_root().unwrap());
    let located = |sha256: &str| {
        files
            .get(sha256)
            .unwrap_or_else(|| panic!("no committed addendum file has SHA-256 {sha256}"))
    };
    for (schema_sha256, addendum_sha256) in VERSIONED_FIXTURES {
        let (schema_path, schema_bytes) = located(schema_sha256);
        let (addendum_path, addendum_bytes) = located(addendum_sha256);
        let schema_value: Value = serde_json::from_slice(schema_bytes)
            .unwrap_or_else(|error| panic!("{schema_path} does not decode as JSON: {error}"));
        let instance: Value = serde_json::from_slice(addendum_bytes)
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
