"""Writes the damage calculator's data file from 0.26g's own configs.

Input: a directory of configs decoded to JSON, one file per resources.db
path ('/' -> '__', '.json' appended), as dump_json.py writes them:

    dbgrab <resources.db> get gameplay/ <raw>
    dbgrab <resources.db> get localization/english <raw>
    python -I dump_json.py <raw> <json>
    python -I gen_calc_data.py <json> <out.json> [<shot-motions.json>]

With the third (../wiki/shot_motions.py's, made from the same files), a weapon
has its "shot": what its shot clips do to the camera and to the gun.

Nothing here is chosen by the calculator: every number is the game's.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "released-game"))
from game_maps import ARTEFACT_HINT  # noqa: E402

source, target = sys.argv[1], sys.argv[2]
shot_motions = json.load(open(sys.argv[3], encoding="utf-8")) if len(sys.argv) > 3 else {}


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


# The other languages' names beside the English ones (`<field>_<code>`), where the game has
# them: its dictionaries, in the site's languages (site-texts/texts.py LANGUAGES).
LANGUAGES = {"uk": "ukrainian", "ru": "russian", "de": "german", "pl": "polish", "sr": "serbian", "cs": "czech",
             "hu": "hungarian", "fi": "finnish", "be": "belarusian", "hr": "croatian", "pt": "portuguese"}
strings_in = {code: load(f"localization__{folder}__localization.json")["strings"]
              if os.path.exists(os.path.join(source, f"localization__{folder}__localization.json")) else {}
              for code, folder in LANGUAGES.items()}


def text(key):
    return strings.get(key, key).strip()


def text_in(code, key):
    """The string in language `code`, or None where the game has none."""
    value = strings_in[code].get(key, "").strip()
    return value or None


def texts(field, key):
    """`key`'s string in each other language, as `<field>_<code>`."""
    return {f"{field}_{code}": text_in(code, key) for code in LANGUAGES}


def nonzero_modifiers(params):
    return {k: r(v) for k, v in params.items() if k.endswith("_modifier") and v}


def ui(cfg):
    desc = cfg.get("ui_desc", {})
    names = desc.get("text_descriptions", {})
    return {
        "name": text(names.get("name", "")),
        **texts("name", names.get("name", "")),
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
        # Its broken-arm spread (0x3317C0); the exe's default false (0x30ABBE).
        "double_handed": bool(p.get("double_handed", False)),
        # Its shot clips' own motions (wiki/shot_motions.py); absent in a build without them.
        **({"shot": shot_motions[file]} if file in shot_motions else {}),
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
        **{f"name_{c}": text_in(c, cfg["ui_desc"]["text_descriptions"]["name"])
           or pickup_name(key, strings_in[c], r"\s*" + ARTEFACT_HINT[c]) for c in LANGUAGES},
        "cooldown_s": r(d.get("cooldown_sec", 0)),
        "passive": {k: r(v) for k, v in (d.get("passive") or {}).items()},
        "active": {k: r(v) if not isinstance(v, (dict, list)) else v for k, v in (d.get("active") or {}).items()},
        "hit_type": d.get("hit_type"),
        "protected_locations": d.get("protected_body_parts", []),
        # What the game says of it: at the pickup (its passive) and at the
        # activation ("%s": the active's seconds).
        "pickup_hint": text(d["pickup_hint"]) if d.get("pickup_hint") in strings else None,
        **texts("pickup_hint", d.get("pickup_hint", "")),
        "activation_hint": text(d["activation_hint"]) if d.get("activation_hint") in strings else None,
        **texts("activation_hint", d.get("activation_hint", "")),
    }
    if artefacts[key]["active"].get("duration") is not None:
        artefacts[key]["active"]["duration_s"] = artefacts[key]["active"]["duration"]

# Drugs: protection while active (0x33db30: D = 0 below threshold, else
# (D - threshold) * hit_coeff on that pool and type), healing, injuries cleared.
drugs, drug_props = {}, {}
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
        # Native medkit construction divides each course total by duration.
        "heal_total": {p["body_part"]: r(p["amount"]) for p in rows(d.get("influences") or []) if p["amount"]},
        "heal_per_s": {p["body_part"]: r(p["amount"] / max(d.get("activity_time_sec", 0), 0.001))
                       for p in rows(d.get("influences") or []) if p["amount"]},
        "remove_affects": [[p["body_part"], p["affect"]] for p in rows(d.get("remove_affects") or [])],
    }
    drug_props[key_of(file)] = rows(cfg.get("ui_desc", {}).get("props_list") or [])

