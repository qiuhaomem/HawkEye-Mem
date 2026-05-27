"""
analyzer.py — Combine parsed data and compute token audit analytics.

Constraints:
- Zero dependencies (stdlib only)
- Hardcoded provider pricing table
- All calculations based on aggregate data only
"""

from . import state_db_parser, agent_log_parser, cron_parser

# Provider pricing per token (USD)
# Source: official provider pricing pages as of 2025
PROVIDER_PRICING = {
    "deepseek": {
        "input": 0.00000015,
        "output": 0.00000060,
        "cache_read": 0.000000015,
    },
    "openai": {
        "input": 0.00000250,
        "output": 0.00001000,
        "cache_read": 0.00000250,  # same as input for non-cached
    },
    "anthropic": {
        "input": 0.00000300,
        "output": 0.00001500,
        "cache_read": 0.00000300,  # same as input for non-cached
    },
}


def analyze(
    days: int | None = None,
    compare_days: str | None = None,
    source_filter: str | None = None,
) -> dict:
    """
    Run full token audit analysis.

    Args:
        days: Number of days to audit (None = today only).
        compare_days: Comma-separated day counts for comparison (e.g., "7,30").
        source_filter: Optional source name to filter by.

    Returns:
        dict with complete audit results.
    """
    # 1. Parse state.db
    db_data = state_db_parser.parse_state_db(days=days)

    # 2. Parse agent.log for errors
    log_data = agent_log_parser.parse_agent_log()

    # 3. Parse crontab
    cron_data = cron_parser.parse_cron()

    # 4. Calculate results
    result = _compute_analysis(db_data, log_data, cron_data, days)

    # 5. Comparison period (if requested)
    if compare_days:
        parts = [int(d.strip()) for d in compare_days.split(",") if d.strip().isdigit()]
        if len(parts) >= 2:
            d1, d2 = parts[0], parts[1]
            data_1 = state_db_parser.parse_state_db(days=d1)
            data_2 = state_db_parser.parse_state_db(days=d2)
            result["comparison"] = {
                f"period_{d1}d": _summarize_period(data_1),
                f"period_{d2}d": _summarize_period(data_2),
            }

    return result


def _compute_analysis(
    db_data: dict | None,
    log_data: dict,
    cron_data: dict,
    days: int | None,
) -> dict:
    """Core computation from parsed data."""
    if db_data is None or db_data.get("session_count", 0) == 0:
        return {
            "total_tokens": 0,
            "total_cost": 0.0,
            "session_count": 0,
            "daily_avg": 0,
            "by_source": {},
            "waste": {
                "wasted_tokens": log_data.get("estimated_wasted_tokens", 0),
                "waste_pct": 0.0,
            },
            "cost_comparison": {
                "actual_cost": 0.0,
                "no_cache_cost": 0.0,
                "waste_cost": 0.0,
            },
            "cron_report": cron_data,
            "one_liner_summary": "",
            "db_accessible": db_data is not None,
            "log_accessible": log_data.get("log_accessible", False),
        }

    total_tokens = db_data["total_tokens"]
    total_cost = db_data["total_cost"]
    session_count = db_data["session_count"]
    by_source = db_data["by_source"]

    # Daily average (use 'days' or estimate from session timestamps)
    effective_days = days if days and days > 0 else 1
    daily_avg = total_tokens // max(effective_days, 1)

    # Waste analysis
    wasted_tokens = log_data.get("estimated_wasted_tokens", 0)
    waste_pct = (wasted_tokens / total_tokens * 100) if total_tokens > 0 else 0.0

    # Cost comparison
    waste_cost = _compute_waste_cost(wasted_tokens)
    no_cache_cost = _compute_no_cache_cost(db_data)

    # Generate one-liner summary
    one_liner = _generate_one_liner(
        db_accessible=True,
        session_count=session_count,
        waste_pct=waste_pct,
    )

    return {
        "total_tokens": total_tokens,
        "total_input_tokens": db_data["total_input_tokens"],
        "total_output_tokens": db_data["total_output_tokens"],
        "total_cache_read_tokens": db_data["total_cache_read_tokens"],
        "total_cost": total_cost,
        "session_count": session_count,
        "daily_avg": daily_avg,
        "by_source": by_source,
        "waste": {
            "wasted_tokens": wasted_tokens,
            "waste_pct": round(waste_pct, 2),
        },
        "cost_comparison": {
            "actual_cost": round(total_cost, 6),
            "no_cache_cost": round(no_cache_cost, 6),
            "waste_cost": round(waste_cost, 6),
        },
        "cron_report": cron_data,
        "one_liner_summary": one_liner,
        "db_accessible": True,
        "log_accessible": log_data.get("log_accessible", False),
        "has_known_issues": log_data.get("has_known_issues", False),
    }


def _compute_no_cache_cost(db_data: dict) -> float:
    """Compute what cost would be without cache read pricing."""
    # Estimate using average pricing from by_source data
    # Without cache, all tokens use the non-cache input/output rates
    # This is a rough estimate: input * input_price + output * output_price
    total_input = db_data.get("total_input_tokens", 0)
    total_output = db_data.get("total_output_tokens", 0)
    total_cache = db_data.get("total_cache_read_tokens", 0)

    # Use deepseek pricing as default for estimation
    input_price = PROVIDER_PRICING["deepseek"]["input"]
    output_price = PROVIDER_PRICING["deepseek"]["output"]

    # Without cache, cache_read tokens are charged at input rate
    no_cache = (
        (total_input + total_cache) * input_price
        + total_output * output_price
    )
    return no_cache


def _compute_waste_cost(wasted_tokens: int) -> float:
    """Estimate cost of wasted tokens."""
    return round(wasted_tokens * PROVIDER_PRICING["deepseek"]["output"], 6)


def _generate_one_liner(
    db_accessible: bool,
    session_count: int,
    waste_pct: float,
) -> str:
    """Generate a one-line summary based on waste percentage."""
    if not db_accessible:
        return "未找到 Hermes 数据库，请确认 ~/.hermes/state.db 存在"

    if session_count < 5:
        return "数据量不足，建议运行一段时间后再审计"

    if waste_pct < 5:
        return "✅ 大部分是必要开销，浪费占比极低"
    elif waste_pct < 20:
        return f"⚠️ 有 {waste_pct:.1f}% 的 Token 被浪费，可以优化"
    else:
        return "🚨 超过 1/5 的 Token 被浪费，建议立即检查"


def _summarize_period(data: dict | None) -> dict:
    """Summarize a time period for comparison."""
    if data is None:
        return {"total_tokens": 0, "total_cost": 0.0, "session_count": 0}
    return {
        "total_tokens": data.get("total_tokens", 0),
        "total_cost": data.get("total_cost", 0.0),
        "session_count": data.get("session_count", 0),
    }
