#!/usr/bin/env python3
"""The release's timed anomalies as 0.26g data (patch 42's maps): a patch
folder with the client projects of level_08 (Cologne) and level_09 (London) rewritten and the
anomaly particles they need. README.md has the rules and the commands.

    python build.py --live <resources.db> --live <resources.db.patch_N>... \\
        --release-natives <0.28d0 resources.db> \\
        --release-london <0.31a2 resources.NN.db>... --out <folder> [--maps level_08,level_09] \\
        [--pack <id>@<version>]

--live: the game the players have, stock first, then the live patches in
release/patches.tsv order (a later archive's file wins). --release-*: the
release builds' archives. --pack: the out folder is a content pack
(release/content.tsv's kind `pack`): files/<path> and pack.json, a
replace_file (the live file's sha256) for each project, an add_file for
each particle; --live must then be exactly what the manifest's packs are
composed over: the stock db and its base rows' patches. Needs dbgrab (tools/re-kit/common/dbgrab;
DBGRAB names another). Prints per project what it did; refuses a project
whose result names an object it does not have.
"""
import argparse
import collections
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bcw  # noqa: E402

REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
DBGRAB = os.environ.get("DBGRAB", os.path.join(REPO, "tools", "re-kit", "common", "dbgrab", "target", "release", "dbgrab"))

# The artefact kinds of the release's places that 0.26g has. 758 (mochalka,
# the sponge) is a thrown artefact (data type 13) 0.26g cannot use: its
# anomalies are left out.
KINDS = {54: "lifebone", 74: "spring", 76: "onyx"}
DROPPED_KINDS = {758: "mochalka"}

# The timed anomaly (dynamic_anomalies_026g.asm, docs/notes/dynamic_anomalies.md):
# energy thresholds of its states, highest first.
CHARGING, DAMAGE, ARTEFACT, AFTER_PICKUP, IDLE = 3000, 2000, 1000, 50, 0
NEVER = 100000                     # s: its artefacts never come by themselves (the exe's mark of a timed core)
ARTEFACT_COUNT = 5                 # the core's own artefacts (patch_14's)

# What patch_14 took out of the 0.31a2 objects for 0.26g, and what a kind's
# damage zone takes from patch_14's (its type, body parts, damage, sounds).
GEOMETRY_DROP = {"collision_mask", "is_enabled", "is_visible_in_editor", "offset", "shapes", "view_mode"}
CONTAINER_DROP = {"is_enabled", "is_visible_in_editor", "offset"}
ZONE_DROP = {"apply_hit_type", "charged_state", "charging_state", "discharge_state", "discharged_state", "frequency",
             "game_effect_on_inside", "hit_amount_on_bound", "hit_amount_on_center", "hit_armor_piercing",
             "hit_interval", "hit_interval_type", "is_enabled", "is_visible_in_editor", "min_armor_piercing",
             "min_hit", "mode", "offset", "particle"}
SOUND_KEYS = ("hit_sound", "hit_sound_resource_id", "idle_sound", "idle_sound_resource_id", "activated_sound",
              "activated_sound_resource_id", "deactivated_sound", "deactivated_sound_resource_id")
KIND_KEYS = ("damage_type", "hit_parts_filter", "max_hit") + SOUND_KEYS
# A keep_all group as the stock cores have it most (zones on for the whole state).
GROUP = {"enabled": True, "keep_all_zones_active": True, "max_active_time_sec": 0, "max_charged_count": 0,
         "min_active_time_sec": 0, "recharge_time_sec": 20, "sequential_activation": False}
# The release's anomalies rule, as the exe has it (dynamic_anomalies_026g.asm).
RULE = {"spawn_anomalies_delay_s": 30, "try_spawn_anomaly_interval_s": 30, "try_spawn_anomaly_variation_s": 10,
        "max_active_anomalies_count": 16, "block_new_anomalies_spawn_radius": 15}
VALUES_SEEN = 65535                # the most values of a project the 0.26g game is known to load (London: 43,799)


class Archive:
    """resources.db archives, a later one's file winning."""

    def __init__(self, paths):
        self.paths = list(paths)
        self.listings = {}

    def listing(self, path):
        if path not in self.listings:
            out = subprocess.run([DBGRAB, path, "list"], capture_output=True, text=True, check=True).stdout
            self.listings[path] = {line.split("\t")[0] for line in out.splitlines() if "\t" in line}
        return self.listings[path]

    def where(self, name):
        for path in reversed(self.paths):
            if name in self.listing(path):
                return path
        return None

    def has(self, name):
        return self.where(name) is not None

    def names(self, pattern):
        regex = re.compile(pattern)
        return sorted({n for path in self.paths for n in self.listing(path) if regex.fullmatch(n)})

    def read(self, name, tmp):
        path = self.where(name)
        if path is None:
            raise KeyError(name)
        out = tempfile.mkdtemp(dir=tmp)
        subprocess.run([DBGRAB, path, "get", name, out], capture_output=True, check=True)
        with open(os.path.join(out, name.replace("/", "__")), "rb") as file:
            return file.read()


