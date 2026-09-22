"""Comprehensive test suite for scripts/pre_commit.py to achieve 100% test and doc coverage."""

from __future__ import annotations

import json
import runpy
import subprocess
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

repo_root = Path(__file__).resolve().parent.parent.parent
scripts_dir = repo_root / "scripts"
if str(scripts_dir) not in sys.path:
    sys.path.insert(0, str(scripts_dir))

import pre_commit  # type: ignore[import-untyped]


def test_get_repo_root() -> None:
    """Verifies get_repo_root points to the expected repository workspace directory."""
    root = pre_commit.get_repo_root()
    assert root.is_dir()
    assert (root / "Cargo.toml").is_file()


def test_get_augmented_env(tmp_path: Path) -> None:
    """Verifies get_augmented_env prefixes existing candidate directories to PATH."""
    fake_home = tmp_path / "home"
    (fake_home / ".cargo" / "bin").mkdir(parents=True)

    with (
        patch("pathlib.Path.home", return_value=fake_home),
        patch.dict("os.environ", {"PATH": "/bin:/usr/bin"}),
    ):
        env = pre_commit.get_augmented_env()
        assert str(fake_home / ".cargo" / "bin") in env["PATH"]


def test_run_command_success(tmp_path: Path) -> None:
    """Verifies run_command resolves executable and invokes subprocess.run."""
    with (
        patch("shutil.which", return_value="/usr/bin/cargo"),
        patch("subprocess.run") as mock_run,
    ):
        pre_commit.run_command(["cargo", "--version"], cwd=tmp_path)
        mock_run.assert_called_once()
        args, kwargs = mock_run.call_args
        assert args[0] == ["/usr/bin/cargo", "--version"]
        assert kwargs["cwd"] == tmp_path
        assert kwargs["check"] is True


def test_run_command_explicit_env(tmp_path: Path) -> None:
    """Verifies run_command respects custom passed environment."""
    custom_env = {"PATH": "/custom/bin", "FOO": "BAR"}
    with (
        patch("shutil.which", return_value=None),
        patch("subprocess.run") as mock_run,
    ):
        pre_commit.run_command(
            ["mycmd", "arg"], cwd=tmp_path, env=custom_env, capture_output=True
        )
        args, kwargs = mock_run.call_args
        assert args[0] == ["mycmd", "arg"]
        assert kwargs["env"] == custom_env
        assert kwargs["capture_output"] is True


def test_re_stage_git_files_no_git(tmp_path: Path) -> None:
    """Verifies re_stage_git_files returns early if git binary is absent."""
    with patch("shutil.which", return_value=None):
        pre_commit.re_stage_git_files(tmp_path)


def test_re_stage_git_files_not_in_work_tree(tmp_path: Path) -> None:
    """Verifies re_stage_git_files returns early if not inside a git work tree."""
    mock_rev_parse = MagicMock(returncode=1)
    with (
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("subprocess.run", return_value=mock_rev_parse),
    ):
        pre_commit.re_stage_git_files(tmp_path)


def test_re_stage_git_files_explicit_list(tmp_path: Path) -> None:
    """Verifies re_stage_git_files stages specified files."""
    mock_rev_parse = MagicMock(returncode=0)
    mock_add = MagicMock(returncode=0)

    with (
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("subprocess.run", side_effect=[mock_rev_parse, mock_add]) as mock_run,
    ):
        pre_commit.re_stage_git_files(tmp_path, file_paths=[tmp_path / "file.rs"])
        assert mock_run.call_count == 2


def test_re_stage_git_files_cached_diff(tmp_path: Path) -> None:
    """Verifies re_stage_git_files re-stages cached modified files that exist on disk."""
    mock_rev_parse = MagicMock(returncode=0)
    mock_diff = MagicMock(returncode=0, stdout="test1.rs\nmissing.rs\n")
    mock_add = MagicMock(returncode=0)

    (tmp_path / "test1.rs").touch()

    with (
        patch("shutil.which", return_value="/usr/bin/git"),
        patch(
            "subprocess.run", side_effect=[mock_rev_parse, mock_diff, mock_add]
        ) as mock_run,
    ):
        pre_commit.re_stage_git_files(tmp_path)
        assert mock_run.call_count == 3


