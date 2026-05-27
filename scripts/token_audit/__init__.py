"""
Token Audit — CLI tool for auditing Hermes Agent token usage.

Usage:
    python -m scripts.token_audit --token-audit
    python -m scripts.token_audit --token-audit --days 7
    python -m scripts.token_audit --token-audit --json
    python -m scripts.token_audit --token-audit --source wechat
    python -m scripts.token_audit --token-audit --compare 7,30

Constraints:
- Zero external dependencies (stdlib only)
- All data processing local only — never uploads anything
"""

import argparse
import sys

from . import analyzer
from . import reporter


def main() -> None:
    """Main entry point for the token audit CLI."""
    parser = argparse.ArgumentParser(
        description="Token Audit — Hermes Agent token usage analysis",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --token-audit                    # Run audit for today
  %(prog)s --token-audit --days 7            # Last 7 days
  %(prog)s --token-audit --json              # JSON output
  %(prog)s --token-audit --source wechat     # Filter by source
  %(prog)s --token-audit --compare 7,30      # Compare two periods
        """,
    )

    parser.add_argument(
        "--token-audit",
        action="store_true",
        help="Run token audit and print summary",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        dest="json_output",
        help="Output results in JSON format",
    )
    parser.add_argument(
        "--days",
        type=int,
        default=None,
        help="Audit last N days (default: today only)",
    )
    parser.add_argument(
        "--source",
        type=str,
        default=None,
        help="Filter by source (weixin, cron, api_server, subagent)",
    )
    parser.add_argument(
        "--compare",
        type=str,
        default=None,
        help="Compare two time periods, e.g. 7,30",
    )

    args = parser.parse_args()

    if not args.token_audit:
        parser.print_help()
        sys.exit(0)

    # Run analysis
    result = analyzer.analyze(
        days=args.days,
        compare_days=args.compare,
        source_filter=args.source,
    )

    # Generate one-liner
    result["one_liner_summary"] = reporter.generate_one_liner(result)

    # Output
    if args.json_output:
        reporter.print_json_report(result)
    else:
        reporter.print_terminal_report(result)


if __name__ == "__main__":
    main()
