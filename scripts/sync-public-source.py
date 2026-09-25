#!/usr/bin/env python3
"""Append an exact source tree to public main without exporting private history.

Only committed objects are published. Divergent public/source history fails closed;
no branch deletion, force push, tags, worktree files, or credential copying occurs.
"""
import argparse
import fcntl
import os
from pathlib import Path
import re
import subprocess
import tempfile


SHA = re.compile(r'[0-9a-f]{40}')
MARKER = re.compile(r'^Source-commit: ([0-9a-f]{40})$', re.MULTILINE)


def git(repo, *args, input=None, env=None, check=True):
    result = subprocess.run(['git', '-C', str(repo), '-c', 'core.hooksPath=/dev/null', *args],
                            input=input, text=True, capture_output=True, env=env)
    if check and result.returncode:
        # Do not echo a remote URL/credential-bearing command or raw Git errors.
        raise RuntimeError(f'Git {args[0]} failed (exit {result.returncode}); source sync stopped')
    return result


def remote_head(repo, remote, env):
    output = git(repo, 'ls-remote', '--heads', remote, 'refs/heads/main', env=env).stdout.strip()
    if not output:
        return None
    entries = [line.split() for line in output.splitlines()]
    if len(entries) != 1 or len(entries[0]) != 2 or not SHA.fullmatch(entries[0][0]):
        raise RuntimeError('Unexpected public main response')
    return entries[0][0]


def sync(source, revision, remote):
    source = Path(source).resolve(strict=True)
    if not SHA.fullmatch(revision):
        raise ValueError('An exact full source commit SHA is required')
    git(source, 'cat-file', '-e', revision + '^{commit}')
    env = os.environ.copy()
    # Git hooks export repository-local variables; never leak them into the
    # isolated public repository (especially GIT_DIR and GIT_INDEX_FILE).
    for name in git(source, 'rev-parse', '--local-env-vars').stdout.splitlines():
        env.pop(name, None)
    env['GIT_TERMINAL_PROMPT'] = '0'
    ssh = git(source, 'config', '--get', 'fedkr.publicSshCommand', check=False).stdout.strip()
    if ssh:
        env['GIT_SSH_COMMAND'] = ssh
    common = Path(git(source, 'rev-parse', '--git-common-dir').stdout.strip())
    if not common.is_absolute():
        common = source / common
    with (common / 'fedkr-public-sync.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with tempfile.TemporaryDirectory(prefix='fedkr-public-source-') as directory:
            public = Path(directory)
            git(public, 'init', '--bare', env=env)
            git(public, 'fetch', '--no-tags', str(source), revision, env=env)
            tree = git(public, 'rev-parse', revision + '^{tree}', env=env).stdout.strip()
            previous = remote_head(public, remote, env)
            parents = []
            if previous:
                git(public, 'fetch', '--no-tags', remote, 'refs/heads/main', env=env)
                fetched = git(public, 'rev-parse', 'FETCH_HEAD', env=env).stdout.strip()
                if fetched != previous:
                    raise RuntimeError('Public main moved during synchronization; retry')
                message = git(public, 'log', '-1', '--format=%B', previous, env=env).stdout
                markers = MARKER.findall(message)
                if len(markers) != 1:
                    raise RuntimeError('Public main has independent changes; integrate them explicitly first')
                prior_source = markers[0]
                prior_tree = git(public, 'rev-parse', previous + '^{tree}', env=env).stdout.strip()
                check = git(public, 'rev-parse', prior_source + '^{tree}', env=env, check=False)
                if check.returncode or check.stdout.strip() != prior_tree:
                    raise RuntimeError('Public tree differs from its source marker; refusing overwrite')
                if prior_source == revision:
                    if tree != prior_tree:
                        raise RuntimeError('Public/source tree mismatch')
                    print(f'Already synchronized: source={revision} public={previous} tree={tree}')
                    return previous
                if git(public, 'merge-base', '--is-ancestor', prior_source, revision,
                       env=env, check=False).returncode:
                    raise RuntimeError('Source is not a descendant of the last published source')
                parents = ['-p', previous]
            # This commit records publication, not original authorship. Keep
            # contribution attribution in the canonical history and notices;
            # never export a private source-tip name/email by accident.
            env['GIT_AUTHOR_NAME'] = env['GIT_COMMITTER_NAME'] = 'fediverse.kr source sync'
            env['GIT_AUTHOR_EMAIL'] = env['GIT_COMMITTER_EMAIL'] = 'source-sync@users.noreply.github.com'
            env['GIT_AUTHOR_DATE'] = git(source, 'log', '-1', '--format=%aI', revision).stdout.strip()
            message = f'Publish source snapshot {revision[:12]}\n\nSource-commit: {revision}\n'
            commit = git(public, 'commit-tree', tree, *parents, input=message, env=env).stdout.strip()
            git(public, 'push', remote, commit + ':refs/heads/main', env=env)
            if remote_head(public, remote, env) != commit:
                raise RuntimeError('Public main read-back did not match the pushed snapshot')
            print(f'Synchronized: source={revision} public={commit} tree={tree}')
            return commit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path.cwd())
    parser.add_argument('--revision', required=True)
    parser.add_argument('--remote', help='Explicit public repository; defaults to local Git configuration')
    args = parser.parse_args()
    remote = args.remote or git(args.source, 'config', '--get', 'fedkr.publicRemote').stdout.strip()
    if not remote:
        parser.error('Configure fedkr.publicRemote before publishing')
    try:
        sync(args.source, args.revision, remote)
    except (ValueError, RuntimeError, OSError) as error:
        parser.exit(1, f'Public source synchronization failed: {error}\n')


if __name__ == '__main__':
    main()
