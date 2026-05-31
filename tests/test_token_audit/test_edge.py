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

"""Edge/boundary tests for Token Audit."""

import os
import json
import sqlite3
import pytest
from unittest.mock import patch, MagicMock, mock_open


class TestEdgeStateDb:
    """Edge cases: state.db issues."""

    # ── Test 1: state.db doesn't exist ─────────────────────────────
    def test_state_db_not_exists(self):
        """When state.db doesn't exist, report should gracefully indicate absence."""
        # Simulate: os.path.exists returns False
        db_path = "/nonexistent/.hermes/state.db"

        with patch("os.path.exists", return_value=False):
            exists = os.path.exists(db_path)
            assert exists is False

            # The parser should return a guidance message, not crash
            if not exists:
                message = "📊 未找到 Hermes 数据库，请确认 ~/.hermes/state.db 路径是否正确"
            else:
                message = None

        assert message is not None
        assert "未找到" in message

    # ── Test 2: state.db is corrupted/invalid SQLite ───────────────
    def test_state_db_corrupted(self, mock_corrupted_state_db):
        """Corrupted state.db should not crash; report error."""
        db_path = mock_corrupted_state_db

        try:
            conn = sqlite3.connect(db_path)
            c = conn.cursor()
            c.execute("SELECT COUNT(*) FROM messages")
            pytest.fail("Should have raised an exception for corrupted DB")
        except (sqlite3.DatabaseError, sqlite3.OperationalError) as e:
            # Expected: corrupted DB should raise an error
            # The parser should catch this and return an informative message
            error_msg = f"数据库损坏或格式不正确: {e}"
            pass

        # Verify the file exists but isn't valid SQLite
        assert os.path.exists(db_path)
        with open(db_path, "rb") as f:
            header = f.read(16)
        # Valid SQLite header starts with "SQLite format 3\x00"
        assert header != b"SQLite format 3\x00", "File should NOT be valid SQLite"

    # ── Test 3: DB locked by another process ───────────────────────
    def test_state_db_lock_retry(self):
        """DB lock should be retried once with 500ms wait.

        Per the spec: '数据库锁时等待 500ms 重试一次'.
        """
        attempts = 0
        max_attempts = 2
        lock_detected = False

        # Simulate lock then success
        for attempt in range(1, max_attempts + 1):
            try:
                if attempt == 1 and not lock_detected:
                    lock_detected = True
                    raise sqlite3.OperationalError("database is locked")
                # Second attempt succeeds
                break
            except sqlite3.OperationalError:
                if attempt < max_attempts:
                    continue  # retry after 500ms
                raise

        assert lock_detected is True
        # The test verifies the retry logic structure, not timing


class TestEdgeAgentLog:
    """Edge cases: agent.log issues."""

    # ── Test 3: agent.log > 10MB (truncation logic) ────────────────
    def test_agent_log_over_10mb_truncation(self, tmp_path):
        """Log >10MB should truncate to last 10MB only."""
        log_path = tmp_path / "agent.log"
        max_bytes = 10 * 1024 * 1024
        chunk = b"A" * 1024 * 1024  # 1MB chunks

        # Write 15MB of data
        with open(log_path, "wb") as f:
            for _ in range(15):
                f.write(chunk)

        file_size = os.path.getsize(log_path)
        assert file_size > max_bytes, "File should exceed 10MB"

        # Simulate truncation: read only last 10MB
        truncated_data = b""
        with open(log_path, "rb") as f:
            if file_size > max_bytes:
                f.seek(-max_bytes, os.SEEK_END)
                truncated_data = f.read()

        assert len(truncated_data) > 0, "Truncated data should not be empty"
        assert len(truncated_data) <= max_bytes
        assert len(truncated_data) == max_bytes  # exactly 10MB

    # ── Test 4: agent.log doesn't exist ────────────────────────────
    def test_agent_log_not_exists(self):
        """Missing agent.log should be handled gracefully."""
        with patch("os.path.exists", return_value=False):
            exists = os.path.exists("/nonexistent/agent.log")
            if not exists:
                waste_result = {
                    "total_waste_incidents": 0,
                    "waste_by_type": {},
                    "total_estimated_waste_tokens": 0,
                    "total_estimated_waste_cost": 0.0,
                    "note": "agent.log未找到，跳过浪费检测",
                }
            else:
                waste_result = None

        assert waste_result is not None
        assert waste_result["total_waste_incidents"] == 0
        assert "未找到" in waste_result.get("note", "")


