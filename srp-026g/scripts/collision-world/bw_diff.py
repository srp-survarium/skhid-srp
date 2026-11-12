"""Lays the game's own Bullet world (a capture) against our export of the same project (a flattened map data
file, `mapdata-flat`), object by object, vertex by vertex (docs/notes/collision_world.md).

    python bw_diff.py <capture.bwc> <export.mdf> [--out report.txt] [--tsv rows.tsv]

Frames: the export is in the map frame; the game's world is the physics frame (z negated). Everything is
compared in the map frame, in metres.

What is compared for each pair (a game object and the export placement at the same place with the same group
and mask):
  rotation   the game's basis against the export's rotation matrix (largest element difference)
  mesh       every vertex of the game's triangle mesh, placed, against the export's vertex of the same index;
             the triangles' corner indices; the triangle count
  material   the game's material id of each triangle against the export's walker behaviour of the same triangle
  primitive  each box, sphere, cylinder, capsule of the game as the exact convex shape Bullet casts against
             (its margin included), against the export's triangles for it: the support functions of the two
             over 642 directions (how far one sticks out past the other)
"""
import argparse
import collections
import os
import struct
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_report  # noqa: E402

P = np.diag([1.0, 1.0, -1.0])
STATIC_BODY_USER_VTABLES = None  # every object is tried; the unmatched are listed by their kind


def read_mdf(path):
    data = open(path, "rb").read()
    assert data[:8] == b"SRPMDF1\n", path
    at = 8

    def u32():
        nonlocal at
        (v,) = struct.unpack_from("<I", data, at)
        at += 4
        return v

    def text():
        nonlocal at
        n = u32()
        s = data[at:at + n].decode()
        at += n
        return s

    def array(dtype, count, width):
        nonlocal at
        a = np.frombuffer(data, dtype=dtype, count=count * width, offset=at).reshape(count, width)
        at += a.nbytes
        return a

    project = text()
    libs = []
    for _ in range(u32()):
        name = text()
        primitives = u32()
        verts = array("<f4", u32(), 3)
        nf = u32()
        faces = array("<u4", nf, 3)
        walker = array("u1", nf, 1)[:, 0]
        libs.append({"name": name, "primitives": primitives, "V": verts, "F": faces, "W": walker})
    places = []
    for _ in range(u32()):
        lib = u32()
        pos = array("<f4", 1, 3)[0]
        rot = array("<f4", 1, 3)[0]
        cgroup, cmask = struct.unpack_from("<HH", data, at)
        at += 4
        axes = array("<f4", 3, 3)  # rows: where local x, y, z go
        places.append({"lib": lib, "pos": pos, "rot": rot, "group": cgroup, "mask": cmask, "axes": axes})
    return project, libs, places


def icosphere(level=3):
    t = (1 + 5 ** 0.5) / 2
    v = [(-1, t, 0), (1, t, 0), (-1, -t, 0), (1, -t, 0), (0, -1, t), (0, 1, t), (0, -1, -t), (0, 1, -t),
         (t, 0, -1), (t, 0, 1), (-t, 0, -1), (-t, 0, 1)]
    f = [(0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11), (1, 5, 9), (5, 11, 4), (11, 10, 2),
         (10, 7, 6), (7, 1, 8), (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8), (3, 8, 9), (4, 9, 5), (2, 4, 11),
         (6, 2, 10), (8, 6, 7), (9, 8, 1)]
    v = [np.array(p, float) / np.linalg.norm(p) for p in v]
    for _ in range(level):
        cache = {}
        out = []

        def mid(a, b):
            key = (min(a, b), max(a, b))
            if key not in cache:
                m = v[a] + v[b]
                v.append(m / np.linalg.norm(m))
                cache[key] = len(v) - 1
            return cache[key]

        for a, b, c in f:
            ab, bc, ca = mid(a, b), mid(b, c), mid(c, a)
            out += [(a, ab, ca), (b, bc, ab), (c, ca, bc), (ab, bc, ca)]
        f = out
    return np.array(v)


DIRS = icosphere(3)  # 642 unit directions


