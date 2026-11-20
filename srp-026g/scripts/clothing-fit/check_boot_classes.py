#!/usr/bin/env python3
"""Check imported boot shaft heights against the native boots of their model_type.

--native is a stock extracted resource tree. --tree arguments are extracted
resource trees, in overlay order (a content pack's files/ is also a tree).
Models may be in a different tree from their config. No server data is used.
This catches low boots selecting tucked trousers; it does not establish fit
while animated or detect a narrow trousers mesh. Run the porter fitting tests
and inspect the comparison renders as well.
"""
import argparse
from dataclasses import dataclass
import json
import math
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'research-mode'))
import bcw

PREFIX = 'gameplay/items/armour/boots'
CLASSES = {'data_full', 'data_medium', 'data_long'}
# Allowance for uneven cuff tops on either side, in metres.
TOLERANCE = 0.02


@dataclass(frozen=True)
class Boot:
    path: str
    kind: str
    tops: tuple[float, float]


class Trees:
    def __init__(self, trees):
        self.trees = trees

    def read(self, name):
        for tree in reversed(self.trees):
            path = tree / name
            if path.is_file():
                return path.read_bytes()
        raise ValueError(f'missing resource: {name}')

    def boots(self):
        return sorted({str(p.relative_to(t)) for t in self.trees
                       for p in (t / PREFIX).glob('*.options')})

    def boot(self, name):
        data = bcw.loads(self.read(name))['data']
        kind = data['model_type']
        if kind not in CLASSES:
            raise ValueError(f'{name}: unknown trousers class {kind!r}')
        parts = [data, *data.get('additional_models', [])]
        points = []
        for part in parts:
            raw = self.read(f"models/{part['base_model']}.skinned_model/render/"
                            f"{part['part_name']}/converted_model")
            chunk, size, count = struct.unpack_from('<III', raw)
            if chunk != 3 or not count or (size - 4) % count:
                raise ValueError(f'{name}: invalid vertex chunk')
            stride = (size - 4) // count
            if stride < 12 or len(raw) < size + 8:
                raise ValueError(f'{name}: truncated vertices')
            points.extend(struct.unpack_from('<3f', raw, 12 + i * stride) for i in range(count))
        if not all(math.isfinite(v) for point in points for v in point):
            raise ValueError(f'{name}: non-finite vertex')
        tops = tuple(max(y for x, y, _ in points if x * side > 0) for side in (-1, 1))
        return Boot(name, kind, tops)


class Resources(Trees):
    """Already composed bytes, for the release builder's content gate."""
    def __init__(self, files):
        self.files = files

    def read(self, name):
        try:
            return self.files[name]
        except KeyError as error:
            raise ValueError(f'missing resource: {name}') from error

    def boots(self):
        return sorted(p for p in self.files if is_boot(p))


def is_boot(path):
    return path.startswith(PREFIX + '/') and path.endswith('.options')


def model_paths(configs):
    paths = set()
    for raw in configs.values():
        data = bcw.loads(raw)['data']
        for part in [data, *data.get('additional_models', [])]:
            paths.add(f"models/{part['base_model']}.skinned_model/render/"
                      f"{part['part_name']}/converted_model")
    return sorted(paths)


def audit(native, effective):
    stock = [native.boot(p) for p in native.boots()]
    if {b.kind for b in stock} != CLASSES:
        raise ValueError('native tree must contain all three boot classes')
    floor = {kind: min(min(b.tops) for b in stock if b.kind == kind) for kind in CLASSES}
    rows = []
    for name in effective.boots():
        boot = effective.boot(name)
        # Full-length trousers go over a shaft; tall low-class boots are valid.
        # Medium/long trousers tuck inside it and require a shaft to cover them.
        short = boot.kind != 'data_full' and min(boot.tops) + TOLERANCE < floor[boot.kind]
        rows.append({'path': name, 'class': boot.kind,
                     'shaft_top_m': boot.tops, 'native_class_min_m': floor[boot.kind],
                     'status': 'shaft_too_short' if short else 'ok'})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--tree', type=Path, action='append', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    rows = audit(Trees([args.native]), Trees([args.native, *args.tree]))
    args.out.write_text(json.dumps(rows, indent=2) + '\n')
    failures = [r for r in rows if r['status'] != 'ok']
    for row in failures:
        print(f"{row['path']}: {row['class']} shaft ends at {min(row['shaft_top_m']):.3f} m; "
              f"native class begins at {row['native_class_min_m']:.3f} m")
    print(f'{len(rows)} boots checked; {len(failures)} class/height mismatches')
    return bool(failures)


if __name__ == '__main__':
    sys.exit(main())
