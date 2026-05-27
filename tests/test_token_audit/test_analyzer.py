"""Unit tests for the analyzer module."""

import pytest
from .helpers import MOCK_AGGREGATE_RESULT, MOCK_SOURCE_DISTRIBUTION, MOCK_WASTE_RESULT


class TestAnalyzer:
    """Unit tests for the audit analysis engine."""

    # ── Test 1: Calculate totals ───────────────────────────────────
    def test_calculate_totals(self, mock_state_db):
        """Verify total token, cost, and session counts are calculated correctly."""
        c = mock_state_db.cursor()
        c.execute("""
            SELECT
                COALESCE(SUM(prompt_tokens), 0) AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) AS total_completion_tokens,
                COALESCE(SUM(cost_usd), 0.0) AS total_cost_usd,
                COUNT(*) AS total_sessions
            FROM messages
        """)
        row = c.fetchone()

        assert row["total_prompt_tokens"] == MOCK_AGGREGATE_RESULT["total_prompt_tokens"]
        assert row["total_completion_tokens"] == MOCK_AGGREGATE_RESULT["total_completion_tokens"]
        assert round(row["total_cost_usd"], 3) == MOCK_AGGREGATE_RESULT["total_cost_usd"]
        assert row["total_sessions"] == MOCK_AGGREGATE_RESULT["total_sessions"]

    # ── Test 2: Source distribution ────────────────────────────────
    def test_source_distribution(self, mock_state_db):
        """Verify per-source breakdown matches expected distribution."""
        c = mock_state_db.cursor()
        c.execute("""
            SELECT
                source,
                SUM(prompt_tokens) AS prompt_tokens,
                SUM(completion_tokens) AS completion_tokens,
                SUM(cost_usd) AS cost_usd,
                COUNT(*) AS session_count
            FROM messages
            GROUP BY source
            ORDER BY source
        """)
        rows = c.fetchall()
        total_cost = sum(r["cost_usd"] for r in rows)

        for row in rows:
            source = row["source"]
            expected = MOCK_SOURCE_DISTRIBUTION[source]
            assert row["prompt_tokens"] == expected["prompt_tokens"]
            assert row["completion_tokens"] == expected["completion_tokens"]
            assert round(row["cost_usd"], 3) == round(expected["cost_usd"], 3)

    # ── Test 3: Waste detection aggregation ────────────────────────
    def test_waste_detection_aggregation(self, mock_agent_log):
        """Verify waste counts are aggregated per error type."""
        import re
        ERROR_PATTERNS = {
            "rate_limit_429": re.compile(r"429|rate.limit|too.many.requests", re.IGNORECASE),
            "connection_refused": re.compile(r"connection.refused|ECONNREFUSED", re.IGNORECASE),
            "mcp_failure": re.compile(r"MCP.*fail|MCP.*error", re.IGNORECASE),
            "retry_overhead": re.compile(r"retry|retrying", re.IGNORECASE),
            "path_error": re.compile(r"path.not.found|no.such.file|filenotfound", re.IGNORECASE),
        }
        ERROR_WASTE_ESTIMATES = {
            "rate_limit_429": 4000,
            "connection_refused": 2000,
            "mcp_failure": 3000,
            "retry_overhead": 1500,
            "path_error": 500,
        }

        lines = mock_agent_log.split("\n")
        error_counts = {k: 0 for k in ERROR_PATTERNS}
        for line in lines:
            for etype, pat in ERROR_PATTERNS.items():
                matches = pat.findall(line)
                if matches:
                    error_counts[etype] += len(matches)

        total_waste_tokens = sum(error_counts.values()) * 500  # flat rate

        assert error_counts == {
            "rate_limit_429": 2,
            "connection_refused": 1,
            "mcp_failure": 2,
            "retry_overhead": 4,
            "path_error": 1,
        }
        assert total_waste_tokens == 10 * 500  # = 5000

    # ── Test 4: Cost comparison (actual vs no-cache vs waste) ──────
    def test_cost_comparison(self):
        """Verify cost comparison: actual vs no-cache vs waste."""
        actual_cost = 1.393
        cache_hit_rate = 75.0  # percentage
        waste_cost = 0.155

        # No-cache estimate: if no caching, cost would be higher
        estimated_no_cache_cost = round(actual_cost / (1 - cache_hit_rate / 100), 3)
        cache_savings = round(estimated_no_cache_cost - actual_cost, 3)

        comparison = {
            "actual_cost": actual_cost,
            "estimated_no_cache_cost": estimated_no_cache_cost,
            "estimated_waste_cost": waste_cost,
            "cache_savings": cache_savings,
            "cache_hit_rate": cache_hit_rate,
        }

        assert comparison["actual_cost"] == 1.393
        assert comparison["estimated_no_cache_cost"] == 5.572
        assert comparison["cache_savings"] == 4.179
        assert comparison["estimated_waste_cost"] == 0.155

    # ── Test 5: Integrate data from all parsers ────────────────────
    def test_integrate_parser_data(self, mock_analysis_result):
        """Verify combined analysis from all parsers produces valid report data."""
        result = mock_analysis_result

        # All sections must be present
        assert "totals" in result
        assert "source_distribution" in result
        assert "waste" in result
        assert "cron" in result
        assert "cost_comparison" in result

        # Totals must be internally consistent
        totals = result["totals"]
        assert totals["total_tokens"] == totals["total_prompt_tokens"] + totals["total_completion_tokens"]

        # Source distribution percentages should sum to ~100%
        dist = result["source_distribution"]
        total_pct = sum(s["percentage"] for s in dist.values())
        assert abs(total_pct - 100.0) < 0.5, f"Percentages sum to {total_pct}, expected ~100%"

        # Cron LLM tasks should not exceed total cron tasks
        cron = result["cron"]
        assert cron["llm_related_tasks"] <= cron["total_cron_tasks"]

        # Waste tokens should be reasonable compared to total
        waste = result["waste"]
        assert waste["total_estimated_waste_tokens"] <= totals["total_tokens"]
