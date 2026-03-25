use chrono::{DateTime, Local};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{self, Command};

// ---------------------------------------------------------------------------
// All supported Claude Code hook events
// ---------------------------------------------------------------------------
const ALL_HOOKS: &[&str] = &[
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

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------
fn log_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".xclaude").join("logs")
}

fn log_file() -> PathBuf {
    let date = Local::now().format("%Y-%m-%d").to_string();
    log_dir().join(format!("{date}.jsonl"))
}

// ---------------------------------------------------------------------------
// Write one JSONL entry
// ---------------------------------------------------------------------------
fn write_log(event: &str, input: &Value) {
    let dir = log_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[xclaude] failed to create log dir: {e}");
        return;
    }

    let entry = json!({
        "ts":    Local::now().to_rfc3339(),
        "event": event,
        "data":  input,
    });

    let line = serde_json::to_string(&entry).unwrap_or_default();
    let path = log_file();

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            let _ = writeln!(f, "{line}");
        }
        Err(e) => eprintln!("[xclaude] failed to write log: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Publish event via JSON-RPC 2.0 to TCP or Unix Socket
// ---------------------------------------------------------------------------
fn get_rpc_endpoint() -> Option<String> {
    // 1. Try environment variable
    if let Ok(url) = env::var("XCLAUDE_RPC_URL") {
        return Some(url);
    }
    
    // 2. Try ~/.xclaude/config.json
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let path = PathBuf::from(home).join(".xclaude").join("config.json");
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<Value>(&content) {
            if let Some(url) = v.get("rpc_endpoint").and_then(|u| u.as_str()) {
                return Some(url.to_string());
            }
        }
    }
    None
}

