#[allow(dead_code)]
pub mod hooks;
#[allow(dead_code)]
pub mod tools;
#[allow(dead_code)]
pub mod transcript;

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Validate a hook event payload against its typed schema.
/// Returns warnings (unknown fields, type mismatches). Empty = clean.
pub fn validate_hook(event: &str, raw: &str) -> Vec<String> {
    use hooks::*;

    let mut warnings = match event {
        "SessionStart" => check::<SessionStartPayload>(event, raw),
        "SessionEnd" => check::<SessionEndPayload>(event, raw),
        "UserPromptSubmit" => check::<UserPromptSubmitPayload>(event, raw),
        "PreToolUse" => check::<PreToolUsePayload>(event, raw),
        "PostToolUse" => {
            let mut w = check::<PostToolUsePayload>(event, raw);
            if let Ok(p) = serde_json::from_str::<PostToolUsePayload>(raw) {
                w.extend(validate_tool_response(&p.tool_name, &p.tool_response));
            }
            w
        }
        "Stop" => check::<StopPayload>(event, raw),
        "Notification" => check::<NotificationPayload>(event, raw),
        "ConfigChange" => check::<ConfigChangePayload>(event, raw),
        "InstructionsLoaded" => check::<InstructionsLoadedPayload>(event, raw),
        "SubagentStart" => check::<SubagentStartPayload>(event, raw),
        "SubagentStop" => check::<SubagentStopPayload>(event, raw),
        "PermissionRequest" => check::<PermissionRequestPayload>(event, raw),
        "StopFailure" => check::<StopFailurePayload>(event, raw),
        "PostToolUseFailure" => check::<PostToolUseFailurePayload>(event, raw),
        "Elicitation" => check::<ElicitationPayload>(event, raw),
        "ElicitationResult" => check::<ElicitationResultPayload>(event, raw),
        "TeammateIdle" => check::<TeammateIdlePayload>(event, raw),
        "TaskCompleted" => check::<TaskCompletedPayload>(event, raw),
        "PreCompact" | "PostCompact" => check::<CompactPayload>(event, raw),
        "WorktreeCreate" | "WorktreeRemove" => check::<WorktreePayload>(event, raw),
        _ => vec![format!("no schema for event: {event}")],
    };

    // Deduplicate
    warnings.sort();
    warnings.dedup();
    warnings
}

fn check<T: DeserializeOwned + HasExtra>(event: &str, raw: &str) -> Vec<String> {
    match serde_json::from_str::<T>(raw) {
        Ok(val) => {
            let extra = val.extra_fields();
            if extra.is_empty() {
                vec![]
            } else {
                vec![format!("{event}: unknown fields: [{}]", extra.join(", "))]
            }
        }
        Err(e) => vec![format!("{event}: schema mismatch: {e}")],
    }
}

fn validate_tool_response(tool_name: &str, response: &Value) -> Vec<String> {
    use tools::*;
    match tool_name {
        "Bash" => check_value::<BashResponse>("PostToolUse.tool_response(Bash)", response),
        "Read" => check_value::<ReadResponse>("PostToolUse.tool_response(Read)", response),
        "Glob" => check_value::<GlobResponse>("PostToolUse.tool_response(Glob)", response),
        "Edit" => check_value::<EditResponse>("PostToolUse.tool_response(Edit)", response),
        "Write" => check_value::<WriteResponse>("PostToolUse.tool_response(Write)", response),
        "Grep" => check_value::<GrepResponse>("PostToolUse.tool_response(Grep)", response),
        "Agent" => check_value::<AgentResponse>("PostToolUse.tool_response(Agent)", response),
        _ => vec![], // unknown tool — no schema yet
    }
}

fn check_value<T: DeserializeOwned + HasExtra>(ctx: &str, value: &Value) -> Vec<String> {
    match serde_json::from_value::<T>(value.clone()) {
        Ok(val) => {
            let extra = val.extra_fields();
            if extra.is_empty() {
                vec![]
            } else {
                vec![format!("{ctx}: unknown fields: [{}]", extra.join(", "))]
            }
        }
        Err(e) => vec![format!("{ctx}: schema mismatch: {e}")],
    }
}

/// Trait for structs with `#[serde(flatten)] extra: HashMap<String, Value>`.
pub trait HasExtra {
    fn extra_fields(&self) -> Vec<String>;
}
