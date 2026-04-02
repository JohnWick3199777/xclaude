pub use crate::_events::{
    AgentEnd, AgentStart, AgentStatus, SessionEnd, SessionStart, TokenUsage, ToolEnd, ToolStart,
    ToolStatus,
};
use serde::{Deserialize, Serialize};

// ── JSON-RPC 2.0 notification wrapper ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification<T> {
    pub jsonrpc: String,
    pub method: String,
    pub params: T,
}

fn notification<T>(method: &str, params: T) -> Notification<T> {
    Notification { jsonrpc: "2.0".to_string(), method: method.to_string(), params }
}

// ── constructors ─────────────────────────────────────────────────────────────

pub fn session_start(
    session_id: String, timestamp: String, cwd: String, model: String, flags: Vec<String>,
) -> Notification<SessionStart> {
    notification("session.start", SessionStart { session_id, timestamp, cwd, model, flags })
}

pub fn session_end(
    session_id: String, timestamp: String, duration_ms: u64, exit_code: i32,
    total_agents: u32, total_tool_calls: u32, total_tokens: TokenUsage,
) -> Notification<SessionEnd> {
    notification("session.end", SessionEnd {
        session_id, timestamp, duration_ms, exit_code,
        total_agents, total_tool_calls, total_tokens,
    })
}

pub fn agent_start(
    session_id: String, agent_id: String, parent_agent_id: Option<String>,
    timestamp: String, cwd: String, prompt_summary: String,
) -> Notification<AgentStart> {
    notification("agent.start", AgentStart {
        session_id, agent_id, parent_agent_id, timestamp, cwd, prompt_summary,
    })
}

pub fn agent_end(
    session_id: String, agent_id: String, parent_agent_id: Option<String>,
    timestamp: String, duration_ms: u64, status: AgentStatus,
    files_read: Vec<String>, files_written: Vec<String>, tool_calls: u32, tokens: TokenUsage,
) -> Notification<AgentEnd> {
    notification("agent.end", AgentEnd {
        session_id, agent_id, parent_agent_id, timestamp, duration_ms, status,
        files_read, files_written, tool_calls, tokens,
    })
}

pub fn tool_start(
    session_id: String, agent_id: String, tool_call_id: String,
    timestamp: String, tool: String, input: String,
) -> Notification<ToolStart> {
    notification("tool.start", ToolStart { session_id, agent_id, tool_call_id, timestamp, tool, input })
}

pub fn tool_end(
    session_id: String, agent_id: String, tool_call_id: String,
    timestamp: String, duration_ms: u64, tool: String, status: ToolStatus,
    files_written: Vec<String>,
) -> Notification<ToolEnd> {
    notification("tool.end", ToolEnd {
        session_id, agent_id, tool_call_id, timestamp, duration_ms, tool, status, files_written,
    })
}
