"""Writes the damage calculator's data file from 0.26g's own configs.

Input: a directory of configs decoded to JSON, one file per resources.db
path ('/' -> '__', '.json' appended), as dump_json.py writes them:

    dbgrab <resources.db> get gameplay/ <raw>
    dbgrab <resources.db> get localization/english <raw>
    python -I dump_json.py <raw> <json>
    python -I gen_calc_data.py <json> <out.json>

Nothing here is chosen by the calculator: every number is the game's.
"""
import json
import os
import sys

source, target = sys.argv[1], sys.argv[2]


def load(name):
    with open(os.path.join(source, name), encoding="utf-8") as f:
        return json.load(f)




def rows(table):
    """A config table is a list, or a dict when it has one entry per key."""
    return table if isinstance(table, list) else list(table.values())


# The item dictionary says which items exist (Skhid's patches add rows and
# leave retired configs behind) and names them. An item's key is its config's
# file name, or the dictionary's name where file names clash (the exo pieces
# are all duty_exo_026 / freedom_exo_026, one per slot).
dictionary = load("gameplay__db_static_dictionaries.json")
by_id = {i["dict_id"]: i for i in dictionary["items_dict"]}
by_cfg = {i["cfg_name"]: i for i in by_id.values()}
stems = {}
for c in by_cfg:
    stems.setdefault(c.rsplit("/", 1)[-1].removesuffix(".options"), []).append(c)


def cfg_of(file):
    return file.removesuffix(".json").replace("__", "/")


def key_of(file):
    stem = file.rsplit("__", 1)[1].removesuffix(".options.json")
    row = by_cfg.get(cfg_of(file))
    return row["name"] if row and len(stems.get(stem, [])) > 1 else stem


def files(prefix):
    """The configs under `prefix` the dictionary lists."""
    return sorted(f for f in os.listdir(source)
                  if f.startswith(prefix) and f.endswith(".options.json") and cfg_of(f) in by_cfg)


def r(value, digits=6):
    """f32 values as the shortest decimal that is still close enough."""
    return round(float(value), digits) if isinstance(value, (int, float)) else value


strings = load("localization__english__localization.json")["strings"]


# Ukrainian names beside the English ones, where the game has them.
uk_path = os.path.join(source, "localization__ukrainian__localization.json")
strings_uk = load("localization__ukrainian__localization.json")["strings"] if os.path.exists(uk_path) else {}


def text(key):
    return strings.get(key, key).strip()


def text_uk(key):
    """The Ukrainian string, or None where the game has none."""
    value = strings_uk.get(key, "").strip()
    return value or None


def nonzero_modifiers(params):
    return {k: r(v) for k, v in params.items() if k.endswith("_modifier") and v}


def ui(cfg):
    desc = cfg.get("ui_desc", {})
    names = desc.get("text_descriptions", {})
    return {
        "name": text(names.get("name", "")),
        "name_uk": text_uk(names.get("name", "")),
        "description": text(names.get("description", "")),
        "icon": desc.get("icon"),
    }


weapons = {}
for file in files("gameplay__items__weapons__wpn_"):
    cfg = load(file)
    p = cfg["parameters"]
    weapons[key_of(file)] = {
        **ui(cfg),
        "category": by_cfg[cfg_of(file)]["item_category"],
        "type": p["type"],
        "premium": key_of(file).endswith("_premium"),
        "item_level": p.get("item_level", 0),
        "faction": p.get("faction", 0),
        "bullet_damage": r(p["bullet_damage"]),
        "bullet_pierce": r(p["bullet_pierce"]),
        "min_damage_amount": r(p["min_damage_amount"]),
        "max_damage_distance": r(p["max_damage_distance"]),
        "min_damage_distance": r(p["min_damage_distance"]),
        "rounds_per_minute": r(p.get("rounds_per_minute", 0)),
        "magazine_capacity": p.get("magazine_capacity", 0),
        "fire_queue_types": p.get("fire_queue_types"),
        "stopping_power": r(p.get("stopping_power", 0)),
        "stamina_damage": r(p.get("stamina_damage", 0)),
        "reload_time": r(p.get("reload_time", 0)),
        "weight": r(p.get("weight", 0)),
        # The wiki uses the same source data, including the detailed weapon card.
        # Missing optional parameters stay absent; native defaults are applied
        # by the presentation schema, never by substituting arbitrary zeroes.
        **{k: r(p[k]) for k in ("aim_time", "show_time", "hide_time", "aim_zoom_factor",
                                "movement_speed_modifier") if k in p},
        **{section: {k: r(v) for k, v in cfg.get(section, {}).items()}
           for section in ("dispersion", "recoil", "breath_vibration")},
        "modifiers": nonzero_modifiers(p),
        "default_modifications": cfg.get("default_modifications", []),
        "available_modifications_mask": p.get("available_modifications_mask", ""),
        "available_defects_mask": p.get("available_defects_mask", ""),
    }