def support_local(shape, d):
    """Bullet's support of a convex shape along directions d (rows, in the shape's frame), margin included."""
    kind = shape["kind"]
    e = np.array(shape["implicit_dims"][:3])
    norm = np.linalg.norm(d, axis=1)
    if kind == "box":
        return np.abs(d) @ e + shape["margin"] * norm
    if kind == "sphere":
        return shape["implicit_dims"][0] * shape["local_scaling"][0] * norm
    up = shape.get("up_axis", 1)
    side = [a for a in range(3) if a != up]
    radial = np.sqrt(d[:, side[0]] ** 2 + d[:, side[1]] ** 2)
    if kind == "cylinder":
        return e[up] * np.abs(d[:, up]) + e[side[0]] * radial + shape["margin"] * norm
    if kind == "capsule":
        return e[up] * np.abs(d[:, up]) + e[side[0]] * norm
    raise ValueError(kind)


def components(faces):
    """Connected groups of faces (by shared vertex), in order of their first face."""
    parent = {}

    def find(x):
        while parent.setdefault(x, x) != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b, c in faces:
        ra, rb, rc = find(int(a)), find(int(b)), find(int(c))
        parent[rb] = ra
        parent[rc] = ra
    groups = collections.OrderedDict()
    for i, face in enumerate(faces):
        groups.setdefault(find(int(face[0])), []).append(i)
    return list(groups.values())


class Game:
    def __init__(self, capture):
        self.cap, self.walker, self.world = bw_report.load(capture)
        self.mesh_cache = {}

    def mesh(self, shape):
        addr = shape["addr"]
        if addr not in self.mesh_cache:
            mi = self.walker.meshes[shape["mesh"]]
            part = mi["parts"][0]
            verts = np.frombuffer(self.cap.need(part["vertex_base"], part["vertices"] * 12), "<f4").reshape(-1, 3)
            idx = np.frombuffer(self.cap.need(part["index_base"], part["triangles"] * 12), "<u4").reshape(-1, 3)
            face = None
            if shape.get("face_data"):
                raw = self.cap.read(shape["face_data"], part["triangles"] * 2)
                if raw is not None:
                    face = np.frombuffer(raw, "<u2")
            scaling = np.array(mi["scaling"][:3])
            self.mesh_cache[addr] = (verts.astype(float) * scaling, idx, face, scaling)
        return self.mesh_cache[addr]

    def parts(self, obj):
        """The object's shape as a list of (kind, shape, M, t): x_map = M x_local + t."""
        basis = np.array(obj["transform"]["basis"])
        origin = np.array(obj["transform"]["origin"])
        shape = self.walker.shapes.get(obj["shape"])
        out = []
        if shape is None:
            return out
        if shape["kind"] == "compound":
            for index, child in enumerate(shape["children"]):
                cb = np.array(child["transform"]["basis"])
                co = np.array(child["transform"]["origin"])
                sub = self.walker.shapes.get(child["shape"])
                out.append((sub, P @ basis @ cb, P @ (basis @ co + origin), index))
        else:
            out.append((shape, P @ basis, P @ origin, 0))
        return out

    def shape_materials(self, obj):
        shape = self.walker.shapes.get(obj["shape"])
        wrap = self.walker.wrappers.get(shape.get("wrapper")) if shape else None
        if not wrap or not wrap["shapes_face_data"]:
            return None
        count = len(shape["children"]) if shape["kind"] == "compound" else 1
        raw = self.cap.read(wrap["shapes_face_data"], 2 * count)
        return np.frombuffer(raw, "<u2") if raw is not None else None


