#!/usr/bin/env python3
"""parse_logs.py — Summarize today's xclaude JSONL log."""

import json
from pathlib import Path
from datetime import date
from collections import Counter

log_file = Path.home() / ".xclaude" / "logs" / f"{date.today()}.jsonl"

if not log_file.exists():
    print(f"No log file found at {log_file}")
    raise SystemExit(1)

events = []
skipped = 0
with open(log_file) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            skipped += 1

print(f"Log file: {log_file}")
print(f"Total events: {len(events)}  (skipped malformed: {skipped})\n")

counts = Counter(e["event"] for e in events)
print("Event counts:")
for event, count in counts.most_common():
    print(f"  {count:4d}  {event}")

print(f"\nLast 5 events:")
for e in events[-5:]:
    ts = e.get("ts", "")[:19]
    event = e.get("event", "")
    data_keys = list(e.get("data", {}).keys())
    print(f"  [{ts}] {event}  data keys: {data_keys}")
