//! Validator for the JSON Schema subset used by committed protocol schemas.

use serde_json::{Map, Value};

/// One violation, addressed by JSON Pointer into the instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaViolation {
    /// JSON Pointer to the instance location at which the violation occurred.
    pub instance_path: String,
    /// JSON Pointer to the schema location which produced the violation.
    pub schema_path: String,
    /// Short, human-readable explanation of the violation.
    pub message: String,
}

/// Validates `instance` against `schema`, returning every violation in
/// document order (an empty vector means valid). Boolean schemas and the
/// supported draft 2020-12 object keywords are handled without a full schema
/// compiler. `$ref` values must be local `#/$defs/<name>` pointers.
///
/// Unknown references, unsupported keywords, and malformed supported
/// keywords are returned as violations; this function does not panic. Work is
/// linear in the visited schema and instance nodes for an acyclic reference
/// graph.
pub fn validate(schema: &Value, instance: &Value) -> Vec<SchemaViolation> {
    let mut validator = Validator {
        root: schema,
        violations: Vec::new(),
    };
    validator.inspect_schema(schema, "", true);
    validator.visit(schema, instance, "", "", true);
    validator.violations
}

struct Validator<'a> {
    root: &'a Value,
    violations: Vec<SchemaViolation>,
}

impl<'a> Validator<'a> {
    fn visit(
        &mut self,
        schema: &Value,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
        _at_root: bool,
    ) {
        match schema {
            Value::Bool(true) => {}
            Value::Bool(false) => self.violation(
                instance_path,
                schema_path,
                "boolean schema is false".to_owned(),
            ),
            Value::Object(object) => {
                self.visit_ref(object, instance, instance_path);
                if let Some(type_schema) = object.get("type") {
                    self.check_type(
                        type_schema,
                        instance,
                        instance_path,
                        &child_schema(schema_path, "type"),
                    );
                }

                self.visit_enum(object, instance, instance_path, schema_path);
                self.visit_const(object, instance, instance_path, schema_path);
                self.visit_numeric(object, instance, instance_path, schema_path);
                self.visit_object(object, instance, instance_path, schema_path);
                self.visit_array(object, instance, instance_path, schema_path);
                self.visit_string(object, instance, instance_path, schema_path);
                self.visit_combinators(object, instance, instance_path, schema_path);
            }
            _ => self.violation(
                instance_path,
                schema_path,
                "schema must be a boolean or object".to_owned(),
            ),
        }
    }

    fn inspect_schema(&mut self, schema: &Value, schema_path: &str, at_root: bool) {
        let Value::Object(object) = schema else {
            return;
        };
        for keyword in object.keys() {
            if !is_supported_keyword(keyword, at_root) {
                self.violation(
                    "",
                    &child_schema(schema_path, keyword),
                    format!("unsupported keyword {keyword:?}"),
                );
            }
        }
        if let Some(reference) = object.get("$ref") {
            let ref_path = child_schema(schema_path, "$ref");
            match reference.as_str() {
                Some(reference) => {
                    let known = local_def_name(reference)
                        .zip(self.root.get("$defs").and_then(Value::as_object))
                        .is_some_and(|(name, definitions)| definitions.contains_key(&name));
                    if !known {
                        self.violation("", &ref_path, format!("unknown local $ref {reference:?}"));
                    }
                }
                None => self.violation(
                    "",
                    &ref_path,
                    "keyword \"$ref\" must be a string".to_owned(),
                ),
            }
        }
        if at_root {
            if let Some(definitions) = object.get("$defs") {
                if let Some(definitions) = definitions.as_object() {
                    for (name, definition) in definitions {
                        self.inspect_schema(
                            definition,
                            &child_schema(&child_schema(schema_path, "$defs"), name),
                            false,
                        );
                    }
                }
            }
        }
        if let Some(Value::Object(properties)) = object.get("properties") {
            for (name, property) in properties {
                self.inspect_schema(
                    property,
                    &child_schema(&child_schema(schema_path, "properties"), name),
                    false,
                );
            }
        }
        if let Some(additional) = object.get("additionalProperties") {
            if !additional.is_boolean() {
                self.inspect_schema(
                    additional,
                    &child_schema(schema_path, "additionalProperties"),
                    false,
                );
            }
        }
        if let Some(items) = object.get("items") {
            if !items.is_array() {
                self.inspect_schema(items, &child_schema(schema_path, "items"), false);
            }
        }
        for keyword in ["allOf", "oneOf", "anyOf"] {
            if let Some(Value::Array(branches)) = object.get(keyword) {
                for (index, branch) in branches.iter().enumerate() {
                    self.inspect_schema(
                        branch,
                        &child_schema(&child_schema(schema_path, keyword), &index.to_string()),
                        false,
                    );
                }
            }
        }
    }

