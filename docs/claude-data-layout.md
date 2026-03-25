# Claude Code Local Data Layout

Claude Code stores all local data under `~/.claude/`. This document maps the directory structure and file formats relevant to xclaude.

## Directory Overview

```
~/.claude/
├── history.jsonl              # Global prompt history (all projects)
├── sessions/                  # Session metadata (pid, cwd, timestamps)
│   └── <pid>.json
├── projects/                  # Per-project conversation data
│   └── <encoded-path>/        # e.g. -Users-alice-myproject
│       ├── <sessionId>.jsonl  # Full conversation transcript
│       └── <sessionId>/       # Session artifacts
│           ├── subagents/
│           │   ├── agent-<id>.jsonl      # Subagent transcript
│           │   └── agent-<id>.meta.json  # Subagent metadata
│           └── tool-results/
│               └── <hash>.txt            # Large tool results stored separately
├── settings.json              # User settings
├── settings.local.json        # Local-only settings (not synced)
├── debug/                     # Debug logs
│   ├── <sessionId>.txt
│   └── latest -> ...          # Symlink to most recent
├── session-env/               # Session environment snapshots
├── file-history/              # File change history
├── mcp/                       # MCP server config
├── plugins/                   # Installed plugins
├── skills/                    # Custom skills
├── todos/                     # Todo items
├── statsig/                   # Feature flags / analytics
└── telemetry/                 # Telemetry data
```

## Key Files & Formats

### `history.jsonl` — Global Prompt History

One JSON object per line. Every user prompt across all projects.

```json
{
  "display": "fix the login bug",
  "pastedContents": {},
  "timestamp": 1774436848495,
  "project": "/Users/alice/myproject",
  "sessionId": "224456d0-0b60-4e21-934f-c90343c70817"
}
```

### `sessions/<pid>.json` — Session Metadata

Keyed by the OS process ID. Created when a Claude Code session starts.

```json
{
  "pid": 31194,
  "sessionId": "be56da6f-c0bc-4cc2-acbe-529bbf00d7e2",
  "cwd": "/Users/alice/myproject",
  "startedAt": 1774422984819,
  "kind": "interactive"
}
```

### `projects/<encoded-path>/<sessionId>.jsonl` — Conversation Transcript

The richest data source. Contains the **full conversation** — every user message, assistant response, tool call, and system event. One JSON object per line.

**Event types:**

| `type` | Description |
|--------|-------------|
| `file-history-snapshot` | Snapshot of file state at conversation start |
| `user` | User message (contains `message.content` with full prompt text) |
| `assistant` | Assistant response (text, tool_use blocks, model, usage, stop_reason) |
| `system/local_command` | Local command executed by user (e.g. `! git status`) |
| `system/turn_duration` | Timing metadata for a turn |
| `progress` | Progress indicator events |

**User message example:**

```json
{
  "type": "user",
  "sessionId": "224456d0-...",
  "message": {
    "role": "user",
    "content": "fix the login bug"
  }
}
```

**Assistant message example (with tool use):**

```json
{
  "type": "assistant",
  "message": {
    "role": "assistant",
    "model": "claude-opus-4-6",
    "content": [
      { "type": "text", "text": "Let me look at the code." },
      { "type": "tool_use", "id": "toolu_01Abc...", "name": "Read", "input": { "file_path": "/src/login.rs" } }
    ],
    "usage": {
      "input_tokens": 3,
      "cache_creation_input_tokens": 9977,
      "cache_read_input_tokens": 8654,
      "output_tokens": 220
    },
    "stop_reason": "tool_use"
  }
}
```

### `projects/<path>/<sessionId>/subagents/` — Subagent Data

**`agent-<id>.meta.json`:**

```json
{
  "agentType": "Explore",
  "description": "Explore xclaude GUI setup"
}
```

**`agent-<id>.jsonl`:** Same transcript format as the parent conversation.

### `projects/<path>/<sessionId>/tool-results/<hash>.txt`

Large tool outputs (file reads, command output) stored separately to keep the JSONL manageable.

## Project Path Encoding

The project directory name is the absolute path with `/` replaced by `-` and a leading `-`:

```
/Users/alice/myproject → -Users-alice-myproject
```

## Comparison: xclaude Hooks vs Claude Native Data

| Feature | xclaude hooks (`~/.xclaude/logs/`) | Claude native (`~/.claude/projects/`) |
|---------|-------------------------------------|---------------------------------------|
| User prompts (full text) | No | Yes |
| Assistant responses (full text) | No | Yes |
| Tool call names & inputs | Yes (PreToolUse/PostToolUse) | Yes |
| Tool call outputs | No | Yes (inline or tool-results/) |
| Token usage & model | No | Yes |
| Subagent transcripts | No | Yes |
| Session lifecycle | Yes | Yes |
| Real-time streaming | Yes (socket/RPC) | No (file-based) |
| Structured by session | By date | By session ID |

The native Claude data is significantly richer. For the xclaude UI, reading from `~/.claude/projects/` provides complete conversation replay, while the hook-based logs are better for real-time event streaming.
