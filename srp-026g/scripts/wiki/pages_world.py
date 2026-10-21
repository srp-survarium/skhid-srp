"""The pages of the world and the rules around the items: maps, modes,
anomalies, factions, skills, modifications, artefacts, the shop, drops.
Every number is the game data's or srp's (`rules.json`)."""
import re
from collections import Counter, defaultdict

from model import WEAPON_CLASSES, ARMOUR_SLOTS, rows
from pages_items import HIT_NAMES, PARTS, POOLS, pair
from web import esc, table

# The modes a room plays (srp's rooms window: client-patches/rooms/SrpRooms.as
# MODES): the game names modes 2 and 3 alike; the rooms add ": chargers".
MODE_SUFFIX = {3: (": chargers", ": зарядники")}
# maps_dict's map_weather_id, as the rooms window names them (SrpRooms.as WEATHER).
WEATHER = {1: ("day", "день"), 2: ("rain", "дощ"), 3: ("evening", "вечір"), 4: ("night", "ніч"), 5: ("before rain", "перед дощем")}
GATHER, TIMED = 88, 89


class World:
    def __init__(self, site, game, items, maps, projects):
        self.site, self.game, self.items = site, game, items
        self.maps, self.projects = maps, projects

    def mode_name(self, L, mode, mm):
        return L.n(self.game.texts(mm["mode_name"])) + pair(L, MODE_SUFFIX.get(mode, ("", "")))

    # ----- maps
    def map_info(self, folder):
        """What a map's projects hold, from the first weather of each mode."""
        m = self.maps[folder]
        info = {"modes": {}, "hazards": Counter(), "cores": [], "artefacts": Counter()}
        for mode in sorted(m["modes"]):
            mm = m["modes"][mode]
            project = self.projects[mm["projects"][0]]
            so = project.get("server_objects") or {}
            teams = Counter(o.get("team", -1) for o in so.get("respawn_points") or [])
            info["modes"][mode] = {
                "mm": mm, "respawns": teams,
                "spawners": len(so.get("victory_item_spawners") or []) if mm["rules"] & {GATHER} else 0,
                "stations": len(so.get("victory_item_spawn_points") or []) if TIMED in mm["rules"] else 0,
                "containers": len(so.get("victory_items_containers") or []) if mm["rules"] & {GATHER, TIMED} else 0,
                "weathers": sorted({c["weather"] for c in mm["configs"]}),
            }
        project = self.projects[next(iter(m["modes"].values()))["projects"][0]]
        artefact_names = {i.dict_id: i for i in self.game.of_categories({12})}
        for o in project.get("client_objects") or []:
            t = o.get("game_object_type")
            if t == "damage_zone" and o.get("max_hit", 0) > 0:
                key = (o.get("damage_type"), round(o["max_hit"], 4), o.get("interval_in_msec", 0),
                       tuple(o.get("hit_parts_filter") or ()), bool(o.get("continuous_hit")))
                info["hazards"][key] += 1
            if t == "generic_anomaly_core" and o.get("artefacts_enabled"):
                for a in o.get("artefacts") or []:
                    if a["dict_id"] in artefact_names:
                        info["artefacts"][a["dict_id"]] += 1
                info["cores"].append(o)
        return info

    def map_list(self):
        return sorted(self.maps, key=lambda f: (f.startswith("v"), f))

    def maps_pages(self):
        for folder in self.map_list():
            m = self.maps[folder]
            self.site.add("map-" + folder.replace("_", "-"), self.game.texts(m["map_name"]), [("maps", ("Maps", "Мапи"))],
                          lambda L, f=folder: self.map_page(L, f), section="maps")
        self.site.add("maps", ("Maps", "Мапи"), [], self.maps_index, section="maps")

    def maps_index(self, L):
        body = []
        for folder in self.map_list():
            m, info = self.maps[folder], self.map_info(folder)
            modes = ", ".join(self.mode_name(L, k, v["mm"]) for k, v in info["modes"].items())
            weathers = sorted({w for v in info["modes"].values() for w in v["weathers"]})
            body.append([L.a("map-" + folder.replace("_", "-"), self.game.texts(m["map_name"])), esc(modes),
                         esc(", ".join(pair(L, WEATHER.get(w, (str(w),) * 2)) for w in weathers)),
                         (L.num(sum(info["hazards"].values()), 0), sum(info["hazards"].values())),
                         f'<a href="/maps?lang={L.code}#{folder}">{L.t("spawns", "точки")}</a>'])
        return (f"<p>{L.t('Every map a room can play.', 'Усі мапи, які може грати кімната.')} "
                f'<a href="/maps?lang={L.code}">{L.t("The Maps tab", "Вкладка «Мапи»")}</a> '
                f"{L.t('shows respawns, batteries, chargers, containers and artefacts.', 'показує відродження, батареї, зарядники, контейнери й артефакти.')}</p>"
                + table(L, [L.t("Map", "Мапа"), L.t("Modes", "Режими"), L.t("Weather", "Погода"), "#" + L.t("Hazard zones", "Небезпечні зони"), ""], body))

    def map_page(self, L, folder):
        info, g = self.map_info(folder), self.game
        out = [f'<p><a href="/maps?lang={L.code}#{folder}">{L.t("On the Maps tab", "На вкладці «Мапи»")}</a> · '
               f"{L.a('modes', L.t('Modes', 'Режими'))}</p>"]
        body = []
        for mode, v in info["modes"].items():
            p = v["mm"]["parameters"]
            per_team = " / ".join(L.num(v["respawns"].get(t, 0), 0) for t in (0, 1))
            body.append([f'<a href="/maps?lang={L.code}#{folder}/{mode}">{esc(self.mode_name(L, mode, v["mm"]))}</a>',
                         esc(", ".join(pair(L, WEATHER.get(w, (str(w),) * 2)) for w in v["weathers"])),
                         L.num(p.get("match_time", 0) / 60, 1) + L.t(" min", " хв"), per_team,
                         L.num(p["victory_items_count"], 0) if mode in (2, 3) and "victory_items_count" in p else "—",
                         L.num(v["spawners"], 0) if v["spawners"] else "—",
                         f"{L.num(p.get('spawn_points_count', 0), 0)} / {L.num(v['stations'], 0)}" if v["stations"] else "—",
                         L.num(p["tickets_count"], 0) if "tickets_count" in p else "—"])
        out.append(f"<h2>{L.t('Modes', 'Режими')}</h2>" + table(L, [
            L.t("Mode", "Режим"), L.t("Weather", "Погода"), "#" + L.t("Time", "Час"), "#" + L.t("Respawns per team", "Відроджень на команду"),
            "#" + L.t("Batteries", "Батарей"), "#" + L.t("Battery spawns", "Появ батарей"), "#" + L.t("Chargers working / on map", "Зарядників працює / на мапі"),
            "#" + L.t("Tickets", "Квитків")], body, sort=False))
        if info["hazards"]:
            body = []
            for (kind, hit, ms, parts, cont), count in sorted(info["hazards"].items(), key=lambda kv: (kv[0][0], -kv[0][1])):
                body.append([L.a("anomalies", pair(L, HIT_NAMES.get(kind, (kind, kind)))), (L.hp(hit), hit), L.sec(ms / 1000),
                             esc(", ".join(dict.fromkeys(pair(L, PARTS.get(x, (x, x))) for x in parts)) or "—"),
                             L.t("while inside", "поки всередині") if cont else L.t("on entering", "на вході"), (L.num(count, 0), count)])
            out.append(f"<h2>{L.t('Hazard zones', 'Небезпечні зони')}</h2>" + table(L, [
                L.t("Damage", "Ушкодження"), "#" + L.t("Per hit", "За раз"), "#" + L.t("Every", "Кожні"), L.t("Hits", "Б'є"),
                L.t("When", "Коли"), "#" + L.t("Zones", "Зон")], body))
        if info["artefacts"]:
            names = [f"{L.a('artefacts', self.artefact_name(g.items[a]))} ×{L.num(n, 0)}" for a, n in sorted(info["artefacts"].items())]
            out.append(f"<h2>{L.t('Artefacts', 'Артефакти')}</h2><p>{L.t('Anomalies that grow them', 'Аномалії, де вони ростуть')}: {', '.join(names)}.</p>")
        return "".join(out)

    # ----- modes
    def modes(self):
        self.site.add("modes", ("Game modes", "Режими гри"), [], self.modes_page, section="modes")

    def mode_rows(self):
        """Each mode's parameters as the configs of its maps give them
        (they are alike on every map; a differing value is listed)."""
        by_mode = defaultdict(lambda: defaultdict(set))
        names = {}
        for m in self.maps.values():
            for mode, mm in m["modes"].items():
                names[mode] = mm
                for k, v in mm["parameters"].items():
                    by_mode[mode][k].add(v)
        return names, by_mode

    def modes_page(self, L):
        names, by_mode = self.mode_rows()
        g = self.game
        labels = {"match_time": (L.t("Match time", "Час матчу"), lambda v: L.num(v / 60, 1) + L.t(" min", " хв")),
                  "countdown_time": (L.t("Countdown", "Відлік"), L.sec),
                  "respawn_time": (L.t("Respawn", "Відродження"), L.sec),
                  "victory_items_count": (L.t("Batteries", "Батарей"), lambda v: L.num(v, 0)),
                  "vi_spawn_time": (L.t("A charger gives a battery every", "Зарядник дає батарею кожні"), L.sec),
                  "vi_spawn_variation": (L.t("… give or take", "… плюс-мінус"), L.sec),
                  "spawn_points_count": (L.t("Chargers working at once", "Зарядників працює одночасно"), lambda v: L.num(v, 0)),
                  "tickets_count": (L.t("Tickets per team", "Квитків на команду"), lambda v: L.num(v, 0)),
                  "wait_player_percent": (L.t("Starts when this many are in", "Починається, коли зайшло"), lambda v: L.pct(v, 0))}
        rooms = g.rules["rooms"]
        out = [f"<p>{L.t('Rooms play these, ', 'Кімнати грають ці режими, ')}{L.num(rooms['sizes'][0], 0)}–{L.num(rooms['sizes'][1], 0)} "
               f"{L.t('a side.', 'гравців на бік.')} {L.a('rooms', L.t('Rooms', 'Кімнати'))}</p>"]
        for mode in sorted(names):
            mm = names[mode]
            ui = g.configs["gameplay/game_modes/" + mm["configs"][0]["config"]]["mode"]["ui"]
            out.append(f"<h2>{esc(self.mode_name(L, mode, mm))}</h2><p class=lead>{esc(L.n(g.texts(ui['description'])))}</p><ul>")
            for key, (label, fmt) in labels.items():
                if key in by_mode[mode]:
                    values = sorted(by_mode[mode][key])
                    out.append(f"<li>{label}: {' / '.join(fmt(v) for v in values)}</li>")
            out.append("</ul>")
            if mode in (2, 3):
                out.append(f"<p>{L.a('batteries', L.t('How batteries work', 'Як працюють батареї'))}</p>")
            count = sum(1 for m in self.maps.values() if mode in m["modes"])
            out.append(f"<p class=muted>{L.num(count, 0)} {L.t('maps', 'мап')} · {L.a('maps', L.t('Maps', 'Мапи'))}</p>")
        return "".join(out)

    # ----- anomalies
    def anomalies(self):
        self.site.add("anomalies", ("Anomalies", "Аномалії"), [], self.anomalies_page, section="world")

    def anomalies_page(self, L):
        g = self.game
        front = g.body["front"]["hit_types"]
        out = [f"<p>{L.t('Zones that hurt while you stand in them, or as you step in. Each hits the listed body parts with its damage type; armour of that type on the part takes its share off, as with bullets.', 'Зони, що шкодять, поки ви в них, або на вході. Кожна б’є перелічені частини тіла своїм типом ушкодження; броня цього типу на частині зменшує його, як і з кулями.')} "
               f"{L.a('damage', L.t('Damage', 'Ушкодження'))}</p>"]
        body = []
        for hit in ("toxic", "radiation", "fire", "shock", "blunt"):
            pools = front.get(hit, {}).get("bdb_coeff") or {}
            body.append([esc(pair(L, HIT_NAMES[hit])),
                         esc(", ".join(pair(L, POOLS.get(p, (p, p))) for p in pools)) or "—"])
        out.append(f"<h2>{L.t('Damage types', 'Типи ушкодження')}</h2>" + table(L, [L.t("Type", "Тип"), L.t("Takes from", "Знімає з")], body, sort=False))
        tox = g.body.get("infection")
        if tox:
            out.append(f"<p>{L.t('Toxins: a pool of its own, ', 'Токсини: окремий запас, ')}{L.hp(tox['health'])}"
                       f"{L.t(', that does not recover by itself; empty, you die. ', ', що сам не відновлюється; на нулі — смерть. ')}"
                       f"{L.a('healing', L.t('What heals it', 'Що його лікує'))}</p>")
        body = []
        for folder in self.map_list():
            info = self.map_info(folder)
            for (kind, hit, ms, parts, cont), count in sorted(info["hazards"].items()):
                body.append([L.a("map-" + folder.replace("_", "-"), g.texts(self.maps[folder]["map_name"])),
                             esc(pair(L, HIT_NAMES.get(kind, (kind, kind)))), (L.hp(hit), hit), (L.sec(ms / 1000), ms),
                             (L.num(hit * 100 * 1000 / ms, 1), hit * 1000 / ms),
                             esc(", ".join(dict.fromkeys(pair(L, PARTS.get(x, (x, x))) for x in parts)) or "—"),
                             L.t("while inside", "поки всередині") if cont else L.t("on entering", "на вході"), (L.num(count, 0), count)])
        out.append(f"<h2>{L.t('On the maps', 'На мапах')}</h2>" + table(L, [
            L.t("Map", "Мапа"), L.t("Type", "Тип"), "#" + L.t("Per hit", "За раз"), "#" + L.t("Every", "Кожні"),
            "#" + L.t("Per second", "За секунду"), L.t("Hits", "Б'є"), L.t("When", "Коли"), "#" + L.t("Zones", "Зон")], body))
        out.append(f"<p class=muted>{L.t('Per hit is before armour. Zones that only show effects are left out.', 'За раз — до броні. Зони лише з ефектами пропущено.')}</p>")
        # Protection against them: what each piece gives against the types.
        body = []
        for i in g.items.values():
            if i.category in {c for c, _ in ARMOUR_SLOTS} and g.obtainable(i):
                best = {h: self.items.protection(i, h) for h in ("toxic", "radiation", "fire", "shock")}
                if any(best.values()):
                    body.append([self.items.ic(i), self.items.link(L, i)] + [(L.hp(best[h]), best[h]) for h in best])
        out.append(f"<h2>{L.t('Protection', 'Захист')}</h2>" + table(L, ["", L.t("Item", "Річ")] + ["#" + pair(L, HIT_NAMES[h]) for h in ("toxic", "radiation", "fire", "shock")], body))
        return "".join(out)

    # ----- artefacts
    def artefacts(self):
        self.site.add("artefacts", ("Artefacts", "Артефакти"), [], self.artefacts_page, section="world")

    def artefact_name(self, item):
        """0.26g ships no names for the artefacts' configs; the pickup hints
        name them ("Onyx artifact. ...", "Артефакт Онікс. ...")."""
        g = self.game
        key = item.name.removeprefix("artefact_")
        if g.texts.has((item.cfg.get("ui_desc") or {}).get("text_descriptions", {}).get("name")):
            return item.title
        en = re.match(r"\s*(.+?) artifact\.", g.texts.en.get(f"st_{key}_pickup_hint", ""))
        uk = re.match(r"\s*Артефакт (.+?)\.", g.texts.uk.get(f"st_{key}_pickup_hint", ""))
        return {"en": en.group(1) if en else key, "uk": uk.group(1) if uk else (en.group(1) if en else key)}

    def artefacts_page(self, L):
        g = self.game
        out = []
        for a in sorted(g.of_categories({12}), key=lambda i: i.name):
            d = a.data
            key = a.name.removeprefix("artefact_")
            hint = g.texts(f"st_{key}_pickup_hint", "")
            out.append(f"<h2>{esc(L.n(self.artefact_name(a)))}</h2>")
            if self.items.icon(a):
                out.append(f'<img src="{self.items.icon(a)}" alt="" width="171" height="83">')
            if L.n(hint):
                out.append(f"<p class=lead>{esc(L.n(hint))}</p>")
            facts = [(L.t("Cooldown", "Перезарядка"), L.sec(d["cooldown_sec"])) if d.get("cooldown_sec") else None]
            for k, v in (d.get("passive") or {}).items():
                if isinstance(v, (int, float)):
                    facts.append((L.t("Passive", "Пасивно") + f" <code>{esc(k)}</code>", L.num(v, 3)))
            for k, v in (d.get("active") or {}).items():
                if isinstance(v, (int, float)):
                    facts.append((L.t("Active", "Активно") + f" <code>{esc(k)}</code>", L.num(v, 3)))
            out.append("<ul>" + "".join(f"<li>{k}: {v}</li>" for k, v in filter(None, facts)) + "</ul>")
            where = [L.a("map-" + f.replace("_", "-"), g.texts(self.maps[f]["map_name"])) for f in self.map_list()
                     if self.map_info(f)["artefacts"].get(a.dict_id)]
            if where:
                out.append(f"<p>{L.t('Found on', 'Трапляється на')}: {', '.join(where)}.</p>")
        out.append(f"<p class=muted>{L.a('healing', L.t('Healing', 'Лікування'))} · <a href=\"/damage?lang={L.code}\">{L.t('Calculator', 'Калькулятор')}</a></p>")
        return "".join(out)

    # ----- factions
    def factions(self):
        self.site.add("factions", ("Factions", "Фракції"), [], self.factions_page, section="economy")

    def factions_page(self, L):
        g = self.game
        out = [f"<p>{L.t('Each faction sells its gear by reputation rank. Reputation comes from matches and quests.', 'Кожна фракція продає своє спорядження за рангом репутації. Репутація — за матчі й завдання.')} "
               f"{L.a('shop', L.t('Shop', 'Магазин'))}</p>"]
        sold = defaultdict(list)
        for i in g.items.values():
            for s in i.sellers:
                if not s["gold"]:
                    sold[(s["seller"], s["level"])].append(i)
        gear = {c for c, _ in WEAPON_CLASSES} | {c for c, _ in ARMOUR_SLOTS}
        for fid in sorted(g.factions):
            f = g.factions[fid]
            if not any(k[0] == fid for k in sold):
                continue
            out.append(f"<h2>{esc(L.n(f['name']))}</h2>")
            body = []
            for lv, level in enumerate(f["levels"]):
                items = sorted(sold.get((fid, lv), []), key=lambda i: (i.category not in gear, i.category, i.level))
                body.append([(L.num(lv, 0), lv), esc(L.n(level["name"])), (L.num(level["value"], 0), level["value"]),
                             ", ".join(self.items.link(L, i) for i in items) or "—"])
            out.append(table(L, ["#" + L.t("Rank", "Ранг"), L.t("Title", "Звання"), "#" + L.t("Reputation", "Репутація"),
                                 L.t("Sells", "Продає")], body, sort=False))
        anyone = sorted((i for i in g.items.values() if any(s["seller"] == 0 and not s["gold"] for s in i.sellers)),
                        key=lambda i: (i.category, i.name))
        out.append(f"<h2>{L.t('Any trader', 'Будь-який торговець')}</h2><p>{', '.join(self.items.link(L, i) for i in anyone)}</p>")
        return "".join(out)

    # ----- skills
    def skills(self):
        self.site.add("skills", ("Skills", "Навички"), [], self.skills_page, section="progress")

    def skills_page(self, L):
        g = self.game
        out = [f"<p>{L.t('One point a level. A skill opens when its branch has the points it asks for. A reset costs', 'Одне очко за рівень. Навичка відкривається, коли в гілці є потрібні очки. Скидання коштує')} "
               f"{L.money(g.rules['skills_reset']['silver'])} {L.t('or', 'або')} {L.money(g.rules['skills_reset']['gold'], True)}.</p>"]
        branches = defaultdict(list)
        for s in g.skills:
            branches[s["id"] // 100].append(s)
        for b, skills in sorted(branches.items()):
            out.append(f"<h2>{L.t('Branch', 'Гілка')} {b}</h2>")
            body = []
            for s in sorted(skills, key=lambda s: (s["preconditions"].get("branch_points", 0), s["ui"]["position"])):
                mods = rows(s.get("modifiers"))
                total = defaultdict(float)
                steps = []
                for lvl in mods:
                    for k, v in lvl.items():
                        total[k] += v
                    steps.append(", ".join(self.skill_value(L, k, total[k]) for k in total))
                body.append([esc(L.n(g.texts(s["ui"]["name"]))), esc(L.n(g.texts(s["ui"].get("description"), ""))),
                             (L.num(s["preconditions"].get("branch_points", 0), 0), s["preconditions"].get("branch_points", 0)),
                             " → ".join(steps)])
            out.append(table(L, [L.t("Skill", "Навичка"), L.t("Does", "Дія"), "#" + L.t("Needs points", "Потрібно очок"),
                                 L.t("By points taken", "За вкладеними очками")], body))
        return "".join(out)

    def skill_value(self, L, name, value):
        label, text = self.items.param_bonus(L, name, value)
        return text

    # ----- modifications
    def modifications(self):
        self.site.add("modifications", ("Modifications", "Модифікації"), [], self.mods_page, section="progress")

    def mods_page(self, L):
        g, rules = self.game, self.game.rules["modifications"]
        out = [f"<p>{L.t('Bonuses on a weapon or a piece of gear. At most', 'Бонуси на зброї чи спорядженні. Щонайбільше')} "
               f"{L.num(rules['max'], 0)} {L.t('per item, each of another kind.', 'на предмет, кожен іншого виду.')} "
               f"{L.a('upgrades', L.t('Upgrades and rerolls', 'Покращення й зміна модифікацій'))}</p>"]
        users = defaultdict(set)
        for i in g.items.values():
            if not g.obtainable(i):
                continue
            for mid, _ in i.mods:
                users[g.mods[mid]["type"]].add(i.category)
        cats = {c: L.n(g.texts(k)) for c, k in WEAPON_CLASSES + ARMOUR_SLOTS}
        for title, table_, kind in ((L.t("Bonuses", "Бонуси"), g.mods, "mod"), (L.t("Defects", "Дефекти"), g.defect_rows, "defect")):
            by_type = defaultdict(list)
            for m in table_.values():
                by_type[m["type"]].append(m)
            body = []
            for t, ms in sorted(by_type.items()):
                total = sum(m["drop_weight"] for m in ms) or 1
                steps = " · ".join(f"{self.items.mod_value(L, m)}" + (f" <small>({L.pct(m['drop_weight'] / total, 0)})</small>" if m["drop_weight"] else "") for m in ms)
                where = ", ".join(cats[c] for c in sorted(users.get(t, ())) if c in cats) if kind == "mod" else ""
                fix = ms[0]["fix"]
                body.append([(L.num(t, 0), t), esc(self.items.mod_label(L, ms[0])), steps,
                             esc(where) if kind == "mod" else (L.money(fix[0]) + " / " + L.money(fix[1], True) if fix[0] else "—"),
                             L.t("rolls", "випадає") if any(m["drop_weight"] for m in ms) else L.t("built in only", "лише вбудований")])
            out.append(f"<h2>{title}</h2>" + table(L, ["#" + L.t("Kind", "Вид"), L.t("Bonus", "Бонус"), L.t("Steps (chance)", "Кроки (шанс)"),
                                                       L.t("On", "На") if kind == "mod" else L.t("Fix", "Ремонт"), ""], body))
        slots = rules["defect_slots"]
        out.append(f"<h2>{L.t('When defects come', 'Коли з’являються дефекти')}</h2><ul>"
                   + "".join(f"<li>{L.t('Defect', 'Дефект')} {k + 1}: {L.t('below', 'нижче')} {L.pct(s['condition'][0], 0)} "
                             f"{L.t('condition', 'стану')}, {L.pct(s['chance'][0], 0)} → {L.pct(s['chance'][1], 0)} {L.t('a match as it wears to 0', 'за матч, поки зношується до 0')}</li>"
                             for k, s in enumerate(slots)) + "</ul>")
        return "".join(out)

    # ----- shop, drops, new accounts: srp's numbers
    def economy(self):
        self.site.add("shop", ("Shop", "Магазин"), [], self.shop_page, section="economy")
        self.site.add("drops", ("Drops", "Трофеї"), [], self.drops_page, section="economy")
        self.site.add("new-account", ("New account", "Новий обліковий запис"), [], self.new_account_page, section="economy")

    def shop_page(self, L):
        g, rules = self.game, self.game.rules
        out = [f"<p>{L.t('Silver buys gear by faction rank; gold buys premium items, bundles and premium time.', 'Срібло купує спорядження за рангом фракції; золото — преміум-речі, набори й преміум-час.')} "
               f"{L.t('Exchange', 'Обмін')}: 1 {L.t('gold', 'золото')} = {L.money(rules['exchange_rate'])}. {L.a('exchange', L.t('Exchange', 'Обмін'))}</p>",
               f"<ul><li>{L.t('Selling gives a quarter of the price, times the condition.', 'Продаж дає чверть ціни, помножену на стан.')}</li>"
               f"<li>{L.t('Ammo is sold by the pack.', 'Набої продаються пачками.')}</li></ul>"]
        body = []
        for b in rules["bundles"]:
            parts = []
            for name, count in b["items"]:
                item = g.by_name.get(name)
                parts.append((self.items.link(L, item) if item else esc(name)) + (f" ×{L.num(count, 0)}" if count > 1 else ""))
            body.append([", ".join(parts), (L.money(b["gold"], True), b["gold"])])
        out.append(f"<h2>{L.t('Bundles', 'Набори')}</h2>" + table(L, [L.t("Holds", "Містить"), "#" + L.t("Price", "Ціна")], body))
        body = [[(L.num(p["days"], 0) + L.t(" days", " дн."), p["days"]), (L.money(p["gold"], True), p["gold"])] for p in rules["premium_time"]]
        out.append(f"<h2>{L.t('Premium time', 'Преміум-час')}</h2>" + table(L, ["#" + L.t("Time", "Час"), "#" + L.t("Price", "Ціна")], body))
        gold = sorted((i for i in g.items.values() if any(s["gold"] for s in i.sellers)), key=lambda i: (i.category, i.name))
        body = [[self.items.ic(i), self.items.link(L, i), (self.items.price(L, i), self.items.price_key(i))] for i in gold]
        out.append(f"<h2>{L.t('For gold', 'За золото')}</h2>" + table(L, ["", L.t("Item", "Річ"), "#" + L.t("Price", "Ціна")], body))
        return "".join(out)

    def drops_page(self, L):
        g, d = self.game, self.game.rules["drops"]
        out = [f"<ul><li>{L.t('After a match', 'Після матчу')}: {L.pct(d['chance'], 0)} {L.t('chance of an item', 'шанс на річ')}.</li>"
               f"<li>+{L.pct(d['per_battery'], 1)} {L.t('for each battery your team holds at the end.', 'за кожну батарею вашої команди в кінці.')}</li>"
               f"<li>+{L.pct(d['premium'], 0)} {L.t('with premium.', 'з преміумом.')}</li>"
               f"<li>{L.t('A found weapon or piece of gear comes at', 'Знайдена зброя чи спорядження — у стані')} {L.num(d['condition'], 0)} %"
               f"{L.t(', with a defect.', ', з дефектом.')}</li>"
               + (f"<li>{L.a('modifications', L.t('Modifications', 'Модифікацій'))} {L.t('on a found weapon or piece of gear', 'на знайденій зброї чи спорядженні')}: "
                  + ", ".join(f"{n}: {L.pct(share, 0)}" for n, share in enumerate(d.get("rarity", []))) + ".</li>" if d.get("rarity") else "")
               + f"<li>{L.t('Scopes never drop.', 'Приціли не випадають.')}</li></ul>"]
        dropped = sorted((i for i in g.items.values() if i.droppable), key=lambda i: (i.category, i.level, i.name))
        groups = defaultdict(list)
        names = dict(WEAPON_CLASSES + ARMOUR_SLOTS)
        for i in dropped:
            groups[names.get(i.category)].append(i)
        for key, items in groups.items():
            title = L.n(g.texts(key)) if key else L.t("Other", "Інше")
            out.append(f"<h3>{esc(title)}</h3><p>{', '.join(self.items.link(L, i) for i in items)}</p>")
        return "".join(out)

    def new_account_page(self, L):
        g, rules = self.game, self.game.rules
        s, st = rules["signup"], rules["starter"]
        ammo = ", ".join(f"{self.items.link(L, g.by_name[n])} ×{L.num(c, 0)}" for n, c in s["ammo"] if n in g.by_name)
        worn = ", ".join(self.items.link(L, g.by_name[n]) for n in st["worn"] if n in g.by_name)
        stash = ", ".join(self.items.link(L, g.by_name[n]) for n in st["stash"] if n in g.by_name)
        faction = g.factions.get(s["faction"], {}).get("name")
        return (f"<ul><li>{L.money(s['silver'])}, {L.money(s['gold'], True)}.</li><li>{ammo}.</li>"
                f"<li>{L.t('Worn', 'Вдягнено')}: {worn}.</li><li>{L.t('In the stash', 'На складі')}: {stash}.</li>"
                f"<li>{L.t('Quest faction', 'Фракція завдань')}: {esc(L.n(faction))}.</li>"
                f"<li>{L.num(rules['kits'], 0)} {L.t('kits, all usable.', 'комплекти, усі доступні.')}</li>"
                f"<li>{L.t('No tutorial.', 'Без навчання.')}</li></ul>")