# Modifications and defects that change a gunshot or how fast they come:
# weapon damage, pierce, ranges and rate of fire, and gunshot armour. item_config_cook (0x2f1750) sums the values on
# one path, then applies them once: type 1 adds, type 2 multiplies by
# max(0, 1 + sum); a path the item lacks is skipped. A modification's type is
# the position of the 1 in its type_mask; an item's mask char j is type j+1.
RELEVANT = ("parameters/bullet_damage", "parameters/bullet_pierce", "parameters/min_damage_distance",
            "parameters/max_damage_distance", "parameters/rounds_per_minute")
# A weapon's presets (its built-in bonuses) are shown whatever they change,
# as the lobby lists them; only the relevant ones change the numbers. Armour
# keeps the relevant ones only (its presets are silver and wear).
presets = {str(d) for item in weapons.values() for d in item["default_modifications"]}
ui_props = {}
for q in rows(load("gameplay__static_game_parameters.options.json").get("ui_properties", [])):
    ui_props[q["prop_id"]] = {"label": text(q.get("prop_name", "")), **texts("label", q.get("prop_name", "")), "mod_type": q.get("mod_type", 2),
                              "direction": q.get("direction", 1), "postfix": text(q["postfix"]) if q.get("postfix") else "",
                              **{f"postfix_{c}": text_in(c, q["postfix"]) if q.get("postfix") else None for c in LANGUAGES}}


def card_line(prop_id, value, icon=None, bonus=False):
    """One line of an item's lobby card, "<prop name>: <value><postfix>". A
    bonus (a modification's) is signed, a share as a percentage, and good or not."""
    p, v = ui_props.get(prop_id, {}), float(value)

    def said(postfix):
        if bonus and p.get("mod_type", 2) != 1:
            return f"{'+' if v >= 0 else '−'}{round(abs(v) * 100)}%"
        number = f"{'+' if v >= 0 else '−'}{abs(v):g}" if bonus else f"{v:g}"
        return number + ("" if not postfix else postfix if postfix == "%" else " " + postfix)

    line = {"label": p.get("label"), **{f"label_{c}": p.get(f"label_{c}") for c in LANGUAGES},
            "value": said(p.get("postfix")), **{f"value_{c}": said(p.get(f"postfix_{c}") or p.get("postfix")) for c in LANGUAGES}}
    if icon is not None:
        line["icon"] = icon
    if bonus:
        line["good"] = (p.get("direction", 1) == 1) == (v > 0)
    return line


# A drug's card as the lobby lists it, without its weight (UI property 15).
for key, props in drug_props.items():
    drugs[key]["card"] = [card_line(q["prop_id"], q["prop_value"]) for q in props if q["prop_id"] != 15]
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
    """The relevant modification types an item may carry besides its defaults,
    and the defaults (`shown`: every preset, else the relevant ones). A type
    a default holds does not roll (modifications.rs, `candidates`)."""
    allowed = params.get("available_modifications_mask", "")
    defects = params.get("available_defects_mask", "")
    held = {type_of[d] for d in default if d in type_of}
    out = {"mod": [], "defect": []}
    for mid, m in mods.items():
        if not m["effects"] or (m["kind"] == "mod" and m["type"] in held):
            continue
        mask = allowed if m["kind"] == "mod" else defects
        j = m["type"] - 1
        if 0 <= j < len(mask) and mask[j] == "1" and m["type"] not in out[m["kind"]]:
            out[m["kind"]].append(m["type"])
    out["default"] = [str(d) for d in default if str(d) in mods and (shown or mods[str(d)]["effects"])]
    return out


