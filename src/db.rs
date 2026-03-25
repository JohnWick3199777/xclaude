use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;


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
CREATE TABLE IF NOT EXISTS agents (
    agent_id         TEXT PRIMARY KEY,
    session_id       TEXT REFERENCES sessions(session_id),
    parent_agent_id  TEXT REFERENCES agents(agent_id),
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
CREATE TABLE IF NOT EXISTS tools (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_use_id      TEXT UNIQUE,
    session_id       TEXT REFERENCES sessions(session_id),
    agent_id         TEXT REFERENCES agents(agent_id),
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
    conn.execute_batch("PRAGMA journal_mode=WAL;").ok()?;
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap_or(0);
    if version < 2 {
        // Migrate to v2: drop legacy tables, create sessions/agents/tools schema.
        // Disable FK checks so drops succeed even if FK metadata lingers.
        conn.execute_batch("PRAGMA foreign_keys=OFF;").ok()?;
        conn.execute_batch(
            "DROP TABLE IF EXISTS turns;
             DROP TABLE IF EXISTS prompts;
             DROP TABLE IF EXISTS subagents;
             DROP TABLE IF EXISTS tool_calls;"
        ).ok()?;
        conn.execute_batch(SCHEMA_SQL).ok()?;
        conn.pragma_update(None, "user_version", 2i32).ok()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;").ok()?;
    } else {
        conn.execute_batch("PRAGMA foreign_keys=ON;").ok()?;
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
            // Create main agent (agent_id = session_id, parent_agent_id = NULL)
            conn.execute(
                "INSERT OR IGNORE INTO agents (agent_id, session_id, parent_agent_id, agent_type, started_at) VALUES (?1, ?2, NULL, ?3, ?4)",
                rusqlite::params![sid, sid, "main", now_ts],
            )?;
        }

        "SubagentStart" => {
            let agent_id   = d.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            let agent_type = d.get("agent_type").and_then(|v| v.as_str());
            if !agent_id.is_empty() {
                conn.execute(
                    "INSERT OR IGNORE INTO agents \
                     (agent_id, session_id, parent_agent_id, agent_type, started_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![agent_id, sid, sid, agent_type, now_ts],
                )?;
            }
        }

        "PreToolUse" => {
            let tuid     = d.get("tool_use_id").and_then(|v| v.as_str());
            let name     = d.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
            let input    = d.get("tool_input").unwrap_or(&Value::Null);
            let summary  = input_summary(name, input);
            // agent_id is present for subagent tools; fall back to main agent (= session_id)
            let agent_id = d.get("agent_id").and_then(|v| v.as_str()).unwrap_or(sid);
            conn.execute(
                "INSERT OR IGNORE INTO tools \
                 (tool_use_id, session_id, agent_id, tool_name, input_summary, called_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![tuid, sid, agent_id, name, summary, now_ts],
            )?;
        }

        "PostToolUse" => {
            let tuid    = d.get("tool_use_id").and_then(|v| v.as_str());
            let resp    = d.get("tool_response");
            let is_err  = resp.and_then(|r| r.get("is_error")).and_then(|v| v.as_bool())
                            .unwrap_or(false) as i32;
            let rchars  = resp.map(|r| serde_json::to_string(r).unwrap_or_default().len() as i64);
            conn.execute(
                "UPDATE tools SET returned_at=?1, result_chars=?2, is_error=?3 WHERE tool_use_id=?4",
                rusqlite::params![now_ts, rchars, is_err, tuid],
            )?;
        }

        "SubagentStop" => {
            let agent_id        = d.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            let stats           = d.get("stats");
            let wall_sec        = stats.and_then(|s| s.get("wall_time_ms")).and_then(|v| v.as_i64()).map(|ms| ms / 1000);
            let tool_call_count = stats.and_then(|s| s.get("tool_calls")).and_then(|v| v.as_i64());
            let error_count     = stats.and_then(|s| s.get("errors")).and_then(|v| v.as_i64());
            let output_tokens   = stats.and_then(|s| s.get("output_tokens")).and_then(|v| v.as_i64());
            let cache_hit_rate  = stats.and_then(|s| s.get("cache_hit_rate")).and_then(|v| v.as_f64());
            conn.execute(
                "UPDATE agents SET wall_sec=?1, tool_call_count=?2, error_count=?3, \
                 output_tokens=?4, cache_hit_rate=?5, stopped_at=?6 WHERE agent_id=?7",
                rusqlite::params![wall_sec, tool_call_count, error_count, output_tokens, cache_hit_rate, now_ts, agent_id],
            )?;
        }

        "Stop" => {
            let hit_rate = d.get("cache_hit_rate").and_then(|v| v.as_f64());
            conn.execute(
                "UPDATE sessions SET cache_hit_rate=?1 WHERE session_id=?2",
                rusqlite::params![hit_rate, sid],
            )?;
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

pub(crate) fn print_db_status() {
    let path = db_path();
    let conn = match open_db() {
        Some(c) => c,
        None => {
            eprintln!("[xclaude] could not open db at {}", path.display());
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

    println!("\n=== AGENTS ===");
    println!("{:<36} | {:<36} | {:<10} | {}", "AGENT ID", "PARENT ID", "TYPE", "STARTED AT");
    println!("{:-<110}", "-");
    if let Ok(mut stmt) = conn.prepare("SELECT agent_id, parent_agent_id, agent_type, started_at FROM agents WHERE session_id IN (SELECT session_id FROM sessions WHERE ended_at IS NULL) ORDER BY started_at DESC") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                println!("{:<36} | {:<36} | {:<10} | {}",
                    row.0,
                    row.1.unwrap_or_else(|| "-".to_string()),
                    row.2.unwrap_or_else(|| "-".to_string()),
                    row.3.unwrap_or_else(|| "-".to_string()));
            }
        }
    }

    println!("\n=== LAST 10 TOOLS ===");
    println!("{:<15} | {:<36} | {:<8} | {:<8} | {:<7} | {}", "TOOL", "AGENT ID", "CALLED", "FINISHED", "TOKENS", "STATUS");
    println!("{:-<95}", "-");
    if let Ok(mut stmt) = conn.prepare("SELECT tool_name, agent_id, substr(called_at,12,8), substr(returned_at,12,8), ctx_added, is_error FROM tools ORDER BY called_at DESC LIMIT 10") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        }) {
            for row in rows.flatten() {
                let status = match row.5 {
                    Some(1) => "FAIL",
                    Some(0) => "pass",
                    None    => "-",
                    _       => "-",
                };
                println!("{:<15} | {:<36} | {:<8} | {:<8} | {:<7} | {}",
                    row.0.unwrap_or_else(|| "-".to_string()),
                    row.1.unwrap_or_else(|| "-".to_string()),
                    row.2.unwrap_or_else(|| "-".to_string()),
                    row.3.unwrap_or_else(|| "-".to_string()),
                    row.4.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                    status);
            }
        }
    }

    println!("\nxclaude db saved at: {}", path.display());
}
