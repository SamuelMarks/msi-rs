"""Tests for scripts/update-shields.py to achieve 100% test and doc coverage."""

import importlib
import runpy
import sys
from pathlib import Path
from unittest.mock import patch

repo_root = Path(__file__).resolve().parent.parent.parent
scripts_dir = repo_root / "scripts"
if str(scripts_dir) not in sys.path:
    sys.path.insert(0, str(scripts_dir))


def test_update_shields_main() -> None:
    """Verifies that update_shields main calls coverage calculations and shield updating."""
    update_shields = importlib.import_module("update-shields")

    with (
        patch("update-shields.get_repo_root", return_value=repo_root),
        patch("update-shields.calculate_doc_coverage", return_value=100.0) as mock_doc,
        patch(
            "update-shields.calculate_test_coverage", return_value=100.0
        ) as mock_test,
        patch("update-shields.update_readme_shields") as mock_update,
        patch.object(
            sys, "argv", ["update-shields.py", "--repo-root", str(repo_root), "--force"]
        ),
    ):
        ret = update_shields.main()
        assert ret == 0
        mock_doc.assert_called_once_with(repo_root, force_run=True)
        mock_test.assert_called_once_with(repo_root, force_run=True)
        mock_update.assert_called_once_with(repo_root, 100.0, 100.0)


def test_update_shields_run_as_main() -> None:
    """Verifies execution when update-shields.py is invoked as __main__."""
    script_path = scripts_dir / "update-shields.py"

    # Ensure scripts_dir is not in sys.path temporarily to exercise sys.path insertion branch
    orig_path = list(sys.path)
    try:
        sys.path = [p for p in sys.path if str(scripts_dir) != p]
        with (
            patch("pre_commit.get_repo_root", return_value=repo_root),
            patch("pre_commit.calculate_doc_coverage", return_value=100.0),
            patch("pre_commit.calculate_test_coverage", return_value=100.0),
            patch("pre_commit.update_readme_shields"),
            patch.object(
                sys, "argv", ["update-shields.py", "--repo-root", str(repo_root)]
            ),
            patch.object(sys, "exit") as mock_exit,
        ):
            runpy.run_path(str(script_path), run_name="__main__")
            mock_exit.assert_called_once_with(0)
    finally:
        sys.path = orig_path
