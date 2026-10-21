#!/usr/bin/env python3
"""Builds the website's wiki (`/wiki`) from the game as Skhid's players have it.

    python3 -I build.py --stock <resources.db> --patches <dir> --dbgrab <dbgrab> --icons <dir>
                        --rules <rules.json> --rules-test <rules-test.json>
                        [--with <resources.db.patch_n>] [--list release/patches.tsv] [--work <dir>]

The game data is the stock resources.db with exactly the patches of
release/patches.tsv over it, each checked by its sha256
(srp-026g/scripts/released-game). srp's own rules come from
`session::v026g::wiki::rules` (the example `wiki_rules`): --rules over the
server data made of the same archives, --rules-test over srp's test data,
whose SHA-256 the web-server test compares with the code's. --icons: the
lobby icons as the calculator's build exports them.

Writes srp-026g/servers/web-server/wiki/: pages/<slug>.<en|uk>.html (one a
language, <!--SITE-HEADER--> for the server), wiki.js, icons/, pages.rs and
files.rs (what the server includes), built-from.tsv and rules-from.tsv.
Nothing about an item is typed here: hand-written pages (text/) name their
numbers by fact, and an unknown one stops the build. Plain python; the
README says what comes from where.
"""
import argparse
import hashlib
import json
import re
import shutil
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
sys.path.insert(0, str(SRP / "scripts" / "released-game"))
sys.path.insert(0, str(SRP / "scripts" / "gen-data-026g"))
sys.path.insert(0, str(HERE))
import bincfg  # noqa: E402
from released import LIST, die, extract, offered, revision as revision_of, write_built_from  # noqa: E402
from game_maps import WANTED as MAP_WANTED, load, playable  # noqa: E402

import pages_text  # noqa: E402
from model import Game, rows  # noqa: E402
from pages_items import AFFECTS, HIT_NAMES, Items, PARTS, POOLS, pair  # noqa: E402
from pages_world import World  # noqa: E402
from web import L, esc, page, table  # noqa: E402

OUT = SRP / "servers" / "web-server" / "wiki"
WANTED = sorted(set(MAP_WANTED) - {"flash_movies/hud.swf"} | {"gameplay/"})
TYPES = {"js": "text/javascript; charset=utf-8", "webp": "image/webp"}

# The main page's sections, in order: (key, title).
SECTIONS = [("items", ("Items", "Предмети")), ("world", ("Maps and modes", "Мапи й режими")),
            ("progress", ("Progress", "Розвиток")), ("economy", ("Silver, gold, factions", "Срібло, золото, фракції")),
            ("mechanics", ("How it works", "Як це працює"))]
# Which section a generated hub page belongs to.
HUBS = {"weapons": "items", "ammo": "items", "equipment": "items", "scopes": "items", "supplies": "items",
        "camouflages": "items", "decals": "items", "maps": "world", "modes": "world", "anomalies": "world",
        "artefacts": "world", "skills": "progress", "modifications": "progress", "factions": "economy",
        "shop": "economy", "drops": "economy", "new-account": "economy"}


def version(blob):
    return hashlib.sha256(blob).hexdigest()[:12]


class Site:
    def __init__(self, icons_dir, out):
        self.pages = {}
        self.icons_dir, self.out = Path(icons_dir), out
        self.icons = {}

    def add(self, slug, title, crumbs, render, section=None, kind=None, summary=None, written=False):
        if slug in self.pages:
            die(f"two pages are {slug}")
        if isinstance(title, tuple):
            title = {"en": title[0], "uk": title[1]}
        crumbs = [(s, {"en": t[0], "uk": t[1]} if isinstance(t, tuple) else t) for s, t in crumbs]
        if isinstance(summary, tuple):
            summary = {"en": summary[0], "uk": summary[1]}
        if isinstance(kind, tuple):
            kind = {"en": kind[0], "uk": kind[1]}
        self.pages[slug] = {"title": title, "crumbs": crumbs, "render": render, "section": section,
                            "kind": kind, "summary": summary, "written": written}

    def icon(self, name):
        """An icon's address, the file copied beside the pages."""
        if name not in self.icons:
            src = self.icons_dir / f"{name}.webp"
            if not src.is_file():
                return None
            blob = src.read_bytes()
            (self.out / "icons").mkdir(parents=True, exist_ok=True)
            (self.out / "icons" / f"{name}.webp").write_bytes(blob)
            self.icons[name] = f"/wiki/icons/{name}.webp?v={version(blob)}"
        return self.icons[name]


