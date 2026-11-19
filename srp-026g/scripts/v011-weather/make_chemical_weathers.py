"""make_chemical_weathers.py DAY_2 DAY_4 VOSTOK_WEATHER_PACK: the edit list v011w-chemical-weathers-1.json,
Chemical Plant v0.11's evening and rain, made from its own day.

0.11 shipped Chemical Plant (`level_04`) as day only, in every build (0.11a, b, c, e). Its evening and rain
are a recreation: each is a copy of the day project as players have it (its geometry, objects, lights and
gameplay), with the environment of Vostok v0.11's same weather (pack v011e-radar-weather, 0.11e's own
evening and rain of the same build and art era: sun, ambient, fog, sky, clouds, rain, wind, post), and the
key exe patch 162 (v011w_render) draws a 0.11 weather by. The probes keep the day project's own
`textures.<weather>` cubemaps (native Chemical Plant's evening and rain ones, which the v0.11 port already
named), until probes rendered for the v0.11 layout replace them.

DAY_2 and DAY_4 are `projects/v011e_level_04/day_{2,4}/client_project` as players have them (published
resources.db.patch_62, pinned below); VOSTOK_WEATHER_PACK is the folder of v011e-radar-weather@6 (its
pack.json pinned below). The list goes in release/content.tsv right above modes.tsv; a value an edit list
below it sets in these projects' environment (ported-maps-grain) is read as it was before that list.
Writes the list to srp-026g/scripts/content-edit/ and prints what it took from where.
"""
import hashlib, json, struct, sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
EDITS = REPO / 'srp-026g/scripts/content-edit'
sys.path.insert(0, str(EDITS))
import content_edit as ce  # noqa: E402

DAY_SHA = {'day_2': 'abb6890ae37b2d175c048bf1459b948f267a9f0a6dc01793128b6831b8b1f70f',
           'day_4': 'c5b51d8d20c8a65ef82fa65e0bb73db40dfa9bc8fbab907fb71b7c3654321558'}
PACK_SHA = 'ca3aae5d615398b2e61d66b152ff841ec97130592bb4ce18b41aaa428fd9516a'
NAME = 'v011w-chemical-weathers-1.json'
DAY = 'projects/v011e_level_04/{day}/client_project'
# weather: (project folder, the Vostok project its environment comes from, map weather id, marker)
WEATHERS = {
    'evening': ('v011w_level_04_evn', 'v011w_level_03_evn/evening', 'evn'),
    'rain': ('v011w_level_04_rain', 'v011w_level_03_rain/rain', 'rain'),
}
# exe patch 162's key: the weather's profile, Vostok's own for the same weather (v011w-renderer-1.json)
RENDERER = EDITS / 'v011w-renderer-1.json'


def profile(source, suffix):
    """The srp_renderer_v011w table v011w-renderer-1.json gives Vostok's project `source`_`suffix`."""
    want = f'projects/{source}_{suffix}/client_project'
    found = [e['value'] for e in json.loads(RENDERER.read_text())['add_value']
             if e['key'] == 'srp_renderer_v011w' and want in e['files']]
    assert len(found) == 1 and isinstance(found[0], dict), f'no one profile for {want} in {RENDERER.name}'
    return found[0]
# Where Chemical's layout asks for its own value. Vostok's evening sun stands at azimuth 253.25, set for
# Vostok; Chemical Plant's own authored evening (0.21d's, pack maps-021d-chemical-weathers@1, evening_2)
# puts it at 74.86, and the map's lit facades and its sun views face that way. The cloud texture turns
# with the sun so that its painted glow stays on it (Vostok's -14.2 plus the same 178.39). Vostok's rain
# sun (219.68) is within 3 degrees of Chemical's own (222.2): kept.
SUN = {'evening': (74.86, -14.2 + (74.86 - 253.2528))}


