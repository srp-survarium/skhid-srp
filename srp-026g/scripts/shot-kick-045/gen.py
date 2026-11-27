#!/usr/bin/env python3
"""gen.py: the content pack shot-kick-045, the item fire clips' bone curves as 0.45a6 has them
(../../docs/notes/shot-kick-045.md).

usage: python3 -I gen.py --live <folder> --donor <folder> --out <packs folder>/shot-kick-045-<version>
       python3 -I gen.py --paths

--live   the live files of CLIPS at their game paths (each clip's .b-spline and .bones_names), as
         the stock db and the manifest's base patches give them (make_pack.sh extracts them).
--donor  0.45a6's files at the same paths.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).
--paths  prints the game paths the two folders need.

A clip of the pack is the live clip's bytes up to its bone tracks (the header, the bone table, the
event headers), then 0.45a6's tracks bone by bone, then the live bytes after them (the event
channels). The tracks are 0.45a6's as they are; where its clip is longer than ours every knot is
times ours / its, so the whole of its motion plays in our clip's length (a pass of the fire clip
is one shot in both builds whatever its frames: note). Length, events and bones stay the live clip's.

Refused, not skipped: a clip whose bones are not the live clip's by name and order, whose pose at
frame 0 is further from the live one than SAME_MM / SAME_DEG (another rig: the curves would move
the gun's parts), or that is shorter than ours. A clip that comes out as the live bytes is not in
the pack (said).
"""
import argparse
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

PACK_ID = 'shot-kick-045'
VERSION = '1'
DESCRIPTION = ("the stock guns' item fire clips with 0.45a6's bone curves (the gun's kick on a shot, what a scope shows of it): "
               'the guns both builds rig alike, their last-round clips too; lengths, events and bones the live clips\'')
# gun (animations/single/weapons/<gun>/item/stand/3rd_view/): its fire clips, states/fire/animations.
CLIPS = {
    'a9_91': ('shoot',), 'a_545': ('shoot',), 'ak_12': ('shoot',), 'ash_12': ('shoot',),
    'kiparis': ('shoot', 'shoot_end'), 'mosin_nagant': ('shoot',), 'pp2000': ('shoot',),
    'rem870': ('shoot',), 'socom16': ('shoot', 'shoot_empty'), 'sr3m': ('shoot',), 'sv_98': ('shoot',),
    'svd': ('shoot', 'shoot_empty'), 'toz122': ('shoot',), 'ump_45': ('shoot',), 'val': ('shoot',),
    'vssk': ('shoot',), 'winchester_1894': ('shoot',),
}
LIVE_EVENT, DONOR_EVENT = ce.CLIP_EVENT, 12   # an event channel's header: 16 bytes in 0.26g, 12 in 0.45a6
SAME_MM, SAME_DEG = 0.1, 0.1                  # frame-0 poses closer than this are one rig


def stems():
    return [f'animations/single/weapons/{gun}/item/stand/3rd_view/{clip}' for gun, clips in CLIPS.items() for clip in clips]


def bone_names(data):
    names = []
    for at in ce.children(data, ce.find(data, 'bones_names')):
        start, _, _, kind, count = ce.rec(data, at)
        if kind != ce.T_STR:
            raise ValueError('a bone name is not a string')
        names.append(bytes(data[start:start + count - 1]).decode())
    return names


def read(data, names, event):
    """(the bytes before the bone tracks, {bone: nine curves of (knot, control) pairs}, the bytes
    after them, the clip's frames). A curve is a clamped cubic B-spline: four knots at 0, the last
    knot the clip's end, the last control unused."""
    bones, events, _ = ce.CLIP_HEAD.unpack_from(data)
    if bones != len(names) or len(set(names)) != bones:
        raise ValueError('the bone names are not the clip\'s')
    at = start = ce.CLIP_HEAD.size + bones * ce.CLIP_BONE + events * event
    tracks = {}
    for name in names:
        curves = []
        for _ in range(9):
            count = struct.unpack_from('<I', data, at)[0]
            curves.append([struct.unpack_from('<ff', data, at + 4 + 8 * i) for i in range(count)])
            at += 4 + 8 * count
        tracks[name] = curves
    knots = [[k for k, _ in curve] for curves in tracks.values() for curve in curves]
    ends = {k[-1] for k in knots}
    if len(ends) != 1 or any(len(k) < 5 or k[:4] != [0.0] * 4 or k != sorted(k) for k in knots):
        raise ValueError('a curve is not clamped at 0 or does not end with the clip')
    return data[:start], tracks, data[at:], ends.pop()