    fn visit_ref(&mut self, schema: &Map<String, Value>, instance: &Value, instance_path: &str) {
        let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
            return;
        };
        let Some(name) = local_def_name(reference) else {
            return;
        };
        let Some(definitions) = self.root.get("$defs").and_then(Value::as_object) else {
            return;
        };
        let Some(target) = definitions.get(&name) else {
            return;
        };
        let target_path = child_schema(&child_schema("", "$defs"), &name);
        self.visit(target, instance, instance_path, &target_path, false);
    }

    fn check_type(
        &mut self,
        type_schema: &Value,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) -> bool {
        let names: Vec<&str> = match type_schema {
            Value::String(name) => vec![name.as_str()],
            Value::Array(values) if !values.is_empty() => {
                let mut names = Vec::with_capacity(values.len());
                for value in values {
                    match value.as_str() {
                        Some(name) => names.push(name),
                        None => {
                            self.violation(
                                instance_path,
                                schema_path,
                                "type array must contain only strings".to_owned(),
                            );
                            return true;
                        }
                    }
                }
                names
            }
            Value::Array(_) => {
                self.violation(
                    instance_path,
                    schema_path,
                    "type array must not be empty".to_owned(),
                );
                return true;
            }
            _ => {
                self.violation(
                    instance_path,
                    schema_path,
                    "keyword \"type\" must be a string or array".to_owned(),
                );
                return true;
            }
        };

        if names.iter().any(|name| !is_type_name(name)) {
            self.violation(
                instance_path,
                schema_path,
                "type contains an unsupported type name".to_owned(),
            );
            return true;
        }

        if names.iter().any(|name| type_matches(name, instance)) {
            true
        } else {
            let expected = if names.len() == 1 {
                format!("{:#?}", names[0])
            } else {
                format!("{:?}", names)
            };
            self.violation(
                instance_path,
                schema_path,
                format!(
                    "expected type {expected}, found {:?}",
                    json_type_name(instance)
                ),
            );
            false
        }
    }

    fn visit_enum(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(values) = schema.get("enum") else {
            return;
        };
        let path = child_schema(schema_path, "enum");
        let Some(values) = values.as_array() else {
            self.violation(
                instance_path,
                &path,
                "keyword \"enum\" must be an array".to_owned(),
            );
            return;
        };
        if !values.iter().any(|value| json_equal(value, instance)) {
            self.violation(
                instance_path,
                &path,
                "value is not one of the enum values".to_owned(),
            );
        }
    }

    fn visit_const(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(expected) = schema.get("const") else {
            return;
        };
        if !json_equal(expected, instance) {
            self.violation(
                instance_path,
                &child_schema(schema_path, "const"),
                "value does not equal const".to_owned(),
            );
        }
    }

    fn visit_numeric(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(actual) = instance.as_f64() else {
            return;
        };
        for keyword in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
            let Some(bound_value) = schema.get(keyword) else {
                continue;
            };
            let path = child_schema(schema_path, keyword);
            let Some(bound) = bound_value.as_f64() else {
                self.violation(
                    instance_path,
                    &path,
                    format!("keyword {keyword:?} must be numeric"),
                );
                continue;
            };
            let fails = match keyword {
                "minimum" => actual < bound,
                "maximum" => actual > bound,
                "exclusiveMinimum" => actual <= bound,
                _ => actual >= bound,
            };
            if fails {
                let message = match keyword {
                    "minimum" => "below",
                    "maximum" => "above",
                    "exclusiveMinimum" => "at or below",
                    _ => "at or above",
                };
                self.violation(
                    instance_path,
                    &path,
                    format!(
                        "value {} is {} {keyword} {}",
                        number_text(instance),
                        message,
                        number_text(bound_value)
                    ),
                );
            }
        }
    }

    fn visit_object(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(object) = instance.as_object() else {
            return;
        };
        let properties = match schema.get("properties") {
            None => None,
            Some(Value::Object(properties)) => Some(properties),
            Some(_) => {
                self.violation(
                    instance_path,
                    &child_schema(schema_path, "properties"),
                    "keyword \"properties\" must be an object".to_owned(),
                );
                None
            }
        };

        if let Some(required) = schema.get("required") {
            let path = child_schema(schema_path, "required");
            if let Some(required) = required.as_array() {
                for name in required {
                    let Some(name) = name.as_str() else {
                        self.violation(
                            instance_path,
                            &path,
                            "required array must contain only strings".to_owned(),
                        );
                        continue;
                    };
                    if !object.contains_key(name) {
                        self.violation(
                            instance_path,
                            &path,
                            format!("missing required property {name:?}"),
                        );
                    }
                }
            } else {
                self.violation(
                    instance_path,
                    &path,
                    "keyword \"required\" must be an array".to_owned(),
                );
            }
        }

        if let Some(properties) = properties {
            for (name, property_schema) in properties {
                if let Some(value) = object.get(name) {
                    self.visit(
                        property_schema,
                        value,
                        &child_instance(instance_path, name),
                        &child_schema(&child_schema(schema_path, "properties"), name),
                        false,
                    );
                }
            }
        }

        if let Some(additional) = schema.get("additionalProperties") {
            let additional_path = child_schema(schema_path, "additionalProperties");
            for (name, value) in object {
                if properties.is_some_and(|known| known.contains_key(name)) {
                    continue;
                }
                match additional {
                    Value::Bool(true) => {}
                    Value::Bool(false) => self.violation(
                        &child_instance(instance_path, name),
                        &additional_path,
                        format!("additional property {name:?} is not allowed"),
                    ),
                    subschema => self.visit(
                        subschema,
                        value,
                        &child_instance(instance_path, name),
                        &additional_path,
                        false,
                    ),
                }
            }
        }
    }

    fn visit_array(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(array) = instance.as_array() else {
            return;
        };
        if let Some(minimum) = schema.get("minItems") {
            self.check_array_bound(minimum, array.len(), false, instance_path, schema_path);
        }
        if let Some(maximum) = schema.get("maxItems") {
            self.check_array_bound(maximum, array.len(), true, instance_path, schema_path);
        }
        if let Some(items) = schema.get("items") {
            let path = child_schema(schema_path, "items");
            match items {
                Value::Array(_) => self.violation(
                    instance_path,
                    &path,
                    "keyword \"items\" must be a single subschema".to_owned(),
                ),
                _ => {
                    for (index, value) in array.iter().enumerate() {
                        self.visit(
                            items,
                            value,
                            &child_instance(instance_path, &index.to_string()),
                            &path,
                            false,
                        );
                    }
                }
            }
        }
        if let Some(unique) = schema.get("uniqueItems") {
            match unique.as_bool() {
                Some(false) => {}
                Some(true) => {
                    if array.iter().enumerate().any(|(index, value)| {
                        array[..index].iter().any(|prior| json_equal(prior, value))
                    }) {
                        self.violation(
                            instance_path,
                            &child_schema(schema_path, "uniqueItems"),
                            "array items are not unique".to_owned(),
                        );
                    }
                }
                None => self.violation(
                    instance_path,
                    &child_schema(schema_path, "uniqueItems"),
                    "keyword \"uniqueItems\" must be boolean".to_owned(),
                ),
            }
        }
    }

    fn check_array_bound(
        &mut self,
        bound: &Value,
        length: usize,
        is_maximum: bool,
        instance_path: &str,
        schema_path: &str,
    ) {
        let keyword = if is_maximum { "maxItems" } else { "minItems" };
        let path = child_schema(schema_path, keyword);
        let Some(bound) = bound.as_u64() else {
            self.violation(
                instance_path,
                &path,
                format!("keyword {keyword:?} must be a non-negative integer"),
            );
            return;
        };
        let fails = if is_maximum {
            (length as u64) > bound
        } else {
            (length as u64) < bound
        };
        if fails {
            let comparison = if is_maximum {
                "more than"
            } else {
                "fewer than"
            };
            self.violation(
                instance_path,
                &path,
                format!("array has {length} items, {comparison} {keyword} {bound}"),
            );
        }
    }

    fn visit_string(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(string) = instance.as_str() else {
            return;
        };
        for (keyword, is_minimum) in [("minLength", true), ("maxLength", false)] {
            let Some(bound) = schema.get(keyword) else {
                continue;
            };
            let path = child_schema(schema_path, keyword);
            let Some(bound) = bound.as_u64() else {
                self.violation(
                    instance_path,
                    &path,
                    format!("keyword {keyword:?} must be a non-negative integer"),
                );
                continue;
            };
            let length = string.chars().count() as u64;
            if (is_minimum && length < bound) || (!is_minimum && length > bound) {
                let comparison = if is_minimum {
                    "shorter than"
                } else {
                    "longer than"
                };
                self.violation(
                    instance_path,
                    &path,
                    format!("string has {length} characters, {comparison} {keyword} {bound}"),
                );
            }
        }
    }

    fn visit_combinators(
        &mut self,
        schema: &Map<String, Value>,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        if let Some(all_of) = schema.get("allOf") {
            let path = child_schema(schema_path, "allOf");
            match all_of.as_array() {
                Some(branches) => {
                    for (index, branch) in branches.iter().enumerate() {
                        self.visit(
                            branch,
                            instance,
                            instance_path,
                            &child_schema(&path, &index.to_string()),
                            false,
                        );
                    }
                }
                None => self.violation(
                    instance_path,
                    &path,
                    "keyword \"allOf\" must be an array".to_owned(),
                ),
            }
        }
        self.visit_exactly_one(
            schema.get("oneOf"),
            "oneOf",
            instance,
            instance_path,
            schema_path,
        );
        self.visit_at_least_one(
            schema.get("anyOf"),
            "anyOf",
            instance,
            instance_path,
            schema_path,
        );
    }

    fn visit_exactly_one(
        &mut self,
        combinator: Option<&Value>,
        keyword: &str,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(combinator) = combinator else {
            return;
        };
        let path = child_schema(schema_path, keyword);
        let Some(branches) = combinator.as_array() else {
            self.violation(
                instance_path,
                &path,
                format!("keyword {keyword:?} must be an array"),
            );
            return;
        };
        let matches = branches
            .iter()
            .filter(|branch| self.branch_valid(branch, instance, instance_path, &path))
            .count();
        if matches != 1 {
            self.violation(
                instance_path,
                &path,
                format!("{keyword} matched {matches} schemas; exactly one is required"),
            );
        }
    }

    fn visit_at_least_one(
        &mut self,
        combinator: Option<&Value>,
        keyword: &str,
        instance: &Value,
        instance_path: &str,
        schema_path: &str,
    ) {
        let Some(combinator) = combinator else {
            return;
        };
        let path = child_schema(schema_path, keyword);
        let Some(branches) = combinator.as_array() else {
            self.violation(
                instance_path,
                &path,
                format!("keyword {keyword:?} must be an array"),
            );
            return;
        };
        let matches = branches
            .iter()
            .any(|branch| self.branch_valid(branch, instance, instance_path, &path));
        if !matches {
            self.violation(
                instance_path,
                &path,
                format!("{keyword} matched no schemas"),
            );
        }
    }

    fn branch_valid(
        &mut self,
        branch: &Value,
        instance: &Value,
        instance_path: &str,
        combinator_path: &str,
    ) -> bool {
        let original_len = self.violations.len();
        self.visit(branch, instance, instance_path, combinator_path, false);
        let valid = self.violations.len() == original_len;
        self.violations.truncate(original_len);
        valid
    }

    fn violation(&mut self, instance_path: &str, schema_path: &str, message: String) {
        self.violations.push(SchemaViolation {
            instance_path: instance_path.to_owned(),
            schema_path: schema_path.to_owned(),
            message,
        });
    }
}