fn publish_event_rpc(endpoint: &str, event: &str, input: &Value) {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": event,
        "params": {
            "ts": Local::now().to_rfc3339(),
            "data": input
        },
        "id": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
    });
    
    let payload_str = format!("{}\n", serde_json::to_string(&payload).unwrap());

    // Connect to Unix or TCP socket and fire-and-forget
    #[cfg(unix)]
    if endpoint.starts_with("unix://") {
        use std::os::unix::net::UnixStream;
        use std::time::Duration;
        let path = endpoint.trim_start_matches("unix://");
        if let Ok(mut stream) = UnixStream::connect(path) {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let _ = stream.write_all(payload_str.as_bytes());
        }
        return;
    } 

    if endpoint.starts_with("tcp://") || endpoint.contains(':') {
        use std::net::TcpStream;
        use std::time::Duration;
        let addr = endpoint.trim_start_matches("tcp://");
        if let Ok(mut stream) = TcpStream::connect(addr) {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let _ = stream.write_all(payload_str.as_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// Transcript enrichment helpers
// ---------------------------------------------------------------------------

/// Measure the character length of a tool_result's content field.
fn compute_result_size(item: &Value) -> usize {
    match item.get("content") {
        Some(Value::Array(arr)) => arr.iter()
            .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
            .map(str::len)
            .sum(),
        Some(Value::String(s)) => s.len(),
        _ => 0,
    }
}

/// Compute milliseconds between two RFC 3339 timestamps.
fn ts_diff_ms(from: &str, to: &str) -> Option<i64> {
    if from.is_empty() || to.is_empty() {
        return None;
    }
    let a = DateTime::parse_from_rfc3339(from).ok()?;
    let b = DateTime::parse_from_rfc3339(to).ok()?;
    Some((b - a).num_milliseconds())
}

/// Aggregate stats derived from a transcript.
struct TranscriptStats {
    tool_calls: usize,
    errors: usize,
    error_rate: f64,
    output_tokens: u64,
    cache_creation: u64,
    cache_read: u64,
    cache_hit_rate: f64,
}

/// One-pass scan of transcript lines for token and tool-error counts.
fn compute_stats_from_lines(lines: &[&str]) -> TranscriptStats {
    let mut tool_calls: usize = 0;
    let mut errors: usize = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_creation: u64 = 0;
    let mut cache_read: u64 = 0;

    for line in lines {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        match entry.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                if let Some(u) = entry.get("message").and_then(|m| m.get("usage")) {
                    output_tokens   += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    cache_creation  += u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    cache_read      += u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                }
                if let Some(content) = entry.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
                    for item in content {
                        if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            tool_calls += 1;
                        }
                    }
                }
            }
            Some("user") => {
                if let Some(content) = entry.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
                    for item in content {
                        if item.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                            && item.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)
                        {
                            errors += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let total_cache = cache_creation + cache_read;
    let cache_hit_rate = if total_cache > 0 { cache_read as f64 / total_cache as f64 } else { 0.0 };
    let error_rate = if tool_calls > 0 { errors as f64 / tool_calls as f64 } else { 0.0 };

    TranscriptStats { tool_calls, errors, error_rate, output_tokens, cache_creation, cache_read, cache_hit_rate }
}

/// Parse transcript lines and return enriched tool call list.
/// Each entry: tool_use_id, name, input, call_ts, return_ts, duration_ms, result_size,
///             is_error, context_tokens, ctx_added.
fn parse_tools_from_lines(lines: &[&str]) -> Vec<Value> {
    let mut tool_order: Vec<String> = vec![];
    let mut calls: HashMap<String, serde_json::Map<String, Value>> = HashMap::new();
    let mut results: HashMap<String, serde_json::Map<String, Value>> = HashMap::new();

    // Pass 1: collect tool calls and results.
    for line in lines {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        let ts = entry.get("timestamp").and_then(|t| t.as_str()).unwrap_or("").to_string();

        match entry.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                let usage = entry.get("message").and_then(|m| m.get("usage"));
                let ctx: u64 = usage.map(|u| {
                    ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                        .iter()
                        .map(|k| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0))
                        .sum()
                }).unwrap_or(0);

                let Some(content) = entry
                    .get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array())
                else { continue };

                for item in content {
                    if item.get("type").and_then(|t| t.as_str()) != Some("tool_use") { continue; }
                    let Some(id) = item.get("id").and_then(|v| v.as_str()) else { continue };
                    let id = id.to_string();
                    tool_order.push(id.clone());
                    let mut m = serde_json::Map::new();
                    m.insert("name".into(), item.get("name").cloned().unwrap_or(Value::Null));
                    m.insert("input".into(), item.get("input").cloned().unwrap_or(Value::Null));
                    m.insert("call_ts".into(), Value::String(ts.clone()));
                    m.insert("context_tokens".into(), json!(ctx));
                    calls.insert(id, m);
                }
            }
            Some("user") => {
                let Some(content) = entry
                    .get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array())
                else { continue };

                for item in content {
                    if item.get("type").and_then(|t| t.as_str()) != Some("tool_result") { continue; }
                    let Some(id) = item.get("tool_use_id").and_then(|v| v.as_str()) else { continue };
                    let id = id.to_string();
                    let mut m = serde_json::Map::new();
                    m.insert("return_ts".into(), Value::String(ts.clone()));
                    m.insert("is_error".into(), json!(item.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)));
                    m.insert("result_size".into(), json!(compute_result_size(item)));
                    results.insert(id, m);
                }
            }
            _ => {}
        }
    }

    // Pass 2: compute ctx_added — cache_creation of the next assistant entry after each batch.
    // A "batch" is a set of tool_use items from the same assistant entry (same call_ts).
    let mut ctx_added: HashMap<String, u64> = HashMap::new();
    {
        // Rebuild ordered (type, entry) pairs to find next-assistant boundaries.
        let parsed: Vec<(String, Value)> = lines.iter()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .map(|e| (e.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string(), e))
            .collect();

        for (i, (typ, entry)) in parsed.iter().enumerate() {
            if typ != "assistant" { continue; }
            let ids: Vec<String> = entry
                .get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array())
                .map(|arr| arr.iter()
                    .filter(|it| it.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                    .filter_map(|it| it.get("id").and_then(|v| v.as_str()).map(String::from))
                    .collect())
                .unwrap_or_default();
            if ids.is_empty() { continue; }
            // Find the next assistant entry.
            let added = parsed[i + 1..].iter()
                .find(|(t, _)| t == "assistant")
                .and_then(|(_, e)| e.get("message").and_then(|m| m.get("usage")))
                .and_then(|u| u.get("cache_creation_input_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            for id in ids {
                ctx_added.insert(id, added);
            }
        }
    }

    tool_order.iter().filter_map(|id| {
        let call = calls.get(id)?;
        let result = results.get(id);
        let call_ts   = call.get("call_ts").and_then(|v| v.as_str()).unwrap_or("");
        let return_ts = result.and_then(|r| r.get("return_ts")).and_then(|v| v.as_str()).unwrap_or("");
        Some(json!({
            "tool_use_id":    id,
            "name":           call.get("name"),
            "input":          call.get("input"),
            "call_ts":        if call_ts.is_empty()   { Value::Null } else { json!(call_ts) },
            "return_ts":      if return_ts.is_empty() { Value::Null } else { json!(return_ts) },
            "duration_ms":    ts_diff_ms(call_ts, return_ts),
            "result_size":    result.and_then(|r| r.get("result_size")).cloned().unwrap_or(Value::Null),
            "is_error":       result.and_then(|r| r.get("is_error")).and_then(|v| v.as_bool()).unwrap_or(false),
            "context_tokens": call.get("context_tokens"),
            "ctx_added":      ctx_added.get(id).copied(),
        }))
    }).collect()
}

