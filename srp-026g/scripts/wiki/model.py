"""The game as the wiki shows it: every item, map, mode and rule read from
the released game data (the configs, decoded) and from srp's own rules
(`session::v026g::wiki::rules`, as JSON). Nothing here is a number of its
own: each value is a config's, a localization's or srp's.

    game = Game(configs, rules, icons)   # configs: {path: decoded config}
"""
import math
import re
from collections import defaultdict

# The item dictionary's categories (`items_dict.item_category`), as the
# lobby's shop filter names them (localization `st_filter_*`).
WEAPON_CLASSES = [(8, "st_filter_assault_rifle"), (15, "st_filter_submachine_gun"), (16, "st_filter_machine_gun"),
                  (13, "st_filter_sniper_rifle"), (14, "st_filter_shotgun"), (17, "st_filter_pistol")]
AMMO_CLASSES = [(9, "st_filter_ammo_assault_rifle"), (20, "st_filter_ammo_pistol"), (21, "st_filter_ammo_machine_gun"),
                (18, "st_filter_ammo_sniper_rifle"), (19, "st_filter_ammo_shotgun")]
ARMOUR_SLOTS = [(1, "st_filter_helmet"), (2, "st_filter_mask"), (3, "st_filter_body_armor"), (6, "st_filter_gloves"),
                (5, "st_filter_pants"), (7, "st_filter_boots"), (4, "st_filter_rig")]
SUPPLY = {10: "st_filter_medkit", 11: "st_filter_mine", 12: None, 22: None, 24: "st_filter_grenade",
          25: "st_filter_camo", 26: "st_filter_decal", 27: "st_filter_vouchers"}
SCOPE_CATEGORY = 22
CAMO, DECAL = 25, 26

# The armour's hit types, in the order the protection tables show them.
HIT_TYPES = ["ranged", "blunt", "slash", "explosive", "fire", "shock", "toxic", "radiation"]
# A lobby card's prop ids that are percents of 1 (the card shows x100).
MARKUP = re.compile(r"<[^>]+>")
# The text an unfinished item description has in 0.26g's data.
PLACEHOLDER = "ОПИСАНИЕ НЕ ГОТОВО"


def clean(text):
    """A localization string as plain text: the game's colour tags and
    line ends out, one space between words."""
    return re.sub(r"\s+", " ", MARKUP.sub("", text or "")).strip()


def rows(table):
    """A config table is a list, or a dict when it has one entry per key."""
    if table is None:
        return []
    return table if isinstance(table, list) else list(table.values())


def r(value, digits=4):
    return round(float(value), digits) if isinstance(value, (int, float)) else value


def slug(name):
    return re.sub(r"[^a-z0-9_]+", "-", name.lower()).strip("-")


class Texts:
    def __init__(self, en, uk):
        self.en, self.uk = en, uk

    def has(self, key):
        return bool(key) and bool(clean(self.en.get(key)))

    def __call__(self, key, fallback=None):
        """{en, uk} for a localization key; Ukrainian falls back to English."""
        en = clean(self.en.get(key or "", "")) or (fallback if fallback is not None else (key or ""))
        uk = clean(self.uk.get(key or "", "")) or en
        return {"en": en, "uk": uk}