# --- particles ------------------------------------------------------------------

def convert_particle(data):
    """A 0.31a2 particle as patch_14 has its own: MarginMultiplier out (0.26g
    has no such property), big_sphere -> big_sphere_l09 (0.31a2's big_sphere,
    patch_14's; 0.26g's own big_sphere draws another texture)."""
    def walk(node):
        kind = node[0]
        if kind == bcw.NAMED:
            children = [(key, walk(child)) for key, child in node[1] if key != b"MarginMultiplier"]
            return (kind, children, len(children))
        if kind == bcw.INDEXED:
            return (kind, [walk(child) for child in node[1]], node[2])
        if kind == bcw.STRING and node[1] == b"fx/artifacts/big_sphere":
            return (kind, b"fx/artifacts/big_sphere_l09", len(b"fx/artifacts/big_sphere_l09") + 1)
        return node
    return bcw.write(walk(bcw.read(data)))


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child)


# --- projects -------------------------------------------------------------------

def copy_place(obj, drop, **changes):
    out = {key: value for key, value in obj.items() if key not in drop}
    out.update(changes)
    return out


def state(threshold, active, on_exit, zones):
    return {"active_time_sec": active, "enabled": True, "energy_on_exit": on_exit, "energy_threshold": threshold,
            "groups": [dict(GROUP, zones=list(zones))] if zones else [], "select_priority": 0,
            "shoot_trigger": False, "zone_activity_trigger": False}


def timed_core(template, release_core, name, container, damage_zone, charge_zone, dict_id, after_pickup):
    core = {key: value for key, value in template.items()}
    core.update({
        "artefact_containers": [container], "artefacts": [{"count": ARTEFACT_COUNT, "dict_id": dict_id}],
        "artefacts_enabled": True, "artefacts_first_spawn_time_sec": NEVER, "artefacts_respawn_chance": 1,
        "artefacts_respawn_time_sec": NEVER, "artefacts_respawn_variation_sec": 0,
        "energy_af_container_use": AFTER_PICKUP, "energy_decrease_speed": 0, "energy_enabled": True,
        "energy_initial": IDLE, "energy_maximum": 0,
        "full_name": name, "name": name.rsplit("/", 1)[1], "max_artefacts_count": 1,
        "position": release_core["position"], "rotation": release_core["rotation"],
        "scale": release_core["scale"], "pivot": release_core["pivot"],
        "states": [
            state(CHARGING, release_core["charging_state_timeout"], DAMAGE, [charge_zone]),
            state(DAMAGE, release_core["damage_state_timeout"], ARTEFACT, [damage_zone, charge_zone]),
            state(ARTEFACT, 0, 0, [damage_zone]),
            state(AFTER_PICKUP, after_pickup, IDLE, []),
            state(IDLE, 0, 0, []),
        ],
    })
    for key in [k for k in core if k.startswith("energy_on_")]:
        core[key] = 0
    return core


def references(obj, names):
    return {s for s in strings(obj) if s in names and s != obj.get("full_name")}


def remove_anomalies(objects):
    """The generic anomaly cores out, and what only they (and things going out) name."""
    names = {o["full_name"] for o in objects}
    refs = {o["full_name"]: references(o, names) for o in objects}
    referrers = collections.defaultdict(set)
    for name, named in refs.items():
        for other in named:
            referrers[other].add(name)
    out = {o["full_name"] for o in objects if o["game_object_type"] == "generic_anomaly_core"}
    changed = True
    while changed:
        changed = False
        for name in {n for o in out for n in refs[o]} - out:
            if referrers[name] <= out:
                out.add(name)
                changed = True
    return [o for o in objects if o["full_name"] not in out], [o for o in objects if o["full_name"] in out]


