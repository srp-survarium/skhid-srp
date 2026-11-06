#!/usr/bin/env python3
"""Offline sight survey, interactive alignment, and checked content-edit export."""

import argparse
import copy
from dataclasses import asdict
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess

import numpy as np
from PIL import Image

import model
import render

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
# A collimator's dot mesh: patch 21's glowing dot (red and green), the Walther Competition II's point.
# Not the EOTech's emissive reticle (a ring and a dot: its box centre is no pick of ours).
EMITTERS = ("glowing_dot", "reddot", "walther_comp2_point")
# `plan` centres a collimator whose dot is further off than this, px at 1080 lines (1.5 px at 720:
# about half the glowing dot, ~2.5 px there; the AK family's 1 px stays as it is).
TOLERANCE_PX_1080 = 2.25


def emitter(name):
    return any(word in name for word in EMITTERS)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def layers(args):
    result = ["--db", str(args.stock)]
    for line in args.patches.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        name, expected, size, *_ = line.split("\t")
        path = args.cache / name
        if path.stat().st_size != int(size) or digest(path) != expected:
            raise ValueError(f"{path}: differs from patch manifest")
        result += ["--patch", str(path)]
    return result


def extract(args):
    """Extract for inspection only. Never assemble a players' patch folder."""
    archives = layers(args)
    rows = subprocess.check_output(
        [str(args.bin), "files", *archives], text=True
    ).splitlines()
    prefixes = (
        "gameplay/items/weapons/",
        "gameplay/items/scopes/",
        "models/weapons/",
        "animations/skeletons/weapons/",
        "material_instances/weapons/",
        "textures/weapons/",
    )
    paths = []
    for line in rows:
        path = line.split("\t")[0]
        if (
            path.startswith(prefixes)
            or path == "gameplay/db_static_dictionaries"
            or path.startswith("animations/single/weapons/")
            and "/item/" in path
        ):
            paths.append(path)
    # The get command refuses to overwrite the destination.
    subprocess.run(
        [str(args.bin), "get", *archives, "--out", str(args.out), *paths], check=True
    )
    (args.out / "alignment-sources.json").write_text(
        json.dumps(
            {
                "stock": str(args.stock.resolve()),
                "stock_sha256": digest(args.stock),
                "patches": args.patches.read_text(),
                "files": paths,
            },
            indent=2,
        )
        + "\n"
    )


def catalog(root):
    items = model.config(root / "gameplay/db_static_dictionaries")["items_dict"]
    by_id = {row["dict_id"]: row for row in items}
    by_name = {row["name"]: row for row in items}
    weapons = [row for row in items if row["item_category"] in (8, 13, 14, 15, 16, 17)]
    scopes = [row for row in items if row["item_category"] == 22]
    result = []
    for row in weapons:
        cfg = model.config(root / row["cfg_name"])
        sid = cfg.get("addons", {}).get("rifle_scope_dict_id")
        scope = by_id[sid] if sid is not None else None
        result.append(
            {
                "id": row["name"] + "--stock",
                "weapon": row["name"],
                "weapon_file": row["cfg_name"],
                "cfg": cfg,
                "scope_file": scope["cfg_name"] if scope else None,
            }
        )
    for row in scopes:
        scope = model.config(root / row["cfg_name"])
        if "srp_scope_fit" not in scope:
            continue
        modification = scope["parameters"]["modification"]
        mods = [m for m in scope["modifications"] if m["id"] == modification]
        if len(mods) != 1:
            raise ValueError(f"{row['name']}: modification missing or ambiguous")
        for key in scope["srp_scope_fit"]["weapons"]:
            w = by_id[key] if isinstance(key, int) else by_name[key]
            cfg = copy.deepcopy(model.config(root / w["cfg_name"]))
            for mod in mods[0]["modifiers"]:
                path = mod["path"]
                target = cfg
                for part in path[:-1]:
                    target = target[part]
                if mod["type"] == 0:
                    target[path[-1]] = mod["value"]
                elif mod["type"] == 3:
                    name, value = mod["value"]
                    target[path[-1]][name] = value
                else:
                    raise ValueError(
                        f"{row['name']}: unsupported modifier {mod['type']}"
                    )
            result.append(
                {
                    "id": w["name"] + "--" + row["name"],
                    "weapon": w["name"],
                    "weapon_file": w["cfg_name"],
                    "cfg": cfg,
                    "scope_file": row["cfg_name"],
                }
            )
    return sorted(result, key=lambda row: row["id"])


