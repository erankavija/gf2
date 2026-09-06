use serde_json::{json, Value};
use tuning_campaign_support::schema::{validate, SchemaViolation};

fn has_message(violations: &[SchemaViolation], text: &str) -> bool {
    violations
        .iter()
        .any(|violation| violation.message.contains(text))
}

#[test]
fn types_required_and_additional_properties_are_enforced() {
    let schema = json!({
        "type": "object",
        "properties": { "family": { "type": "string" }, "count": { "type": "integer" } },
        "required": ["family", "count"],
        "additionalProperties": false
    });
    let violations = validate(&schema, &json!({ "family": 3, "extra": true }));

    assert!(has_message(
        &violations,
        "missing required property \"count\""
    ));
    assert!(has_message(
        &violations,
        "expected type \"string\", found \"number\""
    ));
    assert!(has_message(
        &violations,
        "additional property \"extra\" is not allowed"
    ));
}

#[test]
fn numeric_bounds_and_enums_are_enforced() {
    let schema = json!({
        "type": "number",
        "minimum": 1,
        "maximum": 3,
        "exclusiveMinimum": 0,
        "exclusiveMaximum": 4,
        "enum": [1, 2, 3]
    });

    let violations = validate(&schema, &json!(0.5));
    assert!(has_message(&violations, "value 0.5 is below minimum 1"));
    assert!(has_message(
        &violations,
        "value is not one of the enum values"
    ));
    assert!(validate(&schema, &json!(2)).is_empty());
    assert!(!validate(&json!({ "const": "ready" }), &json!("waiting")).is_empty());
}

#[test]
fn arrays_refs_and_combinators_are_enforced() {
    let schema = json!({
        "$defs": { "x": { "type": "string", "minLength": 2 } },
        "$ref": "#/$defs/x"
    });
    assert!(validate(&schema, &json!("ok")).is_empty());
    assert!(!validate(&schema, &json!("x")).is_empty());

    let array_schema = json!({
        "type": "array",
        "minItems": 2,
        "maxItems": 3,
        "uniqueItems": true,
        "items": { "type": "integer" }
    });
    let violations = validate(&array_schema, &json!([1, 1]));
    assert!(has_message(&violations, "array items are not unique"));
    assert!(validate(&array_schema, &json!([1, 2])).is_empty());
    assert!(has_message(
        &validate(&array_schema, &json!([1])),
        "fewer than"
    ));
    assert!(!validate(&array_schema, &json!([1, 2, 3, 4])).is_empty());

    let all_of = json!({ "allOf": [{ "type": "number" }, { "minimum": 2 }] });
    assert!(has_message(
        &validate(&all_of, &json!(1)),
        "below minimum 2"
    ));

    let one_of = json!({ "oneOf": [{ "type": "string" }, { "type": "boolean" }] });
    assert!(has_message(
        &validate(&one_of, &json!(1)),
        "oneOf matched 0"
    ));
    let two_matches = json!({ "oneOf": [{ "type": "number" }, { "minimum": 0 }] });
    assert!(has_message(
        &validate(&two_matches, &json!(1)),
        "oneOf matched 2"
    ));

    let any_of = json!({ "anyOf": [{ "type": "string" }, { "type": "boolean" }] });
    assert!(validate(&any_of, &json!(true)).is_empty());
    assert!(has_message(
        &validate(&any_of, &json!(1)),
        "anyOf matched no schemas"
    ));
}

#[test]
fn nullable_type_arrays_accept_null_and_reject_other_types() {
    let schema = json!({ "type": ["number", "null"] });
    assert!(validate(&schema, &Value::Null).is_empty());
    assert!(validate(&schema, &json!(1.5)).is_empty());
    assert!(!validate(&schema, &json!("1.5")).is_empty());
}

#[test]
fn unsupported_keywords_and_dangling_refs_are_reported_not_ignored() {
    let unsupported = validate(&json!({ "type": "string", "pattern": "^x" }), &json!("y"));
    assert!(unsupported.iter().any(|violation| {
        violation.schema_path == "/pattern" && violation.message.contains("unsupported keyword")
    }));

    let dangling = validate(&json!({ "$ref": "#/$defs/missing" }), &json!(null));
    assert!(dangling.iter().any(|violation| {
        violation.schema_path == "/$ref" && violation.message.contains("unknown local $ref")
    }));
}

#[test]
fn violations_are_addressed_by_json_pointer() {
    let schema = json!({
        "type": "object",
        "properties": {
            "cells": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "margins": {
                            "type": "object",
                            "properties": { "improvement": { "type": "number", "minimum": 0 } }
                        }
                    }
                }
            }
        }
    });
    let violations = validate(
        &schema,
        &json!({ "cells": [{ "margins": { "improvement": 1 } }, { "margins": { "improvement": -1 } }] }),
    );
    let violation = violations
        .iter()
        .find(|violation| violation.message.contains("below minimum"))
        .expect("nested numeric bound violation");
    assert_eq!(violation.instance_path, "/cells/1/margins/improvement");
}