/// Return only the last turn's tool calls (bounded by `turn_duration` system entries).
fn extract_last_turn_tools(lines: &[&str]) -> Vec<Value> {
    let is_turn_dur = |line: &&str| -> bool {
        serde_json::from_str::<Value>(line).ok()
            .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("system")
                && e.get("subtype").and_then(|s| s.as_str()) == Some("turn_duration"))
            .is_some()
    };

    let ends: Vec<usize> = lines.iter().enumerate()
        .filter(|(_, l)| is_turn_dur(l))
        .map(|(i, _)| i)
        .collect();

    let (start, end) = match ends.len() {
        0 => (0, lines.len()),
        1 => (0, ends[0]),
        n => (ends[n - 2] + 1, ends[n - 1]),
    };

    parse_tools_from_lines(&lines[start..end])
}

/// Return (turn_duration_ms, turn_message_count) from the last `turn_duration` system entry.
fn extract_turn_stats(lines: &[&str]) -> (Option<u64>, Option<u64>) {
    for line in lines.iter().rev() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        if entry.get("type").and_then(|t| t.as_str()) == Some("system")
            && entry.get("subtype").and_then(|s| s.as_str()) == Some("turn_duration")
        {
            return (
                entry.get("durationMs").and_then(|v| v.as_u64()),
                entry.get("messageCount").and_then(|v| v.as_u64()),
            );
        }
    }
    (None, None)
}

/// Return the raw `usage` blob from the last assistant entry in the transcript.
fn extract_last_usage(lines: &[&str]) -> Option<Value> {
    let mut last = None;
    for line in lines {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        if entry.get("type").and_then(|t| t.as_str()) != Some("assistant") { continue; }
        if let Some(u) = entry.get("message").and_then(|m| m.get("usage")) {
            last = Some(u.clone());
        }
    }
    last
}

/// Sum the four standard token fields across all assistant entries.
fn extract_total_usage(lines: &[&str]) -> Option<Value> {
    let mut input: u64 = 0;
    let mut cache_creation: u64 = 0;
    let mut cache_read: u64 = 0;
    let mut output: u64 = 0;
    let mut found = false;

    for line in lines {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        if entry.get("type").and_then(|t| t.as_str()) != Some("assistant") { continue; }
        let Some(u) = entry.get("message").and_then(|m| m.get("usage")) else { continue };
        found = true;
        input          += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        cache_creation += u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        cache_read     += u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        output         += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    }

    if !found { return None; }
    Some(json!({
        "input_tokens":                input,
        "cache_creation_input_tokens": cache_creation,
        "cache_read_input_tokens":     cache_read,
        "output_tokens":               output,
    }))
}

/// Search today's log for the `SubagentStart` timestamp of a given agent_id.
fn find_agent_start_ts(agent_id: &str) -> Option<String> {
    let content = fs::read_to_string(log_file()).ok()?;
    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        if entry.get("event").and_then(|e| e.as_str()) != Some("SubagentStart") { continue; }
        if entry.get("data").and_then(|d| d.get("agent_id")).and_then(|id| id.as_str()) == Some(agent_id) {
            return entry.get("ts").and_then(|t| t.as_str()).map(String::from);
        }
    }
    None
}