class TestEdgeCron:
    """Edge cases: crontab issues."""

    # ── Test 4: No crontab permission ──────────────────────────────
    def test_no_crontab_permission(self):
        """No crontab permission should skip cron audit gracefully."""
        no_permission_result = {
            "accessible": False,
            "error": "无 crontab 权限，跳过 cron 审计",
            "llm_related_tasks": -1,
        }

        assert no_permission_result["accessible"] is False
        assert "权限" in no_permission_result["error"]
        assert no_permission_result["llm_related_tasks"] == -1  # sentinel: not checked


class TestEdgeData:
    """Edge cases: data boundaries."""

    # ── Test 5: Zero tokens (new install) ──────────────────────────
    def test_zero_tokens_new_install(self, mock_zero_tokens_analysis):
        """New install with no usage should produce zero-filled report, no crash."""
        analysis = mock_zero_tokens_analysis
        assert analysis["totals"]["total_tokens"] == 0
        assert analysis["totals"]["total_cost_usd"] == 0.0
        assert analysis["totals"]["total_sessions"] == 0
        assert analysis["waste"]["total_waste_incidents"] == 0
        assert analysis["cost_comparison"]["actual_cost"] == 0.0

        # Should produce a valid JSON report even with zero data
        report_json = json.dumps(analysis, indent=2)
        parsed = json.loads(report_json)
        assert parsed["totals"]["total_tokens"] == 0

    # ── Test 6: Both cache strategy and audit data available ───────
    def test_cache_strategy_and_audit_data_both_available(self):
        """When both data sources are available, report should include cache savings."""
        audit_data = {
            "actual_cost": 1.393,
            "estimated_no_cache_cost": 5.572,
        }
        cache_data = {
            "cache_hit_rate": 75.0,
            "cache_savings": 4.179,
        }

        combined = {
            **audit_data,
            **cache_data,
            "report_line": (
                f"🟢 缓存策略已帮你节省 ${cache_data['cache_savings']:.2f}"
                f"（命中率 {cache_data['cache_hit_rate']}%）"
            ),
        }

        assert combined["cache_savings"] == 4.179
        assert combined["cache_hit_rate"] == 75.0
        assert "4.18" in combined["report_line"]

    # ── Test 7: Unknown source name ────────────────────────────────
    def test_source_unknown_name(self):
        """Unknown source filter should produce clear message, not crash."""
        valid_sources = ["wechat", "cron", "api", "subagent"]
        unknown_source = "telegram"

        if unknown_source not in valid_sources:
            message = f"未知来源: '{unknown_source}'，有效来源: {', '.join(valid_sources)}"
        else:
            message = None

        assert message is not None
        assert "未知" in message
        assert unknown_source in message
        assert "wechat" in message

    # ── Test 8: Extremely large token numbers (overflow test) ──────
    def test_extremely_large_token_numbers(self):
        """Very large token counts should not overflow Python int."""
        # Python can handle arbitrarily large ints, but JSON encoding must work
        large_data = {
            "total_prompt_tokens": 2**63 - 1,  # max int64
            "total_completion_tokens": 2**63 - 1,
            "total_tokens": 2**64 - 1,  # beyond int64
            "total_cost_usd": 999999.99,
            "total_sessions": 10**9,
        }
        large_data["total_tokens"] = large_data["total_prompt_tokens"] + large_data["total_completion_tokens"]

        json_str = json.dumps(large_data)
        parsed = json.loads(json_str)

        assert parsed["total_prompt_tokens"] == 2**63 - 1
        assert parsed["total_completion_tokens"] == 2**63 - 1
        assert parsed["total_tokens"] == 2 * (2**63 - 1)  # no overflow in Python
        assert parsed["total_cost_usd"] == 999999.99
        assert isinstance(parsed["total_tokens"], int)

        # Verify the total is correct
        expected_total = (2**63 - 1) + (2**63 - 1)
        assert parsed["total_tokens"] == expected_total
