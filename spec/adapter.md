# Agentic Adapter Spec

An **adapter** is a producer process that observes a specific AI agent runtime (Claude, Gemini, Codex, …) and emits a normalized stream of agentic events over a local socket as JSON-RPC 2.0 notifications.

Each adapter:
- Is tool-specific (`xclaude`, `xgemini`, `xcodex`, …)
- Owns its own socket (no shared bus at this layer)
- Is responsible for enrichment — the events it emits should be as complete as the runtime allows
- Has no opinion on what consumers do with the events

---

## Terminology

### Session
The top-level lifecycle unit. A session begins when the user starts the agent runtime and ends when it exits. All activity — turns, tool calls, subagents — belongs to exactly one session. A session has a single stable `session_id`. Bounded by `session.start` and `session.end`.

### Agent
The execution unit within a session. The **main agent** is created at session start. A session may also spawn one or more **subagents** — isolated agent instances delegated a subtask, each with their own `agent_id`. Subagents may themselves spawn further subagents (a tree rooted at the main agent). Every event in the stream is attributed to a specific agent via `agent_id`. Bounded by `agent.start` and `agent.end`.

### Turn
One prompt→response cycle. A turn begins when input is submitted (by the user or programmatically) and ends when the agent has finished responding. Within a turn an agent may invoke zero or more tools. Turns are sequential within a given agent; subagents each have their own turn sequence. A turn has a stable `turn_id` that links `turn.start`, `tool.*`, and `turn.end` events together. Bounded by `turn.start` and `turn.end`.

### Tool
A discrete capability the agent can invoke during a turn — file reads, web searches, code execution, etc. A tool has a stable `tool_id` that identifies what the tool is (e.g. `"bash"`, `"read_file@2.1.0"`), independent of any specific invocation. Each individual invocation additionally has a unique `tool_use_id`. A tool call always produces a `tool.end` event — `error` is `null` on success and populated on failure. The pair `(turn_id, tool_use_id)` uniquely identifies a call within a session. Bounded by `tool.start` and `tool.end`.

---

## Transport

- **Protocol**: JSON-RPC 2.0, notifications only (`id` is absent)
- **Socket**: Unix socket (default) or TCP — configured per adapter
- **Framing**: newline-delimited JSON (`\n` after each message)
- **Direction**: adapter → consumer (unidirectional)

---

## Envelope

Every event follows this envelope:

```json
{
  "jsonrpc": "2.0",
  "method": "<event-name>",
  "params": {
    "ts":         "<RFC3339>",
    "adapter":    "<string>",
    "session_id": "<string>",
    "agent_id":   "<string>",
    "data":       { }
  }
}
```

| Field | Type | Description |
|---|---|---|
| `ts` | RFC3339 string | When the event was emitted |
| `adapter` | string | Adapter identity — `"claude"`, `"gemini"`, etc. |
| `session_id` | string | Unique ID for the top-level session |
| `agent_id` | string | ID of the agent that produced the event (main agent or subagent) |
| `data` | object | Event-specific payload (see below) |

---

## Common Sub-Schemas

### TokenUsage
```json
{
  "input":       0,
  "output":      0,
  "cache_read":  0,
  "cache_write": 0
}
```

### Runtime
```json
{
  "started_at":  "<RFC3339>",
  "ended_at":    "<RFC3339>",
  "duration_ms": 0
}
```

### Context
```json
{
  "window":   0,
  "used":     0,
  "percent":  0.0
}
```
`window` and `used` are token counts. `percent` = used / window × 100.

---

## Event Catalog

### `session.start`

The session has started.

```json
{
  "model":   "string",
  "cwd":     "string"
}
```

---

### `session.end`

The session has ended.

```json
{
  "reason":      "string",
  "runtime":     { Runtime },
  "token_usage": { TokenUsage }
}
```

`reason` values: `"end_of_turn"`, `"user_exit"`, `"error"`, `"timeout"`, `"other"`

