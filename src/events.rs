use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Completed,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolStatus {
    Success,
    Error,
    Blocked,
}

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

// ---------------------------------------------------------------------------
// Event constructors — one function per event
// ---------------------------------------------------------------------------

fn notification<T>(method: &str, params: T) -> Notification<T> {
    Notification {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
    }
}

pub fn session_start(
    session_id: String,
    timestamp: String,
    cwd: String,
    model: String,
    flags: Vec<String>,
) -> Notification<SessionStart> {
    notification("session.start", SessionStart {
        session_id,
        timestamp,
        context: SessionStartContext { cwd, model, flags },
    })
}

pub fn session_end(
    session_id: String,
    timestamp: String,
    duration_ms: u64,
    exit_code: i32,
    total_agents: u32,
    total_tool_calls: u32,
    total_tokens: TokenUsage,
) -> Notification<SessionEnd> {
    notification("session.end", SessionEnd {
        session_id,
        timestamp,
        duration_ms,
        exit_code,
        context: SessionEndContext { total_agents, total_tool_calls, total_tokens },
    })
}

pub fn agent_start(
    session_id: String,
    agent_id: String,
    parent_agent_id: Option<String>,
    timestamp: String,
    cwd: String,
    prompt_summary: String,
) -> Notification<AgentStart> {
    notification("agent.start", AgentStart {
        session_id,
        agent_id,
        parent_agent_id,
        timestamp,
        context: AgentStartContext { cwd, prompt_summary },
    })
}

pub fn agent_end(
    session_id: String,
    agent_id: String,
    parent_agent_id: Option<String>,
    timestamp: String,
    duration_ms: u64,
    status: AgentStatus,
    files_read: Vec<String>,
    files_written: Vec<String>,
    tool_calls: u32,
    tokens: TokenUsage,
) -> Notification<AgentEnd> {
    notification("agent.end", AgentEnd {
        session_id,
        agent_id,
        parent_agent_id,
        timestamp,
        duration_ms,
        status,
        context: AgentEndContext { files_read, files_written, tool_calls, tokens },
    })
}

pub fn tool_start(
    session_id: String,
    agent_id: String,
    tool_call_id: String,
    timestamp: String,
    tool: String,
    input: String,
) -> Notification<ToolStart> {
    notification("tool.start", ToolStart {
        session_id,
        agent_id,
        tool_call_id,
        timestamp,
        tool,
        input,
    })
}

pub fn tool_end(
    session_id: String,
    agent_id: String,
    tool_call_id: String,
    timestamp: String,
    duration_ms: u64,
    tool: String,
    status: ToolStatus,
    files_written: Vec<String>,
) -> Notification<ToolEnd> {
    notification("tool.end", ToolEnd {
        session_id,
        agent_id,
        tool_call_id,
        timestamp,
        duration_ms,
        tool,
        status,
        context: ToolEndContext { files_written },
    })
}
