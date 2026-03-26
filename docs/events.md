# Claude Code Session Events

An agentic coding session emits **events** — structured JSON payloads fired at well-defined points during the session lifecycle. These events are the formal contract between Claude Code and any wrapper/consumer.

A wrapper (like xclaude) subscribes to these events and routes them to data sinks:

```
Claude Code  ──events──>  Wrapper  ──>  Unix socket (real-time)
                                   ──>  SQLite DB (queryable)
                                   ──>  JSONL log (archival)
```

The events themselves are **transport-agnostic** — a wrapper decides where they go.

## Schema Files

Each event has a JSON Schema definition in `schemas/events/`:

```
schemas/events/
  SessionStart.json
  SessionEnd.json
  UserPromptSubmit.json
  PreToolUse.json
  PostToolUse.json
  PostToolUseFailure.json
  Stop.json
  StopFailure.json
  PermissionRequest.json
  SubagentStart.json
  SubagentStop.json
  Notification.json
  ConfigChange.json
  InstructionsLoaded.json
  Elicitation.json
  ElicitationResult.json
  TeammateIdle.json
  TaskCompleted.json
  PreCompact.json
  PostCompact.json
  WorktreeCreate.json
  WorktreeRemove.json
```

Each schema file is **fully self-contained** — no `$ref` or shared files. All schemas use [JSON Schema draft 2020-12](https://json-schema.org/draft/2020-12/schema) with `additionalProperties: false` to catch schema drift.

## Common Fields

Every event carries these four fields (included in each schema file):

| Field | Type | Description |
|-------|------|-------------|
| `cwd` | string | Working directory of the session |
| `hook_event_name` | string | Name of the event (e.g. `SessionStart`) |
| `session_id` | string | Unique session identifier |
| `transcript_path` | string | Path to the session transcript JSONL file |

## Event Reference

### Session Lifecycle

#### `SessionStart`

Fired once when a session begins.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model used for this session |
| `source` | string | yes | How the session was started (`cli`, `api`, etc.) |

#### `SessionEnd`

Fired once when a session ends.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `reason` | string | yes | Why the session ended |

### User Interaction

#### `UserPromptSubmit`

Fired when the user submits a prompt.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `prompt` | string | yes | The user's prompt text |
| `permission_mode` | string | yes | Current permission mode |

#### `Elicitation`

Fired when Claude asks the user a question.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permission_mode` | string | no | Current permission mode |

#### `ElicitationResult`

Fired when the user answers an elicitation.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permission_mode` | string | no | Current permission mode |

### Tool Execution

#### `PreToolUse`

Fired before a tool is executed.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tool_name` | string | yes | Name of the tool being called |
| `tool_use_id` | string | yes | Unique identifier for this tool call |
| `permission_mode` | string | yes | Current permission mode |
| `tool_input` | object | yes | Tool-specific input (opaque) |

#### `PostToolUse`

Fired after a tool completes successfully.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tool_name` | string | yes | Name of the tool that was called |
| `tool_use_id` | string | yes | Unique identifier for this tool call |
| `permission_mode` | string | yes | Current permission mode |
| `tool_input` | object | yes | Tool-specific input (opaque) |
| `tool_response` | object | yes | Tool-specific response (opaque) |

#### `PostToolUseFailure`

Fired after a tool fails.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tool_name` | string | yes | Name of the tool that failed |
| `tool_use_id` | string | yes | Unique identifier for this tool call |
| `permission_mode` | string | yes | Current permission mode |
| `tool_input` | object | yes | Tool-specific input (opaque) |
| `error` | string | no | Error message from the failed tool |

#### `PermissionRequest`

Fired when a tool requires permission approval.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tool_name` | string | yes | Name of the tool requesting permission |
| `tool_use_id` | string | yes | Unique identifier for this tool call |
| `permission_mode` | string | yes | Current permission mode |
| `tool_input` | object | yes | Tool-specific input (opaque) |

### Turn Control

#### `Stop`

Fired when Claude stops (end of a turn).

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `last_assistant_message` | string | yes | The last message Claude produced |
| `permission_mode` | string | yes | Current permission mode |
| `stop_hook_active` | boolean | yes | Whether a stop hook is active |

#### `StopFailure`

Fired when a stop hook itself fails.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permission_mode` | string | no | Current permission mode |
| `error` | string | no | Error message |

### Subagents

#### `SubagentStart`

Fired when a subagent is spawned.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | yes | Unique identifier for the subagent |
| `agent_type` | string | no | Type of the subagent |

#### `SubagentStop`

Fired when a subagent finishes.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | yes | Unique identifier for the subagent |
| `agent_transcript_path` | string | no | Path to the subagent's transcript |

#### `TeammateIdle`

Fired when a teammate agent becomes idle.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | no | Identifier of the idle teammate |

#### `TaskCompleted`

Fired when a task is completed.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `task_id` | string | no | Identifier of the completed task |

### Context & Config

#### `InstructionsLoaded`

Fired when CLAUDE.md or memory files are loaded.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | no | Path to the loaded instructions file |
| `load_reason` | string | yes | Why instructions were loaded |
| `memory_type` | string | no | Type of memory file (if applicable) |

#### `PreCompact`

Fired before context compaction.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permission_mode` | string | no | Current permission mode |

#### `PostCompact`

Fired after context compaction.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permission_mode` | string | no | Current permission mode |

#### `ConfigChange`

Fired when a settings file changes.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | string | yes | Source of the config change |
| `file_path` | string | yes | Path to the changed settings file |

#### `Notification`

Fired for system notifications.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `message` | string | yes | Notification message text |
| `notification_type` | string | yes | Type of notification |

### Worktree

#### `WorktreeCreate`

Fired when a git worktree is created.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `worktree_path` | string | no | Path to the created worktree |

#### `WorktreeRemove`

Fired when a git worktree is removed.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `worktree_path` | string | no | Path to the removed worktree |

## Event Flow

A typical session produces events in this order:

```
SessionStart
InstructionsLoaded          (one or more)
UserPromptSubmit
├── PreToolUse              ─┐
│   PermissionRequest?       │  repeated per tool call
│   PostToolUse              │
│   PostToolUseFailure?     ─┘
├── SubagentStart?          ─┐
│   └── (nested tool events) │  if subagents are used
│   SubagentStop?           ─┘
Stop
UserPromptSubmit            (next turn)
...
SessionEnd
```

## Using Schemas From Other Languages

The JSON Schema files are standalone and can be consumed by any language:

**Python:**
```python
import json
from jsonschema import validate

schema = json.load(open("schemas/events/Stop.json"))
event = {"cwd": "/tmp", "hook_event_name": "Stop", ...}
validate(instance=event, schema=schema)
```

**Node.js:**
```javascript
import Ajv from "ajv";
const ajv = new Ajv();
const schema = JSON.parse(fs.readFileSync("schemas/events/Stop.json"));
const valid = ajv.validate(schema, event);
```
