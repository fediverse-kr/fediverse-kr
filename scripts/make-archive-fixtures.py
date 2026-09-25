#!/usr/bin/env python3
"""Create synthetic AES-256 ZIP fixtures for the private-backup reader.

Every payload in this file is a fixed, public test value.  The password is
deliberately not accepted as a command-line argument and is never printed.
This generator does not open or inspect any user archive.
"""

from __future__ import annotations

import argparse
import json
import stat
import struct
import sys
import zipfile
from pathlib import Path
from typing import Iterable


PUBLIC_TEST_PASSWORD = b"PUBLIC_TEST_PASSWORD"
"""Fixed synthetic password; never use for a real backup."""

REPO_ROOT = Path(__file__).resolve().parents[1]
PYZIPPER_ROOT = REPO_ROOT / ".local" / "tools" / "pyzipper-0.4.0"
if PYZIPPER_ROOT.is_dir():
    sys.path.insert(0, str(PYZIPPER_ROOT))

try:
    import pyzipper
except ImportError as exc:  # pragma: no cover - exercised by the caller's setup
    raise SystemExit(
        "pyzipper 0.4.0 is required under .local/tools/pyzipper-0.4.0"
    ) from exc


NORMAL_MEMBERS: tuple[tuple[str, bytes], ...] = (
    ("database.dump", b"PGDMP\x01\x00SYNTHETIC-DATABASE-DUMP\n"),
    ("restore.sql", b"-- SYNTHETIC SQL BACKUP\nSELECT 'synthetic';\n"),
    ("notes.txt", b"SYNTHETIC BACKUP NOTE\n"),
    ("metadata.json", b'{"synthetic":true,"kind":"backup"}\n'),
    ("README.md", b"# Synthetic backup fixture\n"),
    ("checksum.sha256", b"SYNTHETIC-CHECKSUM-NOT-FOR-PRODUCTION\n"),
)


def write_aes_zip(
    path: Path,
    members: Iterable[tuple[str | zipfile.ZipInfo, bytes]],
    *,
    compression: int = pyzipper.ZIP_DEFLATED,
) -> None:
    """Write a WinZip AES-256 archive using pyzipper's public API."""

    with pyzipper.AESZipFile(
        path,
        mode="w",
        compression=compression,
        encryption=pyzipper.WZ_AES,
    ) as archive:
        archive.setpassword(PUBLIC_TEST_PASSWORD)
        archive.setencryption(pyzipper.WZ_AES, nbits=256)
        for name, payload in members:
            if isinstance(name, zipfile.ZipInfo):
                # AESZipFile requires its matching ZipInfo subclass.  Copy
                # only the synthetic metadata needed for the symlink case;
                # encryption and compression remain library-managed.
                aes_info = archive.zipinfo_cls(name.filename)
                aes_info.create_system = name.create_system
                aes_info.external_attr = name.external_attr
                aes_info.comment = name.comment
                aes_info.extra = name.extra
                archive.writestr(aes_info, payload, compress_type=compression)
            else:
                archive.writestr(name, payload)


def write_unencrypted_zip(
    path: Path,
    members: Iterable[tuple[str, bytes]],
    *,
    compression: int = zipfile.ZIP_DEFLATED,
) -> None:
    """Write a deliberately unencrypted synthetic archive."""

    with zipfile.ZipFile(path, mode="w", compression=compression) as archive:
        for name, payload in members:
            archive.writestr(name, payload)


def tamper_final_authentication_byte(path: Path) -> None:
    """Flip only the final byte of the synthetic AES member payload.

    The ZIP header is read solely to locate the generated member; no member
    data is decoded.  The final encrypted-member byte is the WinZip AES
    authentication tag byte for this one-member fixture.
    """

    with zipfile.ZipFile(path) as archive:
        info = archive.infolist()[0]
        header_offset = info.header_offset
        with path.open("rb") as source:
            source.seek(header_offset)
            local_header = source.read(30)
        if len(local_header) != 30 or local_header[:4] != b"PK\x03\x04":
            raise RuntimeError("generated ZIP local header is invalid")
        name_length, extra_length = struct.unpack_from("<HH", local_header, 26)
        payload_offset = header_offset + 30 + name_length + extra_length
        payload_end = payload_offset + info.compress_size
        if info.compress_size < 1:
            raise RuntimeError("generated AES member has no payload")

    with path.open("r+b") as archive:
        archive.seek(payload_end - 1)
        original = archive.read(1)
        if len(original) != 1:
            raise RuntimeError("generated AES member ended unexpectedly")
        archive.seek(payload_end - 1)
        archive.write(bytes((original[0] ^ 0x01,)))


def preserve_backslash_member_name(path: Path, generated_name: str, preserved_name: str) -> None:
    """Restore a backslash in generated ZIP name metadata.

    pyzipper follows the usual ZIP writer behavior and normalizes ``\\`` to
    ``/``.  The fixture must exercise a hostile raw name, so update only the
    equal-length filename fields in the generated local and central headers;
    the encrypted member bytes are not touched.
    """

    old = generated_name.encode("utf-8")
    new = preserved_name.encode("utf-8")
    if len(old) != len(new):
        raise ValueError("backslash fixture names must have equal byte length")
    raw = bytearray(path.read_bytes())
    replaced = 0
    for signature, length_offset, name_delta, comment_delta in (
        (b"PK\x03\x04", 26, 4, None),
        (b"PK\x01\x02", 28, 18, 4),
    ):
        cursor = 0
        while True:
            header_offset = raw.find(signature, cursor)
            if header_offset < 0:
                break
            if header_offset + length_offset + 2 > len(raw):
                break
            name_length = int.from_bytes(
                raw[header_offset + length_offset : header_offset + length_offset + 2],
                "little",
            )
            extra_length = int.from_bytes(
                raw[header_offset + length_offset + 2 : header_offset + length_offset + 4],
                "little",
            )
            name_offset = header_offset + length_offset + name_delta
            if raw[name_offset : name_offset + name_length] == old:
                raw[name_offset : name_offset + name_length] = new
                replaced += 1
            comment_length = (
                int.from_bytes(
                    raw[header_offset + length_offset + comment_delta :
                        header_offset + length_offset + comment_delta + 2],
                    "little",
                )
                if comment_delta is not None
                else 0
            )
            cursor = name_offset + name_length + extra_length + comment_length
    if replaced != 2:
        raise RuntimeError("generated backslash fixture did not have two name headers")
    path.write_bytes(raw)