/// Count `SubagentStart` events for a given session_id in today's log.
fn count_session_subagents(session_id: &str) -> usize {
    let content = match fs::read_to_string(log_file()) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    content.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|entry| {
            entry.get("event").and_then(|e| e.as_str()) == Some("SubagentStart")
                && entry.get("data").and_then(|d| d.get("session_id")).and_then(|s| s.as_str()) == Some(session_id)
        })
        .count()
}

/// Selectively enrich the hook payload by reading the relevant transcript.
/// `now_ts` is used as the SubagentStop time for wall-time computation.
/// Returns a modified clone; falls back to the original on any error.
fn enrich_input(event: &str, input: &Value, now_ts: &str) -> Value {
    match event {
        "SubagentStop" => {
            let Some(path) = input.get("agent_transcript_path").and_then(|v| v.as_str()) else {
                return input.clone();
            };
            let content = fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().collect();
            let tools = parse_tools_from_lines(&lines);
            let s = compute_stats_from_lines(&lines);
            let wall_time_ms = input.get("agent_id")
                .and_then(|id| id.as_str())
                .and_then(|id| find_agent_start_ts(id))
                .and_then(|start| ts_diff_ms(&start, now_ts));

            let mut out = input.clone();
            let obj = out.as_object_mut().unwrap();
            obj.insert("tools".into(), json!(tools));
            obj.insert("stats".into(), json!({
                "tool_calls":           s.tool_calls,
                "errors":               s.errors,
                "error_rate":           s.error_rate,
                "output_tokens":        s.output_tokens,
                "cache_creation_tokens": s.cache_creation,
                "cache_read_tokens":    s.cache_read,
                "cache_hit_rate":       s.cache_hit_rate,
                "wall_time_ms":         wall_time_ms,
            }));
            out
        }
        "Stop" => {
            let Some(path) = input.get("transcript_path").and_then(|v| v.as_str()) else {
                return input.clone();
            };
            let content = fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().collect();
            let s = compute_stats_from_lines(&lines);

            let mut out = input.clone();
            let obj = out.as_object_mut().unwrap();
            if let Some(u) = extract_last_usage(&lines) {
                obj.insert("usage".into(), u);
            }
            obj.insert("tools".into(), json!(extract_last_turn_tools(&lines)));
            let (dur, msg) = extract_turn_stats(&lines);
            if let Some(d) = dur { obj.insert("turn_duration_ms".into(), json!(d)); }
            if let Some(m) = msg { obj.insert("turn_message_count".into(), json!(m)); }
            obj.insert("cache_hit_rate".into(), json!(s.cache_hit_rate));
            out
        }
        "SessionEnd" => {
            let Some(path) = input.get("transcript_path").and_then(|v| v.as_str()) else {
                return input.clone();
            };
            let content = fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().collect();
            let s = compute_stats_from_lines(&lines);
            let session_id = input.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            let subagents = count_session_subagents(session_id);

            let mut out = input.clone();
            let obj = out.as_object_mut().unwrap();
            if let Some(u) = extract_total_usage(&lines) {
                obj.insert("usage".into(), u);
            }
            obj.insert("stats".into(), json!({
                "tool_calls":    s.tool_calls,
                "errors":        s.errors,
                "error_rate":    s.error_rate,
                "output_tokens": s.output_tokens,
                "cache_hit_rate": s.cache_hit_rate,
                "subagents":     subagents,
            }));
            out
        }
        _ => input.clone(),
    }
}

// ---------------------------------------------------------------------------
// SQLite persistent store
// ---------------------------------------------------------------------------

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    session_id       TEXT PRIMARY KEY,
    slug             TEXT,
    model            TEXT,
    cwd              TEXT,
    started_at       TEXT,
    ended_at         TEXT,
    end_reason       TEXT,
    total_out_tokens INTEGER,
    cache_hit_rate   REAL,
    subagent_count   INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS turns (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id       TEXT REFERENCES sessions(session_id),
    duration_ms      INTEGER,
    message_count    INTEGER,
    output_tokens    INTEGER,
    ts               TEXT
);
CREATE TABLE IF NOT EXISTS prompts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id       TEXT REFERENCES sessions(session_id),
    prompt           TEXT,
    ts               TEXT
);
CREATE TABLE IF NOT EXISTS subagents (
    agent_id         TEXT PRIMARY KEY,
    session_id       TEXT REFERENCES sessions(session_id),
    agent_type       TEXT,
    task_prompt      TEXT,
    wall_sec         INTEGER,
    tool_call_count  INTEGER,
    error_count      INTEGER,
    output_tokens    INTEGER,
    cache_hit_rate   REAL,
    started_at       TEXT,
    stopped_at       TEXT
);
CREATE TABLE IF NOT EXISTS tool_calls (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_use_id      TEXT UNIQUE,
    session_id       TEXT REFERENCES sessions(session_id),
    agent_id         TEXT,
    tool_name        TEXT,
    input_summary    TEXT,
    called_at        TEXT,
    returned_at      TEXT,
    duration_ms      INTEGER,
    result_chars     INTEGER,
    is_error         INTEGER,
    ctx_before       INTEGER,
    ctx_added        INTEGER
);
";