def test_re_stage_git_files_diff_error_and_exception(tmp_path: Path) -> None:
    """Verifies re_stage_git_files handles diff failure or exceptions gracefully."""
    mock_rev_parse = MagicMock(returncode=0)
    mock_diff = MagicMock(returncode=1)

    with (
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("subprocess.run", side_effect=[mock_rev_parse, mock_diff]),
    ):
        pre_commit.re_stage_git_files(tmp_path)

    # Subprocess exception handling
    with (
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("subprocess.run", side_effect=OSError("spawn error")),
    ):
        pre_commit.re_stage_git_files(tmp_path)


def test_hook_fmt(tmp_path: Path) -> None:
    """Verifies hook_fmt runs cargo fmt fix and check."""
    with patch("pre_commit.run_command") as mock_cmd:
        assert pre_commit.hook_fmt(tmp_path) == 0
        assert mock_cmd.call_count == 2


def test_hook_build(tmp_path: Path) -> None:
    """Verifies hook_build runs cargo build."""
    with patch("pre_commit.run_command") as mock_cmd:
        assert pre_commit.hook_build(tmp_path) == 0
        mock_cmd.assert_called_once_with(
            ["cargo", "build", "--workspace", "--all-targets"], cwd=tmp_path
        )


def test_hook_clippy(tmp_path: Path) -> None:
    """Verifies hook_clippy runs cargo clippy with pedantic checks."""
    with patch("pre_commit.run_command") as mock_cmd:
        assert pre_commit.hook_clippy(tmp_path) == 0
        mock_cmd.assert_called_once()


def test_hook_test(tmp_path: Path) -> None:
    """Verifies hook_test runs cargo test."""
    with patch("pre_commit.run_command") as mock_cmd:
        assert pre_commit.hook_test(tmp_path) == 0
        mock_cmd.assert_called_once_with(
            ["cargo", "test", "--workspace", "--all-targets"], cwd=tmp_path
        )


def test_get_badge_color() -> None:
    """Verifies percentage to color mappings."""
    assert pre_commit.get_badge_color(98.0) == "brightgreen"
    assert pre_commit.get_badge_color(95.0) == "brightgreen"
    assert pre_commit.get_badge_color(92.0) == "green"
    assert pre_commit.get_badge_color(90.0) == "green"
    assert pre_commit.get_badge_color(85.0) == "yellowgreen"
    assert pre_commit.get_badge_color(80.0) == "yellowgreen"
    assert pre_commit.get_badge_color(75.0) == "yellow"
    assert pre_commit.get_badge_color(70.0) == "yellow"
    assert pre_commit.get_badge_color(65.0) == "orange"
    assert pre_commit.get_badge_color(60.0) == "orange"
    assert pre_commit.get_badge_color(50.0) == "red"


def test_format_pct() -> None:
    """Verifies formatting percentages with decimals or whole numbers."""
    assert pre_commit.format_pct(100.0) == "100%"
    assert pre_commit.format_pct(99.98) == "100%"
    assert pre_commit.format_pct(95.0) == "95%"
    assert pre_commit.format_pct(88.5) == "88.5%"


def test_calculate_doc_coverage_force(tmp_path: Path) -> None:
    """Verifies calculate_doc_coverage unlinks old reports and aggregates doc items."""
    doc_dir = tmp_path / "target" / "doc"
    doc_dir.mkdir(parents=True)
    old_json = doc_dir / "old.json"
    old_json.touch()

    def mock_run_cmd(cmd: list, cwd: Path, env: dict | None = None) -> None:
        report = {
            "crate_a": {"total": 50, "with_docs": 45},
            "crate_b": {"total": 50, "with_docs": 45},
            "ignored": "not a dict",
        }
        with open(doc_dir / "report.json", "w", encoding="utf-8") as f:
            json.dump(report, f)

    with (
        patch("pre_commit.run_command", side_effect=mock_run_cmd),
        patch("pathlib.Path.unlink") as mock_unlink,
    ):
        mock_unlink.side_effect = OSError("unlink error")
        pct = pre_commit.calculate_doc_coverage(tmp_path, force_run=True)
        assert pct == 90.0


