"""rooms.py: what make_pack.py and make_edits.py share (exe patch 155, room acoustics).

The configs of both builds read into one typed tree (lobby-bulb/make_pack.py's: a value is
(type, data); a named table's data a list of (name, value), an indexed one's a list of values, a
string's its text, a vector's its floats, a number's or a boolean's its record's raw 32 bits), which
that script's `serialize` writes as 0.26g's 24-byte records. 0.69d's 16-byte records (resource-porter
src/binary_config.rs, parse_compact): a u64 of data (bits 0..44, an inline scalar sign-extended or
the offset of a table, string or vector), kind (44..48, the same type codes) and count (48..64), then
a u64 whose bits 0..48 are the offset of a named child's key; the root's children start at byte 16.

The new paths: every 0.69d sound this content brings keeps its 0.69d path with `rooms_069d` after
its first segment (`weapons/environments/main_close_indoor` -> `weapons/rooms_069d/environments/
main_close_indoor`), so no live file is replaced (289 of the sample paths are 0.26g's too, other
sounds), and each .spl preset is `rooms_069d_<name>`.
"""
import hashlib
import importlib.util
import math
import re
import struct
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    'config_codec', Path(__file__).resolve().parent.parent / 'lobby-bulb' / 'make_pack.py')
codec = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(codec)
T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = codec.T_BOOL, codec.T_INT, codec.T_FLOAT, codec.T_NAMED, \
    codec.T_INDEXED, codec.T_STR
serialize = codec.serialize

SEGMENT = 'rooms_069d'
SOUND_PATH = re.compile(r'[a-z0-9_]+(/[a-z0-9_]+)*$')
BED_PATH_MAX = 63          # the exe's bed path buffer (64 bytes with its NUL)
MAX_ENVIRONMENTS = 32      # the exe's tables per map
MAX_MESHES = 256
# The nine 0.69d maps whose zones 0.26g takes (projects/<level>/day_tdm: 0.69d's other modes and
# weathers load the same rooms), at the same coordinates as 0.26g's maps of the same level.
LEVELS = ('level_01', 'level_02', 'level_03', 'level_04', 'level_05', 'level_06', 'level_07', 'level_08',
          'level_09')
# The 0.26g step kinds (material.pairs' mtrl_1) and the 0.69d room composite their 0.69d step composites
# nest ({257, step/environment_<kind>} in step/run/<mat>, sprint, run_3rd, sprint_3rd). 0.69d has no
# crouch room layer.
STEP_ROOTS = {
    'actor_local': 'step/environment_run',
    'actor_local_sprint': 'step/environment_sprint',
    'actor': 'step/environment_run_3rd',
    'actor_sprint': 'step/environment_sprint_3rd',
}
RES_SINGLE, RES_COMPOSITE, RES_COLLECTION = 256, 257, 258


def moved(path):
    """A 0.69d sound path at its new place: `rooms_069d` after its first segment."""
    if not SOUND_PATH.match(path):
        raise ValueError(f'{path!r}: not a sound path of lower-case letters, digits and _')
    head, _, rest = path.partition('/')
    return f'{head}/{SEGMENT}/{rest}' if rest else f'{SEGMENT}/{head}'


def preset(name):
    return f'{SEGMENT}_{name}'


def step_composite(kind, collection):
    """The new composite of a step pair whose sound is a collection: the collection and the rooms."""
    return f'step/{SEGMENT}/{STEP_ROOTS[kind].split("_", 1)[1]}/{collection}'


def is_outdoor(room):
    return room.endswith('_outdoor')


def parse(buf):
    """Either build's config as the typed tree."""
    if len(buf) >= 16:
        a = struct.unpack_from('<Q', buf)[0]
        if a & ((1 << 44) - 1) == 16 and (a >> 44) & 0xf in (T_NAMED, T_INDEXED):
            return _compact(buf, 0)
    return codec.parse(buf)


def _compact(buf, off):
    a, b = struct.unpack_from('<QQ', buf, off)
    data, kind, count = a & ((1 << 44) - 1), (a >> 44) & 0xf, a >> 48
    if kind in (T_NAMED, T_INDEXED):
        kids = []
        for k in range(count):
            c = data + 16 * k
            v = _compact(buf, c)
            if kind == T_NAMED:
                name_at = struct.unpack_from('<Q', buf, c + 8)[0] & ((1 << 48) - 1)
                kids.append((bytes(buf[name_at:buf.index(b'\0', name_at)]).decode('cp1251'), v))
            else:
                kids.append(v)
        return (kind, kids)
    if kind == T_STR:
        return (kind, bytes(buf[data:data + count - 1]).decode('cp1251'))
    if kind > T_STR:
        return (kind, list(struct.unpack_from('<%df' % (kind - 4), buf, data)))
    if kind in (T_BOOL, T_INT, T_FLOAT):
        return (kind, data & 0xffffffff)
    raise ValueError(f'config value type {kind} at {off}')