def pose_gap(ours, theirs):
    """How far two bones' poses at frame 0 are apart: mm, degrees, scale. A clamped curve starts
    on its first control."""
    a, b = [curve[0][1] for curve in ours], [curve[0][1] for curve in theirs]
    return (1000 * math.dist(a[:3], b[:3]), math.degrees(max(abs(x - y) for x, y in zip(a[3:6], b[3:6]))),
            max(abs(x - y) for x, y in zip(a[6:], b[6:])))


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def scaled(curve, by, end):
    """The curve with its time axis times `by`, ending on `end` exactly."""
    return [(f32(k * by), c) for k, c in curve[:-1]] + [(end, curve[-1][1])]


def make(live, live_names, donor, donor_names):
    if live_names != donor_names:
        raise ValueError(f'the bones differ: ours {live_names}, 0.45a6\'s {donor_names}')
    head, ours, tail, end = read(live, live_names, LIVE_EVENT)
    _, theirs, _, donor_end = read(donor, donor_names, DONOR_EVENT)
    if donor_end < end:
        raise ValueError(f'0.45a6\'s clip is {donor_end:g} frames, shorter than our {end:g}')
    out, worst = bytearray(head), (0.0, 0.0)
    for name in live_names:
        mm, deg, scale = pose_gap(ours[name], theirs[name])
        if mm > SAME_MM or deg > SAME_DEG or scale > 1e-4:
            raise ValueError(f'another rig: {name} is {mm:.1f} mm, {deg:.1f} degrees from ours at frame 0')
        worst = max(worst[0], mm), max(worst[1], deg)
        for curve in theirs[name]:
            new = curve if donor_end == end else scaled(curve, end / donor_end, end)
            out += struct.pack('<I', len(new)) + b''.join(struct.pack('<ff', k, c) for k, c in new)
    new = bytes(out + tail)
    # Read back: around the tracks the live bytes, the live length, and 0.45a6's controls on its
    # knots times ours / its.
    start, heads, tracks, channels, pads = ce.clip_parse(new)
    live_start, live_heads, live_tracks, live_channels, live_pads = ce.clip_parse(live)
    if (start, heads, channels, pads) != (live_start, live_heads, live_channels, live_pads) \
            or ce.clip_bytes(start, heads, tracks, channels, pads) != new or ce.clip_end(tracks) != ce.clip_end(live_tracks):
        raise ValueError('the clip made does not keep the live header, events and length')
    _, back, _, back_end = read(new, live_names, LIVE_EVENT)
    for name in live_names:
        for got, want in zip(back[name], theirs[name]):
            if back_end != end or [c for _, c in got] != [c for _, c in want] \
                    or any(abs(k - w * end / donor_end) > 1e-5 for (k, _), (w, _) in zip(got, want)):
                raise ValueError(f'{name}: the curve made is not 0.45a6\'s')
    return new, end, donor_end, worst


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--paths', action='store_true')
    parser.add_argument('--live', type=Path)
    parser.add_argument('--donor', type=Path)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    if args.paths:
        print('\n'.join(stem + suffix for stem in stems() for suffix in ('.b-spline', '.bones_names')))
        return
    if not (args.live and args.donor and args.out):
        parser.error('--live, --donor and --out')
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    operations, files = [], {}
    for stem in stems():
        path = stem + '.b-spline'
        live, donor = (args.live / path).read_bytes(), (args.donor / path).read_bytes()
        try:
            new, end, donor_end, (mm, deg) = make(live, bone_names((args.live / (stem + '.bones_names')).read_bytes()),
                                                  donor, bone_names((args.donor / (stem + '.bones_names')).read_bytes()))
        except ValueError as why:
            sys.exit(f'{path}: {why}')
        if new == live:
            print(f'{path}: the live bytes already, not in the pack')
            continue
        timing = f'{end:g} frames' if donor_end == end else f'{donor_end:g} frames scaled to our {end:g}'
        print(f'{path}: {timing}; frame-0 pose within {mm:.3f} mm, {deg:.3f} degrees of ours')
        files[path] = new
        operations.append({'op': 'replace_file', 'path': path, 'base_sha256': hashlib.sha256(live).hexdigest()})
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    pack = {'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
            'description': DESCRIPTION, 'operations': operations}
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = hashlib.sha256((args.out / 'pack.json').read_bytes()).hexdigest()
    print(f'{args.out}: {len(files)} clips, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{DESCRIPTION}')


if __name__ == '__main__':
    main()
