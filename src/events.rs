use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStart {
    pub session_id: String,
    pub timestamp: String,
    pub context: SessionStartContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartContext {
    pub cwd: String,
    pub model: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnd {
    pub session_id: String,
    pub timestamp: String,
    pub duration_ms: u64,
    pub exit_code: i32,
    pub context: SessionEndContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndContext {
    pub total_agents: u32,
    pub total_tool_calls: u32,
    pub total_tokens: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStart {
    pub session_id: String,
    pub agent_id: String,
    pub parent_agent_id: Option<String>,
    pub timestamp: String,
    pub context: AgentStartContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStartContext {
    pub cwd: String,
    pub prompt_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Completed,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEnd {
    pub session_id: String,
    pub agent_id: String,
    pub parent_agent_id: Option<String>,
    pub timestamp: String,
    pub duration_ms: u64,
    pub status: AgentStatus,
    pub context: AgentEndContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndContext {
    pub files_read: Vec<String>,
    pub files_written: Vec<String>,
    pub tool_calls: u32,
    pub tokens: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStart {
    pub session_id: String,
    pub agent_id: String,
    pub tool_call_id: String,
    pub timestamp: String,
    pub tool: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Success,
    Error,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEnd {
    pub session_id: String,
    pub agent_id: String,
    pub tool_call_id: String,
    pub timestamp: String,
    pub duration_ms: u64,
    pub tool: String,
    pub status: ToolStatus,
    pub context: ToolEndContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEndContext {
    pub files_written: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification<T> {
    pub jsonrpc: String,
    pub method: String,
    pub params: T,
}

