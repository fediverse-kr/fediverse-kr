#!/usr/bin/env python3
"""Run contribution gates locally; GitHub CI/Actions is not used."""

import argparse
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parents[1]
CARGO_DENY_VERSION = "cargo-deny 0.20.2"
GRAPHS = (
    ("wasm32-unknown-unknown", "web"),
    ("aarch64-unknown-linux-gnu", "server"),
    ("x86_64-unknown-linux-gnu", "server"),
)
TEST_RESULT = re.compile(
    r"^test result: .*?\b(\d+) passed; (\d+) failed; (\d+) ignored;",
    re.MULTILINE,
)


class CheckFailed(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


def run(command, *, require_tests=False, environment=None):
    print("$ " + shlex.join(command), flush=True)
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode:
        raise CheckFailed(f"command failed with exit {result.returncode}: {shlex.join(command)}", result.returncode)
    if require_tests:
        summaries = TEST_RESULT.findall(result.stdout)
        executed = sum(int(passed) + int(failed) for passed, failed, _ignored in summaries)
        if executed == 0:
            raise CheckFailed("Cargo reported success but executed zero tests")
        print(f"Verified {executed} executed Rust tests.")
    return result


def cargo_graph(subcommand, target, feature):
    return [
        "cargo", subcommand, "--locked", "--target", target,
        "--no-default-features", "--features", feature,
    ]


def test_target():
    target = os.environ.get("FEDKR_TEST_TARGET", "x86_64-unknown-linux-gnu")
    if target not in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
        raise CheckFailed("FEDKR_TEST_TARGET must be a supported native Linux server target")
    return target


def quick():
    version = run(["cargo", "deny", "--version"])
    if version.stdout.strip() != CARGO_DENY_VERSION:
        raise CheckFailed(
            f"expected {CARGO_DENY_VERSION}, got {version.stdout.strip() or 'no version output'}"
        )
    # Build and execute tests for the local native architecture; no cross C/sysroot.
    # Policy resolution below still checks all three graphs on every machine.
    native = test_target()
    for target, feature in (GRAPHS[0], (native, "server")):
        run(cargo_graph("check", target, feature))
    run(cargo_graph("test", native, "server") + ["--bin", "fediversekr2"],
        require_tests=True)
    run(["cargo", "deny", "fetch", "db"])
    failures = []
    for target, feature in GRAPHS:
        try:
            run([
                "cargo", "deny", "--locked", "--offline", "--target", target,
                "--no-default-features", "--features", feature, "check", "all",
            ])
        except CheckFailed as error:
            failures.append(error)
    if failures:
        raise CheckFailed(f"{len(failures)} dependency policy graphs failed", failures[0].code)


def disposable_database_url():
    raw = os.environ.get("FEDKR_TEST_DATABASE_URL", "")
    try:
        parsed = urlsplit(raw)
        port = parsed.port
    except ValueError as error:
        raise CheckFailed("FEDKR_TEST_DATABASE_URL must identify the disposable loopback fedkr_test database") from error
    if (
        parsed.scheme not in ("postgres", "postgresql")
        or parsed.hostname != "127.0.0.1"
        or parsed.path != "/fedkr_test"
        or parsed.query
        or parsed.fragment
        or not parsed.username
        or port != 16439
    ):
        raise CheckFailed("FEDKR_TEST_DATABASE_URL must identify the disposable loopback fedkr_test database")
    return raw


def database():
    environment = os.environ.copy()
    environment["FEDKR_TEST_DATABASE_URL"] = disposable_database_url()
    run(
        cargo_graph("test", test_target(), "server")
        + ["--bin", "fediversekr2", "--", "--include-ignored", "--test-threads=1"],
        require_tests=True,
        environment=environment,
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("quick", "database"))
    args = parser.parse_args()
    try:
        quick() if args.mode == "quick" else database()
    except CheckFailed as error:
        print(f"contribution check failed: {error}", file=sys.stderr)
        return error.code
    print(f"Contribution {args.mode} checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
