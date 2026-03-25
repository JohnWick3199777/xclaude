#!/usr/bin/env python3
"""query_db.py — Query xclaude SQLite database for session stats."""

import sqlite3
from pathlib import Path

db_path = Path.home() / ".xclaude" / "xclaude.db"

if not db_path.exists():
    print(f"No database found at {db_path}")
    raise SystemExit(1)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row

print(f"Database: {db_path}\n")

# List tables
tables = con.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()
print("Tables:", [t["name"] for t in tables])

# Sessions summary
rows = con.execute("""
    SELECT session_id, slug, model, cwd, start_time, input_tokens, output_tokens
    FROM sessions ORDER BY start_time DESC LIMIT 5
""").fetchall()
print(f"\nRecent sessions ({len(rows)}):")
for r in rows:
    print(f"  [{r['start_time'][:19]}] {r['slug'] or r['session_id'][:8]}  model={r['model']}  in={r['input_tokens']} out={r['output_tokens']}")

# Tool calls summary
rows = con.execute("""
    SELECT tool_name, COUNT(*) as calls, AVG(duration_ms) as avg_ms, SUM(error) as errors
    FROM tool_calls GROUP BY tool_name ORDER BY calls DESC LIMIT 10
""").fetchall()
if rows:
    print(f"\nTop tool calls:")
    for r in rows:
        print(f"  {r['calls']:4d}x  {r['tool_name']:<20}  avg={r['avg_ms']:.0f}ms  errors={r['errors']}")

con.close()
