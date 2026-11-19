"""The edit lists that change ported 0.21 materials after the edits whose snapshots the checkers here
pin (#2170 and later): the renderer keys of exe patch 130 (`v021d-renderer-*.json`: one new value
leaf a material) and the anomaly materials' numbers (`v021d-anomaly-materials-1.json`). A checker
that proves an earlier edit left every other leaf alone undoes exactly these first."""
import json
from pathlib import Path

EDITS = Path(__file__).resolve().parent.parent / 'content-edit'


def _key(path):
    return '.'.join(path.split('/'))


def _target(file):
    return file.removeprefix('material_instances/').removesuffix('.material')


def _changes():
    added, changed = {}, {}
    for path in sorted(EDITS.glob('v021d-renderer-*.json')):
        for e in json.loads(path.read_text()).get('add_value', []):
            for file in e['files']:
                if file.startswith('material_instances/'):
                    leaf = _key(f"{e['table']}/{e['key']}")
                    added.setdefault(_target(file), set()).update({f'{leaf}.value', f'{leaf}.is_model_parameter'})
    anomaly = json.loads((EDITS / 'v021d-anomaly-materials-1.json').read_text())
    for op in ('set', 'set_bool'):
        for e in anomaly.get(op, []):
            changed.setdefault(_target(e['file']), {})[_key(e['path'])] = (e['from'], e['to'])
    return added, changed


ADDED, CHANGED = _changes()


def undo(leaves, material):
    """`leaves` (a material's `--leaves` dump, by key) as it was before those edits."""
    out = {k: v for k, v in leaves.items() if k not in ADDED.get(material, set())}
    for key, (was, now) in CHANGED.get(material, {}).items():
        assert abs(out[key] - now) < 1e-6, (material, key, out[key], now)
        out[key] = was
    return out
