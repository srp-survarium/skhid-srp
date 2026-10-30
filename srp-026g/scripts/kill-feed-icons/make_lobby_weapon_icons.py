#!/usr/bin/env python3
"""make_lobby_weapon_icons.py: the kill feed's weapon icons for the lobby movie (2026-10-30;
../../docs/notes/kill-feed-icons.md "In the lobby").

usage: python3 -I make_lobby_weapon_icons.py (--hud <hud.swf> | --patches <folder>) --ffdec <ffdec.jar>
           [--java <java>] --hud-bitmaps <folder> --lobby <folder> [--check]

--hud          the hud movie's original (srp-026g/flash/hud/movie.json: resources.db.patch_22, sha256 checked)
--patches      or the folder of the published patches (~/.cache/skhid-release/patches): the original is
               read from the archive and offset the hud's movie.json names
--ffdec        FFDec's jar (private/actionscript/tools.json), which exports the clip's bitmaps as PNG
--java         the java that runs it (tools.json's; default: java on PATH)
--check        write nothing: compare what it would write with the lobby's files (pixels and text),
               exit 1 on any difference (scripts/weight-ui/check_ui.sh runs it)
--hud-bitmaps  srp-026g/flash/hud/bitmaps: CombatLogWeapon53..68.png, the icons past the clip's frames
--lobby        srp-026g/flash/lobby_menu: writes bitmaps/KillFeedWeapon<icon>.png, a class a bitmap
               (src/KillFeedWeapon<icon>.as) and src/survarium/player/KillFeedIcons.as, their index

The same images the kill feed shows: a weapon's icon is its config's `ui_desc/combat_log_icon`, frame
(icon + 1) of the hud's weapon icon clip (sprite 698, object_icon of CombatLogItem: each frame one shape
filled with its icon's bitmap at 1:1), and past its 53 frames the hud's own CombatLogWeapon<icon>
bitmaps. A frame with no shape (icon 17) has no icon. Writes the files anew; a check run compares.

Needs Pillow.
"""
import argparse
import filecmp
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'tools' / 'actionscript'))
import swf  # noqa: E402

HUD_SHA256 = 'b924b29db06070dfe7d89827a3cf3bc71f14ae4cbca9a2546d95ac977951ec80'
CLIP = 698
DEFINE_SPRITE, PLACE_OBJECT2, SHOW_FRAME, DEFINE_SHAPE = 39, 26, 1, 2
MORE = range(53, 69)


class Bits:
    def __init__(self, data, pos):
        self.data, self.bit = data, pos * 8

    def read(self, n, signed=False):
        value = 0
        for _ in range(n):
            value = value << 1 | (self.data[self.bit >> 3] >> (7 - (self.bit & 7)) & 1)
            self.bit += 1
        if signed and n and value >> (n - 1):
            value -= 1 << n
        return value

    def byte(self):
        return (self.bit + 7) >> 3


def rect_end(data, pos):
    bits = Bits(data, pos)
    n = bits.read(5)
    for _ in range(4):
        bits.read(n, True)
    return bits.byte()


def matrix_end(data, pos):
    bits = Bits(data, pos)
    for flag in range(2):
        if bits.read(1):
            n = bits.read(5)
            bits.read(n)
            bits.read(n)
    n = bits.read(5)
    bits.read(n)
    bits.read(n)
    return bits.byte()


def shape_bitmaps(body):
    """The bitmap ids a DefineShape's fill styles use."""
    pos = rect_end(body, 2)
    count = body[pos]
    pos += 1
    ids = []
    for _ in range(count):
        kind = body[pos]
        pos += 1
        if kind == 0x00:
            pos += 3
        elif kind in (0x40, 0x41, 0x42, 0x43):
            ids.append(struct.unpack_from('<H', body, pos)[0])
            pos = matrix_end(body, pos + 2)
        else:
            raise ValueError(f'fill style {kind:#x} not expected in a weapon icon')
    return ids


def clip_frames(tags):
    """{frame (1-based): shape id} of the weapon icon clip."""
    for kind, raw in tags:
        body = swf.payload(raw)
        if kind != DEFINE_SPRITE or struct.unpack_from('<H', body)[0] != CLIP:
            continue
        frames, frame, pos = {}, 1, 4
        while pos < len(body):
            word, = struct.unpack_from('<H', body, pos)
            pos += 2
            kind2, size = word >> 6, word & 63
            if size == 63:
                size, = struct.unpack_from('<I', body, pos)
                pos += 4
            inner = body[pos:pos + size]
            pos += size
            if kind2 == SHOW_FRAME:
                frame += 1
            elif kind2 == PLACE_OBJECT2 and inner[0] & 0x02:
                frames[frame] = struct.unpack_from('<H', inner, 3)[0]
            elif kind2 == 0:
                break
        return frames
    raise SystemExit(f'the hud has no sprite {CLIP}')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ('ffdec', 'hud-bitmaps', 'lobby'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--hud', type=Path)
    parser.add_argument('--patches', type=Path)
    parser.add_argument('--java', default='java')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.hud is not None:
        data = args.hud.read_bytes()
    elif args.patches is not None:
        movie = json.loads((args.hud_bitmaps.parent / 'movie.json').read_text())
        with open(args.patches / movie['archive'], 'rb') as archive:
            archive.seek(movie['archive_offset'])
            data = archive.read(movie['movie_size'])
    else:
        sys.exit('--hud or --patches is needed')
    if args.check:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / 'src' / 'survarium' / 'player').mkdir(parents=True)
            names = write(args, data, out)
            bad = [n for n in names if not same(out / n, args.lobby / n)]
            if bad:
                sys.exit('the lobby\'s kill feed icons differ from the hud\'s: ' + ', '.join(bad) + ' (rerun without --check)')
            print(f'ok: the lobby\'s {len(names)} kill feed icon files are the hud\'s')
        return
    write(args, data, args.lobby)