def read_configs(raw):
    """Every gameplay config and the two localizations, decoded, by path
    (files that are no config, as sounds, are left out)."""
    configs = {}
    for f in sorted(Path(raw).iterdir()):
        if not f.name.startswith(("gameplay__", "localization__")):
            continue
        try:
            configs[f.name.replace("__", "/")] = bincfg.parse(f.read_bytes())
        except Exception:  # noqa: BLE001 - not a config
            continue
    return configs


def facts_of(game, world):
    """The numbers hand-written pages name: {name: f(L) -> text}."""
    g, p = game, game.player
    f = {}
    states = rows(g.battery.get("states"))
    take = next(s["take_time"] for s in states if "take_time" in s)
    put = next(s["put_time"] for s in states if "put_time" in s)
    f["battery.take_time"] = lambda L: L.sec(take)
    f["battery.put_time"] = lambda L: L.sec(put)
    f["battery.speed"] = lambda L: L.pct(g.battery["parameters"]["movement_speed_modifier"], 0)
    fall = p["fall_params"]
    f["fall.harmless"] = lambda L: L.metres(fall["harmless_height"])
    f["fall.fatal"] = lambda L: L.metres(fall["fatal_height"])
    f["fall.damage"] = lambda L: L.hp(fall["fatal_damage"])
    f["fall.gravity"] = lambda L: L.num(fall["gravity"], 2)
    stamina = p["stamina_params"]
    f["carry.max"] = lambda L: L.kg(stamina["max_carried_weight"])
    f["carry.slow"] = lambda L: L.pct(stamina["max_carried_weight_movement_speed_modifier"], 0)
    for pool in ("body", "head", "right_leg", "pain"):
        row = g.body[pool]
        f[f"{pool}.health"] = lambda L, row=row: L.hp(row["health"])
        f[f"{pool}.regen_wait"] = lambda L, row=row: L.sec(row["regeneration_timeout"])
        f[f"{pool}.regen_speed"] = lambda L, row=row: L.hp(row["regeneration_speed"])
        f[f"{pool}.regen_cap"] = lambda L, row=row: L.pct(row["regeneration_threshold"], 0)
    f["toxins.health"] = lambda L: L.hp(g.body["infection"]["health"])
    names, by_mode = world.mode_rows()

    def mode_value(mode, key, fmt):
        values = sorted(by_mode.get(mode, {}).get(key, ()))
        if not values:
            die(f"fact: no {key} in mode {mode}")
        return lambda L: " / ".join(fmt(L, v) for v in values)

    f["mode.match_time"] = mode_value(2, "match_time", lambda L, v: L.num(v / 60, 1) + L.t(" min", " хв"))
    f["mode.countdown"] = mode_value(2, "countdown_time", lambda L, v: L.sec(v))
    f["mode.respawn"] = mode_value(2, "respawn_time", lambda L, v: L.sec(v))
    f["mode.batteries"] = mode_value(2, "victory_items_count", lambda L, v: L.num(v, 0))
    f["mode.chargers_working"] = mode_value(3, "spawn_points_count", lambda L, v: L.num(v, 0))
    f["mode.charger_every"] = mode_value(3, "vi_spawn_time", lambda L, v: L.sec(v))
    f["mode.charger_variation"] = mode_value(3, "vi_spawn_variation", lambda L, v: L.sec(v))
    f["mode.tickets"] = mode_value(4, "tickets_count", lambda L, v: L.num(v, 0))
    f["mode.start_share"] = mode_value(2, "wait_player_percent", lambda L, v: L.pct(v, 0))
    r = g.rules
    rooms = r["rooms"]
    f["rooms.min"] = lambda L: L.num(rooms["sizes"][0], 0)
    f["rooms.max"] = lambda L: L.num(rooms["sizes"][1], 0)
    f["rooms.start_min"] = lambda L: L.num(rooms["min_sides"][0], 0)
    f["rooms.start_max"] = lambda L: L.num(rooms["min_sides"][1], 0)
    f["rooms.wait_min"] = lambda L: L.sec(min(rooms["waits"]))
    f["rooms.wait_max"] = lambda L: L.sec(max(rooms["waits"]))
    f["rooms.level_max"] = lambda L: L.num(rooms["level_max"], 0)
    f["kits"] = lambda L: L.num(r["kits"], 0)
    f["exchange.rate"] = lambda L: L.num(r["exchange_rate"], 0)
    f["skills.reset_silver"] = lambda L: L.money(r["skills_reset"]["silver"])
    f["skills.reset_gold"] = lambda L: L.money(r["skills_reset"]["gold"], True)
    m = r["modifications"]
    f["mods.max"] = lambda L: L.num(m["max"], 0)
    f["mods.reroll_choices"] = lambda L: L.num(m["reroll_choices"], 0)
    f["mods.reroll_wait"] = lambda L: L.num(m["reroll_wait_s"] / 3600, 1) + L.t(" h", " год")
    f["mods.reroll_growth"] = lambda L: "×" + L.num(m["reroll_growth"], 2)
    d = r["drops"]
    f["drops.chance"] = lambda L: L.pct(d["chance"], 0)
    f["drops.per_battery"] = lambda L: L.pct(d["per_battery"], 1)
    f["drops.premium"] = lambda L: L.pct(d["premium"], 0)
    f["drops.condition"] = lambda L: L.num(d["condition"], 0) + " %"
    s = r["signup"]
    f["signup.silver"] = lambda L: L.money(s["silver"])
    f["signup.gold"] = lambda L: L.money(s["gold"], True)
    return f


