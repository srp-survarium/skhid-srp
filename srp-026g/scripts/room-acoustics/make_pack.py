#!/usr/bin/env python3
"""make_pack.py: the content pack of the room acoustics (exe patch 155): 0.69d's room layers of the
guns' shots and of the steps, and the rooms' ambient beds.

usage: python3 -I make_pack.py --donor <folder> --live <folder> --out <packs folder>/room-acoustics-<version>

--donor  0.69d's files at their game paths (`vostok-port extract --db <0.69d resources.00-NN.db split>
         --path <path> --output <folder>/<path>`, with a porter that reads 0.69d's 16-byte configs):
         the twelve room composites below and the 0.69d composites, collections, singles (.high.ogg,
         .high.rms, .raw_options) and sounds/spl presets they reach; each 0.26g gun composite's
         same-named 0.69d composite (`_069` left out of the name); and the nine maps'
         projects/<level>/day_tdm/client_project, for their beds. Every file read is pinned together:
         DONOR_SHA256 is the sha256 of the list of `<path>\t<sha256>` lines, sorted.
--live   players' 0.26g files at their game paths: gameplay/items/weapons/wpn_*.options,
         sounds/composite/**, game_materials/game.materials and material.pairs.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

Needs ffmpeg (with libvorbis and libsoxr) on PATH, for the pitched samples (`nix-shell -p ffmpeg`).

A composite_sound item may carry `sound_environment = "<room>"`: with exe patch 155 it plays only
where the sound's room (the zone of a placed sound's source, the listener's for a hud sound; the maps'
zones are edit list room-acoustics-1.json's, make_edits.py) is that room, as in 0.69d. The pack:

- 0.69d's samples, collections and presets these layers and beds play, at new paths (rooms.py: a
  `rooms_069d` segment after the first; presets `rooms_069d_<name>`), so nothing live is replaced:
  each .sound_collection, .raw_options and .spl read from 0.69d's 16-byte records and written as
  0.26g's 24-byte ones (lobby-bulb/make_pack.py's writer), the filenames and presets renamed, the
  collection items' `localization` (-1, which 0.26g has not) left out; .high.ogg and .high.rms as
  they are (Vorbis 44.1 kHz stereo; the RMS cache's format is 0.26g's), but for a pitched sample.
- the pitch: 0.26g reads no `pitch` key, which most room samples have (1.1 to 1.3). 0.69d plays it
  as a frequency ratio (XAudio2 SetFrequencyRatio: speed and pitch together, the sample shorter), so
  it is baked in: the sample read at its rate x pitch (asetrate), resampled back to its rate
  (soxr), Vorbis again as 0.69d's own (libvorbis, its channels and rate, 112 kb/s nominal, no
  bounds, no tags, bitexact), its .high.rms made anew as music/make_pack.py's (the first channel's
  RMS of each 0.02 s window, 882 samples at 44.1 kHz, summed in floats; the header the float 0.02,
  the length in ms and the window count), and the key dropped. A pitch of 1 is dropped alone, the
  sample as it is. rms_cache gives 0.69d's own .high.rms of each unpitched sample the pack copies
  (checked on every one: the same windows, each value within 2e-4, the length within 5 ms).
- the guns: each fire composite of a live gun (first and third view, every slot) keeps its items and
  takes, appended (`config_append`, so it builds on the composite as players have it), the items of
  the 0.69d room composite its same-named 0.69d composite nests (weapons/environments/weapons_*),
  for its indoor rooms only, flattened (rooms.nest: offsets add, distance windows meet), each tagged
  with its room. The outdoor rooms and the untagged items stay out: today's tails remain the
  outdoor sound, heard in every room as before. mp_153_m has no 0.69d composite: it takes the
  third-view one-shot rooms, as the other pump/semi shotguns' third views.
- the steps: the run and sprint pairs, first and third person (rooms.STEP_ROOTS), take the room
  layers of 0.69d's step/environment_<kind> (their files here): a pair whose sound is a collection
  gets a new composite (rooms.step_composite) of that collection, untagged at 0-2000 m (it plays as
  before), and the room layers, which make_edits.py points the pair at. A pair's composite takes
  them in the edit list, not here: crouch-steps-1.json copies the walking composites for the
  crouch pairs after the packs, and its copies must stay without them (0.69d has no crouch room
  layer; their filenames would take its crouch/ prefix).
- the beds: the single sound each map's environment loops (rooms.bed).
"""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rooms  # noqa: E402

