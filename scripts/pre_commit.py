#!/usr/bin/env python3
"""Cross-platform Pre-commit Hook Pipeline for msi-rs.

Performs:
1. Code formatting auto-fix and check (cargo fmt)
2. Compilation and build check (cargo build)
3. Strong pedantic linting (cargo clippy)
4. Unit and integration test suite execution (cargo test)
5. Doc & test coverage calculation and shield updates in README.md
"""

import argparse
import glob
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


def get_repo_root() -> Path:
    """Returns the workspace root directory."""
    return Path(__file__).resolve().parent.parent


def get_augmented_env() -> dict:
    """Returns os.environ augmented with standard cargo and toolchain paths."""
    env = dict(os.environ)
    cargo_bin = Path.home() / ".cargo" / "bin"
    current_path = env.get("PATH", "")
    paths = current_path.split(os.pathsep)

    candidate_dirs = [
        cargo_bin,
        Path("/opt/homebrew/bin"),
        Path("/usr/local/bin"),
    ]
    for d in candidate_dirs:
        if d.exists() and str(d) not in paths:
            paths.insert(0, str(d))

    env["PATH"] = os.pathsep.join(paths)
    return env


def run_command(
    cmd: list, cwd: Path, env: dict = None, capture_output: bool = False
) -> subprocess.CompletedProcess:
    """Runs a shell command cross-platform and handles errors."""
    if env is None:
        env = get_augmented_env()

    # On Windows, look for command.exe if command not found
    executable = cmd[0]
    resolved = shutil.which(executable, path=env.get("PATH"))
    if resolved:
        cmd = [resolved] + cmd[1:]

    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=capture_output,
        text=True,
    )