# Recovery (the healing section): what changes a pool's regeneration and a
# medicine's healing besides the Larkspur. body_part::regenerate (0x32A4B0)
# multiplies each pool's speed by max(0, 1 + the kit's sum of
# health_regeneration_speed_modifier); exe patch 94 takes the kit's
# engineer_success_chance_modifier as seconds on the delay after a hit and
# its engineer_use_time_modifier as a share of the pool on the limit
# (docs/notes/regeneration-booster.md).
REGENERATION = {"parameters/health_regeneration_speed_modifier": "speed",
                "parameters/engineer_success_chance_modifier": "delay_s",
                "parameters/engineer_use_time_modifier": "limit"}


def regeneration_of(m):
    effects = (("/".join(e["path"]), e["value"]) for e in rows(m.get("modifiers") or []))
    return {REGENERATION[path]: r(value) for path, value in effects if path in REGENERATION}


every_mod = {m["id"]: m for m in rows(mods_cfg["modifications"])}
type_of = {i: m["type_mask"].index("1") for i, m in every_mod.items()}
# The bonus that rolls on equipment: one kind, its grades, and the slots with
# an item that takes it (a piece carries one bonus of a kind at most).
rolled = sorted((m for m in every_mod.values() if m.get("drop_weight", 0) > 0 and "speed" in regeneration_of(m)),
                key=lambda m: regeneration_of(m)["speed"])
kinds = {m["type_mask"].index("1") for m in rolled}
if len(kinds) > 1:
    sys.exit(f"the regeneration bonus rolls as several kinds: {sorted(kinds)}")
gear = None
if rolled:
    kind = kinds.pop()
    takes = {a["slot"] for a in armour.values() if a["available_modifications_mask"][kind - 1:kind] == "1"}
    prop = ui_props.get(rows(rolled[0]["ui_desc"]["props_list"])[0]["prop_id"], {})
    gear = {"label": prop.get("label"), **{f"label_{c}": prop.get(f"label_{c}") for c in LANGUAGES}, "icon": rolled[0]["ui_desc"].get("icon"),
            "slots": sorted(takes), "grades": [{"id": str(m["id"]), "value": regeneration_of(m)["speed"]} for m in rolled]}
# A booster's built-in bonuses, while it is in a pocket.
boosters = {}
for file in files("gameplay__items__boosters__"):
    cfg = load(file)
    total, card = {}, []
    for d in cfg.get("default_modifications", []):
        desc = every_mod.get(d, {}).get("ui_desc", {})
        card += [card_line(q["prop_id"], q["prop_value"], desc.get("icon"), bonus=True) for q in rows(desc.get("props_list") or [])]
        for k, v in regeneration_of(every_mod.get(d, {})).items():
            total[k] = r(total.get(k, 0) + v)
    if total:
        boosters[key_of(file)] = {"name": ui(cfg)["name"], **{f"name_{c}": ui(cfg)[f"name_{c}"] for c in LANGUAGES}, **total, "card": card}
# The skills the page counts, by their ids in the tree: the medical ones that
# change recovery (docs/notes/medical-skills.md) and Forest Protection, which
# changes a hit (docs/notes/artefact-skills.md); rules.js has what each does.
# Of each: its ranks and
# what one rank gives, the number the lobby prints for it (" <green>+0.5"),
# and its tooltip's line for a rank as the lobby writes it: the description,
# then that number with the postfix.
SKILLS = {311: "metabolism", 332: "strong_bones", 312: "medical_training", 313: "take_cover", 302: "fast_bandaging",
          523: "forest_protection"}
skills = {}
skills_tree = rows(load("gameplay__skills__skills_tree.options.json")["skills_tree"])
for s in skills_tree:
    if s["id"] in SKILLS:
        printed = re.sub(r"<[^>]*>", "", s["ui"]["skill_descriptions"][0][0]).strip()
        step = float(re.search(r"\d+(?:\.\d+)?", printed).group(0))
        postfix = s["ui"].get("postfix", "")
        skills[SKILLS[s["id"]]] = {
            "id": s["id"], "name": text(s["ui"]["name"]), **texts("name", s["ui"]["name"]), "ranks": s["levels_amount"],
            "step": r(step / 100 if postfix == "st_percent_postfix" else step),
            "description": text(s["ui"]["description"]), **texts("description", s["ui"]["description"]),
            # The postfix as the game has it (" s", "%"): its own space or none.
            "rank": printed + strings.get(postfix, "").rstrip(),
            **{f"rank_{c}": printed + strings_in[c][postfix].rstrip() if strings_in[c].get(postfix) else None for c in LANGUAGES}}
