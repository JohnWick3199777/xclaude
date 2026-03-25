# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run directly (wrapper mode)
cargo run -- <claude-args>

# Run a subcommand
cargo run -- hook PreToolUse
cargo run -- logs
cargo run -- pretty
cargo run -- hooks
```

## Install

```bash
./install.sh          # builds release + installs to ~/.local/bin + symlinks claude
```

Or manually: `cargo build --release && cp target/release/xclaude ~/.local/bin/`

## Architecture

xclaude is a single-binary Rust CLI (`src/main.rs`) with two operating modes:

**Hook mode** (`xclaude hook <EVENT>`): Called by Claude Code via `--settings` hooks. Reads JSON from stdin, appends a JSONL entry to `~/.xclaude/logs/YYYY-MM-DD.jsonl`, optionally publishes a JSON-RPC 2.0 notification to a socket, then exits 0. Always non-blocking — never returns a non-zero exit code to avoid blocking Claude.

**Wrapper mode** (invoked as `claude` via symlink): Finds the real `claude` binary by scanning PATH (skipping itself by canonical path), injects all 22 hook events as a `--settings` JSON argument, then `exec`s the real binary. Subcommands `mcp`, `config`, `api-key`, `rc`, `remote-control` are passed through without hook injection.

**Installation model**: `xclaude install` creates a symlink `~/.local/bin/claude -> xclaude`. Because `~/.local/bin` is prepended to PATH, `claude` resolves to xclaude, which finds the real claude further down PATH.

## RPC Publishing

Events can be forwarded to a socket endpoint (fire-and-forget, 500ms timeout):
- Set `XCLAUDE_RPC_URL` env var, or add `"rpc_endpoint"` to `~/.xclaude/config.json`
- Supports `unix:///path/to/socket` and `tcp://host:port`
- Payload is JSON-RPC 2.0 with `method` = event name, `params.ts` + `params.data`

See `consumer.py` for a reference Unix socket consumer and `openapi.json` for the RPC payload schema.

## Key Design Decisions

- **No async runtime**: uses `std::process::Command::exec` (replaces process) for zero overhead in wrapper mode; hook mode uses synchronous I/O.
- **Async hooks**: `PreToolUse`, `PostToolUse`, and several other high-frequency events are fired with `"async": true` so they don't block Claude's execution.
- **Hook timeout**: 5 seconds per hook (well under any blocking threshold).
- Logs rotate daily by filename; `xclaude logs` polls at 100ms intervals to tail.
