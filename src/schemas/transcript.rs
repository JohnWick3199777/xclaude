use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use super::HasExtra;

// ─── Transcript entry types ───

/// An entry in Claude Code's .jsonl transcript file.
/// Dispatched by the `type` field.
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum TranscriptEntry {
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(rename = "progress")]
    Progress(ProgressEntry),
    #[serde(rename = "system")]
    System(SystemEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
}

// ─── Common fields shared by assistant, user, progress, system ───

macro_rules! transcript_entry {
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
            // ── Common transcript fields ──
            pub timestamp: Option<String>,
            pub uuid: Option<String>,
            #[serde(rename = "sessionId")]
            pub session_id: Option<String>,
            pub version: Option<String>,
            pub cwd: Option<String>,
            pub entrypoint: Option<String>,
            #[serde(rename = "gitBranch")]
            pub git_branch: Option<String>,
            #[serde(rename = "isSidechain")]
            pub is_sidechain: Option<bool>,
            #[serde(rename = "userType")]
            pub user_type: Option<String>,
            #[serde(rename = "parentUuid")]
            pub parent_uuid: Option<String>,
            pub slug: Option<String>,

            // ── Entry-specific fields ──
            $(
                $(#[$fmeta])*
                pub $field: $ty,
            )*

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

// ─── Assistant entry ───

transcript_entry! {
    /// An assistant (model) response in the transcript.
    AssistantEntry {
        message: AssistantMessage,
        #[serde(rename = "requestId")]
        request_id: Option<String>,
    }
}

#[derive(Deserialize, Debug)]
pub struct AssistantMessage {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub message_type: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub content: Vec<ContentBlock>,
    pub usage: Option<Usage>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A content block in an assistant message.
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(default)]
        caller: Option<Value>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
}

/// Token usage reported on each assistant message.
#[derive(Deserialize, Debug)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub service_tier: Option<String>,
    pub inference_geo: Option<String>,
    pub cache_creation: Option<CacheCreation>,
    pub server_tool_use: Option<ServerToolUse>,
    pub iterations: Option<Vec<Value>>,
    pub speed: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct CacheCreation {
    pub ephemeral_1h_input_tokens: Option<u64>,
    pub ephemeral_5m_input_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct ServerToolUse {
    pub web_search_requests: Option<u64>,
    pub web_fetch_requests: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── User entry ───

transcript_entry! {
    /// A user message or tool result in the transcript.
    UserEntry {
        message: UserMessage,
        #[serde(rename = "promptId")]
        prompt_id: Option<String>,
        #[serde(rename = "permissionMode")]
        permission_mode: Option<String>,
        #[serde(rename = "isMeta")]
        is_meta: Option<bool>,
        #[serde(rename = "sourceToolAssistantUUID")]
        source_tool_assistant_uuid: Option<String>,
        #[serde(rename = "toolUseResult")]
        tool_use_result: Option<Value>,
    }
}

/// User message content — either a raw string or array of content blocks.
#[derive(Deserialize, Debug)]
pub struct UserMessage {
    pub role: Option<String>,
    pub content: UserContent,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// User content is either a plain string (human prompt) or array (tool results).
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Blocks(Vec<UserContentBlock>),
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum UserContentBlock {
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: UserResultContent,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(rename = "text")]
    Text { text: String },
}

/// Tool result content — can be a string or structured.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum UserResultContent {
    Text(String),
    Blocks(Vec<Value>),
}

// ─── Progress entry ───

transcript_entry! {
    /// A streaming progress event (hook execution, tool progress, etc.).
    ProgressEntry {
        #[serde(rename = "toolUseID")]
        tool_use_id: Option<String>,
        #[serde(rename = "parentToolUseID")]
        parent_tool_use_id: Option<String>,
        data: Option<ProgressData>,
    }
}

#[derive(Deserialize, Debug)]
pub struct ProgressData {
    #[serde(rename = "type")]
    pub data_type: Option<String>,
    #[serde(rename = "hookEvent")]
    pub hook_event: Option<String>,
    #[serde(rename = "hookName")]
    pub hook_name: Option<String>,
    pub command: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── System entry ───

transcript_entry! {
    /// A system event (stop hook summary, etc.).
    SystemEntry {
        subtype: Option<String>,
        #[serde(rename = "hookCount")]
        hook_count: Option<u64>,
        #[serde(rename = "hookInfos")]
        hook_infos: Option<Vec<HookInfo>>,
        #[serde(rename = "hookErrors")]
        hook_errors: Option<Vec<Value>>,
        #[serde(rename = "preventedContinuation")]
        prevented_continuation: Option<bool>,
        #[serde(rename = "stopReason")]
        stop_reason: Option<String>,
        #[serde(rename = "hasOutput")]
        has_output: Option<bool>,
        level: Option<String>,
        #[serde(rename = "toolUseID")]
        tool_use_id: Option<String>,
    }
}

#[derive(Deserialize, Debug)]
pub struct HookInfo {
    pub command: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── File history snapshot ───

/// File backup snapshot — completely different structure from other entries.
#[derive(Deserialize, Debug)]
pub struct FileHistorySnapshotEntry {
    #[serde(rename = "isSnapshotUpdate")]
    pub is_snapshot_update: Option<bool>,
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    pub snapshot: Option<SnapshotData>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl HasExtra for FileHistorySnapshotEntry {
    fn extra_fields(&self) -> Vec<String> {
        self.extra.keys().cloned().collect()
    }
}

#[derive(Deserialize, Debug)]
pub struct SnapshotData {
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "trackedFileBackups")]
    pub tracked_file_backups: Option<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
