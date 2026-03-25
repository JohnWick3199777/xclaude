#!/usr/bin/env python3
"""token_usage.py — Show token usage across all sessions."""

import sqlite3
from pathlib import Path

db_path = Path.home() / ".xclaude" / "xclaude.db"

if not db_path.exists():
    print(f"No database found at {db_path}")
    raise SystemExit(1)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row

totals = con.execute("""
    SELECT
        COUNT(*) as session_count,
        SUM(input_tokens) as total_input,
        SUM(output_tokens) as total_output,
        SUM(cache_creation_tokens) as total_cache_write,
        SUM(cache_read_tokens) as total_cache_read
    FROM sessions
""").fetchone()

print("=== Token Usage (all sessions) ===")
print(f"  Sessions:       {totals['session_count']}")
print(f"  Input tokens:   {totals['total_input'] or 0:,}")
print(f"  Output tokens:  {totals['total_output'] or 0:,}")
print(f"  Cache writes:   {totals['total_cache_write'] or 0:,}")
print(f"  Cache reads:    {totals['total_cache_read'] or 0:,}")

rows = con.execute("""
    SELECT date(start_time) as day,
           SUM(input_tokens) as input,
           SUM(output_tokens) as output
    FROM sessions
    GROUP BY day ORDER BY day DESC LIMIT 7
""").fetchall()

if rows:
    print("\n=== Daily breakdown (last 7 days) ===")
    for r in rows:
        print(f"  {r['day']}  input={r['input'] or 0:>8,}  output={r['output'] or 0:>8,}")

con.close()
