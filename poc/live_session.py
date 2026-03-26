#!/usr/bin/env python3
"""
Live session viewer — shows tool calls with inputs, duration, and context tokens.

Usage:
    # Terminal 1: start the viewer
    python3 poc/live_session.py

    # Terminal 2: run claude (xclaude wrapper auto-publishes events)
    XCLAUDE_RPC_URL=unix:///tmp/xclaude.sock claude "fix the bug"
"""

import json
import os
import socket
import sys
from datetime import datetime

SOCKET_PATH = os.environ.get("XCLAUDE_SOCK", "/tmp/xclaude.sock")

# ── ANSI ─────────────────────────────────────────────────────────────────────

RESET   = "\033[0m"
BOLD    = "\033[1m"
DIM     = "\033[2m"
RED     = "\033[31m"
GREEN   = "\033[32m"
YELLOW  = "\033[33m"
BLUE    = "\033[34m"
MAGENTA = "\033[35m"
CYAN    = "\033[36m"

# ── State ────────────────────────────────────────────────────────────────────

# Buffer tool inputs from PreToolUse so we can print them alongside
# the enriched data from Stop.
pending_tools = {}  # tool_use_id -> {tool_name, tool_input}
tool_counter = 0

# ── Helpers ──────────────────────────────────────────────────────────────────

def ts_short(iso: str) -> str:
    try:
        return datetime.fromisoformat(iso).strftime("%H:%M:%S")
    except Exception:
        return iso[:8] if len(iso) >= 8 else iso

def fmt_tokens(n) -> str:
    if n is None:
        return "—"
    if isinstance(n, (int, float)):
        if n >= 1000:
            return f"{n/1000:.1f}k"
        return str(int(n))
    return str(n)

def fmt_duration(ms) -> str:
    if ms is None:
        return "—"
    if ms >= 1000:
        return f"{ms/1000:.1f}s"
    return f"{ms}ms"

def tool_input_oneliner(name: str, inp: dict) -> str:
    if name == "Bash":
        cmd = inp.get("command", "")
        if len(cmd) > 90:
            cmd = cmd[:87] + "..."
        return f"$ {cmd}"
    if name == "Read":
        return inp.get("file_path", "?")
    if name == "Write":
        return inp.get("file_path", "?")
    if name == "Edit":
        return inp.get("file_path", "?")
    if name == "Glob":
        return inp.get("pattern", "?")
    if name == "Grep":
        pat = inp.get("pattern", "?")
        path = inp.get("path", ".")
        return f"/{pat}/ in {path}"
    if name == "Agent":
        desc = inp.get("description", inp.get("prompt", ""))
        if len(desc) > 70:
            desc = desc[:67] + "..."
        return desc
    # Generic
    for k, v in inp.items():
        sv = str(v)
        if len(sv) > 70:
            sv = sv[:67] + "..."
        return f"{k}={sv}"
    return ""

# ── Event handlers ───────────────────────────────────────────────────────────

def on_session_start(ts: str, d: dict):
    model = d.get("model", "?")
    sid = d.get("session_id", "")[:8]
    print(f"\n{GREEN}{BOLD}{'━' * 70}")
    print(f"  SESSION  {ts_short(ts)}  model={model}  [{sid}...]")
    print(f"{'━' * 70}{RESET}")

def on_session_end(ts: str, d: dict):
    stats = d.get("stats", {})
    usage = d.get("usage", {})
    print(f"\n{GREEN}{BOLD}{'━' * 70}")
    print(f"  END  {ts_short(ts)}  tools={stats.get('tool_calls', '?')}  errors={stats.get('errors', 0)}  output={fmt_tokens(usage.get('output_tokens'))}")
    print(f"{'━' * 70}{RESET}\n")

def on_pre_tool(ts: str, d: dict):
    tid = d.get("tool_use_id", "")
    pending_tools[tid] = {
        "tool_name": d.get("tool_name", "?"),
        "tool_input": d.get("tool_input", {}),
    }

