use chrono::DateTime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

use crate::logger::log_file;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn ts_diff_ms(from: &str, to: &str) -> Option<i64> {
    if from.is_empty() || to.is_empty() {
        return None;
    }
    let a = DateTime::parse_from_rfc3339(from).ok()?;
    let b = DateTime::parse_from_rfc3339(to).ok()?;
    Some((b - a).num_milliseconds())
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

pub(crate) struct TranscriptStats {
    pub tool_calls: usize,
    pub errors: usize,
    pub error_rate: f64,
    pub output_tokens: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub cache_hit_rate: f64,
}

pub(crate) fn compute_stats_from_lines(lines: &[&str]) -> TranscriptStats {
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

// ---------------------------------------------------------------------------
// Tool call parsing
// ---------------------------------------------------------------------------

pub(crate) fn parse_tools_from_lines(lines: &[&str]) -> Vec<Value> {
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

    // Pass 2: compute ctx_added — delta between next assistant ctx and calling assistant ctx.
    let mut ctx_added: HashMap<String, i64> = HashMap::new();
    {
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
            let current_ctx = entry.get("message").and_then(|m| m.get("usage")).map(|u| {
                ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                    .iter()
                    .map(|k| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0))
                    .sum::<u64>()
            }).unwrap_or(0) as i64;
            let next_ctx = parsed[i + 1..].iter()
                .find(|(t, _)| t == "assistant")
                .and_then(|(_, e)| e.get("message").and_then(|m| m.get("usage")))
                .map(|u| {
                    ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                        .iter()
                        .map(|k| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0))
                        .sum::<u64>()
                }).unwrap_or(0) as i64;
            let added = next_ctx - current_ctx;
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

// ---------------------------------------------------------------------------
// Turn-scoped extractors
// ---------------------------------------------------------------------------

pub(crate) fn extract_last_turn_tools(lines: &[&str]) -> Vec<Value> {
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

pub(crate) fn extract_turn_stats(lines: &[&str]) -> (Option<u64>, Option<u64>) {
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

pub(crate) fn extract_last_usage(lines: &[&str]) -> Option<Value> {
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

pub(crate) fn extract_total_usage(lines: &[&str]) -> Option<Value> {
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

// ---------------------------------------------------------------------------
// Log-file lookups
// ---------------------------------------------------------------------------

pub(crate) fn find_agent_start_ts(agent_id: &str) -> Option<String> {
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

pub(crate) fn count_session_subagents(session_id: &str) -> usize {
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

// ---------------------------------------------------------------------------
// Enrichment dispatcher
// ---------------------------------------------------------------------------

pub(crate) fn enrich_input(event: &str, input: &Value, now_ts: &str) -> Value {
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
