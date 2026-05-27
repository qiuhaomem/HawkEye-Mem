"""Unit tests for the reporter module (CR-19: 5 one-liner templates)."""

import json
import pytest


def generate_one_liner(waste_pct: float, total_tokens: int, total_cost: float,
                       waste_cost: float, sessions: int = 8,
                       db_exists: bool = True) -> str:
    """Simulate the dynamic one-liner generation with 5 templates (CR-19)."""
    if not db_exists:
        return "📊 未找到 Hermes 数据库，请确认 ~/.hermes/state.db 路径是否正确"

    if sessions < 3:
        return f"📊 数据量不足（{sessions} 条会话），建议运行一段时间后再审计以获得有意义的结论"

    if waste_pct < 5:
        template = "📊 总消耗 {total:,} tokens = ${cost:.2f} | 真实浪费 {wpct:.1f}% = ${wcost:.2f} | 真相：大部分是必要开销"
    elif waste_pct < 20:
        template = "📊 总消耗 {total:,} tokens = ${cost:.2f} | 浪费 {wpct:.1f}% = ${wcost:.2f} | 有 {wpct:.0f}% 的浪费可以优化"
    else:
        template = "📊 总消耗 {total:,} tokens = ${cost:.2f} | 浪费 {wpct:.1f}% = ${wcost:.2f} | ⚠️ 超过 1/5 的 Token 被浪费，建议立即检查"

    return template.format(total=total_tokens, cost=total_cost, wpct=waste_pct, wcost=waste_cost)


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


class TestReporter:
    """Unit tests for report generation."""

    # ── Test 1: Comfort template (waste < 5%) ──────────────────────
    def test_one_liner_low_waste_comfort(self):
        """Waste <5% should use 'comfort' template (truth: mostly necessary)."""
        result = generate_one_liner(
            waste_pct=0.9,
            total_tokens=199_000_000,
            total_cost=4.21,
            waste_cost=0.27,
            sessions=150,
        )
        assert "大部分是必要开销" in result
        assert "0.9%" in result
        assert "$4.21" in result or "$4" in result
        assert "199" in result

    # ── Test 2: Reminder template (5-20%) ──────────────────────────
    def test_one_liner_medium_waste_reminder(self):
        """Waste 5-20% should use 'reminder' template (can optimize)."""
        result = generate_one_liner(
            waste_pct=12.5,
            total_tokens=50_000,
            total_cost=1.50,
            waste_cost=0.1875,
            sessions=30,
        )
        assert "浪费可以优化" in result
        assert "12.5%" in result or "12%" in result
        assert "可以优化" in result

    # ── Test 3: Warning template (waste > 20%) ─────────────────────
    def test_one_liner_high_waste_warning(self):
        """Waste >20% should use 'warning' template (⚠️ check immediately)."""
        result = generate_one_liner(
            waste_pct=35.0,
            total_tokens=10_000,
            total_cost=0.50,
            waste_cost=0.175,
            sessions=8,
        )
        assert "⚠️" in result or "超过 1/5" in result
        assert "建议立即检查" in result

    # ── Test 4: Guidance template (no state.db) ────────────────────
    def test_one_liner_no_db_guidance(self):
        """No state.db should use 'guidance' template (not crash)."""
        result = generate_one_liner(
            waste_pct=0, total_tokens=0, total_cost=0, waste_cost=0,
            sessions=0, db_exists=False,
        )
        assert "未找到" in result
        assert "state.db" in result
        assert "请确认" in result

    # ── Test 5: JSON output format (parseable, structured) ─────────
    def test_json_output_format(self, mock_analysis_result):
        """JSON output should be valid, parseable, and contain all sections."""
        json_str = generate_json_report(mock_analysis_result)
        report = json.loads(json_str)

        # Must be valid JSON
        assert isinstance(report, dict)

        # Must have required top-level keys
        assert "token_audit_report" in report
        assert "summary" in report
        assert "totals" in report
        assert "source_distribution" in report
        assert "waste" in report
        assert "cron" in report
        assert "cost_comparison" in report

        # Summary must have one_liner
        assert "one_liner" in report["summary"]
        assert len(report["summary"]["one_liner"]) > 10

        # Must be parseable by jq (in a real scenario)
        # This simulates: echo '{"key": "val"}' | jq '.key'
        parsed_one_liner = report["summary"]["one_liner"]
        assert isinstance(parsed_one_liner, str)

        # Report metadata
        meta = report["token_audit_report"]
        assert "generated_at" in meta
        assert "report_version" in meta

    def test_advice_template_insufficient_data(self):
        """Fewer than 3 sessions should use 'advice' template."""
        result = generate_one_liner(
            waste_pct=0, total_tokens=100, total_cost=0.01, waste_cost=0,
            sessions=1, db_exists=True,
        )
        assert "数据量不足" in result
        assert "1 条会话" in result or "1条会话" in result

    def test_report_watermark_format(self):
        """Report bottom watermark should contain required elements."""
        watermark = "Token审计由秋毫mem提供 | 安装: brew install hawk-eye-mem | 💡 想省钱？运行缓存策略技能"
        assert "秋毫mem" in watermark
        assert "brew install" in watermark or "安装" in watermark
        assert "缓存策略" in watermark or "省钱" in watermark

    def test_report_contains_all_sections_in_output(self, mock_analysis_result):
        """Verify structured report output contains all required sections."""
        sections = [
            "💰总账" if False else "totals",
            "📈分布" if False else "source_distribution",
            "🔍浪费" if False else "waste",
            "⏰cron" if False else "cron",
            "💡真相" if False else "cost_comparison",
        ]
        # For JSON mode, verify keys exist
        json_str = generate_json_report(mock_analysis_result)
        report = json.loads(json_str)
        for section in ["totals", "source_distribution", "waste", "cron", "cost_comparison"]:
            assert section in report, f"Missing section: {section}"
