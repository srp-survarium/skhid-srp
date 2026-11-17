#!/usr/bin/env python3
"""Build the wiki's weapons catalogue and simulators from the calculator's generated data.

No item values live here. The calculator build calls this after writing calc.js,
so both pages use exactly the same released configs, localizations and images.
"""
import argparse
import hashlib
import html
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from stats import sections  # noqa: E402
SITE = HERE.parents[1] / "servers" / "web-server"
CLASSES = {
    8: ("Assault rifle", "Штурмова гвинтівка"),
    16: ("Machine gun", "Кулемет"),
    15: ("Submachine gun", "Пістолет-кулемет"),
    14: ("Shotgun", "Рушниця"),
    13: ("Sniper rifle", "Снайперська гвинтівка"),
    17: ("Pistol", "Пістолет"),
}


def number(value):
    return f"{value:.3f}".rstrip("0").rstrip(".")


DOLL = HERE.parent / "damage-calc" / "assets" / "doll" / "doll_front.svg"


def doll():
    """The game's HUD body doll (hud.swf sprite 119, traced: the calculator's
    doll_front.svg), its zones as one shape the crosshair view draws as a
    target."""
    paths = re.findall(r'<path id="[^"]+" class="zone"[^>]*?transform="([^"]+)"[^>]*?d="([^"]+)"', DOLL.read_text())
    assert len(paths) == 11, f"{DOLL}: {len(paths)} zones"
    shape = "".join(f'<path transform="{t}" d="{d}"/>' for t, d in paths)
    return f'<svg class="doll-source" aria-hidden="true" focusable="false"><g id="doll-shape">{shape}</g></svg>'


# What each spread modifier a skill can carry does (spread.js, from the exe);
# `{v}` its value a rank. None of them changes the fully aimed spread.
SKILL_TIPS = {
    "dispersion_from_hip": ("Spread from the hip {v} per rank, sprinting and jumping included. Not while aiming.",
                            "Розкид від стегна {v} за ранг, зокрема в бігу й у стрибку. Не під час прицілювання."),
    "dispersion_movement_influence": ("Spread while walking {v} per rank, from the hip or aiming in. Not crouch-walking.",
                                      "Розкид під час ходьби {v} за ранг, від стегна й під час прицілювання. Не в ходьбі присівши."),
    "injury_penalty": ("Spread from broken arms {v} per rank.", "Розкид від зламаних рук {v} за ранг."),
    "recoil": ("Each shot's kick {v} per rank: up, sideways and the camera shake.",
               "Поштовх кожного пострілу {v} за ранг: угору, убік і тремтіння камери."),
}
# What a modification of each kind does: of the kit's player modifiers, or
# the weapon's kick ranges.
MOD_TIPS = {
    ("dispersion",): ("The stance's spread and each shot's. No effect sprinting, jumping or fully aimed.",
                      "Розкид від положення й від кожного пострілу. Не діє в бігу, у стрибку й при повному прицілюванні."),
    ("recoil",): ("Each shot's kick with any weapon: up, sideways and the camera shake.",
                  "Поштовх кожного пострілу з будь-якої зброї: угору, убік і тремтіння камери."),
    "kick": ("This weapon's kick ranges, up and sideways. Not the camera shake.",
             "Межі поштовху цієї зброї, угору й убік. Не тремтіння камери."),
}


def skill_tip(skill, tr):
    """A spread skill's help line: what each of its modifiers does a rank (its
    ranks the sums of equal levels), then that fully aimed it does nothing."""
    first, *rest = skill["ranks"][1:]
    for n, rank in enumerate(rest, 2):
        assert rank == {m: round(v * n, 6) for m, v in first.items()}, f'skill {skill["id"]}: ranks not equal steps'
    lines = []
    for modifier, value in first.items():
        en, uk = SKILL_TIPS[modifier]
        v = f'{"−" if value < 0 else "+"}{abs(value) * 100:g}%'
        lines.append(tr(en, uk).format(v=v))
    return " ".join(lines + [tr("No effect fully aimed.", "Не діє при повному прицілюванні.")])


