#!/usr/bin/env python3
"""
xclaude consumer — connects to the Unix socket and prints events as they arrive.
Retries indefinitely if the socket does not exist or the connection is lost.

Usage:
    python consumer.py [socket_path]

Default socket path: /tmp/xclaude.sock
"""

import json
import socket
import sys
import time

SOCKET_PATH = sys.argv[1] if len(sys.argv) > 1 else "/tmp/xclaude.sock"
RETRY_INTERVAL = 1  # seconds between reconnect attempts

COLORS = {
    "session.start": "\033[92m",   # green
    "session.end":   "\033[91m",   # red
    "agent.start":   "\033[94m",   # blue
    "agent.end":     "\033[93m",   # yellow
    "tool.start":    "\033[96m",   # cyan
    "tool.end":      "\033[95m",   # magenta
}
RESET = "\033[0m"
DIM   = "\033[2m"


def print_event(msg: dict):
    method = msg.get("method", "unknown")
    params = msg.get("params", {})
    color = COLORS.get(method, "")
    print(f"{color}▶ {method}{RESET}")
    print(json.dumps(params, indent=2))
    print()


def connect() -> socket.socket:
    while True:
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect(SOCKET_PATH)
            print(f"Connected to {SOCKET_PATH}\n")
            return sock
        except (FileNotFoundError, ConnectionRefusedError):
            print(f"{DIM}waiting for {SOCKET_PATH} ...{RESET}", end="\r")
            time.sleep(RETRY_INTERVAL)


def read_loop(sock: socket.socket):
    buf = b""
    while True:
        chunk = sock.recv(4096)
        if not chunk:
            print(f"\n{DIM}Connection closed.{RESET}")
            return
        buf += chunk
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
                print_event(msg)
            except json.JSONDecodeError as e:
                print(f"[decode error] {e}: {line!r}")


def main():
    while True:
        sock = connect()
        try:
            read_loop(sock)
        finally:
            sock.close()
        time.sleep(RETRY_INTERVAL)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nBye.")
