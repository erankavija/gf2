//! Validate family addenda against the committed JSON Schema and typed rules.

use std::fs;
use tuning_campaign_support::protocol::FamilyAddendum;
use tuning_campaign_support::schema;

fn main() {
    let mut args = std::env::args().skip(1);
    let schema_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: validate_addenda <schema.json> <addendum.json>...");
        std::process::exit(2);
    });
    let schema_bytes = fs::read(&schema_path)
        .unwrap_or_else(|error| panic!("cannot read schema {schema_path}: {error}"));
    let schema_value: serde_json::Value = serde_json::from_slice(&schema_bytes)
        .unwrap_or_else(|error| panic!("cannot decode schema {schema_path}: {error}"));
    let mut checked = 0usize;
    for path in args {
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
        let instance: serde_json::Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("cannot decode {path}: {error}"));
        let violations = schema::validate(&schema_value, &instance);
        assert!(
            violations.is_empty(),
            "{path}: schema violations: {violations:#?}"
        );
        let addendum = FamilyAddendum::decode(&bytes)
            .unwrap_or_else(|error| panic!("cannot decode typed addendum {path}: {error}"));
        addendum
            .validate()
            .unwrap_or_else(|errors| panic!("{path}: semantic violations: {errors:#?}"));
        assert!(
            addendum.frozen.frozen_utc.is_some(),
            "{path}: addendum is not frozen"
        );
        println!("PASS schema and typed rules: {path}");
        checked += 1;
    }
    assert!(checked > 0, "no addenda supplied");
}
