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
agent_log_parser.py — Parse ~/.hermes/agent.log for error patterns.

Constraints:
- Zero dependencies (stdlib only: re, os)
- Regex only matches error patterns, NEVER extracts chat content
- Max 10MB read (seek to end-10MB)
- Each error ~500 wasted tokens estimate
"""

import re
import os

HERMES_AGENT_LOG = os.path.expanduser("~/.hermes/agent.log")
MAX_READ_BYTES = 10 * 1024 * 1024  # 10 MB
WASTED_TOKENS_PER_ERROR = 500

# Error patterns — these match failure indicators, NEVER chat content
ERROR_PATTERNS = {
    "rate_limit_429": re.compile(r"\b429\b.*\b(rate|limit|too many|retry)", re.IGNORECASE),
    "connection_refused": re.compile(r"Connection refused", re.IGNORECASE),
    "mcp_failed": re.compile(r"MCP.*failed", re.IGNORECASE),
    "retry": re.compile(r"\bretry\b", re.IGNORECASE),
    "error": re.compile(r"\berror\b", re.IGNORECASE),
}


def parse_agent_log() -> dict:
    """
    Read the last 10MB of ~/.hermes/agent.log and count error patterns.

    Returns:
        dict with:
            - errors: {pattern_name: count}
            - estimated_wasted_tokens: int
            - has_known_issues: bool
            - log_accessible: bool
    """
    log_path = HERMES_AGENT_LOG

    if not os.path.isfile(log_path):
        return {
            "errors": {},
            "estimated_wasted_tokens": 0,
            "has_known_issues": False,
            "log_accessible": False,
        }

    try:
        file_size = os.path.getsize(log_path)
    except OSError:
        return {
            "errors": {},
            "estimated_wasted_tokens": 0,
            "has_known_issues": False,
            "log_accessible": False,
        }

    try:
        with open(log_path, "r", errors="replace") as f:
            # Seek to end-10MB (or start if file is smaller)
            if file_size > MAX_READ_BYTES:
                f.seek(file_size - MAX_READ_BYTES)
                # Skip partial line at the start
                f.readline()

            content = f.read()
    except (OSError, PermissionError):
        return {
            "errors": {},
            "estimated_wasted_tokens": 0,
            "has_known_issues": False,
            "log_accessible": False,
        }

    # Count error patterns — only aggregate, never extract content
    error_counts: dict[str, int] = {}
    total_errors = 0

    for name, pattern in ERROR_PATTERNS.items():
        count = len(pattern.findall(content))
        if count > 0:
            error_counts[name] = count
            total_errors += count

    estimated_wasted = total_errors * WASTED_TOKENS_PER_ERROR

    return {
        "errors": error_counts,
        "estimated_wasted_tokens": estimated_wasted,
        "has_known_issues": total_errors > 0,
        "log_accessible": True,
    }
