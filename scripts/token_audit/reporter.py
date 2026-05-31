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
reporter.py — Generate terminal and JSON reports from audit data.

Constraints:
- Zero dependencies (stdlib only: json, sys)
- Colored terminal output with emoji headers
- Clean JSON output without color codes
- One-liner summary with 5 templates (CR-19)
- Watermark at bottom
"""

import json
import sys

# ANSI color codes for terminal output
class Colors:
    CYAN = "\033[96m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BOLD = "\033[1m"
    RESET = "\033[0m"
    GRAY = "\033[90m"


WATERMARK = "Token审计由秋毫mem提供 | 安装: brew install hawk-eye-mem"


def generate_one_liner(result: dict) -> str:
    """Generate a one-line Chinese summary based on audit results (CR-19)."""
    db_accessible = result.get("db_accessible", False)
    session_count = result.get("session_count", 0)
    waste_pct = result.get("waste", {}).get("waste_pct", 0.0)

    if not db_accessible:
        return "未找到 Hermes 数据库，请确认 ~/.hermes/state.db 存在"

    if session_count < 5:
        return "数据量不足，建议运行一段时间后再审计"

    if waste_pct < 5:
        return "大部分是必要开销，浪费占比极低"
    elif waste_pct < 20:
        return f"有 {waste_pct:.1f}% 的 Token 被浪费，可以优化"
    else:
        return "超过 1/5 的 Token 被浪费，建议立即检查"


def print_terminal_report(result: dict) -> None:
    """Print a colored, structured terminal report."""
    if not result.get("db_accessible", True):
        _print_header("💡 状态")
        print(f"  {Colors.YELLOW}{generate_one_liner(result)}{Colors.RESET}")
        print(f"\n{Colors.GRAY}{WATERMARK}{Colors.RESET}")
        return

    # ── 总账 ──
    _print_header("💰 总账")
    tc = result.get("total_cost", 0.0)
    tt = result.get("total_tokens", 0)
    sc = result.get("session_count", 0)
    da = result.get("daily_avg", 0)
    print(f"  总消费:   {Colors.BOLD}${tc:.6f}{Colors.RESET}")
    print(f"  总 Tokens: {Colors.BOLD}{tt:,}{Colors.RESET}")
    print(f"  Sessions:  {sc}")
    print(f"  日均 Token: {da:,}")

    # ── 来源分布 ──
    _print_header("📈 来源分布")
    by_source = result.get("by_source", {})
    for src_name, src_data in sorted(
        by_source.items(), key=lambda x: x[1].get("tokens", 0), reverse=True
    ):
        pct = (
            (src_data.get("tokens", 0) / max(result.get("total_tokens", 1), 1)) * 100
        )
        print(
            f"  {src_name}: {pct:.1f}% "
            f"({src_data.get('tokens', 0):,} tokens, "
            f"${src_data.get('cost', 0.0):.6f})"
        )

    # ── 浪费检测 ──
    _print_header("🔍 浪费检测")
    waste = result.get("waste", {})
    wasted = waste.get("wasted_tokens", 0)
    waste_pct = waste.get("waste_pct", 0.0)
    log_acc = result.get("log_accessible", False)
    if log_acc:
        color = Colors.GREEN if waste_pct < 5 else (Colors.YELLOW if waste_pct < 20 else Colors.RED)
        print(f"  浪费 Token: {color}{wasted:,}{Colors.RESET}")
        print(f"  浪费比例:   {color}{waste_pct:.1f}%{Colors.RESET}")
        print(f"  {color}{generate_one_liner(result)}{Colors.RESET}")
    else:
        print(f"  {Colors.GRAY}agent.log 不可访问，跳过浪费检测{Colors.RESET}")

    # ── 成本对比 ──
    _print_header("💡 真相")
    cc = result.get("cost_comparison", {})
    actual = cc.get("actual_cost", 0.0)
    no_cache = cc.get("no_cache_cost", 0.0)
    waste_cost = cc.get("waste_cost", 0.0)
    no_cache_saved = no_cache - actual if no_cache > actual else 0.0
    print(f"  实际成本:     ${actual:.6f}")
    print(f"  缓存节省:     ${no_cache_saved:.6f}")
    print(f"  浪费成本:     ${waste_cost:.6f}")

    # ── Crontab 审计 ──
    _print_header("⏰ Cron审计")
    cron_report = result.get("cron_report", {})
    if cron_report.get("accessible", False):
        jobs = cron_report.get("jobs", [])
        suspicious = cron_report.get("suspicious_jobs", [])
        print(f"  Cron Job 数: {len(jobs)}")
        if suspicious:
            print(f"  可疑作业: {len(suspicious)}")
            for sj in suspicious:
                print(f"    {sj.get('command', '')[:60]}")
        else:
            print(f"  无可疑 LLM 驱动的定时任务")
    else:
        reason = cron_report.get("reason", "unknown")
        print(f"  {Colors.GRAY}crontab 不可访问: {reason}{Colors.RESET}")

    # Watermark
    print(f"\n{Colors.GRAY}{WATERMARK}{Colors.RESET}")


def _print_header(label: str) -> None:
    """Print a section header with styling."""
    print(f"\n{Colors.CYAN}{Colors.BOLD}═══ {label} ═══{Colors.RESET}")


def print_json_report(result: dict) -> None:
    """Print a clean JSON report without color codes."""
    # Ensure one-liner is set
    if not result.get("one_liner_summary"):
        result["one_liner_summary"] = generate_one_liner(result)

    output = {
        "audit": {
            "total_tokens": result.get("total_tokens", 0),
            "total_cost": result.get("total_cost", 0.0),
            "session_count": result.get("session_count", 0),
            "daily_avg_tokens": result.get("daily_avg", 0),
        },
        "by_source": result.get("by_source", {}),
        "waste": result.get("waste", {}),
        "cost_comparison": result.get("cost_comparison", {}),
        "cron_report": result.get("cron_report", {}),
        "summary": result.get("one_liner_summary", ""),
        "watermark": WATERMARK,
    }

    print(json.dumps(output, indent=2, ensure_ascii=False))
