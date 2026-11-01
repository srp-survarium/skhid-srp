#!/usr/bin/env python3
"""Make content-edit/recoil-sideways-1.json: every weapon's side kick one fixed step a shot.

How the 0.26g client kicks a weapon sideways (weapon_recoil_calculator::on_shot 0x331C50):
each shot draws R uniform in [min_right_recoil, max_right_recoil] and L uniform in
[min_left_recoil, max_left_recoil], both every shot, and moves the side kick x by
(R - L) / 21.48 x (1 - |x|^recoil_power), x clamped to -1..1; the PID (0x3321E0) pulls x back to 0
after the shot interval. There is no left-or-right choice: only R - L counts, so a weapon whose two
ranges are equal and of zero width (every bolt action, shotgun and pistol) has no side kick at all,
and one with equal ranges of some width wanders at random about 0.

Here each weapon that has a side kick gets R - L = STEP, the same every shot: min = max on each side,
STEP = SIDE_SCALE x the weapon's side step today (the root mean square of R - L, so a weapon keeps
its place among the others), its sign the weapon's own bias today (the sign of the mean of R - L)
or, for a weapon without one, the side its family gets in DIRECTION. Both sides sit about the
middle of today's four bounds. The top kick, recoil_power, the shake and the PID gains are not
touched. The lobby card's side-kick rows (props 148-151, copied from these keys by
weapon-details-ui-1) take the new values.

Usage (from the repository root):
  srp-game-data get <the layers> --out <folder> --paths <the wpn_*.options paths>
  python3 -I srp-026g/scripts/recoil-sideways/make_recoil_sideways.py <folder> [--table]
writes srp-026g/scripts/content-edit/recoil-sideways-1.json (and prints the per-weapon table with
--table). The folder holds the weapon configs as the manifest's rows before this edit leave them.
"""
import glob
import json
import math
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'content-edit'))
import content_edit as ce  # noqa: E402

# The one number to tune: the fixed step as a share of the weapon's random step today.
SIDE_SCALE = 0.55
# The side of a weapon family whose kick today has no bias (mean R - L = 0): +1 right, -1 left.
# A family not named here alternates by name (sorted), so neighbours differ.
DIRECTION = {}

KEYS = ('min_left_recoil', 'max_left_recoil', 'min_right_recoil', 'max_right_recoil')
PROPS = dict(zip(KEYS, (148, 149, 150, 151)))
SIDE_DEG_PER_UNIT = 21.48
OUT = os.path.join(HERE, '..', 'content-edit', 'recoil-sideways-1.json')


def family(gun):
    for suffix in ('_premium_2', '_premium', '_special', '_camo'):
        if gun.endswith(suffix):
            return gun[:-len(suffix)]
    return gun


