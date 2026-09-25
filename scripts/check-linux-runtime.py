#!/usr/bin/env python3
"""Owned Linux PG + real ELF server. Synthetic data only, no production inputs.

The browser's release WASM bundle is portable; the ELF must also have passed
`dx tools assets`. Neither raw cargo output nor development assets are sufficient.
This is not a browser test.
"""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import uuid
from urllib.parse import quote


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--public-dir', required=True)
    parser.add_argument('--unit-tests', action='store_true')
    parser.add_argument('--rust-filter', default='')
    parser.add_argument('--rust-only', action='store_true')
    parser.add_argument('--container-image', help='Optional existing local OCI image ID; no pulls/pushes.')
    args = parser.parse_args()
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise RuntimeError('Run as the unprivileged Linux development user.')
    workspace = Path(__file__).resolve().parents[1]
    binary = Path(args.binary).resolve(strict=True)
    public = Path(args.public_dir).resolve(strict=True)
    if binary.read_bytes()[:4] != b'\x7fELF' or not (public / 'index.html').is_file():
        raise RuntimeError('A Linux ELF server and complete Dioxus public bundle are required.')
    public_files = [p for p in public.rglob('*') if p.is_file() and p.name != 'index.html']
    if not 0 < len(public_files) <= 1024 or not any(p.suffix == '.wasm' for p in public_files):
        raise RuntimeError('Expected a bounded public release bundle including WASM.')
    if any(p.is_symlink() for p in public.rglob('*')):
        raise RuntimeError('Public artifact links are not accepted by this test.')
    pg_bin = Path('/usr/lib/postgresql/14/bin')
    # Refuse to touch an existing listener. pg_ctl must also successfully bind.
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 16439))
    cache = Path.home() / '.cache/fediversekr2'
    cache.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='linux-e2e-', dir=cache))
    pg = root / 'pg'
    processes = []
    pg_started = False
    checks = []
    slow = None
    started = time.monotonic()
    env = {k: v for k, v in os.environ.items() if not k.startswith(('FEDKR_', 'DIOXUS_', 'PG'))}
    env.update(PGCLIENTENCODING='UTF8', LC_ALL='C.UTF-8', COLUMNS='160')

    def check(name, value):
        if not value:
            raise RuntimeError('Linux runtime check failed: ' + name)
        checks.append(name)

    def command(argv, *, data=None, timeout=30, environment=None):
        return subprocess.run([str(v) for v in argv], input=data, text=True,
                              encoding='utf-8', stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              cwd=root, env=environment or env, timeout=timeout)

    def sql(query, db='fedkr_dev'):
        if db not in ('postgres', 'fedkr_dev', 'fedkr_test'):
            raise RuntimeError('Unexpected fixture database.')
        result = command([pg_bin / 'psql', '-X', '-h', '127.0.0.1', '-p', '16439',
                          '-U', 'fedkr_dev', '-d', db, '-At', '-v', 'ON_ERROR_STOP=1',
                          '-v', 'VERBOSITY=sqlstate'], data=query)
        if result.returncode:
            state = re.search(r'(?:ERROR|FATAL):\s+([0-9A-Z]{5})', result.stderr)
            raise RuntimeError('Synthetic SQL failed: ' + (state[1] if state else 'unknown'))
        return result.stdout.strip()

    def spawn(executable, environment):
        child = subprocess.Popen([str(executable)], cwd=root, env=environment,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        processes.append(child)
        return child

    def finish(child, code, message=None):
        out, err = child.communicate(timeout=35)
        check('owned process exit', child.returncode == code)
        check('no secret material in process output', not re.search(
            rb'BEGIN (RSA )?PRIVATE KEY|postgres(?:ql)?://', out + err))
        if message:
            check('expected fail-closed reason', message.encode() in err)

    def fetch(path, token=None, data=None, *, content_type='application/json', with_origin=True):
        connection = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
        try:
            headers = {'Cookie': 'fedkr_session=' + token} if token else {}
            if data is not None:
                headers['Content-Type'] = content_type
                if with_origin:
                    headers['Origin'] = origin
            connection.request('POST' if data is not None else 'GET', path,
                               json.dumps(data).encode() if data is not None else None, headers)
            response = connection.getresponse()
            return response.status, response.read(), dict(response.getheaders())
        finally:
            connection.close()

    def wait_for(test, limit=40):
        deadline = time.monotonic() + limit
        while time.monotonic() < deadline:
            if test():
                return
            time.sleep(.1)
        raise RuntimeError('Synthetic runtime condition exceeded deadline.')

    def ready():
        if server.poll() is not None:
            raise RuntimeError('Owned Linux server stopped before readiness; raw logs withheld.')
        try:
            return fetch('/readyz')[0] == 200
        except (OSError, http.client.HTTPException):
            return False

    def rust_tests():
        print('Running ' + ('filtered' if args.rust_filter else 'full') +
              ' Rust/PG suite against separate synthetic Linux test DB.', flush=True)
        test_env = dict(env, FEDKR_TEST_DATABASE_URL='postgresql://fedkr_dev@127.0.0.1:16439/fedkr_test',
                        CARGO_TARGET_DIR=str(cache / 'target'))
        result = subprocess.run(['cargo', 'test', '--locked', '--offline', '--release',
                                 '--no-default-features', '--features', 'server', '--bin', 'fediversekr2',
                                 '--message-format', 'short', args.rust_filter, '--', '--include-ignored', '--test-threads=1'],
                                cwd=workspace, env=test_env, capture_output=True, text=True, timeout=900)
        summaries = re.findall(r'^test result: .*$', result.stdout, re.M)
        print('\n'.join(summaries) if summaries else 'No Rust suite result.', flush=True)
        if result.returncode:
            failures = re.findall(r'^test [a-zA-Z0-9_:]+ .*FAILED$', result.stdout, re.M)
            print('\n'.join(failures), flush=True)
            locations = re.findall(r'panicked at (src/[a-zA-Z0-9_/.-]+:\d+:\d+):\n([^\n]{0,180})', result.stdout)
            print('Synthetic assertion locations:', locations, flush=True)
            log = (root / 'postgres.log').read_text()
            print('Known SQL errors:', {name: log.count(name) for name in (
                'deadlock detected', 'canceling statement due to lock timeout',
                'canceling statement due to statement timeout')}, flush=True)
            # Only SQL templates/relations from this generated fixture; redact
            # literals and never dump parameter values or the rest of the log.
            templates = re.findall(r'Process \d+: ([^\n]+)', log)
            print('Deadlock SQL templates:', [re.sub(r"'[^']*'", "'[value]'", q)[:700]
                                               for q in templates], flush=True)
            print('Locked relations:', re.findall(r'in relation "([a-z_]+)"', log), flush=True)
            raise RuntimeError('Linux Rust/PG suite failed; raw output withheld.')
        check(('filtered' if args.rust_filter else 'full') + ' Linux Rust/PG suite passes', bool(summaries))

    try:
        print('Preparing private, disposable Linux PG14 cluster.', flush=True)
        result = command([pg_bin / 'initdb', '-D', pg, '-U', 'fedkr_dev', '-A', 'trust',
                          '--no-locale', '--encoding=UTF8'])
        if result.returncode:
            raise RuntimeError('Synthetic initdb failed; raw output withheld.')
        result = command([pg_bin / 'pg_ctl', '-D', pg, '-l', root / 'postgres.log',
                          '-o', f'-h 127.0.0.1 -p 16439 -k {root}', '-w', 'start'])
        if result.returncode:
            raise RuntimeError('Owned test PG could not bind; no other cluster is modified.')
        pg_started = True
        check('only owned PG cluster is connected', Path(sql('SHOW data_directory', 'postgres')) == pg)
        for database in ('fedkr_dev', 'fedkr_test'):
            sql('CREATE DATABASE ' + database, 'postgres')
        if args.rust_only:
            rust_tests()
            return
        deploy = root / 'deploy'
        deploy.mkdir()
        executable = deploy / 'server'
        shutil.copy2(binary, executable)
        shutil.copytree(public, deploy / 'public')
        with socket.socket() as reserve:
            reserve.bind(('127.0.0.1', 0))
            port = reserve.getsockname()[1]
        origin = f'http://127.0.0.1:{port}'
        media = root / 'media'
        (media / 'objects').mkdir(parents=True)
        runtime_env = dict(env, FEDKR_DATABASE_URL='postgresql://fedkr_dev@127.0.0.1:16439/fedkr_dev',
                           FEDKR_PUBLIC_ORIGIN=origin, FEDKR_MEDIA_DIR=str(media), IP='127.0.0.1', PORT=str(port))
        incomplete = root / 'incomplete'
        incomplete.mkdir()
        shutil.copy2(binary, incomplete / 'server')
        finish(spawn(incomplete / 'server', runtime_env), 1, 'Web bundle is missing')
        check('incomplete deployment never migrates DB', sql("SELECT count(*) FROM pg_tables WHERE schemaname='public'") == '0')
        with socket.socket() as occupied:
            occupied.bind(('127.0.0.1', port))
            occupied.listen()
            finish(spawn(executable, runtime_env), 1, 'HTTP listener failed')
        check('bind failure never migrates DB or starts worker', sql("SELECT count(*) FROM pg_tables WHERE schemaname='public'") == '0')
        print('Running Linux ELF HTTP, asset and worker checks.', flush=True)
        server = spawn(executable, runtime_env)
        wait_for(ready, 60)
        check('Linux liveness/readiness', fetch('/healthz')[0] == 200)
        status, landing, _ = fetch('/')
        check('Linux real SSR and hydration entry', status == 200 and b'portal-root' in landing and b'.js' in landing)
        # Verify Linux SSR references, not merely that unrelated static files exist.
        references = re.findall(rb'(?:src|href)="(/assets/[^"<>]+)"', landing)
        check('Linux SSR references the portable assets', len(references) >= 8)
        for reference in set(references):
            check('Linux SSR asset resolves', fetch(reference.decode())[0] == 200)
        for path in public_files:
            route = '/' + path.relative_to(public).as_posix()
            status, body, _ = fetch(route)
            check('portable release file is byte-exact', status == 200 and
                  hashlib.sha256(body).digest() == hashlib.sha256(path.read_bytes()).digest())
        actor = json.loads(fetch('/actor')[1])
        check('persistent AP identity served', actor['id'] == origin + '/actor' and 'PUBLIC KEY' in actor['publicKey']['publicKeyPem'])
        check('Phoenix actor inbox and host advertised', actor['inbox'] == origin + '/actor/inbox'
              and actor['preferredUsername'] == '127.0.0.1'
              and actor['publicKey']['id'] == origin + '/actor#main-key'
              and actor['publicKey']['owner'] == actor['id'])
        receipt = fetch('/actor/inbox', data={'type': 'Delete', 'object': 'https://fixture.example/notes/1'},
                        content_type='application/activity+json', with_origin=False)
        check('Linux anonymous server inbox is empty no-op 202', receipt[0] == 202 and receipt[1] == b''
              and 'no-store' in {k.lower(): v for k, v in receipt[2].items()}.get('cache-control', ''))
        check('Linux inbox GET is not an SSR fallback', fetch('/actor/inbox')[0] == 405)
        check('Linux inbox body limit', fetch('/actor/inbox', data='x' * 65537,
              content_type='application/activity+json', with_origin=False)[0] == 413)
        check('actual DB catalog is not preview', json.loads(fetch('/api/public/catalog')[1])['preview'] is False)
        check('private media denied anonymously', fetch('/api/member/avatar')[0] == 401)
        # The monitor is an administrator-only, read-only view. Seed only
        # synthetic diagnostic text; its public shape must never reveal it.
        monitor_member, monitor_session = str(uuid.uuid4()), str(uuid.uuid4())
        monitor_token = os.urandom(32).hex()
        monitor_hash = hashlib.sha256(monitor_token.encode()).hexdigest()
        monitor_site, monitor_job = str(uuid.uuid4()), str(uuid.uuid4())
        sql(f"INSERT INTO member_users(id,display_name) VALUES('{monitor_member}','Linux worker monitor fixture');"
            f"INSERT INTO member_admin_roles(member_id) VALUES('{monitor_member}');"
            f"INSERT INTO member_sessions(id,member_id,token_hash,expires_at) VALUES('{monitor_session}','{monitor_member}',decode('{monitor_hash}','hex'),now()+interval '1 hour');"
            f"INSERT INTO directory_sites(id,domain) VALUES('{monitor_site}','127.0.0.2');"
            f"INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,last_error) VALUES('{monitor_job}','{monitor_site}',now()-interval '1 minute','dead',3,'SECRET-worker-detail');"
            f"INSERT INTO directory_observations(site_id,is_alive,nodeinfo_error,checked_at) VALUES('{monitor_site}',false,'SECRET-remote-detail',now());")
        check('worker monitor denies anonymous reads',
              fetch('/api/member/moderation/workers')[0] == 401)
        monitor_status, monitor_body, _ = fetch('/api/member/moderation/workers', monitor_token)
        monitor = json.loads(monitor_body)
        check('worker monitor returns an administrator snapshot',
              monitor_status == 200 and monitor['queue']['dead'] >= 1 and
              monitor['job_issues_total'] >= 1 and monitor['site_issues_total'] >= 1)
        check('worker monitor omits raw lease, error, and private fields',
              all(value not in monitor_body.decode() for value in
                  ('SECRET-', 'lease_token', 'last_error', 'token_hash', monitor_member)))
        # Stop and restart only this run's PG data directory. The HTTP process
        # stays up, readiness becomes unavailable, and a new queue result is
        # committed only after the owned database recovers.
        result = command([pg_bin / 'pg_ctl', '-D', pg, '-m', 'fast', '-w', 'stop'])
        if result.returncode:
            raise RuntimeError('Owned PG interruption could not stop its cluster.')
        pg_started = False
        check('DB interruption keeps HTTP liveness', fetch('/healthz')[0] == 200)
        def database_unavailable():
            if server.poll() is not None:
                return False
            try:
                return fetch('/readyz')[0] == 503
            except (OSError, http.client.HTTPException):
                return False
        wait_for(database_unavailable)
        check('DB interruption makes readiness unavailable', True)
        result = command([pg_bin / 'pg_ctl', '-D', pg, '-l', root / 'postgres.log',
                          '-o', f'-h 127.0.0.1 -p 16439 -k {root}', '-w', 'start'])
        if result.returncode:
            raise RuntimeError('Owned PG interruption could not restart its cluster.')
        pg_started = True
        check('DB recovery reconnects only the owned cluster',
              Path(sql('SHOW data_directory', 'postgres')) == pg)
        wait_for(ready, 60)
        recovery_site, recovery_job = str(uuid.uuid4()), str(uuid.uuid4())
        sql(f"INSERT INTO directory_sites(id,domain,is_hidden) VALUES('{recovery_site}','127.0.0.3',true);"
            f"INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,owner_requested) VALUES('{recovery_job}','{recovery_site}',now(),'pending',0,true);")
        wait_for(lambda: sql(f"SELECT EXISTS(SELECT 1 FROM directory_jobs WHERE id='{recovery_job}' AND state='complete' AND attempts=1 AND lease_token IS NULL AND last_error='invalid_domain')") == 't')
        check('DB recovery commits one fresh synthetic queued result', True)
        # A closed synthetic site cannot schedule outbound health requests.
        svg_site = str(uuid.uuid4())
        svg_domain = f'svg-{svg_site}.example.org'
        svg_icon = b"<svg xmlns='http://www.w3.org/2000/svg'><style>rect{fill:red}</style><rect width='1' height='1'/></svg>"
        sql(f"INSERT INTO directory_sites(id,domain,is_closed) VALUES('{svg_site}','{svg_domain}',true);"
            f"INSERT INTO directory_icons(site_id,mime,bytes) VALUES('{svg_site}','image/svg+xml',decode('{svg_icon.hex()}','hex'));")
        status, icon_body, headers = fetch('/api/public/server-icon/' + svg_domain)
        check('Linux seeded SVG cache serves original bytes', status == 200 and icon_body == svg_icon)
        icon_headers = {k.lower(): v for k, v in headers.items()}
        check('Linux SVG cache uses common sandbox policy', icon_headers.get('content-type') == 'image/svg+xml'
              and 'sandbox' in icon_headers.get('content-security-policy', '')
              and "style-src 'unsafe-inline'" in icon_headers.get('content-security-policy', '')
              and icon_headers.get('cross-origin-resource-policy') == 'same-origin'
              and icon_headers.get('cache-control') == 'no-store')
        sql(f"UPDATE directory_sites SET is_hidden=true WHERE id='{svg_site}';")
        check('Linux hidden SVG cache is not exposed', fetch('/api/public/server-icon/' + svg_domain)[0] == 404)
        # Private synthetic member/session only, not a public AP proof shortcut.
        member, session = str(uuid.uuid4()), str(uuid.uuid4())
        token = os.urandom(32).hex()
        token_hash = hashlib.sha256(token.encode()).hexdigest()
        payload = b"<svg xmlns='http://www.w3.org/2000/svg'><circle r='1'/></svg>"
        content_hash = hashlib.sha256(payload).hexdigest()
        object_path = media / 'objects' / content_hash
        object_path.write_bytes(payload)
        key = f'avatars/{member}.svg'
        emoji_key = f'emojis/{member}/fixture.svg'
        emoji_names = ['wave', 'blob-cat.@/한국어', '.', '..', 'a+b', '%2F']
        emoji_map = json.dumps(dict.fromkeys(emoji_names, emoji_key), ensure_ascii=True)
        sql(f"INSERT INTO member_users(id,display_name) VALUES('{member}','Linux cleanup fixture');"
            f"INSERT INTO member_sessions(id,member_id,token_hash,expires_at) VALUES('{session}','{member}',decode('{token_hash}','hex'),now()+interval '1 hour');"
            f"INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,avatar_key,emojis,inserted_at,updated_at) VALUES('{member}','{member}@cleanup.example.org','cleanup.example.org','{key}','{emoji_map}'::jsonb,now(),now());"
            f"INSERT INTO stored_files VALUES('{key}','{content_hash}',{len(payload)}),('{emoji_key}','{content_hash}',{len(payload)});")
        status, body, _ = fetch('/api/member/avatar', token)
        check('Linux OpenDAL reads actual private avatar bytes', status == 200 and body == payload)
        check('Linux private metadata preserves logical shortcodes',
              sorted(json.loads(fetch('/api/member/profile-media', token)[1])['emojis']) == sorted(emoji_names))
        for name in emoji_names:
            status, body, headers = fetch('/api/member/emoji?name=' + quote(name, safe=''), token)
            check('Linux canonical emoji is byte-exact and private', status == 200 and body == payload
                  and 'private' in {k.lower(): v for k, v in headers.items()}.get('cache-control', ''))
        check('Linux old emoji alias works', fetch('/api/member/emoji/wave', token)[0] == 200)
        check('Linux canonical emoji rejects anonymous access', fetch('/api/member/emoji?name=..')[0] == 401)
        for query in ('name=%', 'name=%zz', 'name=%ff', 'name=', 'name=wave&name=wave', 'name=wave&v='):
            check('Linux canonical emoji rejects malformed query', fetch('/api/member/emoji?' + query, token)[0] == 400)
        check('Linux real HTTP withdrawal commits', fetch('/api/member/withdraw', token, {'confirmation': '탈퇴'})[0] == 200)
        check('Linux withdrawal removes private access', fetch('/api/member/avatar', token)[0] == 401)
        check('Linux withdrawal revokes canonical emoji access', fetch('/api/member/emoji?name=..', token)[0] == 401)
        wait_for(lambda: not object_path.exists() and sql('SELECT count(*) FROM media_deletions') == '0')
        check('Linux same-process OpenDAL cleanup removes bytes and acknowledges work', True)
        check('Linux withdrawn member and logical mapping are gone', sql(f"SELECT NOT EXISTS(SELECT 1 FROM member_users WHERE id='{member}') AND NOT EXISTS(SELECT 1 FROM stored_files WHERE object_key='{key}')") == 't')
        site = str(uuid.uuid4())
        sql(f"INSERT INTO directory_sites(id,domain,is_hidden) VALUES('{site}','127.0.0.1',true)")
        wait_for(lambda: sql(f"SELECT EXISTS(SELECT 1 FROM directory_jobs WHERE site_id='{site}' AND state='complete' AND last_error='invalid_domain')") == 't')
        check('same Linux process schedules and completes pre-network rejection', True)
        check('Linux scheduler runs both maintenance tasks', sql('SELECT count(*) FROM maintenance_schedule') == '2')
        sql(f"UPDATE directory_sites SET is_closed=true WHERE id='{site}'")
        # Expect: 100-continue proves the handler is reading this unfinished body.
        slow = socket.create_connection(('127.0.0.1', port), timeout=10)
        slow.sendall((f'POST /api/member/logout HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: 1000\r\nExpect: 100-continue\r\n\r\n').encode())
        interim = slow.recv(4096)
        check('in-flight request entered real body reader', b'100 Continue' in interim)
        slow.sendall(b'{')
        server.send_signal(signal.SIGTERM)
        signaled = time.monotonic()

        def listener_closed():
            try:
                with socket.create_connection(('127.0.0.1', port), timeout=.1):
                    return False
            except OSError:
                return True

        wait_for(listener_closed, 2)
        check('SIGTERM closes listener before slow request drains', server.poll() is None)
        response = b''
        while True:
            chunk = slow.recv(4096)
            if not chunk:
                break
            response += chunk
        slow.close()
        slow = None
        finish(server, 0)
        check('in-flight body timeout returns 408 before orderly exit', b'408 Request Timeout' in response and time.monotonic() - signaled < 10)
        # Model a crash-expired claim, then prove Linux runtime recovery on restart.
        job, lease = str(uuid.uuid4()), str(uuid.uuid4())
        sql(f"INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,lease_token,lease_until,owner_requested) VALUES('{job}','{site}',now()-interval '3 minutes','running',1,'{lease}',now()-interval '1 second',true)")
        profile_admin, profile_batch, profile_account = (str(uuid.uuid4()) for _ in range(3))
        # A persisted interrupted profile job; the invalid identity is rejected
        # before network access, while the real AP resolver/worker path runs.
        sql(f"INSERT INTO member_users(id,display_name) VALUES('{profile_admin}','Linux profile batch fixture');"
            f"INSERT INTO member_admin_roles(member_id) VALUES('{profile_admin}');"
            f"INSERT INTO member_linked_accounts(id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES('{profile_account}','{profile_admin}','https://127.0.0.1/users/fixture','@fixture@127.0.0.1','Synthetic profile','https://127.0.0.1/users/fixture','activitypub_post','https://127.0.0.1','fixture',now());"
            f"INSERT INTO profile_refresh_batches(id,actor_id,state) VALUES('{profile_batch}','{profile_admin}','running');"
            f"INSERT INTO profile_refresh_jobs(batch_id,member_id,state,attempts,lease_token,lease_until) VALUES('{profile_batch}','{profile_admin}','running',1,gen_random_uuid(),now()-interval '1 second');")
        server = spawn(executable, runtime_env)
        wait_for(ready)
        wait_for(lambda: sql(f"SELECT EXISTS(SELECT 1 FROM directory_jobs WHERE id='{job}' AND state='complete' AND attempts=2 AND lease_token IS NULL)") == 't')
        check('Linux restart recovers synthetic expired lease', True)
        wait_for(lambda: sql(f"SELECT EXISTS(SELECT 1 FROM profile_refresh_batches b JOIN profile_refresh_jobs j ON j.batch_id=b.id WHERE b.id='{profile_batch}' AND b.state='complete' AND j.state='failed' AND j.attempts=2 AND j.lease_token IS NULL)") == 't')
        check('Linux restart completes expired profile job in same binary', True)
        check('profile refresh never publishes private queue anonymously', fetch('/api/member/moderation/profile-batch')[0] == 401)
        check('Linux restart preserves signing key', json.loads(fetch('/actor')[1])['publicKey'] == actor['publicKey'])
        server.send_signal(signal.SIGTERM)
        finish(server, 0)
        # Backpressure must not leave process termination unbounded. This file
        # exists only in the owned test copy, never in the real release bundle.
        with (deploy / 'public/slow-fixture.bin').open('xb') as fixture:
            fixture.truncate(16 * 1024 * 1024)
        server = spawn(executable, runtime_env)
        wait_for(ready)
        slow = socket.socket()
        slow.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
        slow.settimeout(5)
        slow.connect(('127.0.0.1', port))
        slow.sendall(f'GET /slow-fixture.bin HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n'.encode())
        check('slow reader receives a real response', slow.recv(1) == b'H')
        signaled = time.monotonic()
        server.send_signal(signal.SIGTERM)
        wait_for(listener_closed, 2)
        finish(server, 1, 'HTTP shutdown deadline exceeded')
        check('HTTP backpressure is bounded by the shutdown deadline', 28 <= time.monotonic() - signaled < 35)
        slow.close()
        slow = None
        if args.unit_tests:
            rust_tests()
        if args.container_image:
            sys.dont_write_bytecode = True
            from container_smoke import run as run_container_checks
            print('Running baked OCI image against the same owned synthetic PG/media.', flush=True)
            run_container_checks(args.container_image, binary=binary, env=env, media=media,
                                 public_files=public_files, public=public, sql=sql, check=check)
        print(json.dumps(dict(checks=len(checks), public_files=len(public_files),
                              seconds=round(time.monotonic()-started, 2), synthetic_only=True,
                              browser_hydration=False, remote_ap=False)), flush=True)
    finally:
        if slow:
            slow.close()
        for child in processes:
            if child.poll() is None:
                child.kill()
            child.communicate()
        if pg_started:
            check('cleanup still targets owned cluster', Path(sql('SHOW data_directory', 'postgres')) == pg)
            result = command([pg_bin / 'pg_ctl', '-D', pg, '-m', 'fast', '-w', 'stop'])
            if result.returncode:
                raise RuntimeError('Owned PG did not stop; retained fixture directory for recovery.')
        if root.parent != cache or not root.name.startswith('linux-e2e-') or root.is_symlink():
            raise RuntimeError('Unsafe temporary cleanup refused.')
        shutil.rmtree(root)
        print('Stopped owned Linux processes and removed only generated synthetic cluster/bundle.', flush=True)


if __name__ == '__main__':
    main()
