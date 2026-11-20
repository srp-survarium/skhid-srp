#!/usr/bin/env python3
import sys
import importlib
import pathlib
import json
import csv
import re
import hashlib
import argparse

p = argparse.ArgumentParser(
    description="Collect audit metadata from released calculator data, content packs and retail donor leaves"
)
p.add_argument("--packs", type=pathlib.Path, required=True)
p.add_argument("--donor-leaves", type=pathlib.Path, required=True)
p.add_argument("--dictionary-leaves", type=pathlib.Path, required=True)
args = p.parse_args()
root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(root / "srp-026g/scripts/content-edit"))
e = importlib.import_module("content_edit")


def decode(buf, off=0):
    d, ident, crc, t, n = e.rec(buf, off)
    if t == e.T_NAMED:
        return {
            e.name_at(buf, e.rec(buf, c)[1]).decode(): decode(buf, c)
            for c in e.children(buf, off)
        }
    if t == e.T_INDEXED:
        return [decode(buf, c) for c in e.children(buf, off)]
    if t == e.T_STR:
        return bytes(buf[d : d + n - 1]).decode()
    if t in (e.T_INT, e.T_FLOAT):
        return e.as_number(t, d & 0xFFFFFFFF)
    if t == e.T_BOOL:
        return bool(d)
    return None


calculator = (root / "srp-026g/servers/web-server/damage/calc.js").read_text()
d, _ = json.JSONDecoder().raw_decode(calculator.split("const DATA = ", 1)[1])
packs = args.packs
provenance = []
for pack in ["weapons-031-7", "weapons-046-10", "weapons-069-11"]:
    p = packs / pack
    provenance.append(
        {
            "pack": pack,
            "sha256": hashlib.sha256((p / "pack.json").read_bytes()).hexdigest(),
        }
    )
    for f in (p / "files/gameplay/items/weapons").glob("wpn_*.options"):
        c = decode(f.read_bytes())
        w = c["parameters"].copy()
        w.update(
            {k: c[k] for k in ["recoil", "dispersion", "breath_vibration"] if k in c}
        )
        w["name"] = f.stem
        d["weapons"][f.stem] = w
    for f in (p / "files/gameplay/items/weapons/ammo").glob("*.options"):
        c = decode(f.read_bytes())
        d["ammo"][f.stem] = dict(c["data"], name=f.stem)
    dictfile = p / "files/gameplay/db_static_dictionaries"
    if dictfile.exists():
        dic = decode(dictfile.read_bytes())
        ids = {
            i["dict_id"]: pathlib.Path(i["cfg_name"]).stem for i in dic["items_dict"]
        }
        compat = {}
        for pair in dic["items_compatibility"]:
            a, b = ids.get(pair["first_item"]), ids.get(pair["second_item"])
            for w, m in [(a, b), (b, a)]:
                if w in d["weapons"] and m in d["ammo"]:
                    compat.setdefault(w, set()).add(m)
        d["compat"].update({w: sorted(ms) for w, ms in compat.items()})
ids = {}
entries = {}
for line in args.dictionary_leaves.read_text().splitlines():
    k, _, v = line.partition("\t")
    m = re.match(r"(items_dict\[[0-9]+\])\.(dict_id|cfg_name)$", k)
    if m:
        entries.setdefault(m[1], {})[m[2]] = json.loads(v.split(":", 1)[1])
for v in entries.values():
    if "dict_id" in v and "cfg_name" in v:
        ids[v["dict_id"]] = pathlib.Path(v["cfg_name"]).stem
for pack in ["weapons-031-7", "weapons-046-10", "weapons-069-11"]:
    for op in json.loads((packs / pack / "pack.json").read_text())["operations"]:
        if op["op"] == "dictionary_add" and op["table"] == "items_dict":
            for v in op["entries"]:
                ids[v["dict_id"]] = pathlib.Path(v["cfg_name"]).stem
        if op["op"] == "config_set" and op["path"] == "gameplay/db_static_dictionaries":
            for k, v in op["values"].items():
                if k.startswith("items_compatibility."):
                    pair = v["table"]
                    a, b = ids.get(pair["first_item"]), ids.get(pair["second_item"])
                    for w, m in [(a, b), (b, a)]:
                        if w in d["weapons"] and m in d["ammo"]:
                            if m not in d["compat"].setdefault(w, []):
                                d["compat"][w].append(m)

