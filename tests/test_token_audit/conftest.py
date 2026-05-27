"""Shared pytest fixtures for Token Audit tests.

Fixtures only — shared constants and helpers live in helpers.py.
"""

import sqlite3
import pytest
from unittest.mock import MagicMock, patch, mock_open
from .helpers import (
    MOCK_SQLITE_ROWS,
    MOCK_AGENT_LOG_LINES,
    MOCK_CRONTAB_LINES,
    MOCK_ANALYSIS_RESULT,
    ONE_LINER_TEMPLATES,
)


# ── Fixtures ───────────────────────────────────────────────────────

@pytest.fixture
def mock_state_db():
    """Create an in-memory SQLite database with mock data for testing."""
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    c = conn.cursor()
    c.execute("""
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY,
            source TEXT,
            agent_name TEXT,
            prompt_tokens INTEGER,
            completion_tokens INTEGER,
            cost_usd REAL,
            created_at TEXT
        )
    """)
    for row in MOCK_SQLITE_ROWS:
        c.execute(
            "INSERT INTO messages (source, agent_name, prompt_tokens, completion_tokens, cost_usd, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            row,
        )
    conn.commit()
    yield conn
    conn.close()


@pytest.fixture
def mock_empty_state_db():
    """Create an in-memory SQLite database with NO data."""
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    c = conn.cursor()
    c.execute("""
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY,
            source TEXT,
            agent_name TEXT,
            prompt_tokens INTEGER,
            completion_tokens INTEGER,
            cost_usd REAL,
            created_at TEXT
        )
    """)
    conn.commit()
    yield conn
    conn.close()


@pytest.fixture
def mock_corrupted_state_db(tmp_path):
    """Create a corrupted (non-SQLite) file."""
    db_path = tmp_path / "state.db"
    db_path.write_bytes(b"\x00\x01\x02\x03Not a valid SQLite database\xFF\xFE")
    return str(db_path)


@pytest.fixture
def mock_agent_log():
    """Return mock agent.log content."""
    return "\n".join(MOCK_AGENT_LOG_LINES)


@pytest.fixture
def mock_large_agent_log():
    """Return mock agent.log > 10MB for truncation testing."""
    line = "2026-05-27 10:00:00 [INFO] Normal operation log line for testing truncation\n"
    content = line * 300000  # ~30MB
    return content


@pytest.fixture
def mock_crontab_output():
    """Return mock crontab -l output."""
    return "\n".join(MOCK_CRONTAB_LINES)


@pytest.fixture
def mock_empty_crontab():
    """Return empty crontab output."""
    return ""


@pytest.fixture
def mock_analysis_result():
    """Return a complete mock analysis result dict."""
    return dict(MOCK_ANALYSIS_RESULT)


@pytest.fixture
def mock_zero_tokens_analysis():
    """Return analysis result for a fresh install with zero tokens."""
    return {
        "totals": {
            "total_prompt_tokens": 0,
            "total_completion_tokens": 0,
            "total_tokens": 0,
            "total_cost_usd": 0.0,
            "total_sessions": 0,
            "daily_avg_tokens": 0,
            "daily_avg_cost": 0.0,
        },
        "source_distribution": {},
        "waste": {
            "total_waste_incidents": 0,
            "waste_by_type": {},
            "total_estimated_waste_tokens": 0,
            "total_estimated_waste_cost": 0.0,
        },
        "cron": {
            "total_cron_tasks": 0,
            "llm_related_tasks": 0,
            "llm_tasks": [],
            "non_llm_tasks": [],
        },
        "cost_comparison": {
            "actual_cost": 0.0,
            "estimated_no_cache_cost": 0.0,
            "estimated_waste_cost": 0.0,
            "cache_savings": 0.0,
            "cache_hit_rate": 0.0,
        },
    }


@pytest.fixture
def mock_one_liner_templates():
    """Return the 5 one-liner templates (CR-19)."""
    return dict(ONE_LINER_TEMPLATES)