---

### `agent.start`

An agent has started. Emitted for both the main agent (at session open) and any subagent spawned during the session.

```json
{
  "parent_agent_id": "string | null",
  "agent_type":      "string"
}
```

`parent_agent_id` is `null` for the main agent. `agent_type` is adapter-defined (e.g. `"main"`, `"task"`, `"code_execution"`).

---

### `agent.end`

An agent has finished.

```json
{
  "parent_agent_id": "string | null",
  "runtime":         { Runtime },
  "token_usage":     { TokenUsage },
  "tool_call_count": 0,
  "error_count":     0
}
```

`parent_agent_id` is `null` for the main agent.

---

### `turn.start`

A new turn has begun. Input was submitted to the agent — by the user, programmatically, or as a continuation.

```json
{
  "turn_id":   "string",
  "content":   "string | null",
  "initiator": "string"
}
```

`content` is `null` when the turn is initiated programmatically or as a continuation without explicit input. `initiator` values: `"user"`, `"system"`, `"agent"` (agent-initiated continuation)

---

### `turn.end`

The agent finished the turn — all tool calls complete, response delivered.

```json
{
  "turn_id":         "string",
  "runtime":         { Runtime },
  "token_usage":     { TokenUsage },
  "context":         { Context },
  "tool_call_count": 0,
  "error_count":     0
}
```

---

### `tool.start`

A tool invocation has begun.

```json
{
  "turn_id":     "string",
  "tool_use_id": "string",
  "tool_id":     "string",
  "tool_name":   "string",
  "input":       { },
  "context":     { Context }
}
```

`input` is the raw tool input object. Adapters may summarize large inputs.

---

### `tool.end`

A tool invocation completed — successfully or not.

```json
{
  "turn_id":     "string",
  "tool_use_id": "string",
  "tool_id":     "string",
  "tool_name":   "string",
  "runtime":     { Runtime },
  "token_usage": { TokenUsage },
  "output":      "string | null",
  "result_size": 0,
  "error":       "string | null"
}
```

`error` is `null` on success. `output` is `null` only when the tool produced no output at all; on failure, runtimes often still return error detail as output text. Adapters may truncate large outputs; `result_size` reflects the original size before truncation.

---

## Field Glossary

| Field | Type | Description |
|---|---|---|
| `session_id` | string | Top-level session ID, stable for the lifetime of one user invocation |
| `agent_id` | string | ID of the agent (main or subagent) that produced the event |
| `turn_id` | string | Links all events within one prompt→response cycle |
| `tool_id` | string | Stable identifier for the tool itself, optionally versioned — e.g. `"bash"`, `"read_file@2.1.0"` |
| `tool_use_id` | string | Unique per tool call; links `tool.start` ↔ `tool.end` |
| `duration_ms` | int | Wall-clock milliseconds |
| `output` | string | Raw tool result; may be truncated by the adapter |
| `result_size` | int | Original output size in characters/bytes, before any truncation |
| `window` | int | Total context window capacity in tokens |
| `used` | int | Tokens currently occupying the context |
| `percent` | float | Context utilization: used / window × 100 |

---

## Adapter Requirements

1. **All envelope fields are required.** If a value is unavailable, use an empty string — never omit the field.
2. **`session.start` must be the first event** of a session; `session.end` must be the last.
3. **`turn.start` must precede `turn.end`** for the same `turn_id`.
4. **`tool.start` must precede `tool.end`** for the same `tool_use_id`.
5. **`agent.start` must precede `agent.end`** for the same `agent_id`.
6. **Enrichment is the adapter's responsibility.** Token counts, context size, durations — if the runtime exposes it, the adapter must compute and include it. Consumers must not need to re-derive it.
7. **Fire-and-forget.** The adapter must never block the agent runtime waiting for consumer acknowledgement.
8. **Extensions are allowed.** Additional fields in `data` are permitted as adapter-specific extensions but must not replace or rename standard fields.
