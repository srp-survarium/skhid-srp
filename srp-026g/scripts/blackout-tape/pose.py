#!/usr/bin/env python3
"""Pose skinned models in the review poses with the porter's pose-blackout-bands ($BANDS_POSER):
copies of their converted_models (and joints.tsv) under $BANDS_WORK/posed/<pose>/.

usage: pose.py [--table bands.tsv] [--model BASE_MODEL ...] [--jobs N]

The poses: a third-person hold is the AK's full-body clip with the weapon's additive
hands_only clip layered on, as the game mixes them.
"""
import argparse
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bandsenv as E

W = 'ak/player/stand/3rd_view'
POSES = {
    'rifle_aim': ['--clip', W + '/on_site/aimed_on_site_idle', '--at', '0.5'],
    'pistol': ['--clip', W + '/on_site/on_site_idle', '--at', '0.5',
               '--add', 'tt33/player/stand/3rd_view/on_site/hands_only/idle', '--add-at', '0.5'],
    'shotgun': ['--clip', W + '/on_site/on_site_idle', '--at', '0.5',
                '--add', 'rem870/player/stand/3rd_view/on_site/hands_only/idle', '--add-at', '0.5'],
    'reload': ['--clip', W + '/on_site/on_site_idle', '--at', '0.5',
               '--add', W + '/on_site/hands_only/reload', '--add-at', '0.5'],
    'run': ['--clip', W + '/run/forward', '--at', '0.25'],
    'sprint': ['--clip', W + '/sprint/sprint', '--at', '0.5'],
    'crouch': ['--clip', 'ak/player/crouch/3rd_view/aimed_on_site_idle', '--at', '0.5'],
    'death': ['--clip', W + '/death/on_site_back', '--at', '1.0'],
}


def posed_dir(pose):
    return os.path.join(E.WORK, 'posed', pose)


def table_models(path):
    rows = [l.split('\t') for l in open(path).read().splitlines() if l and not l.startswith('#')]
    return sorted({r[0] for r in rows[1:]})


def pose_one(base, pose):
    out = posed_dir(pose)
    if os.path.exists(os.path.join(out, 'models', base + '.skinned_model', 'joints.tsv')):
        return None
    args = [E.POSER, '--target-db', E.layers()[0]]
    for p in E.layers()[1:]:
        args += ['--patch', p]
    args += ['--torso', base, '--output', out] + POSES[pose]
    r = subprocess.run(args, capture_output=True, text=True)
    return None if r.returncode == 0 else '%s %s: %s' % (base, pose, r.stderr.strip()[-300:])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--table')
    ap.add_argument('--model', action='append', default=[])
    ap.add_argument('--jobs', type=int, default=6)
    a = ap.parse_args()
    if not E.POSER:
        sys.exit('BANDS_POSER is not set')
    models = (table_models(a.table) if a.table else []) + a.model
    jobs = [(m, p) for m in models for p in POSES]
    with ThreadPoolExecutor(a.jobs) as ex:
        for err in ex.map(lambda j: pose_one(*j), jobs):
            if err:
                print('FAILED', err)
    print('posed', len(models), 'models in', len(POSES), 'poses under', os.path.join(E.WORK, 'posed'))


if __name__ == '__main__':
    main()
