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
   |
   +-- launches --> claude (subprocess)
                       |
                       +-- hooks
                       +-- stdout / stderr
                       +-- transcript files
                       +-- sqlite databases
```

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
