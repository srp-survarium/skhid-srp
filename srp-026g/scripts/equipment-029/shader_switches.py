#!/usr/bin/env python3
"""List the shader switch combinations the released 0.26g game can draw.

The game ships its shaders compiled, one per combination of a material's
`use_*` switches (`shaders/sm_4_0/<pass>/<config>`), and compiles none: a
material whose switches name a combination no shipped material has stops the
game with "cannot find shader". This reads every material instance of the
stock resources.db and of the released patches (each was drawn by players)
through `vostok-port dump-db-config` and writes each combination once: a
stage's effect id and the switches that are on, sorted.

    python shader_switches.py --vostok-port $SKHID_VOSTOK_PORT \
        --db <stock resources.db> --db <resources.db.patch_12> ... > shader_switches.tsv
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import os
import subprocess
import sys

SWITCH = '.effect.use_'


def combinations(leaves):
    """{stage: (effect id, switches on)} of one material's dumped leaves."""
    stages = {}
    for line in leaves.splitlines():
        key, _, value = line.partition('\t')
        parts = key.split('.')
        if len(parts) < 4 or parts[0] != 'material' or parts[2] != 'effect':
            continue
        stage = stages.setdefault(parts[1], [None, set()])
        if parts[3] == 'effect_id':
            stage[0] = value.removeprefix('string:').strip('"')
        elif parts[3].startswith('use_') and parts[-1] == 'value' and value == 'bool:true':
            stage[1].add(parts[3])
    return {name: (effect, frozenset(on)) for name, (effect, on) in stages.items() if effect}


def line(stage, effect, on):
    return '\t'.join([stage, effect, ','.join(sorted(on))])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--vostok-port', required=True)
    parser.add_argument('--db', action='append', required=True)
    args = parser.parse_args()
    def dump(job):
        db, path = job
        dumped = subprocess.run([args.vostok_port, 'dump-db-config', '--leaves', '--db', db, '--path', path],
                                capture_output=True, text=True)
        if dumped.returncode:
            print(f'{db}: {path}: not a config, skipped', file=sys.stderr)
            return {}
        return combinations(dumped.stdout)

    jobs = []
    for db in args.db:
        listed = subprocess.run([args.vostok_port, 'list', '--db', db, '--prefix', 'material_instances/'],
                                check=True, capture_output=True, text=True).stdout.splitlines()
        jobs += [(db, path) for path in listed if path.endswith('.material')]
    seen = set()
    with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        for stages in pool.map(dump, jobs):
            seen.update(line(stage, effect, on) for stage, (effect, on) in stages.items())
    sys.stdout.write('# stage\teffect_id\tswitches on (shader_switches.py)\n')
    sys.stdout.write(''.join(entry + '\n' for entry in sorted(seen)))


if __name__ == '__main__':
    main()