class Item:
    """One row of the item dictionary with its config."""

    def __init__(self, game, row, cfg):
        self.row, self.cfg = row, cfg or {}
        self.dict_id = row["dict_id"]
        self.name = row["name"]
        self.slug = slug(self.name)
        self.category = row["item_category"]
        self.premium = bool(row.get("is_premium"))
        self.stack = bool(row.get("is_stack"))
        self.base_cost = row.get("base_cost", 0)
        self.cfg_name = row["cfg_name"]
        self.params = self.cfg.get("parameters") or {}
        self.data = self.cfg.get("data") or {}
        ui = self.cfg.get("ui_desc") or {}
        names = ui.get("text_descriptions") or {}
        self.title = game.texts(names.get("name"), self.name)
        self.description = game.texts(names.get("description"), "") if game.texts.has(names.get("description")) else None
        # Some descriptions are the developers' "not ready" placeholder.
        if self.description and PLACEHOLDER in self.description["en"].upper():
            self.description = None
        self.props = [(int(q["prop_id"]), q["prop_value"]) for q in rows(ui.get("props_list"))]
        self.level = self.params.get("item_level", 0)
        self.faction = self.params.get("faction", 0)
        self.weight = self.params.get("weight", 0)
        self.icon = None
        rules = game.rules_items.get(self.dict_id, {})
        self.sellers = rules.get("sellers", [])
        self.sell = rules.get("sell", 0)
        self.unit = rules.get("unit", 1)
        self.repair = rules.get("repair_from_zero", 0)
        self.droppable = rules.get("droppable", False)
        self.mods = rules.get("modifications", [])
        self.defects = rules.get("defects", [])
        self.defaults = [int(d) for d in self.cfg.get("default_modifications") or []]
        self.bundles = []
        self.kind = None


