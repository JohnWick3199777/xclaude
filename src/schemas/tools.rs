use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use super::HasExtra;

macro_rules! tool_response {
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

// ─── PostToolUse tool_response per tool ───

tool_response! {
    /// Bash tool response.
    BashResponse {
        stdout: String,
        stderr: String,
        interrupted: bool,
        #[serde(rename = "isImage")]
        is_image: bool,
        #[serde(rename = "noOutputExpected")]
        no_output_expected: bool,
    }
}

tool_response! {
    /// Read tool response.
    ReadResponse {
        #[serde(rename = "type")]
        response_type: String,
        file: ReadFileInfo,
    }
}

#[derive(Deserialize, Debug)]
pub struct ReadFileInfo {
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub content: String,
    #[serde(rename = "numLines")]
    pub num_lines: u64,
    #[serde(rename = "startLine")]
    pub start_line: u64,
    #[serde(rename = "totalLines")]
    pub total_lines: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl HasExtra for ReadFileInfo {
    fn extra_fields(&self) -> Vec<String> {
        self.extra.keys().cloned().collect()
    }
}

tool_response! {
    /// Glob tool response.
    GlobResponse {
        filenames: Vec<String>,
        #[serde(rename = "numFiles")]
        num_files: u64,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        truncated: bool,
    }
}

tool_response! {
    /// Edit tool response.
    EditResponse {
        /// The file path that was edited.
        #[serde(rename = "filePath")]
        file_path: Option<String>,
        /// Snippet of the edited content.
        snippet: Option<String>,
    }
}

tool_response! {
    /// Write tool response.
    WriteResponse {
        #[serde(rename = "filePath")]
        file_path: Option<String>,
    }
}

tool_response! {
    /// Grep tool response.
    GrepResponse {
        #[serde(rename = "durationMs")]
        duration_ms: Option<u64>,
        #[serde(rename = "numFiles")]
        num_files: Option<u64>,
        truncated: Option<bool>,
    }
}

tool_response! {
    /// Agent (subagent) tool response.
    AgentResponse {
        result: Option<String>,
    }
}

// ─── Enriched tool entry (in Stop/SubagentStop payloads, produced by xclaude) ───

#[derive(Deserialize, Debug)]
pub struct ToolEntry {
    pub tool_use_id: Option<String>,
    pub name: Option<String>,
    pub input: Option<Value>,
    pub call_ts: Option<String>,
    pub return_ts: Option<String>,
    pub duration_ms: Option<i64>,
    pub result_size: Option<i64>,
    pub is_error: Option<bool>,
    pub context_tokens: Option<i64>,
    pub ctx_added: Option<i64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl HasExtra for ToolEntry {
    fn extra_fields(&self) -> Vec<String> {
        self.extra.keys().cloned().collect()
    }
}
