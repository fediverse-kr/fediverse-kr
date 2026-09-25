#!/usr/bin/env python3
"""Behavior tests for the contributor check entrypoint."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


SCRIPT = Path(__file__).with_name("check-contribution.py")


class ContributionChecksTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "commands.jsonl"
        cargo = self.bin / "cargo"
        cargo.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import json
                import os
                from pathlib import Path
                import sys

                args = sys.argv[1:]
                with Path(os.environ["CHECK_LOG"]).open("a", encoding="utf-8") as stream:
                    stream.write(json.dumps(args) + "\\n")
                failure = os.environ.get("FAKE_FAIL_MATCH")
                if failure and failure in " ".join(args):
                    print("synthetic child failure", file=sys.stderr)
                    raise SystemExit(7)
                if args == ["deny", "--version"]:
                    print(os.environ.get("FAKE_DENY_VERSION", "cargo-deny 0.20.2"))
                elif "test" in args:
                    if os.environ.get("FAKE_ZERO_TESTS"):
                        print("test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out")
                    elif "--include-ignored" in args:
                        print("test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out")
                    else:
                        print("test result: ok. 42 passed; 0 failed; 133 ignored; 0 measured; 0 filtered out")
                """
            ),
            encoding="utf-8",
        )
        cargo.chmod(0o755)

    def tearDown(self):
        self.temp.cleanup()

    def run_check(self, mode, **updates):
        env = os.environ.copy()
        env.update(
            PATH=str(self.bin) + os.pathsep + env["PATH"],
            CHECK_LOG=str(self.log),
            PYTHONDONTWRITEBYTECODE="1",
            FEDKR_TEST_TARGET="x86_64-unknown-linux-gnu",
        )
        env.update(updates)
        return subprocess.run(
            [sys.executable, str(SCRIPT), mode],
            text=True,
            capture_output=True,
            env=env,
            timeout=30,
        )

    def commands(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()]

    def test_quick_checks_native_and_web_builds_and_every_policy_graph(self):
        result = self.run_check("quick")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        self.assertIn(["deny", "--version"], commands)
        self.assertNotIn(["fmt", "--check"], commands)
        for target, feature in (
            ("wasm32-unknown-unknown", "web"),
            ("aarch64-unknown-linux-gnu", "server"),
            ("x86_64-unknown-linux-gnu", "server"),
        ):
            build = ["check", "--locked", "--target", target,
                     "--no-default-features", "--features", feature]
            if target == "aarch64-unknown-linux-gnu":
                self.assertNotIn(build, commands)
            else:
                self.assertIn(build, commands)
            self.assertIn(
                ["deny", "--locked", "--offline", "--target", target,
                 "--no-default-features", "--features", feature, "check", "all"],
                commands,
            )
        self.assertIn(
            ["test", "--locked", "--target", "x86_64-unknown-linux-gnu",
             "--no-default-features", "--features", "server", "--bin", "fediversekr2"],
            commands,
        )
        self.assertIn(["deny", "fetch", "db"], commands)
        self.assertIn("42 passed", result.stdout)

    def test_zero_test_success_is_rejected(self):
        result = self.run_check("quick", FAKE_ZERO_TESTS="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("zero tests", result.stderr.lower())

    def test_policy_failure_still_checks_the_remaining_supported_graphs(self):
        result = self.run_check("quick", FAKE_FAIL_MATCH="--features server check all")
        self.assertNotEqual(result.returncode, 0)
        policies = [command for command in self.commands() if command[-2:] == ["check", "all"]]
        self.assertEqual(len(policies), 3)
        self.assertIn("2 dependency policy graphs failed", result.stderr)

    def test_arm64_contributors_can_run_tests_on_their_native_supported_target(self):
        result = self.run_check("quick", FEDKR_TEST_TARGET="aarch64-unknown-linux-gnu")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            ["test", "--locked", "--target", "aarch64-unknown-linux-gnu",
             "--no-default-features", "--features", "server", "--bin", "fediversekr2"],
            self.commands(),
        )
        self.assertIn(
            ["check", "--locked", "--target", "aarch64-unknown-linux-gnu",
             "--no-default-features", "--features", "server"],
            self.commands(),
        )
        self.assertNotIn(
            ["check", "--locked", "--target", "x86_64-unknown-linux-gnu",
             "--no-default-features", "--features", "server"],
            self.commands(),
        )

    def test_child_failure_stops_later_commands(self):
        result = self.run_check("quick", FAKE_FAIL_MATCH="wasm32-unknown-unknown")
        self.assertEqual(result.returncode, 7)
        self.assertIn("synthetic child failure", result.stderr)
        commands = self.commands()
        self.assertFalse(any(command[:2] == ["deny", "fetch"] for command in commands))
        self.assertFalse(any(command and command[0] == "test" for command in commands))

    def test_database_mode_requires_exact_disposable_loopback_database(self):
        for unsafe in (
            "postgresql://user@db.example/fedkr_test",
            "postgresql://user@127.0.0.1/production",
            "postgresql://user@127.0.0.1:5432/fedkr_test",
            "postgresql://user@127.0.0.1/fedkr_test?host=/var/run/postgresql",
            "postgresql://user@[::1]:16439/fedkr_test",
        ):
            result = self.run_check("database", FEDKR_TEST_DATABASE_URL=unsafe)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("disposable loopback", result.stderr.lower())
        self.log.unlink(missing_ok=True)
        safe = "postgresql://fedkr_test@127.0.0.1:16439/fedkr_test"
        result = self.run_check("database", FEDKR_TEST_DATABASE_URL=safe)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.commands(),
            [["test", "--locked", "--target", "x86_64-unknown-linux-gnu",
              "--no-default-features", "--features", "server", "--bin", "fediversekr2",
              "--", "--include-ignored", "--test-threads=1"]],
        )
        self.assertIn("133 passed", result.stdout)


if __name__ == "__main__":
    unittest.main()
