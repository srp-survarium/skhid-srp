"""Fixtures added to a project's existing lamp table by a later edit list
(skeleton-cap-2, kitchen-ware-1, lab-capsule-glass-1, school-bells-1): the
visuals they need placed after the project's last visual (so no index an
earlier list bound moves), their rows, grouped by the projects that have the
same one, and the collision objects of the things that fly off.

The client reads a row's own break_material, effect_material and leftover
over the table's (docs/notes/breakable_lamps.md "Rows of their own"): a
skeleton's cap among School's lamps marks as stone, a tin can plays its
metal hit.
"""
import json

import make_edits as lamps

BOUNDS_KEYS = ('min_x', 'min_y', 'min_z', 'max_x', 'max_y', 'max_z')
GLASS_KEYS = ('glass_from_x', 'glass_from_y', 'glass_from_z', 'glass_to_x', 'glass_to_y', 'glass_to_z')


def names(v):
    """The names a map's visual carries (the v0.69 ports' have no visual_name)."""
    return [key for key in ('visual_name', 'lib_name') if key in v]


def where(v):
    return {key: v[key] for key in names(v) + ['position', 'rotation', 'scale']}


class Placer:
    """One project's new visuals, each at the end of its render_visuals (its last may have a twin:
    Rudnya's Research projects end in two transmitter slots at one place)."""

    def __init__(self, path, visuals):
        self.path, self.next, self.entries = path, len(visuals), []

    def place(self, like, model, position=None, scale=None):
        """A new visual of `model` placed as `like` (at `position`, of `scale`): its index."""
        values = {key: model for key in names(like)}
        if position is not None:
            values['position'] = position
        if scale is not None:
            values['scale'] = scale
        self.entries.append({'files': self.path, 'list': 'render_visuals', 'at_end': True, 'like': where(like),
                             'values': values})
        self.next += 1
        return self.next - 1


def compact(placed, every):
    """The add_entry rows of `placed` ({path: its Placer's entries}), a
    level's projects that take the same entries in one row each, its files
    `projects/<level>/*/client_project` when that is every project of the
    level in `every` (the paths read), else each project's own."""
    alike = {}
    for path, entries in placed.items():
        same = json.dumps([{k: v for k, v in e.items() if k != 'files'} for e in entries], sort_keys=True)
        alike.setdefault((path.split('/')[1], same), []).append(path)
    out = []
    for (level, same), paths in sorted(alike.items(), key=lambda kv: kv[1]):
        level_paths = {p for p in every if p.split('/')[1] == level}
        groups = [f'projects/{level}/*/client_project'] if set(paths) == level_paths else sorted(paths)
        for files in groups:
            out += [dict(e, files=files) for e in json.loads(same)]
    return out


def row(lo, hi, start, end, **rest):
    return dict(zip(BOUNDS_KEYS, list(lo) + list(hi))) | dict(zip(GLASS_KEYS, list(start) + list(end))) | rest


def table_of(path, root):
    """The project's lamp table (every project a later list adds to has one)."""
    table = root.get('srp_lamps')
    if table is None:
        raise ValueError(f'{path}: no lamp table to add to')
    return table


def check_keys(path, table, keys):
    taken = set(keys) & set(table['lamps'])
    if taken:
        raise ValueError(f'{path}: {sorted(taken)} already fixtures')


def grouped(found):
    """add_value rows of `found` ({path: {key: row}}): one a row, its files
    every project that has that row alike."""
    alike = {}
    for path, rows in found.items():
        for key, value in rows.items():
            alike.setdefault(json.dumps([key, value], sort_keys=True), []).append(path)
    return [{'files': files, 'table': 'srp_lamps/lamps', 'key': key, 'value': value}
            for spec, files in sorted(alike.items(), key=lambda kv: (kv[1], kv[0]))
            for key, value in [json.loads(spec)]]


def collisions(path, root, models):
    """remove_entry rows: every collision object (`collision`,
    `walker_collision`) of each visual of `models`, matched by its model and
    position (a port's rotation differs by a turn's rounding). A visual has
    one of each or none (some ports place no collision for a jug)."""
    objects = root.get('collision_objects', [])
    out = []
    for v in root.get('render_visuals', []):
        if lamps.model_name(v) not in models:
            continue
        for part in ('collision', 'walker_collision'):
            name = f'resources/models/{lamps.model_name(v)}.model/{part}'
            found = [o for o in objects if o.get('lib_name') == name
                     and all(abs(a - b) <= 5e-4 for a, b in zip(o['position'], v['position']))]
            if len(found) > 1:
                raise ValueError(f'{path}: {len(found)} {part} objects of the {lamps.model_name(v)} at {v["position"]}')
            if found:
                out.append({'file': path, 'list': 'collision_objects',
                            'where': {'lib_name': name, 'position': v['position']}})
    return out


def projects(folder, levels=None):
    """Every map project of `folder` (`projects/<level>/<project>/client_project`), the lobby's not:
    (its path, its bytes)."""
    for p in sorted((folder / 'projects').glob('*/*/client_project')):
        if p.parts[-3] == 'lobby_scene' or (levels is not None and p.parts[-3] not in levels):
            continue
        yield str(p.relative_to(folder)), p.read_bytes()