AMMO_DATA = (
    "k_damage", "k_arp", "k_min_damage_amount", "k_max_damage_distance",
    "k_min_damage_distance", "air_resistance", "muzzle_speed", "buck_shot",
    "buck_dispersion", "dispersion", "k_stamina_damage", "k_power", "mass",
    "distance", "type", "ricochet_chance", "ricochet_angle",
)
ammo = {}
for file in files("gameplay__items__weapons__ammo__"):
    cfg = load(file)
    p, d = cfg["parameters"], cfg["data"]
    ammo[key_of(file)] = {
        **ui(cfg),
        "item_level": p.get("item_level", 0),
        "clip_size": p.get("clip_size", 0),
        **{k: r(d[k]) for k in AMMO_DATA if k in d},
    }

# additional_model_slots names the slots as the character model does.
SLOT_OF_MODEL = {"pants": "legs"}
armour = {}
for file in files("gameplay__items__armour__"):
    cfg = load(file)
    p = cfg["parameters"]
    parts = {}
    for part, row in cfg.get("hit_params", {}).items():
        parts[part] = {t: r(v.get("armor", 0)) for t, v in row.get("hit_types", {}).items()}
    armour[key_of(file)] = {
        **ui(cfg),
        "slot": file.split("__")[3],
        # Slots the item's model takes over: a closed helmet takes the mask.
        "takes_slots": [SLOT_OF_MODEL.get(s, s) for s in cfg.get("data", {}).get("additional_model_slots", [])],
        "type": p.get("type", ""),
        "premium": key_of(file).endswith("_premium"),
        "item_level": p.get("item_level", 0),
        "faction": p.get("faction", 0),
        "weight": r(p.get("weight", 0)),
        "hit_params": parts,
        "modifiers": nonzero_modifiers(p),
        "default_modifications": cfg.get("default_modifications", []),
        "available_modifications_mask": p.get("available_modifications_mask", ""),
        "available_defects_mask": p.get("available_defects_mask", ""),
    }

body = []
for row in load("gameplay__hit_params__human_hit_params.options.json")["hit_params"]:
    ranged = row["hit_types"].get("ranged", {})
    body.append({
        "name": row["name"],
        "health": r(row["health"]),
        "regeneration_timeout": r(row["regeneration_timeout"]),
        "regeneration_speed": r(row["regeneration_speed"]),
        "regeneration_threshold": r(row["regeneration_threshold"]),
        "thresholds": row["thresholds"] if isinstance(row["thresholds"], list) else [],
        "ranged_routes": {k: r(v) for k, v in (ranged.get("bdb_coeff") or {}).items()},
        "routes": {
            t: {k: r(v) for k, v in (h.get("bdb_coeff") or {}).items()}
            for t, h in row["hit_types"].items()
        },
        "invulnerable": [t for t, h in row["hit_types"].items() if h.get("invulnerable")],
    })

