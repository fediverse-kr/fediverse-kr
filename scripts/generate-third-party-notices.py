#!/usr/bin/env python3
"""Generate sanitized, per-target third-party notices with cargo-about.

The raw cargo-about JSON contains local registry paths.  It is read only from a
temporary directory; published output is escaped HTML plus a small inventory.
"""
import argparse
import hashlib
import html
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tempfile
import tomllib
from urllib.parse import urlparse


EXPECTED_CARGO_ABOUT = "0.9.2"
MAX_INPUT_BYTES = 4 * 1024 * 1024
MAX_SUPPLEMENT_BYTES = 1024 * 1024
MAX_ASSET_FILES = 128
MAX_ASSET_TREE_BYTES = 16 * 1024 * 1024
PROFILES = {
    "linux-server": (("server", "x86_64-unknown-linux-gnu", "server"),
                     ("web", "wasm32-unknown-unknown", "web")),
    "linux-aarch64-server": (("server", "aarch64-unknown-linux-gnu", "server"),
                             ("web", "wasm32-unknown-unknown", "web")),
    "windows-server": (("server", "x86_64-pc-windows-msvc", "server"),
                       ("web", "wasm32-unknown-unknown", "web")),
}
STATIC_NOTICE_FILES = (
    "docs/icon-pack-notices.md",
    "docs/licenses/assets-manifest.md",
    "assets/fonts/OFL.txt",
)
SUPPLEMENT = "docs/licenses/supplemental.json"
PINNED_COMMIT = re.compile(r"(?i)(?<![0-9a-f])[0-9a-f]{40,64}(?![0-9a-f])")
LEGACY_DECLARATION_TUPLE = (
    "mac", "0.1.1", "c41e0c4fef86961ac6d6f8a82609f55f31b05e4fce149ac5710e439df7619ba4",
    "MIT/Apache-2.0", "MIT OR Apache-2.0",
)


class NoticeError(RuntimeError):
    """An intentionally path- and content-free publication failure."""


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def checked_repo_file(root, relative, maximum=MAX_INPUT_BYTES):
    """Resolve a normal, bounded repository file without following it outward."""
    if not isinstance(relative, str) or not relative or "\\" in relative or ":" in relative:
        raise NoticeError("Invalid notice input.")
    posix = PurePosixPath(relative)
    if posix.is_absolute() or ".." in posix.parts or "." in posix.parts:
        raise NoticeError("Invalid notice input.")
    candidate = root
    for part in posix.parts:
        candidate = candidate / part
        if candidate.is_symlink():
            raise NoticeError("Invalid notice input.")
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise NoticeError("Required notice input is unavailable.") from error
    if not resolved.is_relative_to(root) or not resolved.is_file() or resolved.is_symlink():
        raise NoticeError("Invalid notice input.")
    try:
        if resolved.stat().st_size > maximum:
            raise NoticeError("Notice input exceeds its bounded size.")
    except OSError as error:
        raise NoticeError("Required notice input is unavailable.") from error
    return resolved


def text_file(root, relative, maximum=MAX_INPUT_BYTES):
    path = checked_repo_file(root, relative, maximum)
    try:
        return path.read_text(encoding="utf-8"), path
    except (OSError, UnicodeError) as error:
        raise NoticeError("Required notice text is unreadable.") from error


def validate_about(root):
    text, path = text_file(root, "about.toml")
    try:
        config = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise NoticeError("Invalid cargo-about policy.") from error
    private = config.get("private")
    if (not isinstance(config.get("accepted"), list) or not config["accepted"]
            or not isinstance(private, dict) or private.get("ignore") is not True
            or config.get("ignore-dev-dependencies") is not True
            or config.get("ignore-build-dependencies") is not False
            or config.get("ignore-transitive-dependencies") is not False
            or not isinstance(config.get("workarounds"), list)
            or not {"ring", "chrono", "rustls"}.issubset(config["workarounds"])):
        raise NoticeError("cargo-about policy does not preserve required settings.")
    return path