fn db_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".xclaude").join("xclaude.db")
}

fn open_db() -> Option<rusqlite::Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    let conn = rusqlite::Connection::open(&path).ok()?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").ok()?;
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if version < 1 {
        conn.execute_batch(SCHEMA_SQL).ok()?;
        conn.execute_batch("PRAGMA user_version = 1").ok()?;
    }
    Some(conn)
}

/// Produce a short human-readable summary of a tool's input (max 256 chars).
fn input_summary(tool_name: &str, input: &Value) -> String {
    let s = match tool_name {
        "Bash" => input.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "Read" | "Write" | "Edit" => input.get("file_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "Glob" => format!(
            "{}  {}",
            input.get("pattern").and_then(|v| v.as_str()).unwrap_or(""),
            input.get("path").and_then(|v| v.as_str()).unwrap_or(""),
        ).trim().to_string(),
        "Grep" => input.get("pattern").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "Agent" => format!(
            "[{}] {}",
            input.get("subagent_type").and_then(|v| v.as_str()).unwrap_or("?"),
            input.get("prompt").and_then(|v| v.as_str()).unwrap_or(""),
        ),
        _ => serde_json::to_string(input).unwrap_or_default(),
    };
    if s.len() > 256 { s[..256].to_string() } else { s }
}

fn write_db(event: &str, payload: &Value, now_ts: &str) {
    let conn = match open_db() {
        Some(c) => c,
        None => {
            eprintln!("[xclaude] db: could not open {}", db_path().display());
            return;
        }
    };
    if let Err(e) = write_db_inner(event, payload, now_ts, &conn) {
        eprintln!("[xclaude] db write error ({event}): {e}");
    }
}

