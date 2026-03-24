# xclaude

A Rust wrapper for Claude Code that logs all hook events to `~/.xclaude/logs/`.

## What it does

xclaude intercepts your `claude` invocations via PATH, injects all 22 Claude Code hook events as `--settings`, and logs every event to a daily JSONL file.

No changes to your workflow — just run `claude` as normal and every action is recorded.

## Logged events

| Event | Description |
|---|---|
| `SessionStart` | Session began |
| `InstructionsLoaded` | CLAUDE.md / system instructions loaded |
| `UserPromptSubmit` | User submitted a prompt |
| `PreToolUse` | Before a tool call (file write, bash, etc.) |
| `PermissionRequest` | Claude requested a permission |
| `PostToolUse` | After a tool call succeeded |
| `PostToolUseFailure` | After a tool call failed |
| `Stop` | Claude finished a turn |
| `StopFailure` | Claude's stop failed |
| `Notification` | Claude is waiting for input |
| `SubagentStart` | Subagent spawned |
| `SubagentStop` | Subagent finished |
| `TeammateIdle` | Teammate agent went idle |
| `TaskCompleted` | Task completed |
| `PreCompact` | Before context compaction |
| `PostCompact` | After context compaction |
| `ConfigChange` | Config changed at runtime |
| `WorktreeCreate` | Git worktree created |
| `WorktreeRemove` | Git worktree removed |
| `Elicitation` | Claude requested user input |
| `ElicitationResult` | User answered an elicitation |
| `SessionEnd` | Session ended |

## Install

**One-line install (Recommended):**
```bash
curl -sSL https://raw.githubusercontent.com/JohnWick3199777/xclaude/master/install.sh | bash
```

Alternatively, you can clone and run the interactive setup manually:
```bash
git clone https://github.com/JohnWick3199777/xclaude.git
cd xclaude
./install.sh
```

## Usage

```bash
# Just use claude normally — xclaude intercepts transparently
claude "write a hello world in rust"

# Live-tail the log (stays open, streams events as they happen)
xclaude logs

# Pretty-print today's log
xclaude pretty

# Raw JSONL — pipe to jq for filtering
xclaude logs | jq 'select(.event == "PreToolUse") | .data.tool_input'

# Show all bash commands Claude ran
xclaude logs | jq 'select(.event == "PreToolUse" and .data.tool_name == "Bash") | .data.tool_input.command'

# Show all files written
xclaude logs | jq 'select(.event == "PreToolUse" and .data.tool_name == "Write") | .data.tool_input.file_path'

# List all hook event names
xclaude hooks
```

## Log format

One JSON line per event at `~/.xclaude/logs/YYYY-MM-DD.jsonl`:

```json
{"ts":"2026-03-24T16:38:45+02:00","event":"PreToolUse","data":{"session_id":"abc123","tool_name":"Write","tool_input":{"file_path":"main.rs","content":"fn main() {}"}}}
```

## License

MIT
