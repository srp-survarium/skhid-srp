#!/usr/bin/env python3
"""Exercise corpus replacement with a local recording host; no network."""
from pathlib import Path
import fcntl
import os
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name('world-corpus')
OLD = 'rec026g_20261107-000000_00000001.srprec'
NEW = 'rec026g_20261112-000000_00000002.srprec'


class Refresh(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.active = self.root / 'corpus/live'
        self.archive = self.root / 'history/live'
        self.remote = self.root / 'remote/recordings'
        for folder in (self.active, self.remote):
            folder.mkdir(parents=True)
        (self.active / OLD).write_bytes(b'old recording')
        (self.remote / NEW).write_bytes(b'new recording')
        shutil.copy2(SCRIPT, self.root / 'world-corpus')
        # Same script, transport replaced by a local host. List and tar are
        # separate, so a recording disappearing between them is reproducible.
        (self.root / 'lib.sh').write_text('''set -euo pipefail
die() { echo "$*" >&2; exit 1; }
say() { echo "$*"; }
need_cmds() { :; }
use_target() { :; }
inspect_target() { RUN_VOLUME_DIR=$TEST_REMOTE; }
q() { printf '%q' "$1"; }
on_target() {
    if [[ $1 == *find* ]]; then
        [[ ${TEST_EMPTY:-0} == 1 ]] || echo "$TEST_NAME"
    else
        tar -C "$TEST_REMOTE/recordings" -cf - -- "$TEST_NAME"
        [[ ${TEST_STREAM_FAIL:-0} != 1 ]]
    fi
}
''')

    def run_refresh(self, replace=True, **env):
        args = ['bash', str(self.root / 'world-corpus'), '--out', str(self.active), '--pause', '0']
        if replace:
            args += ['--archive', str(self.archive)]
        return subprocess.run(args, env={**os.environ, 'TEST_REMOTE': str(self.remote.parent),
                                        'TEST_NAME': NEW, **env}, capture_output=True, text=True)

    def assert_original(self):
        self.assertEqual((self.active / OLD).read_bytes(), b'old recording')
        self.assertFalse(self.archive.exists())
        self.assertEqual(list(self.active.parent.glob('live.incoming.*')), [])

    def test_replacement_preserves_old_and_installs_only_new(self):
        result = self.run_refresh()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.archive / OLD).read_bytes(), b'old recording')
        self.assertEqual([p.name for p in self.active.iterdir()], [NEW])
        self.assertEqual((self.active / NEW).read_bytes(), b'new recording')

    def test_additive_refresh_keeps_old(self):
        result = self.run_refresh(replace=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual({p.name for p in self.active.iterdir()}, {OLD, NEW})

    def test_missing_remote_file_does_not_replace_active(self):
        (self.remote / NEW).unlink()
        self.assertNotEqual(self.run_refresh().returncode, 0)
        self.assert_original()

    def test_stream_error_with_files_present_does_not_replace_active(self):
        self.assertNotEqual(self.run_refresh(TEST_STREAM_FAIL='1').returncode, 0)
        self.assert_original()

    def test_empty_remote_does_not_replace_active(self):
        self.assertNotEqual(self.run_refresh(TEST_EMPTY='1').returncode, 0)
        self.assert_original()

    def test_existing_archive_is_not_overwritten(self):
        self.archive.mkdir(parents=True)
        (self.archive / 'kept').write_bytes(b'keep')
        self.assertNotEqual(self.run_refresh().returncode, 0)
        self.assertEqual((self.archive / 'kept').read_bytes(), b'keep')
        self.assertEqual((self.active / OLD).read_bytes(), b'old recording')

    def test_archive_inside_active_is_refused(self):
        self.archive = self.active / 'history'
        self.assertNotEqual(self.run_refresh().returncode, 0)
        self.assert_original()

    def test_concurrent_refresh_is_refused(self):
        with Path(str(self.active) + '.lock').open('w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.assertNotEqual(self.run_refresh().returncode, 0)
        self.assert_original()

    def test_invalid_remote_name_does_not_replace_active(self):
        self.assertNotEqual(self.run_refresh(TEST_NAME='../unexpected').returncode, 0)
        self.assert_original()


if __name__ == '__main__':
    unittest.main()
