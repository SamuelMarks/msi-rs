"""Tests for scripts/install_hooks.py to achieve 100% test, line, function, and branch coverage."""

from __future__ import annotations

import importlib
import runpy
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

repo_root = Path(__file__).resolve().parent.parent.parent
scripts_dir = repo_root / "scripts"
if str(scripts_dir) not in sys.path:
    sys.path.insert(0, str(scripts_dir))


def test_get_augmented_env(tmp_path: Path) -> None:
    """Verifies that get_augmented_env adds candidate directories to PATH if existing."""
    install_hooks = importlib.import_module("install_hooks")

    fake_cargo = tmp_path / "cargo"
    fake_cargo.mkdir()
    fake_home = tmp_path / "home"
    fake_home.mkdir()

    with (
        patch("pathlib.Path.home", return_value=fake_home),
        patch.dict("os.environ", {"PATH": "/usr/bin"}),
    ):
        # Create a candidate dir
        (fake_home / ".cargo" / "bin").mkdir(parents=True)
        env = install_hooks.get_augmented_env()
        assert str(fake_home / ".cargo" / "bin") in env["PATH"]


def test_find_pre_commit_cmd_which() -> None:
    """Verifies find_pre_commit_cmd finds executable in PATH via which."""
    install_hooks = importlib.import_module("install_hooks")

    with patch("shutil.which", return_value="/usr/local/bin/pre-commit"):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/local/bin"})
        assert cmd == ["/usr/local/bin/pre-commit"]


def test_find_pre_commit_cmd_python_module() -> None:
    """Verifies find_pre_commit_cmd detects python -m pre_commit in current interpreter."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        return None

    mock_run = MagicMock()
    mock_run.returncode = 0

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", return_value=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == [sys.executable, "-m", "pre_commit"]


def test_find_pre_commit_cmd_python_module_nonzero() -> None:
    """Verifies find_pre_commit_cmd handles non-zero returncode from python -m pre_commit."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        return None

    mock_run = MagicMock()
    mock_run.returncode = 1

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", return_value=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == []


def test_find_pre_commit_cmd_python3_fallback() -> None:
    """Verifies find_pre_commit_cmd falls back to external python3/python binary."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd == "python3":
            return "/usr/bin/python3"
        return None

    def mock_run(args: list, **kwargs: object) -> MagicMock:
        if args[0] == sys.executable:
            raise RuntimeError("module not found")
        if args[0] == "/usr/bin/python3":
            res = MagicMock()
            res.returncode = 0
            return res
        raise RuntimeError("not found")

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", side_effect=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == ["/usr/bin/python3", "-m", "pre_commit"]


def test_find_pre_commit_cmd_python3_nonzero_then_python() -> None:
    """Verifies find_pre_commit_cmd checks python binary if python3 fails or returns nonzero."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd == "python":
            return "/usr/bin/python"
        return None

    def mock_run(args: list, **kwargs: object) -> MagicMock:
        if args[0] == sys.executable:
            res = MagicMock()
            res.returncode = 1
            return res
        if args[0] == "/usr/bin/python":
            res = MagicMock()
            res.returncode = 0
            return res
        raise RuntimeError("not found")

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", side_effect=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == ["/usr/bin/python", "-m", "pre_commit"]


def test_find_pre_commit_cmd_python_both_nonzero() -> None:
    """Verifies find_pre_commit_cmd continues loop when python binary returns nonzero."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd in ["python3", "python"]:
            return f"/usr/bin/{cmd}"
        return None

    mock_run = MagicMock()
    mock_run.returncode = 1

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", return_value=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == []


def test_find_pre_commit_cmd_not_found() -> None:
    """Verifies find_pre_commit_cmd returns empty list when pre-commit is unavailable."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd in ["python3", "python"]:
            return f"/usr/bin/{cmd}"
        return None

    def mock_run(args: list, **kwargs: object) -> MagicMock:
        raise OSError("failed to spawn")

    with (
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", side_effect=mock_run),
    ):
        cmd = install_hooks.find_pre_commit_cmd({"PATH": "/usr/bin"})
        assert cmd == []


def test_main_no_git() -> None:
    """Verifies main exits with error 1 if git binary is not found."""
    install_hooks = importlib.import_module("install_hooks")

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": ""}),
        patch("shutil.which", return_value=None),
    ):
        assert install_hooks.main() == 1