def tables_of(game, items, world):
    """The generated tables hand-written pages place: {name: f(L) -> html}."""
    g = game
    t = {}

    def regen(L):
        body = []
        for pool in ("body", "head", "right_leg", "right_hand", "pain"):
            row = g.body[pool]
            full = row["regeneration_timeout"] + row["health"] * row["regeneration_threshold"] / row["regeneration_speed"] if row["regeneration_speed"] else None
            body.append([esc(pair(L, POOLS.get(pool, (pool, pool)))), L.hp(row["health"]), L.sec(row["regeneration_timeout"]),
                         L.hp(row["regeneration_speed"]) + L.t("/s", "/с"), L.pct(row["regeneration_threshold"], 0),
                         L.sec(full) if full else "—"])
        return table(L, [L.t("Pool", "Запас"), "#" + L.t("Full", "Повний"), "#" + L.t("Starts after", "Починає через"),
                         "#" + L.t("Speed", "Швидкість"), "#" + L.t("Up to", "До"), "#" + L.t("Empty to the cap", "Від нуля до межі")], body, sort=False)
    t["regen"] = regen

    def medicine(L):
        body = []
        for i in sorted(g.of_categories({10}), key=lambda i: i.level):
            if not i.cfg_name.startswith("gameplay/items/drugs/") or not g.obtainable(i):
                continue
            d = i.data
            body.append([items.ic(i), items.link(L, i), esc(items.effect_line(L, i)), L.sec(d.get("activation_delay_sec", 0)),
                         L.sec(d.get("activity_time_sec", 0))])
        return table(L, ["", L.t("Item", "Річ"), L.t("Does", "Дія"), "#" + L.t("After", "Через"), "#" + L.t("Lasts", "Триває")], body, sort=False)
    t["medicine"] = medicine

    def regen_bonuses(L):
        """What else speeds regeneration: modifications, boosters, skills."""
        out = []
        steps = [m for m in g.mods.values() if any(p == "parameters/health_regeneration_speed_modifier" for p, _, _ in m["modifiers"])]
        kinds = sorted({m["type"] for m in steps})
        holders = [i for i in g.items.values() if g.obtainable(i) and any(g.mods.get(mid, {}).get("type") in kinds for mid, _ in i.mods)]
        if steps:
            out.append(f"<p>{L.a('modifications', L.t('Modification', 'Модифікація'))} «{esc(items.mod_label(L, steps[0]))}»: "
                       + " · ".join(items.mod_value(L, m) for m in steps if m["drop_weight"]) + ". "
                       + L.t("Rolls on", "Випадає на") + " " + L.num(len(holders), 0) + " " + L.t("items", "предметах") + ".</p>")
        boosters = [i for i in g.items.values() if g.obtainable(i) and any(m in [s["id"] for s in steps] for m in i.defaults)]
        if boosters:
            out.append("<p>" + ", ".join(f"{items.link(L, i)} ({esc(items.effect_line(L, i))})" for i in boosters) + ".</p>")
        skill_rows = []
        for s in g.skills:
            for lvl in rows(s.get("modifiers")):
                if "health_regeneration_speed_modifier" in lvl or "pain_regeneration_speed_modifier" in lvl:
                    skill_rows.append(s)
                    break
        if skill_rows:
            out.append(f"<p>{L.a('skills', L.t('Skills', 'Навички'))}: " + ", ".join(esc(L.n(g.texts(s["ui"]["name"]))) for s in skill_rows) + ".</p>")
        return "".join(out)
    t["regen_bonuses"] = regen_bonuses

    def fall(L):
        fp = g.player["fall_params"]
        gv, h0, h1, top = fp["gravity"], fp["harmless_height"], fp["fatal_height"], fp["fatal_damage"]
        v0, v1 = (2 * gv * h0) ** 0.5, (2 * gv * h1) ** 0.5
        body = []
        for h in range(int(h0), int(h1) + 1):
            v = (2 * gv * h) ** 0.5
            dmg = 0 if v <= v0 else top if v >= v1 else top * (v - v0) / (v1 - v0)
            body.append([L.metres(h), L.num(v, 1) + L.t(" m/s", " м/с"), L.hp(dmg)])
        return table(L, ["#" + L.t("Height", "Висота"), "#" + L.t("Landing speed", "Швидкість"), "#" + L.t("Damage to each foot", "Ушкодження кожній стопі")], body, sort=False)
    t["fall"] = fall

    def upgrades(L):
        m = g.rules["modifications"]
        body = []
        for lv in range(len(m["craft_price_silver"])):
            body.append([L.num(lv, 0), L.money(m["craft_price_silver"][lv]), L.money(m["craft_price_gold"][lv], True),
                         L.money(m["reroll_price_silver"][lv]), L.money(m["reroll_price_gold"][lv], True)])
        return table(L, ["#" + L.t("Item level", "Рівень предмета"), "#" + L.t("Upgrade, silver", "Покращення, срібло"),
                         "#" + L.t("Upgrade, gold", "Покращення, золото"), "#" + L.t("Change, silver", "Зміна, срібло"),
                         "#" + L.t("Change, gold", "Зміна, золото")], body, sort=False)
    t["upgrade_prices"] = upgrades

    def chances(L):
        m = g.rules["modifications"]
        body = []
        for diff in range(len(m["craft_chances_silver"])):
            s, gd = m["craft_chances_silver"][diff], m["craft_chances_gold"][diff]
            body.append([L.num(diff, 0), f"{L.num(s[0])} % / {L.num(s[1])} %", f"{L.num(gd[0])} % / {L.num(gd[1])} %"])
        return table(L, ["#" + L.t("Item has more mods than the donor by", "Модифікацій більше, ніж у донора, на"),
                         L.t("Silver: +1 / −1", "Срібло: +1 / −1"), L.t("Gold: +1 / −1", "Золото: +1 / −1")], body, sort=False)
    t["upgrade_chances"] = chances

    def hit_types(L):
        front = g.body["front"]["hit_types"]
        body = [[esc(pair(L, HIT_NAMES[h])), esc(", ".join(pair(L, POOLS.get(p, (p, p))) for p in (front.get(h, {}).get("bdb_coeff") or {})))]
                for h in HIT_NAMES if h in front]
        return table(L, [L.t("Damage type", "Тип ушкодження"), L.t("Takes from (chest)", "Знімає з (груди)")], body, sort=False)
    t["hit_types"] = hit_types

    def routes(L):
        """Where a bullet's damage goes from each hit box, and how much."""
        body = []
        for part, name in PARTS.items():
            row = g.body.get(part)
            if not row:
                continue
            ranged = (row["hit_types"].get("ranged") or {}).get("bdb_coeff") or {}
            body.append([esc(pair(L, name)), esc(", ".join(f"{pair(L, POOLS.get(k, (k, k)))} ×{L.num(v)}" for k, v in ranged.items()))])
        return table(L, [L.t("Hit box", "Зона"), L.t("Takes from", "Знімає з")], body, sort=False)
    t["routes"] = routes

    def injuries(L):
        """What a pool's level does (`thresholds`): injuries and slowdowns."""
        body = []
        for pool in ("head", "right_leg", "right_hand", "body", "pain", "infection"):
            row = g.body.get(pool)
            for th in rows(row.get("thresholds")) if row else []:
                does = [pair(L, AFFECTS.get(a, (str(a), str(a)))) for a in rows(th.get("affects"))]
                if th.get("movement_speed_modifier"):
                    does.append(L.t("speed", "швидкість") + " " + L.pct(th["movement_speed_modifier"], 0))
                if does:
                    at = L.t("empty", "порожній") if th["value"] == 0 else L.t("below ", "нижче ") + L.pct(th["value"], 0)
                    body.append([esc(pair(L, POOLS.get(pool, (pool, pool)))), at, esc(", ".join(does))])
        return table(L, [L.t("Pool", "Запас"), L.t("When", "Коли"), L.t("Effect", "Наслідок")], body, sort=False)
    t["injuries"] = injuries

    def az(L):
        return glossary(L, site_ref[0])
    t["az"] = az
    return t


