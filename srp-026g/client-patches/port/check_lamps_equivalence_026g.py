#!/usr/bin/env python3
"""Stateful C-to-Rust lamp equivalence gate.

Usage: python port/check_lamps_equivalence_026g.py ORIGINAL OLD_PATCHED NEW_PATCHED

Both executables run native insertion, bullet, history, frame and removal
paths. Compare the renderer/effect call traces, complete native history bytes,
object fields and normalized vtable ownership after each event. Private C and
Rust state layouts and generated entry addresses are not a public contract.
The ordinary single-hook byte-write gate cannot compare that representation
change or the merged ragdoll/lamp detours. Invalid metadata's stronger atomic
binding guarantee is checked separately by check_lamps_026g.py.
"""
import argparse
import json
from pathlib import Path
import random
import struct
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_lamps_026g as C
import hooks_026g as H


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original', type=Path)
    parser.add_argument('old', type=Path)
    parser.add_argument('new', type=Path)
    parser.add_argument('--cases', type=int, default=400)
    parser.add_argument('--seed', type=int, default=764)
    args = parser.parse_args()
    native = H.original_reader(args.original.read_bytes())
    games = [C.Game(p.read_bytes()) for p in (args.old, args.new)]
    checked = 0

    def observations(g):
        visuals = []
        for i in range(g.r(C.PROJECT + 0x1ec)):
            address = C.VISUALS + i * 0x48
            table = g.r(address)
            # A copied visual table preserves RTTI; both replaced slots run
            # through native project loops in these scenarios.
            if table != 0xa029b4:
                assert bytes(g.u.mem_read(table - 4, 4)) == native(0xa029b0, 4)
            visuals.append((table == 0xa029b4, bytes(g.u.mem_read(address + 4, 0x44))))
        lights = []
        for at in range(g.r(C.PROJECT + 0x184), g.r(C.PROJECT + 0x188), 4):
            address = g.r(at)
            table = g.r(address)
            if table != 0xa0adec:
                # RTTI and the eight untouched slots, including load.
                assert bytes(g.u.mem_read(table - 4, 36)) == native(0xa0ade8, 36)
            lights.append((table == 0xa0adec, bytes(g.u.mem_read(address + 4, 0x250))))
        return (sorted(g.active), g.events, g.effects, g.save(), visuals, lights,
                bytes(g.u.mem_read(C.BULLET, 0x100)))

    def same(label):
        nonlocal checked
        old, new = (observations(g) for g in games)
        for name, a, b in zip(('renderer membership', 'renderer calls', 'effect calls', 'history',
                               'visual fields/ownership', 'light fields/ownership', 'bullet'), old, new):
            assert a == b, f'{label}: {name} differs: old={a!r}; new={b!r}'
        checked += 1

    def both(method, *a, **kw):
        for g in games: getattr(g, method)(*a, **kw)
        same(method)

    both('insert')
    fresh = games[0].save()
    padded = bytearray(fresh)
    struct.pack_into('<I', padded, len(padded) - 4, 0xfffffffc)
    both('load', bytes(padded))
    assert games[0].save() == fresh
    both('present')
    for invalid in (fresh[:-1], fresh[:16], fresh[:16] + bytes(12), fresh[:20] + struct.pack('<I', 256) + fresh[24:]):
        errors = []
        for g in games:
            try:
                g.load(invalid)
            except C.UcError as error:
                errors.append(error.errno)
            else:
                raise AssertionError('malformed history accepted')
        assert errors[0] == errors[1]
        same('malformed history leaves state unchanged')
        both('present')
    saved = [fresh]
    rng = random.Random(args.seed)
    for i in range(args.cases):
        for g in games: g.u.mem_write(C.BULLET + 0x80, bytes([int(i % 9 == 0)]))
        direction = (rng.choice((-1., 1.)), 0., 0.)
        start = (rng.uniform(-2, 5), rng.uniform(-.15, .15), rng.uniform(-.15, .15))
        length = rng.uniform(.01, 8)
        hit = (start[0] + direction[0] * length * .2, start[1], start[2]) if i % 5 == 0 else None
        both('shoot', start=start, length=length, direction=direction, hit=hit)
        both('present')
        if i % 7 == 0:
            saved.append(games[0].save())
            both('load', rng.choice(saved))
            both('present')
        if i % 31 == 0:
            both('remove')
            both('insert')
            assert games[0].save() == fresh
    both('remove')
    both('insert')
    for g in games:
        g.run(0x285330, 0x28533a, regs={C.UC_X86_REG_ECX: C.PROJECT})
    same('destructor fallback')
    source = json.loads((C.ROOT / 'scripts/content-edit/breakable-lamps-1.json').read_text())
    for entry in source['add_value']:
        metadata = entry['value']
        for g in games:
            g.map_metadata(metadata)
            g.u.mem_write(C.BULLET + 0x80, bytes([0]))
        both('insert')
        intact = games[0].save()
        fixture = next(iter(metadata['lamps'].values()))
        centre = tuple((fixture['min_' + k] + fixture['max_' + k]) * .5 for k in 'xyz')
        both('shoot', start=(centre[0] - .25, centre[1], centre[2]), length=.5)
        both('present')
        broken = games[0].save()
        both('load', intact)
        both('present')
        both('load', broken)
        both('present')
        both('remove')
    print(f'PASS: {checked} stateful comparisons, {args.cases} seeded shots, all {len(source["add_value"])} map variants; '
          'renderer/effect traces, history, native object fields and vtable lifecycles match C.')


if __name__ == '__main__':
    main()
