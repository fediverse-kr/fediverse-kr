#!/usr/bin/env python3
"""Copy only a built Linux release into a new OCI context; never a checkout/DB."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat


NOTICE_FILES = ('THIRD-PARTY-NOTICES.html', 'third-party-inventory.json')
LINUX_PROFILE_TARGETS = {
    'linux-server': 'x86_64-unknown-linux-gnu',
    'linux-aarch64-server': 'aarch64-unknown-linux-gnu',
}
ELF_MACHINE_TARGETS = {
    62: 'x86_64-unknown-linux-gnu',
    183: 'aarch64-unknown-linux-gnu',
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def notice_inputs(root: Path) -> set:
    """Required local inputs, not a second dependency/license resolver."""
    required = {'Cargo.toml', 'Cargo.lock', 'about.toml', 'Dioxus.toml',
                'docs/licenses/supplemental.json', 'docs/icon-pack-notices.md',
                'docs/licenses/assets-manifest.md'}
    for directory in ('assets', 'docs/licenses/assets'):
        required.update(path.relative_to(root).as_posix()
                        for path in (root / directory).rglob('*') if path.is_file())
    supplement = json.loads((root / 'docs/licenses/supplemental.json').read_text(encoding='utf-8'))
    for entry in supplement['entries']:
        required.update(item['path'] for item in entry['files'])
    return required


def load_notices(directory: Path) -> tuple[list[Path], str]:
    if (not directory.is_absolute() or directory.is_symlink() or not directory.is_dir()
            or {path.name for path in directory.iterdir()} != set(NOTICE_FILES)):
        raise ValueError('Expected a complete, dedicated notice output directory.')
    files = [directory / name for name in NOTICE_FILES]
    for path in files:
        if (path.is_symlink() or not stat.S_ISREG(path.stat().st_mode)
                or not 0 < path.stat().st_size <= 16 * 1024 * 1024):
            raise ValueError('Expected bounded regular notice files.')
    inventory = json.loads(files[1].read_text(encoding='utf-8'))
    if inventory.get('format') != 1 or inventory.get('html_sha256') != sha256_file(files[0]):
        raise ValueError('Notice HTML integrity check failed.')
    provenance = inventory['provenance']
    if (provenance['cargo_about']['version'] != 'cargo-about 0.9.2'
            or not re.fullmatch('[0-9a-f]{64}', provenance['cargo_about']['sha256'])):
        raise ValueError('Unrecognized notice generator evidence.')
    profile_targets = []
    for graphs in (provenance['graph'], inventory['profiles']):
        profile_names = {g.get('profile') for g in graphs}
        if len(profile_names) != 1:
            raise ValueError('A single Linux server plus web notice profile is required.')
        profile = profile_names.pop()
        expected_target = LINUX_PROFILE_TARGETS.get(profile)
        expected = {('server', expected_target), ('web', 'wasm32-unknown-unknown')}
        if (expected_target is None or len(graphs) != 2
                or {(g.get('feature'), g.get('target')) for g in graphs} != expected
                or any(not isinstance(g.get('package_count'), int) or g['package_count'] <= 0
                       for g in graphs)):
            raise ValueError('A Linux server plus web notice profile is required.')
        profile_targets.append(expected_target)
    if profile_targets[0] != profile_targets[1]:
        raise ValueError('Notice provenance and inventory target differ.')
    root = Path(__file__).resolve().parents[1]
    inputs = provenance['inputs']
    if not notice_inputs(root).issubset(inputs):
        raise ValueError('Notice source evidence is incomplete.')
    for relative, expected_hash in inputs.items():
        parsed = PurePosixPath(relative)
        if (not relative or parsed.is_absolute() or ':' in relative or '\\' in relative
                or '..' in parsed.parts or not re.fullmatch('[0-9a-f]{64}', expected_hash)):
            raise ValueError('Unsafe notice source evidence.')
        path = root / relative
        if (any(part.is_symlink() for part in (path, *path.parents) if part != root and root in part.parents)
                or not path.is_file() or not path.resolve(strict=True).is_relative_to(root)
                or path.stat().st_size > 512 * 1024 * 1024 or sha256_file(path) != expected_hash):
            raise ValueError('Notice input changed; regenerate before packaging.')
    return files, profile_targets[0]


def linux_elf_target(server: Path) -> str:
    with server.open('rb') as handle:
        header = handle.read(20)
    if len(header) != 20 or header[:4] != b'\x7fELF' or header[4:6] != b'\x02\x01':
        raise ValueError('Only an already-built 64-bit little-endian Linux ELF release is accepted.')
    target = ELF_MACHINE_TARGETS.get(int.from_bytes(header[18:20], 'little'))
    if target is None:
        raise ValueError('Unsupported Linux ELF target in release context.')
    return target


def prepare(bundle: Path, output: Path, notices: Path) -> dict:
    if not bundle.is_absolute() or not output.is_absolute():
        raise ValueError('Absolute bundle/output paths are required.')
    bundle = bundle.resolve(strict=True)
    if not bundle.is_dir() or output.exists() or output.is_symlink():
        raise ValueError('Use a release directory and a new output path.')
    output = output.parent.resolve(strict=True) / output.name
    if output == bundle or bundle in output.parents or output in bundle.parents:
        raise ValueError('The context and release must be disjoint.')
    server, public = bundle / 'server', bundle / 'public'
    if server.is_symlink() or not server.is_file() or public.is_symlink() or not public.is_dir():
        raise ValueError('Expected regular server and public/ entries.')
    server_target = linux_elf_target(server)
    if not 0 < server.stat().st_size <= 512 * 1024 * 1024 or not os.access(server, os.X_OK):
        raise ValueError('Expected a bounded executable release.')
    files = []
    for path in public.rglob('*'):
        mode = path.lstat().st_mode
        if path.is_symlink() or not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
            raise ValueError('Public release links and special files are not accepted.')
        relative = path.relative_to(public)
        if any(part.startswith('.') for part in relative.parts):
            raise ValueError('Hidden files do not belong in this public release context.')
        if path.is_file():
            files.append(path)
    if (not (public / 'index.html').is_file() or not 0 < len(files) <= 1024
            or not any(path.suffix == '.wasm' for path in files)
            or sum(path.stat().st_size for path in files) > 512 * 1024 * 1024):
        raise ValueError('Expected the bounded matching Dioxus public release, including WASM.')
    notice_files, notice_target = load_notices(notices)
    if notice_target != server_target:
        raise ValueError('Release ELF and notice target must match.')
    if any((public / name).exists() for name in NOTICE_FILES):
        raise ValueError('Release notice paths already exist; do not silently replace them.')
    recipe = Path(__file__).resolve().parents[1] / 'deploy/container/Containerfile'
    # Exclusive mkdir protects existing output. A copy failure leaves this newly
    # created context for inspection; this command never recursively deletes it.
    output.mkdir(mode=0o700)
    shutil.copy2(server, output / 'server')
    shutil.copytree(public, output / 'public')
    for notice in notice_files:
        shutil.copy2(notice, output / 'public' / notice.name)
    shutil.copy2(recipe, output / 'Containerfile')
    return dict(public_files=len(files) + len(notice_files), third_party_notices_included=True,
                source_or_private_files_copied=False,
                runtime_verified=False)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--notices', required=True, type=Path,
                        help='Fresh linux-server output from generate-third-party-notices.py')
    args = parser.parse_args()
    try:
        print(json.dumps(prepare(args.bundle, args.output, args.notices)))
    except (OSError, ValueError, KeyError, TypeError):
        raise SystemExit('Context preparation refused or incomplete; no existing output was replaced. '
                         'Any newly created partial context is retained. Details withheld.')