def convert(project, release, templates, allowed, after_pickup):
    """project: the live 0.26g client project; release: the release's for its mode."""
    kept, removed = remove_anomalies(project["client_objects"])
    by_name = {o["full_name"]: o for o in release["client_objects"]}
    taken = {o["full_name"] for o in kept}
    added, places, dropped = [], set(), collections.Counter()

    def add(obj):
        if obj["full_name"] in taken:
            if any(o["full_name"] == obj["full_name"] and o == obj for o in added):
                return
            raise SystemExit(f"{obj['full_name']}: already in the project")
        taken.add(obj["full_name"])
        added.append(obj)

    for core in release["client_objects"]:
        if core.get("game_object_type") != "timeout_anomaly_core":
            continue
        dict_id = core["artefact_dict_id"]
        if dict_id not in KINDS:
            dropped[DROPPED_KINDS.get(dict_id, dict_id)] += 1
            continue
        kind = KINDS[dict_id]
        place = core["full_name"].rsplit("/", 1)[0]
        places.add(place)
        zone, container = by_name[core["damage_zone"]], by_name[core["artefact_container"]]
        geometries = [by_name[g["name"]] for g in zone["collision_geometries"] + container["collision_geometries"]]
        for geometry in geometries:
            add(copy_place(geometry, GEOMETRY_DROP, layer="vip"))
        add(copy_place(container, CONTAINER_DROP, layer="default"))
        damage = copy_place(zone, ZONE_DROP, layer="default", idle_particle=core["damage_anomaly_particle"])
        for key in KIND_KEYS:
            if key in templates[kind]:
                damage[key] = templates[kind][key]
            else:
                damage.pop(key, None)
        damage.setdefault("sound_zones", [{"position": (0.0, 0.0, 0.0)}])
        add(damage)
        use_box = by_name[container["collision_geometries"][0]["name"]]
        charge = {key: value for key, value in damage.items() if key not in SOUND_KEYS + ("sound_zones",)}
        charge.update({
            "full_name": f"{place}/charge_zone_{kind}", "name": f"charge_zone_{kind}",
            "position": container["position"], "rotation": (0.0, 0.0, 0.0), "scale": (1.0, 1.0, 1.0),
            "collision_geometries": [{"name": use_box["full_name"]}], "max_hit": 0, "hit_parts_filter": {},
            "idle_particle": core["charging_artefact_particle"],
            "effect_zones": [{"position": (0.0, 0.0, 0.0), "rotation": (0.0, 0.0, 0.0)}],
        })
        add(charge)
        add(timed_core(templates["core"], core, core["full_name"], container["full_name"], damage["full_name"],
                       charge["full_name"], dict_id, after_pickup))
    for obj in added:
        extra = set(obj) - allowed[obj["game_object_type"]]
        if extra:
            raise SystemExit(f"{obj['full_name']}: keys 0.26g's {obj['game_object_type']} never has: {sorted(extra)}")
    result = dict(project, client_objects=kept + added)
    names = {o["full_name"] for o in result["client_objects"]}
    for obj in added:
        missing = [s for s in strings(obj) if "/" in s and s.count("/") >= 1 and s not in names
                   and not s.startswith(("fx/", "anomalies/"))]
        if missing:
            raise SystemExit(f"{obj['full_name']}: names what is not in the project: {missing}")
    return result, added, removed, places, dropped


def templates_of(london):
    """patch_14's per-kind damage zones and its core, the 0.26g forms."""
    by_name = {o["full_name"]: o for o in london["client_objects"]}
    out = {}
    for obj in london["client_objects"]:
        if obj["game_object_type"] != "generic_anomaly_core":
            continue
        out["core"] = obj
        kind = next(k for k in KINDS.values() if f"timeout_anomaly_{k}" in obj["name"])
        zone = by_name[obj["states"][0]["groups"][0]["zones"][0]]
        if kind == "lifebone" and "indoor" not in zone["idle_particle"]:
            continue                                  # the outdoor lifebone: same keys
        out.setdefault(kind, zone)
    missing = [k for k in ("core", *KINDS.values()) if k not in out]
    if missing:
        raise SystemExit(f"patch_14's London has no {missing}")
    return out


