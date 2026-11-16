#!/usr/bin/env python3
"""make_pack.py: 0.69d's grenade bounce sounds for 0.26g, the content pack grenade-hits-069d
(docs/notes/grenade_bounce.md, "0.69d's bounces").

usage: python3 -I make_pack.py --donor <folder> --stock <folder> --out <packs folder>/grenade-hits-069d-<version>

--donor  0.69d's samples at their game paths: sounds/single/weapons/f1/<name>.high.ogg and .high.rms of
         every name below (resources.02.db of the 0.69d build: `vostok-port extract --db
         <resources.02.db> --path <path> --output <folder>/<path>`), each pinned in donor.sha256
--stock  the stock 0.26g resources.db's templates at their game paths (STOCK below)
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv)

0.69d plays a grenade's bounce (the grenade material 45's collision_sound) from two composites,
weapons/f1_surface_hit (ground, wood, concrete, stone, glass, a wooden bridge) and weapons/f1_metal_hit
(metal, a car, a gas tank, a tank, mesh), each a close and a distant collection of its own recordings
by distance (0 to 5 m and 5 to 25 m; metal 0 to 15 m and 15 to 25 m). The edit list
grenade-hits-069d-1 points the pairs at them, the pairs 0.69d points.

0.69d's sound configs are of a later binary format than 0.26g reads, so each is made here in 0.26g's
from a stock config of its kind, with 0.69d's values (as `vostok-port dump-db-config` reads them,
vectors to three decimals): the samples' .raw_options (volume_k, spl_value, spl_preset, use_hdr,
can_be_displaced, lp_filter_k; pan_3d, which 0.26g does not read, too), the four collections (random,
the samples in 0.69d's order) and the two composites. The samples' distance preset is 0.69d's
vss_main, which differs from 0.26g's (a nearer fall-off: 0.38 at 3 m, 0.17 at 7 m, silent at 25 m;
0.26g's 0.38 at 6 m): it is added as sounds/spl/grenade_hit_069d.spl, so 0.26g's VSS keeps its own.
The .high.ogg and .high.rms are 0.69d's, byte for byte.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("codec", HERE.parent / "lobby-bulb" / "make_pack.py")
codec = importlib.util.module_from_spec(spec)
spec.loader.exec_module(codec)
T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = (codec.T_BOOL, codec.T_INT, codec.T_FLOAT, codec.T_NAMED,
                                                      codec.T_INDEXED, codec.T_STR)
T_FLOAT2 = 6

PACK_ID = "grenade-hits-069d"
VERSION = "1"
SINGLE = "sounds/single/weapons/f1/"
PRESET = "grenade_hit_069d"
STOCK = {
    "sounds/single/weapons/f1/grenade_hit_ground_01.raw_options":
        "065786135163294c468b04fe14b158a2338eba174f5dd39a4e796cefd003f76e",
    "sounds/collections/weapons/f1/f1_hit_ground.sound_collection":
        "8031e0d15aabd2a560c0e204d29b8f314aa821a99143dd736d4eb15188f8c3cd",
    "sounds/composite/step/asphalt_sprint.composite_sound":
        "0093aa5dbf9c6a16c836080bca8f2639ffd55bdc016dcfb7c0e767024b9ca7e0",
    "sounds/spl/vss_main.spl":
        "c05b647e1deb4675e9b82a758ce96aa4c8ffc5c4cdab0d9180abcf100195e46e",
}
# collection: (its samples in 0.69d's order, the samples' volume_k)
COLLECTIONS = {
    "greande_hit_close": (["grenade_hit_close_%02d" % n for n in [1] + list(range(33, 1, -1))], 0.5),
    "grenade_hit_distant": (["grenade_hit_distant_%02d" % n for n in [1] + list(range(10, 1, -1))], 1.0),
    "grenade_hit_metal_close": (["grenade_hit_metal_close_%02d" % n
                                 for n in [1, 18, 17, 16, 15, 14, 13, 12, 10, 9, 8, 7, 6, 5, 4]], 0.8),
    "grenade_hit_metal_distant": (["grenade_hit_metal_distant_%02d" % n for n in [1, 7, 6, 5, 4, 3, 2, 8, 9, 10]], 0.8),
}
# composite: its layers (collection, min_distance, max_distance)
COMPOSITES = {
    "f1_surface_hit": [("greande_hit_close", 0, 5), ("grenade_hit_distant", 5, 25)],
    "f1_metal_hit": [("grenade_hit_metal_close", 0, 15), ("grenade_hit_metal_distant", 15, 25)],
}
# 0.69d's vss_main: curve -> key -> (key_type, left_tangent, position, right_tangent)
CURVES = {
    "spl": [(2, (-1.0, 0.0), (0.0, 100.0), (0.234, 98.669)),
            (0, (0.467, 85.424), (0.701, 80.006), (1.101, 70.732)),
            (0, (1.501, 63.144), (1.901, 55.923), (2.394, 47.017)),
            (0, (2.888, 35.789), (3.381, 31.625), (4.645, 20.957)),
            (0, (5.909, 15.674), (7.173, 11.429), (8.634, 6.522)),
            (0, (10.095, 5.471), (11.556, 4.169), (14.371, 1.661)),
            (2, (17.185, 0.088), (20.0, 0.0), (21.0, -0.526))],
    "volume": [(2, (-1.0, 2.949), (0.0, 1.0), (0.172, 0.975)),
               (0, (0.343, 0.874), (0.515, 0.825), (0.783, 0.749)),
               (0, (1.052, 0.671), (1.32, 0.624), (1.895, 0.523)),
               (0, (2.47, 0.427), (3.045, 0.38), (4.335, 0.274)),
               (0, (5.625, 0.201), (6.914, 0.167), (9.807, 0.089)),
               (0, (12.699, 0.07), (15.591, 0.043), (18.728, 0.014)),
               (2, (21.864, 0.0), (25.0, 0.0), (0.0, 0.0))],
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read(folder, path, want):
    data = (folder / path).read_bytes()
    if sha256(data) != want:
        sys.exit(f"{path}: sha256 {sha256(data)}, not the pinned {want}")
    return data


def float_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def with_values(tree, path, values):
    """`tree` with the table at `path` holding `values` (name: (type, data)): its own keys, each
    set, none added or lost."""
    table = codec.get(tree, path)
    names = [name for name, _ in table[1]]
    assert sorted(names) == sorted(values), (path, names)
    for name, value in values.items():
        assert dict(table[1])[name][0] == value[0], (path, name, "keeps its type")
        codec.put(tree, f"{path}/{name}", value)
    return tree


def options(template, volume_k):
    """A sample's .raw_options: 0.69d's for these samples."""
    tree = codec.parse(template)
    return codec.serialize(with_values(tree, "options", {
        "volume_k": (T_FLOAT, float_bits(volume_k)), "spl_value": (T_INT, 60), "spl_preset": (T_STR, PRESET),
        "use_hdr": (T_BOOL, 1), "can_be_displaced": (T_BOOL, 1), "lp_filter_k": (T_INT, 1), "pan_3d": (T_BOOL, 1)}))


def collection(template, samples):
    tree = codec.parse(template)
    table = codec.get(tree, "collection")
    item = dict(table[1])["sound_items"][1][0]
    assert sorted(name for name, _ in item[1]) == ["filename", "start_offset", "start_offset_max"], item
    items = [(T_NAMED, [("filename", (T_STR, "weapons/f1/" + name)), ("start_offset", (T_INT, 0)),
                        ("start_offset_max", (T_INT, 0))]) for name in samples]
    with_values(tree, "collection", {"type": (T_STR, "random"), "cyclic_repeat_from_sound": (T_INT, 0),
                                     "dont_repeat_sound_successively": (T_BOOL, 0),
                                     "sound_items": (T_INDEXED, items)})
    return codec.serialize(tree)


def composite(template, layers):
    tree = codec.parse(template)
    items = [(T_NAMED, [("filename", (T_STR, "weapons/f1/" + name)), ("enabled", (T_BOOL, 1)),
                        ("max_distance", (T_INT, far)), ("resource_type", (T_INT, 258)),
                        ("start_offset", (T_INT, 0)), ("start_offset_max", (T_INT, 0)),
                        ("min_distance", (T_INT, near))]) for name, near, far in layers]
    first = dict(codec.get(tree, "composite_sound")[1])["sound_items"][1][0]
    assert sorted(name for name, _ in first[1]) == sorted(name for name, _ in items[0][1]), first
    with_values(tree, "composite_sound", {"master_layer_index": (T_INT, 0), "use_master_layer_index": (T_BOOL, 0),
                                          "sound_items": (T_INDEXED, items)})
    return codec.serialize(tree)


def preset(template):
    """0.69d's vss_main in 0.26g's format: the template's two curves, each key 0.69d's."""
    tree = codec.parse(template)
    for curve, keys in CURVES.items():
        table = codec.get(tree, curve)
        assert sorted(name for name, _ in table[1]) == sorted("key%d" % k for k in range(len(keys))), curve
        for k, (kind, left, position, right) in enumerate(keys):
            with_values(tree, f"{curve}/key{k}", {"key_type": (T_INT, kind), "left_tangent": (T_FLOAT2, list(left)),
                                                  "position": (T_FLOAT2, list(position)),
                                                  "right_tangent": (T_FLOAT2, list(right))})
    return codec.serialize(tree)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--donor", required=True, type=Path)
    ap.add_argument("--stock", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    a = ap.parse_args()
    if a.out.exists():
        sys.exit(f"{a.out}: a new folder")
    stock = {path: read(a.stock, path, want) for path, want in STOCK.items()}
    pins = dict(reversed(line.split()) for line in (HERE / "donor.sha256").read_text().splitlines() if line.strip())
    files = {}
    for name, (samples, volume_k) in COLLECTIONS.items():
        for sample in samples:
            for ext in (".high.ogg", ".high.rms"):
                path = SINGLE + sample + ext
                files[path] = read(a.donor, path, pins.pop(path))
            files[SINGLE + sample + ".raw_options"] = options(stock[next(p for p in STOCK if p.endswith(".raw_options"))],
                                                              volume_k)
        files[f"sounds/collections/weapons/f1/{name}.sound_collection"] = collection(
            stock["sounds/collections/weapons/f1/f1_hit_ground.sound_collection"], samples)
    if pins:
        sys.exit(f"donor.sha256 pins files no collection takes: {sorted(pins)}")
    for name, layers in COMPOSITES.items():
        files[f"sounds/composite/weapons/{name}.composite_sound"] = composite(
            stock["sounds/composite/step/asphalt_sprint.composite_sound"], layers)
    files[f"sounds/spl/{PRESET}.spl"] = preset(stock["sounds/spl/vss_main.spl"])
    for path, data in files.items():
        (a.out / "files" / path).parent.mkdir(parents=True, exist_ok=True)
        (a.out / "files" / path).write_bytes(data)
    pack = {
        "format_version": 1, "id": PACK_ID, "version": VERSION, "target_build": "0.26g",
        "description": "0.69d's grenade bounces: the composites weapons/f1_surface_hit and weapons/f1_metal_hit, "
                       "their close and distant collections, 68 recordings and 0.69d's vss_main as the preset "
                       "grenade_hit_069d",
        "operations": [{"op": "add_file", "path": path} for path in sorted(files)],
        "notes": ["made by srp-026g/scripts/grenade-hits-069d/make_pack.py from the 0.69d build's resources.02.db "
                  "and the stock resources.db's templates (docs/notes/grenade_bounce.md)"],
    }
    (a.out / "pack.json").write_text(json.dumps(pack, indent=2) + "\n")
    size = sum(len(d) for d in files.values())
    print(f"{a.out}: {len(files)} files, {size} bytes; pack.json sha256 {sha256((a.out / 'pack.json').read_bytes())}")


if __name__ == "__main__":
    main()
