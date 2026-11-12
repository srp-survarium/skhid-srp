"""Reads a capture (bwmem.Capture) and prints what the game's Bullet world holds.

    python bw_report.py <capture.bwc> [--json out.json] [--objects N]

The JSON holds every object (transform, group, mask, shape address) and every shape (kind, dims, scaling,
margin, children, mesh parts), with the triangle meshes left in the capture (read them with bw_geom).
"""
import argparse
import collections
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_walk  # noqa: E402
import bwmem  # noqa: E402


def load(path):
    cap = bwmem.Capture(path)
    delta = cap.meta["module_base"] - 0x10000
    walker = bw_walk.Walker(cap, delta, chase=False)
    world = walker.world(cap.meta["world"])
    return cap, walker, world


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--json")
    ap.add_argument("--objects", type=int, default=0)
    args = ap.parse_args()
    cap, walker, world = load(args.capture)
    meta = cap.meta
    print(f"capture of pid {meta['pid']} taken {meta['taken_utc']} ({meta.get('note', '')})")
    print(f"exe sha256 {meta['exe_sha256'][:16]}..., module base {meta['module_base']:#x}, replay {os.path.basename(meta['replay'])}")
    print(f"world {world['world']:#x}, dynamics world vtable {world['dynamics_vtable']:#x}, "
          f"aabb {world['aabb_min']} .. {world['aabb_max']}")
    objects = world["objects"]
    print(f"objects: {len(objects)}; shapes: {len(walker.shapes)}; mesh interfaces: {len(walker.meshes)}; "
          f"wrappers: {len(walker.wrappers)}; problems now: {len(walker.problems)}")
    for line in walker.problems[:20]:
        print("  problem:", line)

    def table(title, counter, limit=40):
        print(f"\n{title}")
        for key, n in counter.most_common(limit):
            print(f"  {n:6d}  {key}")

    table("objects by vtable / internal type / flags",
          collections.Counter((hex(o["vtable"]), o["internal_type"], hex(o["flags"])) for o in objects))
    table("objects by group / mask",
          collections.Counter((o["group"], o["mask"]) for o in objects))
    table("objects by user object vtable",
          collections.Counter(hex(walker.users.get(o["user"], {}).get("vtable") or 0) for o in objects))
    table("objects by shape kind",
          collections.Counter(walker.shapes.get(o["shape"], {}).get("kind", "none") for o in objects))
    table("shapes by kind / vtable",
          collections.Counter((s.get("kind"), hex(s.get("vtable", 0))) for s in walker.shapes.values()))
    table("margins by kind",
          collections.Counter((s.get("kind"), round(s["margin"], 5)) for s in walker.shapes.values()
                              if "margin" in s))
    table("mesh interface scaling",
          collections.Counter(tuple(round(x, 4) for x in m["scaling"]) for m in walker.meshes.values()), 25)
    table("convex / compound local scaling",
          collections.Counter((s["kind"], tuple(round(x, 4) for x in s["local_scaling"]))
                              for s in walker.shapes.values() if "local_scaling" in s), 25)
    tris = sum(p["triangles"] for m in walker.meshes.values() for p in m["parts"])
    verts = sum(p["vertices"] for m in walker.meshes.values() for p in m["parts"])
    print(f"\nmesh parts: {sum(len(m['parts']) for m in walker.meshes.values())}, triangles {tris}, vertices {verts}")
    table("mesh part layout (index stride, vertex stride, index type, vertex type)",
          collections.Counter((p["index_stride"], p["vertex_stride"], p["index_type"], p["vertex_type"])
                              for m in walker.meshes.values() for p in m["parts"]))
    shared = collections.Counter(o["shape"] for o in objects)
    print(f"\nshapes used by more than one object: {sum(1 for n in shared.values() if n > 1)} "
          f"(most: {shared.most_common(3)})")
    for o in objects[: args.objects]:
        print(o)
        print("   ", walker.shapes.get(o["shape"]))
    if args.json:
        with open(args.json, "w", newline="") as out:
            json.dump({"meta": meta, "world": world, "shapes": walker.shapes, "meshes": walker.meshes,
                       "wrappers": walker.wrappers, "users": walker.users}, out)
        print(f"\nwrote {args.json}")


if __name__ == "__main__":
    main()
