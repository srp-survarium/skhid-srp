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
sys.path.insert(0, str(HERE.parent / "site-texts"))
from stats import sections  # noqa: E402
from texts import CODES, LANG, WORDS, Texts  # noqa: E402
SITE = HERE.parents[1] / "servers" / "web-server"
CLASSES = {8: "Assault rifle", 16: "Machine gun", 15: "Submachine gun", 14: "Shotgun", 13: "Sniper rifle", 17: "Pistol"}


def number(value):
    return f"{value:.3f}".rstrip("0").rstrip(".")


CALC = HERE.parent / "damage-calc"
DOLL = CALC / "assets" / "doll" / "doll_front.svg"


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
    "dispersion_from_hip": "Spread from the hip {v} per rank, sprinting and jumping included. Not while aiming.",
    "dispersion_movement_influence": "Spread while walking {v} per rank, from the hip or aiming in. Not crouch-walking.",
    "injury_penalty": "Spread from broken arms {v} per rank.",
    "recoil": "Each shot's kick {v} per rank: up, sideways and the camera shake.",
}
# What a modification of each kind does: of the kit's player modifiers, or
# the weapon's kick ranges.
MOD_TIPS = {
    ("dispersion",): "The stance's spread and each shot's. No effect sprinting, jumping or fully aimed.",
    ("recoil",): "Each shot's kick with any weapon: up, sideways and the camera shake.",
    "kick": "This weapon's kick ranges, up and sideways. Not the camera shake.",
}


def named(item, field="name"):
    """The game's text `field` of `item` in each language (the calculator's data: `field`
    English, `<field>_<code>` the others)."""
    return {"en": item[field], **{code: item.get(f"{field}_{code}") for code in CODES[1:]}}


def skill_tip(skill, texts):
    """A spread skill's help line: what each of its modifiers does a rank (its
    ranks the sums of equal levels), then that fully aimed it does nothing."""
    first, *rest = skill["ranks"][1:]
    for n, rank in enumerate(rest, 2):
        assert rank == {m: round(v * n, 6) for m, v in first.items()}, f'skill {skill["id"]}: ranks not equal steps'
    lines = []
    for modifier, value in first.items():
        v = f'{"−" if value < 0 else "+"}{abs(value) * 100:g}%'
        lines.append(texts.tf(SKILL_TIPS[modifier], v=v))
    return " ".join(lines + [texts.t("No effect fully aimed.")])


