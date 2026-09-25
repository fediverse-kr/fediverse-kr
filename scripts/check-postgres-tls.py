#!/usr/bin/env python3
"""Real release + owned TLS PostgreSQL; generated certs and synthetic data only.

Requires unprivileged Linux, PG14 and OpenSSL. No external database, credentials,
DNS changes, system trust edits, public port, or production archive is accepted.
"""
import argparse
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--public-dir', required=True, type=Path)
    args = parser.parse_args()
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise RuntimeError('Use the unprivileged Linux development user.')
    binary, public = args.binary.resolve(strict=True), args.public_dir.resolve(strict=True)
    with binary.open('rb') as source:
        if source.read(4) != b'\x7fELF' or not (public / 'index.html').is_file():
            raise RuntimeError('A processed Linux release with matching public files is required.')
    if any(p.is_symlink() for p in public.rglob('*')):
        raise RuntimeError('Release public links refused.')
    env = {k: v for k, v in os.environ.items() if not k.startswith(('FEDKR_', 'DIOXUS_', 'PG'))}
    env.update(LC_ALL='C.UTF-8', PGCLIENTENCODING='UTF8')
    pg_bin = Path('/usr/lib/postgresql/14/bin')
    cache = Path.home() / '.cache/fediversekr2'
    cache.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='pg-tls-check-', dir=cache))
    pg, certificates = root / 'pg', root / 'certificates'
    processes, checks = [], []
    pg_started = False
    started = time.monotonic()

    def command(argv, *, data=None, timeout=30, environment=None):
        result = subprocess.run([str(a) for a in argv], cwd=root, env=environment or env,
                                input=data, text=True, capture_output=True, timeout=timeout)
        if result.returncode:
            raise RuntimeError('Owned TLS fixture operation failed; raw output withheld.')
        return result.stdout.strip()

    def check(name, condition):
        if not condition:
            raise RuntimeError('PostgreSQL TLS check failed: ' + name)
        checks.append(name)

    def free_port():
        with socket.socket() as reserve:
            reserve.bind(('127.0.0.1', 0))
            return reserve.getsockname()[1]

    pg_port, http_port = free_port(), free_port()

    def sql(query, db='fedkr_dev'):
        if db not in ('postgres', 'fedkr_dev'):
            raise RuntimeError('Unexpected fixture database.')
        return command([pg_bin / 'psql', '-X', '-h', root, '-p', pg_port, '-U', 'fedkr_dev',
                        '-d', db, '-At', '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=sqlstate'], data=query)

    def spawn(extra=None, ca=True, db_url=None):
        runtime = dict(env, FEDKR_DATABASE_URL=db_url or url, FEDKR_PUBLIC_ORIGIN=origin,
                       IP='127.0.0.1', PORT=str(http_port))
        if ca:
            runtime['FEDKR_DATABASE_CA_FILE'] = str(certificates / 'ca.pem')
        runtime.update(extra or {})
        child = subprocess.Popen([str(binary)], cwd=root, env=runtime,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        processes.append(child)
        return child

    def finish(child, code):
        out, err = child.communicate(timeout=35)
        check('process exit', child.returncode == code)
        check('fixed diagnostics hide credentials, paths and keys', not re.search(
            rb'BEGIN .*PRIVATE KEY|postgres(?:ql)?://|pg-tls-check-|certificates/', out + err))

    def ready(child):
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if child.poll() is not None:
                raise RuntimeError('Owned TLS app exited before readiness; raw output withheld.')
            connection = http.client.HTTPConnection('127.0.0.1', http_port, timeout=4)
            try:
                connection.request('GET', '/readyz')
                response = connection.getresponse()
                response.read()
                if response.status == 200:
                    return
            except (OSError, http.client.HTTPException):
                pass
            finally:
                connection.close()
            time.sleep(.15)
        raise RuntimeError('Owned TLS readiness deadline exceeded.')

    try:
        print('Generating private test certificates and owned loopback TLS PG14.', flush=True)
        certificates.mkdir(mode=0o700)
        for name in ('ca', 'untrusted'):
            command(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
                     '-subj', '/CN=Fedkr synthetic ' + name, '-addext', 'basicConstraints=critical,CA:TRUE',
                     '-addext', 'keyUsage=critical,keyCertSign,cRLSign',
                     '-keyout', certificates / (name + '.key'), '-out', certificates / (name + '.pem')])
        command(['openssl', 'req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=127.0.0.1',
                 '-keyout', certificates / 'server.key', '-out', certificates / 'server.csr'])
        (certificates / 'extensions').write_text('subjectAltName=IP:127.0.0.1\n'
            'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\n'
            'extendedKeyUsage=serverAuth\n')
        command(['openssl', 'x509', '-req', '-in', certificates / 'server.csr',
                 '-CA', certificates / 'ca.pem', '-CAkey', certificates / 'ca.key',
                 '-set_serial', '2', '-days', '1', '-extfile', certificates / 'extensions',
                 '-out', certificates / 'server.pem'])
        for key in certificates.glob('*.key'):
            key.chmod(0o600)
        command([pg_bin / 'initdb', '-D', pg, '-U', 'fedkr_dev', '-A', 'trust', '--encoding=UTF8', '--no-locale'])
        (pg / 'postgresql.conf').write_text(
            f"listen_addresses='127.0.0.1'\nport={pg_port}\nunix_socket_directories='{root}'\n"
            f"ssl=on\nssl_cert_file='{certificates / 'server.pem'}'\nssl_key_file='{certificates / 'server.key'}'\n"
            "log_statement='none'\nlog_min_error_statement='panic'\nlog_error_verbosity='terse'\n"
            "log_parameter_max_length=0\nlog_parameter_max_length_on_error=0\n")
        (pg / 'pg_hba.conf').write_text('local all all trust\nhostnossl all all 127.0.0.1/32 reject\n'
                                      'hostssl all all 127.0.0.1/32 trust\n')
        pg_started = True
        command([pg_bin / 'pg_ctl', '-D', pg, '-l', root / 'postgres.log', '-w', 'start'])
        check('owned PG directory', Path(sql('SHOW data_directory', 'postgres')) == pg)
        check('fixture SSL enabled', sql('SHOW ssl', 'postgres') == 'on')
        sql('CREATE DATABASE fedkr_dev', 'postgres')
        url = f'postgresql://fedkr_dev@127.0.0.1:{pg_port}/fedkr_dev'
        origin = f'http://127.0.0.1:{http_port}'
        verified_control = command([pg_bin / 'psql', '-X', '-At', '-v', 'ON_ERROR_STOP=1',
            url + '?sslmode=verify-full', '-c', 'SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()'],
            environment=dict(env, PGSSLROOTCERT=str(certificates / 'ca.pem')))
        check('fixture certificate and hostname pass libpq verify-full', verified_control == 't')

        # Library-owned release assets stay beside the verified executable; do
        # not rebuild, overwrite or copy any user database/media data for a test.
        if binary.parent / 'public' != public:
            raise RuntimeError('TLS check requires public/ beside the supplied release server.')
        print('Checking real application migration, pool and TLS rejection paths.', flush=True)
        server = spawn()
        ready(server)
        check('embedded migrations work through verified TLS', sql('SELECT count(*) FROM __diesel_schema_migrations') == '25')
        check('all live app connections use TLS', sql("SELECT count(*) > 0 AND bool_and(s.ssl) FROM pg_stat_activity a JOIN pg_stat_ssl s USING(pid) WHERE a.datname='fedkr_dev' AND a.backend_type='client backend' AND a.pid<>pg_backend_pid()") == 't')
        identity = sql('SELECT md5(private_key_pem) FROM federation_instance_keys')
        server.send_signal(signal.SIGTERM)
        finish(server, 0)

        (certificates / 'empty.pem').write_bytes(b'')
        (certificates / 'invalid.pem').write_bytes(b'not a certificate')
        (certificates / 'oversized.pem').write_bytes(b'x' * (256 * 1024 + 1))
        (certificates / 'linked.pem').symlink_to(certificates / 'ca.pem')
        (certificates / 'mixed.pem').write_bytes((certificates / 'ca.pem').read_bytes() +
                                                 (certificates / 'server.key').read_bytes())
        (certificates / 'mixed.pem').chmod(0o600)
        for name, ca in [('missing', certificates / 'absent.pem'), ('empty', certificates / 'empty.pem'),
                         ('invalid', certificates / 'invalid.pem'), ('large', certificates / 'oversized.pem'),
                         ('directory', certificates), ('relative', Path('certificates/ca.pem')),
                         ('symlink', certificates / 'linked.pem'), ('key-mixed', certificates / 'mixed.pem'),
                         ('untrusted', certificates / 'untrusted.pem')]:
            finish(spawn({'FEDKR_DATABASE_CA_FILE': str(ca)}), 1)
            check('CA ' + name + ' refused', True)
        finish(spawn(ca=False, db_url=url + '?sslmode=require'), 1)
        check('untrusted private issuer is not accepted by public roots', True)
        wrong_name = url.replace('@127.0.0.1:', '@localhost:') + '?hostaddr=127.0.0.1'
        name_control = command([pg_bin / 'psql', '-X', '-At', '-v', 'ON_ERROR_STOP=1',
            wrong_name + '&sslmode=verify-ca', '-c', 'SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()'],
            environment=dict(env, PGSSLROOTCERT=str(certificates / 'ca.pem')))
        check('wrong-name control reaches same TLS server without DNS ambiguity', name_control == 't')
        finish(spawn(db_url=wrong_name), 1)
        check('trusted CA does not bypass certificate hostname validation', True)

        # Permit only this generated fixture's loopback plaintext as a control.
        # A CA-configured app must still refuse a PostgreSQL server without TLS.
        sql("ALTER SYSTEM SET ssl = 'off'")
        (pg / 'pg_hba.conf').write_text('local all all trust\nhost all all 127.0.0.1/32 trust\n')
        sql('SELECT pg_reload_conf()')
        deadline = time.monotonic() + 5
        while sql('SHOW ssl') != 'off' and time.monotonic() < deadline:
            time.sleep(.1)
        check('fixture TLS disabled for downgrade control', sql('SHOW ssl') == 'off')
        plain = command([pg_bin / 'psql', '-X', '-At', '-v', 'ON_ERROR_STOP=1',
                         url + '?sslmode=disable', '-c', 'SELECT 1'])
        check('fixture plaintext control succeeds', plain == '1')
        finish(spawn(), 1)
        check('explicit CA never downgrades to plaintext', True)
        check('failed starts retain signing identity', sql('SELECT md5(private_key_pem) FROM federation_instance_keys') == identity)
        print(json.dumps(dict(checks=len(checks), seconds=round(time.monotonic() - started, 2),
                              synthetic_only=True, tls_connections_verified=True, production_verified=False)), flush=True)
    finally:
        for child in processes:
            if child.poll() is None:
                child.kill()
            child.communicate()
        if pg_started:
            if Path(sql('SHOW data_directory', 'postgres')) != pg:
                raise RuntimeError('Owned PG cleanup identity mismatch; retained.')
            command([pg_bin / 'pg_ctl', '-D', pg, '-m', 'fast', '-w', 'stop'])
        if root.parent != cache or not root.name.startswith('pg-tls-check-') or root.is_symlink():
            raise RuntimeError('Unexpected fixture cleanup target; retained.')
        shutil.rmtree(root)
        print('Stopped owned TLS processes and removed generated synthetic certificates/PG.', flush=True)


if __name__ == '__main__':
    main()
