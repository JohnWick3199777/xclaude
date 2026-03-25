use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::transcript::find_agent_start_ts;

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

pub(crate) fn write_db(event: &str, payload: &Value, now_ts: &str) {
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

pub(crate) fn print_live_sessions() {
    let conn = match open_db() {
        Some(c) => c,
        None => {
            eprintln!("[xclaude] could not open db at {}", db_path().display());
            return;
        }
    };

    println!("\n=== SESSIONS ===");
    println!("{:<36} | {:<30} | {}", "SESSION ID", "DIRECTORY", "STARTED AT");
    println!("{:-<95}", "-");
    if let Ok(mut stmt) = conn.prepare("SELECT session_id, cwd, started_at FROM sessions WHERE ended_at IS NULL ORDER BY started_at DESC") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                println!("{:<36} | {:<30} | {}", 
                    row.0, row.1.unwrap_or_else(|| "-".to_string()), row.2.unwrap_or_else(|| "-".to_string()));
            }
        }
    }

    println!("\n=== TOOLS ===");
    println!("{:<38} | {:<36} | {:<36} | {}", "TOOL ID", "AGENT ID", "SESSION ID", "STARTED AT");
    println!("{:-<142}", "-");
    if let Ok(mut stmt) = conn.prepare("SELECT tool_use_id, agent_id, session_id, called_at FROM tool_calls WHERE session_id IN (SELECT session_id FROM sessions WHERE ended_at IS NULL) ORDER BY called_at DESC") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                println!("{:<38} | {:<36} | {:<36} | {}", 
                    row.0, 
                    row.1.unwrap_or_else(|| "-".to_string()), 
                    row.2.unwrap_or_else(|| "-".to_string()), 
                    row.3.unwrap_or_else(|| "-".to_string()));
            }
        }
    }
}
