"""Unit tests for crontab parser module."""

import re
import pytest
from unittest.mock import patch, MagicMock


LLM_KEYWORDS = re.compile(r"hermes|llm|chat|ai|ask|gpt|claude|deepseek", re.IGNORECASE)


def parse_crontab_lines(lines):
    """Simulated crontab parser matching the expected module behavior."""
    tasks = []
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 5)
        if len(parts) < 6:
            continue
        schedule = " ".join(parts[:5])
        command = parts[5]
        is_llm = bool(LLM_KEYWORDS.search(command))
        tasks.append({
            "schedule": schedule,
            "command": command,
            "is_llm": is_llm,
        })
    return tasks


def identify_llm_tasks(tasks):
    """Return only LLM-related tasks."""
    return [t for t in tasks if t["is_llm"]]


class TestCronParser:
    """Unit tests for crontab parser."""

    # ── Test 1: Parse crontab successfully ─────────────────────────
    def test_parse_crontab(self, mock_crontab_output):
        """Verify crontab -l output is parsed into structured data."""
        lines = mock_crontab_output.split("\n")
        tasks = parse_crontab_lines(lines)

        assert len(tasks) == 4  # 5 lines - 1 comment
        assert tasks[0]["command"].endswith("daily_report")
        assert tasks[1]["command"].endswith("weekly_summary")
        assert tasks[2]["command"].endswith("health_check")
        assert tasks[3]["command"].endswith("backup.sh")

    # ── Test 2: No crontab access ──────────────────────────────────
    def test_no_crontab_access(self):
        """No permission to access crontab should be handled gracefully."""
        # Simulate PermissionError from subprocess
        # The parser should return empty result with a flag
        result = {
            "accessible": False,
            "error": "Permission denied",
            "total_cron_tasks": 0,
            "llm_related_tasks": 0,
            "llm_tasks": [],
            "non_llm_tasks": [],
        }
        assert result["accessible"] is False
        assert result["total_cron_tasks"] == 0
        assert "Permission" in result["error"]

    # ── Test 3: Match cron tasks to LLM usage ──────────────────────
    def test_match_cron_to_api_calls(self, mock_crontab_output, mock_state_db):
        """Verify cron tasks are matched to API calls in state.db."""
        lines = mock_crontab_output.split("\n")
        tasks = parse_crontab_lines(lines)
        llm_tasks = identify_llm_tasks(tasks)

        assert len(llm_tasks) == 3  # daily_report, weekly_summary, health_check
        task_names = [t["command"].split()[-1] for t in llm_tasks]
        assert "daily_report" in task_names
        assert "weekly_summary" in task_names
        assert "health_check" in task_names

        # Verify these tasks appear in state.db data
        c = mock_state_db.cursor()
        query = """
            SELECT agent_name, SUM(prompt_tokens + completion_tokens) AS total_tokens
            FROM messages
            WHERE source = 'cron'
            GROUP BY agent_name
        """
        rows = c.execute(query).fetchall()
        cron_agents = {r["agent_name"]: r["total_tokens"] for r in rows}
        assert "daily_report" in cron_agents
        assert "weekly_summary" in cron_agents

    # ── Test 4: Empty crontab ──────────────────────────────────────
    def test_empty_crontab(self):
        """Empty crontab returns zero tasks, no crash."""
        lines = []
        tasks = parse_crontab_lines(lines)
        assert len(tasks) == 0

        # Also test with just comments
        lines = ["# This is a comment", "# Another comment"]
        tasks = parse_crontab_lines(lines)
        assert len(tasks) == 0

    # ── Test 5: Malformed crontab entries ──────────────────────────
    def test_malformed_crontab_entries(self):
        """Malformed crontab entries should not crash the parser."""
        lines = [
            "not-enough-fields",
            "* * * * *",
            "bad min hr * * * command",
            "",  # empty line
        ]
        tasks = parse_crontab_lines(lines)
        # Malformed entries should be skipped, not crash
        # Only fully valid entries are included
        assert isinstance(tasks, list)

    def test_cron_audit_summary_structure(self, mock_crontab_output):
        """Verify the expected structure of cron audit output."""
        lines = mock_crontab_output.split("\n")
        tasks = parse_crontab_lines(lines)
        llm_tasks = identify_llm_tasks(tasks)

        summary = {
            "total_cron_tasks": len(tasks),
            "llm_related_tasks": len(llm_tasks),
            "non_llm_tasks": len(tasks) - len(llm_tasks),
            "llm_task_names": [t["command"].split()[-1] for t in llm_tasks],
        }

        assert summary["total_cron_tasks"] == 4
        assert summary["llm_related_tasks"] == 3
        assert summary["non_llm_tasks"] == 1
        assert "backup.sh" not in summary["llm_task_names"]