def scene(root, entry, time):
    cfg = entry["cfg"]
    directory, rig, aim, _, body = model.weapon(root, cfg, time)
    optical, mount = [], None
    kind = "iron"
    zoom = 1 / cfg["parameters"]["aim_zoom_factor"]
    if entry["scope_file"]:
        scope = model.config(root / entry["scope_file"])["data"]
        kind = "scope" if scope["hide_weapon_on_aim"] else "collimator"
        zoom = scope["fov_factor"]
        mount = model.HALF @ model.locator_matrix(
            model.locator(directory, "scope_point"), rig
        )
        optical = model.meshes(model.model_dir(root, scope["aimed_model"]))
        for mesh in optical:
            mesh.points = model.transform(mesh.points, mount)
        if scope["hide_weapon_on_aim"]:
            body = []
    # Normalize into the aim locator's bone frame. The editor can then change
    # this locator using the actual retail matrix order without reposing meshes.
    parent = rig[aim.bone].pose if aim.bone is not None else np.eye(4)
    inverse = np.linalg.inv(parent)
    visible = body + optical
    for mesh in visible:
        mesh.points = model.transform(mesh.points, inverse)
    optical_names = [mesh.name for mesh in optical]
    candidates = [
        mesh for mesh in optical if emitter(mesh.name)
    ]
    # A textured grid's centre is not necessarily its zero (chevrons in particular).
    # Do not invent one or batch-centre it. The viewer permits an explicit pick.
    marker = (
        (candidates[0].points.min(0) + candidates[0].points.max(0)) / 2
        if len(candidates) == 1
        else None
    )
    props = directory / "render/export_properties"
    return {
        "id": entry["id"],
        "weapon": entry["weapon"],
        "kind": kind,
        "scope_file": entry["scope_file"],
        "weapon_file": entry["weapon_file"],
        "file": props.relative_to(root).as_posix(),
        "sha256": digest(props),
        "locator": asdict(aim),
        "offset": cfg["parameters"].get("aim_offset", 0),
        "fov": 65 * zoom,
        "time": time,
        # the optic as the weapon draws it, in the aim bone's frame (None for iron sights)
        "mount": mount @ inverse if mount is not None else None,
        "meshes": visible,
        "optical_names": optical_names,
        "marker": marker,
        "marker_basis": "emitter geometry centre; verify in game"
        if marker is not None
        else "manual pick required",
    }


def view_of(s, locator=None):
    loc = locator or s["locator"]
    camera = (
        model.matrix((0, 0, s["offset"]))
        @ model.HALF
        @ model.matrix(loc["position"], loc["rotation"])
    )
    return np.linalg.inv(camera) @ model.TO_VIEW


def error_pixels(s, height=1080):
    if s["marker"] is None:
        return None
    q = model.transform(s["marker"], view_of(s))
    if q[2] >= 0:
        return None
    f = height / 2 / math.tan(math.radians(s["fov"]) / 2)
    return [float(f * q[0] / -q[2]), float(-f * q[1] / -q[2])]


def review(args):
    if not math.isfinite(args.time) or args.time < 0:
        raise ValueError("animation time must be finite and nonnegative")
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / "scenes").mkdir()
    (args.out / "textures").mkdir()
    rows = catalog(args.live)
    if args.only:
        rows = [r for r in rows if args.only in r["id"]]
    if not rows:
        raise ValueError("no matching weapon–sight pair")
    summary = []
    textures = {}
    for row in rows:
        status = {"id": row["id"], "weapon": row["weapon"]}
        try:
            s = scene(args.live, row, args.time)
            status.update(
                {key: s[key] for key in ("kind", "file", "fov", "marker_basis")}
            )
            status["error_px_1080"] = error_pixels(s)
            status["status"] = "review" if s["marker"] is not None else "pick"
            meshes = []
            for mesh in s.pop("meshes"):
                if mesh.material not in textures:
                    name = (
                        hashlib.sha256(mesh.material.encode()).hexdigest()[:20] + ".png"
                    )
                    tex = render.texture([args.live, *args.textures], mesh.material)
                    Image.fromarray(tex).save(args.out / "textures" / name)
                    textures[mesh.material] = "textures/" + name
                points = np.round(mesh.points, 8).tolist()
                meshes.append(
                    {
                        "name": mesh.name,
                        "positions": points,
                        "triangles": mesh.triangles.tolist(),
                        "uv": np.round(mesh.uv, 6).tolist(),
                        "texture": textures[mesh.material],
                        "emitter": (
                            emitter(mesh.name)
                        ),
                        "glass": any(
                            word in mesh.name for word in ("lense", "lens", "glass")
                        ),
                    }
                )
            s["meshes"] = meshes
            s["marker"] = s["marker"].tolist() if s["marker"] is not None else None
            s.pop("mount")
            (args.out / "scenes" / (row["id"] + ".json")).write_text(
                json.dumps(s, separators=(",", ":"), allow_nan=False)
            )
        except (ValueError, KeyError, FileNotFoundError, IndexError) as exc:
            status.update(status="unsupported", reason=str(exc))
        summary.append(status)
        print(status["id"], status["status"], status.get("reason", ""), flush=True)
    for row in summary:
        row["shared"] = [
            r["id"] for r in summary if "file" in row and r.get("file") == row["file"]
        ]
    (args.out / "catalog.json").write_text(
        json.dumps(summary, indent=2, allow_nan=False) + "\n"
    )
    for name in ("index.html", "viewer.js"):
        shutil.copyfile(HERE / name, args.out / name)
    print(
        f"{len(summary)} pairs: {sum(r['status'] != 'unsupported' for r in summary)} inspectable; "
        f"{sum(r['status'] == 'unsupported' for r in summary)} explicitly unsupported"
    )
    print(
        f"Preview: python -m http.server --bind 127.0.0.1 --directory {args.out} 8765"
    )