site_ref = [None]


def glossary(L, site):
    """Every page, A to Z in the reader's language."""
    entries = []
    for slug, p in site.pages.items():
        if slug in ("", "glossary"):
            continue
        section = dict(SECTIONS).get(p["section"])
        kind = L.n(p["kind"]) if p["kind"] else (L.t(*section) if section else "")
        entries.append((L.n(p["title"]), slug, kind))
    entries.sort(key=lambda e: e[0].lower())
    by_letter = defaultdict(list)
    for title, slug, kind in entries:
        by_letter[title[:1].upper()].append((title, slug, kind))
    out = ['<input class="find" type="search" data-find="az" placeholder="' + L.t("Find", "Пошук") + '">']
    body = []
    for letter in sorted(by_letter):
        for title, slug, kind in by_letter[letter]:
            body.append([esc(letter), L.a(slug, title), esc(kind)])
    out.append(table(L, ["", L.t("Entry", "Стаття"), L.t("Kind", "Вид")], body, cls="az"))
    return "".join(out)


def index_page(site, game):
    def render(L):
        out = [f"<p class=lead>{L.t('Everything about Survarium 0.26g as Skhid plays it, from the game data of the latest release.', 'Усе про Survarium 0.26g, як його грає Схід, з даних гри останнього випуску.')} "
               f"{L.a('glossary', L.t('A to Z', 'Від А до Я'))}</p>"]
        for key, title in SECTIONS:
            members = [(s, p) for s, p in site.pages.items() if (p["section"] == key and (s in HUBS or p["written"]))]
            if not members:
                continue
            out.append(f"<h2>{esc(L.t(*title))}</h2><div class=tiles>")
            for s, p in members:
                summary = L.n(p["summary"]) if p["summary"] else ""
                out.append(f'<a class="tile" href="{L.href(s)}"><b>{esc(L.n(p["title"]))}</b>'
                           + (f"<span>{esc(summary)}</span>" if summary else "") + "</a>")
            out.append("</div>")
        out.append(f'<p class=muted><a href="/damage?lang={L.code}">{L.t("Calculator", "Калькулятор")}</a> · '
                   f'<a href="/maps?lang={L.code}">{L.t("Maps", "Мапи")}</a></p>')
        return "".join(out)
    site.add("", ("Wiki", "Вікі"), [], render, section=None)


