#!/usr/bin/env python3
"""Builds the website's Maps tab (`/maps`) from the game as Skhid's players have it.

    python3 -I build.py --stock <resources.db> --patches <dir> --dbgrab <dbgrab> --ffdec <ffdec-cli.jar>
                        --icons <dir> [--list release/patches.tsv] [--out srp-026g/servers/web-server/maps]

The patches are exactly the rows of release/patches.tsv (each checked by its
sha256), mounted over the stock resources.db in their order, as the game does.
Nothing about a map is written here by hand: the maps are the playable ids of
`maps_dict` (a config of mode 2, 3 or 4 whose project exists, as the server
plays them), and of each map

- the name, the modes and their parameters: `gameplay/game_modes/`;
- respawn points, battery spawners, charging stations ("chargers") and the
  teams' battery containers: the project's `server_objects`;
- artefact containers and what spawns there: the project's `client_objects`
  (`artefact_container`, grouped by their `generic_anomaly_core`);
- the minimap and its frame: `flash_movies/hud.swf`, class
  `survarium.controls.Minimap` (`_minimapProps`, `_levels`), keyed by the
  project's `project_name` as the HUD does;
- the top-down render: srp-026g/scripts/damage-calc/assets/maps/ (maps.json
  and the webp in servers/web-server/damage/maps/), the one thing not read
  from the game. A map without a render fails the build: render it first
  (that folder's README).

Every weather of a map and mode must have the same objects (they do); if a
patch makes them differ the build stops rather than pick one.

Writes <out>/maps.html ({{LANG}} and <!--SITE-HEADER--> for the server),
<out>/maps.js (data and UI), <out>/minimap/<project_name>.webp and
<out>/files.rs (those files, for the server's include) and
<out>/built-from.tsv (the patches it was built from: release/release.sh
rebuilds the page, through release/build-site, when it offers others; --with
<a patch> previews one not listed yet). Needs pillow, java on PATH and the
icons the damage calculator's build exports (assets/tools/build_icons.py).
"""
import argparse
import hashlib
import json
import math
import re
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
sys.path.insert(0, str(SRP / "scripts" / "released-game"))
from released import LIST, die, extract, offered, revision as revision_of, write_built_from  # noqa: E402
from game_maps import WANTED, hud_minimaps, load, playable  # noqa: E402

TYPES = {"js": "text/javascript; charset=utf-8", "webp": "image/webp", "svg": "image/svg+xml"}
# The rule classes (a mode config's `rules`) that use the battery objects
# (docs/notes/modes_maps.md section 4): gather_victory_items (88) the spawners and
# containers; match_time_vi_spawn (89) those and the charging stations. A
# project can hold objects no rule of its mode reads (level_05 has a station
# in modes 2 and 4): those are not shown.
GATHER, TIMED = 88, 89
SITE = "/maps"
RENDERS = SRP / "servers" / "web-server" / "damage" / "maps"
RENDER_TABLE = SRP / "scripts" / "damage-calc" / "assets" / "maps" / "maps.json"


def version(blob):
    return hashlib.sha256(blob).hexdigest()[:12]


# The HUD's own icons, frame 1 of each class: the minimap's (Minimap._icons)
# and the battery, the game's render of it that the HUD shows a carrier
# (SrvContainerSymbol: the game calls the battery a container).
HUD_ICONS = {"player": "PlayerBlue", "base": "Base", "charger": "SpawnIcon", "resupply": "ResupplyIcon",
             "battery": "SrvContainerSymbol"}


def lightness(svg):
    """The icon's bitmaps: how light its faces are (the 75th percentile of
    the visible pixels' lightness) and how opaque it is at most, so the page
    can paint each in a team's colour alike (the Base's arrow is grey, the
    crate half transparent)."""
    import base64
    import io
    import numpy as np
    pixels = []
    for data in re.findall(r"base64,([A-Za-z0-9+/=]+)", svg):
        im = np.asarray(Image.open(io.BytesIO(base64.b64decode(data))).convert("RGBA"), dtype=float) / 255
        pixels.append(im.reshape(-1, 4))
    if not pixels:
        return 1.0, 1.0
    px = np.concatenate(pixels)
    seen = px[px[:, 3] > 0.05]
    light = 0.3 * seen[:, 0] + 0.59 * seen[:, 1] + 0.11 * seen[:, 2]
    return round(float(np.percentile(light, 75)), 3), round(float(seen[:, 3].max()), 3)


