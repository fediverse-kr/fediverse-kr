#!/usr/bin/env python3
"""Exercise public snapshot publishing against real, disposable Git repositories."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('sync-public-source.py')


def git(path, *args):
    return subprocess.check_output(['git', '-C', str(path), *args], text=True,
                                   stderr=subprocess.PIPE).strip()


class SourceSyncTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'
        self.public = self.root / 'public.git'
        self.source.mkdir()
        git(self.source, 'init', '-b', 'main')
        git(self.source, 'config', 'user.name', 'Test contributor')
        git(self.source, 'config', 'user.email', 'contributor@example.invalid')
        git(self.root, 'init', '--bare', str(self.public))
        (self.source / 'old-private.txt').write_text('must not be published')
        self.old = self.commit('private historical content')
        (self.source / 'old-private.txt').unlink()
        (self.source / 'README.md').write_text('public source')
        self.current = self.commit('ready for publication')
        (self.source / 'untracked.env').write_text('must not be published')

    def tearDown(self):
        self.temp.cleanup()

    def commit(self, message):
        git(self.source, 'add', '-u')
        git(self.source, 'add', 'README.md' if (self.source / 'README.md').exists() else 'old-private.txt')
        git(self.source, 'commit', '-m', message)
        return git(self.source, 'rev-parse', 'HEAD')

    def sync(self, revision=None):
        return subprocess.run([sys.executable, str(SCRIPT), '--source', str(self.source),
                               '--revision=' + (revision or self.current), '--remote', str(self.public)],
                              text=True, capture_output=True)

    def test_first_snapshot_has_exact_tree_without_source_history(self):
        result = self.sync()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(git(self.public, 'rev-list', '--count', 'main'), '1')
        self.assertEqual(git(self.public, 'rev-parse', 'main^{tree}'),
                         git(self.source, 'rev-parse', 'HEAD^{tree}'))
        objects = git(self.public, 'rev-list', '--objects', '--all')
        self.assertNotIn('old-private.txt', objects)
        self.assertNotIn('untracked.env', objects)
        self.assertIn('Source-commit: ' + self.current, git(self.public, 'log', '-1', '--format=%B', 'main'))


    def test_repeated_publish_is_idempotent(self):
        self.assertEqual(self.sync().returncode, 0)
        first = git(self.public, 'rev-parse', 'main')
        self.assertEqual(self.sync().returncode, 0)
        self.assertEqual(git(self.public, 'rev-parse', 'main'), first)
        self.assertEqual(git(self.public, 'rev-list', '--count', 'main'), '1')

    def test_next_snapshot_appends_without_exporting_source_ancestry(self):
        self.assertEqual(self.sync().returncode, 0)
        first = git(self.public, 'rev-parse', 'main')
        (self.source / 'README.md').write_text('updated public source')
        second_source = self.commit('private internal release message')
        self.assertEqual(self.sync(second_source).returncode, 0)
        self.assertEqual(git(self.public, 'rev-parse', 'main^'), first)
        self.assertEqual(git(self.public, 'rev-list', '--count', 'main'), '2')
        self.assertEqual(git(self.public, 'rev-parse', 'main^{tree}'),
                         git(self.source, 'rev-parse', 'HEAD^{tree}'))
        self.assertNotIn('private internal release message',
                         git(self.public, 'log', '--all', '--format=%B'))
        probe = subprocess.run(['git', '-C', str(self.public), 'cat-file', '-e', self.old],
                               capture_output=True)
        self.assertNotEqual(probe.returncode, 0)

    def test_snapshot_does_not_publish_private_author_email(self):
        self.assertEqual(self.sync().returncode, 0)
        self.assertEqual(git(self.public, 'log', '-1', '--format=%an <%ae>', 'main'),
                         'fediverse.kr source sync <source-sync@users.noreply.github.com>')
        self.assertEqual(git(self.source, 'log', '-1', '--format=%ae'),
                         'contributor@example.invalid')

    def test_source_rollback_is_refused_without_changing_public(self):
        self.assertEqual(self.sync().returncode, 0)
        before = git(self.public, 'rev-parse', 'main')
        result = self.sync(self.old)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue('not a descendant' in result.stderr or
                        'differs from its source marker' in result.stderr, result.stderr)
        self.assertEqual(git(self.public, 'rev-parse', 'main'), before)

    def test_independent_public_commit_is_never_overwritten(self):
        self.assertEqual(self.sync().returncode, 0)
        clone = self.root / 'contributor'
        git(self.root, 'clone', '-b', 'main', str(self.public), str(clone))
        git(clone, 'config', 'user.name', 'Public contributor')
        git(clone, 'config', 'user.email', 'public@example.invalid')
        (clone / 'README.md').write_text('independent public change')
        git(clone, 'commit', '-am', 'public contribution')
        git(clone, 'push', 'origin', 'main')
        before = git(self.public, 'rev-parse', 'main')
        result = self.sync()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('independent changes', result.stderr)
        self.assertEqual(git(self.public, 'rev-parse', 'main'), before)

    def test_forged_source_marker_does_not_authorize_overwrite(self):
        self.assertEqual(self.sync().returncode, 0)
        git(self.public, 'config', 'user.name', 'Public contributor')
        git(self.public, 'config', 'user.email', 'public@example.invalid')
        tree = git(self.source, 'rev-parse', self.old + '^{tree}')
        git(self.public, 'fetch', str(self.source), self.old)
        forged = git(self.public, 'commit-tree', tree, '-p', 'main',
                     '-m', 'Source-commit: ' + self.current)
        git(self.public, 'update-ref', 'refs/heads/main', forged)
        result = self.sync()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('differs from its source marker', result.stderr)
        self.assertEqual(git(self.public, 'rev-parse', 'main'), forged)

    def test_short_or_symbolic_revision_is_refused(self):
        for revision in ('HEAD', self.current[:12], '--all'):
            result = self.sync(revision)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('exact full source commit', result.stderr)
        self.assertEqual(git(self.public, 'for-each-ref', '--format=%(refname)'), '')


if __name__ == '__main__':
    unittest.main()