SUMMARIES = {
    "weapons": ("Every gun by class, with sellers and prices.", "Уся зброя за класами, з продавцями й цінами."),
    "ammo": ("What each round does and fits.", "Що робить кожен набій і до чого пасує."),
    "equipment": ("Helmets to boots: armour, isolation, weight.", "Від шолома до черевиків: броня, ізоляція, вага."),
    "scopes": ("Which scope fits which gun.", "Який приціл на яку зброю."),
    "supplies": ("Medicine, boosters, grenades, traps.", "Ліки, бустери, гранати, пастки."),
    "camouflages": ("Weapon and gear paint.", "Фарбування зброї та спорядження."),
    "decals": ("Patches for gear.", "Нашивки на спорядження."),
    "maps": ("Every playable map and its modes.", "Усі мапи та їхні режими."),
    "modes": ("Rules and timers of each mode.", "Правила й таймери кожного режиму."),
    "anomalies": ("Hazard zones: damage, where, protection.", "Небезпечні зони: ушкодження, де, захист."),
    "artefacts": ("What each artefact does.", "Що робить кожен артефакт."),
    "skills": ("The tree, value by value.", "Дерево навичок, значення за значенням."),
    "modifications": ("Every bonus and defect, steps and chances.", "Усі бонуси й дефекти, кроки й шанси."),
    "factions": ("Ranks and what each sells.", "Ранги і що кожна продає."),
    "shop": ("Bundles, premium time, gold items.", "Набори, преміум-час, речі за золото."),
    "drops": ("What a match can give.", "Що може дати матч."),
    "new-account": ("What a new player starts with.", "З чим починає новий гравець."),
}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--stock", required=True)
    ap.add_argument("--patches", required=True, help="the folder with the list's resources.db.patch_<n>")
    ap.add_argument("--list", default=str(LIST))
    ap.add_argument("--with", dest="with_patch", metavar="PATCH", help="the next release's resources.db.patch_<n>, not in the list yet")
    ap.add_argument("--dbgrab", required=True)
    ap.add_argument("--icons", required=True, help="the lobby icons as <config name>.webp (the calculator's build)")
    ap.add_argument("--rules", required=True, help="wiki_rules over the server data of these archives")
    ap.add_argument("--rules-test", required=True, help="wiki_rules --test-data")
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--work", help="keep the extracted files here and reuse them")
    args = ap.parse_args()
    rows_ = offered(args.list, args.with_patch)
    revision = revision_of(rows_)
    out = Path(args.out)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(args.work or tmp)
        raw = work / "raw"
        if not (args.work and raw.is_dir()):
            raw.mkdir(parents=True, exist_ok=True)
            extract(args.dbgrab, args.stock, rows_, args.patches, WANTED, raw)
        configs = read_configs(raw)
        maps = playable(raw)
        projects = {}
        for m in maps.values():
            for mm in m["modes"].values():
                for path in mm["projects"][:1]:
                    projects[path] = load(raw, f"projects/{path}/client_project")
    rules = json.loads(Path(args.rules).read_text())
    for sub in ("pages", "icons"):
        shutil.rmtree(out / sub, ignore_errors=True)
    out.mkdir(parents=True, exist_ok=True)
    icons = {p.stem for p in Path(args.icons).glob("*.webp")}
    game = Game(configs, rules, icons)
    site = Site(args.icons, out)
    site_ref[0] = site
    items = Items(site, game)
    world = World(site, game, items, maps, projects)
    items.weapons()
    items.ammo()
    items.armour()
    items.scopes()
    items.supplies()
    items.cosmetics()
    world.maps_pages()
    world.modes()
    world.anomalies()
    world.artefacts()
    world.factions()
    world.skills()
    world.modifications()
    world.economy()
    for slug, s in SUMMARIES.items():
        if slug in site.pages:
            site.pages[slug]["summary"] = {"en": s[0], "uk": s[1]}
            site.pages[slug]["section"] = HUBS[slug]
    pages_text.add_all(site, facts_of(game, world), tables_of(game, items, world))
    index_page(site, game)

    # The pages, one file a language.
    script = (HERE / "wiki.js").read_text()
    (out / "wiki.js").write_text(script)
    script_url = f"/wiki/wiki.js?v={version(script.encode())}"
    faces = "\n".join(line for line in (SRP / "servers" / "web-server" / "damage" / "damage.html").read_text().splitlines()
                      if line.startswith("@font-face"))
    (out / "pages").mkdir(parents=True, exist_ok=True)
    stamp = {"en": f"Skhid revision {revision} · from the game data and the server's rules",
             "uk": f"Схід, ревізія {revision} · з даних гри та правил сервера"}
    broken = set()
    for slug, p in sorted(site.pages.items()):
        for code in ("en", "uk"):
            lang = L(code)
            body = p["render"](lang)
            broken |= {(slug, target) for target in re.findall(r'href="/wiki/([^"?#]*)', body) if target not in site.pages}
            crumbs = [("", {"en": "Wiki", "uk": "Вікі"})] + p["crumbs"] if slug else []
            html_ = page(lang, lang.n(p["title"]), body, crumbs, faces, script_url, stamp[code])
            (out / "pages" / f"{slug or 'index'}.{code}.html").write_text(html_)
    if broken:
        die("links to no page: " + ", ".join(f"{s or 'index'} -> {t}" for s, t in sorted(broken)))
    names = sorted(site.pages)
    (out / "pages.rs").write_text(
        "// Built by srp-026g/scripts/wiki/build.py; do not edit. The pages: address, English, Ukrainian.\n"
        "pages! {\n" + "".join(f'    "{s}" => "{s or "index"}",\n' for s in names) + "}\n")
    served = ["wiki.js"] + [f"icons/{f.name}" for f in sorted((out / "icons").iterdir())] if (out / "icons").is_dir() else ["wiki.js"]
    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/wiki/build.py; do not edit. The files the pages load.\n"
        "files! {\n" + "".join(f'    "{f}" => "{TYPES[f.rsplit(".", 1)[1]]}",\n' for f in served) + "}\n")
    write_built_from(out / "built-from.tsv", rows_, "the wiki (srp-026g/scripts/wiki/build.py)")
    test_sha = hashlib.sha256(Path(args.rules_test).read_bytes()).hexdigest()
    (out / "rules-from.tsv").write_text(
        "# The SHA-256 of session::v026g::wiki::rules over srp's test data when the wiki was built\n"
        "# (cargo run -p session --example wiki_rules -- --test-data): a web-server test compares it\n"
        "# with the code's, so a change of srp's rules rebuilds the wiki.\n"
        f"rules-test\t{test_sha}\n")
    count = defaultdict(int)
    for p in site.pages.values():
        count["written" if p["written"] else "generated"] += 1
    print(f"{out}: revision {revision}, {len(site.pages)} pages ({count['generated']} generated, {count['written']} written), "
          f"{len(site.icons)} icons")


if __name__ == "__main__":
    main()
