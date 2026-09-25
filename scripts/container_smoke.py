"""Optional actual OCI checks using check-linux-runtime's owned synthetic PG.

No application/PG data is mounted except that run's disposable media directory.
The tested server/public files are baked into the image, not bind-mounted over it.
"""
import base64
import hashlib
import http.client
import json
import os
import re
import socket
import subprocess
import tempfile
import time
import uuid
from pathlib import Path


def run(image, *, binary, env, media, public_files, public, sql, check):
    if not re.fullmatch(r'(?:sha256:)?[0-9a-f]{64}', image):
        raise RuntimeError('Use an existing local OCI image ID, not a mutable tag or remote reference.')
    binary = Path(binary).resolve(strict=True)
    public = Path(public).resolve(strict=True)
    owned = {}
    label = uuid.uuid4().hex

    def podman(*args, environment=None, timeout=45):
        result = subprocess.run(['podman', '--remote=false', *args], env=environment or env,
                                capture_output=True, text=True, timeout=timeout)
        if result.returncode:
            raise RuntimeError('Owned OCI operation failed; raw output withheld.')
        return result.stdout.strip()

    def podman_logs(name):
        # Application diagnostics are only inspected for the fixed fail-closed
        # reason below.  Do not expose either stream to a checker caller.
        result = subprocess.run(['podman', '--remote=false', 'logs', name], env=env,
                                capture_output=True, text=True, timeout=45)
        if result.returncode:
            raise RuntimeError('Owned OCI operation failed; raw output withheld.')
        return result.stdout + result.stderr

    def state(name):
        info = json.loads(podman('inspect', name))[0]
        if info.get('Config', {}).get('Labels', {}).get('io.fedkr.synthetic-run') != label:
            raise RuntimeError('OCI ownership label changed; automatic removal refused.')
        return info

    metadata = json.loads(podman('image', 'inspect', image))[0]
    config = metadata['Config']
    check('OCI image default is nonroot with one executable entrypoint',
          config['User'] == '65532:65532' and config['Entrypoint'] == ['/app/server'])
    check('OCI configured-service defaults cannot silently enter preview',
          'FEDKR_DATABASE_URL=' in config['Env'] and 'FEDKR_PUBLIC_ORIGIN=' in config['Env'])

    def start(settings, with_media=False):
        name = 'fedkr-oci-' + uuid.uuid4().hex
        command = ['run', '-d', '--name', name, '--label', 'io.fedkr.synthetic-run=' + label,
                   '--pull', 'never', '--network', 'host' if with_media else 'none',
                   '--read-only', '--read-only-tmpfs=false', '--cap-drop', 'ALL',
                   '--security-opt', 'no-new-privileges', '--http-proxy=false', '--cgroups', 'disabled']
        if with_media:
            # Podman 3.4 supports keep-id but not the later uid/gid remapping
            # syntax. This checks a nonroot caller UID; production still uses
            # image UID 65532 with separately provisioned volume permissions.
            command += ['--userns', 'keep-id', '--user', f'{os.getuid()}:{os.getgid()}',
                        '--mount', f'type=bind,source={media},destination=/media,rw']
        for key in settings:
            command += ['--env', key]
        owned[name] = False
        podman(*command, image, environment=dict(env, **settings))
        state(name)
        owned[name] = True
        return name

    def baked_file(container, source, destination):
        # Copy from an exact generated, label-checked container.  The image
        # contents are never overmounted and no foreign container is removed.
        state(container)
        podman('cp', f'{container}:{source}', destination)
        return Path(destination)

    def sha256_file(path):
        digest = hashlib.sha256()
        with Path(path).open('rb') as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b''):
                digest.update(chunk)
        return digest.digest()

    def wait_for(predicate, timeout=60):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if predicate():
                return
            time.sleep(.2)
        raise RuntimeError('Owned OCI condition exceeded deadline.')

    with socket.socket() as reserve:
        reserve.bind(('127.0.0.1', 0))
        port = reserve.getsockname()[1]
    origin = f'http://127.0.0.1:{port}'

    def fetch(path, token=None, data=None):
        connection = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
        try:
            headers = {'Cookie': 'fedkr_session=' + token} if token else {}
            if data is not None:
                headers.update({'Origin': origin, 'Content-Type': 'application/json'})
            connection.request('POST' if data is not None else 'GET', path,
                               None if data is None else json.dumps(data).encode(), headers)
            reply = connection.getresponse()
            return reply.status, reply.read()
        finally:
            connection.close()

    try:
        missing = start({})
        wait_for(lambda: not state(missing)['State']['Running'], 15)
        check('OCI missing required configuration exits before serving preview',
              state(missing)['State']['ExitCode'] == 1 and
              'Invalid public origin' in podman_logs(missing))
        settings = dict(FEDKR_DATABASE_URL='postgresql://fedkr_dev@127.0.0.1:16439/fedkr_dev',
                        FEDKR_PUBLIC_ORIGIN=origin, FEDKR_MEDIA_DIR='/media', IP='127.0.0.1', PORT=str(port))
        current = start(settings, True)

        def ready():
            if not state(current)['State']['Running']:
                raise RuntimeError('Owned OCI server stopped before readiness; raw logs withheld.')
            try:
                return fetch('/readyz')[0] == 200
            except (OSError, http.client.HTTPException):
                return False

        wait_for(ready)
        check('OCI live/readiness and DB-backed catalog', fetch('/healthz')[0] == 200 and
              json.loads(fetch('/api/public/catalog')[1])['preview'] is False)
        check('OCI root is read-only and actual runtime UID is nonroot',
              state(current)['HostConfig']['ReadonlyRootfs'] and
              state(current)['Config']['User'].split(':')[0] != '0')
        with tempfile.TemporaryDirectory(prefix='fedkr-oci-baked-') as temporary:
            os.chmod(temporary, 0o700)
            copied_server = baked_file(current, '/app/server', Path(temporary) / 'server')
            copied_index = baked_file(current, '/app/public/index.html', Path(temporary) / 'index.html')
            check('OCI baked server matches the tested release',
                  sha256_file(copied_server) == sha256_file(binary))
            check('OCI baked index matches the tested release',
                  sha256_file(copied_index) == sha256_file(public / 'index.html'))
        status, landing = fetch('/')
        check('OCI baked server renders SSR with assets', status == 200 and b'portal-root' in landing)
        for path in public_files:
            status, body = fetch('/' + path.relative_to(public).as_posix())
            check('OCI baked public file matches the tested release', status == 200 and
                  hashlib.sha256(body).digest() == hashlib.sha256(path.read_bytes()).digest())
        references = set(re.findall(rb'(?:src|href)="(/assets/[^"<>]+)"', landing))
        check('OCI SSR references the expected portable assets', len(references) >= 8)
        for reference in references:
            check('OCI SSR asset reference resolves', fetch(reference.decode())[0] == 200)
        before_key = json.loads(fetch('/actor')[1])['publicKey']
        member, session = str(uuid.uuid4()), str(uuid.uuid4())
        name = 'container-' + uuid.uuid4().hex
        software_id = -(int(uuid.uuid4().hex[:12], 16) + 1)
        token = os.urandom(32).hex()
        token_hash = hashlib.sha256(token.encode()).hexdigest()
        sql(f"INSERT INTO member_users(id,display_name) VALUES('{member}','OCI synthetic admin');"
            f"INSERT INTO member_admin_roles(member_id) VALUES('{member}');"
            f"INSERT INTO member_sessions(id,member_id,token_hash,expires_at) VALUES('{session}','{member}',decode('{token_hash}','hex'),now()+interval '1 hour');"
            f"INSERT INTO catalog_software(id,name,display_name,is_featured,display_order,inserted_at,updated_at) VALUES({software_id},'{name}','OCI synthetic software',false,0,now(),now());")
        svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><circle cx='8' cy='8' r='5'/></svg>"
        payload = {'request': dict(name=name, revision=0, note='Synthetic OCI media check',
                                   data=base64.b64encode(svg).decode())}
        status, response = fetch('/api/member/moderation/software/logo', token, payload)
        check('OCI nonroot process uploads through PG and OpenDAL', status == 200)
        revision = json.loads(response)['revision']
        status, body = fetch('/api/public/software-logo/' + name)
        check('OCI uploaded bytes are served without transformation', status == 200 and body == svg)
        object_path = media / 'objects' / hashlib.sha256(svg).hexdigest()
        check('OCI upload persists outside the read-only image', object_path.read_bytes() == svg)
        stopped = time.monotonic()
        podman('stop', '--time', '40', current)
        check('OCI SIGTERM reaches server and drains normally',
              state(current)['State']['ExitCode'] == 0 and time.monotonic() - stopped < 35)
        current = start(settings, True)
        wait_for(ready)
        check('OCI restart retains PG signing identity and external media',
              json.loads(fetch('/actor')[1])['publicKey'] == before_key and
              fetch('/api/public/software-logo/' + name)[1] == svg)
        payload['request'].update(revision=revision, data=None)
        check('OCI media removal commits through the same endpoint',
              fetch('/api/member/moderation/software/logo', token, payload)[0] == 200)
        wait_for(lambda: not object_path.exists() and sql('SELECT count(*) FROM media_deletions') == '0')
        check('OCI same-process cleanup removes retired external bytes',
              fetch('/api/public/software-logo/' + name)[0] == 404)
        podman('stop', '--time', '40', current)
        check('OCI final shutdown exits normally', state(current)['State']['ExitCode'] == 0)
    finally:
        # Explicit labels + exact generated names, never rm/prune unrelated state.
        for name, confirmed in owned.items():
            result = subprocess.run(['podman', '--remote=false', 'container', 'exists', name],
                                    env=env, capture_output=True, timeout=10)
            if result.returncode == 1:
                continue
            if result.returncode != 0:
                raise RuntimeError('Cannot confirm OCI cleanup target; retained for inspection.')
            state(name)
            podman('rm', '--force', name)
