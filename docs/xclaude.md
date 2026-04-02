# xclaude

xclaude is a wrapper around the Claude Code CLI that exposes a Unix socket interface using JSON-RPC 2.0.

## What it does

xclaude launches `claude` as a subprocess and taps into all of its available data sources — hooks, stdout/stderr, transcript files, databases, and more. It translates the raw signals from these sources into well-defined agentic events and publishes them over a Unix socket.

Consumers connect to the socket to receive a structured, real-time event stream of everything happening inside a Claude session — which agents are running, what tools are being called, what sub-agents were spawned, and so on.

xclaude also listens on the same socket, so consumers can send requests back to influence or query the running session.

## Architecture

```
consumer
   |
   |  unix socket (json-rpc 2.0)
   |
xclaude
   |  \
   |   +-- hook receiver socket (unix, per-process)
   |         ^
   |         | hook payloads (JSON on stdin, forwarded by python one-liner)
   |
   +-- launches --> claude (subprocess)
                       |
                       +-- hooks  <-- injected via --settings on every launch
                       +-- stdout / stderr
                       +-- transcript files
                       +-- sqlite databases
```

## Hook injection

xclaude always injects all available Claude Code hooks. On startup it:

1. Binds a per-process hook receiver socket at `/tmp/xclaude-hook-<pid>.sock`.
2. Writes a temporary `settings.json` that registers hooks for every lifecycle
   event (`PreToolUse`, `PostToolUse`, `SubagentStart`, `SubagentStop`).
3. Passes `--settings <tmp-path>` to `claude` so the hooks are merged with any
   existing user/project settings.

Each hook command is a small Python one-liner that reads the hook payload from
stdin and forwards it to the hook receiver socket:

```
python3 -c "import socket,sys; s=socket.socket(socket.AF_UNIX); \
    s.connect('/tmp/xclaude-hook-<pid>.sock'); \
    s.sendall(sys.stdin.buffer.read()); s.close()"
```

The receiver translates hook payloads into xclaude events and broadcasts them
over the main consumer socket. Hook events map to xclaude events as follows:

| Claude hook     | xclaude event |
|-----------------|---------------|
| `PreToolUse`    | `tool.start`  |
| `PostToolUse`   | `tool.end`    |
| `SubagentStart` | `agent.start` |
| `SubagentStop`  | `agent.end`   |

## Communication

All messages on the socket follow the [JSON-RPC 2.0](https://www.jsonrpc.org/specification) protocol.

- **Notifications** (xclaude → consumer): agentic events emitted in real time
- **Requests / Responses** (consumer ↔ xclaude): consumer-initiated queries or commands

## Events

The event taxonomy is TBD. Events will model the agentic hierarchy that Claude Code exposes:

- Sessions
- Agents and sub-agents
- Tool calls and results
- Messages and transcripts
