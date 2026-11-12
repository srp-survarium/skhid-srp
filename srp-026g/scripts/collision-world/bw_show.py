"""Shows one or more game objects of a capture beside the export library the diff paired them with.

    python bw_show.py <capture.bwc> <export.mdf> <substring of the library name> [...]
"""
import collections
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_diff  # noqa: E402


def main():
    capture, export, *needles = sys.argv[1:]
    game = bw_diff.Game(capture)
    project, libs, places = bw_diff.read_mdf(export)
    P = bw_diff.P
    by_key = collections.defaultdict(list)
    for i, place in enumerate(places):
        by_key[bw_diff.key_of(place["pos"], place["group"], place["mask"])].append(i)
    shown = set()
    for obj in game.world["objects"]:
        origin = P @ np.array(obj["transform"]["origin"])
        for i in by_key.get(bw_diff.key_of(origin, obj["group"], obj["mask"]), []):
            lib = libs[places[i]["lib"]]
            if not any(n in lib["name"] for n in needles) or lib["name"] in shown:
                continue
            shown.add(lib["name"])
            print(f"\n{lib['name']}  (object {obj['index']}, group {obj['group']}, mask {obj['mask']}, at {np.round(origin, 2).tolist()})")
            print(f"  export library: {len(lib['V'])} vertices, {len(lib['F'])} triangles, {lib['primitives']} primitives"
                  + (f", bbox {np.round(lib['V'].min(0), 3).tolist()} .. {np.round(lib['V'].max(0), 3).tolist()}" if len(lib["V"]) else ""))
            shape = game.walker.shapes.get(obj["shape"])
            print(f"  game shape: {shape['kind']}, wrapper {shape.get('wrapper', 0):#x}")
            for sub, M, t, child in game.parts(obj):
                if sub["kind"] == "triangle_mesh":
                    verts, idx, face, scaling = game.mesh(sub)
                    raw = verts / scaling
                    part = game.walker.meshes[sub["mesh"]]["parts"][0]
                    mats = collections.Counter(face.tolist()).most_common(4) if face is not None else None
                    print(f"    mesh: {len(verts)} vertices, {len(idx)} triangles, scaling {scaling.tolist()}, raw bbox "
                          f"{np.round(raw.min(0), 3).tolist()} .. {np.round(raw.max(0), 3).tolist()}, vertex buffer "
                          f"{part['vertex_base']:#x}, materials {mats}")
                else:
                    print(f"    {sub['kind']}: dims {np.round(sub['implicit_dims'][:3], 4).tolist()} margin {sub['margin']:.4f} "
                          f"scaling {np.round(sub['local_scaling'][:3], 3).tolist()} at {np.round(t, 3).tolist()}")


if __name__ == "__main__":
    main()
