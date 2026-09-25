#!/usr/bin/env python3
"""Synthetic context preparation checks. No container engine or network."""
import importlib.util
import hashlib
import json
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('context_builder', Path(__file__).with_name('prepare-container-context.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

with tempfile.TemporaryDirectory(prefix='fedkr-context-test-') as temporary:
    root = Path(temporary)
    def elf64(machine):
        return (b'\x7fELF' + b'\x02\x01\x01' + b'\0' * 9 + b'\x02\0'
                + machine.to_bytes(2, 'little') + b'\x01\0\0\0' + b'\0' * 40)

    bundle = root / 'release'
    public = bundle / 'public'
    public.mkdir(parents=True)
    (bundle / 'server').write_bytes(elf64(62) + b'synthetic-not-a-real-release')
    (bundle / 'server').chmod(0o755)
    (public / 'index.html').write_text('<html>synthetic</html>')
    (public / 'app.wasm').write_bytes(b'\0asm')
    (public / 'app.js').write_bytes(b'console.log("synthetic");\n')
    (public / 'assets').mkdir()
    (public / 'assets/icon.bin').write_bytes(b'public nested bytes')
    (bundle / '.env').write_text('synthetic private sentinel, never copied')
    checks = 0
    notices = root / 'notices'
    notices.mkdir()
    notice_html = b'<!doctype html><title>Synthetic notices</title><p>not real license evidence</p>'
    (notices / builder.NOTICE_FILES[0]).write_bytes(notice_html)
    project = Path(__file__).resolve().parents[1]
    graphs = [dict(profile='linux-server', variant=feature, feature=feature, target=target,
                   package_count=1) for feature, target in
              [('server', 'x86_64-unknown-linux-gnu'), ('web', 'wasm32-unknown-unknown')]]
    inventory = dict(format=1, html_sha256=hashlib.sha256(notice_html).hexdigest(),
                     provenance=dict(cargo_about=dict(version='cargo-about 0.9.2', sha256='a' * 64),
                                     inputs={p: builder.sha256_file(project / p)
                                             for p in builder.notice_inputs(project)}, graph=graphs),
                     profiles=graphs)

    def save_inventory(value):
        (notices / builder.NOTICE_FILES[1]).write_text(json.dumps(value), encoding='utf-8')

    save_inventory(inventory)

    def rejected(source, target):
        global checks
        try:
            builder.prepare(source, target, notices)
        except (ValueError, OSError):
            checks += 1
            return
        raise AssertionError('Unsafe context accepted')

    rejected(bundle, bundle / 'nested')
    rejected(bundle, root)
    rejected(Path('relative'), root / 'relative-output')
    invalid = root / 'invalid-elf'
    (invalid / 'public').mkdir(parents=True)
    (invalid / 'server').write_bytes(b'not an ELF')
    (invalid / 'server').chmod(0o755)
    (invalid / 'public/index.html').write_text('<html>invalid</html>')
    (invalid / 'public/app.wasm').write_bytes(b'\0asm')
    rejected(invalid, root / 'invalid-elf-output')

    symlink_server = root / 'symlink-server'
    (symlink_server / 'public').mkdir(parents=True)
    (symlink_server / 'server-target').write_bytes(elf64(62) + b'synthetic')
    (symlink_server / 'server-target').chmod(0o755)
    (symlink_server / 'server').symlink_to(symlink_server / 'server-target')
    (symlink_server / 'public/index.html').write_text('<html>symlink</html>')
    (symlink_server / 'public/app.wasm').write_bytes(b'\0asm')
    rejected(symlink_server, root / 'symlink-server-output')

    symlink_public = root / 'symlink-public'
    (symlink_public / 'public-target').mkdir(parents=True)
    (symlink_public / 'server').write_bytes(elf64(62) + b'synthetic')
    (symlink_public / 'server').chmod(0o755)
    (symlink_public / 'public').symlink_to(symlink_public / 'public-target', target_is_directory=True)
    (symlink_public / 'public-target/index.html').write_text('<html>symlink</html>')
    (symlink_public / 'public-target/app.wasm').write_bytes(b'\0asm')
    rejected(symlink_public, root / 'symlink-public-output')

    output = root / 'context'
    builder.prepare(bundle, output, notices)
    assert {p.name for p in output.iterdir()} == {'server', 'public', 'Containerfile'}
    source_public_files = sorted(path.relative_to(public) for path in public.rglob('*') if path.is_file())
    output_public_files = sorted(path.relative_to(output / 'public') for path in (output / 'public').rglob('*') if path.is_file())
    assert output_public_files == sorted(source_public_files + [Path(p) for p in builder.NOTICE_FILES])
    assert all((output / 'public' / path).read_bytes() == (public / path).read_bytes() for path in source_public_files)
    assert (output / 'server').read_bytes() == (bundle / 'server').read_bytes()
    checks += 4
    assert all((output / 'public' / p).read_bytes() == (notices / p).read_bytes()
               for p in builder.NOTICE_FILES)
    checks += 1

    arm_bundle = root / 'aarch64-release'
    arm_public = arm_bundle / 'public'
    arm_public.mkdir(parents=True)
    (arm_bundle / 'server').write_bytes(elf64(183) + b'synthetic-aarch64-release')
    (arm_bundle / 'server').chmod(0o755)
    (arm_public / 'index.html').write_text('<html>aarch64</html>')
    (arm_public / 'app.wasm').write_bytes(b'\0asm')
    graphs[0].update(profile='linux-aarch64-server', target='aarch64-unknown-linux-gnu')
    graphs[1]['profile'] = 'linux-aarch64-server'
    save_inventory(inventory)
    arm_context = root / 'aarch64-context'
    builder.prepare(arm_bundle, arm_context, notices)
    assert (arm_context / 'server').read_bytes() == (arm_bundle / 'server').read_bytes()
    rejected(bundle, root / 'mismatched-arm64-notice')
    graphs[0].update(profile='linux-server', target='x86_64-unknown-linux-gnu')
    graphs[1]['profile'] = 'linux-server'
    save_inventory(inventory)
    checks += 1

    # Real input hashes are read-only evidence here; mutate only this synthetic
    # inventory, never repository sources, Cargo cache or user data.
    inventory['provenance']['inputs']['Cargo.lock'] = '0' * 64
    save_inventory(inventory)
    rejected(bundle, root / 'stale-notices')
    inventory['provenance']['inputs']['Cargo.lock'] = builder.sha256_file(project / 'Cargo.lock')
    omitted = inventory['provenance']['inputs'].pop('Cargo.toml')
    save_inventory(inventory)
    rejected(bundle, root / 'omitted-notice-input')
    inventory['provenance']['inputs']['Cargo.toml'] = omitted
    inventory['provenance']['inputs']['../private-sentinel'] = 'a' * 64
    save_inventory(inventory)
    rejected(bundle, root / 'unsafe-notice-input')
    del inventory['provenance']['inputs']['../private-sentinel']
    graphs[0]['target'] = 'x86_64-pc-windows-msvc'
    save_inventory(inventory)
    rejected(bundle, root / 'wrong-notice-target')
    graphs[0]['target'] = 'x86_64-unknown-linux-gnu'
    save_inventory(inventory)
    (notices / builder.NOTICE_FILES[0]).write_bytes(notice_html + b'tampered')
    rejected(bundle, root / 'tampered-notices')
    (notices / builder.NOTICE_FILES[0]).write_bytes(notice_html)
    (notices / '.INCOMPLETE').write_text('synthetic incomplete marker')
    rejected(bundle, root / 'incomplete-notices')
    (notices / '.INCOMPLETE').unlink()
    (public / builder.NOTICE_FILES[0]).write_bytes(b'existing public sentinel')
    rejected(bundle, root / 'notice-collision')
    (public / builder.NOTICE_FILES[0]).unlink()
    (notices / builder.NOTICE_FILES[0]).unlink()
    (notices / builder.NOTICE_FILES[0]).symlink_to(bundle / '.env')
    rejected(bundle, root / 'notice-link')
    (notices / builder.NOTICE_FILES[0]).unlink()
    (notices / builder.NOTICE_FILES[0]).write_bytes(notice_html)
    assert not any((root / name).exists() for name in
                   ('stale-notices', 'omitted-notice-input', 'unsafe-notice-input', 'wrong-notice-target',
                    'tampered-notices', 'incomplete-notices', 'notice-collision', 'notice-link'))
    checks += 1

    existing = root / 'existing-output'
    existing.mkdir()
    sentinel = existing / 'sentinel'
    sentinel.write_text('preserve existing output')
    rejected(bundle, existing)
    assert sentinel.read_text() == 'preserve existing output'
    assert not (existing / 'server').exists() and not (existing / 'public').exists()
    checks += 1

    rejected(bundle, output)
    (public / 'link').symlink_to(bundle / '.env')
    rejected(bundle, root / 'linked-output')
    (public / 'link').unlink()
    (public / '.env').write_text('synthetic sentinel')
    rejected(bundle, root / 'hidden-output')
    (public / '.env').unlink()
    (public / 'index.html').unlink()
    rejected(bundle, root / 'incomplete-output')
    assert not any((root / name).exists() for name in ('relative-output', 'invalid-elf-output', 'symlink-server-output', 'symlink-public-output', 'linked-output', 'hidden-output', 'incomplete-output'))
    checks += 1
    print(f'Context preparation checks passed: {checks}; synthetic inputs only, not ELF runtime proof.')