def re_stage_git_files(repo_root: Path, file_paths: list = None):
    """Re-stages files in git index if git repository is present."""
    git_bin = shutil.which("git", path=get_augmented_env().get("PATH"))
    if not git_bin:
        return

    try:
        is_git = subprocess.run(
            [git_bin, "rev-parse", "--is-inside-work-tree"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if is_git.returncode != 0:
            return

        if file_paths:
            for fp in file_paths:
                subprocess.run([git_bin, "add", str(fp)], cwd=repo_root, check=False)
        else:
            # Re-stage previously staged files that were auto-formatted
            staged = subprocess.run(
                [git_bin, "diff", "--cached", "--name-only", "--diff-filter=ACM"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            if staged.returncode == 0:
                for filename in staged.stdout.splitlines():
                    full_path = repo_root / filename
                    if full_path.is_file():
                        subprocess.run(
                            [git_bin, "add", filename], cwd=repo_root, check=False
                        )
    except Exception:
        pass


def hook_fmt(repo_root: Path) -> int:
    """First runs auto-formatting fix, then verifies formatting check."""
    print("==> [fmt] Auto-fixing code formatting with cargo fmt...")
    run_command(["cargo", "fmt", "--all"], cwd=repo_root)

    print("==> [fmt] Verifying code formatting (cargo fmt -- --check)...")
    run_command(["cargo", "fmt", "--all", "--", "--check"], cwd=repo_root)

    print("==> [fmt] Code formatting verified successfully.")
    return 0


def hook_build(repo_root: Path) -> int:
    """Builds all packages and targets in the workspace."""
    print("==> [build] Compiling workspace packages and targets (cargo build)...")
    run_command(["cargo", "build", "--workspace", "--all-targets"], cwd=repo_root)
    print("==> [build] Workspace compiled successfully.")
    return 0


def hook_clippy(repo_root: Path) -> int:
    """Runs clippy across all targets and features with pedantic checks."""
    print("==> [clippy] Running clippy linter with pedantic checks...")
    run_command(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
            "-D",
            "clippy::pedantic",
        ],
        cwd=repo_root,
    )
    print("==> [clippy] Clippy checks passed with zero warnings.")
    return 0


def hook_test(repo_root: Path) -> int:
    """Runs all workspace unit and integration tests."""
    print("==> [test] Running test suite (cargo test)...")
    run_command(["cargo", "test", "--workspace", "--all-targets"], cwd=repo_root)
    print("==> [test] All tests passed.")
    return 0


def get_badge_color(pct: float) -> str:
    """Maps percentage threshold to standard shields.io badge color."""
    if pct >= 95.0:
        return "brightgreen"
    if pct >= 90.0:
        return "green"
    if pct >= 80.0:
        return "yellowgreen"
    if pct >= 70.0:
        return "yellow"
    if pct >= 60.0:
        return "orange"
    return "red"


def format_pct(pct: float) -> str:
    """Formats percentage string cleanly (integer if whole or 100%, otherwise 1 decimal place)."""
    if round(pct, 1) >= 100.0 or pct.is_integer():
        return f"{int(round(pct))}%"
    return f"{pct:.1f}%"


def calculate_doc_coverage(repo_root: Path, force_run: bool = True) -> float:
    """Calculates aggregate documentation coverage across all workspace crates."""
    doc_dir = repo_root / "target" / "doc"

    if force_run and doc_dir.exists():
        for old_json in doc_dir.glob("*.json"):
            try:
                old_json.unlink()
            except OSError:
                pass

    doc_json_files = list(doc_dir.glob("*.json")) if doc_dir.exists() else []

    if force_run or not doc_json_files:
        print("==> [shields] Generating rustdoc documentation coverage...")
        env = get_augmented_env()
        env["RUSTDOCFLAGS"] = "-Z unstable-options --show-coverage --output-format json"
        run_command(
            ["cargo", "doc", "--workspace", "--no-deps"], cwd=repo_root, env=env
        )
        doc_json_files = list(doc_dir.glob("*.json")) if doc_dir.exists() else []

    total_items = 0
    with_docs = 0
    for json_file in doc_json_files:
        try:
            with open(json_file, "r", encoding="utf-8") as f:
                data = json.load(f)
                for _path, stats in data.items():
                    if (
                        isinstance(stats, dict)
                        and "total" in stats
                        and "with_docs" in stats
                    ):
                        total_items += stats["total"]
                        with_docs += stats["with_docs"]
        except Exception as err:
            print(f"Warning: Failed to parse {json_file}: {err}", file=sys.stderr)

    if total_items == 0:
        return 100.0

    pct = (with_docs / total_items) * 100.0
    print(f"==> [shields] Doc coverage: {with_docs}/{total_items} items ({pct:.2f}%)")
    return pct


def calculate_test_coverage(repo_root: Path, force_run: bool = True) -> float:
    """Calculates aggregate test line coverage via cargo llvm-cov."""
    cov_path = repo_root / "target" / "llvm-cov" / "coverage.json"

    if force_run and cov_path.exists():
        try:
            cov_path.unlink()
        except OSError:
            pass

    if force_run or not cov_path.exists():
        print("==> [shields] Generating test coverage report via cargo llvm-cov...")
        cov_path.parent.mkdir(parents=True, exist_ok=True)
        run_command(
            [
                "cargo",
                "llvm-cov",
                "--workspace",
                "--all-targets",
                "--summary-only",
                "--json",
                "--output-path",
                str(cov_path),
            ],
            cwd=repo_root,
        )

    with open(cov_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    lines_info = data["data"][0]["totals"]["lines"]
    total_lines = lines_info["count"]
    covered_lines = lines_info["covered"]
    pct = lines_info["percent"]
    print(
        f"==> [shields] Test coverage: {covered_lines}/{total_lines} lines ({pct:.2f}%)"
    )
    return pct


def update_readme_shields(repo_root: Path, doc_pct: float, test_pct: float) -> bool:
    """Updates '% doc coverage' and '% test coverage' shields in README.md."""
    readme_path = repo_root / "README.md"
    if not readme_path.exists():
        print(f"Error: README.md not found at {readme_path}", file=sys.stderr)
        return False

    with open(readme_path, "r", encoding="utf-8") as f:
        content = f.read()

    doc_pct_str = format_pct(doc_pct)
    test_pct_str = format_pct(test_pct)
    doc_color = get_badge_color(doc_pct)
    test_color = get_badge_color(test_pct)

    doc_encoded = doc_pct_str.replace("%", "%25")
    test_encoded = test_pct_str.replace("%", "%25")

    doc_badge_url = (
        f"https://img.shields.io/badge/doc%20coverage-{doc_encoded}-{doc_color}"
    )
    test_badge_url = (
        f"https://img.shields.io/badge/test%20coverage-{test_encoded}-{test_color}"
    )

    doc_shield = f"[![% doc coverage]({doc_badge_url})](#)"
    test_shield = f"[![% test coverage]({test_badge_url})](#)"

    doc_pattern = re.compile(
        r"\[?!\[[^\]]*(?:%\s*)?doc\s+coverage[^\]]*\]\([^)]+\)(?:\]\([^)]*\))?",
        re.IGNORECASE,
    )
    test_pattern = re.compile(
        r"\[?!\[[^\]]*(?:%\s*)?test\s+coverage[^\]]*\]\([^)]+\)(?:\]\([^)]*\))?",
        re.IGNORECASE,
    )

    has_doc = bool(doc_pattern.search(content))
    has_test = bool(test_pattern.search(content))

    new_content = content
    if has_doc:
        new_content = doc_pattern.sub(doc_shield, new_content, count=1)
    if has_test:
        new_content = test_pattern.sub(test_shield, new_content, count=1)

    if not has_doc and not has_test:
        lines = new_content.splitlines(keepends=True)
        updated_lines = []
        inserted = False
        for line in lines:
            updated_lines.append(line)
            if not inserted and line.startswith("# "):
                updated_lines.append(f"\n{doc_shield}\n{test_shield}\n")
                inserted = True
        new_content = "".join(updated_lines)
    elif has_doc and not has_test:
        new_content = doc_pattern.sub(
            f"{doc_shield}\n{test_shield}", new_content, count=1
        )
    elif has_test and not has_doc:
        new_content = test_pattern.sub(
            f"{doc_shield}\n{test_shield}", new_content, count=1
        )

    if new_content != content:
        with open(readme_path, "w", encoding="utf-8") as f:
            f.write(new_content)
        print(
            f"==> [shields] Updated README.md with shields:\n    {doc_shield}\n    {test_shield}"
        )
        return True

    print("==> [shields] README.md shields are already up-to-date.")
    return False


def hook_shields(repo_root: Path, force_run: bool = True) -> int:
    """Collects doc and test coverage and updates shields in README.md."""
    doc_pct = calculate_doc_coverage(repo_root, force_run=force_run)
    test_pct = calculate_test_coverage(repo_root, force_run=force_run)
    update_readme_shields(repo_root, doc_pct, test_pct)
    return 0


def hook_all(repo_root: Path) -> int:
    """Executes the complete pre-commit validation pipeline in sequence."""
    print("========================================================")
    print(" Running msi-rs Pre-commit Hook Pipeline (Python)")
    print("========================================================")
    hook_fmt(repo_root)
    hook_build(repo_root)
    hook_clippy(repo_root)
    hook_test(repo_root)
    hook_shields(repo_root, force_run=True)
    print("========================================================")
    print(" All pre-commit checks passed successfully!")
    print("========================================================")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="msi-rs Cross-Platform Pre-commit Hook Pipeline"
    )
    parser.add_argument(
        "stage",
        nargs="?",
        choices=["fmt", "build", "clippy", "test", "shields", "all"],
        default="all",
        help="Pipeline stage to execute (default: all)",
    )
    args = parser.parse_args()
    repo_root = get_repo_root()

    stage_handlers = {
        "fmt": hook_fmt,
        "build": hook_build,
        "clippy": hook_clippy,
        "test": hook_test,
        "shields": hook_shields,
        "all": hook_all,
    }

    try:
        return stage_handlers[args.stage](repo_root)
    except subprocess.CalledProcessError as err:
        cmd_str = " ".join(err.cmd) if isinstance(err.cmd, list) else err.cmd
        print(
            f"\nCommand failed with exit code {err.returncode}: {cmd_str}",
            file=sys.stderr,
        )
        return err.returncode


if __name__ == "__main__":
    sys.exit(main())