def hud_icons(args, raw, work, out):
    """{name: {src, w, h, ax, ay}}: each icon as an SVG file and its centre."""
    swf = raw / "flash_movies__hud.swf"
    target = work / "sprites"
    subprocess.run(["java", "-jar", args.ffdec, "-format", "sprite:svg", "-selectclass", ",".join(HUD_ICONS.values()),
                    "-export", "sprite", str(target), str(swf)], check=True, capture_output=True)
    frame = lambda cls: next((d / "1.svg" for d in target.iterdir() if d.name.endswith(f"_{cls}_{cls}")), None)
    found = {name: frame(cls) for name, cls in HUD_ICONS.items()}
    if not all(found.values()):
        die("hud.swf: a minimap icon is missing (" + ", ".join(c for n, c in HUD_ICONS.items() if not found[n]) + ")")
    icons = {}
    (out / "icons").mkdir(parents=True, exist_ok=True)
    for name, path in found.items():
        svg = re.sub(r'\s(ffdec:\w+)="[^"]*"', "", path.read_text())
        width = float(re.search(r'<svg[^>]*\swidth="([\d.]+)', svg).group(1))
        height = float(re.search(r'<svg[^>]*\sheight="([\d.]+)', svg).group(1))
        # Centred on the point (the symbols' own origins differ: the battery's
        # is its corner).
        ax, ay = width / 2, height / 2
        (out / "icons" / f"hud_{name}.svg").write_text(svg)
        light, opaque = lightness(svg)
        icons[name] = {"src": f"{SITE}/icons/hud_{name}.svg?v={version(svg.encode())}",
                       "w": width, "h": height, "ax": ax, "ay": ay, "light": light, "opaque": opaque}
    return icons


def localization(raw):
    return {lang: load(raw, f"localization/{name}/localization")["strings"]
            for lang, name in (("en", "english"), ("uk", "ukrainian"))}


def artefact_names(texts, key):
    """The names the game's pickup hints give: "Onyx artifact. ..." and "Артефакт Онікс. ..."."""
    hint = f"st_{key}_pickup_hint"
    en = re.match(r"\s*(.+?) artifact\.", texts["en"].get(hint, ""))
    uk = re.match(r"\s*Артефакт (.+?)\.", texts["uk"].get(hint, ""))
    if not (en and uk):
        die(f"{hint}: no artefact name in its text")
    return {"en": en.group(1), "uk": uk.group(1)}


def r1(v):
    return round(v, 1)


def xzy(o):
    x, y, z = o["position"]
    return [r1(x), r1(z), r1(y)]


def heading(o):
    """Which way a respawn point faces, as the SVG rotation of the HUD's
    player triangle (it points down, +y): the engine's yaw is rotation y,
    turned by pi when x and z are both pi; forward is (-sin yaw, cos yaw) in
    x/z. Checked on every map: the points face the enemy's side, at 60
    degrees from its spawns on average."""
    rx, ry, rz = o.get("rotation") or (0, 0, 0)
    yaw = math.pi - ry if abs(abs(rx) - math.pi) < 0.1 and abs(abs(rz) - math.pi) < 0.1 else ry
    vx, vz = -math.sin(yaw), math.cos(yaw)
    return round(math.degrees(math.atan2(-vz, vx)) - 90) % 360


def objects(project, artefact_keys):
    """What the tab shows of one project."""
    so = project.get("server_objects") or {}
    spawners = so.get("victory_item_spawners") or []
    stations = [xzy(o) for o in so.get("victory_item_spawn_points") or []]
    client = project.get("client_objects") or []
    kinds_of = defaultdict(set)
    for o in client:
        if o.get("game_object_type") != "generic_anomaly_core":
            continue
        kinds = {artefact_keys[a["dict_id"]] for a in o.get("artefacts") or [] if a["dict_id"] in artefact_keys}
        for name in o.get("artefact_containers") or []:
            kinds_of[name] |= kinds
    # A container's own position is its anomaly's pivot, shared by all of
    # them; where the artefact is found is its collision geometry (the zone
    # the search runs in).
    by_name = {o.get("full_name"): o for o in client}
    artefacts = []
    for o in client:
        if o.get("game_object_type") == "artefact_container":
            kinds = sorted(kinds_of.get(o.get("full_name"), ()))
            zones = [by_name[c["name"]] for c in o.get("collision_geometries") or [] if c.get("name") in by_name]
            if kinds:
                artefacts.append(xzy(zones[0] if zones else o) + [kinds])
    # A team's container and its stash are where their collision zone is:
    # the object's own position can be a pivot far away (level_04's two
    # containers both stand at one team's side by it).
    def zone_of(o):
        zones = [by_name[c["name"]] for c in o.get("collision_geometries") or [] if c.get("name") in by_name]
        return zones[0] if zones else o
    resupply = [xzy(zone_of(o)) + [o.get("team", -1)] for o in client if o.get("game_object_type") == "resupply_zone"]
    return {
        "respawn": sorted(xzy(o) + [o.get("team", -1), heading(o)] for o in so.get("respawn_points") or []),
        "spawners": [xzy(o) for o in spawners],
        "stations": stations,
        "containers": sorted(xzy(zone_of(o)) + [o.get("team", -1)] for o in so.get("victory_items_containers") or []),
        "resupply": sorted(resupply),
        "artefacts": sorted(artefacts),
    }