recovery = {"gear": gear, "boosters": boosters}

for table in (weapons, armour):
    for item in table.values():
        item["mod_types"] = item_mods(
            {"available_modifications_mask": item.pop("available_modifications_mask"),
             "available_defects_mask": item.pop("available_defects_mask")},
            item["default_modifications"], table is weapons)

# Walls a bullet goes through (survarium::bullet::try_pierce 0x32D960): the
# player's picks, each a group of materials of game_materials/game.materials
# that share one `armor`, in its order; named as retail names materials where
# they fit (in the page's words: ui.js WALLS). The armour is the game's; a group whose materials
# differ stops the build.
WALL_GROUPS = (
    ("Glass", ("glass", "glass_car", "foliage_deciduous", "foliage_coniferous", "cloth", "plastic", "paper")),
    ("Car", ("metal_car", "metal_gas_tank")),
    ("Plywood", ("wood_thin", "glass_hard", "rubber")),
    ("Tinplate", ("metall_thin", "schiefer")),
    ("Brick, steel", ("brick", "metall_thick", "wood_tree")),
    ("Concrete", ("concrete",)),
    ("Stone", ("stone",)),
)
materials = {m["name"]: m for m in load("game_materials__game.materials.json")["materials"] if not m.get("deleted")}
walls = []
for name, members in WALL_GROUPS:
    armor = {r(materials[m]["physic"]["armor"]) for m in members}
    if len(armor) != 1:
        sys.exit(f"wall {name!r}: its materials' armor differs: {sorted(armor)}")
    walls.append({"name": name, "armor": armor.pop(), "materials": list(members)})
walls.sort(key=lambda w: w["armor"])

# Map names (st_<level>_name), for the map picker.
level_names = {k.removeprefix("st_").removesuffix("_name"): {"en": v.strip(), **{c: text_in(c, k) for c in LANGUAGES}}
               for k, v in strings.items() if k.startswith("st_level_") and k.endswith("_name")}

# The wiki's simulators (scripts/wiki/spread.js, recoil.js): the player's
# stance spread and its skill weights (default.player), and what changes the
# spread and the kick. The kit's player modifiers they read: the spread's
# (dispersion_calculator 0x331330, 0x331150, 0x331560, 0x3317C0) and recoil,
# 18, each axis's kick a shot (weapon_recoil_calculator::on_shot 0x331C50).
# They come from the skills, each rank's modifiers (a rank is the sum of the
# levels up to it), and from modifications that add to an armour piece's
# parameter (type 1) and reach the kit from it (armor::add_modifiers
# 0x33F3A0); a weapon's own parameters do not (weapon_core::initialize
# 0x2E2A30 adds its movement node only). A weapon's modifications scale its
# kick ranges instead (type 2, base x max(0, 1 + sum), the cook's 0x358010).
KIT = ("dispersion", "aiming_speed", "dispersion_movement_influence", "dispersion_from_hip", "injury_penalty", "recoil")
KICK = {f"recoil/{end}_{axis}_recoil" for end in ("min", "max") for axis in ("top", "left", "right")}
# The lobby's name for the items of an armour folder (its shop filter).
SLOT_NAMES = {"helmet": "st_filter_helmet", "mask": "st_filter_mask", "torso": "st_filter_body_armor", "back": "st_backpack",
              "gloves": "st_filter_gloves", "legs": "st_filter_pants", "boots": "st_filter_boots"}


def kit_effects(params):
    return {m: r(params[f"{m}_modifier"]) for m in KIT if params.get(f"{m}_modifier")}


player = load("gameplay__players__default.player.json")["player"]
sim_skills = []
for s in sorted(skills_tree, key=lambda s: s["id"]):
    levels = [kit_effects(level) for level in rows(s.get("modifiers") or [])]
    if any(levels):
        assert len(levels) == s["levels_amount"], f"skill {s['id']}: {len(levels)} levels, {s['levels_amount']} ranks"
        ranks = [{m: r(sum(level.get(m, 0) for level in levels[:n])) for m in set().union(*levels[:n])} for n in range(len(levels) + 1)]
        sim_skills.append({"id": s["id"], "name": text(s["ui"]["name"]), **texts("name", s["ui"]["name"]), "ranks": ranks})
