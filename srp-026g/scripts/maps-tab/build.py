#!/usr/bin/env python3
"""Builds the website's maps (`/wiki/maps`) from the game as Skhid's players have it.

    python3 -I build.py --stock <resources.db> --patches <dir> --dbgrab <dbgrab> --ffdec <ffdec-cli.jar>
                        --icons <dir> [--list release/patches.tsv] [--out srp-026g/servers/web-server/maps]

The patches are exactly the rows of release/patches.tsv (each checked by its
sha256), mounted over the stock resources.db in their order, as the game does.
Nothing about a map is written here by hand: the maps are the playable ids of
`maps_dict` (a config of mode 2 to 5, 8, 12 or 13 whose project exists, as the server
plays them), and of each map

- the name, the modes and their parameters: `gameplay/game_modes/`;
- respawn points, battery spawners, charging stations ("chargers") and the
  teams' battery containers: the project's `server_objects` (of a free for
  all, mode 8, the team-2 respawn points alone: its players' only spawns);
- Research's three units (mode 5): the mode config's
  `parameters.capture_zones` (`cp_id`, `position`);
- Artifact Hunt's artefact spawns and the teams' bases (mode 13): the mode
  config's `artefact_hunt.spawners` and `.bases` (`team`, `position`);
- artefact containers and what spawns there: the project's `client_objects`
  (`artefact_container`, grouped by their `generic_anomaly_core`);
- the anomalies (`anomaly`, `objects`): each `generic_anomaly_core`'s pool, timers,
  chance and cap, its `damage_zone`s' hits and their `collision_geometry` seen
  from above (docs/notes/maps_tab.md, "Anomalies");
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
<a patch> previews one not listed yet). Needs pillow, java on PATH, the
icons the damage calculator's build exports (assets/tools/build_icons.py) and
the wiki's mode icons it makes (servers/web-server/wiki/icons/modes/, by
scripts/wiki/build_icons.py): release/build-site builds the calculator first.
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
from game_maps import ARTEFACT_HINT, BUILDS, FREE_FOR_ALL, WANTED, hud_minimaps, load, playable  # noqa: E402
sys.path.insert(0, str(SRP / "scripts" / "site-texts"))
from texts import CODES, DICTIONARIES, WORDS, Texts  # noqa: E402

TYPES = {"js": "text/javascript; charset=utf-8", "webp": "image/webp", "svg": "image/svg+xml"}
# The rule classes (a mode config's `rules`) that use the battery objects
# (docs/notes/modes_maps.md section 4): gather_victory_items (88) the spawners and
# containers; match_time_vi_spawn (89) those and the charging stations. A
# project can hold objects no rule of its mode reads (level_05 has a station
# in modes 2 and 4): those are not shown.
GATHER, TIMED = 88, 89
# Research's rule (480, exe patch 53; docs/notes/research_mode.md) reads its
# units from the mode config (`parameters.capture_zones`), not the project.
RESEARCH = 480
# Artifact Hunt's rule (481, exe patch 171; docs/notes/artefact_hunt.md) reads
# the artefact's spawn points and the teams' bases from the mode config
# (`artefact_hunt.spawners`, `.bases`), not the project.
HUNT = 481
SITE = "/maps"
RENDERS = SRP / "servers" / "web-server" / "damage" / "maps"
# Maps released before their top-down render is made: the tab shows their minimap
# only, said in the build's output (the owner, 2026-11-27: rev75 ships Vostok (v0.69)
# without one). Take a map out once its render is in.
RENDER_PENDING = {"v069d_level_03"}
RENDER_TABLE = SRP / "scripts" / "damage-calc" / "assets" / "maps" / "maps.json"


def version(blob):
    return hashlib.sha256(blob).hexdigest()[:12]


# The HUD's own icons, frame 1 of each class: the minimap's (Minimap._icons).
HUD_ICONS = {"player": "PlayerBlue", "base": "Base", "charger": "SpawnIcon", "resupply": "ResupplyIcon"}
# The battery's: the Protective device mode's own icon (mode 2; the owner,
# 2026-11-27: "for batteries let's use 'protective device' mode icon on wiki"),
# the lobby's loading-screen frame `ui.icon` + 1 of SrvGameModeIcons as the
# wiki's modes page shows it, the very file (/wiki/icons/modes/, its bitmap
# 61x55 in the movie, written 64 square by scripts/wiki/build_icons.py).
PROTECTIVE_DEVICE = 2
MODE_ICONS = SRP / "servers" / "web-server" / "wiki" / "icons" / "modes"


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
        # Centred on the point (the symbols' own origins differ).
        ax, ay = width / 2, height / 2
        (out / "icons" / f"hud_{name}.svg").write_text(svg)
        light, opaque = lightness(svg)
        icons[name] = {"src": f"{SITE}/icons/hud_{name}.svg?v={version(svg.encode())}",
                       "w": width, "h": height, "ax": ax, "ay": ay, "light": light, "opaque": opaque}
    return icons


def battery_icon(raw, maps):
    """The battery's marker: the Protective device's mode icon, the frame its configs' `ui.icon`
    gives (one for all of them), at its /wiki/icons/ address with the version the server gives it
    (src/icons.rs: the same sha256's first 12 hex digits)."""
    frames = {load(raw, "gameplay/game_modes/" + c["config"])["mode"]["ui"]["icon"] + 1
              for m in maps.values() for (mode, _), mm in m["modes"].items() if mode == PROTECTIVE_DEVICE
              for c in mm["configs"]}
    if len(frames) != 1:
        die(f"the Protective device's configs show mode icons {sorted(frames)}, not one")
    path = MODE_ICONS / f"{frames.pop()}.webp"
    if not path.is_file():
        die(f"{path}: no such mode icon (scripts/wiki/build_icons.py makes it)")
    blob = path.read_bytes()
    width, height = Image.open(path).size
    return {"src": f"/wiki/icons/modes/{path.name}?v={version(blob)}",
            "w": width, "h": height, "ax": width / 2, "ay": height / 2, "light": 1.0, "opaque": 1.0}


def localization(raw):
    """Each language's texts (site-texts' codes)."""
    return {code: load(raw, path)["strings"] for code, path in zip(CODES, DICTIONARIES)}


def artefact_names(texts, key):
    """The names the game's pickup hints give, in each language."""
    hint = f"st_{key}_pickup_hint"
    names = {}
    for code in CODES:
        found = re.match(r"\s*" + ARTEFACT_HINT[code], texts[code].get(hint, ""))
        if not found:
            die(f"{hint}: no artefact name in its {code} text")
        names[code] = found.group(1)
    return names


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


def rotation(angles):
    """The rows i, j, k (where local x, y, z go) of the engine's `create_rotation`
    (Rx · Ry · Rz for row vectors), as mapdata's `Rotation::from_angles`."""
    (xs, ys, zs), (xc, yc, zc) = ([math.sin(a) for a in angles], [math.cos(a) for a in angles])
    return ((yc * zc, -yc * zs, ys),
            (ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            (-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc))


def turn(rows, v):
    """`v` through the rotation `rows` (v · R)."""
    return tuple(rows[0][n] * v[0] + rows[1][n] * v[1] + rows[2][n] * v[2] for n in range(3))


def hull(points):
    """The convex hull of [x, z] points, counter-clockwise (Andrew's monotone chain)."""
    points = sorted(set(points))
    if len(points) < 3:
        return points
    cross = lambda o, a, b: (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in points:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(points):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def footprints(geometry):
    """What a `collision_geometry`'s trigger shapes cover seen from above, as mapdata places them
    (`geometry_shapes`: the mesh's rotation and position, then the object's scale, rotation and
    position): {"c": [x, z, r]} for a sphere or an upright cylinder, else {"p": [[x, z], ...]}, the
    hull of its corners (a box) or of its rims (a tilted cylinder)."""
    scale = geometry.get("scale") or (1, 1, 1)
    outer = rotation(geometry.get("rotation") or (0, 0, 0))
    origin = geometry.get("position") or (0, 0, 0)
    place = lambda v: [a + b for a, b in zip(turn(outer, [c * s for c, s in zip(v, scale)]), origin)]
    shapes = []
    for mesh in geometry.get("meshes") or []:
        kind = mesh.get("type")
        if kind not in (0, 1, 2):  # 0 sphere, 1 box, 2 cylinder (no shipped trigger has another)
            continue
        half = [h * s for h, s in zip(mesh.get("scale") or (1, 1, 1), scale)]
        inner = rotation(mesh.get("rotation") or (0, 0, 0))
        rows = tuple(turn(outer, row) for row in inner)
        x, _, z = place(mesh.get("position") or (0, 0, 0))
        up = rows[1][1]
        if kind == 0 or (kind == 2 and abs(up) > 0.999):
            shapes.append({"c": [r1(x), r1(z), r1(half[0])]})
            continue
        if kind == 1:
            local = [(a * half[0], b * half[1], c * half[2]) for a in (-1, 1) for b in (-1, 1) for c in (-1, 1)]
        else:
            local = [(half[0] * math.cos(t), y * half[1], half[0] * math.sin(t))
                     for y in (-1, 1) for t in (2 * math.pi * n / 24 for n in range(24))]
        corners = [turn(rows, v) for v in local]
        shapes.append({"p": [[r1(x + dx), r1(z + dz)] for dx, dz in hull([(dx, dz) for dx, _, dz in corners])]})
    return shapes


# A damage zone's `hit_parts_filter` part as the site names it, both sides together (the equipment
# pages' `part_name`: brain the head, forearm the arms, arm the hands, hip the legs), head to feet.
PARTS = ("head", "face", "chest", "back", "arms", "hands", "legs", "feet")
PART = {"brain": "head", "face": "face", "front": "chest", "back": "back", "left_forearm": "arms",
        "right_forearm": "arms", "left_arm": "hands", "right_arm": "hands", "left_hip": "legs",
        "right_hip": "legs", "left_foot": "feet", "right_foot": "feet"}


def window(middle, spread):
    """A time drawn uniformly in middle ± spread seconds, as the anomaly draws it
    (`artefact_respawn_delay`, never below 0): [earliest, latest], whole seconds."""
    return [max(0, round(middle - spread)), max(0, round(middle + spread))]


def anomaly(core, by_name, artefact_keys):
    """What an anomaly's card says and its area shows (docs/notes/maps_tab.md, "Anomalies"): the
    artefacts it holds (its pool: a kind and how many), when the first one comes and then each next
    (`artefacts_first_spawn_time_sec`, `_respawn_time_sec`, ± `_respawn_variation_sec`), the chance
    each try has, how many of its artefacts may lie uncollected at once (`max_artefacts_count`), its
    containers' search time and cooldown, how often its zones switch (a sequential group's
    `min/max_active_time_sec`), what its zones that hurt do (a hit of `max_hit` of each part's health
    to each part of `hit_parts_filter`, before armour: `continuous_hit` every `interval_in_msec`
    spread over the ticks, else one hit at most that often; mapdata's `Damage`), grouped by damage
    type, parts and timing with the range of their hits (School's fruit cloud has five levels),
    and where it is: its damage zones' shapes seen from above, each marked whether it hurts
    (`max_hit` > 0)."""
    variation = core.get("artefacts_respawn_variation_sec", 0)
    containers = [by_name[name] for name in core.get("artefact_containers") or [] if name in by_name]
    zones, switch = [], set()
    for state in core.get("states") or []:
        if not state.get("enabled", True):
            continue
        for group in state.get("groups") or []:
            if not group.get("enabled", True):
                continue
            if group.get("sequential_activation") and not group.get("keep_all_zones_active"):
                switch.add((group.get("min_active_time_sec", 0), group.get("max_active_time_sec", 0)))
            zones += [name for name in group.get("zones") or [] if name not in zones]
    # Its zones, each shape marked whether it hurts ("hurt": a damage zone with a hit) or not (an
    # idle zone of sound and sight: Rudnya's Lifebone anomaly has only that, its twin the damage).
    areas, hits = [], defaultdict(list)
    for name in zones:
        zone = by_name.get(name) or {}
        if zone.get("game_object_type") != "damage_zone":
            continue
        hurt = bool(zone.get("max_hit"))
        if hurt:
            # The v0.11 and v0.21 ports keep their builds' `body` and `head`, which 0.26g's body has not
            # (its twelve parts: armour.rs `BodyPart`): shown without them, and said at the build
            # (docs/notes/maps_tab.md, "Anomalies").
            unknown = set(zone.get("hit_parts_filter") or []) - set(PART)
            if unknown:
                print(f"note: {name}: hit_parts_filter names parts 0.26g has not ({', '.join(sorted(unknown))}): not shown",
                      file=sys.stderr)
            parts = sorted({PART[p] for p in zone.get("hit_parts_filter") or [] if p in PART}, key=PARTS.index)
            hits[(zone.get("damage_type", ""), tuple(parts), bool(zone.get("continuous_hit")),
                  zone.get("interval_in_msec", 1000))].append(zone["max_hit"])
        for ref in zone.get("collision_geometries") or []:
            geometry = by_name.get(ref if isinstance(ref, str) else ref.get("name"))
            if geometry:
                areas += [{**shape, "hurt": hurt} for shape in footprints(geometry)]
    seconds = lambda key: sorted({c.get(key, 0) for c in containers})
    return {
        "gives": [[artefact_keys[a["dict_id"]], a.get("count", 0)] for a in core.get("artefacts") or []
                  if a["dict_id"] in artefact_keys],
        "first": window(core.get("artefacts_first_spawn_time_sec", 0), variation),
        "next": window(core.get("artefacts_respawn_time_sec", 0), variation),
        "chance": round(core.get("artefacts_respawn_chance", 0), 3),
        "max": core.get("max_artefacts_count", 0),
        "containers": len(containers),
        "search": seconds("artefacts_search_time_sec"),
        "cooldown": seconds("artefacts_cooldown_time_sec"),
        "switch": sorted(switch),
        "damage": sorted(({"type": kind, "parts": list(parts), "continuous": continuous, "interval": interval,
                           "hit": [round(min(values), 3), round(max(values), 3)]}
                          for (kind, parts, continuous, interval), values in hits.items()),
                         key=lambda d: (-d["hit"][1], d["type"])),
        "areas": areas,
    }


def objects(project, artefact_keys):
    """What the tab shows of one project."""
    so = project.get("server_objects") or {}
    spawners = so.get("victory_item_spawners") or []
    stations = [xzy(o) for o in so.get("victory_item_spawn_points") or []]
    client = project.get("client_objects") or []
    by_name = {o.get("full_name"): o for o in client}
    kinds_of, owners_of, anomalies = defaultdict(set), defaultdict(list), []
    for o in client:
        if o.get("game_object_type") != "generic_anomaly_core":
            continue
        # An anomaly gives artefacts when they are on (`artefacts_enabled`) and it has containers;
        # one that does not is shown by its zones alone (v0.21 Rudnya's swamp is seven anomalies,
        # two of them giving artefacts), and one with neither is not shown: Keeper of the
        # Artifacts' hidden pool (content-edit/artefact-keeper-1.json) is one.
        found = anomaly(o, by_name, artefact_keys)
        if not (o.get("artefact_containers") and o.get("artefacts_enabled")):
            if found["areas"]:
                anomalies.append({"gives": [], **{k: found[k] for k in ("switch", "damage", "areas")}})
            continue
        kinds = {artefact_keys[a["dict_id"]] for a in o.get("artefacts") or [] if a["dict_id"] in artefact_keys}
        for name in o.get("artefact_containers") or []:
            kinds_of[name] |= kinds
            owners_of[name].append(len(anomalies))
        anomalies.append(found)
    # Twins: two anomalies on the very same zones, one of them hurting and the other only seen and
    # heard (Rudnya's Lifebone and Spring, Chemical Plant's Spring and Onyx). The later one names
    # the first (`twin`), and the page draws their place once, with one card for both.
    first_at = {}
    for i, a in enumerate(anomalies):
        place = json.dumps(sorted(json.dumps({k: v for k, v in s.items() if k != "hurt"}) for s in a["areas"]))
        if a["areas"] and place in first_at:
            a["twin"] = first_at[place]
        first_at.setdefault(place, i)
    # A container's own position is its anomaly's pivot, shared by all of
    # them; where the artefact is found is its collision geometry (the zone
    # the search runs in). Each is [x, z, y, its kinds, its anomalies (indices)].
    artefacts = []
    for o in client:
        if o.get("game_object_type") == "artefact_container":
            kinds = sorted(kinds_of.get(o.get("full_name"), ()))
            zones = [by_name[c["name"]] for c in o.get("collision_geometries") or [] if c.get("name") in by_name]
            if kinds:
                artefacts.append(xzy(zones[0] if zones else o) + [kinds, owners_of[o.get("full_name")]])
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
        "anomalies": anomalies,
    }


def hunt(config):
    """Artifact Hunt's points of one mode config's `artefact_hunt`: where its
    artefact appears ([x, z, y]) and the teams' bases ([x, z, y, team])."""
    return {"hunt": sorted(xzy(s) for s in config["spawners"]),
            "bases": sorted(xzy(b) + [b["team"]] for b in config["bases"])}


def spawns(mode_id, points):
    """The respawn points a mode's players get, of its project's (each
    [x, z, y, team, heading]). A free-for-all's are the team-2 points alone:
    each player is a side of his own and spawns there and nowhere else (exe
    patch 142, docs/notes/ffa.md); the two teams' points its project keeps, a
    copy of Team Deathmatch's, are never his. Any other mode's are all of
    them."""
    return [point for point in points if point[3] == 2] if mode_id == FREE_FOR_ALL else points


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
        text = lambda key: {code: texts[code].get(key) or texts["en"].get(key, key) for code in CODES}
        dictionary = load(raw, "gameplay/db_static_dictionaries")
        artefact_keys = {i["dict_id"]: i["name"].removeprefix("artefact_") for i in dictionary["items_dict"]
                         if i["cfg_name"].startswith("gameplay/items/artefacts/")}

        # The playable ids, by map (the project's folder) and mode.
        maps = playable(raw)

        minimaps = hud_minimaps(args.ffdec, raw, tmp)
        for sub in ("minimap", "icons"):
            shutil.rmtree(out / sub, ignore_errors=True)
        hud = hud_icons(args, raw, tmp, out)
        hud["battery"] = battery_icon(raw, maps)
        renders = json.loads(RENDER_TABLE.read_text())["maps"]
        out.mkdir(parents=True, exist_ok=True)
        data_maps, missing = [], []
        used_artefacts = set()
        # By build, as the room list (BUILDS), each build's maps by folder.
        for folder in sorted(maps, key=lambda f: (BUILDS.index(maps[f]["build"]), f)):
            m = maps[folder]
            modes, project_name, artefacts, anomalies = [], None, None, None
            for mode_id, layout in sorted(m["modes"]):
                mm = m["modes"][mode_id, layout]
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
                if anomalies is not None and seen["anomalies"] != anomalies:
                    die(f"{folder}: its anomalies differ between modes")
                anomalies = seen.pop("anomalies")
                rules = mm["rules"]
                if not rules & {GATHER, TIMED}:
                    seen["spawners"], seen["containers"] = [], []
                if TIMED in rules:
                    # The batteries come out of the chargers: their spawn
                    # points are the chargers' own boxes, not places to show.
                    seen["spawners"] = []
                else:
                    seen["stations"] = []
                seen["respawn"] = spawns(mode_id, seen["respawn"])
                p = mm["parameters"]
                # [x, z, y, cp_id], by cp_id (0, 1, 2: A, B, C).
                seen["units"] = (sorted((xzy(zone) + [zone.get("cp_id", 0)] for zone in p.get("capture_zones") or []),
                                        key=lambda unit: unit[3]) if RESEARCH in rules else [])
                hunts = [hunt(c["artefact_hunt"]) if HUNT in rules else {"hunt": [], "bases": []} for c in mm["configs"]]
                if any(h != hunts[0] for h in hunts):
                    die(f"{folder}: Artifact Hunt's spawns or bases differ between its configs (same map and mode)")
                seen.update(hunts[0])
                # A mode's other layout (Research's units elsewhere) is a choice of its own: "5.2".
                modes.append({"key": f"{mode_id}.{layout + 1}" if layout else str(mode_id), "layout": layout,
                              "mode": mode_id, "ids": sorted(mm["ids"]), "name": text(mm["mode_name"]),
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
                     "minimap": f"{SITE}/minimap/{name}?v={version(blob)}", "modes": modes, "artefacts": artefacts,
                     "anomalies": anomalies}
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
        pending = [folder for folder in missing if folder in RENDER_PENDING]
        if pending:
            print("note: no top-down render of " + ", ".join(pending) + " yet: its minimap only (RENDER_PENDING)")
        missing = [folder for folder in missing if folder not in RENDER_PENDING]
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
              + (SRP / "scripts" / "site-texts" / "words.js").read_text() + (HERE / "ui.js").read_text())
    (out / "maps.js").write_text(script)
    template = (HERE / "template.html").read_text()
    faces = "\n".join(line for line in (SRP / "servers" / "web-server" / "damage" / "damage.html").read_text().splitlines()
                      if line.startswith("@font-face"))
    page = (template.replace("/*__FONT__*/", faces).replace("__WORDS__", WORDS)
            .replace("__SCRIPT__", f"{SITE}/maps.js?v={version(script.encode())}"))
    (out / "maps.html").write_text(page)
    # Its words are its script's alone (site-texts: the server fills them in).
    Texts().write(out / "maps.texts.json")
    served = ["maps.js"] + [f"{sub}/{f.name}" for sub in ("minimap", "icons") for f in sorted((out / sub).iterdir())]
    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/maps-tab/build.py; do not edit. The files maps.html names.\n"
        "files! {\n" + "".join(
            f'    "{f}" => "{TYPES[f.rsplit(".", 1)[1]]}",\n' for f in served)
        + "}\n")
    write_built_from(out / "built-from.tsv", rows, "the Maps tab (srp-026g/scripts/maps-tab/build.py)")
    points = sum(len(mo["respawn"]) + len(mo["spawners"]) + len(mo["stations"]) + len(mo["containers"]) + len(mo["units"])
                 + len(mo["hunt"]) + len(mo["bases"])
                 for m in data_maps for mo in m["modes"]) + sum(len(m["artefacts"]) for m in data_maps)
    print(f"{out}: revision {revision}, {len(data_maps)} maps, {points} points, maps.js {len(script) / 1024:.0f} KB")


if __name__ == "__main__":
    main()