def simulation(tr, sim, weapons):
    """The simulation's controls and plots (simulators.js RecoilSim finds its
    parts by class); `sim`: the calculator data's `simulators`. Its skills and
    armour modifications are `.sim-source` choosers, each option with the kit
    modifiers it adds (`data-effects`); the weapon's modifications
    `.sim-kick` ones, each option with its scale of the kick ranges
    (`data-kick`). Each with a help line."""
    esc = html.escape

    def control(key, name, tip, cls, options):
        return (f'<label for="{key}"><span class="sim-skill sim-tipped">{name}'
                f'<button type="button" class="sim-help" aria-label="{tr("What it does", "Що це дає")}" aria-describedby="{key}-tip">?</button>'
                f'<span class="sim-tip" role="tooltip" id="{key}-tip">{esc(tip)}</span></span>'
                f'<select id="{key}" class="{cls}" aria-labelledby="{key}-name">'
                + "".join(f'<option {attr}="{esc(json.dumps(value))}">{esc(text)}</option>' for text, attr, value in options)
                + '</select></label>')

    def skill(s):
        icon = SITE / "skills" / "icons" / f'{s["id"]}.webp'
        url = f'/skills/icons/{s["id"]}.webp?v={hashlib.sha256(icon.read_bytes()).hexdigest()[:12]}'
        key = f'sim-skill-{s["id"]}'
        name = f'<img src="{url}" alt="" width="28" height="28"><span id="{key}-name">{esc(tr(s["name"], s["name_uk"] or s["name"]))}</span>'
        return control(key, name, skill_tip(s, tr), "sim-source", [(str(n), "data-effects", e) for n, e in enumerate(s["ranks"])])

    def modification(mods):
        first = mods[0]
        kick = "kick" in first
        slots = ", ".join(tr(x["name"], x["name_uk"] or x["name"]) for x in {(x["name"], x["name_uk"]): x for m in mods for x in m["slots"]}.values())
        key = f'sim-mod-{first["type"]}'
        name = f'<span id="{key}-name">{esc(slots)}: {esc(tr(first["card"]["label"], first["card"]["label_uk"] or first["card"]["label"]))}</span>'
        tip = tr(*MOD_TIPS["kick" if kick else tuple(sorted(first["kit"]))])
        attr = "data-kick" if kick else "data-effects"
        options = [(tr("None", "Немає"), attr, 0 if kick else {})]
        options += [(tr(m["card"]["value"], m["card"]["value_uk"]), attr, m["kick"] if kick else m["kit"]) for m in mods]
        return control(key, name, tip, "sim-kick" if kick else "sim-source", options)

    # The modifications a Skhid item can have: srp rolls no defect
    # (crates/survarium/src/v026g/modifications.rs), one choice a kind.
    kinds = {}
    for m in sim["mods"]:
        if m["kind"] == "mod":
            assert "kit" in m or m["weapons"] == sorted(weapons), f'modification {m["id"]}: not every weapon takes it'
            kinds.setdefault(m["type"], []).append(m)
    for mods in kinds.values():
        mods.sort(key=lambda m: abs(m["kick"] if "kick" in m else sum(m["kit"].values())))
    aims = [("hip", "From the hip", "Від стегна"), ("aiming", "Aiming in", "Прицілювання"), ("aimed", "Fully aimed", "Повне прицілювання")]
    stances = [("idle", "Standing", "Стоячи"), ("walk", "Walking", "Ходьба"), ("crouch", "Crouching", "Присівши"),
               ("crouch_walk", "Crouch-walking", "Ходьба присівши"), ("run", "Sprinting", "Біг"), ("jump", "Jumping", "Стрибок")]
    parts = [tr(en, uk) for en, uk in [("Base", "Базовий"), ("Stance", "Положення"), ("A shot", "За постріл"), ("At most", "Найбільший")]]
    plots = "".join(
        f'<figure><figcaption>{tr(en, uk)} · {tr("game units", "одиниці гри")}</figcaption><svg class="sim-{axis}" viewBox="0 0 600 250" role="img" aria-label="{tr(en, uk)}"></svg></figure>'
        for axis, en, uk in [("vertical", "Vertical kick", "Вертикальний відбій"), ("horizontal", "Horizontal kick", "Бічний відбій")])
    return "".join([
        f'<p class="stat-note">{tr("Modelled kick, recovery, spread and camera shake (from the hip), with no mouse compensation. The plots are in the weapons’ kick units; the crosshair view is in degrees of view (upward exactly as the game does, sideways estimated), with each bullet’s hit and the spread circle.", "Модель поштовху, відновлення, розкиду й тремтіння камери (від стегна), без компенсації мишею. Графіки — в одиницях відбою зброї; приціл — у градусах огляду (вгору точно як у грі, убік приблизно), з влучанням кожної кулі та колом розкиду.")}</p>',
        f'<div class="sim-inputs"><label>{tr("Aim", "Приціл")}<select class="sim-aim">'
        + "".join(f'<option value="{v}">{tr(en, uk)}</option>' for v, en, uk in aims) + '</select></label>'
        + f'<label>{tr("Stance", "Положення")}<select class="sim-stance">'
        + "".join(f'<option value="{v}">{tr(en, uk)}</option>' for v, en, uk in stances) + '</select></label>'
        f'<label>{tr("Trigger", "Спуск")}<select class="sim-trigger"><option value="0">{tr("Hold", "Утримувати")}</option>'
        + "".join(f'<option value="{1 / n:g}">{tr(f"Click, {n} a second", f"Клацати, {n} на секунду")}</option>' for n in (4, 2, 1))
        + f'</select></label><label>{tr("Sample seed", "Зерно вибірки")}<input class="sim-seed" type="number" min="1" max="999999" value="1"></label></div>',
        f'<p class="stat-note">{tr("A full magazine; with two weapons, as many shots as the smaller magazine holds, each at its own fire rate. Random samples use the same seed for comparison; they are not recorded game shots.", "Повний магазин; для двох одиниць зброї стільки пострілів, скільки вміщує менший магазин, кожна у власному темпі. Для порівняння використано однакове зерно випадковості; це не запис пострілів із гри.")}</p>',
        '<div class="sim-inputs">' + "".join(skill(s) for s in sim["skills"]) + "".join(modification(m) for m in kinds.values())
        + f'<label>{tr("Broken arms", "Зламані руки")}<select class="sim-arms">' + "".join(f'<option>{n}</option>' for n in range(3)) + '</select></label></div>',
        f'<table class="sim-parts"><caption>{tr("Spread", "Розкид")} · {tr("degrees", "градуси")}</caption><thead><tr><td></td>'
        + "".join(f'<th scope="col">{p}</th>' for p in parts) + '</tr></thead><tbody></tbody></table>',
        f'<div class="sim-buttons"><button class="sim-play" type="button">{tr("Play", "Відтворити")}</button><button class="sim-last" type="button">{tr("Last shot", "Останній постріл")}</button><button class="sim-reset" type="button">{tr("Reset", "Скинути")}</button></div>',
        f'<label class="sim-timeline">{tr("Timeline", "Шкала часу")} <output class="sim-time-label">0 s</output><input class="sim-time" type="range" min="0" max="1" step="0.01" value="0"></label>',
        '<div class="sim-legend" aria-live="polite"></div>',
        f'<figure class="sim-sight"><figcaption>{tr("Crosshair", "Приціл")} · {tr("degrees of view", "градуси огляду")}</figcaption><div class="sim-sight-views"></div>'
        f'<label class="sim-target-pick">{tr("Target", "Ціль")}<select class="sim-target"><option value="">{tr("None", "Немає")}</option>'
        + "".join(f'<option value="{m}">{tr(f"Enemy at {m} m", f"Ворог на {m} м")}</option>' for m in (5, 10, 25))
        + '</select></label></figure>',
        f'<div class="sim-plots">{plots}</div>',
        f'<p class="sim-unavailable" hidden>{tr("Recoil parameters are unavailable for this selection.", "Параметри відбою для вибраної зброї недоступні.")}</p>',
    ])