weapon_cfgs = {key_of(f): load(f) for f in files("gameplay__items__weapons__wpn_")}
armour_cfgs = {f: load(f) for f in files("gameplay__items__armour__")}


def rollers(cfgs, kind, m):
    """The items of `cfgs` that may roll `m`: an item's mask character j is
    type j + 1 (item_mods; srp's rolls, modifications.rs)."""
    mask, j = ("available_modifications_mask" if kind == "mod" else "available_defects_mask"), m["type_mask"].index("1")
    return [k for k, cfg in cfgs.items() if cfg.get("parameters", {}).get(mask, "")[j - 1:j] == "1"]


def takers(cfgs, kind, m):
    """The items of `cfgs` that may roll `m` or come with it."""
    return sorted(set(rollers(cfgs, kind, m)) | {k for k, cfg in cfgs.items() if m["id"] in cfg.get("default_modifications", [])})


sim_mods = []
for kind, table in (("mod", mods_cfg["modifications"]), ("defect", mods_cfg["defects"])):
    for m in rows(table):
        kit, kick = {}, {}
        for e in rows(m.get("modifiers") or []):
            path, name = "/".join(e["path"]), e["path"][-1].removesuffix("_modifier")
            if e["path"][0] == "parameters" and len(e["path"]) == 2 and name in KIT:
                assert e["type"] == 1, f"modification {m['id']}: {name} type {e['type']}, not added"
                kit[name] = r(kit.get(name, 0) + e["value"])
            elif path in KICK:
                assert e["type"] == 2, f"modification {m['id']}: {path} type {e['type']}, not a scale"
                kick[path] = r(e["value"])
        if not kit and not kick:
            continue
        (prop,) = rows(m["ui_desc"]["props_list"])
        row = {"id": str(m["id"]), "kind": kind, "type": m["type_mask"].index("1"), "icon": m["ui_desc"].get("icon"),
               "card": card_line(prop["prop_id"], prop["prop_value"], bonus=True)}
        if kit:
            if takers(weapon_cfgs, kind, m):
                sys.exit(f"modification {m['id']} changes player modifiers on weapons: no route to the kit")
            # Each armour piece adds its own (one a kind a piece): the slots
            # whose items roll it, or come with it.
            rolls = {f.split("__")[3] for f in rollers(armour_cfgs, kind, m)}
            slots = sorted({f.split("__")[3] for f in takers(armour_cfgs, kind, m)}, key=list(SLOT_NAMES).index)
            row |= {"kit": kit, "slots": [{"slot": x, "name": text(SLOT_NAMES[x]), **texts("name", SLOT_NAMES[x]), "rolls": x in rolls}
                                          for x in slots]}
        if kick:
            if kick.keys() != KICK or len(set(kick.values())) != 1 or kit:
                sys.exit(f"modification {m['id']}: kick {kick}: not one scale of the six ranges")
            row |= {"kick": next(iter(kick.values())), "weapons": takers(weapon_cfgs, kind, m),
                    "slots": [{"slot": "weapon", "name": text("st_filter_weapons"), **texts("name", "st_filter_weapons"), "rolls": True}]}
        sim_mods.append(row)
simulators = {"stance": {k: r(v) for k, v in player["character_dispersion_params"].items()},
              "skill_influence": {k: r(v) for k, v in player["character_dispersion_skill_influence"].items()},
              "skills": sim_skills,
              "mods": sorted(sim_mods, key=lambda m: (m["kind"], m["type"], -abs(sum(m.get("kit", {}).values()) + m.get("kick", 0))))}

out = {
    "source": "Survarium 0.26g resources.db (build 0.26g0)",
    "simulators": simulators,
    "level_names": level_names,
    "mods": mods,
    "drugs": drugs,
    "artefacts": artefacts,
    "walls": walls,
    "compat": compat,
    "default_ammo": default_ammo,
    "weapons": weapons,
    "ammo": ammo,
    "armour": armour,
    "body": body,
    "recovery": recovery,
    "skills": skills,
}
with open(target, "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
print(f"{len(weapons)} weapons, {len(ammo)} ammo, {len(armour)} armour, {len(body)} body rows")
