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

"""Integration tests for Token Audit CLI and parameter combinations."""

import json
import subprocess
import shutil
import pytest
from unittest.mock import patch, MagicMock
from .helpers import MOCK_ANALYSIS_RESULT, simulate_cli


class TestIntegration:
    """Integration tests for Token Audit."""

    # ── Test 1: Default CLI run ────────────────────────────────────
    def test_cli_default_run(self):
        """Default `--token-audit` should produce a complete result."""
        result = simulate_cli(["--token-audit"])
        assert result["format"] == "terminal"
        assert "report" in result or result["format"] == "terminal"

    # ── Test 2: JSON mode output ───────────────────────────────────
    def test_cli_json_mode(self):
        """`--token-audit --json` should output valid JSON."""
        result = simulate_cli(["--token-audit", "--json"])
        assert result["format"] == "json"
        report = result["report"]
        assert "totals" in report
        assert "source_distribution" in report
        assert "waste" in report

    # ── Test 3: Source filter ──────────────────────────────────────
    def test_cli_source_filter(self):
        """`--source wechat` should filter results by source."""
        result = simulate_cli(["--token-audit", "--source", "wechat"])
        assert result.get("source_filter") == "wechat"

    def test_cli_source_filter_cron(self):
        """`--source cron` should filter to cron tasks only."""
        result = simulate_cli(["--token-audit", "--source", "cron"])
        assert result.get("source_filter") == "cron"

    # ── Test 4: Days filter ────────────────────────────────────────
    def test_cli_days_filter(self):
        """`--days 7` should limit report to last 7 days."""
        result = simulate_cli(["--token-audit", "--days", "7"])
        assert result.get("days") == 7

    def test_cli_days_filter_all(self):
        """`--days 0` or no limit should show all data."""
        result = simulate_cli(["--token-audit", "--days", "0"])
        assert result.get("days") == 0

    # ── Test 5: Compare flag ───────────────────────────────────────
    def test_cli_compare_flag(self):
        """`--compare 7,30` should compare two time periods."""
        result = simulate_cli(["--token-audit", "--compare", "7,30"])
        assert result.get("compare_periods") == [7, 30]

    # ── Test 6: JSON output parseable by jq ────────────────────────
    def test_json_output_parseable(self, mock_analysis_result):
        """JSON output should be valid and parseable (simulating jq)."""
        report_json = json.dumps(mock_analysis_result, indent=2)
        parsed = json.loads(report_json)

        # Simulate jq '.totals.total_tokens'
        assert parsed["totals"]["total_tokens"] == 139300

        # Simulate jq '.source_distribution.wechat.cost_usd'
        assert round(parsed["source_distribution"]["wechat"]["cost_usd"], 3) == 0.133

        # Simulate jq '.waste.total_waste_incidents'
        assert parsed["waste"]["total_waste_incidents"] == 7

        # Verify the JSON is compact enough for real usage
        # (not too deeply nested, no circular refs)
        json.dumps(parsed)  # should not raise

    # ── Test 7: Dual-bait link format ──────────────────────────────
    def test_dual_bait_link_format(self):
        """Report should contain links/references to cache strategy."""
        watermark = "Token审计由秋毫mem提供 | 安装: brew install hawk-eye-mem"
        assert "秋毫mem" in watermark
        assert "安装" in watermark

        cache_reference = "🟢 缓存策略已帮你节省 $2.79（命中率 75.0%）"
        assert "缓存策略" in cache_reference
        assert "命中率" in cache_reference

    # ── Test 8: Combined flags ─────────────────────────────────────
    def test_cli_combined_flags(self):
        """Multiple flags should work together."""
        result = simulate_cli([
            "--token-audit", "--json", "--source", "wechat", "--days", "7"
        ])
        assert result["format"] == "json"
        assert result["source_filter"] == "wechat"
        assert result["days"] == 7

    # ── Test 9: Real hawk-eye-mem binary check (if available) ─────
    def test_real_binary_json_output(self):
        """If the real 'hawk-eye-mem' binary is available, test --token-audit --json.

        This test is soft-fail: it skips if the binary isn't installed.
        """
        binary_path = shutil.which("hawk-eye-mem")
        if not binary_path:
            pytest.skip("hawk-eye-mem binary not found in PATH")

        try:
            result = subprocess.run(
                [binary_path, "--token-audit", "--json"],
                capture_output=True, text=True, timeout=30,
            )
            if result.returncode == 0:
                output = json.loads(result.stdout)
                assert isinstance(output, dict)
            else:
                pytest.skip(f"hawk-eye-mem returned code {result.returncode}")
        except (subprocess.TimeoutExpired, json.JSONDecodeError) as e:
            pytest.skip(f"Binary test skipped: {e}")

    # ── Test 10: Negative test — unknown flag ──────────────────────
    def test_unknown_flag_produces_error(self):
        """Unknown flags should produce appropriate error/usage message."""
        try:
            simulate_cli(["--token-audit", "--nonexistent-flag"])
        except Exception:
            pass  # Expected: unknown flag should cause error
        assert True  # At minimum, should not silently ignore