def validate_external_source(value):
    if value is None:
        return None
    if not isinstance(value, str) or not value or "\\" in value or value.startswith("file:"):
        raise NoticeError("Unsafe package provenance.")
    # Cargo registry and git source IDs are retained as provenance, without any
    # local manifest or source path from cargo-about's raw package object.
    if value.startswith(("registry+https://", "git+https://")):
        return value
    parsed = urlparse(value)
    if parsed.scheme == "https" and parsed.netloc and not parsed.username and not parsed.password:
        return value
    raise NoticeError("Unsafe package provenance.")


def validate_repository(value):
    if value is None:
        return None
    if not isinstance(value, str):
        raise NoticeError("Unsafe package provenance.")
    parsed = urlparse(value)
    if parsed.scheme not in ("http", "https") or not parsed.netloc or parsed.username or parsed.password:
        raise NoticeError("Unsafe package provenance.")
    return value


def validate_supplements(root):
    text, manifest = text_file(root, SUPPLEMENT)
    try:
        value = json.loads(text)
    except json.JSONDecodeError as error:
        raise NoticeError("Invalid supplemental license manifest.") from error
    if not isinstance(value, dict) or set(value) != {"version", "entries"} or value["version"] != 1:
        raise NoticeError("Invalid supplemental license manifest.")
    if not isinstance(value["entries"], list):
        raise NoticeError("Invalid supplemental license manifest.")
    entries = {}
    hashes = {SUPPLEMENT: sha256_file(manifest)}
    for entry in value["entries"]:
        if (not isinstance(entry, dict) or not {"name", "version", "license", "files"}.issubset(entry)
                or set(entry).difference({"name", "version", "license", "files", "evidence"})):
            raise NoticeError("Invalid supplemental license manifest.")
        name, version, license_id = entry["name"], entry["version"], entry["license"]
        evidence = entry.get("evidence", "license-text")
        if not all(isinstance(part, str) and part for part in (name, version, license_id)):
            raise NoticeError("Invalid supplemental license manifest.")
        if evidence not in ("license-text", "license-declaration"):
            raise NoticeError("Invalid supplemental license manifest.")
        key = (name, version, license_id)
        if (key in entries or not isinstance(entry["files"], list) or not entry["files"]
                or (evidence == "license-declaration" and len(entry["files"]) != 1)):
            raise NoticeError("Invalid supplemental license manifest.")
        files = []
        for item in entry["files"]:
            if not isinstance(item, dict) or set(item) != {"path", "sha256", "source"}:
                raise NoticeError("Invalid supplemental license manifest.")
            path, declared_hash, source = item["path"], item["sha256"], item["source"]
            if (not isinstance(declared_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", declared_hash)
                    or not isinstance(source, str)):
                raise NoticeError("Invalid supplemental license manifest.")
            parsed = urlparse(source)
            docs_rs_cargo = f"https://docs.rs/crate/{name}/{version}/source/Cargo.toml"
            if (parsed.scheme != "https" or not parsed.netloc or parsed.username or parsed.password
                    or (evidence == "license-text" and not PINNED_COMMIT.search(source))
                    or (evidence == "license-declaration" and source != docs_rs_cargo
                        and not PINNED_COMMIT.search(source))):
                raise NoticeError("Supplemental source is not pinned to a commit.")
            source_text, source_file = text_file(root, path, MAX_SUPPLEMENT_BYTES)
            declaration_license = None
            if evidence == "license-declaration":
                if PurePosixPath(path).name not in ("Cargo.toml", f"{name}-{version}-Cargo.toml.txt"):
                    raise NoticeError("Declaration evidence must be an upstream Cargo.toml.")
                try:
                    package = tomllib.loads(source_text).get("package")
                except tomllib.TOMLDecodeError as error:
                    raise NoticeError("Invalid declaration evidence.") from error
                if (not isinstance(package, dict) or package.get("name") != name
                        or package.get("version") != version
                        or not isinstance(package.get("license"), str) or not package["license"]):
                    raise NoticeError("Declaration evidence does not identify its package.")
                declaration_license = package["license"]
            actual_hash = sha256_file(source_file)
            if actual_hash != declared_hash:
                raise NoticeError("Supplemental license text hash does not match.")
            hashes[path] = actual_hash
            files.append({"path": path, "sha256": actual_hash, "source": source,
                          "text": source_text, "declaration_license": declaration_license})
        entries[key] = {"name": name, "version": version, "license": license_id, "evidence": evidence,
                        "files": files}
    return entries, hashes


def static_assets(root):
    relatives = list(STATIC_NOTICE_FILES)
    asset_root = checked_repo_file(root, "docs/licenses/assets-manifest.md").parent / "assets"
    if not asset_root.is_dir() or asset_root.is_symlink():
        raise NoticeError("Required asset notices are unavailable.")
    for path in sorted(asset_root.glob("*.txt")):
        relatives.append(path.relative_to(root).as_posix())
    assets, hashes = [], {}
    for relative in relatives:
        text, path = text_file(root, relative)
        assets.append({"path": relative, "text": text})
        hashes[relative] = sha256_file(path)
    return assets, hashes


def asset_tree_hashes(root):
    asset_root = root / "assets"
    if not asset_root.is_dir() or asset_root.is_symlink():
        raise NoticeError("Asset tree is unavailable.")
    hashes, files, total = {}, 0, 0
    for path in sorted(asset_root.rglob("*")):
        if path.is_symlink():
            raise NoticeError("Asset tree links are not accepted.")
        if not path.is_file():
            continue
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(asset_root) or not resolved.is_file():
            raise NoticeError("Invalid asset tree input.")
        files += 1
        total += resolved.stat().st_size
        if files > MAX_ASSET_FILES or total > MAX_ASSET_TREE_BYTES:
            raise NoticeError("Asset tree exceeds its bounded inventory.")
        hashes[resolved.relative_to(root).as_posix()] = sha256_file(resolved)
    return hashes


def input_state(root):
    """Return all publication inputs after validating their bounded form."""
    policy = validate_about(root)
    supplements, supplement_hashes = validate_supplements(root)
    assets, asset_hashes = static_assets(root)
    asset_hashes.update(asset_tree_hashes(root))
    hashes = {"Cargo.toml": sha256_file(checked_repo_file(root, "Cargo.toml")),
              "Cargo.lock": sha256_file(checked_repo_file(root, "Cargo.lock")),
              "Dioxus.toml": sha256_file(checked_repo_file(root, "Dioxus.toml")),
              "about.toml": sha256_file(policy), **supplement_hashes, **asset_hashes}
    return policy, supplements, assets, dict(sorted(hashes.items()))


def cargo_lock_checksums(root):
    text, _ = text_file(root, "Cargo.lock")
    try:
        packages = tomllib.loads(text).get("package")
    except tomllib.TOMLDecodeError as error:
        raise NoticeError("Invalid Cargo.lock provenance.") from error
    if not isinstance(packages, list):
        raise NoticeError("Invalid Cargo.lock provenance.")
    checksums = {}
    for package in packages:
        if not isinstance(package, dict):
            raise NoticeError("Invalid Cargo.lock provenance.")
        name, version, checksum = package.get("name"), package.get("version"), package.get("checksum")
        if checksum is None:
            continue
        if (not isinstance(name, str) or not isinstance(version, str)
                or not isinstance(checksum, str) or not re.fullmatch(r"[0-9a-f]{64}", checksum)):
            raise NoticeError("Invalid Cargo.lock provenance.")
        key = (name, version)
        if key in checksums:
            raise NoticeError("Ambiguous Cargo.lock provenance.")
        checksums[key] = checksum
    return checksums


def declaration_matches_metadata(name, version, checksum, declaration_license, metadata_license):
    """Only one audited Cargo legacy spelling differs from normalized metadata."""
    if declaration_license == metadata_license:
        return True
    return (name, version, checksum, declaration_license, metadata_license) == LEGACY_DECLARATION_TUPLE


def run_checked(command, root, runner):
    result = runner(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        raise NoticeError("cargo-about failed; raw command output is withheld.")
    return result.stdout


def cargo_about_version(tool, root, runner):
    output = run_checked([str(tool), "--version"], root, runner).strip()
    if output != "cargo-about " + EXPECTED_CARGO_ABOUT:
        raise NoticeError("Unexpected cargo-about version.")
    return output


def package_key(crate):
    package = crate.get("package") if isinstance(crate, dict) else None
    if not isinstance(package, dict):
        raise NoticeError("Invalid cargo-about package record.")
    name, version = package.get("name"), package.get("version")
    if not isinstance(name, str) or not name or not isinstance(version, str) or not version:
        raise NoticeError("Invalid cargo-about package record.")
    return name, version


def report_from_raw(raw, profile, variant, target, feature, supplements, lock_checksums):
    if not isinstance(raw, dict) or not isinstance(raw.get("crates"), list) or not isinstance(raw.get("licenses"), list):
        raise NoticeError("Invalid cargo-about JSON.")
    packages, crate_ids, raw_metadata = [], {}, {}
    for crate in raw["crates"]:
        name, version = package_key(crate)
        package = crate["package"]
        identifier, declared, package_license = package.get("id"), crate.get("license"), package.get("license")
        if (not isinstance(identifier, str) or not identifier or not isinstance(declared, str) or not declared
                or not isinstance(package_license, str) or not package_license):
            raise NoticeError("Invalid cargo-about package record.")
        if identifier in crate_ids or (name, version) in raw_metadata:
            raise NoticeError("Duplicate cargo-about package record.")
        crate_ids[identifier] = (name, version)
        raw_metadata[(name, version)] = {"license": package_license, "id": identifier}
        packages.append({"name": name, "version": version,
                         "source": validate_external_source(package.get("source")),
                         "repository": validate_repository(package.get("repository")),
                         "declared_license": declared, "selected_licenses": []})
    if not packages:
        raise NoticeError("cargo-about selected no packages.")
    index = {(package["name"], package["version"]): package for package in packages}
    license_sections, required, selected_keys = [], set(), set()
    for item in raw["licenses"]:
        if not isinstance(item, dict):
            raise NoticeError("Invalid cargo-about license record.")
        name, license_id, text = item.get("name"), item.get("id"), item.get("text")
        used_by = item.get("used_by")
        if (not all(isinstance(part, str) and part for part in (name, license_id, text))
                or not isinstance(used_by, list) or not used_by):
            raise NoticeError("Invalid cargo-about license record.")
        users = []
        for use in used_by:
            crate = use.get("crate") if isinstance(use, dict) else None
            if not isinstance(crate, dict):
                raise NoticeError("Invalid cargo-about license record.")
            identifier = crate.get("id")
            if identifier not in crate_ids:
                raise NoticeError("License record references an unknown package.")
            key = crate_ids[identifier]
            users.append(key)
            selected = index[key]["selected_licenses"]
            if license_id not in selected:
                selected.append(license_id)
            selected_keys.add((key[0], key[1], license_id))
            if not item.get("source_path"):
                required.add((key[0], key[1], license_id))
        license_sections.append({"name": name, "id": license_id, "text": text,
                                 "used_by": users})
    missing = sorted(required.difference(supplements))
    if missing:
        raise NoticeError("A selected license has no packaged upstream text.")
    # A source_path satisfies cargo-about's generic license text requirement,
    # but a matching upstream NOTICE/exception remains additive material.
    selected_supplements = [supplements[key] for key in sorted(selected_keys.intersection(supplements))]
    declarations = []
    for supplement in selected_supplements:
        if supplement["evidence"] != "license-declaration":
            continue
        key = (supplement["name"], supplement["version"])
        package_license = raw_metadata[key]["license"]
        checksum = lock_checksums.get(key)
        declaration_license = supplement["files"][0]["declaration_license"]
        if (checksum is None or not declaration_matches_metadata(
                supplement["name"], supplement["version"], checksum,
                declaration_license, package_license)):
            raise NoticeError("Declaration evidence does not match cargo-about metadata.")
        declarations.append({"name": supplement["name"], "version": supplement["version"],
                             "selected_license": supplement["license"],
                             "metadata_license": package_license,
                             "declaration_license": declaration_license,
                             "package_checksum": checksum,
                             "source": supplement["files"][0]["source"]})
    for package in packages:
        package["selected_licenses"].sort()
        if not package["selected_licenses"]:
            raise NoticeError("A cargo-about package has no selected license.")
    packages.sort(key=lambda package: (package["name"], package["version"]))
    return {"profile": profile, "variant": variant, "target": target, "feature": feature,
            "package_count": len(packages), "packages": packages,
            "licenses": license_sections, "supplements": selected_supplements,
            "declaration_only_packages": declarations}


def esc(value):
    return html.escape(value, quote=True)


def render_html(reports, assets, provenance):
    lines = ["<!doctype html>", "<html lang=\"en\"><meta charset=\"utf-8\">",
             "<title>Third-party notices</title>", "<body>",
             "<h1>Third-party notices</h1>",
             "<p>Generated from cargo-about; machine-local source paths are intentionally omitted.</p>",
             "<h2>Provenance</h2><pre>", esc(json.dumps(provenance, indent=2, sort_keys=True)), "</pre>"]
    for report in reports:
        lines.extend(["<section>", "<h2>" + esc(report["profile"] + " / " + report["variant"]) + "</h2>",
                      "<p>Target: <code>" + esc(report["target"]) + "</code>; feature: <code>"
                      + esc(report["feature"]) + "</code>; packages: " + str(report["package_count"]) + "</p>",
                      "<h3>Packages</h3><ul>"])
        for package in report["packages"]:
            selected = ", ".join(package["selected_licenses"])
            lines.append("<li><strong>" + esc(package["name"]) + " " + esc(package["version"])
                         + "</strong> — declared: " + esc(package["declared_license"])
                         + "; selected: " + esc(selected) + "; source: "
                         + esc(package["source"] or "none") + "; repository: "
                         + esc(package["repository"] or "none") + "</li>")
        lines.append("</ul><h3>cargo-about license texts</h3>")
        for license_section in report["licenses"]:
            users = ", ".join(name + " " + version for name, version in license_section["used_by"])
            lines.extend(["<h4>" + esc(license_section["name"]) + " (" + esc(license_section["id"]) + ")</h4>",
                          "<p>Used by: " + esc(users) + "</p><pre>", esc(license_section["text"]), "</pre>"])
        lines.append("<h3>Supplemental upstream license texts</h3>")
        for supplement in report["supplements"]:
            if supplement["evidence"] == "license-declaration":
                continue
            lines.append("<h4>" + esc(supplement["name"] + " " + supplement["version"]
                                            + " — " + supplement["license"]) + "</h4>")
            for source in supplement["files"]:
                lines.extend(["<p>Upstream: " + esc(source["source"]) + "</p><pre>",
                              esc(source["text"]), "</pre>"])
        if report["declaration_only_packages"]:
            lines.append("<h3>License declaration only</h3>")
        for declaration in report["declaration_only_packages"]:
            lines.extend(["<h4>" + esc(declaration["name"] + " " + declaration["version"]
                                         + " — " + declaration["selected_license"]) + "</h4>",
                          "<p><strong>License declaration only; no separate upstream license text located.</strong> "
                          "The original public Cargo.toml declaration is included below without changing its expression.</p>",
                          "<p>Cargo metadata expression: " + esc(declaration["metadata_license"])
                          + "; original Cargo.toml declaration: " + esc(declaration["declaration_license"])
                          + "; source: " + esc(declaration["source"]) + "</p>"])
            for supplement in report["supplements"]:
                if (supplement["evidence"] == "license-declaration"
                        and supplement["name"] == declaration["name"]
                        and supplement["version"] == declaration["version"]
                        and supplement["license"] == declaration["selected_license"]):
                    lines.extend(["<pre>", esc(supplement["files"][0]["text"]), "</pre>"])
        lines.append("</section>")
    lines.append("<section><h2>Asset notices</h2>")
    for asset in assets:
        lines.extend(["<h3>" + esc(asset["path"]) + "</h3><pre>", esc(asset["text"]), "</pre>"])
    lines.extend(["</section>", "</body></html>"])
    return "\n".join(lines) + "\n"


def write_new_output(output, inventory, document):
    if output.exists() or not output.parent.is_dir():
        raise NoticeError("Output directory must be new and its parent must already exist.")
    with tempfile.TemporaryDirectory(prefix=".fedkr-notices-stage-", dir=output.parent) as stage_name:
        stage = Path(stage_name)
        (stage / "third-party-inventory.json").write_bytes(
            (json.dumps(inventory, indent=2, sort_keys=True) + "\n").encode("utf-8"))
        (stage / "THIRD-PARTY-NOTICES.html").write_bytes(document.encode("utf-8"))
        try:
            output.mkdir()
            marker = output / ".INCOMPLETE"
            marker.write_text("Incomplete notice output; do not distribute.\n", encoding="ascii")
            for child in stage.iterdir():
                os.replace(child, output / child.name)
            marker.unlink()
        except OSError as error:
            raise NoticeError("Notice output is incomplete; do not distribute it.") from error


def generate(root, cargo_about, profile, output, runner=subprocess.run):
    root = Path(root).resolve(strict=True)
    try:
        cargo_about = Path(cargo_about).resolve(strict=True)
    except OSError as error:
        raise NoticeError("cargo-about executable is unavailable.") from error
    if not cargo_about.is_file() or cargo_about.is_symlink():
        raise NoticeError("cargo-about executable is unavailable.")
    supplied_output = Path(output)
    if supplied_output.is_symlink():
        raise NoticeError("Output directory links are not accepted.")
    output = supplied_output.resolve()
    policy, supplements, assets, input_hashes = input_state(root)
    tool_hash = sha256_file(cargo_about)
    version = cargo_about_version(cargo_about, root, runner)
    reports = []
    with tempfile.TemporaryDirectory(prefix="fedkr-cargo-about-") as raw_directory:
        raw_directory = Path(raw_directory)
        for variant, target, feature in PROFILES[profile]:
            raw_path = raw_directory / (variant + ".json")
            command = [str(cargo_about), "generate", "--locked", "--offline", "--no-default-features",
                       "--features", feature, "--target", target, "--fail", "--format", "json",
                       "-o", str(raw_path)]
            run_checked(command, root, runner)
            try:
                raw = json.loads(raw_path.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise NoticeError("cargo-about did not produce valid JSON.") from error
            reports.append(report_from_raw(raw, profile, variant, target, feature, supplements,
                                           cargo_lock_checksums(root)))
    _, _, _, after_hashes = input_state(root)
    if cargo_about.is_symlink() or sha256_file(cargo_about) != tool_hash or after_hashes != input_hashes:
        raise NoticeError("Notice inputs changed while cargo-about was running.")
    provenance = {"cargo_about": {"version": version, "sha256": tool_hash},
                  "inputs": input_hashes,
                  "graph": [{key: report[key] for key in ("profile", "variant", "target", "feature", "package_count")}
                            for report in reports]}
    document = render_html(reports, assets, provenance)
    inventory = {"format": 1, "html_sha256": hashlib.sha256(document.encode("utf-8")).hexdigest(),
                 "provenance": provenance,
                 "profiles": [{key: report[key] for key in ("profile", "variant", "target", "feature", "package_count", "packages", "declaration_only_packages")}
                              for report in reports]}
    write_new_output(output, inventory, document)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-about", required=True, help="pinned cargo-about 0.9.2 executable")
    parser.add_argument("--profile", required=True, choices=sorted(PROFILES))
    parser.add_argument("--output-dir", required=True, help="new directory for sanitized notices")
    args = parser.parse_args()
    try:
        output = generate(Path(__file__).resolve().parents[1], args.cargo_about, args.profile,
                          args.output_dir)
    except NoticeError as error:
        parser.error(str(error))
    print(output)


if __name__ == "__main__":
    main()