fn write_db_inner(
    event: &str,
    payload: &Value,
    now_ts: &str,
    conn: &rusqlite::Connection,
) -> rusqlite::Result<()> {
    let d = payload;
    let sid = d.get("session_id").and_then(|v| v.as_str()).unwrap_or("");

    // Ensure a sessions row exists for any event that carries session_id.
    if !sid.is_empty() {
        conn.execute(
            "INSERT OR IGNORE INTO sessions (session_id) VALUES (?1)",
            rusqlite::params![sid],
        )?;
    }

    match event {
        "SessionStart" => {
            let slug  = d.get("slug").and_then(|v| v.as_str());
            let model = d.get("model").and_then(|v| v.as_str());
            let cwd   = d.get("cwd").and_then(|v| v.as_str());
            conn.execute(
                "UPDATE sessions SET slug=?1, model=?2, cwd=?3, started_at=?4 WHERE session_id=?5",
                rusqlite::params![slug, model, cwd, now_ts, sid],
            )?;
        }

        "UserPromptSubmit" => {
            let prompt = d.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            // Skip background-agent task notifications.
            if prompt.trim_start().starts_with("<task-notification>") {
                return Ok(());
            }
            conn.execute(
                "INSERT INTO prompts (session_id, prompt, ts) VALUES (?1, ?2, ?3)",
                rusqlite::params![sid, prompt, now_ts],
            )?;
        }

        "SubagentStop" => {
            let agent_id   = d.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            let agent_type = d.get("agent_type").and_then(|v| v.as_str());
            let stats      = d.get("stats");
            let wall_sec   = stats
                .and_then(|s| s.get("wall_time_ms")).and_then(|v| v.as_i64())
                .map(|ms| ms / 1000);
            let tool_call_count = stats.and_then(|s| s.get("tool_calls")).and_then(|v| v.as_i64());
            let error_count     = stats.and_then(|s| s.get("errors")).and_then(|v| v.as_i64());
            let output_tokens   = stats.and_then(|s| s.get("output_tokens")).and_then(|v| v.as_i64());
            let cache_hit_rate  = stats.and_then(|s| s.get("cache_hit_rate")).and_then(|v| v.as_f64());
            let started_at      = find_agent_start_ts(agent_id);

            conn.execute(
                "INSERT OR REPLACE INTO subagents \
                 (agent_id, session_id, agent_type, wall_sec, tool_call_count, \
                  error_count, output_tokens, cache_hit_rate, started_at, stopped_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    agent_id, sid, agent_type, wall_sec,
                    tool_call_count, error_count, output_tokens,
                    cache_hit_rate, started_at, now_ts
                ],
            )?;

            // Insert per-tool rows for this subagent.
            if let Some(tools) = d.get("tools").and_then(|v| v.as_array()) {
                for t in tools {
                    let tuid     = t.get("tool_use_id").and_then(|v| v.as_str());
                    let name     = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let summary  = input_summary(name, t.get("input").unwrap_or(&Value::Null));
                    let call_ts  = t.get("call_ts").and_then(|v| v.as_str());
                    let ret_ts   = t.get("return_ts").and_then(|v| v.as_str());
                    let dur      = t.get("duration_ms").and_then(|v| v.as_i64());
                    let rchars   = t.get("result_size").and_then(|v| v.as_i64());
                    let is_err   = t.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) as i32;
                    let ctx_bef  = t.get("context_tokens").and_then(|v| v.as_i64());
                    let ctx_add  = t.get("ctx_added").and_then(|v| v.as_i64());
                    conn.execute(
                        "INSERT OR IGNORE INTO tool_calls \
                         (tool_use_id, session_id, agent_id, tool_name, input_summary, \
                          called_at, returned_at, duration_ms, result_chars, \
                          is_error, ctx_before, ctx_added) \
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                        rusqlite::params![
                            tuid, sid, agent_id, name, summary,
                            call_ts, ret_ts, dur, rchars,
                            is_err, ctx_bef, ctx_add
                        ],
                    )?;
                }
            }
        }

        "Stop" => {
            let dur_ms    = d.get("turn_duration_ms").and_then(|v| v.as_i64());
            let msg_count = d.get("turn_message_count").and_then(|v| v.as_i64());
            let out_tok   = d.get("usage").and_then(|u| u.get("output_tokens")).and_then(|v| v.as_i64());
            let hit_rate  = d.get("cache_hit_rate").and_then(|v| v.as_f64());

            conn.execute(
                "UPDATE sessions SET cache_hit_rate=?1 WHERE session_id=?2",
                rusqlite::params![hit_rate, sid],
            )?;
            conn.execute(
                "INSERT INTO turns (session_id, duration_ms, message_count, output_tokens, ts) \
                 VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![sid, dur_ms, msg_count, out_tok, now_ts],
            )?;

            // Insert parent-session tool calls for this turn.
            if let Some(tools) = d.get("tools").and_then(|v| v.as_array()) {
                for t in tools {
                    let tuid    = t.get("tool_use_id").and_then(|v| v.as_str());
                    let name    = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let summary = input_summary(name, t.get("input").unwrap_or(&Value::Null));
                    let call_ts = t.get("call_ts").and_then(|v| v.as_str());
                    let ret_ts  = t.get("return_ts").and_then(|v| v.as_str());
                    let dur     = t.get("duration_ms").and_then(|v| v.as_i64());
                    let rchars  = t.get("result_size").and_then(|v| v.as_i64());
                    let is_err  = t.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) as i32;
                    let ctx_bef = t.get("context_tokens").and_then(|v| v.as_i64());
                    let ctx_add = t.get("ctx_added").and_then(|v| v.as_i64());
                    conn.execute(
                        "INSERT OR IGNORE INTO tool_calls \
                         (tool_use_id, session_id, tool_name, input_summary, \
                          called_at, returned_at, duration_ms, result_chars, \
                          is_error, ctx_before, ctx_added) \
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                        rusqlite::params![
                            tuid, sid, name, summary,
                            call_ts, ret_ts, dur, rchars,
                            is_err, ctx_bef, ctx_add
                        ],
                    )?;
                }
            }
        }

        "SessionEnd" => {
            let reason    = d.get("reason").and_then(|v| v.as_str());
            let out_tok   = d.get("usage").and_then(|u| u.get("output_tokens")).and_then(|v| v.as_i64());
            let hit_rate  = d.get("stats").and_then(|s| s.get("cache_hit_rate")).and_then(|v| v.as_f64());
            let n_agents  = d.get("stats").and_then(|s| s.get("subagents")).and_then(|v| v.as_i64());
            conn.execute(
                "UPDATE sessions SET ended_at=?1, end_reason=?2, total_out_tokens=?3, \
                 cache_hit_rate=?4, subagent_count=?5 WHERE session_id=?6",
                rusqlite::params![now_ts, reason, out_tok, hit_rate, n_agents, sid],
            )?;
        }

        _ => {}
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// `xclaude hook <EVENT>` — called by Claude Code via --settings hooks
// ---------------------------------------------------------------------------
fn run_hook(event: &str) {
    // Capture arrival time before stdin blocks — used for wall-time calculation.
    let now_ts = Local::now().to_rfc3339();

    // Read JSON from stdin (Claude Code pipes it in)
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let input: Value = serde_json::from_str(&buf).unwrap_or(Value::Null);

    // Enrich payload for SubagentStop, Stop, and SessionEnd
    let payload = enrich_input(event, &input, &now_ts);

    // Write to the local JSONL log
    write_log(event, &payload);

    // Write to the SQLite store
    write_db(event, &payload, &now_ts);

    // Publish to the custom RPC endpoint if configured
    if let Some(endpoint) = get_rpc_endpoint() {
        publish_event_rpc(&endpoint, event, &payload);
    }

    // Always exit 0 — we never block Claude
    process::exit(0);
}