def plain(value):
    """The typed tree as Python values: dicts, lists, ints, floats, bools, strings, vectors as lists."""
    t, v = value
    if t == T_NAMED:
        return {k: plain(x) for k, x in v}
    if t == T_INDEXED:
        return [plain(x) for x in v]
    if t == T_INT:
        return struct.unpack('<i', struct.pack('<I', v))[0]
    if t == T_FLOAT:
        return struct.unpack('<f', struct.pack('<I', v))[0]
    if t == T_BOOL:
        return bool(v)
    return v


def typed(value):
    """A Python value as the typed tree (ints, floats, bools, strings, dicts, lists)."""
    if isinstance(value, bool):
        return (T_BOOL, int(value))
    if isinstance(value, int):
        return (T_INT, value & 0xffffffff)
    if isinstance(value, float):
        return (T_FLOAT, struct.unpack('<I', struct.pack('<f', value))[0])
    if isinstance(value, str):
        return (T_STR, value)
    if isinstance(value, dict):
        return (T_NAMED, [(k, typed(v)) for k, v in value.items()])
    if isinstance(value, list):
        return (T_INDEXED, [typed(v) for v in value])
    raise TypeError(value)


class Files:
    """A folder of one build's files at their game paths; remembers the sha256 of each file read."""

    def __init__(self, root):
        self.root = Path(root)
        self.read_sha = {}

    def has(self, path):
        return (self.root / path).is_file()

    def bytes(self, path):
        f = self.root / path
        if not f.is_file():
            raise SystemExit(f'{self.root}: no {path}')
        data = f.read_bytes()
        self.read_sha[path] = hashlib.sha256(data).hexdigest()
        return data

    def config(self, path):
        return parse(self.bytes(path))

    def digest(self):
        """The sha256 of the list of every file read and its sha256 (a line each, sorted)."""
        text = ''.join(f'{p}\t{h}\n' for p, h in sorted(self.read_sha.items()))
        return hashlib.sha256(text.encode()).hexdigest()


def composite_items(files, name):
    root = plain(files.config(f'sounds/composite/{name}.composite_sound'))
    return root['composite_sound']['sound_items']


def collection_items(files, name):
    root = plain(files.config(f'sounds/collections/{name}.sound_collection'))
    return root['collection']['sound_items']


def nest(outer, inner):
    """An item of a composite played as an item of another (resource-porter weapons_026/reverb.rs
    Play::nest): offsets add, distance windows intersect, enabled ANDed; None where they do not meet."""
    play = {
        'start_offset': outer.get('start_offset', 0) + inner.get('start_offset', 0),
        'start_offset_max': outer.get('start_offset_max', 0) + inner.get('start_offset_max', 0),
        'min_distance': max(outer.get('min_distance', 0), inner.get('min_distance', 0)),
        'max_distance': min(outer['max_distance'], inner['max_distance']),
        'enabled': outer.get('enabled', True) and inner.get('enabled', True),
    }
    return play if play['min_distance'] < play['max_distance'] else None


def leaves(donor, item, room):
    """The singles and collections an item of a 0.69d composite plays, flattened, each tagged with its
    room (0.26g composites nest nothing)."""
    if item.get('sound_environment', room) != room:
        raise SystemExit(f'{item["filename"]}: tagged {item["sound_environment"]} inside {room}')
    kind = item.get('resource_type', RES_SINGLE)
    if kind != RES_COMPOSITE:
        if kind not in (RES_SINGLE, RES_COLLECTION):
            raise SystemExit(f'{item["filename"]}: resource type {kind}')
        return [dict(item, resource_type=kind)]
    out = []
    for inner in composite_items(donor, item['filename']):
        play = nest(item, inner)
        if play is not None:
            out += leaves(donor, dict(inner, **play), room)
    return out


def room_items(donor, root):
    """The flattened, tagged items of a 0.69d room composite's indoor rooms, at their new paths, as
    0.26g composite items. The outdoor rooms and the untagged items are left: 0.26g's own tails stay
    the outdoor sound."""
    out = []
    for item in composite_items(donor, root):
        room = item.get('sound_environment')
        if room is None or is_outdoor(room):
            continue
        for leaf in leaves(donor, item, room):
            out.append({
                'enabled': leaf['enabled'],
                'filename': moved(leaf['filename']),
                'max_distance': leaf['max_distance'],
                'min_distance': leaf.get('min_distance', 0),
                'resource_type': leaf['resource_type'],
                'sound_environment': room,
                'start_offset': leaf.get('start_offset', 0),
                'start_offset_max': leaf.get('start_offset_max', 0),
                '_source': leaf['filename'],
            })
    return out


