#[allow(dead_code)]
pub mod hooks;
#[allow(dead_code)]
pub mod tools;
#[allow(dead_code)]
pub mod transcript;

use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

// ─── Embedded JSON Schema files (compiled into the binary) ───

static EVENT_SCHEMAS: LazyLock<HashMap<&'static str, Value>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // _common is referenced by allOf/$ref but not used directly as an event schema.
    m.insert("_common", serde_json::from_str(include_str!("../../schemas/events/_common.json")).unwrap());
    m.insert("SessionStart", serde_json::from_str(include_str!("../../schemas/events/SessionStart.json")).unwrap());
    m.insert("SessionEnd", serde_json::from_str(include_str!("../../schemas/events/SessionEnd.json")).unwrap());
    m.insert("UserPromptSubmit", serde_json::from_str(include_str!("../../schemas/events/UserPromptSubmit.json")).unwrap());
    m.insert("PreToolUse", serde_json::from_str(include_str!("../../schemas/events/PreToolUse.json")).unwrap());
    m.insert("PostToolUse", serde_json::from_str(include_str!("../../schemas/events/PostToolUse.json")).unwrap());
    m.insert("PostToolUseFailure", serde_json::from_str(include_str!("../../schemas/events/PostToolUseFailure.json")).unwrap());
    m.insert("Stop", serde_json::from_str(include_str!("../../schemas/events/Stop.json")).unwrap());
    m.insert("StopFailure", serde_json::from_str(include_str!("../../schemas/events/StopFailure.json")).unwrap());
    m.insert("PermissionRequest", serde_json::from_str(include_str!("../../schemas/events/PermissionRequest.json")).unwrap());
    m.insert("SubagentStart", serde_json::from_str(include_str!("../../schemas/events/SubagentStart.json")).unwrap());
    m.insert("SubagentStop", serde_json::from_str(include_str!("../../schemas/events/SubagentStop.json")).unwrap());
    m.insert("Notification", serde_json::from_str(include_str!("../../schemas/events/Notification.json")).unwrap());
    m.insert("ConfigChange", serde_json::from_str(include_str!("../../schemas/events/ConfigChange.json")).unwrap());
    m.insert("InstructionsLoaded", serde_json::from_str(include_str!("../../schemas/events/InstructionsLoaded.json")).unwrap());
    m.insert("Elicitation", serde_json::from_str(include_str!("../../schemas/events/Elicitation.json")).unwrap());
    m.insert("ElicitationResult", serde_json::from_str(include_str!("../../schemas/events/ElicitationResult.json")).unwrap());
    m.insert("TeammateIdle", serde_json::from_str(include_str!("../../schemas/events/TeammateIdle.json")).unwrap());
    m.insert("TaskCompleted", serde_json::from_str(include_str!("../../schemas/events/TaskCompleted.json")).unwrap());
    m.insert("PreCompact", serde_json::from_str(include_str!("../../schemas/events/PreCompact.json")).unwrap());
    m.insert("PostCompact", serde_json::from_str(include_str!("../../schemas/events/PostCompact.json")).unwrap());
    m.insert("WorktreeCreate", serde_json::from_str(include_str!("../../schemas/events/WorktreeCreate.json")).unwrap());
    m.insert("WorktreeRemove", serde_json::from_str(include_str!("../../schemas/events/WorktreeRemove.json")).unwrap());
    m
});

/// Validate an event payload against its JSON Schema.
/// Returns warnings (schema violations). Empty = clean.
pub fn validate_event(event: &str, raw: &str) -> Vec<String> {
    let schema_value = match EVENT_SCHEMAS.get(event) {
        Some(s) => s,
        None => return vec![format!("no schema for event: {event}")],
    };

    let instance: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => return vec![format!("{event}: invalid JSON: {e}")],
    };

    // Inline the _common.json required fields check (since $ref is not resolved by jsonschema crate
    // without a resolver, we do a simple merge: check required common fields + event-specific schema).
    let common = &EVENT_SCHEMAS["_common"];
    let mut warnings = Vec::new();

    // Check common required fields
    if let Some(required) = common.get("required").and_then(|r| r.as_array()) {
        for field in required {
            if let Some(name) = field.as_str() {
                if instance.get(name).is_none() {
                    warnings.push(format!("{event}: missing common field: {name}"));
                }
            }
        }
    }

    // Check event-specific required fields
    if let Some(required) = schema_value.get("required").and_then(|r| r.as_array()) {
        for field in required {
            if let Some(name) = field.as_str() {
                if instance.get(name).is_none() {
                    warnings.push(format!("{event}: missing required field: {name}"));
                }
            }
        }
    }

    // Check types of known properties
    if let Some(props) = schema_value.get("properties").and_then(|p| p.as_object()) {
        for (key, schema_prop) in props {
            // Skip passthrough entries (value is `true`)
            if schema_prop.as_bool() == Some(true) {
                continue;
            }
            if let Some(value) = instance.get(key) {
                if let Some(expected_type) = schema_prop.get("type").and_then(|t| t.as_str()) {
                    let type_ok = match expected_type {
                        "string" => value.is_string(),
                        "boolean" => value.is_boolean(),
                        "number" | "integer" => value.is_number(),
                        "object" => value.is_object(),
                        "array" => value.is_array(),
                        _ => true,
                    };
                    if !type_ok {
                        warnings.push(format!("{event}.{key}: expected {expected_type}, got {}", json_type_name(value)));
                    }
                }
            }
        }
    }

    // Check for unknown fields (additionalProperties: false)
    if schema_value.get("additionalProperties") == Some(&Value::Bool(false)) {
        if let Some(obj) = instance.as_object() {
            let known: std::collections::HashSet<&str> = schema_value
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|p| p.keys().map(|k| k.as_str()).collect())
                .unwrap_or_default();

            let unknown: Vec<&str> = obj.keys()
                .map(|k| k.as_str())
                .filter(|k| !known.contains(k))
                .collect();

            if !unknown.is_empty() {
                warnings.push(format!("{event}: unknown fields: [{}]", unknown.join(", ")));
            }
        }
    }

    warnings.sort();
    warnings.dedup();
    warnings
}

/// Backward-compatible alias — called from hooks.rs
pub fn validate_hook(event: &str, raw: &str) -> Vec<String> {
    validate_event(event, raw)
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ─── Legacy trait (still used by tools/transcript modules) ───

/// Trait for structs with `#[serde(flatten)] extra: HashMap<String, Value>`.
#[allow(dead_code)]
pub trait HasExtra {
    fn extra_fields(&self) -> Vec<String>;
}