def entry_values(buf, entry):
    """{key path in the entry: (type, value)} of every leaf under the entry at `entry`."""
    root = dict(zip(ce.value_offsets(buf), (q for q, _, _ in ce.walk(buf))))[entry]
    out = {}
    for path, t, raw in ce.walk(buf, entry, ''):
        if t in (ce.T_NAMED, ce.T_INDEXED):
            continue
        key = path.lstrip('/')
        if t == ce.T_STR:
            out[key] = (t, raw.decode())
        elif t in (ce.T_INT, ce.T_FLOAT):
            out[key] = (t, ce.as_number(t, raw))
        elif t == ce.T_BOOL:
            out[key] = (t, bool(raw & 1))
        else:
            out[key] = (t, list(struct.unpack('<%df' % (t - 4), raw)))
    return root, out


def environment(buf):
    """(index, root path, values) of the project's one environment object."""
    found = [(i, c) for i, c in enumerate(ce.children(buf, ce.find(buf, 'client_objects')))
             if ce.value_of(buf, ce.find(buf, 'game_object_type', c)) == 'environment']
    assert len(found) == 1, 'not one environment object'
    i, c = found[0]
    root, values = entry_values(buf, c)
    return i, root, values


def later_sets(path):
    """{edit path: value it had} for `set` rows of the lists below this one's place on `path`."""
    rows = [line.split('\t') for line in (REPO / 'release/content.tsv').read_text().splitlines()
            if line.startswith('edit\t')]
    names = [r[1] for r in rows]
    below = names[names.index('modes.tsv'):]
    before = {}
    for name in below:
        if name.endswith('.tsv') or not (EDITS / name).exists():
            continue
        edits = json.loads((EDITS / name).read_text())
        for op in ('set', 'set_vector', 'set_bool'):
            for e in edits.get(op, []):
                if e.get('file') == path:
                    before.setdefault(e['path'], (name, e['from']))
        for op in ('set_entry', 'add_value', 'remove_key', 'replace_string'):
            for e in edits.get(op, []):
                files = e.get('files', e.get('file'))
                assert path not in (files if isinstance(files, list) else [files]), \
                    f'{name} {op}s {path}: read it as it was before that list'
    return before


def number_type(t):
    return 'float' if t == ce.T_FLOAT else 'int'


