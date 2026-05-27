"""
cron_parser.py — Parse crontab and correlate with token usage.

Constraints:
- Zero dependencies (stdlib only: subprocess, re, os)
- Handle no crontab access gracefully
- Detect suspicious (LLM-driven) cron jobs
"""

import subprocess
import re
import os

# Patterns that indicate an LLM-driven cron job
SUSPICIOUS_PATH_PATTERNS = re.compile(
    r"\b(hermes|python|curl|wget|node|deno|bun)\b", re.IGNORECASE
)

# Common cron schedule patterns
CRON_SCHEDULE_RE = re.compile(
    r"^\s*((@\w+)|"
    r"(\*|[0-9,\-*/]+)\s+"
    r"(\*|[0-9,\-*/]+)\s+"
    r"(\*|[0-9,\-*/]+)\s+"
    r"(\*|[0-9,\-*/]+)\s+"
    r"(\*|[0-9,\-/]+))"
)


def parse_cron() -> dict:
    """
    Run `crontab -l` and parse cron jobs.

    Returns:
        dict with:
            - accessible: bool — whether crontab could be read
            - reason: str | None — if not accessible, why
            - jobs: list of {schedule, command, matched_api_calls, estimated_tokens}
            - suspicious_jobs: list of job dicts that look LLM-driven
    """
    try:
        result = subprocess.run(
            ["crontab", "-l"],
            capture_output=True,
            text=True,
            timeout=10,
        )
    except FileNotFoundError:
        return {
            "accessible": False,
            "reason": "cron_not_installed",
            "jobs": [],
            "suspicious_jobs": [],
        }
    except subprocess.TimeoutExpired:
        return {
            "accessible": False,
            "reason": "timeout",
            "jobs": [],
            "suspicious_jobs": [],
        }
    except PermissionError:
        return {
            "accessible": False,
            "reason": "no_permission",
            "jobs": [],
            "suspicious_jobs": [],
        }

    if result.returncode != 0:
        stderr = result.stderr.strip().lower()
        if "no crontab" in stderr:
            return {
                "accessible": True,
                "reason": "no_crontab_entries",
                "jobs": [],
                "suspicious_jobs": [],
            }
        if "permission" in stderr:
            return {
                "accessible": False,
                "reason": "no_permission",
                "jobs": [],
                "suspicious_jobs": [],
            }
        return {
            "accessible": False,
            "reason": f"crontab_error: {result.stderr.strip()}",
            "jobs": [],
            "suspicious_jobs": [],
        }

    lines = result.stdout.splitlines()
    jobs = []
    suspicious_jobs = []

    for line in lines:
        line = line.strip()
        # Skip comments and empty lines
        if not line or line.startswith("#"):
            continue

        match = CRON_SCHEDULE_RE.match(line)
        if not match:
            continue

        schedule = match.group(1).strip()
        command = line[match.end():].strip()

        if not command:
            continue

        job = {
            "schedule": schedule,
            "command": command,
            "matched_api_calls": 0,
            "estimated_tokens": 0,
        }
        jobs.append(job)

        # Check if this job looks suspicious (LLM-driven)
        if SUSPICIOUS_PATH_PATTERNS.search(command):
            suspicious_jobs.append(job)

    return {
        "accessible": True,
        "reason": None,
        "jobs": jobs,
        "suspicious_jobs": suspicious_jobs,
    }
