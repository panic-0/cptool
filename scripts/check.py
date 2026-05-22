#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parent.parent


def run_step(name: str, command: list[str]) -> int:
    print(f"\n==> {name}", flush=True)
    print("+ " + " ".join(command), flush=True)
    completed = subprocess.run(command, cwd=REPO_ROOT)
    if completed.returncode != 0:
        print(f"\n{name} failed with exit code {completed.returncode}", file=sys.stderr)
    return completed.returncode


def main() -> int:
    steps = [
        ("Check formatting", ["cargo", "fmt", "--check"]),
        (
            "Run clippy",
            [
                "cargo",
                "clippy",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("Run tests", ["cargo", "test", "--all-targets", "--all-features"]),
    ]

    for name, command in steps:
        exit_code = run_step(name, command)
        if exit_code != 0:
            return exit_code

    print("\nAll checks passed.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