def key_of(pos, group, mask):
    return (round(float(pos[0]), 2), round(float(pos[1]), 2), round(float(pos[2]), 2), group, mask)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("export")
    ap.add_argument("--out")
    ap.add_argument("--tsv")
    args = ap.parse_args()
    lines = []

    def say(text=""):
        lines.append(text)
        print(text)

    game = Game(args.capture)
    project, libs, places = read_mdf(args.export)
    objects = game.world["objects"]
    say(f"game: {len(objects)} objects ({os.path.basename(args.capture)}); export: project {project}, "
        f"{len(places)} placements, {len(libs)} libraries")

    # ---- pairing: same place (map frame), same group and mask; several at one place (level pieces at the
    # origin) are told apart by their shape: the primitive count and the mesh's own vertices ---------------
    by_key = collections.defaultdict(list)
    for i, place in enumerate(places):
        by_key[key_of(place["pos"], place["group"], place["mask"])].append(i)
    obj_by_key = collections.defaultdict(list)
    for obj in objects:
        origin = P @ np.array(obj["transform"]["origin"])
        obj_by_key[key_of(origin, obj["group"], obj["mask"])].append(obj)

    def signature(obj):
        tris, prims, mesh = 0, 0, None
        for s, _, _, _ in game.parts(obj):
            if s is None:
                continue
            if s["kind"] == "triangle_mesh":
                verts, idx, _, _ = game.mesh(s)
                tris += len(idx)
                mesh = verts * np.array([1.0, 1.0, -1.0])  # the library's own space
            else:
                prims += 1
        return tris, prims, mesh

    def cost(sig, i):
        tris, prims, mesh = sig
        lib = libs[places[i]["lib"]]
        value = abs(int(lib["primitives"]) - prims) * 1e6 + abs(len(lib["F"]) - tris) * 1e-3
        if mesh is not None:
            n = len(mesh)
            same = len(lib["V"]) >= n and n > 0 and np.abs(lib["V"][:n] - mesh).max() < 1e-3
            value += 0 if same else 1e5
        return value

    pairs = []
    game_only = []
    used = set()
    for key, group in obj_by_key.items():
        candidates = by_key.get(key, [])
        if not candidates:
            game_only += group
            continue
        if len(group) == 1 and len(candidates) == 1:
            used.add(candidates[0])
            pairs.append((group[0], candidates[0]))
            continue
        sigs = [signature(obj) for obj in group]
        options = sorted((cost(sigs[a], i), a, i) for a in range(len(group)) for i in candidates)
        taken = set()
        for _, a, i in options:
            if a in taken or i in used:
                continue
            taken.add(a)
            used.add(i)
            pairs.append((group[a], i))
        game_only += [obj for a, obj in enumerate(group) if a not in taken]
    pairs.sort(key=lambda pair: pair[0]["index"])
    game_only.sort(key=lambda obj: obj["index"])
    export_only = [i for i in range(len(places)) if i not in used]
    say(f"paired {len(pairs)}; game objects with no export placement: {len(game_only)}; "
        f"export placements with no game object: {len(export_only)}")

    # ---- the unpaired ------------------------------------------------------------------------------------
    kinds = collections.Counter()
    for obj in game_only:
        shape = game.walker.shapes.get(obj["shape"], {})
        user = game.walker.users.get(obj["user"], {}).get("vtable") or 0
        kinds[(hex(obj["vtable"]), hex(user), obj["group"], obj["mask"], shape.get("kind"))] += 1
    say("\ngame objects with no export placement, by (object vtable, user vtable, group, mask, shape):")
    for key, n in kinds.most_common():
        say(f"  {n:5d}  {key}")
    # a game object at an export place but with another group/mask
    by_pos = collections.defaultdict(list)
    for i in export_only:
        by_pos[key_of(places[i]["pos"], 0, 0)].append(i)
    regroup = collections.Counter()
    for obj in game_only:
        origin = P @ np.array(obj["transform"]["origin"])
        for i in by_pos.get(key_of(origin, 0, 0), []):
            regroup[((places[i]["group"], places[i]["mask"]), (obj["group"], obj["mask"]))] += 1
    if regroup:
        say("same place, other group/mask (export -> game): " + str(dict(regroup)))
    say("\nexport placements with no game object, by (group, mask, primitives, triangles, library):")
    for i in export_only[:40]:
        lib = libs[places[i]["lib"]]
        say(f"  ({places[i]['group']}, {places[i]['mask']}) prim {lib['primitives']} tris {len(lib['F'])} "
            f"at {np.round(places[i]['pos'], 3).tolist()} {lib['name']}")
    if len(export_only) > 40:
        say(f"  ... {len(export_only) - 40} more")

    # ---- pair by pair ------------------------------------------------------------------------------------
    rows = []
    rot_diffs = []
    vert_stats = collections.Counter()
    worst_vertex = []
    material_walker = collections.defaultdict(collections.Counter)
    prim_material_walker = collections.defaultdict(collections.Counter)
    prim_rows = []
    shape_mismatch = collections.Counter()
    empty_libs = collections.Counter()
    lib_by_name = {lib["name"]: lib for lib in libs}
    for obj, pi in pairs:
        place = places[pi]
        lib = libs[place["lib"]]
        r_export = place["axes"].astype(float).T  # columns: images of x, y, z
        r_game = P @ np.array(obj["transform"]["basis"]) @ P
        rot = float(np.abs(r_export - r_game).max())
        rot_diffs.append(rot)
        pos = place["pos"].astype(float)
        parts = game.parts(obj)
        meshes = [(s, M, t) for s, M, t, _ in parts if s and s["kind"] == "triangle_mesh"]
        prims = [(s, M, t, i) for s, M, t, i in parts if s and s["kind"] != "triangle_mesh"]
        mesh_tris = 0
        mesh_verts = 0
        vmax = None
        index_same = None
        if meshes:
            shape, M, t = meshes[0]
            verts, idx, face, _ = game.mesh(shape)
            mesh_tris, mesh_verts = len(idx), len(verts)
            if len(lib["V"]) >= mesh_verts and len(lib["F"]) >= mesh_tris:
                world_game = verts @ M.T + t
                world_export = lib["V"][:mesh_verts].astype(float) @ r_export.T + pos
                vmax = float(np.linalg.norm(world_game - world_export, axis=1).max()) if mesh_verts else 0.0
                index_same = bool(np.array_equal(idx, lib["F"][:mesh_tris]))
                if face is not None:
                    # the exe's walker behaviour of a face (0x356D00): 0 for 0xFFFF, else the id's top two bits
                    game_walker = np.where(face == 0xFFFF, 0, face >> 14)
                    export_walker = lib["W"][:mesh_tris]
                    vert_stats["triangles with a material"] += len(face)
                    vert_stats["triangles whose walker behaviour differs"] += int((game_walker != export_walker).sum())
                    pairs_mw = np.unique(np.stack([face.astype(np.int64), export_walker.astype(np.int64)]), axis=1,
                                         return_counts=True)
                    for (material, walker), n in zip(pairs_mw[0].T.tolist(), pairs_mw[1].tolist()):
                        material_walker[material][walker] += n
                else:
                    vert_stats["meshes with no face data in the game"] += 1
                vert_stats["mesh pairs"] += 1
                vert_stats["vertices"] += mesh_verts
                if vmax > 0.001:
                    vert_stats["mesh pairs over 1 mm"] += 1
                    worst_vertex.append((vmax, obj["index"], lib["name"], rot))
                if not index_same:
                    vert_stats["mesh pairs with other indices"] += 1
            else:
                shape_mismatch[("mesh larger than the export library", lib["name"])] += 1
        elif len(lib["F"]) and lib["primitives"] == 0:
            shape_mismatch[("export has a mesh, the game has none", lib["name"])] += 1
        if len(lib["V"]) == 0 and (meshes or prims):
            # an empty export library (a folder link in the archive the export does not follow). For a
            # walker library the map builder stands the model's own `collision` in: is that what the game has?
            stand = None
            if "/walker_collision" in lib["name"]:
                stand = lib_by_name.get(lib["name"].replace("/walker_collision", "/collision"))
            if stand is None:
                verdict = "nothing in the export"
            else:
                same_prims = int(stand["primitives"]) == len(prims)
                if meshes:
                    own = game.mesh(meshes[0][0])[0] * np.array([1.0, 1.0, -1.0])
                    same_mesh = bool(len(stand["V"]) >= mesh_verts and mesh_verts > 0
                                     and np.abs(stand["V"][:mesh_verts] - own).max() < 1e-3)
                else:
                    same_mesh = len(components(stand["F"])) == int(stand["primitives"])
                verdict = "the stand-in (own collision) is what the game has" if same_prims and same_mesh else (
                    f"the stand-in DIFFERS: game {mesh_tris} mesh triangles + {len(prims)} primitives, own collision "
                    f"{len(stand['F'])} triangles in all, {stand['primitives']} primitives")
            empty_libs[(lib["name"].split("#")[0], verdict, obj["group"])] += 1
        # primitives: the export's faces after the mesh, one connected group each
        prim_faces = lib["F"][mesh_tris:]
        groups = components(prim_faces) if len(prim_faces) else []
        if len(groups) != len(prims) or lib["primitives"] != len(prims):
            shape_mismatch[(f"primitives: game {len(prims)}, export {lib['primitives']} "
                            f"({len(groups)} groups)", lib["name"])] += 1
        else:
            materials = game.shape_materials(obj)
            for (shape, M, t, child), group in zip(prims, groups):
                if shape["kind"] not in ("box", "sphere", "cylinder", "capsule"):
                    shape_mismatch[("primitive kind " + shape["kind"], lib["name"])] += 1
                    continue
                vidx = np.unique(prim_faces[group])
                world_export = lib["V"][vidx].astype(float) @ r_export.T + pos
                h_game = support_local(shape, DIRS @ M) + DIRS @ t
                h_export = (DIRS @ world_export.T).max(axis=1)
                out_game = float((h_game - h_export).max())    # the game's shape past the export's
                out_export = float((h_export - h_game).max())  # the export's past the game's
                size = float(max(shape["implicit_dims"][:3])) + shape["margin"]
                prim_rows.append((shape["kind"], out_game, out_export, size, obj["index"], lib["name"],
                                  tuple(round(x, 4) for x in shape["local_scaling"][:3]), obj["group"]))
                if materials is not None and child < len(materials):
                    material = int(materials[child])
                    game_walker = 0 if material == 0xFFFF else material >> 14
                    walkers = set(lib["W"][mesh_tris:][group].tolist())
                    vert_stats["primitives with a material"] += 1
                    if walkers != {game_walker}:
                        vert_stats["primitives whose walker behaviour differs"] += 1
                    for walker in walkers:
                        prim_material_walker[material][walker] += 1
        rows.append((obj["index"], pi, obj["group"], obj["mask"], lib["name"], rot, mesh_tris,
                     -1 if vmax is None else vmax, index_same, len(prims)))

    rot_diffs = np.array(rot_diffs)
    say(f"\nrotation (game basis against export matrix), {len(rot_diffs)} pairs: max {rot_diffs.max():.3g}; "
        f"over 1e-5: {(rot_diffs > 1e-5).sum()}; over 1e-3: {(rot_diffs > 1e-3).sum()}; "
        f"over 0.01: {(rot_diffs > 0.01).sum()}")
    say(f"meshes: {dict(vert_stats)}")
    for vmax, index, name, rot in sorted(worst_vertex, reverse=True)[:25]:
        say(f"  vertex off {vmax:8.4f} m  rotation diff {rot:.3g}  object {index}  {name}")
    say("\nempty export libraries the game has a shape for (model, verdict, group): placements")
    for (name, verdict, group), n in sorted(empty_libs.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        say(f"  {n:4d}  g{group}  {name}: {verdict}")
    say("\nshape mismatches:")
    for (what, name), n in sorted(shape_mismatch.items(), key=lambda kv: -kv[1])[:40]:
        say(f"  {n:4d}  {what}: {name}")

    say("\nprimitives: the game's exact convex shape against the export's triangles (metres; + = sticks out past)")
    for kind in ("box", "sphere", "cylinder", "capsule"):
        sel = [r for r in prim_rows if r[0] == kind]
        if not sel:
            continue
        a = np.array([r[1] for r in sel])
        b = np.array([r[2] for r in sel])
        say(f"  {kind:8s} {len(sel):6d}  game past export: median {np.median(a):+.4f} p95 {np.percentile(a, 95):+.4f} "
            f"max {a.max():+.4f} | export past game: median {np.median(b):+.4f} p95 {np.percentile(b, 95):+.4f} "
            f"max {b.max():+.4f} | either over 0.02 m: {int(((a > 0.02) | (b > 0.02)).sum())}, "
            f"over 0.05 m: {int(((a > 0.05) | (b > 0.05)).sum())}")
    say("  worst (kind, game past export, export past game, size, object, library, shape scaling, group):")
    for r in sorted(prim_rows, key=lambda r: -max(r[1], r[2]))[:30]:
        say(f"    {r[0]:8s} {r[1]:+8.4f} {r[2]:+8.4f} size {r[3]:7.3f} obj {r[4]:5d} g{r[7]} {r[6]} {r[5]}")

    say("\nmaterial id of a mesh triangle (game) -> walker behaviour of the same triangle (export): triangles")
    for material in sorted(material_walker):
        say(f"  material {material:5d}: {dict(material_walker[material])}")
    say("material id of a primitive (game) -> walker behaviour of its triangles (export): primitives")
    for material in sorted(prim_material_walker):
        say(f"  material {material:5d}: {dict(prim_material_walker[material])}")

    if args.out:
        with open(args.out, "w", newline="") as out:
            out.write("\n".join(lines) + "\n")
    if args.tsv:
        with open(args.tsv, "w", newline="") as out:
            out.write("object\tplacement\tgroup\tmask\tlibrary\trotation_diff\tmesh_triangles\tvertex_max_m\tindices_same\tprimitives\n")
            for row in rows:
                out.write("\t".join(str(x) for x in row) + "\n")
            out.write("# primitives: kind\tgame_past_export\texport_past_game\tsize\tobject\tlibrary\tscaling\tgroup\n")
            for r in prim_rows:
                out.write("#P\t" + "\t".join(str(x) for x in r) + "\n")


if __name__ == "__main__":
    main()
