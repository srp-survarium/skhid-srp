"""The item pages: weapons, ammo, equipment, scopes, supplies, artefacts and
cosmetics, and their category pages. Every number is the game's (the item's
config, the dictionary) or srp's (sellers, prices, what a sale gives, what an
item can roll: `rules.json`)."""
from collections import defaultdict

from web import esc, infobox, table
from model import (AMMO_CLASSES, ARMOUR_SLOTS, CAMO, DECAL, HIT_TYPES, SCOPE_CATEGORY, WEAPON_CLASSES, rows)

# What the calculator's page calls the affects a drug clears (ui.js AFFECTS).
AFFECTS = {0: ("death", "смерть"), 1: ("bleeding", "кровотеча"), 2: ("concussion", "контузія"),
           3: ("hand injury", "травма руки"), 4: ("broken leg", "зламана нога"), 6: ("intoxication", "отруєння")}
POOLS = {"body": ("Body", "Тіло"), "head": ("Head", "Голова"), "pain": ("Pain", "Біль"),
         "right_leg": ("Right leg", "Права нога"), "left_leg": ("Left leg", "Ліва нога"),
         "right_hand": ("Right hand", "Права кисть"), "left_hand": ("Left hand", "Ліва кисть"),
         "infection": ("Toxins", "Токсини")}
HIT_NAMES = {"ranged": ("Bullets", "Кулі"), "blunt": ("Blunt", "Удар"), "slash": ("Slash", "Поріз"),
             "explosive": ("Explosion", "Вибух"), "fire": ("Fire", "Вогонь"), "shock": ("Electric", "Електрика"),
             "toxic": ("Toxic", "Токсичне"), "radiation": ("Radiation", "Радіація")}
PARTS = {"front": ("Chest", "Груди"), "back": ("Back", "Спина"), "brain": ("Head", "Голова"), "face": ("Face", "Обличчя"),
         "right_forearm": ("Right arm", "Права рука"), "left_forearm": ("Left arm", "Ліва рука"),
         "right_arm": ("Right hand", "Права кисть"), "left_arm": ("Left hand", "Ліва кисть"),
         "right_hip": ("Right leg", "Права нога"), "left_hip": ("Left leg", "Ліва нога"),
         "right_foot": ("Right foot", "Права стопа"), "left_foot": ("Left foot", "Ліва стопа")}
FIRE_MODES = {-1: ("auto", "авто"), 1: ("single", "одиночний"), 2: ("2-round burst", "черга по 2"),
              3: ("3-round burst", "черга по 3")}


def pair(L, en_uk):
    return L.t(*en_uk)


