"""Lays the game's own Bullet shapes (a capture) against the export's (`mapdata-shapes` of a version 6 map
data file, and `mapdata-flat` of the same file for the mesh vertices), field by field, as BITS
(docs/notes/collision_world.md section 5.5).

    python bw_shapes.py <capture.bwc> <shapes.txt> <export.mdf> [--show N]

Pairs: a game object and the export placement with the same group, mask and origin bits (several at one origin
in the order of each side). Then for each pair, every field the exe holds and the export says:
  object     the world transform (+10h of the btCollisionObject): 9 basis values, 3 origin values
  kind       mesh, convex (its Bullet type and up axis), compound (its child count)
  mesh       the mesh interface's scaling; every triangle's corners as the game's cast takes them (the file's
             vertex x the scaling, in f32) against the export's library vertices with z negated; the walker
             behaviour of each triangle (face data >> 14)
  convex     local scaling (+10h), implicit dimensions (+20h), margin (+30h), walker behaviour of the part
  compound   local scaling (+50h), each child's transform (+0) and shape
Counts are of values: "same" is the same 32 bits.
"""
import argparse
import collections
import os
import struct
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_diff  # noqa: E402
import bw_report  # noqa: E402


def f32_bits(values):
    return [struct.unpack("<I", struct.pack("<f", v))[0] for v in values]


def read_shapes(path):
    """The export's objects: (placement, library, group, mask, transform bits, shape)."""
    lines = open(path).read().split("\n")
    at = 0

    def hexes(fields):
        return [int(x, 16) for x in fields]

    def part(fields):
        if fields[0] == "mesh":
            return {"kind": "mesh", "scaling": hexes(fields[1:4]), "faces": (int(fields[4]), int(fields[5]))}
        if fields[0] == "convex":
            return {"kind": "convex", "type": int(fields[1]), "up": None if fields[2] == "-" else int(fields[2]),
                    "scaling": hexes(fields[3:6]), "dims": hexes(fields[6:9]), "margin": hexes(fields[9:10]),
                    "walker": int(fields[10]), "faces": (int(fields[11]), int(fields[12]))}
        raise ValueError(fields)

    objects = []
    while at < len(lines) and lines[at]:
        f = lines[at].split()
        assert f[0] == "object", f
        obj = {"placement": int(f[1]), "library": int(f[2]), "group": int(f[3]), "mask": int(f[4]),
               "transform": hexes(f[5:17])}
        at += 1
        g = lines[at].split()
        at += 1
        if g[0] == "empty":
            obj["shape"] = {"kind": "empty"}
        elif g[0] == "compound":
            children = []
            for _ in range(int(g[4])):
                c = lines[at].split()
                assert c[0] == "child", c
                children.append({"transform": hexes(c[1:13]), "part": part(lines[at + 1].split())})
                at += 2
            obj["shape"] = {"kind": "compound", "scaling": hexes(g[1:4]), "children": children}
        else:
            obj["shape"] = part(g)
        objects.append(obj)
    return objects


KINDS = {"empty": 0, "mesh": 1, "convex": 2, "compound": 3}


class Count:
    def __init__(self):
        self.same = collections.Counter()
        self.total = collections.Counter()
        self.examples = collections.defaultdict(list)

    def add(self, field, game, export, where, show):
        for i, (a, b) in enumerate(zip(game, export)):
            self.total[field] += 1
            if a == b:
                self.same[field] += 1
            elif len(self.examples[field]) < show:
                if field.endswith("kind"):
                    names = {v: k for k, v in KINDS.items()}
                    self.examples[field].append(f"{where} game {names.get(a, a)} export {names.get(b, b)}")
                else:
                    fa, fb = (struct.unpack("<f", struct.pack("<I", x))[0] for x in (a, b))
                    self.examples[field].append(f"{where} [{i}] game {a:08x} ({fa!r}) export {b:08x} ({fb!r})")
        if len(game) != len(export):
            self.total[field + " (count)"] += 1