def angles(m):
    """The locator angles whose model.rotation is m."""
    return [
        math.atan2(-m[1, 2], m[2, 2]),
        math.asin(max(-1.0, min(1.0, m[0, 2]))),
        math.atan2(-m[0, 1], m[0, 0]),
    ]


def on_axis(s):
    """The aim locator that sees a collimator's dot at the centre the way the reference variants do:
    looking along the optic's own axis (the gun's posed frame at scope_point: the identity on most
    guns, the item idle pose on the 9A-91), its position moved across that axis onto the dot's line.
    A stock attachment faces the other way on a scope_point turned half about Y (the Kriss's
    Walther): its axis is that frame turned back."""
    rotation = np.linalg.inv(model.HALF[:3, :3]) @ s["mount"][:3, :3]
    if (s["marker"] - np.array(s["locator"]["position"])) @ rotation[2] > 0:  # the camera looks along -row 2
        rotation = model.HALF[:3, :3] @ rotation
    loc = {"position": list(s["locator"]["position"]), "rotation": angles(rotation)}
    if np.abs(model.rotation(loc["rotation"]) - rotation).max() > 1e-6:
        raise ValueError(f"{s['id']}: the optic's frame is no rotation of locator angles")
    camera = (
        model.matrix((0, 0, s["offset"]))
        @ model.HALF
        @ model.matrix(loc["position"], loc["rotation"])
    )
    q = model.transform(s["marker"], np.linalg.inv(camera))
    if q[2] <= 0:
        raise ValueError(f"{s['id']}: the dot is behind the optic's axis")
    loc["position"] = (
        np.array(loc["position"]) + np.array([q[0], q[1], 0]) @ camera[:3, :3]
    ).tolist()
    return loc


def plan(args):
    """A plan centring every collimator whose dot is off by more than TOLERANCE_PX_1080, one
    change a model; every pair sharing a model must be a collimator that the change centres."""
    scenes = {}
    for row in catalog(args.live):
        s = scene(args.live, row, args.time)
        s.pop("meshes")
        scenes[row["id"]] = s
    by_file = {}
    for s in scenes.values():
        by_file.setdefault(s["file"], []).append(s)
    changes, report = [], []
    for file, group in sorted(by_file.items()):
        off = [
            s
            for s in group
            if s["kind"] == "collimator"
            and s["marker"] is not None
            and math.hypot(*error_pixels(s)) > TOLERANCE_PX_1080
        ]
        if not off:
            continue
        if any(s["kind"] != "collimator" or s["marker"] is None for s in group):
            raise ValueError(
                f"{file}: shared by {[s['id'] for s in group]}, not all collimators with a dot"
            )
        after = on_axis(off[0])
        for s in group:
            moved = dict(s, locator=after)
            residual = error_pixels(moved)
            if residual is None or math.hypot(*residual) > 0.05:
                raise ValueError(f"{s['id']}: {file}'s new eye leaves its dot at {residual}")
            report.append(
                "\t".join(
                    [s["id"], file]
                    + ["%+.1f" % (v * 720 / 1080) for v in error_pixels(s) + residual]
                )
            )
        before = {k: list(off[0]["locator"][k]) for k in ("position", "rotation")}
        changes.append(
            {
                "file": file,
                "sha256": off[0]["sha256"],
                "before": before,
                "after": after,
                "example": off[0]["id"],
                "marker": off[0]["marker"].tolist(),
                "verification": "offline; requires in-game check",
            }
        )
    with args.out.open("x") as f:
        json.dump(
            {
                "format": "skhid-sight-alignment-v1",
                "changes": changes,
                "source": json.loads((args.live / "alignment-sources.json").read_text())["patches"]
                .strip()
                .splitlines()[-1]
                .split("\t")[0],
                "status": f"every collimator off by more than {TOLERANCE_PX_1080 * 720 / 1080:.1f} px at 720 "
                "lines: its eye on the optic's axis",
            },
            f,
            indent=2,
        )
        f.write("\n")
    print("# pair\tmodel\tright\tdown\tright after\tdown after (px at 720 lines)")
    print("\n".join(report))
    print(f"{args.out}: {len(changes)} model(s), {len(report)} pair(s)")