def setup(texts, sim, weapons, mod_icons):
    """What a weapon's setup offers (simulators.js draws a panel of it per
    side with hud.js's rows, as the calculator draws its picks): the aim, the
    stance and the broken arms; a row per skill, its ranks; a row per
    modification kind a piece can have: the weapon's, scaling its kick
    ranges (`kick`), and each armour piece's that rolls or comes with one,
    adding to the kit's player modifiers (`effects`; the pieces' sum, as
    apply_modifier takes it). Each a card of what it does."""
    t = texts.t

    def pct(value):
        return f'{"−" if value < 0 else "+"}{abs(value) * 100:g}%'

    rows = [{"key": "aim", "label": t("Aim"), "required": True, "options": [[v, t(en)] for v, en in
             [("hip", "From the hip"), ("aiming", "Aiming in"), ("aimed", "Fully aimed")]]},
            {"key": "stance", "label": t("Stance"), "required": True, "options": [[v, t(en)] for v, en in
             [("idle", "Standing"), ("walk", "Walking"), ("crouch", "Crouching"),
              ("crouch_walk", "Crouch-walking"), ("run", "Sprinting"), ("jump", "Jumping")]]},
            {"key": "arms", "label": t("Broken arms"), "required": True, "options": [[str(n), str(n)] for n in range(3)]}]
    for sk in sim["skills"]:
        icon = SITE / "skills" / "icons" / f'{sk["id"]}.webp'
        name = texts.game(named(sk))
        rows.append({"key": f'skill-{sk["id"]}', "label": name, "card": [name, skill_tip(sk, texts)],
                     "icon": f'/skills/icons/{sk["id"]}.webp?v={hashlib.sha256(icon.read_bytes()).hexdigest()[:12]}',
                     "options": [[str(n), " ".join(pct(v) for v in rank.values())] for n, rank in enumerate(sk["ranks"]) if n],
                     "effects": {str(n): rank for n, rank in enumerate(sk["ranks"])}})
    # The modifications a Skhid item can have: srp rolls no defect
    # (crates/survarium/src/v026g/modifications.rs). A row per kind and piece.
    pieces = {}
    for m in sim["mods"]:
        if m["kind"] != "mod":
            continue
        assert "kit" in m or m["weapons"] == sorted(weapons), f'modification {m["id"]}: not every weapon takes it'
        for slot in m["slots"]:
            pieces.setdefault((m["type"], slot["slot"]), (slot, []))[1].append(m)
    for (kind, _), (slot, mods) in sorted(pieces.items(), key=lambda kv: (kv[0][1] != "weapon", kv[0][1], kv[0][0])):
        mods.sort(key=lambda m: abs(m["kick"] if "kick" in m else sum(m["kit"].values())))
        first = mods[0]
        labels, slots = named(first["card"], "label"), named(slot)
        label = texts.game({code: labels[code] and f'{labels[code]}, {(slots[code] or slots["en"]).lower()}'
                            for code in CODES})
        tip = t(MOD_TIPS["kick" if "kick" in first else tuple(sorted(first["kit"]))])
        rows.append({"key": f'mod-{kind}-{slot["slot"]}', "label": label, "card": [label, tip], "icon": mod_icons.get(str(first["icon"])),
                     "options": [[m["id"], texts.game(named(m["card"], "value"))] for m in mods],
                     **({"kick": {m["id"]: m["kick"] for m in mods}} if "kick" in first else {"effects": {m["id"]: m["kit"] for m in mods}})})
    return rows


def simulation(texts):
    """The simulation's own controls and plots (simulators.js RecoilSim finds
    its parts by class); each weapon's setup is beside its slot."""
    t, tf = texts.t, texts.tf
    parts = [t(en) for en in ("Base", "Stance", "A shot", "At most")]
    plots = "".join(
        f'<figure><figcaption>{t(en)} · {t("game units")}</figcaption><svg class="sim-{axis}" viewBox="0 0 600 250" role="img" aria-label="{t(en)}"></svg></figure>'
        for axis, en in [("vertical", "Vertical kick"), ("horizontal", "Horizontal kick")])
    return "".join([
        f'<div class="sim-inputs">'
        f'<label>{t("Trigger")}<select class="sim-trigger"><option value="0">{t("Hold")}</option>'
        + "".join(f'<option value="{1 / n:g}">{tf("Click, {n} a second", n=n)}</option>' for n in (4, 2, 1))
        + f'</select></label><label>{t("Sample seed")}<input class="sim-seed" type="number" min="1" max="999999" value="1"></label>'
        + f'<label>{t("Speed")}<select class="sim-speed">'
        + "".join(f'<option value="{value}">{label}</option>' for value, label in (("1", "1×"), ("0.25", "1/4"), ("0.1", "1/10")))
        + '</select></label></div>',
        '<div class="sim-layers">'
        + "".join(f'<label><input class="sim-layer-{key}" type="checkbox" checked> {t(en)}</label>' for key, en in [
            ("recoil", "Recoil"), ("camera", "Camera wobble"), ("gun", "Gun kick")])
        + '</div>',
        f'<table class="sim-parts"><caption>{t("Spread")} · {t("degrees")}</caption><thead><tr><td></td>'
        + "".join(f'<th scope="col">{p}</th>' for p in parts) + '</tr></thead><tbody></tbody></table>',
        f'<div class="sim-buttons"><button class="sim-play" type="button">{t("Play")}</button><button class="sim-last" type="button">{t("Last shot")}</button><button class="sim-reset" type="button">{t("Reset")}</button></div>',
        f'<label class="sim-timeline">{t("Timeline")} <output class="sim-time-label">0 s</output><input class="sim-time" type="range" min="0" max="1" step="0.01" value="0"></label>',
        '<div class="sim-legend" aria-live="polite"></div>',
        f'<figure class="sim-sight"><figcaption>{t("Crosshair")} · {t("degrees of view")}</figcaption><div class="sim-sight-views"></div>'
        '<p class="sim-key">'
        + "".join(f'<span><svg viewBox="0 0 16 16" aria-hidden="true">{shape}</svg>{t(en)}</span>' for shape, en in [
            ('<path class="sim-key-view" d="M8 1v5M8 10v5M1 8h5M10 8h5"/>', "bullet"),
            ('<circle class="sim-key-sight" cx="8" cy="8" r="6"/>', "sight"),
            ('<circle class="sim-key-hit" cx="8" cy="8" r="3.5"/>', "hit")])
        + '</p>'
        f'<label class="sim-target-pick">{t("Target")}<select class="sim-target"><option value="">{t("None")}</option>'
        + "".join(f'<option value="{m}">{tf("Enemy at {m} m", m=m)}</option>' for m in (5, 10, 25))
        + '</select></label></figure>',
        f'<div class="sim-plots">{plots}</div>',
        f'<p class="sim-unavailable" hidden>{t("Recoil parameters are unavailable for this selection.")}</p>',
    ])


