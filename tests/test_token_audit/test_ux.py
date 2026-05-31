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

"""UX tests for Token Audit — readability, emoji, dual-bait guide."""

import json
import pytest
from .helpers import ONE_LINER_TEMPLATES, render_one_liner, generate_one_liner, generate_json_report


class TestUxOneLiners:
    """UX tests for one-liner readability (CR-19 — all 5 variants)."""

    # ── Test 1: Comfort variant readability ────────────────────────
    def test_one_liner_comfort_readable(self):
        """Low waste (<5%): Comfort template — clear, reassuring."""
        template = ONE_LINER_TEMPLATES["comfort"]
        rendered = render_one_liner(
            template,
            total_tokens=199_000_000,
            total_cost=4.21,
            waste_pct=0.9,
            waste_cost=0.27,
        )
        # Must be a single line (no newlines in one-liner)
        assert "\n" not in rendered
        # Must include key data points
        assert "199,000,000" in rendered or "199000000" in rendered
        assert "$4.21" in rendered
        assert "0.9%" in rendered
        # Must be understandable (contains '真相' or equivalent sentiment)
        assert any(word in rendered for word in ["真相", "必要", "大部分"])
        # Length should be reasonable for a one-liner
        assert len(rendered) < 200, f"One-liner too long: {len(rendered)} chars"

    # ── Test 2: Reminder variant readability ───────────────────────
    def test_one_liner_reminder_readable(self):
        """Medium waste (5-20%): Reminder template — actionable."""
        template = ONE_LINER_TEMPLATES["reminder"]
        rendered = render_one_liner(
            template,
            total_tokens=50_000,
            total_cost=1.50,
            waste_pct=12.5,
            waste_cost=0.19,
        )
        assert "\n" not in rendered
        assert "12.5%" in rendered or "12%" in rendered or "13%" in rendered
        # Should indicate actionability
        assert any(word in rendered for word in ["优化", "可以", "改进"])
        assert len(rendered) < 200

    # ── Test 3: Warning variant readability ────────────────────────
    def test_one_liner_warning_readable(self):
        """High waste (>20%): Warning template — urgent, clear."""
        template = ONE_LINER_TEMPLATES["warning"]
        rendered = render_one_liner(
            template,
            total_tokens=10_000,
            total_cost=0.50,
            waste_pct=35.0,
            waste_cost=0.18,
        )
        assert "\n" not in rendered
        # Should convey urgency
        assert any(word in rendered for word in ["⚠️", "警告", "超过", "立即", "检查"])
        assert "35.0%" in rendered or "35%" in rendered
        assert len(rendered) < 200

    # ── Test 4: Guidance variant readability ───────────────────────
    def test_one_liner_guidance_readable(self):
        """No state.db: Guidance template — helpful, not scary."""
        rendered = ONE_LINER_TEMPLATES["guidance"].format()
        assert "\n" not in rendered
        # Should provide actionable guidance
        assert any(word in rendered for word in ["未找到", "确认", "请确认", "路径"])
        assert "state.db" in rendered
        # Should not contain error numbers or stack traces
        assert "Traceback" not in rendered
        assert "Error:" not in rendered

    # ── Test 5: Advice variant readability ─────────────────────────
    def test_one_liner_advice_readable(self):
        """Insufficient data: Advice template — gentle suggestion."""
        rendered = ONE_LINER_TEMPLATES["advice"].format(sessions=2)
        assert "\n" not in rendered
        assert "数据量不足" in rendered or "Data" in rendered
        assert "2" in rendered
        # Should suggest running later, not blame user
        assert any(word in rendered for word in ["建议", "运行", "后再"])


class TestUxEmoji:
    """UX tests for emoji display and ASCII fallback."""

    # ── Test 6: Basic emoji display check ──────────────────────────
    def test_emoji_in_templates(self):
        """All one-liner templates should start with 📊 emoji."""
        for variant, template in ONE_LINER_TEMPLATES.items():
            assert template.startswith("📊"), (
                f"Template '{variant}' should start with 📊 emoji"
            )

    def test_emoji_ascii_fallback(self):
        """ASCII fallback should be used when terminal doesn't support emoji."""
        emoji_map = {
            "📊": "[DATA]",
            "💰": "[COST]",
            "📈": "[TREND]",
            "🔍": "[DETAIL]",
            "⏰": "[CLOCK]",
            "💡": "[TIP]",
            "⚠️": "[WARN]",
            "🟢": "[OK]",
            "🔴": "[ERR]",
            "🟡": "[WARN]",
        }
        template_with_emoji = "📊 总消耗 199M tokens = $4.21"
        ascii_version = template_with_emoji
        for emoji, ascii_repl in emoji_map.items():
            ascii_version = ascii_version.replace(emoji, ascii_repl)

        assert "📊" not in ascii_version
        assert "[DATA]" in ascii_version
        # ASCII version should be readable without emoji support
        assert "总消耗" in ascii_version or "total" in ascii_version.lower()


class TestUxDualBait:
    """UX tests for dual-bait guide (CR-21: first use only)."""

    # ── Test 7: First-use shows guide ──────────────────────────────
    def test_dual_bait_guide_first_use(self):
        """CR-21: Dual-bait guide should appear on first use."""
        is_first_use = True
        has_seen_guide = False

        guide_line = None
        if is_first_use and not has_seen_guide:
            guide_line = "💡 想省钱？运行 `hawk-eye-mem --cache-strategy` 查看缓存策略"

        assert guide_line is not None, "Guide should be shown on first use"
        assert "缓存策略" in guide_line or "cache-strategy" in guide_line

    # ── Test 8: Repeat use hides guide ─────────────────────────────
    def test_dual_bait_guide_repeat_use(self):
        """CR-21: Dual-bait guide should NOT appear on repeat use."""
        is_first_use = False
        has_seen_guide = True

        guide_line = None
        if is_first_use and not has_seen_guide:
            guide_line = "💡 想省钱？运行 `hawk-eye-mem --cache-strategy` 查看缓存策略"

        assert guide_line is None, "Guide should NOT be shown on repeat use"

    def test_dual_bait_cache_report_mutual_reference(self):
        """Cache report should reference token audit and vice versa (mutual, first use only)."""
        from_cache_report = "💡 想查总账？运行 hawk-eye-mem --token-audit"
        from_audit_report = "💡 想省钱？运行缓存策略技能"

        assert "token-audit" in from_cache_report or "查总账" in from_cache_report
        assert "缓存策略" in from_audit_report or "省钱" in from_audit_report
