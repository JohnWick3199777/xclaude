use chrono::Local;
use serde_json::{Value, json};
use std::io::{self, Read};
use std::process;

use crate::db;
use crate::logger;
use crate::rpc;
use crate::transcript;

pub(crate) const ALL_HOOKS: &[&str] = &[
    "SessionStart",
    "InstructionsLoaded",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "StopFailure",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "TeammateIdle",
    "TaskCompleted",
    "PreCompact",
    "PostCompact",
    "ConfigChange",
    "WorktreeCreate",
    "WorktreeRemove",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
];

pub(crate) fn run_hook(event: &str) {
    let now_ts = Local::now().to_rfc3339();

    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let input: Value = serde_json::from_str(&buf).unwrap_or(Value::Null);

    let payload = transcript::enrich_input(event, &input, &now_ts);

    logger::write_log(event, &payload);

    db::write_db(event, &payload, &now_ts);

    if let Some(endpoint) = rpc::get_rpc_endpoint() {
        rpc::publish_event_rpc(&endpoint, event, &payload);
    }

    process::exit(0);
}

pub(crate) fn build_hooks_json(bin: &str) -> String {
    let hooks: serde_json::Map<String, Value> = ALL_HOOKS
        .iter()
        .map(|event| {
            let entry = json!([{
                "matcher": "",
                "hooks": [{
                    "type":    "command",
                    "command": format!("{bin} hook {event}"),
                    "timeout": 5,
                    "async":   matches!(*event, "PreToolUse" | "PostToolUse" | "PostToolUseFailure"
                                              | "SubagentStart" | "SubagentStop"
                                              | "PreCompact" | "PostCompact"
                                              | "WorktreeCreate" | "WorktreeRemove"
                                              | "ConfigChange" | "TeammateIdle"
                                              | "TaskCompleted" | "StopFailure"
                                              | "InstructionsLoaded"),
                }]
            }]);
            (event.to_string(), entry)
        })
        .collect();

    serde_json::to_string(&json!({ "hooks": hooks })).unwrap()
}