def f32(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def card_text(v):
    """As content_edit.filled writes a number on the card."""
    v = f32(v)
    return str(int(v)) if v == int(v) else '%g' % v


def read(folder):
    guns = {}
    for path in sorted(glob.glob(os.path.join(folder, 'gameplay/items/weapons/wpn_*.options'))):
        buf = open(path, 'rb').read()
        values = {p: (t, d) for p, t, d in ce.walk(buf)}
        if not all('/recoil/' + k in values for k in KEYS):
            continue
        kick = {k: values['/recoil/' + k] for k in KEYS}
        props = {}
        for prop_id in PROPS.values():
            for p, (t, d) in values.items():
                if p.startswith('/ui_desc/props_list[') and p.endswith('/prop_id') and \
                        t == ce.T_INT and ce.as_number(t, d & 0xffffffff) == prop_id:
                    text = values[p[:-len('prop_id')] + 'prop_value'][1]
                    props[prop_id] = text.decode() if isinstance(text, bytes) else text
        game_path = os.path.relpath(path, folder).replace(os.sep, '/')
        guns[os.path.basename(path)[4:-len('.options')]] = dict(path=game_path, kick=kick, props=props)
    return guns


def plan(guns):
    """Each weapon's new bounds; None for one without a side kick (left as it is)."""
    out = {}
    unbiased = sorted({family(g) for g, x in guns.items() if x['mean'] == 0 and x['rms'] > 0} - set(DIRECTION))
    sides = dict(DIRECTION, **{f: (1 if i % 2 == 0 else -1) for i, f in enumerate(unbiased)})
    for gun, x in guns.items():
        if x['rms'] == 0:
            out[gun] = None
            continue
        sign = (1 if x['mean'] > 0 else -1) if x['mean'] != 0 else sides[family(gun)]
        step = round(SIDE_SCALE * x['rms'], 3)
        middle = round(sum(x['now']) / 4, 2)
        left = round(middle - step / 2 * sign, 3)
        right = round(left + step * sign, 3)
        out[gun] = dict(sign=sign, step=step, values=(left, left, right, right))
    return out


def main():
    folder = sys.argv[1]
    guns = read(folder)
    for x in guns.values():
        a, b, c, d = (ce.as_number(t, v & 0xffffffff) for t, v in (x['kick'][k] for k in KEYS))
        x['now'] = (a, b, c, d)
        x['mean'] = round((c + d) / 2 - (a + b) / 2, 6)
        x['rms'] = math.sqrt(x['mean'] ** 2 + ((b - a) ** 2 + (d - c) ** 2) / 12)
    new = plan(guns)
    sets, props, floats, rows = [], [], [], []
    for gun in sorted(guns):
        x, n = guns[gun], new[gun]
        if n is None:
            rows.append((gun, x['now'], None, None, 0.0, 0.0))
            continue
        for key, now, to in zip(KEYS, x['now'], n['values']):
            t = x['kick'][key][0]
            if f32(to) == f32(now):
                continue
            e = {'file': x['path'], 'path': 'recoil/' + key, 'from': float(card_text(now)), 'to': to}
            if t == ce.T_INT:
                e['from'] = int(now)
                if to != int(to):
                    e['as_float'] = True
                    floats.append('%s %s' % (gun, key))
            sets.append(e)
            prop_id = PROPS[key]
            if prop_id in x['props'] and x['props'][prop_id] != card_text(to):
                props.append({'file': x['path'], 'prop_id': prop_id, 'from': x['props'][prop_id],
                              'to': card_text(to)})
        rows.append((gun, x['now'], n['values'], n['sign'], x['rms'], n['step']))
    about = (
        "Side kick one fixed step a shot (the owner, 2026-11-01: \"sideways should still exist, but be "
        "consistent\"; generated by srp-026g/scripts/recoil-sideways/make_recoil_sideways.py, SIDE_SCALE "
        "%g). The client moves the side kick by R - L a shot, R and L drawn uniform in the right and left "
        "ranges every shot (weapon_recoil_calculator::on_shot 0x331C50), so only their difference counts. "
        "Each weapon that has a side kick gets min = max on both sides with R - L = SIDE_SCALE x the root "
        "mean square of R - L today, signed by its bias today (the mean of R - L) or, without one, by its "
        "family's side in the script; both sides about the middle of today's bounds. The %d weapons whose "
        "ranges are equal and fixed (no side kick today) are left alone. Top kick, recoil_power, shake and "
        "the PID gains unchanged. The card's side-kick rows (props 148-151 of weapon-details-ui-1) follow. "
        "An int bound that becomes a fraction is written as a float (as_float). Changes the simulation."
        % (SIDE_SCALE, sum(1 for v in new.values() if v is None)))
    edit = {'_about': about, 'set': sets, 'set_prop': props}
    with open(OUT, 'w', encoding='utf-8') as f:
        json.dump(edit, f, ensure_ascii=False, indent=1)
        f.write('\n')
    print('%s: %d values, %d card texts, %d weapons changed, %d left alone; as_float: %s' % (
        os.path.relpath(OUT), len(sets), len(props), sum(1 for v in new.values() if v),
        sum(1 for v in new.values() if v is None), ', '.join(floats) or 'none'), file=sys.stderr)
    if '--table' in sys.argv:
        print('| weapon | left now | right now | R - L now (mean, rms) | left new | right new | step R - L | side |')
        print('|---|---|---|---|---|---|---|---|')
        for gun, now, to, sign, rms, step in rows:
            a, b, c, d = now
            mean = (c + d) / 2 - (a + b) / 2
            if to is None:
                print('| %s | %s-%s | %s-%s | 0 | - | - | 0 | none |' % (
                    gun, card_text(a), card_text(b), card_text(c), card_text(d)))
                continue
            print('| %s | %s-%s | %s-%s | %+.3f, %.3f | %s | %s | %+.3f | %s |' % (
                gun, card_text(a), card_text(b), card_text(c), card_text(d), mean, rms,
                card_text(to[0]), card_text(to[2]), sign * step, 'right' if sign > 0 else 'left'))


if __name__ == '__main__':
    main()