def test_main_success(tmp_path: Path) -> None:
    """Verifies main executes git config, deletes legacy hook, and runs install."""
    install_hooks = importlib.import_module("install_hooks")

    legacy_hook = tmp_path / ".git" / "hooks" / "pre-commit.legacy"
    legacy_hook.parent.mkdir(parents=True)
    legacy_hook.touch()

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd == "git":
            return "/usr/bin/git"
        if cmd == "pre-commit":
            return "/usr/bin/pre-commit"
        return None

    mock_run_res = MagicMock()
    mock_run_res.returncode = 0

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", return_value=mock_run_res),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
    ):
        assert install_hooks.main() == 0
        assert not legacy_hook.exists()


def test_main_git_config_exception(tmp_path: Path) -> None:
    """Verifies main handles git config unset failure notice gracefully."""
    install_hooks = importlib.import_module("install_hooks")

    def mock_which(cmd: str, path: str | None = None) -> str | None:
        if cmd == "git":
            return "/usr/bin/git"
        if cmd == "pre-commit":
            return "/usr/bin/pre-commit"
        return None

    def mock_run(cmd: list, **kwargs: object) -> MagicMock:
        if "config" in cmd:
            raise OSError("git config error")
        res = MagicMock()
        res.returncode = 0
        return res

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", side_effect=mock_which),
        patch("subprocess.run", side_effect=mock_run),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
    ):
        assert install_hooks.main() == 0


def test_main_pip_install_fallback(tmp_path: Path) -> None:
    """Verifies main falls back to pip install when pre-commit is not found initially."""
    install_hooks = importlib.import_module("install_hooks")

    first_call = [True]

    def mock_find_cmd(env: dict) -> list:
        if first_call[0]:
            first_call[0] = False
            return []
        return ["/usr/bin/pre-commit"]

    mock_run = MagicMock()
    mock_run.returncode = 0

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("install_hooks.find_pre_commit_cmd", side_effect=mock_find_cmd),
        patch("subprocess.run", return_value=mock_run),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
    ):
        assert install_hooks.main() == 0


def test_main_install_failure(tmp_path: Path) -> None:
    """Verifies main reports failure when pip install and finding cmd fails or pre-commit install fails."""
    install_hooks = importlib.import_module("install_hooks")

    # 1. Total failure to find pre-commit
    mock_run_pip_fail = MagicMock()
    mock_run_pip_fail.returncode = 1

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", return_value="/usr/bin/git"),
        patch("install_hooks.find_pre_commit_cmd", return_value=[]),
        patch("subprocess.run", return_value=mock_run_pip_fail),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
    ):
        assert install_hooks.main() == 1

    # 2. pre-commit install fails with error code
    mock_run_install_fail = MagicMock()
    mock_run_install_fail.returncode = 2

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", return_value="/usr/bin/git"),
        patch(
            "install_hooks.find_pre_commit_cmd", return_value=["/usr/bin/pre-commit"]
        ),
        patch("subprocess.run", return_value=mock_run_install_fail),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
    ):
        assert install_hooks.main() == 2


def test_main_legacy_hook_unlink_oserror(tmp_path: Path) -> None:
    """Verifies main handles legacy hook unlinking OSError gracefully."""
    install_hooks = importlib.import_module("install_hooks")

    legacy_hook = tmp_path / ".git" / "hooks" / "pre-commit.legacy"
    legacy_hook.parent.mkdir(parents=True)
    legacy_hook.touch()

    with (
        patch("install_hooks.get_augmented_env", return_value={"PATH": "/usr/bin"}),
        patch("shutil.which", return_value="/usr/bin/git"),
        patch(
            "install_hooks.find_pre_commit_cmd", return_value=["/usr/bin/pre-commit"]
        ),
        patch("subprocess.run", return_value=MagicMock(returncode=0)),
        patch.object(
            Path, "resolve", return_value=tmp_path / "scripts" / "install_hooks.py"
        ),
        patch.object(Path, "unlink", side_effect=OSError("permission denied")),
    ):
        assert install_hooks.main() == 0


def test_install_hooks_run_as_main(tmp_path: Path) -> None:
    """Verifies execution when install_hooks.py is invoked as __main__."""
    script_path = scripts_dir / "install_hooks.py"

    with (
        patch("install_hooks.main", return_value=0),
        patch.object(sys, "exit") as mock_exit,
    ):
        runpy.run_path(str(script_path), run_name="__main__")
        mock_exit.assert_called_once_with(0)