// ---------------------------------------------------------------------------
// Build the --settings JSON that injects all hooks
// ---------------------------------------------------------------------------
fn build_hooks_json(bin: &str) -> String {
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

// ---------------------------------------------------------------------------
// Find the real `claude` binary, skipping ourselves
// ---------------------------------------------------------------------------
fn find_real_claude() -> Option<PathBuf> {
    // Resolve our own canonical path (follow symlinks) so we can skip it
    let self_exe = env::current_exe().ok();
    let self_canonical = self_exe
        .as_ref()
        .and_then(|p| fs::canonicalize(p).ok());

    // 1. Search PATH — skip any entry that resolves to us
    let path_var = env::var("PATH").unwrap_or_default();
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join("claude");
        if !candidate.is_file() {
            continue;
        }
        // Skip if it's us (by canonical path)
        if let Some(ref sc) = self_canonical {
            if fs::canonicalize(&candidate).ok().as_ref() == Some(sc) {
                continue;
            }
        }
        return Some(candidate);
    }

    // 2. Fallback: known Claude install locations
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let fallback_dirs = vec![
        // claude's own versioned install dir (latest symlink or numeric version)
        format!("{home}/.local/share/claude"),
        format!("{home}/.local/share/claude/bin"),
        // common npm global locations
        format!("{home}/.npm-global/bin"),
        format!("{home}/.nvm/versions/node/current/bin"),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];

    for dir in &fallback_dirs {
        let candidate = PathBuf::from(dir).join("claude");
        if candidate.is_file() {
            if let Some(ref sc) = self_canonical {
                if fs::canonicalize(&candidate).ok().as_ref() == Some(sc) {
                    continue;
                }
            }
            return Some(candidate);
        }
    }

    // 3. Search ~/.local/share/claude/versions/ for the latest versioned binary
    let versions_dir = PathBuf::from(&home).join(".local/share/claude/versions");
    if let Ok(entries) = fs::read_dir(&versions_dir) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        // Sort descending — latest version last alphabetically works for semver
        versions.sort();
        if let Some(latest) = versions.last() {
            if let Some(ref sc) = self_canonical {
                if fs::canonicalize(latest).ok().as_ref() != Some(sc) {
                    return Some(latest.clone());
                }
            } else {
                return Some(latest.clone());
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Wrapper mode — intercept `claude` and inject hooks
// ---------------------------------------------------------------------------
fn run_wrapper(original_args: Vec<String>) {
    let real_claude = match find_real_claude() {
        Some(p) => p,
        None => {
            eprintln!("[xclaude] Error: could not find real claude binary in PATH");
            process::exit(127);
        }
    };

    // Path to ourselves (so hook commands resolve correctly)
    let self_bin = env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "xclaude".to_string());

    let settings_json = build_hooks_json(&self_bin);

    // Check if user already provided --settings; if so append our hooks differently.
    // For simplicity we always inject ours — Claude Code merges additively.
    // Pass-through subcommands that don't support --settings.
    if let Some(sub) = original_args.first() {
        match sub.as_str() {
            "mcp" | "config" | "api-key" | "rc" | "remote-control" => {
                let err = Command::new(&real_claude).args(&original_args).exec();
                eprintln!("[xclaude] exec failed: {err}");
                process::exit(1);
            }
            _ => {}
        }
    }

    // Build final args: inject --settings <json> then all original args
    let mut args: Vec<String> = vec![
        "--settings".to_string(),
        settings_json,
    ];
    args.extend(original_args);

    let err = Command::new(&real_claude).args(&args).exec();
    eprintln!("[xclaude] exec failed: {err}");
    process::exit(1);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        // xclaude hook <EVENT>  — log the event and exit
        Some("hook") => {
            let event = args.get(1).cloned().unwrap_or_else(|| "Unknown".to_string());
            run_hook(&event);
        }

        // xclaude hooks  — print all hook events we support
        Some("hooks") => {
            for h in ALL_HOOKS {
                println!("{h}");
            }
        }

        // xclaude logs  — live-tail today's log (like tail -f)
        Some("logs") => {
            let path = log_file();

            // Print existing content first
            let mut file = match fs::OpenOptions::new().read(true).open(&path) {
                Ok(f) => f,
                Err(_) => {
                    eprintln!("[xclaude] waiting for log at {} ...", path.display());
                    // File doesn't exist yet — wait for it
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if path.exists() {
                            break;
                        }
                    }
                    fs::OpenOptions::new().read(true).open(&path).expect("could not open log")
                }
            };

            let mut buf = String::new();
            let _ = io::Read::read_to_string(&mut file, &mut buf);
            print!("{buf}");
            let _ = io::Write::flush(&mut io::stdout());

            // Then tail — poll for new bytes
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Re-open each day in case the date rolled over
                let current_path = log_file();
                if current_path != path {
                    // New day — restart with new file
                    eprintln!("[xclaude] new log file: {}", current_path.display());
                    file = match fs::OpenOptions::new().read(true).open(&current_path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                }

                let mut new_buf = String::new();
                let _ = io::Read::read_to_string(&mut file, &mut new_buf);
                if !new_buf.is_empty() {
                    print!("{new_buf}");
                    let _ = io::Write::flush(&mut io::stdout());
                }
            }
        }

        // xclaude logs --pretty  — pretty-print today's log
        Some("pretty") => {
            let path = log_file();
            match fs::read_to_string(&path) {
                Ok(content) => {
                    for line in content.lines() {
                        if let Ok(v) = serde_json::from_str::<Value>(line) {
                            let ts = v["ts"].as_str().unwrap_or("?");
                            let event = v["event"].as_str().unwrap_or("?");
                            let data = &v["data"];
                            println!("[{ts}] {event}");
                            println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
                            println!("---");
                        }
                    }
                }
                Err(_) => println!("[xclaude] no log yet at {}", path.display()),
            }
        }

        // xclaude install  — symlink xclaude as `claude` on PATH
        Some("install") => {
            let self_bin = env::current_exe().expect("cannot find self");
            let home = env::var("HOME").unwrap_or_else(|_| "/usr/local".to_string());
            let bin_dir = PathBuf::from(&home).join(".local").join("bin");
            fs::create_dir_all(&bin_dir).expect("cannot create ~/.local/bin");
            let link = bin_dir.join("claude");
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(&self_bin, &link).expect("symlink failed");
            println!("[xclaude] installed: {} -> {}", link.display(), self_bin.display());
            println!("[xclaude] make sure {} is first in your PATH", bin_dir.display());
        }

        // xclaude <anything else>  — wrapper mode, pass through to real claude
        _ => {
            // Remove leading "xclaude" token if invoked as `xclaude <args>` not as `claude`
            // but if invoked as `claude` (via symlink) args are already clean
            run_wrapper(args);
        }
    }
}