def rot(a):
    """resource-porter census level.py's rot(): world = local @ rot(a) + position, row vectors."""
    xs, xc = math.sin(a[0]), math.cos(a[0])
    ys, yc = math.sin(a[1]), math.cos(a[1])
    zs, zc = math.sin(a[2]), math.cos(a[2])
    return [[yc * zc, -yc * zs, ys],
            [ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc],
            [-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc]]


def environments(donor, level):
    """A 0.69d map's sound environments: [(object name, its keys, [mesh, ...])], a mesh
    (kind, world centre, rows, half sizes); and the scripted ones' names, left out."""
    project = plain(donor.config(f'projects/{level}/day_tdm/client_project'))
    objects = project['client_objects']
    geometry = {o['guid_path']: o for o in objects if o.get('game_object_type') == 'primitive_geometry'
                and o.get('lib_name') == 'base:sound_environment_geometry'}
    out, scripted = [], []
    for o in objects:
        if o.get('game_object_type') != 'sound_environment':
            continue
        if o['is_scripted']:
            scripted.append(o['name'])
            continue
        if not o['is_enabled']:
            raise SystemExit(f'{level} {o["name"]}: a disabled environment')
        meshes = []
        for link in o.get('primitive_geometries', {}).values():
            if link['__removed']:
                continue
            g = geometry[link['path'][0]]
            if not g['is_enabled'] or g['is_scripted']:
                raise SystemExit(f'{level} {g["name"]}: a disabled or scripted geometry')
            if any(abs(x) > 1e-6 for x in g['rotation']) or any(abs(x - 1) > 1e-6 for x in g['scale']):
                raise SystemExit(f'{level} {g["name"]}: an object rotation or scale (rot and scale 1 only)')
            for m in g['meshes']:
                rows = rot(m['rotation'])
                for i in range(3):
                    for j in range(3):
                        dot = sum(rows[i][k] * rows[j][k] for k in range(3))
                        if abs(dot - (i == j)) > 1e-5:
                            raise SystemExit(f'{level} {g["name"]}: rows not orthonormal')
                centre = [g['position'][k] + m['position'][k] for k in range(3)]
                kind = m['type']
                if kind not in (0, 1, 2):
                    raise SystemExit(f'{level} {g["name"]}: mesh type {kind}')
                meshes.append((kind, centre, rows, [abs(x) for x in m['scale']]))
        out.append((o['name'], o, meshes))
    names = [n for n, _, _ in out]
    if len(set(names)) != len(names):
        raise SystemExit(f'{level}: two environments of one name')
    return out, scripted


def bed(donor, env):
    """An environment's bed as the exe plays it, a looping single sound: (its 0.69d single path or
    None, the 0.69d resource, its samples), or None for an environment without one. A collection
    plays its first sample; a composite (0.69d's Cologne outdoor birds, two random layers of short
    calls) has no single sound that loops, and is left out (None for its single)."""
    path, kind = env.get('ambient_sound', ['', 0])
    if not path or not kind:
        return None
    if kind == RES_SINGLE:
        return path, path, [path]
    if kind == RES_COLLECTION:
        items = collection_items(donor, path)
        if any(i.get('resource_type', RES_SINGLE) != RES_SINGLE for i in items):
            raise SystemExit(f'{path}: a bed collection of other than singles')
        return items[0]['filename'], path, [i['filename'] for i in items]
    if kind == RES_COMPOSITE:
        return None, path, [i['filename'] for i in composite_items(donor, path)]
    raise SystemExit(f'{path}: bed resource type {kind}')


def step_pairs(live):
    """{(kind, resource type, sound)} of the live run and sprint pairs (material.pairs' mtrl_1 the kind)."""
    names = {m['id']: m['name'] for m in plain(live.config('game_materials/game.materials'))['materials']}
    out = set()
    for p in plain(live.config('game_materials/material.pairs'))['pairs']:
        kind = names.get(p['mtrl_1_id'])
        if kind in STEP_ROOTS and p.get('collision_sound'):
            out.add((kind, p['collision_sound_type'], p['collision_sound']))
    return out


def live_items(live, name):
    """A live 0.26g composite's items, checked: it nests none and has no room items yet."""
    items = composite_items(live, name)
    if any(i.get('resource_type') == RES_COMPOSITE for i in items):
        raise SystemExit(f'{name}: a 0.26g composite that nests one')
    if any('sound_environment' in i for i in items):
        raise SystemExit(f'{name}: has room items already')
    return items


def as_item(item):
    """A room item as a 0.26g composite item (its keys sorted, the source path out)."""
    return {k: v for k, v in sorted(item.items()) if not k.startswith('_')}