class Page:
    """What both pages say of the calculator data in one language."""

    def __init__(self, data, code):
        self.data, self.code = data, code

    def tr(self, en, uk):
        return en if self.code == "en" else uk

    def title(self, w):
        name = (w.get("name_uk") if self.code == "uk" else None) or w["name"]
        return name + (self.tr(" (premium)", " (преміум)") if w.get("premium") else "")

    def round_name(self, key):
        ammo = self.data["ammo"][key]
        return (ammo.get("name_uk") if self.code == "uk" else None) or ammo["name"]

    def class_of(self, w):
        return CLASSES[w["category"]][self.code == "uk"]

    def round_keys(self, key):
        """The weapon's rounds, its default first."""
        default = self.data["default_ammo"][key]
        keys = self.data["compat"][key]
        assert default in keys, f"{key}: default ammo {default} not among {keys}"
        return [default] + [k for k in keys if k != default]

    def round_pick(self, name, fires=()):
        """A round chooser: its label, the chooser picker.js fills, and the
        hidden select that holds the choice."""
        esc = html.escape
        options = "".join(f'<option value="{esc(r["id"])}">{esc(r["name"])}</option>' for r in fires)
        return (f'<div class="round-pick"><label id="{name}-label" for="{name}-btn">{self.tr("Round", "Набій")}</label>'
                f'<div class="wpick" id="{name}-picker"></div><select id="{name}" class="round-select" autocomplete="off" hidden>{options}</select></div>')

    def slots(self):
        """The two weapon slots, A and B, with their rounds (picker.js WikiWeaponSlots)."""
        tr, esc, out = self.tr, html.escape, ['<div class="compare-pickers">']
        for side in ("a", "b"):
            out.append(f'<div class="compare-field"><label id="compare-{side}-label" for="compare-{side}-btn">{tr("Weapon", "Зброя")} {side.upper()}</label><div class="wpick" id="compare-{side}-picker"></div><select id="compare-{side}" hidden><option value="">{tr("Choose a weapon…", "Виберіть зброю…")}</option>')
            for key, w in sorted(self.data["weapons"].items(), key=lambda kv: self.title(kv[1])):
                out.append(f'<option value="{esc(key)}">{esc(self.title(w))}</option>')
            out.append('</select>' + self.round_pick(f"compare-{side}-round") + '</div>')
        out.append('</div>')
        return "".join(out)

    def round_options(self):
        """Every round once, for the round choosers (picker.js), with its lobby icon."""
        esc, data, out = html.escape, self.data, ['<div id="round-options" hidden>']
        for key in sorted({k for keys in data["compat"].values() for k in keys}):
            ammo = data["ammo"][key]
            icon = data.get("icons", {}).get(key)
            out.append(f'<div id="{esc(key)}" data-class="{self.tr("Rounds", "Набої")}" data-level="{ammo["item_level"]}" data-title="{esc(self.round_name(key))}" data-search-name="{esc(ammo["name"])}">'
                       + (f'<img src="{esc(icon)}" alt="">' if icon else '') + '</div>')
        out.append('</div>')
        return "".join(out)

    def stamp(self):
        return f'<p class="wiki-stamp">{html.escape(self.data.get("stamp_uk" if self.code == "uk" else "stamp", ""))}</p>'