def checked_edits(root, plan):
    if plan.get("format") != "skhid-sight-alignment-v1":
        raise ValueError("unknown alignment plan format")
    edits = []
    seen = set()
    for change in plan["changes"]:
        path = Path(change["file"])
        if (
            path.is_absolute()
            or ".." in path.parts
            or not path.as_posix().startswith("models/weapons/")
            or path.name != "export_properties"
        ):
            raise ValueError("plan may only edit a weapon model export_properties")
        if path in seen:
            raise ValueError(f"{path}: conflicting shared-model edits")
        seen.add(path)
        source = root / path
        if digest(source) != change["sha256"]:
            raise ValueError(f"{path}: source hash changed; reopen the review")
        before = change["before"]
        after = change["after"]
        if set(before) != {"position", "rotation"} or set(after) != set(before):
            raise ValueError("only aim position and rotation may change")
        for values in [*before.values(), *after.values()]:
            if (
                not isinstance(values, list)
                or len(values) != 3
                or not all(
                    isinstance(v, (int, float))
                    and not isinstance(v, bool)
                    and math.isfinite(v)
                    for v in values
                )
            ):
                raise ValueError("locator vectors must contain three finite numbers")
        edit = {
            "file": path.as_posix(),
            "list": "locators",
            "where": {"name": "aim"},
            "from": before,
            "to": after,
        }
        data = bytearray(source.read_bytes())
        original = model.ce.walk(data)
        index = model.ce.set_entry(data, "locators", {"name": "aim"}, before, after)
        # Independent walk: editing the aim transform cannot modify another locator.
        walked = model.ce.walk(data)
        allowed = {f"/locators[{index}]/position", f"/locators[{index}]/rotation"}
        if len(walked) != len(original) or any(
            a != b and a[0] not in allowed for a, b in zip(original, walked)
        ):
            raise ValueError("unexpected values changed")
        edits.append(edit)
    if not edits:
        raise ValueError("plan contains no changes")
    return {
        "_about": "Sight alignment (scripts/sight-alignment, align.py export): aim locator changes only, visual, "
        "no ballistics changes"
        + (f"; {plan['status']}" if plan.get("status") else "")
        + ".",
        "set_entry": edits,
    }


def export(args):
    edits = checked_edits(args.live, json.loads(args.plan.read_text()))
    with args.out.open("x") as f:
        json.dump(edits, f, indent=2)
        f.write("\n")
    print(f"{args.out}: {len(edits['set_entry'])} checked model edit(s)")
    print(
        "Add its edit row to release/content.tsv when accepting the correction, then use release/build-content."
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser(
        "extract", help="extract the exact released layers for inspection"
    )
    p.add_argument("--stock", required=True, type=Path)
    p.add_argument("--patches", type=Path, default=ROOT / "release/patches.tsv")
    p.add_argument(
        "--cache", type=Path, default=Path.home() / ".cache/skhid-release/patches"
    )
    p.add_argument(
        "--bin", type=Path, default=ROOT / "srp-026g/target/release/srp-game-data"
    )
    p.add_argument("--out", type=Path, required=True)
    p.set_defaults(run=extract)
    p = commands.add_parser(
        "review", help="build an offline viewer and a complete pair inventory"
    )
    p.add_argument("--live", required=True, type=Path)
    p.add_argument("--textures", type=Path, action="append", default=[])
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--only")
    p.add_argument("--time", type=float, default=0)
    p.set_defaults(run=review)
    p = commands.add_parser(
        "plan", help="a plan centring every collimator whose dot is off, eye on the optic's axis"
    )
    p.add_argument("--live", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--time", type=float, default=0)
    p.set_defaults(run=plan)
    p = commands.add_parser(
        "export", help="validate a viewer plan and emit a content edit list"
    )
    p.add_argument("--live", required=True, type=Path)
    p.add_argument("--plan", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    p.set_defaults(run=export)
    args = parser.parse_args()
    try:
        args.run(args)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as exc:
        parser.exit(1, str(exc) + "\n")


if __name__ == "__main__":
    main()
