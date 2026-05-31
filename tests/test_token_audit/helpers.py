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

"""Shared constants and helper functions for Token Audit tests.

Separated from conftest.py to allow direct imports from test files.
conftest.py only contains pytest fixtures (auto-discovered by pytest).
"""

import re
import json


# ── Mock state.db data ──────────────────────────────────────────────
MOCK_SQLITE_ROWS = [
    # (source, agent_name, prompt_tokens, completion_tokens, cost_usd, created_at)
    ("wechat", "hermes", 1500, 3000, 0.045, "2026-05-27T10:00:00"),
    ("wechat", "hermes", 800, 2000, 0.028, "2026-05-27T10:30:00"),
    ("cron", "daily_report", 5000, 8000, 0.130, "2026-05-27T06:00:00"),
    ("cron", "weekly_summary", 20000, 35000, 0.550, "2026-05-26T08:00:00"),
    ("api", "code_review", 3000, 5000, 0.080, "2026-05-27T11:00:00"),
    ("api", "code_review", 4000, 6000, 0.100, "2026-05-27T12:00:00"),
    ("subagent", "deep_research", 15000, 25000, 0.400, "2026-05-26T14:00:00"),
    ("wechat", "hermes", 2000, 4000, 0.060, "2026-05-25T09:00:00"),
]

MOCK_AGGREGATE_RESULT = {
    "total_prompt_tokens": 51300,
    "total_completion_tokens": 88000,
    "total_tokens": 139300,
    "total_cost_usd": 1.393,
    "total_sessions": 8,
    "daily_avg_tokens": 46433,
    "daily_avg_cost": 0.464,
}

MOCK_SOURCE_DISTRIBUTION = {
    "wechat": {"prompt_tokens": 4300, "completion_tokens": 9000, "cost_usd": 0.133, "percentage": 9.5},
    "cron": {"prompt_tokens": 25000, "completion_tokens": 43000, "cost_usd": 0.680, "percentage": 48.8},
    "api": {"prompt_tokens": 7000, "completion_tokens": 11000, "cost_usd": 0.180, "percentage": 12.9},
    "subagent": {"prompt_tokens": 15000, "completion_tokens": 25000, "cost_usd": 0.400, "percentage": 28.7},
}

MOCK_WASTE_RESULT = {
    "total_waste_incidents": 7,
    "waste_by_type": {
        "rate_limit_429": {"count": 1, "estimated_waste_tokens": 4000},
        "connection_refused": {"count": 1, "estimated_waste_tokens": 2000},
        "mcp_failure": {"count": 2, "estimated_waste_tokens": 6000},
        "retry_overhead": {"count": 2, "estimated_waste_tokens": 3000},
        "path_error": {"count": 1, "estimated_waste_tokens": 500},
    },
    "total_estimated_waste_tokens": 15500,
    "total_estimated_waste_cost": 0.155,
}

MOCK_CRON_RESULT = {
    "total_cron_tasks": 5,
    "llm_related_tasks": 3,
    "llm_tasks": [
        {"command": "daily_report", "schedule": "0 6 * * *", "last_run": None},
        {"command": "weekly_summary", "schedule": "0 8 * * 1", "last_run": None},
        {"command": "health_check", "schedule": "*/30 * * * *", "last_run": None},
    ],
    "non_llm_tasks": [
        {"command": "backup.sh", "schedule": "0 2 * * 0"},
    ],
}

MOCK_ANALYSIS_RESULT = {
    "totals": dict(MOCK_AGGREGATE_RESULT),
    "source_distribution": dict(MOCK_SOURCE_DISTRIBUTION),
    "waste": dict(MOCK_WASTE_RESULT),
    "cron": dict(MOCK_CRON_RESULT),
    "cost_comparison": {
        "actual_cost": 1.393,
        "estimated_no_cache_cost": 4.179,
        "estimated_waste_cost": 0.155,
        "cache_savings": 2.786,
        "cache_hit_rate": 75.0,
    },
}

# Report templates (CR-19 — 5 one-liner variants)
ONE_LINER_TEMPLATES = {
    "comfort": "📊 总消耗 {total_tokens:,} tokens = ${total_cost:.2f} | 真实浪费 {waste_pct:.1f}% = ${waste_cost:.2f} | 真相：大部分是必要开销",
    "reminder": "📊 总消耗 {total_tokens:,} tokens = ${total_cost:.2f} | 浪费 {waste_pct:.1f}% = ${waste_cost:.2f} | 有 {waste_pct:.0f}% 的浪费可以优化",
    "warning": "📊 总消耗 {total_tokens:,} tokens = ${total_cost:.2f} | 浪费 {waste_pct:.1f}% = ${waste_cost:.2f} | ⚠️ 超过 1/5 的 Token 被浪费，建议立即检查",
    "guidance": "📊 未找到 Hermes 数据库，请确认 ~/.hermes/state.db 路径是否正确",
    "advice": "📊 数据量不足（{sessions} 条会话），建议运行一段时间后再审计以获得有意义的结论",
}