def render_simulators(data, code, script_url):
    page = Page(data, code)
    tr, esc = page.tr, html.escape
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={code}">{tr("Wiki", "Вікі")}</a> / {tr("Simulators", "Симулятори")}</p>',
           f'<div class="wiki-heading"><h1>{tr("Simulators", "Симулятори")}</h1></div>',
           f'<section class="comparison" id="comparison" hidden aria-labelledby="sim-title"><h2 id="sim-title">{tr("Recoil and spread", "Відбій і розкид")}</h2>',
           page.slots(),
           f'<div class="compare-options"><button type="button" id="clear-comparison">{tr("Clear", "Очистити")}</button></div>',
           f'<p id="compare-empty">{tr("Choose a weapon; a second one to compare.", "Виберіть зброю; другу — для порівняння.")}</p>',
           f'<section id="recoil-sim" class="recoil-sim" hidden data-player="{esc(json.dumps({k: data["simulators"][k] for k in ("stance", "skill_influence")}))}">'
           + simulation(tr, data["simulators"], data["weapons"]) + '</section></section>',
           doll(), page.round_options(), '<div id="weapon-options" hidden>']
    for key, w in sorted(data["weapons"].items(), key=lambda kv: (kv[1]["item_level"], page.title(kv[1]).casefold())):
        rounds = [{"id": k, "name": page.round_name(k), "spread": data["ammo"][k]["dispersion"]} for k in page.round_keys(key)]
        recoil = {"rpm": w["rounds_per_minute"], "magazine": w["magazine_capacity"], **w.get("recoil", {}),
                  "dispersion": w.get("dispersion", {}), "double_handed": w["double_handed"]}
        icon = data.get("icons", {}).get(key)
        out.append(f'<div id="{esc(key)}" data-class="{esc(page.class_of(w))}" data-level="{w["item_level"]}" data-title="{esc(page.title(w))}" '
                   f'data-search-name="{esc(w["name"])}" data-rounds="{esc(json.dumps(rounds, ensure_ascii=False))}" data-recoil="{esc(json.dumps(recoil))}">'
                   + (f'<img src="{esc(icon)}" alt="">' if icon else '') + '</div>')
    out.append('</div>')
    out.append(page.stamp())
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out)