class Page:
    """What both pages say of the calculator data, in every language: its words places of
    `texts` (site-texts/texts.py), which the web server fills in the reader's."""

    def __init__(self, data):
        self.data, self.texts = data, Texts()
        self.t = self.texts.t

    def title(self, w):
        name = self.texts.game(named(w))
        return name + (f' {self.t("(premium)")}' if w.get("premium") else "")

    def round_name(self, key):
        return self.texts.game(named(self.data["ammo"][key]))

    def class_of(self, w):
        return self.t(CLASSES[w["category"]])

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
        return (f'<div class="round-pick"><label id="{name}-label" for="{name}-btn">{self.t("Round")}</label>'
                f'<div class="wpick" id="{name}-picker"></div><select id="{name}" class="round-select" autocomplete="off" hidden>{options}</select></div>')

    def slots(self):
        """The two weapon slots, A and B, with their rounds (picker.js WikiWeaponSlots)."""
        t, esc, out = self.t, html.escape, ['<div class="compare-pickers">']
        for side in ("a", "b"):
            out.append(f'<div class="compare-field"><label id="compare-{side}-label" for="compare-{side}-btn">{self.texts.tf("Weapon {side}", side=side.upper())}</label><div class="wpick" id="compare-{side}-picker"></div><select id="compare-{side}" hidden><option value="">{t("Choose a weapon…")}</option>')
            # In English's order: the chooser (picker.js) orders them in the reader's.
            for key, w in sorted(self.data["weapons"].items(), key=lambda kv: english_title(kv[1])):
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
            out.append(f'<div id="{esc(key)}" data-class="{self.t("Rounds")}" data-level="{ammo["item_level"]}" data-title="{esc(self.round_name(key))}" data-search-name="{esc(ammo["name"])}">'
                       + (f'<img src="{esc(icon)}" alt="">' if icon else '') + '</div>')
        out.append('</div>')
        return "".join(out)

    def stamp(self):
        return f'<p class="wiki-stamp">{self.texts.tf("Skhid revision {revision}", revision=self.data["revision"])}</p>'

    def words(self):
        """The words of the page's script (the server's, by its T() calls)."""
        return f'<script type="application/json" id="page-words">{WORDS}</script>'


