#!/usr/bin/env python3
"""Build only the wiki's weapons catalogue from the calculator's generated data.

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


def simulation(tr):
    """A recoil simulation's controls and plots, for the comparison and each
    weapon card (weapons.js RecoilSim finds its parts by class)."""
    plots = "".join(
        f'<figure><figcaption>{tr(en, uk)} · {tr("game units", "одиниці гри")}</figcaption><svg class="sim-{axis}" viewBox="0 0 600 250" role="img" aria-label="{tr(en, uk)}"></svg></figure>'
        for axis, en, uk in [("vertical", "Vertical kick", "Вертикальний відбій"), ("horizontal", "Horizontal kick", "Бічний відбій")])
    return "".join([
        f'<p class="stat-note">{tr("Modelled kick, recovery, spread and camera shake (from the hip), standing still, with no mouse compensation. The plots are in the weapons’ kick units; the crosshair view is in degrees of view (upward exactly as the game does, sideways estimated), with each bullet’s hit and the spread circle.", "Модель поштовху, відновлення, розкиду й тремтіння камери (від стегна), стоячи на місці, без компенсації мишею. Графіки — в одиницях відбою зброї; приціл — у градусах огляду (вгору точно як у грі, убік приблизно), з влучанням кожної кулі та колом розкиду.")}</p>',
        f'<div class="sim-inputs"><label>{tr("Firing", "Стрільба")}<select class="sim-stance"><option value="aimed">{tr("Aimed", "Прицільно")}</option><option value="hip">{tr("From the hip", "Від стегна")}</option></select></label>'
        f'<label>{tr("Trigger", "Спуск")}<select class="sim-trigger"><option value="0">{tr("Hold", "Утримувати")}</option>'
        + "".join(f'<option value="{1 / n:g}">{tr(f"Click, {n} a second", f"Клацати, {n} на секунду")}</option>' for n in (4, 2, 1))
        + f'</select></label><label>{tr("Sample seed", "Зерно вибірки")}<input class="sim-seed" type="number" min="1" max="999999" value="1"></label></div>',
        f'<p class="stat-note">{tr("A full magazine; with two weapons, as many shots as the smaller magazine holds, each at its own fire rate. Random samples use the same seed for comparison; they are not recorded game shots.", "Повний магазин; для двох одиниць зброї стільки пострілів, скільки вміщує менший магазин, кожна у власному темпі. Для порівняння використано однакове зерно випадковості; це не запис пострілів із гри.")}</p>',
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


def render(data, code, script_url):
    def tr(en, uk):
        return en if code == "en" else uk

    def title(w):
        name = (w.get("name_uk") if code == "uk" else None) or w["name"]
        return name + (tr(" (premium)", " (преміум)") if w.get("premium") else "")

    esc = html.escape
    classes = sorted({CLASSES[w["category"]][code == "uk"] for w in data["weapons"].values()})
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={code}">{tr("Wiki", "Вікі")}</a> / {tr("Weapons", "Зброя")}</p>',
           f'<div class="wiki-heading"><div><p class="eyebrow">{tr("Equipment reference", "Довідник спорядження")}</p><h1>{tr("Weapons", "Зброя")}</h1></div>',
           f'<span class="catalogue-count" role="status" aria-live="polite" data-count data-label="{tr("weapons", "од. зброї")}">{len(data["weapons"])} {tr("weapons", "од. зброї")}</span></div>',
           f'<p class="wiki-intro">{tr("Compare weapons or expand a row for damage, handling, accuracy, recoil and aim sway.", "Порівнюйте зброю або розгортайте рядки, щоб побачити ушкодження, поводження, точність, відбій і хитання прицілу.")}</p>',
           f'<p class="wiki-note">{tr("Base weapon values. Ammunition, upgrades and character bonuses are not included.", "Базові характеристики зброї. Набої, модифікації та бонуси персонажа не враховано.")}</p>',
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
    out.append('<div class="compare-pickers">')
    for side in ("a", "b"):
        out.append(f'<div class="compare-field"><label id="compare-{side}-label" for="compare-{side}-btn">{tr("Weapon", "Зброя")} {side.upper()}</label><div class="wpick" id="compare-{side}-picker"></div><select id="compare-{side}" hidden><option value="">{tr("Choose a weapon…", "Виберіть зброю…")}</option>')
        for key, w in sorted(data["weapons"].items(), key=lambda kv: title(kv[1])):
            name = title(w)
            out.append(f'<option value="{esc(key)}">{esc(name)}</option>')
        out.append('</select></div>')
    out.append(f'</div><div class="compare-options"><label class="check"><input type="checkbox" id="only-differences">{tr("Only differences", "Лише відмінності")}</label><label class="check"><input type="checkbox" id="compare-colors" checked>{tr("Color better / worse values", "Позначати кращі / гірші значення")}</label><button type="button" id="clear-comparison">{tr("Clear", "Очистити")}</button><a href="#recoil-sim">{tr("Recoil simulation ↓", "Моделювання відбою ↓")}</a></div>')
    out.append(f'<p id="compare-empty">{tr("Choose two weapons to compare; one is enough for its recoil simulation. Equal values and neutral traits keep their normal color.", "Виберіть дві одиниці зброї для порівняння; для моделювання відбою досить однієї. Однакові значення та нейтральні властивості зберігають звичайний колір.")}</p>')
    out.append(f'<p id="compare-same" hidden>{tr("No differences in the displayed stats.", "Відмінностей у показаних характеристиках немає.")}</p><div id="compare-table"></div>')
    out.append(f'<section id="recoil-sim" class="recoil-sim" hidden aria-labelledby="recoil-title"><h2 id="recoil-title">{tr("Recoil simulation", "Моделювання відбою")}</h2>{simulation(tr)}</section></section>')
    # Each weapon card's own simulation is made from this when the card opens.
    out.append(doll())
    out.append(f'<template id="recoil-template"><h2>{tr("Recoil simulation", "Моделювання відбою")}</h2>{simulation(tr)}</template>')
    out.append('<div id="weapon-list">')

    def stat(label, value, unit=""):
        return f'<div><dt>{label}</dt><dd>{number(value)}{(" <small>" + unit + "</small>") if unit else ""}</dd></div>'

    for key, w in sorted(data["weapons"].items(), key=lambda kv: (kv[1]["item_level"], title(kv[1]).casefold())):
        name = title(w)
        cls = CLASSES[w["category"]][code == "uk"]
        attrs = {"name": name.casefold(), "search-name": w["name"], "premium": int(bool(w.get("premium"))), "class": cls, "level": w["item_level"],
                 "damage": w["bullet_damage"], "rpm": w["rounds_per_minute"], "weight": w["weight"]}
        attrs["recoil"] = json.dumps({"rpm": w["rounds_per_minute"], "magazine": w["magazine_capacity"], **w.get("recoil", {}),
                                      "dispersion": w.get("dispersion", {}), "stance": data.get("character_dispersion", {})})
        out.append(f'<details class="weapon" id="{esc(key)}" ' + " ".join(f'data-{k}="{esc(str(v))}"' for k, v in attrs.items()) + '>')
        out.append('<summary>')
        icon = data.get("icons", {}).get(key)
        if icon:
            out.append(f'<img class="weapon-image" src="{esc(icon)}" alt="" width="128" height="64" loading="lazy">')
        else:
            out.append('<span class="weapon-image" aria-hidden="true"></span>')
        out.append(f'<span class="weapon-title"><strong>{esc(name)}</strong><span>{esc(cls)} · {tr("Level", "Рівень")} {number(w["item_level"])}</span></span>')
        out.append('<dl class="quick-stats">' + stat(tr("Damage", "Ушкодження"), w["bullet_damage"] * 100)
                   + stat(tr("Fire rate", "Темп стрільби"), w["rounds_per_minute"], tr("rpm", "постр./хв"))
                   + stat(tr("Magazine", "Магазин"), w["magazine_capacity"])
                   + stat(tr("Weight", "Вага"), w["weight"], tr("kg", "кг")) + '</dl>')
        out.append(f'<span class="expand-label">{tr("Details", "Докладніше")} <span aria-hidden="true">+</span></span></summary>')
        out.append('<div class="weapon-details">')
        for section in sections(w, code):
            out.append(f'<section data-section="{section["id"]}"><h2>{esc(section["title"])}</h2>')
            if section.get("note"):
                out.append(f'<p class="stat-note">{esc(section["note"])}</p>')
            out.append('<dl>')
            for row in section["rows"]:
                out.append(f'<div data-stat="{row["id"]}" data-values="{esc(json.dumps(row["values"]))}" data-direction="{row["direction"]}"><dt>{esc(row["label"])}</dt><dd>{esc(row["text"])}</dd></div>')
            out.append('</dl></section>')
        out.append('<div class="weapon-recoil recoil-sim"></div>')
        out.append(f'<div class="compare-from-card" hidden><button type="button" data-compare-side="a">{tr("Set as weapon A", "Вибрати як зброю A")}</button><button type="button" data-compare-side="b">{tr("Set as weapon B", "Вибрати як зброю B")}</button></div>')
        out.append('</div></details>')
    out.append(f'</div><p id="weapons-empty" hidden>{tr("No weapons match. Try another name or class.", "Зброю не знайдено. Спробуйте іншу назву або клас.")}</p>')
    out.append(f'<p class="wiki-stamp">{esc(data.get("stamp_uk" if code == "uk" else "stamp", ""))}</p>')
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out)


def build(calculator, out):
    blob = calculator.read_bytes()
    # build_page.py writes a JSON literal. Decode that literal, never execute JS.
    data = json.JSONDecoder().raw_decode(blob.decode().split("const DATA = ", 1)[1])[0]
    # The stock camera shake clip (camerashake.json: its source and units).
    clip = json.loads((HERE / "camerashake.json").read_text())
    shake = b"globalThis.CAMERA_SHAKE = " + json.dumps({k: clip[k] for k in ("period_frames", "pitch", "yaw")}, separators=(",", ":")).encode() + b";"
    script = b"\n".join([shake] + [(HERE / name).read_bytes() for name in ("recoil.js", "picker.js", "weapons.js")])
    script_url = "/wiki/weapons.js?v=" + hashlib.sha256(script).hexdigest()[:12]
    out.mkdir(parents=True, exist_ok=True)
    for code in ("en", "uk"):
        (out / f"weapons.{code}.html").write_text(render(data, code, script_url))
    (out / "weapons.js").write_bytes(script)
    # The modifiers page is the web server's (servers/web-server/src/modifiers.rs);
    # its script is the same chooser and its own part.
    (out / "modifiers.js").write_bytes(b"\n".join((HERE / name).read_bytes() for name in ("picker.js", "modifiers.js")))
    # The equipment page is the web server's too (src/equipment.rs).
    (out / "equipment.js").write_bytes((HERE / "equipment.js").read_bytes())
    # So is the stats page (src/wiki.rs), filled by its script from /api/history.
    (out / "history.js").write_bytes((HERE / "history.js").read_bytes())
    (out / "source.sha256").write_text(hashlib.sha256(blob).hexdigest() + "\n")
    print(f"{out}: {len(data['weapons'])} weapons, EN/UK, from {calculator}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--calculator", type=Path, default=SITE / "damage" / "calc.js")
    parser.add_argument("--out", type=Path, default=SITE / "wiki")
    args = parser.parse_args()
    build(args.calculator, args.out)