PACK_ID = 'room-acoustics'
VERSION = '1'
DONOR_SHA256 = 'c056d68f6da1ab140d0859b4124cb929ff08fc5d6ea57607e0625eaeecd8cec6'
GUN_ROOTS = ('weapons/environments/weapons_',)
NO_DONOR = {'weapons/mp_153_m': 'weapons/environments/weapons_3rd_view_one_shot'}
# The packs whose composites this one appends to (vostok-port: an edit of a file a pack adds needs it).
REQUIRES = ['weapons-031', 'weapons-046', 'weapons-069']
ALWAYS = 2000
RMS_PERIOD = 0.02
VORBIS_RATE = '112k'


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def gun_composites(live):
    """Each fire composite of the live guns: {name: [(weapon, view, slot)]}."""
    out = collections.defaultdict(list)
    weapons = sorted((live.root / 'gameplay/items/weapons').glob('wpn_*.options'))
    for f in weapons:
        cfg = rooms.plain(live.config(f'gameplay/items/weapons/{f.name}'))
        for state, body in cfg.get('states', {}).items():
            if 'fire' not in state:
                continue
            for view in ('first_view', 'third_view'):
                for slot, (kind, name) in enumerate(body.get('sounds', {}).get(view, [])):
                    if kind == rooms.RES_COMPOSITE:
                        out[name].append((f.stem, view, slot))
    return out


def gun_root(donor, name):
    if name in NO_DONOR:
        return NO_DONOR[name]
    own = name.replace('_069', '')
    found = [i['filename'] for i in rooms.composite_items(donor, own)
             if i.get('resource_type') == rooms.RES_COMPOSITE and i['filename'].startswith(GUN_ROOTS)]
    if len(found) != 1:
        raise SystemExit(f'{own}: {len(found)} room composites in 0.69d, not 1')
    return found[0]


def ffmpeg(args, data):
    run = subprocess.run(['ffmpeg', '-nostdin', '-hide_banner', '-v', 'error', *args], input=data, capture_output=True)
    if run.returncode:
        sys.exit(f"ffmpeg {' '.join(args)}: {run.stderr.decode(errors='replace')}")
    return run.stdout


def vorbis_head(ogg):
    """(channels, rate, nominal bit rate) of an Ogg Vorbis file's identification header."""
    at = ogg.find(b'\x01vorbis')
    _, channels, rate, _, nominal, _ = struct.unpack_from('<IBIiii', ogg, at + 7)
    return channels, rate, nominal


def pitched(ogg, pitch):
    """The sample played at `pitch` x its frequency, at its own rate and channels."""
    channels, rate, nominal = vorbis_head(ogg)
    if nominal != 112000:
        sys.exit(f'a pitched sample of nominal {nominal} b/s, not 0.69d\'s 112000')
    return ffmpeg(['-i', 'pipe:0', '-af', f'asetrate={round(rate * pitch)},aresample={rate}:resampler=soxr',
                   '-ac', str(channels), '-ar', str(rate), '-c:a', 'libvorbis', '-b:a', VORBIS_RATE,
                   '-map_metadata', '-1', '-fflags', '+bitexact', '-flags:a', '+bitexact', '-f', 'ogg', 'pipe:1'], ogg)


