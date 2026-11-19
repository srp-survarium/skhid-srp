#!/usr/bin/env python3
"""layered_porter.py: the resource porter, with one resource read from the candidate patch
falling through to what players already have.

Since a released patch holds a map's files, a later quick build carries only what changed: a
checker that reads a released file from the candidate alone (`dump-db-config --db <candidate>`)
finds nothing. Called as the porter, this runs the real one; when a single-resource read
(`dump-db-config`, `extract`, `stat`) of the candidate says the resource does not exist, it reads
it again from the offered patches, newest first, then the stock database: the file the game
mounts. Every other command, and a read of any other database, is the porter's own.

    LAYERED_PORTER   the real porter
    LAYERED_CANDIDATE   the candidate patch
    LAYERED_BELOW   the archives under it, newest first, ':' between them
"""
import os
import subprocess
import sys

porter, candidate = os.environ['LAYERED_PORTER'], os.environ['LAYERED_CANDIDATE']
below = [p for p in os.environ.get('LAYERED_BELOW', '').split(':') if p]
args = sys.argv[1:]
reads = args[:1] in (['dump-db-config'], ['extract'], ['stat'])
if not reads or '--db' not in args or args[args.index('--db') + 1] != candidate:
    os.execv(porter, [porter, *args])
at = args.index('--db') + 1
for db in [candidate, *below]:
    args[at] = db
    run = subprocess.run([porter, *args], capture_output=True)
    if run.returncode == 0 or b'resource does not exist' not in run.stderr:
        sys.stdout.buffer.write(run.stdout)
        sys.stderr.buffer.write(run.stderr)
        sys.exit(run.returncode)
sys.stderr.buffer.write(run.stderr)
sys.exit(run.returncode)
