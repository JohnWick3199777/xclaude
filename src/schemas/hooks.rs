use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use super::HasExtra;

// ─── Macro: common hook fields + event-specific fields + extra ───

macro_rules! hook_payload {
    (
        $(#[doc = $doc:literal])*
        $name:ident {
            $(
                $(#[$fmeta:meta])*
                $field:ident : $ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[doc = $doc])*
        #[derive(Deserialize, Debug)]
        #[allow(dead_code)]
        pub struct $name {
            // ── Common fields (present on every hook event) ──
            pub cwd: String,
            pub hook_event_name: String,
            pub session_id: String,
            pub transcript_path: String,

            // ── Event-specific fields ──
            $(
                $(#[$fmeta])*
                pub $field: $ty,
            )*

            // ── Unknown fields gate ──
            #[serde(flatten)]
            pub extra: HashMap<String, Value>,
        }

        impl HasExtra for $name {
            fn extra_fields(&self) -> Vec<String> {
                self.extra.keys().cloned().collect()
            }
        }
    };
}

// ─── Hook event payloads ───

hook_payload! {
    /// Fired once when a session begins.
    SessionStartPayload {
        model: String,
        source: String,
    }
}

hook_payload! {
    /// Fired once when a session ends.
    SessionEndPayload {
        reason: String,
    }
}

hook_payload! {
    /// Fired when the user submits a prompt.
    UserPromptSubmitPayload {
        prompt: String,
        permission_mode: String,
    }
}

hook_payload! {
    /// Fired before a tool is executed.
    PreToolUsePayload {
        tool_name: String,
        tool_use_id: String,
        permission_mode: String,
        /// Tool-specific input — structure varies per tool_name.
        tool_input: Value,
    }
}

hook_payload! {
    /// Fired after a tool completes successfully.
    PostToolUsePayload {
        tool_name: String,
        tool_use_id: String,
        permission_mode: String,
        /// Tool-specific input — structure varies per tool_name.
        tool_input: Value,
        /// Tool-specific response — structure varies per tool_name.
        /// Validated separately via tools::* schemas.
        tool_response: Value,
    }
}

hook_payload! {
    /// Fired after a tool fails.
    PostToolUseFailurePayload {
        tool_name: String,
        tool_use_id: String,
        permission_mode: String,
        tool_input: Value,
        /// Error message from the failed tool.
        error: Option<String>,
    }
}

hook_payload! {
    /// Fired when Claude stops (end of a turn).
    StopPayload {
        last_assistant_message: String,
        permission_mode: String,
        stop_hook_active: bool,
    }
}

hook_payload! {
    /// Fired when a stop hook itself fails.
    StopFailurePayload {
        permission_mode: Option<String>,
        error: Option<String>,
    }
}

hook_payload! {
    /// Fired when a tool requires permission approval.
    PermissionRequestPayload {
        tool_name: String,
        tool_use_id: String,
        permission_mode: String,
        tool_input: Value,
    }
}

hook_payload! {
    /// Fired when a subagent is spawned.
    SubagentStartPayload {
        agent_id: String,
        agent_type: Option<String>,
    }
}

hook_payload! {
    /// Fired when a subagent finishes.
    SubagentStopPayload {
        agent_id: String,
        agent_transcript_path: Option<String>,
    }
}

hook_payload! {
    /// Fired for system notifications (idle, etc.).
    NotificationPayload {
        message: String,
        notification_type: String,
    }
}

hook_payload! {
    /// Fired when a settings file changes.
    ConfigChangePayload {
        source: String,
        file_path: String,
    }
}

hook_payload! {
    /// Fired when CLAUDE.md or memory files are loaded.
    InstructionsLoadedPayload {
        file_path: Option<String>,
        load_reason: String,
        memory_type: Option<String>,
    }
}

hook_payload! {
    /// Fired when Claude asks the user a question.
    ElicitationPayload {
        permission_mode: Option<String>,
    }
}

hook_payload! {
    /// Fired when the user answers an elicitation.
    ElicitationResultPayload {
        permission_mode: Option<String>,
    }
}

hook_payload! {
    /// Fired when a teammate agent becomes idle.
    TeammateIdlePayload {
        agent_id: Option<String>,
    }
}

hook_payload! {
    /// Fired when a task is completed.
    TaskCompletedPayload {
        task_id: Option<String>,
    }
}

hook_payload! {
    /// Fired before/after context compaction.
    CompactPayload {
        permission_mode: Option<String>,
    }
}

hook_payload! {
    /// Fired when a worktree is created/removed.
    WorktreePayload {
        worktree_path: Option<String>,
    }
}