# Which ammo a weapon fires: the dictionary's items_compatibility pairs, the
# only link there is (ammo has no calibre). The game has no default ammo; the
# page starts on the cheapest.
cost = {key_of(i["cfg_name"].replace("/", "__") + ".json"): i.get("base_cost", 0) for i in by_id.values()}
compat = {}
pairs = dictionary["items_compatibility"]
for pair in pairs.values() if isinstance(pairs, dict) else pairs:
    a, b = by_id.get(pair["first_item"]), by_id.get(pair["second_item"])
    if not a or not b:
        continue
    ka = key_of(a["cfg_name"].replace("/", "__") + ".json")
    kb = key_of(b["cfg_name"].replace("/", "__") + ".json")
    for w, m in ((ka, kb), (kb, ka)):
        if w in weapons and m in ammo:
            compat.setdefault(w, set()).add(m)
compat = {w: sorted(s, key=lambda m: (cost.get(m, 0), m)) for w, s in compat.items()}
default_ammo = {w: s[0] for w, s in compat.items()}

# Artefacts: their data blocks as they are. Onyx is the one that changes a
# hit (0x312bf0); Spring and Lifebone only touch movement and regeneration.
# 0.26g ships no names for the artefacts' configs; the game's pickup hints
# name them (st_<artefact>_pickup_hint: "Onyx artifact. ...", "Артефакт
# Онікс. ..."): the Lifebone config is the Larkspur, Живокіст.
def pickup_name(key, strings, pattern):
    import re
    m = re.match(pattern, strings.get(f"st_{key}_pickup_hint", ""))
    return m.group(1) if m else None


artefacts = {}
for file in files("gameplay__items__artefacts__"):
    cfg = load(file)
    d = cfg["data"]
    key = key_of(file).removeprefix("artefact_")
    artefacts[key] = {
        **ui(cfg),
        "name": text(cfg["ui_desc"]["text_descriptions"]["name"]) if cfg["ui_desc"]["text_descriptions"]["name"] in strings
        else pickup_name(key, strings, r"\s*(.+?) artifact\.") or key,
        "name_uk": text_uk(cfg["ui_desc"]["text_descriptions"]["name"]) or pickup_name(key, strings_uk, r"\s*Артефакт (.+?)\."),
        "cooldown_s": r(d.get("cooldown_sec", 0)),
        "passive": {k: r(v) for k, v in (d.get("passive") or {}).items()},
        "active": {k: r(v) if not isinstance(v, (dict, list)) else v for k, v in (d.get("active") or {}).items()},
        "hit_type": d.get("hit_type"),
        "protected_locations": d.get("protected_body_parts", []),
    }
    if artefacts[key]["active"].get("duration") is not None:
        artefacts[key]["active"]["duration_s"] = artefacts[key]["active"]["duration"]

# Drugs: protection while active (0x33db30: D = 0 below threshold, else
# (D - threshold) * hit_coeff on that pool and type), healing, injuries cleared.
drugs = {}
# The medicines are made whole at gameplay/items/medicines/ (docs/notes/medicines.md).
for file in files("gameplay__items__drugs__") + files("gameplay__items__medicines__"):
    cfg = load(file)
    d = cfg["data"]
    drugs[key_of(file)] = {
        **ui(cfg),
        "activation_delay_s": r(d.get("activation_delay_sec", 0)),
        "activity_s": r(d.get("activity_time_sec", 0)),
        "damage_protection": [
            {"pool": p["body_part"], "hit_type": p["hit_type"], "hit_coeff": r(p["hit_coeff"]), "threshold": r(p["threshold"])}
            for p in rows(d.get("damage_protection") or [])
        ],
        "heal_per_s": {p["body_part"]: r(p["amount"]) for p in rows(d.get("influences") or []) if p["amount"]},
        "remove_affects": [[p["body_part"], p["affect"]] for p in rows(d.get("remove_affects") or [])],
    }

# Modifications and defects that change a gunshot or how fast they come:
# weapon damage, pierce, ranges and rate of fire, and gunshot armour. item_config_cook (0x2f1750) sums the values on
# one path, then applies them once: type 1 adds, type 2 multiplies by
# max(0, 1 + sum); a path the item lacks is skipped. A modification's type is
# the position of the 1 in its type_mask; an item's mask char j is type j+1.
RELEVANT = ("parameters/bullet_damage", "parameters/bullet_pierce", "parameters/min_damage_distance",
            "parameters/max_damage_distance", "parameters/rounds_per_minute")
