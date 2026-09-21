#!/usr/bin/env python3
"""Cross-platform installer for msi-rs pre-commit hooks using PyPI pre-commit."""

import os
import shutil
import subprocess
import sys
from pathlib import Path


def get_augmented_env() -> dict:
    """Returns os.environ augmented with standard python and cargo paths."""
    env = dict(os.environ)
    paths = env.get("PATH", "").split(os.pathsep)

    candidate_dirs = [
        Path.home() / ".cargo" / "bin",
        Path.home() / ".local" / "bin",
        Path("/opt/homebrew/bin"),
        Path("/usr/local/bin"),
        Path(sys.prefix) / "bin",
        Path(sys.prefix) / "Scripts",
    ]
    for d in candidate_dirs:
        if d.exists() and str(d) not in paths:
            paths.insert(0, str(d))

    env["PATH"] = os.pathsep.join(paths)
    return env


def find_pre_commit_cmd(env: dict) -> list:
    """Locates the PyPI pre-commit executable or module runner."""
    # 1. Check PATH for pre-commit binary
    pre_commit_path = shutil.which("pre-commit", path=env.get("PATH"))
    if pre_commit_path:
        return [pre_commit_path]

    # 2. Check current python interpreter for pre_commit module
    try:
        res = subprocess.run(
            [sys.executable, "-m", "pre_commit", "--version"],
            capture_output=True,
            text=True,
            env=env,
        )
        if res.returncode == 0:
            return [sys.executable, "-m", "pre_commit"]
    except Exception:
        pass

    # 3. Check python3 / python binaries
    for py_bin in ["python3", "python"]:
        py_path = shutil.which(py_bin, path=env.get("PATH"))
        if py_path:
            try:
                res = subprocess.run(
                    [py_path, "-m", "pre_commit", "--version"],
                    capture_output=True,
                    text=True,
                    env=env,
                )
                if res.returncode == 0:
                    return [py_path, "-m", "pre_commit"]
            except Exception:
                pass

    return []


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    env = get_augmented_env()
    print("==> Installing pre-commit hooks for msi-rs via PyPI pre-commit...")

    git_bin = shutil.which("git", path=env.get("PATH"))
    if not git_bin:
        print("Error: git executable not found in PATH.", file=sys.stderr)
        return 1

    # 1. Unset core.hooksPath if set, preventing "Cowardly refusing to install hooks" error
    try:
        subprocess.run(
            [git_bin, "config", "--unset-all", "core.hooksPath"],
            cwd=repo_root,
            capture_output=True,
            check=False,
        )
        print("Ensured git core.hooksPath is unset.")
    except Exception as err:
        print(f"Notice: Could not check/unset core.hooksPath: {err}", file=sys.stderr)

    # 2. Remove any legacy hook files
    legacy_hook = repo_root / ".git" / "hooks" / "pre-commit.legacy"
    if legacy_hook.exists():
        try:
            legacy_hook.unlink()
        except OSError:
            pass

    # 3. Locate PyPI pre-commit
    cmd = find_pre_commit_cmd(env)
    if not cmd:
        print("pre-commit not found. Attempting installation via pip...")
        pip_install = subprocess.run(
            [sys.executable, "-m", "pip", "install", "pre-commit"],
            cwd=repo_root,
            env=env,
        )
        if pip_install.returncode == 0:
            cmd = find_pre_commit_cmd(env)

    if not cmd:
        print(
            "Error: PyPI pre-commit could not be found or installed.\n"
            "Please install it manually via: pip install pre-commit",
            file=sys.stderr,
        )
        return 1

    # 4. Install pre-commit hook into .git/hooks/pre-commit
    print(f"Using pre-commit: {' '.join(cmd)}")
    install_res = subprocess.run(
        cmd + ["install", "-f"],
        cwd=repo_root,
        env=env,
    )
    if install_res.returncode != 0:
        print("Error: 'pre-commit install -f' failed.", file=sys.stderr)
        return install_res.returncode

    print("Pre-commit hook successfully installed via PyPI pre-commit framework.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
