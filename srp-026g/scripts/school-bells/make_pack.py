#!/usr/bin/env python3
"""make_pack.py: the content pack of School's bells' ring (exe patch 81's bells; docs/notes/breakable_lamps.md "Bells").

usage: python3 -I make_pack.py --takes <folder> --stock <folder> --out <packs folder>/school-bell-<version>

--takes  synth.py's takes (<take>.wav each), pinned below by sha256
--stock  the stock 0.26g resources.db's TEMPLATE at its game path (a bullet's metal hit's options)
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the manifest's
         `pack` row (release/content.tsv)

A bell a shot or a blast reaches rings (the owner, 2026-11-27: "should be directional and just once
or every n seconds"): the lamp patch plays one of RINGS collections, sounds/collections/
props/school_bell_srp/ring_<k>, each one take, a point sound at the bell; which one it picks from the
shot (the owner, 2026-11-27: "can be w/some randomness, so everyone hears the same one"), so every
game hears the same take. The takes are TAKES in their order (the owner kept all but 05 and 06). Each
take is its
synth.py wav encoded as 0.26g's own single sounds (Vorbis, 44.1 kHz mono, libvorbis at quality 4, no
tags, bitexact), its .high.rms made as the engine's sound_rms does (scripts/music/make_pack.py's
rms_cache), its .raw_options the stock metal hit's (3D, HDR, the material_hit preset: heard to 35 m).
Needs ffmpeg (with libvorbis and libsoxr) on PATH.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('music_pack', SCRIPTS / 'music/make_pack.py')
music = importlib.util.module_from_spec(spec)
spec.loader.exec_module(music)
parse, serialize = music.parse, music.serialize
T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5

PACK_ID = 'school-bell'
VERSION = '1'
TEMPLATE = 'sounds/single/material/bullet_hit/metal_baloon_hit/metal_baloon_hit_01.raw_options'
TEMPLATE_SHA = '013d13221eb5c2edc7dd8a271227e80a07b59d2fa6bcb055f37ff2c6fc869083'
TAKES_FOLDER = 'props/school_bell_srp/'
COLLECTIONS = 'props/school_bell_srp/ring_'
# The collections the lamp patch names (lamps.rs RING_TAKES): one a take.
RINGS = 10
# each take: its wav's sha256 (synth.py's)
TAKES = {
    '01_dying_ring': '9371a47f0a5c6bceaae0b21c56a02720f3accd7ebc5154c884691a24df75f042',
    '02_rusted_choke': 'b3f24bbdd83e1ce6d4c7a6beb09d67ff00d17131d4f08c06511aa670e3e62e76',
    '03_bad_contacts': 'b0c7b6b653c189c84b5e4305334658f1a94053073ad95ef80190c553d00187eb',
    '04_cracked_dome': 'df6f8bfe5339835fac35446c375b06265763a569406d80e34b8cbc0409c866d6',
    '07_bent_dome': '91a641897aaa07e3184d4eedb0eab23340395f42d4758769433303c120701ecf',
    '08_far_corridor': 'f78052d69d7b6da10047a35aeedc115b6e23514ad87c55ca497cc5170fbd781b',
    '09_cold_start': '13f60823e1b41ec9cd0a554f4ed58950008cd53adc92e1841eebeea359685d3f',
    '10_drowned': '2aa216e963f13485065d9741a08c213fbb705ead242c20aaea3cb99b4e1689d1',
    '11_old_but_working': 'ec3559ffcbf3aaa19aeaab407541afcfb1c025fb21fc65ee2e133536ed115199',
    '12_shot_dent': 'c1c9c09af2bce49808a7155f337020191e1d66266a3e37f54ca14ff96d1e4426',
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read(path, want):
    data = path.read_bytes()
    if want is not None and sha256(data) != want:
        sys.exit(f'{path}: sha256 {sha256(data)}, not the pinned {want}')
    return data


def encode(wav):
    """Vorbis, 44.1 kHz mono, libvorbis quality 4, as 0.26g's own single sounds."""
    return music.ffmpeg(['-v', 'error', '-i', 'pipe:0', '-af', f'aresample={music.RATE}:resampler=soxr', '-ac', '1',
                         '-c:a', 'libvorbis', '-q:a', '4', '-map_metadata', '-1', '-fflags', '+bitexact',
                         '-flags:a', '+bitexact', '-f', 'ogg', 'pipe:1'], wav).stdout


def collection(takes):
    """A random collection of `takes` (their names under sounds/single), never one twice in a row
    (one take: always it)."""
    items = [(T_NAMED, [('filename', (T_STR, TAKES_FOLDER + take)), ('start_offset', (T_INT, 0)),
                        ('start_offset_max', (T_INT, 0))]) for take in takes]
    return serialize((T_NAMED, [('collection', (T_NAMED, [
        ('cyclic_repeat_from_sound', (T_INT, 0)), ('type', (T_STR, 'random')),
        ('dont_repeat_sound_successively', (T_BOOL, 1)), ('sound_items', (T_INDEXED, items))]))]))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--takes', required=True, type=Path)
    ap.add_argument('--stock', required=True, type=Path)
    ap.add_argument('--out', required=True, type=Path)
    a = ap.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out}: a new folder')
    options = read(a.stock / TEMPLATE, TEMPLATE_SHA)
    takes = sorted(TAKES)
    files = {}
    for take in takes:
        ogg = encode(read(a.takes / f'{take}.wav', TAKES.get(take)))
        files[f'sounds/single/{TAKES_FOLDER}{take}.high.ogg'] = ogg
        files[f'sounds/single/{TAKES_FOLDER}{take}.high.rms'] = music.rms_cache(ogg)
        files[f'sounds/single/{TAKES_FOLDER}{take}.raw_options'] = options
    assert len(takes) == RINGS, f'{len(takes)} takes, the lamp patch names {RINGS}'
    for k, take in enumerate(takes):
        files[f'sounds/collections/{COLLECTIONS}{k}.sound_collection'] = collection([take])
    for path, data in files.items():
        (a.out / 'files' / path).parent.mkdir(parents=True, exist_ok=True)
        (a.out / 'files' / path).write_bytes(data)
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': f"School's bells' ring: {len(takes)} takes of a wrecked electric school bell "
                       '(synthesized), each the collection sounds/collections/props/school_bell_srp/ring_<k>, '
                       'for exe patch 81 (which picks one from the shot)',
        'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
        'notes': ['made by srp-026g/scripts/school-bells/make_pack.py from synth.py\'s takes; the stock metal hit\'s '
                  'options (3D, heard to 35 m)'],
    }
    (a.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    size = sum(len(d) for d in files.values())
    print(f"{a.out}: {len(files)} files, {size} bytes; pack.json sha256 {sha256((a.out / 'pack.json').read_bytes())}")
    for take in takes:
        print(f"    '{take}': '{sha256((a.takes / f'{take}.wav').read_bytes())}',")


if __name__ == '__main__':
    main()