def english_title(w):
    """A weapon's name in English, the order of the page's lists (each language's reader gets
    them in its own from the page's script)."""
    return w["name"] + (" (premium)" if w.get("premium") else "")


def render_simulators(data, script_url):
    page = Page(data)
    t, esc = page.t, html.escape
    out = [f'<style>{(CALC / "hud.css").read_text()}</style>',
           f'<p class="breadcrumb"><a href="/wiki?lang={LANG}">{t("Wiki")}</a> / {t("Simulators")}</p>',
           f'<div class="wiki-heading"><h1>{t("Simulators")}</h1></div>',
           f'<section class="comparison" id="comparison" hidden aria-labelledby="sim-title"><h2 id="sim-title">{t("Recoil and spread")}</h2>',
           page.slots(),
           f'<div class="compare-options"><button type="button" id="clear-comparison">{t("Clear", "list")}</button></div>',
           f'<p id="compare-empty">{t("Choose a weapon; a second one to compare.")}</p>',
           '<div class="sim-setups">' + "".join(f'<div class="sim-setup" data-side="{n}" hidden><p class="sim-setup-name series-{n}"></p></div>' for n in range(2)) + '</div>',
           f'<section id="recoil-sim" class="recoil-sim" hidden data-player="{esc(json.dumps({k: data["simulators"][k] for k in ("stance", "skill_influence")}))}" '
           f'data-setup="{page.texts.attribute_json(setup(page.texts, data["simulators"], data["weapons"], data.get("mod_icons", {})))}">'
           + simulation(page.texts) + '</section></section>',
           doll(), page.round_options(), '<div id="weapon-options" hidden>']
    for key, w in sorted(data["weapons"].items(), key=lambda kv: (kv[1]["item_level"], english_title(kv[1]).casefold())):
        rounds = [{"id": k, "name": page.round_name(k), "spread": data["ammo"][k]["dispersion"]} for k in page.round_keys(key)]
        recoil = {"rpm": w["rounds_per_minute"], "magazine": w["magazine_capacity"], **w.get("recoil", {}),
                  "dispersion": w.get("dispersion", {}), "double_handed": w["double_handed"],
                  **({"shot": w["shot"]} if "shot" in w else {})}
        icon = data.get("icons", {}).get(key)
        out.append(f'<div id="{esc(key)}" data-class="{esc(page.class_of(w))}" data-level="{w["item_level"]}" data-title="{esc(page.title(w))}" '
                   f'data-search-name="{esc(w["name"])}" data-rounds="{page.texts.attribute_json(rounds)}" data-recoil="{esc(json.dumps(recoil))}">'
                   + (f'<img src="{esc(icon)}" alt="">' if icon else '') + '</div>')
    out.append('</div>')
    out.append(page.stamp())
    out.append(page.words())
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out), page.texts