def rule_of(release_archive, level, mode, tmp):
    options = bcw.loads(release_archive.read(f"gameplay/game_modes/{level}_day_{mode}.options", tmp))
    rule = options["mode"]["anomalies"]
    for key, value in RULE.items():
        if rule[key] != value:
            raise SystemExit(f"{level}_day_{mode}: the release's {key} is {rule[key]}, the exe's {value}")
    return rule["respawn_anomalies_timeout_s"]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--live", action="append", required=True)
    parser.add_argument("--release-natives", action="append", required=True)
    parser.add_argument("--release-london", action="append", required=True)
    parser.add_argument("--out", required=True)
    # The owner (2026-10-22): only London and Cologne; every other map keeps its static anomalies.
    parser.add_argument("--maps", default="level_08,level_09")
    parser.add_argument("--pack", help="<id>@<version>: write a content pack (files/ and pack.json)")
    args = parser.parse_args()
    if args.pack and not re.fullmatch(r"[a-z0-9._-]+@[A-Za-z0-9._-]+", args.pack):
        raise SystemExit(f"--pack {args.pack}: a pack is named <id>@<version>")
    operations = []

    def put(name, data, base=None):
        """Writes a game file into the out folder (under files/ for a pack) and notes its pack operation."""
        path = os.path.join(args.out, *(["files"] if args.pack else []), name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as file:
            file.write(data)
        operations.append({"op": "add_file", "path": name} if base is None else
                          {"op": "replace_file", "path": name, "base_sha256": hashlib.sha256(base).hexdigest()})

    live, natives, london = Archive(args.live), Archive(args.release_natives), Archive(args.release_london)
    maps = args.maps.split(",")
    if os.path.exists(args.out) and os.listdir(args.out):
        raise SystemExit(f"{args.out}: not empty")
    with tempfile.TemporaryDirectory() as tmp:
        london_now = bcw.loads(live.read("projects/level_09/day_2/client_project", tmp))
        templates = templates_of(london_now)
        allowed = collections.defaultdict(set)
        projects = [n for n in live.names(r"projects/level_0\d/[a-z]+_\d/client_project") if n.split("/")[1] in maps]
        loaded = {}
        originals = {}
        for name in projects:
            originals[name] = live.read(name, tmp)
            loaded[name] = bcw.loads(originals[name])
            for obj in loaded[name]["client_objects"]:
                allowed[obj["game_object_type"]] |= set(obj)
        particles, total = set(), collections.Counter()
        for name in projects:
            level, variant = name.split("/")[1:3]
            mode = int(variant.rsplit("_", 1)[1])
            source = london if level == "level_09" else natives
            release_name = f"projects/{level}/day_{mode}/client_project"
            release = bcw.loads(source.read(release_name, tmp))
            after_pickup = rule_of(source, level, mode, tmp)
            result, added, removed, places, dropped = convert(loaded[name], release, templates, allowed, after_pickup)
            data = bcw.dump(result)
            again = bcw.loads(data)
            if again != bcw.loads(bcw.dump(result)) or [o for o in again["client_objects"] if o not in added] != \
                    [o for o in loaded[name]["client_objects"] if o not in removed]:
                raise SystemExit(f"{name}: the written project does not read back as built")
            put(name, data, originals[name])
            values = bcw.count(bcw.read(data))
            kinds = collections.Counter(o["game_object_type"] for o in added)
            gone = collections.Counter(o["game_object_type"] for o in removed)
            particles |= {o["idle_particle"] for o in added if "idle_particle" in o}
            total["places"] += len(places)
            total["cores"] += kinds["generic_anomaly_core"]
            print(f"{name} (from {release_name}): {len(places)} places, {kinds['generic_anomaly_core']} timed cores, "
                  f"{kinds['damage_zone']} zones, {kinds['artefact_container']} containers, "
                  f"{kinds['collision_geometry']} geometries; removed {dict(gone)}; left out {dict(dropped)}; "
                  f"{values} values" + (f" (WARNING: above {VALUES_SEEN})" if values > VALUES_SEEN else ""))
        print(f"{len(projects)} projects, {total['places']} places, {total['cores']} timed cores")
        materials = set()
        for particle in sorted(particles):
            name = f"particles/{particle}.particle"
            if live.has(name):
                print(f"{name}: the game has it")
                data = live.read(name, tmp)
            else:
                source = london if london.has(name) else natives
                data = convert_particle(source.read(name, tmp))
                put(name, data)
                print(f"{name}: from the release ({len(data)} bytes)")
            materials |= {s for s in strings(bcw.loads(data)) if s.startswith("fx/")}
        missing = sorted(m for m in materials if not live.has(f"material_instances/{m}.material"))
        print(f"materials the particles name: {len(materials)}; not in the game: {missing or 'none'}")
    if args.pack:
        ident, version = args.pack.split("@", 1)
        manifest = {"format_version": 1, "id": ident, "version": version, "target_build": "0.26g",
                    "description": "dynamic anomalies: the release's timed anomalies on every map (the client "
                                   "projects of level_08 and level_09 and the particles they need, "
                                   "srp-026g/scripts/dynamic-anomalies; client patch 42)",
                    "operations": sorted(operations, key=lambda o: o["path"])}
        with open(os.path.join(args.out, "pack.json"), "w") as file:
            file.write(json.dumps(manifest, indent=2) + "\n")
        with open(os.path.join(args.out, "pack.json"), "rb") as file:
            digest = hashlib.sha256(file.read()).hexdigest()
        print(f"pack {args.pack}: {len(operations)} operations "
              f"({sum(o['op'] == 'replace_file' for o in operations)} replace a file); pack.json sha256 {digest}")


if __name__ == "__main__":
    main()