def webp(png, out):
    im = Image.open(png).convert("RGBA")
    out.parent.mkdir(parents=True, exist_ok=True)
    im.save(out, "WEBP", quality=92, method=6)
    return im.getbbox(), im.size


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--stock", required=True)
    ap.add_argument("--patches", required=True, help="the folder with the list's resources.db.patch_<n>")
    ap.add_argument("--list", default=str(LIST))
    ap.add_argument("--with", dest="with_patch", metavar="PATCH",
                    help="the next release's resources.db.patch_<n>, not in the list yet")
    ap.add_argument("--dbgrab", required=True)
    ap.add_argument("--ffdec", required=True, help="ffdec-cli.jar")
    ap.add_argument("--icons", required=True, help="the lobby icons as <config name>.webp")
    ap.add_argument("--out", default=str(SRP / "servers" / "web-server" / "maps"))
    ap.add_argument("--allow-missing-renders", action="store_true", help="a map without a render shows its minimap only")
    ap.add_argument("--work", help="keep the extracted files here and reuse them (while working on the page)")
    args = ap.parse_args()
    rows = offered(args.list, args.with_patch)
    revision = revision_of(rows)
    out = Path(args.out)

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(args.work or tmp)
        raw = tmp / "raw"
        if not (args.work and raw.is_dir()):
            raw.mkdir(parents=True, exist_ok=True)
            extract(args.dbgrab, args.stock, rows, args.patches, WANTED, raw)
        texts = localization(raw)
        text = lambda key: {lang: texts[lang].get(key, key) for lang in texts}
        dictionary = load(raw, "gameplay/db_static_dictionaries")
        artefact_keys = {i["dict_id"]: i["name"].removeprefix("artefact_") for i in dictionary["items_dict"]
                         if i["cfg_name"].startswith("gameplay/items/artefacts/")}

        # The playable ids, by map (the project's folder) and mode.
        maps = playable(raw)

        minimaps = hud_minimaps(args.ffdec, raw, tmp)
        for sub in ("minimap", "icons"):
            shutil.rmtree(out / sub, ignore_errors=True)
        hud = hud_icons(args, raw, tmp, out)
        renders = json.loads(RENDER_TABLE.read_text())["maps"]
        out.mkdir(parents=True, exist_ok=True)
        data_maps, missing = [], []
        used_artefacts = set()
        for folder in sorted(maps, key=lambda f: (f.startswith("v"), f)):
            m = maps[folder]
            modes, project_name, artefacts = [], None, None
            for mode_id in sorted(m["modes"]):
                mm = m["modes"][mode_id]
                seen = None
                for path in mm["projects"]:
                    project = load(raw, f"projects/{path}/client_project")
                    got = objects(project, artefact_keys)
                    if seen is not None and got != seen:
                        die(f"{path}: its objects differ from {mm['projects'][0]}'s (same map and mode)")
                    seen = got
                    project_name = project.get("project_name") or folder
                if artefacts is not None and seen["artefacts"] != artefacts:
                    die(f"{folder}: artefact containers differ between modes")
                artefacts = seen.pop("artefacts")
                rules = mm["rules"]
                if not rules & {GATHER, TIMED}:
                    seen["spawners"], seen["containers"] = [], []
                if TIMED in rules:
                    # The batteries come out of the chargers: their spawn
                    # points are the chargers' own boxes, not places to show.
                    seen["spawners"] = []
                else:
                    seen["stations"] = []
                p = mm["parameters"]
                modes.append({"mode": mode_id, "ids": sorted(mm["ids"]), "name": text(mm["mode_name"]),
                              "stations_active": p.get("spawn_points_count"), "batteries": p.get("victory_items_count"),
                              **seen})
            for a in artefacts:
                used_artefacts.update(a[3])
            if project_name not in minimaps:
                die(f"{folder}: hud.swf has no minimap for project_name {project_name}")
            frame, bitmap = minimaps[project_name]
            name = f"{project_name}.webp"
            bbox, size = webp(bitmap, out / "minimap" / name)
            blob = (out / "minimap" / name).read_bytes()
            s, px, py = (float(frame[k]) for k in ("size", "posX", "posY"))
            k = 2 * s / size[0]
            # World rectangles as [x0, -z1, x1, -z0]: the SVG's x right, y down = -z.
            box = [r1(px - s), r1(-py - s), r1(px + s), r1(-py + s)]
            play = [r1(px - s + bbox[0] * k), r1(-py - s + bbox[1] * k), r1(px - s + bbox[2] * k), r1(-py - s + bbox[3] * k)]
            entry = {"key": folder, "name": text(m["map_name"]), "box": box, "play": play,
                     "minimap": f"{SITE}/minimap/{name}?v={version(blob)}", "modes": modes, "artefacts": artefacts}
            render = renders.get(folder)
            if render and (RENDERS / render["image"]).is_file():
                a, b, c, d, e, f = render["pixel_to_world_affine"]
                w, h = render["image_size_px"]
                blob = (RENDERS / render["image"]).read_bytes()
                entry["render"] = {"src": f"/damage/maps/{render['image']}?v={version(blob)}",
                                   "box": [r1(c), r1(-f), r1(a * w + c), r1(-(e * h + f))]}
                # The render's play area (the native minimap's, per map: that
                # folder's play_area.py) where it has one: some minimaps are
                # opaque to their edge.
                if render.get("play_area_px"):
                    x0, y0, x1, y1 = render["play_area_px"]
                    entry["play"] = [r1(a * x0 + c), r1(-(e * y0 + f)), r1(a * x1 + c), r1(-(e * y1 + f))]
            else:
                missing.append(folder)
            data_maps.append(entry)
        if missing and not args.allow_missing_renders:
            die("no top-down render of " + ", ".join(missing)
                + " (srp-026g/scripts/damage-calc/assets/maps/README.md; then the site's copy in servers/web-server/damage/maps/)")

        icons = {}
        for key in sorted(used_artefacts):
            icon = Path(args.icons) / f"artefact_{key}.webp"
            if not icon.is_file():
                die(f"{icon}: no such icon")
            icons[key] = {"name": artefact_names(texts, key)}
            # The lobby icon cut to the artefact, on a square.
            im = Image.open(icon).convert("RGBA")
            im = im.crop(im.getbbox())
            side = max(im.size)
            square = Image.new("RGBA", (side, side))
            square.paste(im, ((side - im.width) // 2, (side - im.height) // 2))
            (out / "icons").mkdir(exist_ok=True)
            square.save(out / "icons" / f"{key}.webp", "WEBP", quality=92, method=6)
            blob = (out / "icons" / f"{key}.webp").read_bytes()
            icons[key]["icon"] = f"{SITE}/icons/{key}.webp?v={version(blob)}"

    data = {"revision": revision, "maps": data_maps, "artefacts": icons, "hud": hud}
    script = ('"use strict";\n// Built by srp-026g/scripts/maps-tab/build.py; do not edit.\n'
              f"const MAPS = {json.dumps(data, ensure_ascii=False, separators=(',', ':'))};\n"
              + (HERE / "ui.js").read_text())
    (out / "maps.js").write_text(script)
    template = (HERE / "template.html").read_text()
    faces = "\n".join(line for line in (SRP / "servers" / "web-server" / "damage" / "damage.html").read_text().splitlines()
                      if line.startswith("@font-face"))
    page = (template.replace("/*__FONT__*/", faces)
            .replace("__SCRIPT__", f"{SITE}/maps.js?v={version(script.encode())}"))
    (out / "maps.html").write_text(page)
    served = ["maps.js"] + [f"{sub}/{f.name}" for sub in ("minimap", "icons") for f in sorted((out / sub).iterdir())]
    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/maps-tab/build.py; do not edit. The files maps.html names.\n"
        "files! {\n" + "".join(
            f'    "{f}" => "{TYPES[f.rsplit(".", 1)[1]]}",\n' for f in served)
        + "}\n")
    write_built_from(out / "built-from.tsv", rows, "the Maps tab (srp-026g/scripts/maps-tab/build.py)")
    points = sum(len(mo["respawn"]) + len(mo["spawners"]) + len(mo["stations"]) + len(mo["containers"])
                 for m in data_maps for mo in m["modes"]) + sum(len(m["artefacts"]) for m in data_maps)
    print(f"{out}: revision {revision}, {len(data_maps)} maps, {points} points, maps.js {len(script) / 1024:.0f} KB")


if __name__ == "__main__":
    main()