def test_calculate_doc_coverage_zero_items(tmp_path: Path) -> None:
    """Verifies calculate_doc_coverage returns 100.0 when no items exist."""
    doc_dir = tmp_path / "target" / "doc"
    doc_dir.mkdir(parents=True)

    with open(doc_dir / "empty.json", "w", encoding="utf-8") as f:
        json.dump({}, f)

    pct = pre_commit.calculate_doc_coverage(tmp_path, force_run=False)
    assert pct == 100.0


def test_calculate_doc_coverage_corrupted_json(tmp_path: Path) -> None:
    """Verifies calculate_doc_coverage catches JSON parsing errors gracefully."""
    doc_dir = tmp_path / "target" / "doc"
    doc_dir.mkdir(parents=True)

    with open(doc_dir / "bad.json", "w", encoding="utf-8") as f:
        f.write("invalid json content")

    pct = pre_commit.calculate_doc_coverage(tmp_path, force_run=False)
    assert pct == 100.0


def test_calculate_test_coverage_force(tmp_path: Path) -> None:
    """Verifies calculate_test_coverage unlinks old report and parses llvm-cov json."""
    cov_path = tmp_path / "target" / "llvm-cov" / "coverage.json"
    cov_path.parent.mkdir(parents=True)
    cov_path.touch()

    def mock_run_cmd(cmd: list, cwd: Path, env: dict | None = None) -> None:
        data = {
            "data": [
                {
                    "totals": {
                        "lines": {
                            "count": 200,
                            "covered": 190,
                            "percent": 95.0,
                        }
                    }
                }
            ]
        }
        with open(cov_path, "w", encoding="utf-8") as f:
            json.dump(data, f)

    with (
        patch("pre_commit.run_command", side_effect=mock_run_cmd),
        patch("pathlib.Path.unlink") as mock_unlink,
    ):
        mock_unlink.side_effect = OSError("cannot delete")
        pct = pre_commit.calculate_test_coverage(tmp_path, force_run=True)
        assert pct == 95.0


def test_calculate_test_coverage_existing(tmp_path: Path) -> None:
    """Verifies calculate_test_coverage reuses existing report when force_run is False."""
    cov_path = tmp_path / "target" / "llvm-cov" / "coverage.json"
    cov_path.parent.mkdir(parents=True)
    data = {
        "data": [
            {
                "totals": {
                    "lines": {
                        "count": 100,
                        "covered": 100,
                        "percent": 100.0,
                    }
                }
            }
        ]
    }
    with open(cov_path, "w", encoding="utf-8") as f:
        json.dump(data, f)

    with patch("pre_commit.run_command") as mock_run:
        pct = pre_commit.calculate_test_coverage(tmp_path, force_run=False)
        assert pct == 100.0
        mock_run.assert_not_called()


def test_update_readme_shields_no_readme(tmp_path: Path) -> None:
    """Verifies update_readme_shields returns False if README.md does not exist."""
    assert not pre_commit.update_readme_shields(tmp_path, 100.0, 100.0)


def test_update_readme_shields_insert_both(tmp_path: Path) -> None:
    """Verifies update_readme_shields inserts both shields after title if none exist."""
    readme = tmp_path / "README.md"
    readme.write_text("# Project Title\n\nSome description.\n", encoding="utf-8")

    assert pre_commit.update_readme_shields(tmp_path, 95.0, 90.0)
    updated = readme.read_text(encoding="utf-8")
    assert "doc%20coverage-95%25-brightgreen" in updated
    assert "test%20coverage-90%25-green" in updated

    # Calling again without changes returns False (already up to date)
    assert not pre_commit.update_readme_shields(tmp_path, 95.0, 90.0)


