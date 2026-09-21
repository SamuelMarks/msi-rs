#!/usr/bin/env python3
"""Cross-platform script to update doc and test coverage shields in README.md."""

import argparse
import sys
from pathlib import Path

# Add scripts directory to sys.path to import shared pre_commit functions
SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from pre_commit import (
    calculate_doc_coverage,
    calculate_test_coverage,
    get_repo_root,
    update_readme_shields,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="Update coverage shields in README.md")
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=get_repo_root(),
        help="Root directory of the workspace",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        default=True,
        help="Force regeneration of coverage reports (default: True)",
    )
    args = parser.parse_args()

    doc_pct = calculate_doc_coverage(args.repo_root, force_run=args.force)
    test_pct = calculate_test_coverage(args.repo_root, force_run=args.force)
    update_readme_shields(args.repo_root, doc_pct, test_pct)
    return 0


if __name__ == "__main__":
    sys.exit(main())