MOCK_AGENT_LOG_LINES = [
    "2026-05-27 10:15:00 [WARNING] 429 Too Many Requests - provider=openai model=gpt-4",
    "2026-05-27 10:15:01 [ERROR] Connection refused: MCP server at localhost:9876",
    "2026-05-27 10:15:02 [ERROR] MCP call failed: tool=get_memory_status, retry=1",
    "2026-05-27 10:15:03 [INFO] Retrying request (attempt 2/3)...",
    "2026-05-27 11:00:00 [ERROR] MCP call failed: tool=get_memory_metric, retry=2",
    "2026-05-27 11:00:01 [INFO] Retrying request (attempt 3/3)...",
    "2026-05-27 12:00:00 [ERROR] Path not found: /tmp/invalid_cache",
]

MOCK_CRONTAB_LINES = [
    "0 6 * * * /usr/local/bin/hermes run daily_report",
    "0 8 * * 1 /usr/local/bin/hermes run weekly_summary",
    "*/30 * * * * /usr/local/bin/hermes run health_check",
    "# Weekly backup (not LLM related)",
    "0 2 * * 0 /usr/local/bin/backup.sh",
]


# ── Error pattern definitions ─────────────────────────────────────
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


# ── Helper: CR-23 compliance check ────────────────────────────────
def assert_no_select_star(sql_query: str):
    """Assert that a SQL query does not use SELECT * (CR-23 compliance)."""
    normalized = sql_query.strip().upper().replace("\n", " ")
    assert "SELECT *" not in normalized, (
        f"CR-23 violation: query uses SELECT *\nQuery: {sql_query}"
    )
    assert "SELECT *" not in normalized.replace("SELECT  ", "SELECT "), (
        f"CR-23 violation: query uses SELECT * variant\nQuery: {sql_query}"
    )


# ── Simulated CLI runner for integration tests ────────────────────
def simulate_cli(args: list, analysis_result: dict = None) -> dict:
    """Simulate CLI execution and return structured result."""
    result = analysis_result or MOCK_ANALYSIS_RESULT
    output = {}

    if "--json" in args:
        output["format"] = "json"
        output["report"] = result
    else:
        output["format"] = "terminal"

    if "--source" in args:
        idx = args.index("--source")
        if idx + 1 < len(args):
            output["source_filter"] = args[idx + 1]

    if "--days" in args:
        idx = args.index("--days")
        if idx + 1 < len(args):
            output["days"] = int(args[idx + 1])

    if "--compare" in args:
        idx = args.index("--compare")
        if idx + 1 < len(args):
            periods = args[idx + 1].split(",")
            output["compare_periods"] = [int(p) for p in periods]

    return output


# ── One-liner template renderer for UX tests ──────────────────────
def render_one_liner(template: str, **kwargs) -> str:
    """Render a one-liner template with the given values."""
    return template.format(**kwargs)


def generate_one_liner(waste_pct: float, total_tokens: int, total_cost: float,
                       waste_cost: float, sessions: int = 8,
                       db_exists: bool = True) -> str:
    """Simulate dynamic one-liner generation with 5 templates (CR-19)."""
    if not db_exists:
        return ONE_LINER_TEMPLATES["guidance"].format()

    if sessions < 3:
        return ONE_LINER_TEMPLATES["advice"].format(sessions=sessions)

    if waste_pct < 5:
        template = ONE_LINER_TEMPLATES["comfort"]
    elif waste_pct < 20:
        template = ONE_LINER_TEMPLATES["reminder"]
    else:
        template = ONE_LINER_TEMPLATES["warning"]

    return template.format(
        total_tokens=total_tokens,
        total_cost=total_cost,
        waste_pct=waste_pct,
        waste_cost=waste_cost,
    )


def generate_json_report(analysis: dict) -> str:
    """Simulate generating a JSON report from analysis data."""
    report = {
        "token_audit_report": {
            "generated_at": "2026-05-27T12:00:00Z",
            "report_version": "1.0",
        },
        "summary": {
            "one_liner": generate_one_liner(
                waste_pct=analysis.get("waste_pct", 0),
                total_tokens=analysis["totals"]["total_tokens"],
                total_cost=analysis["totals"]["total_cost_usd"],
                waste_cost=analysis["waste"]["total_estimated_waste_cost"],
                sessions=analysis["totals"]["total_sessions"],
                db_exists=True,
            ),
        },
        "totals": analysis["totals"],
        "source_distribution": analysis["source_distribution"],
        "waste": analysis["waste"],
        "cron": analysis["cron"],
        "cost_comparison": analysis["cost_comparison"],
    }
    return json.dumps(report, indent=2, ensure_ascii=False)