# Keep only data needed to reproduce the audit; runtime stats always come from the live TSVs.
keep = ["weapons", "ammo", "armour", "body", "compat", "mods"]
data = {k: d[k] for k in keep}
data["armour"] = {
    k: v for k, v in d["armour"].items() if "renesanse_" in k and k.endswith("_6")
}
data["mods"] = {"1004": d["mods"]["1004"]}
data["provenance"] = {
    "base_commit": "009b2dabe",
    "calculator_sha256": hashlib.sha256(
        (root / "srp-026g/servers/web-server/damage/calc.js").read_bytes()
    ).hexdigest(),
    "packs": provenance,
    "dictionary_leaves_sha256": hashlib.sha256(
        args.dictionary_leaves.read_bytes()
    ).hexdigest(),
    "donor_leaves_sha256": hashlib.sha256(args.donor_leaves.read_bytes()).hexdigest(),
}
for w in data["weapons"].values():
    for k in list(w):
        if k not in ["name", "fire_queue_types", "chamber_a_round_time", "reload_time"]:
            del w[k]
for a in data["ammo"].values():
    for k in list(a):
        if k not in ["name", "air_resistance", "distance"]:
            del a[k]
# Preserve selected numeric leaves from the actual 0.69 game config.
f = args.donor_leaves
leaves = {}
roles = list(csv.DictReader((out / "roles.tsv").open(), delimiter="\t"))
names = [r["donor"] for r in roles]
for line in f.read_text().splitlines():
    k, _, v = line.partition("\t")
    m = re.match(r"weapons\.([^.]+)\.(.*)", k)
    if m and m[1] in names and v.startswith(("f32:", "i32:")):
        field = m[2]
        if (
            re.fullmatch(
                r"parameters\.(bullet_damage|rounds_per_minute|magazine_capacity|reload_time|aim_time|max_damage_distance|min_damage_distance|min_damage_amount|bullet_pierce|material_pierce|effective_distance|headshot_multiplier|item_level|match_making_level)",
                field,
            )
            or field.startswith("parameters.aim_recoil.shots_params")
            or re.match(r"parameters.aim_recoil.(.*shake.*|compensation_.*)$", field)
        ):
            leaves.setdefault(m[1], {})[field] = float(v[4:])
with (out / "donor-069.tsv").open("w") as dst:
    dst.write(
        "# Retail 0.69d0 numeric leaves; source dump SHA256 "
        + hashlib.sha256(f.read_bytes()).hexdigest()
        + "\n"
    )
    fields = [
        "weapon",
        "damage",
        "rpm",
        "magazine",
        "effective_m",
        "level",
        "aim_s",
        "reload_s",
        "first_kick",
        "mean_kick",
        "peak_kick",
    ]
    writer = csv.DictWriter(dst, fieldnames=fields, delimiter="\t", lineterminator="\n")
    writer.writeheader()
    for role in roles:
        r = leaves[role["donor"]]
        prefix = "parameters."
        kicks = [v for k, v in r.items() if "shots_params" in k and k.endswith("[0]")]
        writer.writerow(
            dict(
                weapon=role["donor"],
                damage=r[prefix + "bullet_damage"],
                rpm=r[prefix + "rounds_per_minute"],
                magazine=r[prefix + "magazine_capacity"],
                effective_m=r[prefix + "effective_distance"],
                level=r[prefix + "item_level"],
                aim_s=r[prefix + "aim_time"],
                reload_s=r[prefix + "reload_time"],
                first_kick=r[prefix + "aim_recoil.shots_params[0][0]"],
                mean_kick=round(sum(kicks) / len(kicks), 6),
                peak_kick=max(kicks),
            )
        )
selected = {r["weapon"] for r in roles}
data["weapons"] = {k: v for k, v in data["weapons"].items() if k in selected}
data["compat"] = {k: v for k, v in data["compat"].items() if k in selected}
assert set(data["compat"]) == selected, selected - set(data["compat"])
used = {a for aa in data["compat"].values() for a in aa}
data["ammo"] = {k: v for k, v in data["ammo"].items() if k in used}
(out / "reference.json").write_text(
    json.dumps(data, indent=2, ensure_ascii=False) + "\n"
)
print(
    f"Collected {len(selected)} weapons, {len(used)} ammo choices and {len(leaves)} donor records"
)