class Items:
    def __init__(self, site, game):
        self.site, self.game = site, game

    # ----- shared pieces
    def icon(self, item):
        return self.site.icon(item.icon) if item.icon else None

    def ic(self, item):
        url = self.icon(item)
        return f'<img class="ic" src="{url}" alt="" loading="lazy">' if url else ""

    def link(self, L, item):
        if item.slug in self.site.pages:
            return L.a(item.slug, item.title)
        return esc(L.n(item.title))

    def seller(self, L, s):
        if s["gold"]:
            return L.a("shop", L.t("for gold", "за золото"))
        name = self.game.seller_name(s["seller"])
        if name is None:
            return L.t("any trader", "будь-який торговець")
        levels = self.game.factions[s["seller"]]["levels"]
        rank = levels[s["level"]]["name"] if 0 < s["level"] < len(levels) else None
        return (f'{L.a("factions", name)}, {L.t("rank", "ранг")} {s["level"]}'
                + (f" <small>({esc(L.n(rank))})</small>" if rank else ""))

    def price(self, L, item):
        if not item.sellers:
            return None
        s = item.sellers[0]
        unit = f" <small>/ {item.unit} {L.t('pcs', 'шт.')}</small>" if item.stack and item.unit > 1 else ""
        return L.money(s["cost"], s["gold"]) + unit

    def sellers_cell(self, L, item):
        """Who sells it, for an index table; else how else it is had."""
        if item.sellers:
            s = item.sellers[0]
            return "; ".join(self.seller(L, s) for s in item.sellers), 10000 * s["gold"] + 100 * s["seller"] + s["level"]
        if item.bundles:
            return L.a("shop", L.t("bundle", "набір")), 20000
        if item.droppable:
            return L.a("drops", L.t("drops only", "лише трофей")), 30000
        return L.a("new-account", L.t("new accounts", "нові облікові записи")), 40000

    def price_key(self, item):
        return item.sellers[0]["cost"] * (100 if item.sellers[0]["gold"] else 1) if item.sellers else -1

    def where(self, L, item):
        """Where a player gets it: the shop, drops, bundles, the start."""
        out = []
        for s in item.sellers:
            out.append(f"<li>{L.t('Shop', 'Магазин')}: {self.seller(L, s)} · {L.money(s['cost'], s['gold'])}</li>")
        for bundle, count in item.bundles:
            out.append(f"<li>{L.t('Bundle', 'Набір')} {L.a('shop', L.t('for', 'за') + ' ' + L.money(bundle['gold'], True))}"
                       + (f" ×{L.num(count, 0)}" if count > 1 else "") + "</li>")
        if item.droppable:
            out.append(f"<li>{L.a('drops', L.t('Drops after a match', 'Випадає після матчу'))}</li>")
        if item.name in self.game.starter:
            out.append(f"<li>{L.a('new-account', L.t('Every new account has it', 'Є в кожного нового облікового запису'))}</li>")
        if not out:
            out.append(f"<li class=muted>{L.t('Not sold and not dropped on Skhid.', 'На Сході не продається й не випадає.')}</li>")
        return f"<h2>{L.t('Where to get', 'Де взяти')}</h2><ul>{''.join(out)}</ul>"

    def common_rows(self, L, item):
        return [
            (L.t("Level", "Рівень"), L.num(item.level, 0)),
            (L.t("Price", "Ціна"), self.price(L, item)),
            (L.t("Sells for", "Продаж"), L.money(item.sell, item.premium and False) if item.sell else None),
            (L.t("Full repair", "Повний ремонт"), L.money(item.repair) if item.repair else None),
        ]

    def card(self, L, item):
        """The lobby card's lines, as the game prints them."""
        out = []
        for prop_id, value in item.props:
            ui = self.game.ui_props.get(prop_id)
            if not ui or not L.n(ui["label"]):
                continue
            post = L.n(ui["postfix"]) if ui["postfix"] else ""
            out.append((esc(L.n(ui["label"])), esc(value) + (f" {esc(post)}" if post else "")))
        return out

    def mod_value(self, L, m):
        """A modification's step as the lobby's card shows it."""
        if not m["props"]:
            path, kind, value = m["modifiers"][0]
            return L.pct(value, sign=True) if kind == 2 else ("+" if value > 0 else "") + L.num(value)
        prop_id, value = m["props"][0]
        ui = self.game.ui_props.get(prop_id, {})
        if ui.get("mod_type") == 1:
            post = L.n(ui["postfix"]) if ui.get("postfix") else ""
            return ("+" if value > 0 else "") + L.num(value) + (f" {post}" if post else "")
        return L.pct(value, sign=True)

    def param_bonus(self, L, name, value):
        """An item's own `<name>_modifier` as the card would print it."""
        ui = self.game.ui_props.get(self.game.param_props.get(name))
        label = L.n(ui["label"]) if ui and L.n(ui["label"]) else name.removesuffix("_modifier").replace("_", " ")
        if not ui:  # no card line names it: the value as the data has it
            return esc(label), ("+" if value > 0 else "") + L.num(value, 3)
        if ui and ui.get("mod_type") == 1:
            post = L.n(ui["postfix"]) if ui.get("postfix") else ""
            return esc(label), ("+" if value > 0 else "") + L.num(value) + (f"\u202f{post}" if post else "")
        return esc(label), L.pct(value, sign=True)

    def mod_label(self, L, m):
        labels = []
        for prop_id, _ in m["props"]:
            ui = self.game.ui_props.get(prop_id)
            if ui and L.n(ui["label"]):
                labels.append(L.n(ui["label"]))
        return ", ".join(labels) or m["modifiers"][0][0].rsplit("/", 1)[-1]

    def mods_section(self, L, item):
        """What the item can roll: each bonus, its steps and how likely each
        is (srp's roll: by drop weight among the bonuses the item takes)."""
        out = []
        if item.defaults:
            lines = [f"<li>{esc(self.mod_label(L, self.game.mod(d)))} {self.mod_value(L, self.game.mod(d))}</li>"
                     for d in item.defaults if self.game.mod(d)]
            if lines:
                out.append(f"<h3>{L.t('Built in', 'Вбудовані')}</h3><ul>{''.join(lines)}</ul>")
        for title, pool in ((L.t("Bonuses it can roll", "Бонуси, що можуть випасти"), item.mods),
                            (L.t("Defects it can get", "Дефекти, які можуть з'явитися"), item.defects)):
            if not pool:
                continue
            total = sum(w for _, w in pool) or 1
            by_type = defaultdict(list)
            for mid, w in pool:
                m = self.game.mod(mid)
                if m:
                    by_type[m["type"]].append((m, w))
            body = []
            for t, entries in sorted(by_type.items(), key=lambda kv: -sum(w for _, w in kv[1])):
                type_total = sum(w for _, w in entries)
                steps = " · ".join(f"{self.mod_value(L, m)} <small>({L.pct(w / type_total, 0)})</small>" for m, w in entries)
                body.append([esc(self.mod_label(L, entries[0][0])), steps, (L.pct(type_total / total, 1), type_total / total)])
            first = pool is item.mods
            out.append(f"<h3>{title}</h3>" + table(L, [L.t("Bonus", "Бонус") if first else L.t("Defect", "Дефект"),
                                                       L.t("Steps (chance of each)", "Кроки (шанс кожного)"),
                                                       "#" + (L.t("This bonus first", "Цей бонус першим") if first else L.t("This defect", "Цей дефект"))], body))
        if not out:
            return ""
        return (f"<h2>{L.a('modifications', L.t('Modifications', 'Модифікації'))}</h2>" + "".join(out)
                + f"<p class=muted>{L.t('Each bonus once per item. The first column of chances is within a bonus.', 'Кожен бонус — один раз на предмет. Перші шанси — усередині бонуса.')}</p>")

    def describe(self, L, item):
        return f'<p class="lead">{esc(L.n(item.description))}</p>' if item.description and L.n(item.description) else ""

    # ----- weapons
    def weapon_classes(self):
        return [(cat, key, sorted((i for i in self.game.of_categories({cat}) if self.game.obtainable(i)),
                                  key=lambda i: (i.level, i.premium, L_key(i))))
                for cat, key in WEAPON_CLASSES]

    def weapons(self):
        for cat, key, items in self.weapon_classes():
            for w in items:
                self.site.add(w.slug, w.title, [("weapons", ("Weapons", "Зброя"))], lambda L, w=w: self.weapon(L, w),
                              section="weapons", kind=self.game.texts(key))
        self.site.add("weapons", ("Weapons", "Зброя"), [], self.weapons_index, section="weapons")

    def weapons_index(self, L):
        out = [f"<p>{L.t('Sorted by level. Click a column to sort.', 'За рівнем. Клацніть стовпчик, щоб упорядкувати.')}</p>"]
        out.append('<input class="find" type="search" data-find="weapons" placeholder="' + L.t("Find", "Пошук") + '">')
        for cat, key, items in self.weapon_classes():
            if not items:
                continue
            out.append(f"<h2>{esc(L.n(self.game.texts(key)))}</h2>")
            body = []
            for w in items:
                p = w.params
                body.append([self.ic(w), self.link(L, w), (L.num(w.level, 0), w.level),
                             (L.hp(p["bullet_damage"]), p["bullet_damage"]), (L.hp(p["bullet_pierce"]), p["bullet_pierce"]),
                             (L.num(p.get("rounds_per_minute", 0), 0), p.get("rounds_per_minute", 0)),
                             (L.num(p.get("magazine_capacity", 0), 0), p.get("magazine_capacity", 0)),
                             (L.metres(p["max_damage_distance"]), p["max_damage_distance"]),
                             (L.kg(w.weight), w.weight),
                             self.sellers_cell(L, w),
                             (self.price(L, w) or "—", self.price_key(w))])
            out.append(table(L, ["", L.t("Weapon", "Зброя"), "#" + L.t("Level", "Рівень"), "#" + L.t("Damage", "Ушкодження"),
                                 "#" + L.t("Penetration", "Бронебійність"), "#" + L.t("Rate", "Темп"), "#" + L.t("Magazine", "Магазин"),
                                 "#" + L.t("Full damage to", "Повне до"), "#" + L.t("Weight", "Вага"), L.t("Seller", "Продавець"),
                                 "#" + L.t("Price", "Ціна")], body, cls="weapons"))
        return "".join(out)

    def weapon(self, L, w):
        p, g = w.params, self.game
        cls = next(g.texts(k) for c, k in WEAPON_CLASSES if c == w.category)
        modes = ", ".join(pair(L, FIRE_MODES.get(m, (str(m), str(m)))) for m in p.get("fire_queue_types") or [])
        rec = w.cfg.get("recoil") or {}
        box = infobox(L, L.n(w.title), self.icon(w), [
            (L.t("Class", "Клас"), L.a("weapons", cls)),
            *self.common_rows(L, w),
            (L.t("Premium", "Преміум"), L.t("yes", "так") if w.premium else None),
        ], [(L.t("Card", "Картка"), self.card(L, w)),
            (L.t("Details", "Подробиці"), [
                (L.t("Full damage to", "Повне ушкодження до"), L.metres(p["max_damage_distance"])),
                (L.t("Lowest from", "Найменше від"), L.metres(p["min_damage_distance"])),
                (L.t("Lowest damage", "Найменше ушкодження"), L.pct(p["min_damage_amount"], 0)),
                (L.t("Fire modes", "Режими вогню"), modes),
                (L.t("Reload", "Перезарядка"), L.sec(p["reload_time"]) if p.get("reload_time") else None),
                (L.t("Draw / holster", "Дістати / сховати"), f"{L.sec(p['show_time'])} / {L.sec(p['hide_time'])}" if p.get("show_time") else None),
                (L.t("Stamina damage", "Удар по енергії"), L.num(p["stamina_damage"]) if p.get("stamina_damage") else None),
                (L.t("Recoil up", "Віддача вгору"), f"{L.num(rec['min_top_recoil'])}–{L.num(rec['max_top_recoil'])}" if "min_top_recoil" in rec else None),
                (L.t("Recoil sideways", "Віддача вбік"), f"{L.num(rec['min_left_recoil'])}–{L.num(rec['max_left_recoil'])} / {L.num(rec['min_right_recoil'])}–{L.num(rec['max_right_recoil'])}" if "min_left_recoil" in rec else None),
                (L.t("Two hands", "Дворучна"), L.t("yes", "так") if p.get("double_handed") else L.t("no", "ні")),
            ])])
        out = [box, self.describe(L, w)]
        ammo = g.ammo_of.get(w.dict_id, [])
        if ammo:
            body = []
            for a in ammo:
                b = g.ballistics(w, a)
                pellets = f" ×{b['pellets']}" if b["pellets"] > 1 else ""
                body.append([self.ic(a), self.link(L, a),
                             (f"{L.hp(b['d_max'])}{pellets} → {L.hp(b['d_min'])}{pellets}", b["d_max"]),
                             (f"{L.hp(b['p_max'])} → {L.hp(b['p_min'])}", b["p_max"]),
                             (f"{L.metres(b['near'])} → {L.metres(b['far'])}", b["near"]),
                             (self.price(L, a) or "—", self.price_key(a))])
            out.append(f"<h2>{L.t('Ammo', 'Набої')}</h2>"
                       + table(L, ["", L.t("Ammo", "Набій"), "#" + L.t("Damage near → far", "Ушкодження близько → далеко"),
                                   "#" + L.t("Penetration", "Бронебійність"), "#" + L.t("Falls off between", "Спадає між"),
                                   "#" + L.t("Price", "Ціна")], body)
                       + f"<p class=muted>{L.a('damage', L.t('How damage works', 'Як рахується ушкодження'))} · "
                       + f'<a href="/damage?lang={L.code}">{L.t("Calculator", "Калькулятор")}</a></p>')
        scopes = g.scopes_of.get(w.dict_id, [])
        if scopes:
            out.append(f"<h2>{L.a('scopes', L.t('Scopes', 'Приціли'))}</h2><ul>"
                       + "".join(f"<li>{self.link(L, s)} · {self.price(L, s) or '—'}</li>" for s in scopes) + "</ul>")
        out.append(self.mods_section(L, w))
        out.append(self.where(L, w))
        return "".join(out)

    # ----- ammo
    def ammo_classes(self):
        return [(cat, key, sorted((i for i in self.game.of_categories({cat}) if self.game.obtainable(i)),
                                  key=lambda i: (i.level, i.base_cost, i.name)))
                for cat, key in AMMO_CLASSES]

    def ammo(self):
        for cat, key, items in self.ammo_classes():
            for a in items:
                self.site.add(a.slug, a.title, [("ammo", ("Ammo", "Набої"))], lambda L, a=a: self.ammo_page(L, a),
                              section="ammo", kind=self.game.texts(key))
        self.site.add("ammo", ("Ammo", "Набої"), [], self.ammo_index, section="ammo")

    def ammo_index(self, L):
        out = [f"<p>{L.t('Damage and penetration are of the weapon’s own; distances scale the weapon’s.', 'Ушкодження й бронебійність — частка від зброї; дистанції множать дистанції зброї.')}</p>"]
        for cat, key, items in self.ammo_classes():
            if not items:
                continue
            out.append(f"<h2>{esc(L.n(self.game.texts(key)))}</h2>")
            body = []
            for a in items:
                d = a.data
                body.append([self.ic(a), self.link(L, a),
                             (L.pct(d.get("k_damage", 1), 0), d.get("k_damage", 1)), (L.pct(d.get("k_arp", 1), 0), d.get("k_arp", 1)),
                             (L.pct(d.get("k_max_damage_distance", 1), 0), d.get("k_max_damage_distance", 1)),
                             (L.num(a.params.get("clip_size", 0), 0), a.params.get("clip_size", 0)),
                             ", ".join(self.link(L, w) for w in self.game.weapons_of.get(a.dict_id, [])) or "—",
                             (self.price(L, a) or "—", self.price_key(a))])
            out.append(table(L, ["", L.t("Ammo", "Набій"), "#" + L.t("Damage", "Ушкодження"), "#" + L.t("Penetration", "Бронебійність"),
                                 "#" + L.t("Distance", "Дистанція"), "#" + L.t("Pack", "Пачка"), L.t("Weapons", "Зброя"),
                                 "#" + L.t("Price", "Ціна")], body))
        return "".join(out)

    def ammo_page(self, L, a):
        d, g = a.data, self.game
        cls = next(g.texts(k) for c, k in AMMO_CLASSES if c == a.category)
        box = infobox(L, L.n(a.title), self.icon(a), [
            (L.t("Class", "Клас"), L.a("ammo", cls)),
            *self.common_rows(L, a),
            (L.t("Pack", "Пачка"), L.num(a.params.get("clip_size", 0), 0)),
        ], [(L.t("Card", "Картка"), self.card(L, a)),
            (L.t("Of the weapon’s", "Від зброї"), [
                (L.t("Damage", "Ушкодження"), L.pct(d.get("k_damage", 1), 0)),
                (L.t("Penetration", "Бронебійність"), L.pct(d.get("k_arp", 1), 0)),
                (L.t("Full-damage distance", "Дистанція повного"), L.pct(d.get("k_max_damage_distance", 1), 0)),
                (L.t("Lowest-damage distance", "Дистанція найменшого"), L.pct(d.get("k_min_damage_distance", 1), 0)),
                (L.t("Lowest damage", "Найменше ушкодження"), L.pct(d.get("k_min_damage_amount", 1), 0)),
                (L.t("Stamina damage", "Удар по енергії"), L.pct(d.get("k_stamina_damage", 1), 0) if "k_stamina_damage" in d else None),
                (L.t("Pellets", "Шротин"), L.num(d["buck_shot"], 0) if d.get("buck_shot", 0) > 1 else None),
                (L.t("Muzzle speed", "Початкова швидкість"), L.num(d["muzzle_speed"], 0) + L.t(" m/s", " м/с") if d.get("muzzle_speed") else None),
            ])])
        out = [box, self.describe(L, a)]
        weapons = g.weapons_of.get(a.dict_id, [])
        if weapons:
            body = []
            for w in weapons:
                b = g.ballistics(w, a)
                pellets = f" ×{b['pellets']}" if b["pellets"] > 1 else ""
                body.append([self.ic(w), self.link(L, w), (f"{L.hp(b['d_max'])}{pellets} → {L.hp(b['d_min'])}{pellets}", b["d_max"]),
                             (f"{L.hp(b['p_max'])} → {L.hp(b['p_min'])}", b["p_max"]),
                             (f"{L.metres(b['near'])} → {L.metres(b['far'])}", b["near"])])
            out.append(f"<h2>{L.t('Weapons', 'Зброя')}</h2>" + table(L, ["", L.t("Weapon", "Зброя"),
                       "#" + L.t("Damage near → far", "Ушкодження близько → далеко"), "#" + L.t("Penetration", "Бронебійність"),
                       "#" + L.t("Falls off between", "Спадає між")], body))
        out.append(self.where(L, a))
        return "".join(out)

    # ----- equipment
    def armour_slots(self):
        return [(cat, key, sorted((i for i in self.game.of_categories({cat}) if self.game.obtainable(i)),
                                  key=lambda i: (i.level, i.premium, i.name)))
                for cat, key in ARMOUR_SLOTS]

    def armour(self):
        for cat, key, items in self.armour_slots():
            for a in items:
                self.site.add(a.slug, a.title, [("equipment", ("Equipment", "Спорядження"))], lambda L, a=a: self.armour_page(L, a),
                              section="equipment", kind=self.game.texts(key))
        self.site.add("equipment", ("Equipment", "Спорядження"), [], self.armour_index, section="equipment")

    def protection(self, item, hit="ranged"):
        """The item's armour against `hit` on the parts it covers: the most."""
        values = [row.get("hit_types", {}).get(hit, {}).get("armor", 0) for row in (item.cfg.get("hit_params") or {}).values()]
        return max(values, default=0)

    def armour_index(self, L):
        out = [f"<p>{L.t('Armour against bullets and isolation (anomalies) are the most the piece gives on any part it covers.', 'Броня від куль та ізоляція (аномалії) — найбільше, що річ дає на будь-якій частині, яку вкриває.')}</p>",
               '<input class="find" type="search" data-find="gear" placeholder="' + L.t("Find", "Пошук") + '">']
        for cat, key, items in self.armour_slots():
            if not items:
                continue
            out.append(f"<h2>{esc(L.n(self.game.texts(key)))}</h2>")
            body = []
            for a in items:
                bullets, iso = self.protection(a), max(self.protection(a, h) for h in ("shock", "toxic", "radiation", "fire"))
                body.append([self.ic(a), self.link(L, a), (L.num(a.level, 0), a.level), (L.hp(bullets), bullets), (L.hp(iso), iso),
                             (L.kg(a.weight), a.weight),
                             self.sellers_cell(L, a),
                             (self.price(L, a) or "—", self.price_key(a))])
            out.append(table(L, ["", L.t("Item", "Річ"), "#" + L.t("Level", "Рівень"), "#" + L.t("Armour", "Броня"),
                                 "#" + L.t("Isolation", "Ізоляція"), "#" + L.t("Weight", "Вага"), L.t("Seller", "Продавець"),
                                 "#" + L.t("Price", "Ціна")], body, cls="gear"))
        return "".join(out)

    def armour_page(self, L, a):
        g = self.game
        slot = next(g.texts(k) for c, k in ARMOUR_SLOTS if c == a.category)
        bonus = [self.param_bonus(L, k, v) for k, v in a.params.items() if k.endswith("_modifier") and v]
        box = infobox(L, L.n(a.title), self.icon(a), [
            (L.t("Slot", "Слот"), L.a("equipment", slot)),
            *self.common_rows(L, a),
            (L.t("Weight", "Вага"), L.kg(a.weight)),
            (L.t("Oxygen", "Кисень"), L.sec(a.data["amount_time_sec"]) if a.data.get("amount_time_sec") else None),
            (L.t("Premium", "Преміум"), L.t("yes", "так") if a.premium else None),
        ], [(L.t("Card", "Картка"), self.card(L, a))])
        out = [box, self.describe(L, a)]
        hp = a.cfg.get("hit_params") or {}
        if hp:
            body = []
            for part, row in hp.items():
                types = row.get("hit_types") or {}
                body.append([esc(pair(L, PARTS.get(part, (part, part))))] +
                            [(L.hp(types.get(h, {}).get("armor", 0)), types.get(h, {}).get("armor", 0)) for h in HIT_TYPES])
            out.append(f"<h2>{L.t('Protection', 'Захист')}</h2>"
                       + table(L, [L.t("Part", "Частина")] + ["#" + pair(L, HIT_NAMES[h]) for h in HIT_TYPES], body, sort=False)
                       + f"<p class=muted>{L.a('damage', L.t('How armour works', 'Як працює броня'))}</p>")
        takes = (a.cfg.get("data") or {}).get("additional_model_slots") or []
        if takes:
            out.append(f"<p>{L.t('Also takes the slot of', 'Також займає слот')}: {esc(', '.join(takes))}</p>")
        if bonus:
            out.append(f"<h2>{L.t('Bonuses', 'Бонуси')}</h2><ul>" + "".join(f"<li>{k}: {v}</li>" for k, v in bonus) + "</ul>")
        out.append(self.mods_section(L, a))
        out.append(self.where(L, a))
        return "".join(out)

    # ----- scopes
    def scopes(self):
        g = self.game
        items = sorted((i for i in g.of_categories({SCOPE_CATEGORY}) if g.weapons_of_scope.get(i.dict_id)),
                       key=lambda i: (i.base_cost, i.name))
        for s in items:
            self.site.add(s.slug, s.title, [("scopes", ("Scopes", "Приціли"))], lambda L, s=s: self.scope_page(L, s), section="scopes")
        self.site.add("scopes", ("Scopes", "Приціли"), [], lambda L: self.scopes_index(L, items), section="scopes")

    def scopes_index(self, L, items):
        g = self.game
        out = [f"<p>{L.t('Bought once, fitted on any weapon it fits.', 'Купується один раз, ставиться на будь-яку зброю, якій пасує.')} "
               f"{L.num(len(g.rules['scopes']), 0)} {L.t('pairs', 'пар')}.</p>"]
        body = [[self.ic(s), self.link(L, s), (self.price(L, s) or "—", self.price_key(s)),
                 ", ".join(self.link(L, w) for w in g.weapons_of_scope[s.dict_id])] for s in items]
        out.append(table(L, ["", L.t("Scope", "Приціл"), "#" + L.t("Price", "Ціна"), L.t("Weapons", "Зброя")], body))
        return "".join(out)

    def scope_page(self, L, s):
        g = self.game
        box = infobox(L, L.n(s.title), self.icon(s), [*self.common_rows(L, s)], [(L.t("Card", "Картка"), self.card(L, s))])
        weapons = g.weapons_of_scope[s.dict_id]
        return "".join([box, self.describe(L, s), f"<h2>{L.t('Fits', 'Пасує')}</h2><ul class=cols>",
                        "".join(f"<li>{self.link(L, w)}</li>" for w in weapons), "</ul>", self.where(L, s)])

    # ----- supplies: medicine, boosters, grenades, traps
    def supplies(self):
        g = self.game
        groups = [("medicine", ("Medicine", "Медикаменти"), [i for i in g.of_categories({10}) if i.cfg_name.startswith("gameplay/items/drugs/")]),
                  ("boosters", ("Boosters", "Бустери"), [i for i in g.of_categories({10}) if i.cfg_name.startswith("gameplay/items/boosters/")]),
                  ("grenades", ("Grenades", "Гранати"), g.of_categories({24})),
                  ("traps", ("Traps", "Пастки"), g.of_categories({11}))]
        self.supply_groups = [(k, t, sorted((i for i in items if g.obtainable(i)), key=lambda i: (i.level, i.name))) for k, t, items in groups]
        for key, title, items in self.supply_groups:
            for i in items:
                self.site.add(i.slug, i.title, [("supplies", ("Supplies", "Витратники"))], lambda L, i=i: self.supply_page(L, i), section="supplies", kind=title)
        self.site.add("supplies", ("Supplies", "Витратники"), [], self.supplies_index, section="supplies")

    def supplies_index(self, L):
        out = []
        for key, title, items in self.supply_groups:
            if not items:
                continue
            out.append(f"<h2>{esc(L.n(title))}</h2>")
            body = [[self.ic(i), self.link(L, i), esc(self.effect_line(L, i)), (self.price(L, i) or "—", self.price_key(i))] for i in items]
            out.append(table(L, ["", L.t("Item", "Річ"), L.t("Does", "Дія"), "#" + L.t("Price", "Ціна")], body))
        return "".join(out)

    def effect_line(self, L, i):
        d, p = i.data, i.params
        if i.cfg_name.startswith("gameplay/items/drugs/"):
            parts = []
            heal = [x for x in rows(d.get("influences")) if x.get("amount")]
            if heal:
                parts.append(L.t("heals ", "лікує: ") + ", ".join(f"{pair(L, POOLS.get(x['body_part'], (x['body_part'],) * 2)).lower()} +{L.hp(x['amount'])}" for x in heal))
            for x in rows(d.get("damage_protection")):
                pool = pair(L, POOLS.get(x["body_part"], (x["body_part"],) * 2)).lower()
                hit = pair(L, HIT_NAMES.get(x["hit_type"], (x["hit_type"],) * 2)).lower()
                above = (L.t(" above ", " понад ") + L.hp(x["threshold"])) if x.get("threshold") else ""
                parts.append(L.t(f"{pool} from {hit}{above} ×{L.num(x['hit_coeff'])}", f"{pool} від: {hit}{above} ×{L.num(x['hit_coeff'])}"))
            fixes = [pair(L, AFFECTS.get(x["affect"], (str(x["affect"]),) * 2)) for x in rows(d.get("remove_affects"))]
            if fixes:
                parts.append(L.t("cures ", "усуває: ") + ", ".join(dict.fromkeys(fixes)))
            return "; ".join(parts)
        if i.defaults:
            return "; ".join(f"{self.mod_label(L, self.game.mod(m))} {self.mod_value(L, self.game.mod(m))}" for m in i.defaults if self.game.mod(m))
        if p.get("type") == "grenade":
            return L.t(f"{L.hp(p.get('splash_max_damage', 0))} damage within {L.metres(p.get('splash_radius', 0))}",
                       f"{L.hp(p.get('splash_max_damage', 0))} ушкодження в радіусі {L.metres(p.get('splash_radius', 0))}") if p.get("splash_max_damage") else ""
        if p.get("type") == "trap":
            return L.t(f"{L.hp(p.get('trap_damage', 0))} to each foot", f"{L.hp(p.get('trap_damage', 0))} кожній стопі")
        return ""

    def supply_page(self, L, i):
        d, p = i.data, i.params
        rows_ = [*self.common_rows(L, i), (L.t("Weight", "Вага"), L.kg(i.weight))]
        if i.cfg_name.startswith("gameplay/items/drugs/"):
            rows_ += [(L.t("Takes effect after", "Діє через"), L.sec(d.get("activation_delay_sec", 0))),
                      (L.t("Lasts", "Триває"), L.sec(d.get("activity_time_sec", 0)))]
        if p.get("type") == "grenade":
            rows_ += [(L.t("Damage at the centre", "Ушкодження в центрі"), L.hp(p.get("splash_max_damage", 0))),
                      (L.t("Damage at the edge", "Ушкодження на краю"), L.hp(p.get("splash_min_damage", 0))),
                      (L.t("Radius", "Радіус"), L.metres(p.get("splash_radius", 0))),
                      (L.t("Penetration", "Бронебійність"), L.hp(p.get("splash_pierce", 0))),
                      (L.t("Fuse", "Запал"), L.sec(d["ttl_ms"] / 1000) if d.get("ttl_ms") else None)]
        if p.get("type") == "trap":
            rows_ += [(L.t("Damage to each foot", "Ушкодження кожній стопі"), L.hp(p.get("trap_damage", 0))),
                      (L.t("Penetration", "Бронебійність"), L.hp(p.get("trap_pierce", 0))),
                      (L.t("Defuse time", "Знешкодження"), L.sec(d["defuse_time"]) if d.get("defuse_time") else None),
                      (L.t("Set within", "Ставиться до"), L.metres(d["max_deploy_distance"]) if d.get("max_deploy_distance") else None)]
        box = infobox(L, L.n(i.title), self.icon(i), rows_, [(L.t("Card", "Картка"), self.card(L, i))])
        out = [box, self.describe(L, i)]
        line = self.effect_line(L, i)
        if line:
            out.append(f"<h2>{L.t('What it does', 'Що робить')}</h2><p>{esc(line[0].upper() + line[1:])}.</p>")
        if i.cfg_name.startswith("gameplay/items/drugs/"):
            out.append(f"<p class=muted>{L.a('healing', L.t('Healing', 'Лікування'))} · {L.a('anomalies', L.t('Anomalies', 'Аномалії'))}</p>")
        out.append(self.where(L, i))
        return "".join(out)

    # ----- cosmetics: one list each
    def cosmetics(self):
        g = self.game
        for cat, slug_, title in ((CAMO, "camouflages", ("Camouflages", "Камуфляжі")), (DECAL, "decals", ("Decals", "Нашивки"))):
            items = sorted((i for i in g.of_categories({cat}) if g.obtainable(i)), key=lambda i: (i.base_cost, i.name))
            self.site.add(slug_, title, [], lambda L, items=items: self.cosmetics_page(L, items), section="cosmetics")

    def cosmetics_page(self, L, items):
        body = [[esc(L.n(i.title)), ("; ".join(self.seller(L, s) for s in i.sellers) or "—", i.sellers[0]["seller"] if i.sellers else 9),
                 (self.price(L, i) or "—", self.price_key(i)),
                 L.t("yes", "так") if i.droppable else "—"] for i in items]
        return (f"<p>{L.num(len(items), 0)} {L.t('in all.', 'усього.')}</p>"
                + table(L, [L.t("Name", "Назва"), L.t("Seller", "Продавець"), "#" + L.t("Price", "Ціна"), L.t("Drops", "Випадає")], body))


def L_key(item):
    return item.name