class Game:
    def __init__(self, configs, rules, icons):
        self.configs = configs
        self.rules = rules
        self.texts = Texts(configs["localization/english/localization"]["strings"],
                           configs["localization/ukrainian/localization"]["strings"])
        self.dictionary = configs["gameplay/db_static_dictionaries"]
        self.rules_items = {i["dict_id"]: i for i in rules["items"]}
        self.items = {}
        for row in self.dictionary["items_dict"]:
            cfg = configs.get(row["cfg_name"])
            self.items[row["dict_id"]] = Item(self, row, cfg)
        self.by_name = {i.name: i for i in self.items.values()}
        # The icons as the calculator's build names them: the config's file
        # name, or the dictionary's name where file names clash.
        stems = defaultdict(list)
        for i in self.items.values():
            stems[i.cfg_name.rsplit("/", 1)[-1].removesuffix(".options")].append(i)
        for stem, items in stems.items():
            for i in items:
                key = i.name if len(items) > 1 else stem
                if key in icons:
                    i.icon = key
        for bundle in rules["bundles"]:
            for name, count in bundle["items"]:
                if name in self.by_name:
                    self.by_name[name].bundles.append((bundle, count))
        self.starter = set(rules["starter"]["worn"]) | set(rules["starter"]["stash"])
        self._modifications()
        self._compat()
        self._factions()
        self.ui_props = {}
        for q in rows(configs["gameplay/static_game_parameters.options"].get("ui_properties")):
            self.ui_props[q["prop_id"]] = {"label": self.texts(q.get("prop_name"), ""),
                                           "postfix": self.texts(q.get("postfix"), "") if q.get("postfix") else None,
                                           "mod_type": q.get("mod_type"), "direction": q.get("direction", 1)}
        self.player = configs["gameplay/players/default.player"]["player"]
        self.body = {row["name"]: row for row in configs["gameplay/hit_params/human_hit_params.options"]["hit_params"]}
        self.battery = configs["gameplay/victory_items/default.options"]
        self.skills = configs["gameplay/skills/skills_tree.options"]["skills_tree"]

    # ----- what a player can have
    def obtainable(self, item):
        return bool(item.sellers or item.droppable or item.bundles or item.name in self.starter)

    def of_categories(self, categories):
        return [i for i in self.items.values() if i.category in categories]

    # ----- modifications
    def _modifications(self):
        cfg = self.configs["gameplay/items/modifications/modifications.options"]
        self.mods, self.defect_rows = {}, {}
        for kind, table, out in (("mod", cfg["modifications"], self.mods), ("defect", cfg["defects"], self.defect_rows)):
            for m in rows(table):
                desc = m.get("ui_desc") or {}
                out[m["id"]] = {
                    "id": m["id"], "kind": kind, "type": m["type_mask"].index("1"),
                    "drop_weight": m.get("drop_weight", 0),
                    "props": [(int(q["prop_id"]), float(q["prop_value"])) for q in rows(desc.get("props_list"))],
                    "modifiers": [("/".join(e["path"]), e["type"], float(e["value"])) for e in rows(m.get("modifiers"))],
                    "fix": (m.get("fix_cost_general"), m.get("fix_cost_gold"), m.get("fix_time_general")),
                }

        # A parameter's name on the lobby card: the prop of the modifications
        # that change it (`parameters/<name>` -> prop id).
        self.param_props = {}
        for row in list(self.mods.values()) + list(self.defect_rows.values()):
            for path, _, _ in row["modifiers"]:
                if path.startswith("parameters/") and row["props"]:
                    self.param_props.setdefault(path.split("/", 1)[1], row["props"][0][0])

    def mod(self, mod_id):
        return self.mods.get(mod_id) or self.defect_rows.get(mod_id)

    # ----- which ammo a weapon takes: the dictionary's items_compatibility
    def _compat(self):
        self.ammo_of, self.weapons_of = defaultdict(list), defaultdict(list)
        weapon_cats = {c for c, _ in WEAPON_CLASSES}
        ammo_cats = {c for c, _ in AMMO_CLASSES}
        for pair in rows(self.dictionary["items_compatibility"]):
            a, b = self.items.get(pair["first_item"]), self.items.get(pair["second_item"])
            if not a or not b:
                continue
            for w, m in ((a, b), (b, a)):
                if w.category in weapon_cats and m.category in ammo_cats and m not in self.ammo_of[w.dict_id]:
                    self.ammo_of[w.dict_id].append(m)
                    self.weapons_of[m.dict_id].append(w)
        for table in (self.ammo_of, self.weapons_of):
            for k in table:
                table[k].sort(key=lambda i: (i.level, i.base_cost, i.name))
        self.scopes_of, self.weapons_of_scope = defaultdict(list), defaultdict(list)
        for fit in self.rules["scopes"]:
            w, s = self.items.get(fit["weapon"]), self.items.get(fit["scope"])
            if w and s:
                self.scopes_of[w.dict_id].append(s)
                self.weapons_of_scope[s.dict_id].append(w)

    # ----- factions and their reputation levels
    def _factions(self):
        self.factions = {}
        for f in rows(self.dictionary["factions_dict"]):
            self.factions[f["id"]] = {"id": f["id"], "name": self.texts(f["name"]),
                                      "levels": [{"value": lv["value"], "name": self.texts(lv["name"])} for lv in f["levels"]]}

    def seller_name(self, seller):
        if seller == 0:
            return None
        return self.factions.get(seller, {}).get("name")

    # ----- shooting: a weapon with an ammo, as the calculator's engine
    def ballistics(self, w, a):
        """Damage and penetration near and far, and the two distances, of
        weapon w firing ammo a (scripts/damage-calc/engine.js `bulletAt`)."""
        p, d = w.params, a.data
        W, Q, m = p["bullet_damage"], p["bullet_pierce"], p["min_damage_amount"]
        v0, drag = d.get("muzzle_speed", 0), d.get("air_resistance", 0)
        reach = math.inf
        if drag > 0:
            tc = 1 / (1.1 * drag)
            reach = v0 * (tc - drag * tc * tc / 2)
        return {
            "d_max": W * d["k_damage"], "d_min": W * m * d["k_min_damage_amount"] * d["k_damage"],
            "p_max": Q * d["k_arp"], "p_min": Q * m * d["k_min_damage_amount"] * d["k_arp"],
            "near": min(p["max_damage_distance"] * d["k_max_damage_distance"], reach),
            "far": min(p["min_damage_distance"] * d["k_min_damage_distance"], reach),
            "pellets": max(1, round(d.get("buck_shot") or 1)),
        }
