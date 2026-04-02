# Harness Events

All events are emitted as JSON-RPC 2.0 notifications over the Unix socket.

```json
{
  "jsonrpc": "2.0",
  "method": "<event>",
  "params": { ... }
}
```

---

## ID Model

Every entity has a stable unique ID. Events carry IDs for the entity and all of its ancestors, making each event self-contained and linkable without needing to track state.

| Field            | Description                                               |
|------------------|-----------------------------------------------------------|
| `session_id`     | Unique per Claude Code invocation                         |
| `agent_id`       | Unique per agent (root or sub-agent)                      |
| `parent_agent_id`| ID of the spawning agent, `null` for the root agent       |
| `tool_call_id`   | Unique per tool invocation                                |

---

## Hierarchy

```
session
  └── agent (root)
        ├── tool call
        ├── tool call
        └── sub-agent
              └── tool call
```

---

## Events

### `session.start`

Fired when xclaude launches the `claude` subprocess.

```json
{
  "session_id": "s_01abc",
  "timestamp": "2026-04-01T10:00:00.000Z",
  "context": {
    "cwd": "/home/user/project",
    "model": "claude-opus-4-6",
    "flags": ["--dangerously-skip-permissions"]
  }
}
```

---

### `session.end`

Fired when the `claude` process exits.

```json
{
  "session_id": "s_01abc",
  "timestamp": "2026-04-01T10:05:32.100Z",
  "duration_ms": 332100,
  "exit_code": 0,
  "context": {
    "total_agents": 3,
    "total_tool_calls": 17,
    "total_tokens": { "input": 48200, "output": 6300 }
  }
}
```

---

### `agent.start`

Fired when a new agent (root or sub-agent) begins.

```json
{
  "session_id": "s_01abc",
  "agent_id": "a_02def",
  "parent_agent_id": null,
  "timestamp": "2026-04-01T10:00:01.000Z",
  "context": {
    "cwd": "/home/user/project",
    "prompt_summary": "Refactor the auth module"
  }
}
```

For a sub-agent:

```json
{
  "session_id": "s_01abc",
  "agent_id": "a_03ghi",
  "parent_agent_id": "a_02def",
  "timestamp": "2026-04-01T10:02:10.000Z",
  "context": {
    "cwd": "/home/user/project",
    "prompt_summary": "Write tests for auth/login.ts"
  }
}
```

---

### `agent.end`

Fired when an agent finishes (either completes or errors out).

```json
{
  "session_id": "s_01abc",
  "agent_id": "a_02def",
  "parent_agent_id": null,
  "timestamp": "2026-04-01T10:04:55.000Z",
  "duration_ms": 294000,
  "status": "completed",
  "context": {
    "files_read":    ["src/auth/login.ts", "src/auth/session.ts"],
    "files_written": ["src/auth/login.ts"],
    "tool_calls": 12,
    "tokens": { "input": 31000, "output": 4200 }
  }
}
```

`status` is one of: `completed` | `error` | `cancelled`

---

### `tool.start`

Fired when an agent invokes a tool.

```json
{
  "session_id": "s_01abc",
  "agent_id": "a_02def",
  "tool_call_id": "tc_04jkl",
  "timestamp": "2026-04-01T10:01:15.000Z",
  "tool": "Edit",
  "input": "{\"file_path\": \"src/auth/login.ts\", \"old_string\": \"let token = req.body.token\", \"new_string\": \"const token = req.body.token\"}"
}
```

---

### `tool.end`

Fired when a tool call returns.

```json
{
  "session_id": "s_01abc",
  "agent_id": "a_02def",
  "tool_call_id": "tc_04jkl",
  "timestamp": "2026-04-01T10:01:15.220Z",
  "duration_ms": 220,
  "tool": "Edit",
  "status": "success",
  "context": {
    "files_written": ["src/auth/login.ts"]
  }
}
```

`status` is one of: `success` | `error` | `blocked`

---

## Context Delta

`context` appears on `*.end` events and describes what changed as a result of the entity's execution.

| Field              | Type             | Present on                    |
|--------------------|------------------|-------------------------------|
| `files_read`       | `string[]`       | `agent.end`                   |
| `files_written`    | `string[]`       | `agent.end`, `tool.end`       |
| `tool_calls`       | `number`         | `agent.end`                   |
| `tokens`           | `{input, output}`| `agent.end`                   |
| `total_agents`     | `number`         | `session.end`                 |
| `total_tool_calls` | `number`         | `session.end`                 |
| `total_tokens`     | `{input, output}`| `session.end`                 |