# A weapon's presets (premium weapons) are shown whatever they change, as the
# lobby lists them; only the relevant ones change the numbers. Armour keeps
# the relevant ones only (its presets are silver and wear).
presets = {str(d) for item in weapons.values() for d in item["default_modifications"]}
ui_props = {}
for q in rows(load("gameplay__static_game_parameters.options.json").get("ui_properties", [])):
    ui_props[q["prop_id"]] = {"label": text(q.get("prop_name", "")), "label_uk": text_uk(q.get("prop_name", "")), "mod_type": q.get("mod_type", 2),
                              "direction": q.get("direction", 1), "postfix": text(q["postfix"]) if q.get("postfix") else ""}
mods_cfg = load("gameplay__items__modifications__modifications.options.json")
mods = {}
for kind, table in (("mod", mods_cfg["modifications"]), ("defect", mods_cfg["defects"])):
    for m in rows(table):
        effects = []
        for e in rows(m.get("modifiers") or []):
            path = "/".join(e["path"])
            ranged_armour = path.startswith("hit_params/") and path.endswith("/ranged/armor")
            if path in RELEVANT or ranged_armour:
                effects.append({"path": path, "op": "add" if e["type"] == 1 else "mul", "value": r(e["value"])})
        if effects or str(m["id"]) in presets:
            desc = m.get("ui_desc", {})
            mods[str(m["id"])] = {
                "kind": kind, "type": m["type_mask"].index("1"), "effects": effects,
                "icon": desc.get("icon"),
                # What the lobby prints: "<prop name>: <value>", per ui_properties.
                "props": [{**ui_props.get(q["prop_id"], {}), "value": float(q["prop_value"])}
                          for q in rows(desc.get("props_list") or [])],
            }


def item_mods(params, default, shown):
    """The relevant modification types an item may carry, and its defaults
    (`shown`: every preset, else the relevant ones)."""
    allowed = params.get("available_modifications_mask", "")
    defects = params.get("available_defects_mask", "")
    out = {"mod": [], "defect": []}
    for mid, m in mods.items():
        if not m["effects"]:
            continue
        mask = allowed if m["kind"] == "mod" else defects
        j = m["type"] - 1
        if 0 <= j < len(mask) and mask[j] == "1" and m["type"] not in out[m["kind"]]:
            out[m["kind"]].append(m["type"])
    out["default"] = [str(d) for d in default if str(d) in mods and (shown or mods[str(d)]["effects"])]
    return out


for table in (weapons, armour):
    for item in table.values():
        item["mod_types"] = item_mods(
            {"available_modifications_mask": item.pop("available_modifications_mask"),
             "available_defects_mask": item.pop("available_defects_mask")},
            item["default_modifications"], table is weapons)

# Map names (st_<level>_name), for the map picker.
level_names = {k.removeprefix("st_").removesuffix("_name"): {"en": v.strip(), "uk": text_uk(k)}
               for k, v in strings.items() if k.startswith("st_level_") and k.endswith("_name")}

# The player's stance spread (character_dispersion_params): the wiki's spread
# model takes the standing still values, aimed and from the hip.
stance = load("gameplay__players__default.player.json")["player"]["character_dispersion_params"]
character_dispersion = {k: r(stance[k]) for k in ("idle_dispersion", "idle_aim_dispersion")}

out = {
    "source": "Survarium 0.26g resources.db (build 0.26g0)",
    "character_dispersion": character_dispersion,
    "level_names": level_names,
    "mods": mods,
    "drugs": drugs,
    "artefacts": artefacts,
    "compat": compat,
    "default_ammo": default_ammo,
    "weapons": weapons,
    "ammo": ammo,
    "armour": armour,
    "body": body,
}
with open(target, "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
print(f"{len(weapons)} weapons, {len(ammo)} ammo, {len(armour)} armour, {len(body)} body rows")