def render(data, code, script_url):
    page = Page(data, code)
    tr, title, round_name, round_pick = page.tr, page.title, page.round_name, page.round_pick
    esc = html.escape

    def rounds(key):
        """The weapon's rounds, its default first, each with the rows it
        changes from the default's (weapons.js swaps them in)."""
        default, *_ = keys = page.round_keys(key)
        base = {row["id"]: row for section in sections(data["weapons"][key], data["ammo"][default], code) for row in section["rows"]}
        out = []
        for k in keys:
            rows = {row["id"]: {"values": row["values"], "text": row["text"]}
                    for section in sections(data["weapons"][key], data["ammo"][k], code) for row in section["rows"]
                    if row["text"] != base[row["id"]]["text"]}
            out.append({"id": k, "name": round_name(k), "rows": rows})
        return out

    classes = sorted({CLASSES[w["category"]][code == "uk"] for w in data["weapons"].values()})
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={code}">{tr("Wiki", "Вікі")}</a> / {tr("Weapons", "Зброя")}</p>',
           f'<div class="wiki-heading"><div><p class="eyebrow">{tr("Equipment reference", "Довідник спорядження")}</p><h1>{tr("Weapons", "Зброя")}</h1></div>',
           f'<span class="catalogue-count" role="status" aria-live="polite" data-count data-label="{tr("weapons", "од. зброї")}">{len(data["weapons"])} {tr("weapons", "од. зброї")}</span></div>',
           f'<p class="wiki-intro">{tr("Compare weapons or expand a row for damage, handling, accuracy, recoil and aim sway.", "Порівнюйте зброю або розгортайте рядки, щоб побачити ушкодження, поводження, точність, відбій і хитання прицілу.")}</p>',
           f'<p class="wiki-note">{tr("Values with the chosen round, the weapon’s standard one first. Upgrades and character bonuses are not included. The game’s card shows the highest damage and the highest penetration among all the weapon’s rounds.", "Характеристики з вибраним набоєм, спершу зі стандартним для зброї. Модифікації та бонуси персонажа не враховано. Картка в грі показує найбільші ушкодження й найбільшу бронебійність серед усіх набоїв зброї.")}</p>',
           '<div class="catalogue-tools" hidden>',
           f'<label>{tr("Find a weapon", "Знайти зброю")}<input type="search" id="weapon-search" placeholder="{tr("Search by name…", "Пошук за назвою…")}" autocomplete="off"></label>',
           f'<label>{tr("Weapon class", "Клас зброї")}<select id="weapon-class"><option value="">{tr("All classes", "Усі класи")}</option>',
           *[f'<option>{esc(c)}</option>' for c in classes], '</select></label>',
           f'<label>{tr("Sort by", "Упорядкувати за")}<select id="weapon-sort">']
    for value, label in [("level", tr("Level", "Рівень")), ("name", tr("Name", "Назва")),
                         ("damage", tr("Damage: high to low", "Ушкодження: за спаданням")),
                         ("rpm", tr("Fire rate: high to low", "Темп стрільби: за спаданням")),
                         ("weight", tr("Weight: low to high", "Вага: за зростанням"))]:
        out.append(f'<option value="{value}">{label}</option>')
    out.extend(['</select></label></div>'])

    out.append(f'<section class="comparison" id="comparison" hidden aria-labelledby="compare-title"><h2 id="compare-title">{tr("Compare weapons", "Порівняння зброї")}</h2>')
    out.append(page.slots())
    out.append(f'<div class="compare-options"><label class="check"><input type="checkbox" id="only-differences">{tr("Only differences", "Лише відмінності")}</label><label class="check"><input type="checkbox" id="compare-colors" checked>{tr("Color better / worse values", "Позначати кращі / гірші значення")}</label><button type="button" id="clear-comparison">{tr("Clear", "Очистити")}</button><a id="compare-simulate" href="/wiki/simulators?lang={code}">{tr("Recoil and spread →", "Відбій і розкид →")}</a></div>')
    out.append(f'<p id="compare-empty">{tr("Choose two weapons to compare. Equal values and neutral traits keep their normal color.", "Виберіть дві одиниці зброї для порівняння. Однакові значення та нейтральні властивості зберігають звичайний колір.")}</p>')
    out.append(f'<p id="compare-same" hidden>{tr("No differences in the displayed stats.", "Відмінностей у показаних характеристиках немає.")}</p><div id="compare-table"></div></section>')
    out.append(page.round_options())
    out.append('<div id="weapon-list">')

    def stat(label, value, unit=""):
        return f'<div><dt>{label}</dt><dd>{number(value)}{(" <small>" + unit + "</small>") if unit else ""}</dd></div>'

    for key, w in sorted(data["weapons"].items(), key=lambda kv: (kv[1]["item_level"], title(kv[1]).casefold())):
        name = title(w)
        cls = page.class_of(w)
        fired = sections(w, data["ammo"][data["default_ammo"][key]], code)
        damage = next(row for section in fired for row in section["rows"] if row["id"] == "bullet_damage")
        attrs = {"name": name.casefold(), "search-name": w["name"], "premium": int(bool(w.get("premium"))), "class": cls, "level": w["item_level"],
                 "damage": damage["values"][0], "rpm": w["rounds_per_minute"], "weight": w["weight"]}
        fires = rounds(key)
        attrs["rounds"] = json.dumps(fires, ensure_ascii=False)
        out.append(f'<details class="weapon" id="{esc(key)}" ' + " ".join(f'data-{k}="{esc(str(v))}"' for k, v in attrs.items()) + '>')
        out.append('<summary>')
        icon = data.get("icons", {}).get(key)
        if icon:
            out.append(f'<img class="weapon-image" src="{esc(icon)}" alt="" width="128" height="64" loading="lazy">')
        else:
            out.append('<span class="weapon-image" aria-hidden="true"></span>')
        out.append(f'<span class="weapon-title"><strong>{esc(name)}</strong><span>{esc(cls)} · {tr("Level", "Рівень")} {number(w["item_level"])}</span></span>')
        out.append('<dl class="quick-stats">' + f'<div><dt>{tr("Damage", "Ушкодження")}</dt><dd>{esc(damage["text"])}</dd></div>'
                   + stat(tr("Fire rate", "Темп стрільби"), w["rounds_per_minute"], tr("rpm", "постр./хв"))
                   + stat(tr("Magazine", "Магазин"), w["magazine_capacity"])
                   + stat(tr("Weight", "Вага"), w["weight"], tr("kg", "кг")) + '</dl>')
        out.append(f'<span class="expand-label">{tr("Details", "Докладніше")} <span aria-hidden="true">+</span></span></summary>')
        out.append('<div class="weapon-details">')
        out.append(round_pick(f"{key}-round", fires))
        for section in fired:
            out.append(f'<section data-section="{section["id"]}"><h2>{esc(section["title"])}</h2>')
            if section.get("note"):
                out.append(f'<p class="stat-note">{esc(section["note"])}</p>')
            out.append('<dl>')
            for row in section["rows"]:
                out.append(f'<div data-stat="{row["id"]}" data-values="{esc(json.dumps(row["values"]))}" data-direction="{row["direction"]}"><dt>{esc(row["label"])}</dt><dd>{esc(row["text"])}</dd></div>')
            out.append('</dl></section>')
        out.append(f'<p class="weapon-simulate"><a href="/wiki/simulators?lang={code}&amp;a={esc(key)}">{tr("Recoil and spread →", "Відбій і розкид →")}</a></p>')
        out.append(f'<div class="compare-from-card" hidden><button type="button" data-compare-side="a">{tr("Set as weapon A", "Вибрати як зброю A")}</button><button type="button" data-compare-side="b">{tr("Set as weapon B", "Вибрати як зброю B")}</button></div>')
        out.append('</div></details>')
    out.append(f'</div><p id="weapons-empty" hidden>{tr("No weapons match. Try another name or class.", "Зброю не знайдено. Спробуйте іншу назву або клас.")}</p>')
    out.append(page.stamp())
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out)


