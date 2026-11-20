#!/usr/bin/env python3
"""Author original weather geometry deltas over corrected day-derived projects.

School rain's and Chemical Plant rain's two extra hanging lamps are converted by
the same breakable-lamp rule as day fixtures. Fort rain/evening omit three
original day barrel groups.
Only those donor differences change; existing lamp indices follow removals.
Input: pinned original donor projects, the porter's source import bundle, and
the weather content packs. Output is an edit list, never a binary patch.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from check_packs import lamps

GROUPS = ('render_visuals', 'collision_objects')


def difference(left, right):
    a = Counter(json.dumps(v, sort_keys=True) for v in left)
    b = Counter(json.dumps(v, sort_keys=True) for v in right)
    return [json.loads(v) for v, n in (a - b).items() for _ in range(n)]


def identity(row):
    return {k: row[k] for k in ('lib_name', 'position', 'rotation', 'scale')}


def original_identity(row):
    value = identity(row)
    value['lib_name'] = value['lib_name'].replace('ported/v021d/', '', 1)
    return value


def converted(original, entries):
    found = [v for v in entries if original_identity(v) == identity(original)]
    if len(found) != 1:
        raise ValueError(f'expected one converted placement: {identity(original)} ({len(found)})')
    return found[0]


def config(value):
    data = bytearray(24)
    record = lamps.c.value_record(data, 0, '', lamps.c.as_table(value, vectors=True))
    data[:24] = record
    return data


def apply(data, path, edits):
    """The content editor's execution order, for authoring/checking this source."""
    result = bytearray(data)
    for row in edits.get('set', []):
        if row['file'] == path:
            lamps.c.set_number(result, row['path'], row['from'], row['to'])
    for row in edits.get('append_entry', []):
        if row['file'] == path:
            lamps.c.append_entry(result, row['list'], row['expected_count'], row['unique_key'], row['value'], row['vectors'])
    for row in edits.get('add_value', []):
        if path in row['files']:
            lamps.c.add_value(result, row['table'], row['key'], row['value'])
    for row in edits.get('remove_entry', []):
        if row['file'] == path:
            lamps.c.remove_entry(result, row['list'], row['where'])
    return result


def make(donor, imported, packs):
    edits = {'_about': __doc__.strip(), 'set': [], 'append_entry': [], 'add_value': [], 'remove_entry': []}
    provenance = {}

    def read(root, path):
        data = (root / path).read_bytes()
        label = {donor: 'donor', imported: 'import'}.get(root, 'pack')
        provenance[f'{label}:{path}'] = hashlib.sha256(data).hexdigest()
        return data

    for level, weather, counts in [('02', 'rain', (6, 4)), ('05', 'rain', (-3, -6)), ('05', 'evening', (-3, -6)),
                                   ('04', 'rain', (6, 4))]:
        day = lamps.tree(read(donor, f'projects/level_{level}/day/client_project'))
        original = lamps.tree(read(donor, f'projects/level_{level}/{weather}/client_project'))
        adapted = lamps.tree(read(imported, f'projects/v021d_level_{level}/{weather}_2/client_project'))
        extras = {g: difference(original[g], day[g]) for g in GROUPS}
        missing = {g: difference(day[g], original[g]) for g in GROUPS}
        assert tuple(len(extras[g]) - len(missing[g]) for g in GROUPS) == counts
        assert all(not extras[g] or not missing[g] for g in GROUPS)
        for mode in (2, 4):
            path = f'projects/v021d_level_{level}/{weather}_{mode}/client_project'
            pack, = (p for p in packs if (p / 'files' / path).exists())
            data = read(pack / 'files', path)
            root = lamps.tree(data)
            if extras['render_visuals']:
                visuals = [converted(v, adapted['render_visuals']) for v in extras['render_visuals']]
                bounds = {}
                for visual in visuals:
                    name = visual['visual_name']
                    if name in lamps.PARTS:
                        b = lamps.tree(read(imported, f'models/{name}.model/render/export_properties'))
                        bounds[name] = [b['bounding_box'][k] for k in ('min', 'max')]
                mini = config({'render_visuals': visuals, 'client_objects': root['client_objects']})
                existing = root['srp_lamps']
                moves, additions, spec = lamps.author(path, mini, bounds, existing['break_material'], existing['effect_material'])
                assert not moves and len(additions) == 2 and len(spec['value']['lamps']) == 2
                for add in additions:
                    lamps.c.add_entry(mini, add['list'], add['after'], add['like'], add['values'])
                new_visuals = lamps.tree(mini)['render_visuals']
                assert len(new_visuals) == 8
                extra_records = {'render_visuals': new_visuals,
                                 'collision_objects': [converted(v, adapted['collision_objects']) for v in extras['collision_objects']]}
                for group, values in extra_records.items():
                    for i, value in enumerate(values):
                        edits['append_entry'].append({'file': path, 'list': group,
                            'expected_count': len(root[group]) + i, 'unique_key': None,
                            'vectors': True, 'value': value})
                for i, row in enumerate(spec['value']['lamps'].values()):
                    assert len(row['lights']) == 1 and len(row['visuals']) == 3
                    for v in row['visuals'].values():
                        v['index'] += len(root['render_visuals'])
                    key = f"lamp{len(existing['lamps']) + i:03d}"
                    assert key not in existing['lamps']
                    edits['add_value'].append({'files': [path], 'table': 'srp_lamps/lamps', 'key': key, 'value': row})
            else:
                removed_indices = []
                for group in GROUPS:
                    for original_row in missing[group]:
                        row = converted(original_row, root[group])
                        edits['remove_entry'].append({'file': path, 'list': group, 'where': identity(row)})
                        if group == 'render_visuals':
                            removed_indices.append(root[group].index(row))
                for name, row in root['srp_lamps']['lamps'].items():
                    for key, visual in row['visuals'].items():
                        before = visual['index']
                        assert before not in removed_indices
                        after = before - sum(index < before for index in removed_indices)
                        if before != after:
                            edits['set'].append({'file': path, 'path': f'srp_lamps/lamps/{name}/visuals/{key}/index', 'from': before, 'to': after})
            final = apply(data, path, edits)
            assert not lamps.check_built(path, final), path
            now = lamps.tree(final)
            for group, expected in zip(GROUPS, counts):
                broken = 2 if extras['render_visuals'] and group == 'render_visuals' else 0
                assert len(now[group]) - len(root[group]) == expected + broken
            assert now['server_objects'] == root['server_objects']
            assert now['client_objects'] == root['client_objects']
    return edits, provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('donor', 'imported', 'out', 'provenance'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--pack', type=Path, required=True, action='append', help='each weather pack')
    args = parser.parse_args()
    edits, provenance = make(args.donor, args.imported, args.pack)
    args.out.write_text(json.dumps(edits, indent=2) + '\n')
    args.provenance.write_text(json.dumps(provenance, indent=2, sort_keys=True) + '\n')
    print('School and Chemical Plant rain: two complete breakable fixtures each; Fort rain/evening: three donor-omitted barrel groups removed')


if __name__ == '__main__':
    main()