def symlink_info(name: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name)
    info.create_system = 3
    info.external_attr = (stat.S_IFLNK | 0o777) << 16
    return info


def create_fixtures(output: Path) -> dict[str, object]:
    output.mkdir(parents=True, exist_ok=False)

    write_aes_zip(output / "normal-aes256.zip", NORMAL_MEMBERS)
    write_aes_zip(output / "empty-entry.zip", (("empty.txt", b""),))

    write_aes_zip(
        output / "tampered-aes-tag.zip",
        (("tag-check.txt", b"SYNTHETIC AES TAG CHECK\n"),),
    )
    tamper_final_authentication_byte(output / "tampered-aes-tag.zip")

    write_aes_zip(output / "traversal-name.zip", (("../escape.txt", b"SYNTHETIC TRAVERSAL\n"),))
    write_aes_zip(output / "absolute-name.zip", (("/absolute.txt", b"SYNTHETIC ABSOLUTE\n"),))
    write_aes_zip(
        output / "backslash-name.zip",
        ((r"nested\escape.txt", b"SYNTHETIC BACKSLASH\n"),),
    )
    preserve_backslash_member_name(
        output / "backslash-name.zip", "nested/escape.txt", r"nested\escape.txt"
    )
    write_aes_zip(
        output / "case-collision.zip",
        (("Case.txt", b"SYNTHETIC CASE A\n"), ("case.txt", b"SYNTHETIC CASE B\n")),
    )
    write_aes_zip(
        output / "symlink-entry.zip",
        ((symlink_info("link-to-target"), b"target.txt"),),
    )
    write_aes_zip(
        output / "nested-and-directories.zip",
        (
            ("empty/", b""),
            ("nested/", b""),
            ("nested/한국어.txt", b"SYNTHETIC KOREAN NESTED\n"),
        ),
    )
    write_aes_zip(
        output / "file-directory-conflict.zip",
        (("a/b.txt", b"SYNTHETIC CHILD FIRST\n"), ("a", b"SYNTHETIC FILE CONFLICT\n")),
    )
    write_unencrypted_zip(
        output / "unencrypted.zip", (("plain.txt", b"SYNTHETIC UNENCRYPTED\n"),)
    )
    write_aes_zip(
        output / "reserved-name.zip", (("CON.txt", b"SYNTHETIC RESERVED NAME\n"),)
    )
    write_aes_zip(
        output / "too-many-entries.zip",
        ((f"entry-{index:03d}.txt", b"SYNTHETIC ENTRY\n") for index in range(257)),
    )

    ratio_payload = b"A" * (1024 * 1024)
    write_aes_zip(output / "ratio-over-100.zip", (("repeating.bin", ratio_payload),))

    bzip2 = getattr(pyzipper, "ZIP_BZIP2", zipfile.ZIP_BZIP2)
    write_aes_zip(
        output / "unsupported-bzip2.zip",
        (("bzip2.txt", b"SYNTHETIC BZIP2 COMPRESSION\n"),),
        compression=bzip2,
    )

    manifest = {
        "version": 1,
        "password_label": "PUBLIC_TEST_PASSWORD",
        "password_is_test_only": True,
        "fixtures": [
            {"name": "normal-aes256.zip", "kind": "encrypted", "entries": 6},
            {"name": "empty-entry.zip", "kind": "encrypted", "entries": 1},
            {"name": "tampered-aes-tag.zip", "kind": "encrypted-tampered-tag", "entries": 1},
            {"name": "traversal-name.zip", "kind": "encrypted-path-traversal", "entries": 1},
            {"name": "absolute-name.zip", "kind": "encrypted-absolute-path", "entries": 1},
            {"name": "backslash-name.zip", "kind": "encrypted-backslash-path", "entries": 1},
            {"name": "case-collision.zip", "kind": "encrypted-case-collision", "entries": 2},
            {"name": "symlink-entry.zip", "kind": "encrypted-symlink", "entries": 1},
            {"name": "nested-and-directories.zip", "kind": "encrypted-nested-directory", "entries": 3},
            {"name": "file-directory-conflict.zip", "kind": "encrypted-file-directory-conflict", "entries": 2},
            {"name": "unencrypted.zip", "kind": "unencrypted", "entries": 1},
            {"name": "reserved-name.zip", "kind": "encrypted-reserved-name", "entries": 1},
            {"name": "too-many-entries.zip", "kind": "encrypted-entry-count", "entries": 257},
            {"name": "ratio-over-100.zip", "kind": "encrypted-compression-ratio", "entries": 1},
            {"name": "unsupported-bzip2.zip", "kind": "encrypted-unsupported-compression", "entries": 1},
        ],
    }
    (output / "manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="new, non-existent directory for synthetic fixtures",
    )
    args = parser.parse_args()
    manifest = create_fixtures(args.output)
    print(f"created {len(manifest['fixtures'])} synthetic fixtures in {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
