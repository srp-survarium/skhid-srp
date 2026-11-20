#!/usr/bin/env python3
"""Check cold-cache ordering and failure propagation in the drift runner."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class Chunks(unittest.TestCase):
    def run_chunks(self, bad=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / 'tools/checks'
            client = root / 'srp-026g/client-patches'
            for folder in (scripts, client, root / 'release', root / 'private/world-corpus'):
                folder.mkdir(parents=True)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            shutil.copy2(Path(__file__).parent / 'checks/worlds.sh', scripts / 'worlds.sh')
            (root / 'release/lib.sh').write_text('''set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
say() { echo "$*"; }
die() { echo "$*" >&2; exit 1; }
''')
            (client / 'check_world_check_026g.py').write_text('''from pathlib import Path
import sys, time
root = Path(sys.argv[1])
chunk = Path(sys.argv[3])
(root / ("started." + chunk.suffix)).touch()
if not (root / "cache").exists():
    # A concurrent cold builder collides here, as the actual shared
    # sysroot .building directory did. A warm builder never writes it.
    (root / "building").mkdir()
    time.sleep(0.15)
    (root / "cache").touch()
    (root / "building").rmdir()
if chunk.read_text() == "fail":
    sys.exit(1)
print("drift: 1 worlds")
print("all cases passed")
''')
            chunks = []
            for i in range(3):
                path = root / f'chunk.{i:03}'
                path.write_text('fail' if i == bad else 'pass')
                chunks.append(str(path))
            result = subprocess.run(['bash', str(scripts / 'worlds.sh'), 'chunks',
                                     str(root), *chunks], capture_output=True, text=True)
            started = len(list(root.glob('started.*')))
            return result, started

    def test_cold_first_chunk_warms_cache_for_other_chunks(self):
        result, started = self.run_chunks()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(started, 3)
        self.assertIn('drift: 3 world(s)', result.stdout)

    def test_first_failure_prevents_parallel_jobs(self):
        result, started = self.run_chunks(bad=0)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(started, 1)

    def test_later_failure_is_reported(self):
        result, started = self.run_chunks(bad=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(started, 3)
        self.assertIn('1 of 3 chunk(s) failed', result.stdout)


if __name__ == '__main__':
    unittest.main()