def build(calculator, out):
    blob = calculator.read_bytes()
    # build_page.py writes a JSON literal. Decode that literal, never execute JS.
    data = json.JSONDecoder().raw_decode(blob.decode().split("const DATA = ", 1)[1])[0]
    # The stock camera shake clip (camerashake.json: its source and units).
    clip = json.loads((HERE / "camerashake.json").read_text())
    shake = b"globalThis.CAMERA_SHAKE = " + json.dumps({k: clip[k] for k in ("period_frames", "pitch", "yaw")}, separators=(",", ":")).encode() + b";"
    script = b"\n".join((HERE / name).read_bytes() for name in ("picker.js", "slots.js", "weapons.js"))
    script_url = "/wiki/weapons.js?v=" + hashlib.sha256(script).hexdigest()[:12]
    simulators = b"\n".join([shake] + [(HERE / name).read_bytes() for name in ("recoil.js", "spread.js", "picker.js", "slots.js", "simulators.js")])
    simulators_url = "/wiki/simulators.js?v=" + hashlib.sha256(simulators).hexdigest()[:12]
    out.mkdir(parents=True, exist_ok=True)
    for code in ("en", "uk"):
        (out / f"weapons.{code}.html").write_text(render(data, code, script_url))
        (out / f"simulators.{code}.html").write_text(render_simulators(data, code, simulators_url))
    (out / "weapons.js").write_bytes(script)
    (out / "simulators.js").write_bytes(simulators)
    # The modifiers page is the web server's (servers/web-server/src/modifiers.rs);
    # its script is the same chooser and its own part.
    (out / "modifiers.js").write_bytes(b"\n".join((HERE / name).read_bytes() for name in ("picker.js", "modifiers.js")))
    # The equipment page is the web server's too (src/equipment.rs).
    (out / "equipment.js").write_bytes((HERE / "equipment.js").read_bytes())
    (out / "source.sha256").write_text(hashlib.sha256(blob).hexdigest() + "\n")
    print(f"{out}: {len(data['weapons'])} weapons, EN/UK, from {calculator}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--calculator", type=Path, default=SITE / "damage" / "calc.js")
    parser.add_argument("--out", type=Path, default=SITE / "wiki")
    args = parser.parse_args()
    build(args.calculator, args.out)
