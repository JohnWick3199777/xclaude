use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;
use serde_json::Value;

use crate::events::{self as ev, AgentStatus, TokenUsage, ToolStatus};
use crate::socket::{emit, Clients};

// ---------------------------------------------------------------------------
// Hook socket path and settings generation
// ---------------------------------------------------------------------------

pub fn hook_socket_path(pid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/xclaude-hook-{pid}.sock"))
}

/// Generate a settings.json that registers all xclaude hooks pointing at the
/// hook receiver socket. Claude Code merges this with any existing settings
/// when passed via --settings.
pub fn generate_settings(hook_sock: &Path) -> String {
    let sock = hook_sock.display().to_string();
    // Inline Python one-liner: read stdin, forward to hook socket, exit 0.
    // Exit code 0 is required so Claude doesn't treat the hook as an error.
    let cmd = format!(
        "python3 -c \"import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect('{sock}'); s.sendall(sys.stdin.buffer.read()); s.close()\""
    );
    let entry = serde_json::json!([{"matcher": "", "hooks": [{"type": "command", "command": cmd}]}]);
    serde_json::json!({
        "hooks": {
            "PreToolUse":    entry,
            "PostToolUse":   entry,
            "SubagentStart": entry,
            "SubagentStop":  entry,
        }
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Hook payload deserialization
// ---------------------------------------------------------------------------

/// The JSON Claude Code sends to every hook command on stdin.
#[derive(Debug, Deserialize)]
struct HookPayload {
    session_id: Option<String>,
    hook_event_name: Option<String>,
    tool_name: Option<String>,
    tool_use_id: Option<String>,
    tool_input: Option<Value>,
    tool_response: Option<Value>,
    cwd: Option<String>,
}

// ---------------------------------------------------------------------------
// Receiver state
// ---------------------------------------------------------------------------

struct ReceiverState {
    clients: Clients,
    /// Start instant keyed by tool_use_id — used to compute tool duration_ms.
    tool_starts: HashMap<String, Instant>,
    /// Stack of (agent_id, start_instant) for active subagents.
    agent_stack: Vec<(String, Instant)>,
    agent_counter: u32,
}

impl ReceiverState {
    fn new(clients: Clients) -> Self {
        Self {
            clients,
            tool_starts: HashMap::new(),
            agent_stack: Vec::new(),
            agent_counter: 0,
        }
    }

    /// The currently active agent: top of the subagent stack, or the session
    /// itself when no subagent is running.
    fn current_agent_id(&self, session_id: &str) -> String {
        self.agent_stack
            .last()
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| session_id.to_string())
    }

    fn handle(&mut self, payload: HookPayload) {
        let session_id = match payload.session_id {
            Some(ref id) => id.clone(),
            None => return,
        };
        let now = now_iso();

        match payload.hook_event_name.as_deref().unwrap_or("") {
            "PreToolUse" => {
                let tool = payload.tool_name.unwrap_or_default();
                let tool_use_id = payload.tool_use_id.unwrap_or_default();
                let input = payload.tool_input.as_ref().map(Value::to_string).unwrap_or_default();
                let agent_id = self.current_agent_id(&session_id);
                self.tool_starts.insert(tool_use_id.clone(), Instant::now());
                emit(&self.clients, &ev::tool_start(
                    session_id,
                    agent_id,
                    tool_use_id,
                    now,
                    tool,
                    input,
                ));
            }

            "PostToolUse" => {
                let tool = payload.tool_name.unwrap_or_default();
                let tool_use_id = payload.tool_use_id.unwrap_or_default();
                let agent_id = self.current_agent_id(&session_id);
                let duration_ms = self
                    .tool_starts
                    .remove(&tool_use_id)
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                let (status, files_written) = parse_tool_response(
                    &tool,
                    payload.tool_input.as_ref(),
                    payload.tool_response.as_ref(),
                );
                emit(&self.clients, &ev::tool_end(
                    session_id,
                    agent_id,
                    tool_use_id,
                    now,
                    duration_ms,
                    tool,
                    status,
                    files_written,
                ));
            }

            "SubagentStart" => {
                self.agent_counter += 1;
                let agent_id = format!("{session_id}-agent-{}", self.agent_counter);
                let parent_agent_id = self.agent_stack.last().map(|(id, _)| id.clone());
                let cwd = payload.cwd.unwrap_or_default();
                self.agent_stack.push((agent_id.clone(), Instant::now()));
                emit(&self.clients, &ev::agent_start(
                    session_id,
                    agent_id,
                    parent_agent_id,
                    now,
                    cwd,
                    String::new(),
                ));
            }

            "SubagentStop" => {
                if let Some((agent_id, started)) = self.agent_stack.pop() {
                    let parent_agent_id = self.agent_stack.last().map(|(id, _)| id.clone());
                    let duration_ms = started.elapsed().as_millis() as u64;
                    emit(&self.clients, &ev::agent_end(
                        session_id,
                        agent_id,
                        parent_agent_id,
                        now,
                        duration_ms,
                        AgentStatus::Completed,
                        vec![],
                        vec![],
                        0,
                        TokenUsage { input: 0, output: 0 },
                    ));
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tool response parsing
// ---------------------------------------------------------------------------

/// Determine the tool status and collect any files written from the tool
/// input/response. Only Write, Edit, and MultiEdit produce written files.
fn parse_tool_response(
    tool: &str,
    input: Option<&Value>,
    response: Option<&Value>,
) -> (ToolStatus, Vec<String>) {
    let status = response
        .and_then(|r| r.get("success"))
        .and_then(|v| v.as_bool())
        .map(|ok| if ok { ToolStatus::Success } else { ToolStatus::Error })
        .unwrap_or(ToolStatus::Success);

    let mut files_written = Vec::new();
    match tool {
        "Write" | "Edit" => {
            if let Some(path) = input
                .and_then(|v| v.get("file_path"))
                .and_then(|v| v.as_str())
            {
                files_written.push(path.to_string());
            }
        }
        "MultiEdit" => {
            if let Some(edits) = input.and_then(|v| v.get("edits")).and_then(|v| v.as_array()) {
                for edit in edits {
                    if let Some(path) = edit.get("file_path").and_then(|v| v.as_str()) {
                        files_written.push(path.to_string());
                    }
                }
            }
        }
        _ => {}
    }

    (status, files_written)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_iso() -> String {
    use chrono::{SecondsFormat, Utc};
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Bind the hook receiver socket and spawn a background thread that reads
/// incoming hook payloads and emits the corresponding xclaude events.
pub fn start_receiver(hook_sock: PathBuf, clients: Clients) {
    if hook_sock.exists() {
        let _ = std::fs::remove_file(&hook_sock);
    }
    let listener = UnixListener::bind(&hook_sock).unwrap_or_else(|e| {
        eprintln!("xclaude: failed to bind hook socket {}: {e}", hook_sock.display());
        std::process::exit(1);
    });

    std::thread::spawn(move || {
        let mut state = ReceiverState::new(clients);
        for stream in listener.incoming() {
            match stream {
                Ok(mut s) => {
                    let mut buf = String::new();
                    if s.read_to_string(&mut buf).is_ok() {
                        match serde_json::from_str::<HookPayload>(&buf) {
                            Ok(payload) => state.handle(payload),
                            Err(e) => eprintln!("xclaude: hook parse error: {e}\n  raw: {buf}"),
                        }
                    }
                }
                Err(e) => eprintln!("xclaude: hook accept error: {e}"),
            }
        }
    });
}