def same(made, kept):
    """Whether a written file and the lobby's are the same: a PNG by its pixels, the rest by bytes."""
    if not kept.exists():
        return False
    if made.suffix == '.png':
        a, b = Image.open(made).convert('RGBA'), Image.open(kept).convert('RGBA')
        return a.size == b.size and a.tobytes() == b.tobytes()
    return filecmp.cmp(made, kept, shallow=False)


def write(args, data, lobby):
    """Writes the icons, their classes and the index into `lobby`; returns their paths under it."""
    if hashlib.sha256(data).hexdigest() != HUD_SHA256:
        sys.exit(f"{args.hud or args.patches}: not the hud movie's original (sha256 {hashlib.sha256(data).hexdigest()})")
    tags = swf.split(data)[1]
    shapes = {struct.unpack_from('<H', swf.payload(raw))[0]: swf.payload(raw) for kind, raw in tags if kind == DEFINE_SHAPE}
    icons = {}
    for frame, shape in sorted(clip_frames(tags).items()):
        fills = [b for b in shape_bitmaps(shapes[shape]) if b != 30]    # 30: the clips' shared edge fill
        if len(fills) != 1:
            sys.exit(f'frame {frame}: shape {shape} fills {fills}, not one icon bitmap')
        icons[frame - 1] = fills[0]
    with tempfile.TemporaryDirectory() as tmp:
        movie = Path(tmp) / 'hud.swf'
        movie.write_bytes(data)
        exports = Path(tmp) / 'images'
        subprocess.run([args.java, '-jar', str(args.ffdec), '-format', 'image:png', '-selectid',
                        ','.join(str(b) for b in icons.values()), '-export', 'image', str(exports), str(movie)],
                       check=True, stdout=subprocess.DEVNULL)
        exported = {int(p.name.split('.')[0].split('_')[0]): p for p in exports.rglob('*.png')}
        bitmaps = lobby / 'bitmaps'
        bitmaps.mkdir(exist_ok=True)
        written = {}
        for icon, bitmap in icons.items():
            Image.open(exported[bitmap]).convert('RGBA').save(bitmaps / f'KillFeedWeapon{icon}.png', optimize=True)
            written[icon] = Image.open(bitmaps / f'KillFeedWeapon{icon}.png').size
    for icon in MORE:
        source = args.hud_bitmaps / f'CombatLogWeapon{icon}.png'
        shutil.copyfile(source, bitmaps / f'KillFeedWeapon{icon}.png')
        written[icon] = Image.open(source).size
    src = lobby / 'src'
    for icon, (w, h) in sorted(written.items()):
        (src / f'KillFeedWeapon{icon}.as').write_text(
            'package\n{\n   import flash.display.BitmapData;\n\n'
            f'   // The kill feed\'s weapon icon {icon} (ui_desc/combat_log_icon), from the hud\'s: made by\n'
            '   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).\n'
            f'   public dynamic class KillFeedWeapon{icon} extends BitmapData\n   {{\n'
            f'      public function KillFeedWeapon{icon}(w:int = {w}, h:int = {h})\n      {{\n'
            '         super(w,h);\n      }\n   }\n}\n', encoding='utf-8')
    last = max(written)
    entries = ','.join(f'KillFeedWeapon{i}' if i in written else 'null' for i in range(last + 1))
    (src / 'survarium' / 'player' / 'KillFeedIcons.as').write_text(
        'package survarium.player\n{\n   import flash.display.BitmapData;\n\n'
        '   // The kill feed\'s weapon icons by ui_desc/combat_log_icon (null where the hud\'s clip has none):\n'
        '   // made by scripts/kill-feed-icons/make_lobby_weapon_icons.py from the hud\'s own images.\n'
        '   public final class KillFeedIcons\n   {\n'
        f'      private static const ICONS:Array = [{entries}];\n'
        '      // A new copy of the icon\'s bitmap, or null when there is none.\n'
        '      public static function bitmap(icon:int) : BitmapData\n      {\n'
        '         var kind:Class = icon >= 0 && icon < ICONS.length ? ICONS[icon] as Class : null;\n'
        '         return kind != null ? new kind() as BitmapData : null;\n      }\n   }\n}\n', encoding='utf-8')
    if not args.check:
        print(f'Wrote {len(written)} icons (0 to {last}, none for {sorted(set(range(last + 1)) - set(written))})')
    return ([f'bitmaps/KillFeedWeapon{i}.png' for i in written] + [f'src/KillFeedWeapon{i}.as' for i in written]
            + ['src/survarium/player/KillFeedIcons.as'])


if __name__ == '__main__':
    main()