def test_update_readme_shields_has_doc_only(tmp_path: Path) -> None:
    """Verifies update_readme_shields handles README with only doc shield."""
    readme = tmp_path / "README.md"
    readme.write_text(
        "# Title\n\n[![% doc coverage](https://img.shields.io/old)](#)\n",
        encoding="utf-8",
    )

    assert pre_commit.update_readme_shields(tmp_path, 100.0, 100.0)
    updated = readme.read_text(encoding="utf-8")
    assert "doc%20coverage-100%25-brightgreen" in updated
    assert "test%20coverage-100%25-brightgreen" in updated


def test_update_readme_shields_has_test_only(tmp_path: Path) -> None:
    """Verifies update_readme_shields handles README with only test shield."""
    readme = tmp_path / "README.md"
    readme.write_text(
        "# Title\n\n[![% test coverage](https://img.shields.io/old)](#)\n",
        encoding="utf-8",
    )

    assert pre_commit.update_readme_shields(tmp_path, 100.0, 100.0)
    updated = readme.read_text(encoding="utf-8")
    assert "doc%20coverage-100%25-brightgreen" in updated
    assert "test%20coverage-100%25-brightgreen" in updated


def test_hook_shields(tmp_path: Path) -> None:
    """Verifies hook_shields runs both coverage calculations and updates shields."""
    with (
        patch("pre_commit.calculate_doc_coverage", return_value=98.0) as mock_doc,
        patch("pre_commit.calculate_test_coverage", return_value=97.0) as mock_test,
        patch("pre_commit.update_readme_shields") as mock_update,
    ):
        assert pre_commit.hook_shields(tmp_path, force_run=True) == 0
        mock_doc.assert_called_once_with(tmp_path, force_run=True)
        mock_test.assert_called_once_with(tmp_path, force_run=True)
        mock_update.assert_called_once_with(tmp_path, 98.0, 97.0)


def test_hook_all(tmp_path: Path) -> None:
    """Verifies hook_all runs fmt, build, clippy, test, and shields in sequence."""
    with (
        patch("pre_commit.hook_fmt") as mock_fmt,
        patch("pre_commit.hook_build") as mock_build,
        patch("pre_commit.hook_clippy") as mock_clippy,
        patch("pre_commit.hook_test") as mock_test,
        patch("pre_commit.hook_shields") as mock_shields,
    ):
        assert pre_commit.hook_all(tmp_path) == 0
        mock_fmt.assert_called_once_with(tmp_path)
        mock_build.assert_called_once_with(tmp_path)
        mock_clippy.assert_called_once_with(tmp_path)
        mock_test.assert_called_once_with(tmp_path)
        mock_shields.assert_called_once_with(tmp_path, force_run=True)


def test_main_stages() -> None:
    """Verifies main executes selected stage handler or handles failure."""
    for stage in ["fmt", "build", "clippy", "test", "shields", "all"]:
        with (
            patch.object(sys, "argv", ["pre_commit.py", stage]),
            patch(f"pre_commit.hook_{stage}", return_value=0) as mock_stage,
        ):
            assert pre_commit.main() == 0
            mock_stage.assert_called_once()

    # Subprocess failure error path (list command)
    err_list = subprocess.CalledProcessError(1, ["cargo", "test"])
    with (
        patch.object(sys, "argv", ["pre_commit.py", "test"]),
        patch("pre_commit.hook_test", side_effect=err_list),
    ):
        assert pre_commit.main() == 1

    # Subprocess failure error path (string command)
    err_str = subprocess.CalledProcessError(2, "cargo test")
    with (
        patch.object(sys, "argv", ["pre_commit.py", "test"]),
        patch("pre_commit.hook_test", side_effect=err_str),
    ):
        assert pre_commit.main() == 2


def test_pre_commit_run_as_main() -> None:
    """Verifies execution when pre_commit.py is invoked as __main__."""
    script_path = scripts_dir / "pre_commit.py"
    with (
        patch("pre_commit.main", return_value=0),
        patch.object(sys, "argv", ["pre_commit.py", "shields"]),
        patch.object(sys, "exit") as mock_exit,
    ):
        runpy.run_path(str(script_path), run_name="__main__")
        mock_exit.assert_called_once_with(0)