def render(data, script_url):
    page = Page(data)
    t, title, round_name, round_pick = page.t, page.title, page.round_name, page.round_pick
    esc = html.escape

    def rounds(key):
        """The weapon's rounds, its default first, each with the rows it
        changes from the default's (weapons.js swaps them in)."""
        default, *_ = keys = page.round_keys(key)
        base = {row["id"]: row for section in sections(data["weapons"][key], data["ammo"][default], page.texts) for row in section["rows"]}
        out = []
        for k in keys:
            rows = {row["id"]: {"values": row["values"], "text": row["text"]}
                    for section in sections(data["weapons"][key], data["ammo"][k], page.texts) for row in section["rows"]
                    if row["text"] != base[row["id"]]["text"]}
            out.append({"id": k, "name": round_name(k), "rows": rows})
        return out

    # In English's order: the page's script orders them in the reader's.
    classes = sorted({CLASSES[w["category"]] for w in data["weapons"].values()})
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={LANG}">{t("Wiki")}</a> / {t("Weapons")}</p>',
           f'<div class="wiki-heading"><div><p class="eyebrow">{t("Equipment reference")}</p><h1>{t("Weapons")}</h1></div>',
           f'<span class="catalogue-count" role="status" aria-live="polite" data-count data-label="{t("weapons", "count")}">{len(data["weapons"])} {t("weapons", "count")}</span></div>',
           f'<p class="wiki-intro">{t("Compare weapons or expand a row for damage, handling, accuracy, recoil and aim sway.")}</p>',
           f'<p class="wiki-note">{t("Values with the chosen round, the weapon’s standard one first. Upgrades and character bonuses are not included. The game’s card shows the highest damage and the highest penetration among all the weapon’s rounds.")}</p>',
           '<div class="catalogue-tools" hidden>',
           f'<label>{t("Find a weapon")}<input type="search" id="weapon-search" placeholder="{t("Search by name…")}" autocomplete="off"></label>',
           f'<label>{t("Weapon class")}<select id="weapon-class"><option value="">{t("All classes")}</option>',
           *[f'<option>{t(c)}</option>' for c in classes], '</select></label>',
           f'<label>{t("Sort by")}<select id="weapon-sort">']
    for value, label in [("level", t("Level")), ("name", t("Name")),
                         ("damage", t("Damage: high to low")),
                         ("rpm", t("Fire rate: high to low")),
                         ("weight", t("Weight: low to high"))]:
        out.append(f'<option value="{value}">{label}</option>')
    out.extend(['</select></label></div>'])

    out.append(f'<section class="comparison" id="comparison" hidden aria-labelledby="compare-title"><h2 id="compare-title">{t("Compare weapons")}</h2>')
    out.append(page.slots())
    out.append(f'<div class="compare-options"><label class="check"><input type="checkbox" id="only-differences">{t("Only differences")}</label><label class="check"><input type="checkbox" id="compare-colors" checked>{t("Color better / worse values")}</label><button type="button" id="clear-comparison">{t("Clear", "list")}</button><a id="compare-simulate" href="/wiki/simulators?lang={LANG}">{t("Recoil and spread →")}</a></div>')
    out.append(f'<p id="compare-empty">{t("Choose two weapons to compare. Equal values and neutral traits keep their normal color.")}</p>')
    out.append(f'<p id="compare-same" hidden>{t("No differences in the displayed stats.")}</p><div id="compare-table"></div></section>')
    out.append(page.round_options())
    out.append('<div id="weapon-list">')

    def stat(label, value, unit=""):
        return f'<div><dt title="{label}">{label}</dt><dd>{number(value)}{(" <small>" + unit + "</small>") if unit else ""}</dd></div>'

    for key, w in sorted(data["weapons"].items(), key=lambda kv: (kv[1]["item_level"], english_title(kv[1]).casefold())):
        name = title(w)
        cls = page.class_of(w)
        fired = sections(w, data["ammo"][data["default_ammo"][key]], page.texts)
        damage = next(row for section in fired for row in section["rows"] if row["id"] == "bullet_damage")
        # data-name: what the list's search and its sort by name read (in the reader's language).
        attrs = {"name": name, "search-name": w["name"], "premium": int(bool(w.get("premium"))), "class": cls, "level": w["item_level"],
                 "damage": damage["values"][0], "rpm": w["rounds_per_minute"], "weight": w["weight"]}
        fires = rounds(key)
        out.append(f'<details class="weapon" id="{esc(key)}" ' + " ".join(f'data-{k}="{esc(str(v))}"' for k, v in attrs.items())
                   + f' data-rounds="{page.texts.attribute_json(fires)}">')
        out.append('<summary>')
        icon = data.get("icons", {}).get(key)
        if icon:
            out.append(f'<img class="weapon-image" src="{esc(icon)}" alt="" width="128" height="64" loading="lazy">')
        else:
            out.append('<span class="weapon-image" aria-hidden="true"></span>')
        out.append(f'<span class="weapon-title"><strong>{esc(name)}</strong><span>{esc(cls)} · {t("Level")} {number(w["item_level"])}</span></span>')
        out.append('<dl class="quick-stats">' + f'<div><dt title="{t("Damage")}">{t("Damage")}</dt><dd>{esc(damage["text"])}</dd></div>'
                   + stat(t("Fire rate"), w["rounds_per_minute"], t("rpm", "unit"))
                   + stat(t("Magazine"), w["magazine_capacity"])
                   + stat(t("Weight"), w["weight"], t("kg", "unit")) + '</dl>')
        out.append(f'<span class="expand-label">{t("Details")} <span aria-hidden="true">+</span></span></summary>')
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
        out.append(f'<p class="weapon-simulate"><a href="/wiki/simulators?lang={LANG}&amp;a={esc(key)}">{t("Recoil and spread →")}</a></p>')
        out.append(f'<div class="compare-from-card" hidden><button type="button" data-compare-side="a">{t("Set as weapon A")}</button><button type="button" data-compare-side="b">{t("Set as weapon B")}</button></div>')
        out.append('</div></details>')
    out.append(f'</div><p id="weapons-empty" hidden>{t("No weapons match. Try another name or class.")}</p>')
    out.append(page.stamp())
    out.append(page.words())
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out), page.texts


