#!/usr/bin/env python3
"""Synthetic contract tests for generate-third-party-notices.py.

They never invoke Cargo or cargo-about.  The fake runner supplies the small
machine-local JSON that production intentionally keeps in a temporary folder.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("generate-third-party-notices.py")
SPEC = importlib.util.spec_from_file_location("third_party_notices", SCRIPT)
notices = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(notices)


class FakeCargoAbout:
    def __init__(self, raw):
        self.raw = raw
        self.calls = []

    def __call__(self, command, **_kwargs):
        self.calls.append(command)
        if command[-1] == "--version":
            return subprocess.CompletedProcess(command, 0, "cargo-about 0.9.2\n", "")
        output = Path(command[command.index("-o") + 1])
        output.write_text(json.dumps(self.raw), encoding="utf-8")
        return subprocess.CompletedProcess(command, 0, "", "")


def raw_notice(source_path="C:\\private\\registry\\LICENSE", text="<unsafe & generic>"):
    package = {
        "name": "fixture-crate",
        "version": "1.2.3",
        "id": "registry+https://example.invalid#index#fixture-crate@1.2.3",
        "source": "registry+https://example.invalid/index",
        "repository": "https://example.invalid/fixture",
        "license": "MIT OR Apache-2.0",
    }
    return {
        "crates": [{"package": package, "license": "MIT OR Apache-2.0"}],
        "licenses": [{"name": "MIT License", "id": "MIT", "text": text,
                      "source_path": source_path, "used_by": [{"crate": package}]}],
    }


class NoticeGeneratorTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        (self.root / "Cargo.toml").write_text("[package]\nname='fixture'\n", encoding="utf-8")
        (self.root / "Cargo.lock").write_text("version = 4\npackage = []\n", encoding="utf-8")
        (self.root / "Dioxus.toml").write_text("[application]\nname='fixture'\n", encoding="utf-8")
        (self.root / "about.toml").write_text(
            "accepted=['MIT']\nprivate={ignore=true}\nignore-dev-dependencies=true\n"
            "ignore-build-dependencies=false\nignore-transitive-dependencies=false\n"
            "workarounds=['ring','chrono','rustls']\n",
            encoding="utf-8")
        for relative, text in {
            "docs/icon-pack-notices.md": "<icon & notice>",
            "docs/licenses/assets-manifest.md": "<asset & manifest>",
            "docs/licenses/assets/fixture.txt": "<asset source>",
            "assets/fonts/OFL.txt": "<font source>",
        }.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        self.write_supplements([])
        self.tool = self.root / "fake-cargo-about"
        self.tool.write_bytes(b"synthetic tool")

    def tearDown(self):
        self.temp.cleanup()

    def write_supplements(self, entries):
        path = self.root / "docs/licenses/supplemental.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"version": 1, "entries": entries}), encoding="utf-8")

    def supplement(self, relative="docs/licenses/upstream/fixture.txt", text="<upstream & text>",
                   license_id="MIT", digest=None):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode("utf-8"))
        return {"name": "fixture-crate", "version": "1.2.3", "license": license_id,
                "files": [{"path": relative,
                           "sha256": digest or hashlib.sha256(text.encode()).hexdigest(),
                           "source": "https://example.invalid/repo/blob/0123456789abcdef0123456789abcdef01234567/LICENSE"}]}

    def declaration_supplement(self, *, name="fixture-crate", version="1.2.3",
                               license="MIT OR Apache-2.0", source=None):
        relative = "docs/licenses/upstream/fixture-crate-1.2.3-Cargo.toml.txt"
        text = "[package]\nname = %r\nversion = %r\nlicense = %r\n" % (name, version, license)
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode("utf-8"))
        return {"name": "fixture-crate", "version": "1.2.3", "license": "MIT",
                "evidence": "license-declaration",
                "files": [{"path": relative, "sha256": hashlib.sha256(text.encode()).hexdigest(),
                           "source": source or "https://docs.rs/crate/fixture-crate/1.2.3/source/Cargo.toml"}]}

    def declaration_lock(self):
        (self.root / "Cargo.lock").write_text(
            "version = 4\n[[package]]\nname = 'fixture-crate'\nversion = '1.2.3'\n"
            "checksum = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'\n",
            encoding="utf-8")

    def generate(self, raw, name="out"):
        runner = FakeCargoAbout(raw)
        output = self.root / name
        notices.generate(self.root, self.tool, "linux-server", output, runner=runner)
        return output, runner

    def test_selected_target_flags_and_sanitized_escaped_output(self):
        output, runner = self.generate(raw_notice())
        generated = [call for call in runner.calls if "generate" in call]
        self.assertEqual(len(generated), 2)
        self.assertIn("--locked", generated[0])
        self.assertIn("--offline", generated[0])
        self.assertIn("--no-default-features", generated[0])
        self.assertEqual(generated[0][generated[0].index("--features") + 1], "server")
        self.assertEqual(generated[0][generated[0].index("--target") + 1], "x86_64-unknown-linux-gnu")
        self.assertEqual(generated[1][generated[1].index("--features") + 1], "web")
        self.assertEqual(generated[1][generated[1].index("--target") + 1], "wasm32-unknown-unknown")
        document_bytes = (output / "THIRD-PARTY-NOTICES.html").read_bytes()
        document = document_bytes.decode("utf-8")
        inventory = json.loads((output / "third-party-inventory.json").read_text(encoding="utf-8"))
        self.assertIn("&lt;unsafe &amp; generic&gt;", document)
        self.assertIn("&lt;icon &amp; notice&gt;", document)
        self.assertNotIn("C:\\private\\registry", document)
        self.assertNotIn("C:\\private\\registry", json.dumps(inventory))
        self.assertEqual(inventory["html_sha256"], hashlib.sha256(document_bytes).hexdigest())
        self.assertEqual(inventory["profiles"][0]["packages"][0]["declared_license"], "MIT OR Apache-2.0")
        self.assertEqual(inventory["profiles"][0]["packages"][0]["selected_licenses"], ["MIT"])

    def test_linux_aarch64_server_profile_uses_the_arm64_target(self):
        runner = FakeCargoAbout(raw_notice())
        output = self.root / "arm64"
        notices.generate(self.root, self.tool, "linux-aarch64-server", output, runner=runner)
        generated = [call for call in runner.calls if "generate" in call]
        self.assertEqual(len(generated), 2)
        self.assertEqual(generated[0][generated[0].index("--features") + 1], "server")
        self.assertEqual(generated[0][generated[0].index("--target") + 1],
                         "aarch64-unknown-linux-gnu")
        self.assertEqual(generated[1][generated[1].index("--target") + 1],
                         "wasm32-unknown-unknown")
        inventory = json.loads((output / "third-party-inventory.json").read_text(encoding="utf-8"))
        self.assertEqual(inventory["profiles"][0]["profile"], "linux-aarch64-server")
        self.assertEqual(inventory["profiles"][0]["target"], "aarch64-unknown-linux-gnu")

    def test_missing_source_path_requires_exact_supplement(self):
        with self.assertRaises(notices.NoticeError):
            self.generate(raw_notice(source_path=None), "missing")
        self.assertFalse((self.root / "missing").exists())

    def test_matching_supplement_is_additive_even_with_cargo_source(self):
        self.write_supplements([self.supplement()])
        output, _ = self.generate(raw_notice(), "additive")
        document = (output / "THIRD-PARTY-NOTICES.html").read_text(encoding="utf-8")
        self.assertIn("&lt;upstream &amp; text&gt;", document)
        self.assertIn("&lt;unsafe &amp; generic&gt;", document)

    def test_declaration_only_evidence_requires_matching_public_metadata(self):
        self.declaration_lock()
        self.write_supplements([self.declaration_supplement()])
        output, _ = self.generate(raw_notice(source_path=None), "declaration")
        document = (output / "THIRD-PARTY-NOTICES.html").read_text(encoding="utf-8")
        inventory = json.loads((output / "third-party-inventory.json").read_text(encoding="utf-8"))
        self.assertIn("License declaration only; no separate upstream license text located.", document)
        declaration = inventory["profiles"][0]["declaration_only_packages"][0]
        self.assertEqual(declaration["metadata_license"], "MIT OR Apache-2.0")
        self.assertEqual(declaration["declaration_license"], "MIT OR Apache-2.0")
        self.assertEqual(declaration["package_checksum"], "a" * 64)

    def test_only_the_audited_mac_legacy_spelling_is_accepted(self):
        tuple = notices.LEGACY_DECLARATION_TUPLE
        self.assertTrue(notices.declaration_matches_metadata(*tuple))
        self.assertFalse(notices.declaration_matches_metadata(
            "mac", "0.1.1", tuple[2], "MIT/Apache-2.0", "MIT OR Apache-2.0 AND ISC"))
        self.assertFalse(notices.declaration_matches_metadata(
            "not-mac", "0.1.1", tuple[2], tuple[3], tuple[4]))
        self.assertFalse(notices.declaration_matches_metadata(
            "mac", "0.1.2", tuple[2], tuple[3], tuple[4]))
        self.assertFalse(notices.declaration_matches_metadata(
            "mac", "0.1.1", "0" * 64, tuple[3], tuple[4]))
        self.assertFalse(notices.declaration_matches_metadata(
            "mac", "0.1.1", tuple[2], "MIT/Apache-2.0 OR ISC", tuple[4]))

    def test_declaration_forgery_version_license_and_docs_url_are_rejected(self):
        self.declaration_lock()
        for label, arguments in (
            ("forged", {"name": "other"}),
            ("wrong-version", {"version": "9.9.9"}),
            ("wrong-license", {"license": "MIT"}),
            ("wrong-docs-url", {"source": "https://docs.rs/crate/fixture-crate/9.9.9/source/Cargo.toml"}),
        ):
            entry = self.declaration_supplement(**arguments)
            self.write_supplements([entry])
            with self.assertRaises(notices.NoticeError):
                self.generate(raw_notice(source_path=None), label)
            self.assertFalse((self.root / label).exists())

    def test_tampered_or_unsafe_supplement_never_creates_output(self):
        self.write_supplements([self.supplement(digest="0" * 64)])
        with self.assertRaises(notices.NoticeError):
            self.generate(raw_notice(source_path=None), "tampered")
        self.assertFalse((self.root / "tampered").exists())
        unsafe = self.supplement(relative="docs/licenses/upstream/fixture.txt")
        unsafe["files"][0]["path"] = "../outside.txt"
        self.write_supplements([unsafe])
        with self.assertRaises(notices.NoticeError):
            self.generate(raw_notice(source_path=None), "unsafe")
        self.assertFalse((self.root / "unsafe").exists())

    def test_existing_output_is_not_overwritten(self):
        output = self.root / "existing"
        output.mkdir()
        sentinel = output / "keep.txt"
        sentinel.write_text("do not replace", encoding="utf-8")
        with self.assertRaises(notices.NoticeError):
            notices.generate(self.root, self.tool, "linux-server", output,
                             runner=FakeCargoAbout(raw_notice()))
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "do not replace")

    def test_empty_or_unused_package_records_fail_closed(self):
        empty = raw_notice()
        empty["crates"] = []
        with self.assertRaises(notices.NoticeError):
            self.generate(empty, "empty")
        unused = raw_notice()
        unused["licenses"] = []
        with self.assertRaises(notices.NoticeError):
            self.generate(unused, "unused")

    def test_input_change_during_generation_leaves_no_output(self):
        class MutatingCargoAbout(FakeCargoAbout):
            def __call__(runner, command, **kwargs):
                result = super(MutatingCargoAbout, runner).__call__(command, **kwargs)
                if "generate" in command and command[command.index("--features") + 1] == "web":
                    (self.root / "Cargo.lock").write_text("# changed\n", encoding="utf-8")
                return result

        with self.assertRaises(notices.NoticeError):
            notices.generate(self.root, self.tool, "linux-server", self.root / "changed",
                             runner=MutatingCargoAbout(raw_notice()))
        self.assertFalse((self.root / "changed").exists())


if __name__ == "__main__":
    unittest.main()