def main():
    day_files = {'day_2': Path(sys.argv[1]), 'day_4': Path(sys.argv[2])}
    pack = Path(sys.argv[3])
    assert hashlib.sha256((pack / 'pack.json').read_bytes()).hexdigest() == PACK_SHA, 'not v011e-radar-weather@6'
    edits = {'_about': None, 'copy': [], 'set': [], 'set_vector': [], 'set_bool': [], 'set_entry': [],
             'replace_string': [], 'remove_key': [], 'add_value': []}
    notes = []
    for day, file in day_files.items():
        buf = bytearray(file.read_bytes())
        assert hashlib.sha256(buf).hexdigest() == DAY_SHA[day], f'{file} is not the published {day}'
        index, root, have = environment(buf)
        path = DAY.format(day=day)
        for key, (lst, frm) in later_sets(path).items():
            prefix = f'client_objects[{index}]/'
            if key.startswith(prefix):
                k = key[len(prefix):]
                have[k] = (have[k][0], frm)
                notes.append(f'{path} {k}: {frm} as it is above {lst}')
        suffix = day.split('_')[1]
        for weather, (folder, source, short) in WEATHERS.items():
            out = f'projects/{folder}/{weather}_{suffix}/client_project'
            src = bytearray((pack / 'files/projects' / f'{source}_{suffix}' / 'client_project').read_bytes())
            _, _, want = environment(src)
            if weather in SUN:
                azimuth, clouds = SUN[weather]
                clouds = (clouds + 180) % 360 - 180
                want['properties/sun_position_azimut'] = (ce.T_FLOAT, azimuth)
                want['properties/sky_clouds_rotation'] = (ce.T_FLOAT, clouds)
            edits['copy'].append({'files': path, 'from': f'projects/v011e_level_04/{day}/',
                                  'to': f'projects/{folder}/{weather}_{suffix}/'})
            base = f'client_objects[{index}]/'
            strings_from, strings_to = {}, {}
            for k in sorted(set(have) | set(want)):
                if k in ('name', 'full_name', 'lib_guid', 'lib_name', 'game_object_type', 'tool_name', 'type',
                         'position', 'rotation', 'scale', 'pivot', 'is_temporary'):
                    continue
                if k not in want:
                    edits['remove_key'].append({'files': out, 'keys': [base + k]})
                    continue
                t, v = want[k]
                if k not in have:
                    edits['add_value'].append({'files': [out], 'table': (base + k).rsplit('/', 1)[0],
                                               'key': k.rsplit('/', 1)[1], 'value': float(v) if t == ce.T_FLOAT else v})
                    continue
                ht, hv = have[k]
                if hv == v and ht == t:
                    continue
                if t == ce.T_STR:
                    strings_from[k], strings_to[k] = hv, v
                elif t == ce.T_BOOL:
                    edits['set_bool'].append({'file': out, 'path': base + k, 'from': hv, 'to': v})
                elif t in (ce.T_INT, ce.T_FLOAT):
                    if ce.same(hv, v):
                        continue
                    row = {'file': out, 'path': base + k, 'from': hv, 'to': v}
                    if ht == ce.T_INT and v != int(v):
                        row['as_float'] = True
                    edits['set'].append(row)
                else:
                    edits['set_vector'].append({'file': out, 'path': base + k, 'from': hv, 'to': v})
            if strings_from:
                edits['set_entry'].append({'file': out, 'list': 'client_objects',
                                           'where': {'game_object_type': 'environment'},
                                           'from': strings_from, 'to': strings_to})
            edits['add_value'].append({'files': [out], 'table': '', 'key': 'srp_renderer_v011w',
                                       'value': profile(source, suffix)})
            config = f'gameplay/game_modes/v011e_level_04_{day}.options'
            new = f'gameplay/game_modes/{folder}_{suffix}.options'
            edits['copy'].append({'files': config, 'from': config, 'to': new})
            edits['replace_string'] += [
                {'files': new, 'from': f'v011e_level_04/{day}', 'to': f'{folder}/{weather}_{suffix}'},
                {'files': new, 'from': f'v011e_level_04_{suffix}', 'to': f'{folder}_{suffix}'}]
    for weather, (folder, _, _) in WEATHERS.items():
        edits['copy'].append({'files': 'projects/v011e_level_04/vegetation/*', 'from': 'projects/v011e_level_04/vegetation/',
                              'to': f'projects/{folder}/vegetation/'})
    edits['_about'] = (
        "Chemical Plant v0.11's evening and rain, made by srp-026g/scripts/v011-weather/make_chemical_weathers.py "
        "(read it for the why). 0.11 shipped Chemical Plant as day only; each weather is a copy of the day "
        "project as players have it, its environment object set to Vostok v0.11's same weather (pack "
        "v011e-radar-weather@6, 0.11e's own evening and rain), the profile exe patch 162 draws a 0.11 weather by "
        "(srp_renderer_v011w, Vostok's own of that weather, from v011w-renderer-1.json), and a mode config a copy of the day's naming its project. "
        "The probes keep the day project's textures.<weather> cubemaps (native Chemical Plant's evening and "
        "rain). Above modes.tsv, which gives the configs their weather; the maps_dict rows are "
        "v011w-chemical-weathers-maps-1.json, last.")
    edits = {k: v for k, v in edits.items() if v}
    (EDITS / NAME).write_text(json.dumps(edits, indent=1) + '\n')
    print(f'{EDITS / NAME}: ' + ', '.join(f'{len(v)} {k}' for k, v in edits.items() if isinstance(v, list)))
    for n in notes:
        print('  ' + n)


if __name__ == '__main__':
    main()
