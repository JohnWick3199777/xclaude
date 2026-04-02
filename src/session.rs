use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::events::TokenUsage;
use crate::socket::Clients;
use crate::socket::emit;
use crate::events as ev;

// ---------------------------------------------------------------------------
// Claude session file (~/.claude/sessions/<pid>.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSessionFile {
    session_id: String,
    cwd: String,
    /// Epoch milliseconds
    started_at: u64,
}

fn read_session_file(pid: u32) -> Result<ClaudeSessionFile, String> {
    let path = home_dir().join(format!(".claude/sessions/{pid}.json"));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("read {}: {e}", path.display()))?;
            return serde_json::from_str(&raw)
                .map_err(|e| format!("parse {}: {e}", path.display()));
        }
        if Instant::now() >= deadline {
            return Err(format!("session file not found after 3 s: {}", path.display()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn epoch_ms_to_iso(ms: u64) -> String {
    let secs = ms / 1000;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let dt = DateTime::<Utc>::from(UNIX_EPOCH + Duration::new(secs, nanos));
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn extract_model(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--model" {
            return it.next().cloned();
        }
        if let Some(val) = arg.strip_prefix("--model=") {
            return Some(val.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------

/// Reads the session file for `pid`, emits `session.start`, waits for `child`
/// to exit, then emits `session.end`. Returns the process exit code.
pub fn run(clients: &Clients, child: &mut std::process::Child, args: &[String]) -> i32 {
    let pid = child.id();
    let spawn_ms = now_ms();

    let model = extract_model(args).unwrap_or_else(|| "claude-opus-4-6".to_string());
    let flags: Vec<String> = args.iter().filter(|a| a.starts_with("--")).cloned().collect();

    let file = match read_session_file(pid) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("xclaude: {e}");
            return child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        }
    };

    let session_id = file.session_id.clone();
    let agent_id = format!("{session_id}-agent-0");
    let started_at = epoch_ms_to_iso(file.started_at);

    // session.start
    emit(clients, &ev::session_start(
        session_id.clone(),
        started_at.clone(),
        file.cwd.clone(),
        model,
        flags,
    ));

    // agent.start — root agent, lifetime matches the session
    emit(clients, &ev::agent_start(
        session_id.clone(),
        agent_id.clone(),
        None,
        started_at,
        file.cwd,
        String::new(),
    ));

    let exit_code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    let duration_ms = now_ms().saturating_sub(spawn_ms);

    // agent.end
    emit(clients, &ev::agent_end(
        session_id.clone(),
        agent_id,
        None,
        now_iso(),
        duration_ms,
        ev::AgentStatus::Completed,
        vec![],
        vec![],
        0,
        TokenUsage { input: 0, output: 0 },
    ));

    // session.end
    emit(clients, &ev::session_end(
        session_id,
        now_iso(),
        duration_ms,
        exit_code,
        0,
        0,
        TokenUsage { input: 0, output: 0 },
    ));

    exit_code
}