def on_stop(ts: str, d: dict):
    global tool_counter
    tools = d.get("tools", [])
    usage = d.get("usage", {})
    dur = d.get("turn_duration_ms")

    # Context window — compute from usage
    input_tok = usage.get("input_tokens", 0)
    cache_create = usage.get("cache_creation_input_tokens", 0)
    cache_read = usage.get("cache_read_input_tokens", 0)
    output_tok = usage.get("output_tokens", 0)
    ctx_size = input_tok + cache_create + cache_read  # total context at this turn

    if not tools:
        # No tool calls this turn — just a text response (all output is "model" tokens)
        msg = d.get("last_assistant_message", "")
        if msg:
            snippet = msg.strip().split("\n")[0]
            if len(snippet) > 90:
                snippet = snippet[:87] + "..."
            print(f"\n  {MAGENTA}{BOLD}STOP{RESET} {DIM}{ts_short(ts)}  turn={fmt_duration(dur)}{RESET}")
            pct = f"{output_tok/ctx_size:.1%}" if ctx_size > 0 else "—"
            print(f"  {DIM}  model: +{fmt_tokens(output_tok)} tok ({pct} of ctx){RESET}")
            print(f"  {DIM}{snippet}{RESET}")
        return

    # Print header
    print(f"\n  {MAGENTA}{BOLD}STOP{RESET} {DIM}{ts_short(ts)}  turn={fmt_duration(dur)}  ctx={fmt_tokens(ctx_size)}{RESET}")

    # Print each tool call, accumulate tool tokens
    tool_tok_total = 0
    for t in tools:
        tool_counter += 1
        tid = t.get("tool_use_id", "")
        name = t.get("name", "?")
        dur_ms = t.get("duration_ms")
        ctx_added = t.get("ctx_added")
        is_error = t.get("is_error", False)

        if isinstance(ctx_added, (int, float)):
            tool_tok_total += ctx_added

        # Get input from our buffer, or fall back to enriched data
        buffered = pending_tools.pop(tid, None)
        inp = buffered["tool_input"] if buffered else t.get("input", {})

        oneliner = tool_input_oneliner(name, inp if isinstance(inp, dict) else {})

        # Status marker
        if is_error:
            marker = f"{RED}x{RESET}"
        else:
            marker = f"{GREEN}v{RESET}"

        # Format metrics
        dur_str = fmt_duration(dur_ms)
        ctx_str = fmt_tokens(ctx_added)

        print(f"  {marker} {BOLD}{name:<8}{RESET} {DIM}{dur_str:>6}  +{ctx_str:>5} tok{RESET}  {oneliner}")

    # Summary: model output vs tool results
    # output_tok = what the model generated (text + tool_use blocks)
    # tool_tok_total = sum of ctx_added (tool results injected into context)
    # Both contribute to context growth this turn
    total_added = output_tok + tool_tok_total
    model_pct = f"{output_tok/total_added:.0%}" if total_added > 0 else "—"
    tools_pct = f"{tool_tok_total/total_added:.0%}" if total_added > 0 else "—"
    ctx_pct = f"{total_added/ctx_size:.1%}" if ctx_size > 0 else "—"
    print(f"  {DIM}  turn: +{fmt_tokens(total_added)} tok ({ctx_pct} of ctx)  "
          f"model={fmt_tokens(output_tok)} ({model_pct})  "
          f"tools={fmt_tokens(tool_tok_total)} ({tools_pct}){RESET}")

    sys.stdout.flush()

def on_subagent_start(ts: str, d: dict):
    aid = d.get("agent_id", "")[-8:]
    atype = d.get("agent_type", "")
    label = f" ({atype})" if atype else ""
    print(f"  {BLUE}+ subagent{label}{RESET} {DIM}[...{aid}]{RESET}")

def on_subagent_stop(ts: str, d: dict):
    aid = d.get("agent_id", "")[-8:]
    stats = d.get("stats", {})
    tools = stats.get("tool_calls", "?")
    wall = stats.get("wall_time_ms")
    print(f"  {BLUE}- subagent{RESET} {DIM}[...{aid}]  tools={tools}  wall={fmt_duration(wall)}{RESET}")

def on_user_prompt(ts: str, d: dict):
    prompt = d.get("prompt", "")
    if len(prompt) > 100:
        prompt = prompt[:97] + "..."
    print(f"\n  {CYAN}{BOLD}USER{RESET} {DIM}{ts_short(ts)}{RESET}  {prompt}")

# ── Dispatch ─────────────────────────────────────────────────────────────────

HANDLERS = {
    "SessionStart":       on_session_start,
    "SessionEnd":         on_session_end,
    "PreToolUse":         on_pre_tool,
    "Stop":               on_stop,
    "SubagentStart":      on_subagent_start,
    "SubagentStop":       on_subagent_stop,
    "UserPromptSubmit":   on_user_prompt,
}

def handle_event(rpc: dict):
    method = rpc.get("method", "?")
    params = rpc.get("params", {})
    ts = params.get("ts", "")
    data = params.get("data", {})

    handler = HANDLERS.get(method)
    if handler:
        handler(ts, data)
    sys.stdout.flush()

# ── Socket server ────────────────────────────────────────────────────────────

def main():
    if os.path.exists(SOCKET_PATH):
        os.remove(SOCKET_PATH)

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.bind(SOCKET_PATH)
    sock.listen(32)

    print(f"{BOLD}xclaude live session viewer{RESET}")
    print(f"{DIM}listening on {SOCKET_PATH}")
    print(f"set XCLAUDE_RPC_URL=unix://{SOCKET_PATH} to connect{RESET}\n")

    try:
        while True:
            conn, _ = sock.accept()
            with conn:
                buf = b""
                while True:
                    chunk = conn.recv(8192)
                    if not chunk:
                        break
                    buf += chunk
                if buf:
                    try:
                        rpc = json.loads(buf.decode("utf-8").strip())
                        handle_event(rpc)
                    except json.JSONDecodeError:
                        print(f"{RED}[!] invalid JSON{RESET}")
    except KeyboardInterrupt:
        print(f"\n{DIM}shutting down{RESET}")
    finally:
        if os.path.exists(SOCKET_PATH):
            os.remove(SOCKET_PATH)


if __name__ == "__main__":
    main()
