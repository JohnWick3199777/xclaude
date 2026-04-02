# Claude → Harness Events Translation

This document describes how xclaude reads Claude's raw data sources and translates them into harness events.

---

## Data Sources

| Source | Path | What it provides |
|--------|------|-----------------|
| Session metadata | `~/.claude/sessions/<pid>.json` | session start, cwd, entrypoint |
| Transcript | `~/.claude/projects/<cwd>/<session-id>.jsonl` | messages, tool calls, token usage |
| Hooks | injected via `--settings` | real-time pre/post tool signals |
| stdout / stderr | subprocess pipe | raw output, errors |

---

## Session Events

### `session.start`

**Trigger:** xclaude spawns the `claude` subprocess.

**Source:** `~/.claude/sessions/<pid>.json`

```
sessionId   → session_id
startedAt   → timestamp
cwd         → context.cwd
entrypoint  → context.entrypoint
kind        → context.kind
```

---

### `session.end`

**Trigger:** The `claude` subprocess exits.

**Source:** process exit signal + accumulated state

```
sessionId           → session_id
process exit time   → timestamp
exit code           → exit_code
accumulated counts  → context.total_agents, total_tool_calls, total_tokens
```

---

## Agent Events

Agents are identified by `sessionId` in the transcript. A root agent shares the top-level `sessionId`. Sub-agents appear as a new `sessionId` with a `caller` field pointing to the parent.

### `agent.start`

**Trigger:** First transcript entry for a given `sessionId`.

**Source:** First `user` entry in a transcript file

```
sessionId           → session_id
parentSessionId     → parent_agent_id  (null if root agent)
uuid                → agent_id
timestamp           → timestamp
message.content[0]  → context.prompt_summary (first 200 chars)
cwd                 → context.cwd
```

### Sub-agent detection

A sub-agent is identified when a transcript entry has a `caller` field with a non-direct type:

```json
"caller": {
  "type": "subagent",
  "parentSessionId": "<parent-session-uuid>"
}
```

If `caller.type === "direct"` → root agent (`parent_agent_id: null`)
If `caller.type === "subagent"` → sub-agent (`parent_agent_id: caller.parentSessionId`)

> **Note:** Sub-agent caller shape needs to be verified against real sub-agent transcripts.

---

### `agent.end`

**Trigger:** Last transcript entry for a given `sessionId` (session closes or stop_reason is set).

**Source:** Last `assistant` entry with `stop_reason` set

```
sessionId                       → session_id
agent_id                        → agent_id
timestamp                       → timestamp
stop_reason                     → status  (map: "end_turn" → "completed", "error" → "error")
accumulated usage across msgs   → context.tokens
count of tool_use blocks        → context.tool_calls
files touched via tool calls    → context.files_read, context.files_written
```

---

## Tool Events

Tool calls are embedded inside `assistant` message content arrays in the transcript.

### `tool.start`

**Trigger:** A content block of `type: "tool_use"` is observed in an assistant message.

**Source:** `message.content[]` where `type === "tool_use"`

```
sessionId       → session_id
agent_id        → agent_id  (derived from current transcript session)
id              → tool_call_id
timestamp       → timestamp  (parent message timestamp)
name            → tool
JSON.stringify(input) → input
```

---

### `tool.end`

**Trigger:** A content block of `type: "tool_result"` is observed in a subsequent user message.

**Source:** `message.content[]` where `type === "tool_result"`, matched by `tool_use_id`

```
tool_use_id     → tool_call_id  (links back to tool.start)
sessionId       → session_id
timestamp       → timestamp
duration_ms     → derived from tool.start timestamp delta
is_error        → status  (false → "success", true → "error")
files affected  → context.files_written  (inferred from tool name + input)
```

Hook `PostToolUse` can supplement or replace transcript-based `tool.end` with lower latency.

---

## File Context Inference

Since claude does not explicitly log which files were "read" vs "written", xclaude infers this from tool name + input:

| Tool | Infers |
|------|--------|
| `Read` | `files_read += input.file_path` |
| `Edit`, `Write` | `files_written += input.file_path` |
| `Bash` | parse command heuristically (best-effort) |
| `MultiEdit` | `files_written += input.file_path` |

---

## Token Aggregation

Token usage is available per-message in the transcript:

```json
"usage": {
  "input_tokens": 3,
  "cache_creation_input_tokens": 6757,
  "cache_read_input_tokens": 11133,
  "output_tokens": 8
}
```

xclaude sums these across all assistant messages per agent for `agent.end`, and across all agents for `session.end`.

---

## Hook vs Transcript

xclaude has two real-time signal paths:

| Signal | Latency | Detail |
|--------|---------|--------|
| Hooks (`PreToolUse`, `PostToolUse`) | Real-time | Tool name + input/output, no token data |
| Transcript file | Slight delay (file write) | Full message content, token usage, threading |

**Strategy:** Use hooks for low-latency `tool.start` / `tool.end` events. Use transcript for session/agent lifecycle and token aggregation.
