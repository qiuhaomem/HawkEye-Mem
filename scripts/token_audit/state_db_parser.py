# Copyright 2026 秋毫mem Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
state_db_parser.py — Parse ~/.hermes/state.db for token usage data.

Constraints:
- Zero dependencies (stdlib only: sqlite3, os, json, time)
- Aggregate queries ONLY (COUNT, SUM) — no SELECT *
- Handle locked DB with retry
- Handle missing DB gracefully
"""

import sqlite3
import os
import time
import json
from datetime import datetime, timedelta

HERMES_STATE_DB = os.path.expanduser("~/.hermes/state.db")


def parse_state_db(days: int | None = None) -> dict | None:
    """
    Read ~/.hermes/state.db and return aggregated token/cost data.

    Args:
        days: If set, only include sessions from the last N days.
              If None, include all sessions (default to today only).

    Returns:
        dict with total_tokens, total_cost, session_count, daily_avg,
        by_source mapping, or None if DB doesn't exist.
    """
    db_path = HERMES_STATE_DB
    if not os.path.isfile(db_path):
        return None

    # Connect with retry for locked databases
    conn = _connect_with_retry(db_path)
    if conn is None:
        return None

    try:
        cursor = conn.cursor()

        # Check if required tables exist
        cursor.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'"
        )
        if cursor.fetchone() is None:
            conn.close()
            return None

        # Build date filter
        date_filter = ""
        params = []
        if days is not None:
            cutoff = (datetime.now() - timedelta(days=days)).isoformat()
            date_filter = "WHERE created_at >= ?"
            params.append(cutoff)

        # Aggregate query — total across all sources
        cursor.execute(
            f"""
            SELECT
                COUNT(*) as session_count,
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(estimated_cost_usd), 0.0)
            FROM sessions
            {date_filter}
            """,
            params,
        )
        row = cursor.fetchone()
        if row is None or row[0] == 0:
            conn.close()
            return {
                "total_tokens": 0,
                "total_input_tokens": 0,
                "total_output_tokens": 0,
                "total_cache_read_tokens": 0,
                "total_cost": 0.0,
                "session_count": 0,
                "daily_avg": 0,
                "by_source": {},
            }

        session_count = row[0]
        total_input = row[1]
        total_output = row[2]
        total_cache = row[3]
        total_cost = row[4]
        total_tokens = total_input + total_output + total_cache

        # Per-source aggregation
        cursor.execute(
            f"""
            SELECT
                COALESCE(source, 'unknown'),
                COUNT(*) as cnt,
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(estimated_cost_usd), 0.0)
            FROM sessions
            {date_filter}
            GROUP BY COALESCE(source, 'unknown')
            ORDER BY cnt DESC
            """,
            params,
        )
        source_rows = cursor.fetchall()

        by_source = {}
        for src_row in source_rows:
            src = src_row[0]
            src_count = src_row[1]
            src_input = src_row[2]
            src_output = src_row[3]
            src_cache = src_row[4]
            src_cost = src_row[5]
            src_tokens = src_input + src_output + src_cache
            by_source[src] = {
                "tokens": src_tokens,
                "input": src_input,
                "output": src_output,
                "cache_read": src_cache,
                "cost": src_cost,
                "count": src_count,
            }

        conn.close()

        return {
            "total_tokens": total_tokens,
            "total_input_tokens": total_input,
            "total_output_tokens": total_output,
            "total_cache_read_tokens": total_cache,
            "total_cost": total_cost,
            "session_count": session_count,
            "daily_avg": total_tokens // max(session_count, 1),
            "by_source": by_source,
        }

    except sqlite3.Error as e:
        conn.close()
        return {"error": f"SQLite error: {e}"}
    except Exception as e:
        if conn:
            conn.close()
        return {"error": f"Unexpected error: {e}"}


def _connect_with_retry(db_path: str, max_retries: int = 1) -> sqlite3.Connection | None:
    """Try to connect to SQLite DB, retrying once if locked."""
    for attempt in range(max_retries + 1):
        try:
            conn = sqlite3.connect(db_path)
            conn.execute("SELECT 1")  # Verify it works
            return conn
        except sqlite3.OperationalError as e:
            if "locked" in str(e) and attempt < max_retries:
                time.sleep(0.5)
                continue
            return None
    return None