def build(calculator, out):
    blob = calculator.read_bytes()
    # build_page.py writes a JSON literal. Decode that literal, never execute JS.
    data = json.JSONDecoder().raw_decode(blob.decode().split("const DATA = ", 1)[1])[0]
    # The stock camera shake clip (camerashake.json: its source and units).
    clip = json.loads((HERE / "camerashake.json").read_text())
    shake = b"globalThis.CAMERA_SHAKE = " + json.dumps({k: clip[k] for k in ("period_frames", "pitch", "yaw")}, separators=(",", ":")).encode() + b";"
    # The pages' words first (site-texts/words.js).
    words = (HERE.parent / "site-texts" / "words.js").read_bytes()
    script = b"\n".join([words] + [(HERE / name).read_bytes() for name in ("picker.js", "slots.js", "weapons.js")])
    script_url = "/wiki/weapons.js?v=" + hashlib.sha256(script).hexdigest()[:12]
    # The setup panels are the calculator's rows (damage-calc/hud.js, hud.css).
    simulators = b"\n".join([words, shake] + [path.read_bytes() for path in (HERE / "recoil.js", HERE / "spread.js", HERE / "motions.js", HERE / "picker.js",
                                                                              HERE / "slots.js", CALC / "hud.js", HERE / "simulators.js")])
    simulators_url = "/wiki/simulators.js?v=" + hashlib.sha256(simulators).hexdigest()[:12]
    out.mkdir(parents=True, exist_ok=True)
    for name, (markup, texts) in (("weapons", render(data, script_url)), ("simulators", render_simulators(data, simulators_url))):
        (out / f"{name}.html").write_text(markup)
        texts.write(out / f"{name}.texts.json")
    (out / "weapons.js").write_bytes(script)
    (out / "simulators.js").write_bytes(simulators)
    # The modifiers page is the web server's (servers/web-server/src/modifiers.rs);
    # its script is the same chooser and its own part.
    (out / "modifiers.js").write_bytes(b"\n".join([words] + [(HERE / name).read_bytes() for name in ("picker.js", "modifiers.js")]))
    # The equipment page is the web server's too (src/equipment.rs).
    (out / "equipment.js").write_bytes((HERE / "equipment.js").read_bytes())
    print(f"{out}: {len(data['weapons'])} weapons, every language's places, from {calculator}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--calculator", type=Path, default=SITE / "damage" / "calc.js")
    parser.add_argument("--out", type=Path, default=SITE / "wiki")
    args = parser.parse_args()
    build(args.calculator, args.out)