def transform_bits(t):
    return f32_bits([*t["basis"][0], *t["basis"][1], *t["basis"][2], *t["origin"]])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("shapes")
    ap.add_argument("mdf")
    ap.add_argument("--show", type=int, default=3)
    args = ap.parse_args()

    game = bw_diff.Game(args.capture)
    _, libs, _ = bw_diff.read_mdf(args.mdf)
    export = read_shapes(args.shapes)
    walker = game.walker

    by_key = collections.defaultdict(list)
    for e in export:
        by_key[(tuple(e["transform"][9:12]), e["group"], e["mask"])].append(e)
    pairs, game_only = [], []
    for obj in game.world["objects"]:
        key = (tuple(f32_bits(obj["transform"]["origin"])), obj["group"], obj["mask"])
        if by_key.get(key):
            pairs.append((obj, by_key[key].pop(0)))
        else:
            game_only.append(obj)
    export_only = [e for group in by_key.values() for e in group]
    in_order = sum(1 for n, (_, e) in enumerate(pairs) if e["placement"] == n)
    print(f"pairs {len(pairs)} (in the export's order: {in_order}); game objects unpaired {len(game_only)}; "
          f"export placements unpaired {len(export_only)}")
    for e in export_only[: args.show]:
        print(f"  export only: placement {e['placement']} group {e['group']} mask {e['mask']}")

    count = Count()
    kinds = collections.Counter()

    def mesh(shape, part, where):
        mi = walker.meshes[shape["mesh"]]
        count.add("mesh scaling", f32_bits(mi["scaling"][:3]), part["scaling"], where, args.show)
        verts, idx, face, _ = game.mesh(shape)  # verts: float64 of (f32 x scaling); redo it in f32
        meta = mi["parts"][0]
        raw = np.frombuffer(game.cap.need(meta["vertex_base"], meta["vertices"] * 12), "<f4").reshape(-1, 3)
        scaled = (raw * np.array(mi["scaling"][:3], dtype=np.float32)).astype(np.float32)
        lib = libs[current_library[0]]
        first, end = part["faces"]
        if end - first != len(idx):
            count.add("mesh triangle count", [len(idx)], [end - first], where, args.show)
            return
        game_corners = scaled[idx].reshape(-1)
        ours = lib["V"][lib["F"][first:end]].astype(np.float32).copy()
        ours[:, :, 2] = -ours[:, :, 2]
        a = game_corners.view(np.uint32)
        b = ours.reshape(-1).view(np.uint32)
        count.total["mesh corners"] += len(a)
        count.same["mesh corners"] += int((a == b).sum())
        if face is not None:
            gw = (face.astype(np.int32) >> 14).tolist()
            gw = [w if w != 3 else 0 for w in gw]
            count.add("mesh walker", gw, lib["W"][first:end].tolist(), where, args.show)

    def convex(shape, part, word, where):
        kinds[("convex", shape["kind"])] += 1
        count.add("convex type", [shape["type"]], [part["type"]], where, args.show)
        if "up_axis" in shape:
            count.add("convex up axis", [shape["up_axis"]], [part["up"]], where, args.show)
        count.add("convex scaling", f32_bits(shape["local_scaling"][:3]), part["scaling"], where, args.show)
        count.add("convex dims", f32_bits(shape["implicit_dims"][:3]), part["dims"], where, args.show)
        count.add("convex margin", f32_bits([shape["margin"]]), part["margin"], where, args.show)
        if word is not None:
            walk = int(word) >> 14 if int(word) != 0xFFFF else 0
            count.add("convex walker", [walk], [part["walker"]], where, args.show)

    current_library = [0]
    for obj, e in pairs:
        where = f"object {obj['index']} / placement {e['placement']}"
        current_library[0] = e["library"]
        count.add("object transform", transform_bits(obj["transform"]), e["transform"], where, args.show)
        shape = walker.shapes.get(obj["shape"])
        ours = e["shape"]
        words = game.shape_materials(obj)
        if shape is None:
            count.add("shape kind", [KINDS["empty"]], [KINDS[ours["kind"]]], where, args.show)
            continue
        gkind = {"triangle_mesh": "mesh", "compound": "compound"}.get(shape["kind"], "convex")
        count.add("shape kind", [KINDS[gkind]], [KINDS[ours["kind"]]], where, args.show)
        kinds[gkind] += 1
        if gkind != ours["kind"]:
            continue
        if gkind == "mesh":
            mesh(shape, ours, where)
        elif gkind == "convex":
            convex(shape, ours, None if words is None else words[0], where)
        else:
            count.add("compound scaling", f32_bits(shape["local_scaling"][:3]), ours["scaling"], where, args.show)
            count.add("compound children", [len(shape["children"])], [len(ours["children"])], where, args.show)
            for n, (gc, oc) in enumerate(zip(shape["children"], ours["children"])):
                cw = f"{where} child {n}"
                count.add("child transform", transform_bits(gc["transform"]), oc["transform"], cw, args.show)
                sub = walker.shapes.get(gc["shape"])
                skind = "mesh" if sub["kind"] == "triangle_mesh" else "convex"
                count.add("child kind", [KINDS[skind]], [KINDS[oc["part"]["kind"]]], cw, args.show)
                if skind != oc["part"]["kind"]:
                    continue
                if skind == "mesh":
                    mesh(sub, oc["part"], cw)
                else:
                    convex(sub, oc["part"], None if words is None or n >= len(words) else words[n], cw)

    print("shapes:", dict(kinds))
    print(f"{'field':24s} {'same bits':>12s} {'values':>12s}")
    for field in sorted(count.total):
        print(f"{field:24s} {count.same[field]:12d} {count.total[field]:12d}")
    for field, rows in sorted(count.examples.items()):
        print(f"-- {field}")
        for row in rows:
            print("   " + row)


if __name__ == "__main__":
    main()