fn is_supported_keyword(keyword: &str, at_root: bool) -> bool {
    matches!(
        keyword,
        "$ref"
            | "type"
            | "properties"
            | "required"
            | "additionalProperties"
            | "enum"
            | "const"
            | "minimum"
            | "maximum"
            | "exclusiveMinimum"
            | "exclusiveMaximum"
            | "minItems"
            | "maxItems"
            | "items"
            | "uniqueItems"
            | "minLength"
            | "maxLength"
            | "allOf"
            | "oneOf"
            | "anyOf"
            | "$schema"
            | "$id"
            | "title"
            | "description"
            | "$comment"
            | "examples"
            | "default"
    ) || (at_root && keyword == "$defs")
}

fn is_type_name(name: &str) -> bool {
    matches!(
        name,
        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
    )
}

fn type_matches(name: &str, value: &Value) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => {
            value.is_i64()
                || value.is_u64()
                || value.as_f64().is_some_and(|number| number.fract() == 0.0)
        }
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn json_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => match (left.as_f64(), right.as_f64()) {
            (Some(left), Some(right)) => left == right,
            _ => left == right,
        },
        _ => left == right,
    }
}

fn number_text(value: &Value) -> String {
    value.to_string()
}

fn child_instance(parent: &str, token: &str) -> String {
    append_pointer(parent, token)
}

fn child_schema(parent: &str, token: &str) -> String {
    append_pointer(parent, token)
}

fn append_pointer(parent: &str, token: &str) -> String {
    format!("{parent}/{}", escape_pointer_token(token))
}

fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn local_def_name(reference: &str) -> Option<String> {
    let encoded = reference.strip_prefix("#/$defs/")?;
    if encoded.is_empty() || encoded.contains('/') {
        return None;
    }
    let mut decoded = String::with_capacity(encoded.len());
    let mut characters = encoded.chars();
    while let Some(character) = characters.next() {
        if character == '~' {
            match characters.next() {
                Some('0') => decoded.push('~'),
                Some('1') => decoded.push('/'),
                _ => return None,
            }
        } else {
            decoded.push(character);
        }
    }
    Some(decoded)
}