def rms_cache(ogg):
    """The .high.rms of an Ogg as music/make_pack.py makes one (the docstring)."""
    _, rate, _ = vorbis_head(ogg)
    pcm = ffmpeg(['-i', 'pipe:0', '-af', 'pan=mono|c0=c0', '-f', 'f32le', 'pipe:1'], ogg)
    samples = struct.unpack(f'<{len(pcm) // 4}f', pcm)
    window = int(rate * RMS_PERIOD)
    f32 = struct.Struct('<f')
    values = []
    for start in range(0, len(samples), window):
        part = samples[start:start + window]
        total = 0.0
        for v in part:
            total = f32.unpack(f32.pack(total + f32.unpack(f32.pack(v * v))[0]))[0]
        values.append(math.sqrt(f32.unpack(f32.pack(total / len(part)))[0]))
    return struct.pack(f'<fII{len(values)}f', RMS_PERIOD, len(samples) * 1000 // rate, len(values), *values)


def rms_close(made, own):
    """The same windows, the length within 5 ms (a decoder's end trim), each value within 2e-4."""
    a, b = struct.unpack_from('<fII', made), struct.unpack_from('<fII', own)
    if a[0] != b[0] or a[2] != b[2] or abs(a[1] - b[1]) > 5:
        return False
    n = a[2]
    return all(abs(x - y) <= 2e-4 for x, y in zip(struct.unpack_from(f'<{n}f', made, 12),
                                                  struct.unpack_from(f'<{n}f', own, 12)))


class Pack:
    def __init__(self, donor):
        self.donor = donor
        self.files = {}
        self.presets = set()
        self.baked = collections.Counter()

    def add(self, path, data):
        if self.files.get(path, data) != data:
            raise SystemExit(f'{path}: two contents')
        self.files[path] = data

    def single(self, name):
        base = f'sounds/single/{name}'
        new = f'sounds/single/{rooms.moved(name)}'
        if new + '.raw_options' in self.files:
            return
        ogg, rms = self.donor.bytes(base + '.high.ogg'), self.donor.bytes(base + '.high.rms')
        tree = self.donor.config(base + '.raw_options')
        table = dict(tree[1])['options']
        pitch = rooms.plain(dict(table[1]).get('pitch', (rooms.T_FLOAT, 0x3f800000)))
        fields = []
        for key, value in table[1]:
            if key == 'pitch':
                continue
            if key == 'spl_preset':
                self.presets.add(value[1])
                value = (rooms.T_STR, rooms.preset(value[1]))
            fields.append((key, value))
        table[1][:] = fields
        if pitch != 1:
            ogg = pitched(ogg, pitch)
            rms = rms_cache(ogg)
            self.baked[round(pitch, 3)] += 1
        elif not rms_close(rms_cache(ogg), rms):
            sys.exit(f'{base}: rms_cache does not give 0.69d\'s own .high.rms')
        self.add(new + '.high.ogg', ogg)
        self.add(new + '.high.rms', rms)
        self.add(new + '.raw_options', rooms.serialize(tree))

    def collection(self, name):
        tree = self.donor.config(f'sounds/collections/{name}.sound_collection')
        body = dict(tree[1])['collection']
        items = dict(body[1])['sound_items']
        for k, item in enumerate(items[1]):
            fields = []
            for key, value in item[1]:
                if key == 'localization':
                    continue
                if key == 'filename':
                    kind = rooms.plain(dict(item[1]).get('resource_type', (rooms.T_INT, rooms.RES_SINGLE)))
                    if kind == rooms.RES_COLLECTION:
                        self.collection(value[1])
                    elif kind == rooms.RES_SINGLE:
                        self.single(value[1])
                    else:
                        raise SystemExit(f'{name}: an item of resource type {kind}')
                    value = (rooms.T_STR, rooms.moved(value[1]))
                fields.append((key, value))
            items[1][k] = (rooms.T_NAMED, fields)
        self.add(f'sounds/collections/{rooms.moved(name)}.sound_collection', rooms.serialize(tree))

    def leaf(self, item):
        if item['resource_type'] == rooms.RES_COLLECTION:
            self.collection(item['_source'])
        else:
            self.single(item['_source'])

    def spl(self):
        for name in sorted(self.presets):
            self.add(f'sounds/spl/{rooms.preset(name)}.spl',
                     rooms.serialize(self.donor.config(f'sounds/spl/{name}.spl')))


def composite_file(items):
    return rooms.serialize(rooms.typed({'composite_sound': {
        'use_master_layer_index': False, 'master_layer_index': 0, 'sound_items': items}}))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--donor', required=True, type=Path)
    ap.add_argument('--live', required=True, type=Path)
    ap.add_argument('--out', required=True, type=Path)
    a = ap.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out}: a new folder')
    donor, live = rooms.Files(a.donor), rooms.Files(a.live)
    pack = Pack(donor)
    appends, per_root = [], {}

    def layers(root):
        if root not in per_root:
            per_root[root] = rooms.room_items(donor, root)
            for item in per_root[root]:
                pack.leaf(item)
        return per_root[root]

    guns = gun_composites(live)
    by_root = collections.defaultdict(list)
    for name in sorted(guns):
        rooms.live_items(live, name)
        root = gun_root(donor, name)
        by_root[root].append(name)
        appends.append((name, [rooms.as_item(i) for i in layers(root)]))
    for root, names in sorted(by_root.items()):
        print(f'guns {root}: {len(names)} composites, {len(per_root[root])} room items each')

    made = {}
    counts = collections.Counter()
    seen = set()
    for kind, sound_type, sound in sorted(rooms.step_pairs(live)):
        items = [rooms.as_item(i) for i in layers(rooms.STEP_ROOTS[kind])]
        if sound_type == rooms.RES_COMPOSITE:
            rooms.live_items(live, sound)
            if sound not in seen:
                seen.add(sound)
                counts[kind, 'composites (their room layers: the edit list)'] += 1
        elif sound_type == rooms.RES_COLLECTION:
            new = rooms.step_composite(kind, sound)
            if new not in made:
                own = {'enabled': True, 'filename': sound, 'max_distance': ALWAYS, 'min_distance': 0,
                       'resource_type': rooms.RES_COLLECTION, 'start_offset': 0, 'start_offset_max': 0}
                made[new] = composite_file([own] + items)
                pack.add(f'sounds/composite/{new}.composite_sound', made[new])
                counts[kind, 'new composites'] += 1
        else:
            raise SystemExit(f'{kind} {sound}: a step sound of resource type {sound_type}')
    for (kind, what), n in sorted(counts.items()):
        print(f'steps {kind} ({rooms.STEP_ROOTS[kind]}, {len(per_root[rooms.STEP_ROOTS[kind]])} room items): '
              f'{n} {what}')

    beds = {}
    for level in rooms.LEVELS:
        envs, _ = rooms.environments(donor, level)
        for _, env, _ in envs:
            found = rooms.bed(donor, env)
            if found and found[0]:
                pack.single(found[0])
                beds[rooms.moved(found[0])] = found
    for path, (_, source, samples) in sorted(beds.items()):
        if len(path) > rooms.BED_PATH_MAX:
            sys.exit(f'{path}: a bed path over {rooms.BED_PATH_MAX} bytes')
        print(f'bed {path} (0.69d {source}{f", the first of {len(samples)}" if len(samples) > 1 else ""})')
    pack.spl()

    digest = donor.digest()
    if DONOR_SHA256 and digest != DONOR_SHA256:
        sys.exit(f'the donor files read: sha256 {digest}, not the pinned {DONOR_SHA256}')
    for path, data in pack.files.items():
        (a.out / 'files' / path).parent.mkdir(parents=True, exist_ok=True)
        (a.out / 'files' / path).write_bytes(data)
    files_digest = sha256(''.join(f'{p}\t{sha256(d)}\n' for p, d in sorted(pack.files.items())).encode())
    ffmpeg_version = subprocess.run(['ffmpeg', '-version'], capture_output=True, text=True).stdout.split('\n')[0]
    print(f'pitch baked: {dict(sorted(pack.baked.items()))} ({ffmpeg_version})')
    operations = [{'op': 'add_file', 'path': p} for p in sorted(pack.files)]
    operations += [{'op': 'config_append', 'path': f'sounds/composite/{name}.composite_sound',
                    'array_path': 'composite_sound.sound_items', 'values': [{'table': i} for i in items]}
                   for name, items in sorted(appends)]
    manifest = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': "0.69d's room acoustics for exe patch 155: each gun's fire composites take 0.69d's indoor "
                       "room layers, tagged sound_environment; new composites of the run and sprint step "
                       "collections with 0.69d's step room layers; the layers' samples and the rooms' ambient "
                       "beds, at rooms_069d paths",
        'requires': REQUIRES,
        'operations': operations,
        'notes': [f'made by srp-026g/scripts/room-acoustics/make_pack.py from 0.69d (donor files sha256 {digest}) '
                  f'and players\' 0.26g files (sha256 {live.digest()}); its files: sha256 {files_digest} '
                  f'(the list of <path>\\t<sha256>, sorted), {sum(pack.baked.values())} samples with their pitch '
                  f'baked in by {ffmpeg_version}'],
    }
    (a.out / 'pack.json').write_text(json.dumps(manifest, indent=2) + '\n')
    size = sum(len(d) for d in pack.files.values())
    kinds = collections.Counter(p.rsplit('.', 1)[-1] for p in pack.files)
    print(f'{len(appends)} composites appended to, {len(made)} new; {len(pack.presets)} presets')
    print(f'{a.out}: {len(pack.files)} files ({dict(sorted(kinds.items()))}), {size} bytes; '
          f'donor sha256 {digest}; pack.json sha256 {sha256((a.out / "pack.json").read_bytes())}')


if __name__ == '__main__':
    main()
