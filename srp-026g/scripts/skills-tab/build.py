#!/usr/bin/env python3
"""Builds the website's skills (`/wiki/skills`) from the game as Skhid's players have it.

    python3 -I build.py --stock <resources.db> --patches <dir> --dbgrab <dbgrab> --ffdec <ffdec-cli.jar>
                        [--with <resources.db.patch_n>] [--list release/patches.tsv] [--work <dir>]
                        [--out srp-026g/servers/web-server/skills]

The patches are exactly the rows of release/patches.tsv (each checked by its
sha256; srp-026g/scripts/released-game), mounted over the stock resources.db
in their order, as the game does. Nothing about a skill is written here: of
`gameplay/skills/skills_tree.options`, every skill's branch (its id / 100 - 1,
as the exe's fill_skills_properties 0x223a70 makes it), place in the tree
(`ui.position`: column, tier), threshold (`preconditions.branch_points`),
exclusions (`preconditions.antiskills`), ranks (`levels_amount`), name and
description (`ui.name`, `ui.description`), and the tooltip as the lobby writes
it (`ui.skill_descriptions` and `ui.postfix`: per rank r, description +
d[r][0] + postfix + d[r][2], and below the last rank + "Current value:" +
d[r][1] + postfix + d[r][3]): its per-rank line (rank 0's, without "Current
value": the page has no player) and the full rank's value, laid out as
PerkTipRenderer lays it out. A branch's card has its tab's icon
(SrvCharWindow's tab renderer, its `icon` at the frame CharWindow gives the
tab of that index). `ui.invert_sign` is read by nothing in the exe: the
strings carry their signs. The texts are the English and Ukrainian
localization; a branch's name is the label the exe gives its tab (BRANCHES).

The icons are the lobby's, from flash_movies/lobby_menu.swf through FFDec,
drawn as its skill renderer (SrvSkillRendererCh) draws them: what lies under
its iconContainer (a plate, and `background`'s child at frame branch + 1, the
branch's tint), the icon in the container's mask, what lies over it (the
rim); a skill that `survarium.char.MedicalSkillIcon` names (its `show` switch)
has that bitmap, fitted to the icon sets' box; any other is frame `ui.icon` + 1
of the icon set PerkIconContainer places at frame branch + 1 (the movie's
SkillListRenderer). The renderer's overSprite, lockSprite and lockSpriteRed
are the page's hover, locked and excluded states; its skillProgressIndicator
(SkillProgressIndicator: SkillListRenderer's pip class on an arc of `radius`,
`angleStep` apart) is drawn empty, one pip a rank.

Writes <out>/skills.<lang>.html (the wiki page's body, the server adds the
site's header), <out>/icons/<id>.webp, <out>/skills.js (the tooltip: hover, taps, keys;
placed out of the page's flow), <out>/files.rs (those files, for the
server's include) and <out>/built-from.tsv (the patches it was built from:
release/release.sh rebuilds the page, through release/build-site, when it
offers others). Needs pillow and java on PATH.
"""
import argparse
import hashlib
import html
import math
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from xml.etree import ElementTree

import numpy
from PIL import Image

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
sys.path.insert(0, str(SRP / "scripts" / "released-game"))
sys.path.insert(0, str(SRP.parent / "tools" / "actionscript"))
from released import LIST, die, extract, offered, write_built_from  # noqa: E402
from game_maps import load  # noqa: E402
import swf  # noqa: E402

WANTED = ["gameplay/skills/skills_tree.options", "localization/english/localization",
          "localization/ukrainian/localization", "flash_movies/lobby_menu.swf"]
LANGS = {"en": "english", "uk": "ukrainian"}
# The tabs' names, by branch: the exe's table (fill_character_data, 0x224828).
BRANCHES = ("st_shooting_skills_label", "st_physical_skills_label", "st_survival_skills_label",
            "st_engineering_skills_label", "st_anomaly_skills_label")
# The skill renderer's tier numerals (SkillListRenderer.LEVELS).
NUMERALS = ("I", "II", "III", "IV", "V", "VI")
SITE = "/skills"
ICON_SIZE = 128
TYPES = {"webp": "image/webp", "js": "text/javascript; charset=utf-8"}
# The page's own words; the rest is the game's.
WORDS = {
    "title": ("Skills", "Навички"),
    "wiki": ("Wiki", "Вікі"),
    "or": ("or", "або"),
    "max": ("Maximum:", "Максимум:"),
}
# The lobby's skill renderer, and its sprites for the states: hovered, locked (its tier not
# reached), excluded (another skill excluding it is hovered).
RENDERER = "SrvSkillRendererCh"
OVERLAYS = {"over": "overSprite", "lock": "lockSprite", "lock-red": "lockSpriteRed"}
SVG, FFDEC = "http://www.w3.org/2000/svg", "https://www.free-decompiler.com/flash"
SHAPES = {2, 22, 32, 83}
# The movie's scripts read: the medical icons, the rank indicator, the skill renderer.
SCRIPTS = ("survarium.char.MedicalSkillIcon", "survarium.char.SkillProgressIndicator",
           "survarium.buttons.SkillListRenderer", "survarium.char.CharWindow", "SrvCharWindow")
ZOOM = 2
PLACE_OBJECT2, PLACE_OBJECT3, REMOVE_OBJECT, REMOVE_OBJECT2, SHOW_FRAME, DEFINE_SPRITE = 26, 70, 5, 28, 1, 39


def version(blob):
    return hashlib.sha256(blob).hexdigest()[:12]


def points(lang, n):
    """`n` skill points, in words."""
    if lang == "en":
        return f"{n} point" + ("" if n == 1 else "s")
    tens, ones = n % 100, n % 10
    word = "очко" if ones == 1 and tens != 11 else "очки" if 2 <= ones <= 4 and not 12 <= tens <= 14 else "очок"
    return f"{n} {word}"


def sprite_frames(tags, sprite):
    """The characters on each frame of `sprite`'s display list."""
    for kind, raw in tags:
        body = swf.payload(raw)
        if kind != DEFINE_SPRITE or struct.unpack_from("<H", body)[0] != sprite:
            continue
        shown, frames, pos = {}, [], 4
        while pos < len(body):
            word, = struct.unpack_from("<H", body, pos)
            k, size, pos = word >> 6, word & 63, pos + 2
            if size == 63:
                size, = struct.unpack_from("<I", body, pos)
                pos += 4
            b, pos = body[pos:pos + size], pos + size
            if k == PLACE_OBJECT2 and b[0] & 2:
                shown[struct.unpack_from("<H", b, 1)[0]] = struct.unpack_from("<H", b, 3)[0]
            elif k == PLACE_OBJECT3 and b[0] & 2:
                # A class name first, if it has one (HasClassName, or HasImage).
                at = b.index(b"\0", 4) + 1 if b[1] & 0x18 else 4
                shown[struct.unpack_from("<H", b, 2)[0]] = struct.unpack_from("<H", b, at)[0]
            elif k == REMOVE_OBJECT2:
                shown.pop(struct.unpack_from("<H", b)[0], None)
            elif k == REMOVE_OBJECT:
                shown.pop(struct.unpack_from("<H", b, 2)[0], None)
            elif k == SHOW_FRAME:
                frames.append(set(shown.values()))
        return frames
    die(f"lobby_menu.swf: no sprite {sprite}")


def renderer_layers(svg):
    """SrvSkillRendererCh's children as FFDec draws it, bottom to top: [(character id, instance
    name or None, (x, y) of its box's centre)], in the renderer's units."""
    root = ElementTree.fromstring(svg)
    frame = next(iter(root))
    layers = []
    for use in frame.iter(f"{{{SVG}}}use"):
        if not use.get(f"{{{FFDEC}}}characterId"):
            continue
        x, y = map(float, re.findall(r"-?[\d.]+", use.get("transform"))[4:6])
        layers.append((int(use.get(f"{{{FFDEC}}}characterId")), use.get("id"),
                       (x + float(use.get("width")) / 2, y + float(use.get("height")) / 2), (x, y)))
    return layers


def script(work, name):
    path = work / "script" / "scripts" / Path(*name.split(".")).with_suffix(".as")
    if not path.is_file():
        die(f"lobby_menu.swf: FFDec made no script of {name}")
    return path.read_text()


def found(pattern, text, what):
    match = re.search(pattern, text)
    if not match:
        die(f"lobby_menu.swf: {what}")
    return match.groups()


def icons(args, raw, work, tree, out):
    """{skill id: the icon's address}, {overlay: its address} and the disc's size in renderer
    units: each skill's lobby icon as the skill renderer draws it, as webp in <out>/icons/, the
    renderer's state overlays and its rank indicator (empty) for each number of ranks."""
    movie = raw / "flash_movies__lobby_menu.swf"
    ffdec = ["java", "-jar", args.ffdec]
    subprocess.run(ffdec + ["-selectclass", ",".join(SCRIPTS), "-export", "script", str(work / "script"), str(movie)],
                   check=True, capture_output=True)
    bitmaps = {int(i): cls for i, cls in re.findall(r"case (\d+):\s*\w+ = (\w+);", script(work, SCRIPTS[0]))}
    # The rank indicator: SkillListRenderer's pip class; SkillProgressIndicator lays the pips
    # on an arc (radius, angleStep), each at frame `filled` below the rank, `empty` above it.
    pip_class, = found(r"skillProgressIndicator\.registerRenderer\((\w+)\)", script(work, SCRIPTS[2]), "no pip class")
    indicator = script(work, SCRIPTS[1])
    radius, = found(r"radius:uint = (\d+)", indicator, "no indicator radius")
    # The arc's first angle: -90 - count / 2 * step + step / 2, the step a local of FFDec's naming.
    arc = re.search(r"= -90 - \w+ / 2 \* (\w+) \+ \w+ / 2;", indicator)
    if not arc:
        die("lobby_menu.swf: no indicator arc")
    # Its value: the last one given it before (FFDec declares it 0 first).
    steps = re.findall(rf"\b{arc.group(1)}(?::\w+)? = (\d+);", indicator[:arc.start()])
    if not steps:
        die("lobby_menu.swf: no indicator angle step")
    step = steps[-1]
    _, empty = found(r"< _value \? (\d+) : (\d+)\)", indicator, "no pip frames")
    radius, step, empty = int(radius), int(step), int(empty)
    _, tags = swf.split(movie.read_bytes())
    ids = {name: cid for cid, name in swf.symbol_classes(tags)}
    for name in ("PerkIconContainer", RENDERER, pip_class):
        if name not in ids:
            die(f"lobby_menu.swf: no {name}")

    def only_child(sprite, frame=0):
        shown = sprite_frames(tags, sprite)[frame]
        if len(shown) != 1:
            die(f"lobby_menu.swf: sprite {sprite} has {len(shown)} children on frame {frame + 1}, not one")
        return next(iter(shown))

    sets = [only_child(ids["PerkIconContainer"], f) for f in range(len(sprite_frames(tags, ids["PerkIconContainer"])))]
    # The renderer: what lies under its iconContainer, over it, and the states' sprites.
    # The branches' tabs: SrvCharWindow's skillsTabPage renderer, whose `icon` goes to frame
    # `icon` of CharWindow's tab of that index (ButtonIconCenter).
    tab_class, = found(r'skillsTabPage\.itemRendererName = "(\w+)"', script(work, SCRIPTS[4]), "no skill tab renderer")
    tab_icons = {int(i): int(f) for i, f in re.findall(r'"index":(\d+),\s*"icon":(\d+)', script(work, SCRIPTS[3]))}
    if tab_class not in ids or not tab_icons:
        die("lobby_menu.swf: no skill tabs")
    subprocess.run(ffdec + ["-format", "sprite:svg", "-selectid", f"{ids[RENDERER]},{ids[tab_class]}", "-export", "sprite",
                            str(work / "renderer"), str(movie)], check=True, capture_output=True)
    svg_of = lambda cid: next(d / "1.svg" for d in (work / "renderer").iterdir() if re.fullmatch(rf"DefineSprite_{cid}(_.*)?", d.name))
    tab_icon = next((cid for cid, name, *_ in renderer_layers(svg_of(ids[tab_class]).read_text()) if name == "icon"), None)
    if tab_icon is None:
        die(f"lobby_menu.swf: {tab_class} has no icon")
    layers = renderer_layers(svg_of(ids[RENDERER]).read_text())
    named = {name: (cid, centre, origin) for cid, name, centre, origin in layers if name}
    for name in ("background", "iconContainer", "skillProgressIndicator", *OVERLAYS.values()):
        if name not in named:
            die(f"lobby_menu.swf: {RENDERER} has no {name}")
    cids = [cid for cid, *_ in layers]
    at = cids.index(named["iconContainer"][0])
    shapes = {struct.unpack_from("<H", swf.payload(r))[0] for k, r in tags if k in SHAPES}
    under = [cid for cid, name, *_ in layers[:at] if name in (None, "background")]
    over = [cid for cid, name, *_ in layers[at + 1:] if name is None and cid in shapes]
    # The disc's centre: the plate's, the renderer's first layer.
    centre = layers[0][2]
    # The background's one child, its frame branch + 1 the branch's tint.
    background = named["background"][0]
    tint = only_child(background)
    wanted = {}
    for s in tree:
        if s["id"] in bitmaps:
            if bitmaps[s["id"]] not in ids:
                die(f"lobby_menu.swf: no bitmap {bitmaps[s['id']]} (skill {s['id']})")
            wanted[s["id"]] = ("image", ids[bitmaps[s["id"]]], None)
        elif s["branch"] < len(sets):
            wanted[s["id"]] = ("sprite", sets[s["branch"]], s["icon"] + 1)
        else:
            die(f"skill {s['id']}: no icon (MedicalSkillIcon has none, PerkIconContainer no frame {s['branch'] + 1})")
    chars = {cid for _, cid, _ in wanted.values()} | set(under) | set(over) | {tint, named["iconContainer"][0]}
    chars |= {named[n][0] for n in OVERLAYS.values()} | set(sets) | {ids[pip_class], tab_icon}
    subprocess.run(ffdec + ["-format", "sprite:png,shape:png,image:png", "-zoom", str(ZOOM),
                            "-selectid", ",".join(map(str, sorted(chars))), "-export", "sprite,shape,image",
                            str(work / "art"), str(movie)], check=True, capture_output=True)

    def art(cid, frame=1):
        if cid in shapes:
            found = [work / "art" / "shapes" / f"{cid}.png"]
        elif frame is None:
            found = [f for f in (work / "art" / "images").iterdir() if f.name.startswith(f"{cid}_")]
        else:
            found = [d / f"{frame}.png" for d in (work / "art" / "sprites").iterdir()
                     if re.fullmatch(rf"DefineSprite_{cid}(_.*)?", d.name)]
        if not found or not found[0].is_file():
            die(f"lobby_menu.swf: FFDec made no picture of {cid}" + (f" frame {frame}" if frame else ""))
        return Image.open(found[0]).convert("RGBA")

    def pips(count):
        """Each pip's centre from the disc's centre, in renderer units (SkillProgressIndicator)."""
        ox, oy = named["skillProgressIndicator"][2]
        angle = int(-90 - count / 2 * step + step / 2)
        spots = []
        for j in range(count):
            a = math.radians(angle + j * step)
            spots.append((ox + math.cos(a) * radius - centre[0], oy - radius - math.sin(a) * radius - centre[1]))
        return spots

    pip = art(ids[pip_class], empty)
    counts = sorted({s["ranks"] for s in tree if s["ranks"]})
    reach = max((max(abs(x), abs(y)) for n in counts for x, y in pips(n)), default=0) * ZOOM + max(pip.size) / 2
    # The layers are concentric discs: each is centred on one canvas, big enough for the pips.
    under_art = {cid: art(cid) for cid in under if cid != background}
    over_art = [art(cid) for cid in over]
    plate = max(im.width for im in [*under_art.values(), *over_art, art(tint)])
    side = math.ceil(max(plate, 2 * reach))

    def centred(im, canvas=None, offset=(0, 0)):
        canvas = canvas or Image.new("RGBA", (side, side))
        box = im.getbbox() or (0, 0, im.width, im.height)
        cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
        layer = Image.new("RGBA", canvas.size)
        layer.paste(im, (round(side / 2 + offset[0] - cx), round(side / 2 + offset[1] - cy)))
        return Image.alpha_composite(canvas, layer)

    def save(im, name):
        im = im.resize((round(ICON_SIZE * side / plate),) * 2, Image.LANCZOS)
        path = out / "icons" / f"{name}.webp"
        im.save(path, "WEBP", quality=90, method=6)
        return f"{SITE}/icons/{name}.webp?v={version(path.read_bytes())}"

    mask = centred(art(named["iconContainer"][0])).getchannel("A")
    frame_size = art(sets[0]).size
    (out / "icons").mkdir(parents=True)
    urls = {}
    for skill, (kind, cid, frame) in sorted(wanted.items()):
        branch = skill // 100 - 1
        canvas = Image.new("RGBA", (side, side))
        for c in under:
            canvas = centred(art(tint, branch + 1) if c == background else under_art[c], canvas)
        icon = art(cid, frame)
        if kind == "image":
            # MedicalSkillIcon fits its bitmap to the icon set's box.
            icon = icon.resize(frame_size, Image.LANCZOS)
        icon = centred(icon)
        icon.putalpha(Image.fromarray(numpy.minimum(numpy.asarray(icon.getchannel("A")), numpy.asarray(mask))))
        canvas = Image.alpha_composite(canvas, icon)
        for im in over_art:
            canvas = centred(im, canvas)
        urls[skill] = save(canvas, skill)
    overlays = {key: save(centred(art(named[name][0])), key) for key, name in OVERLAYS.items()}
    for n in counts:
        canvas = Image.new("RGBA", (side, side))
        for x, y in pips(n):
            canvas = centred(pip, canvas, (x * ZOOM, y * ZOOM))
        overlays[f"pips-{n}"] = save(canvas, f"pips-{n}")
    # Each branch's tab icon, cut to its picture: a light glyph the page tints in its heading's
    # colour (the wiki's .glyph, as its modification icons).
    branch_icons = {}
    for branch in sorted({s["branch"] for s in tree}):
        if branch not in tab_icons:
            die(f"lobby_menu.swf: CharWindow has no tab {branch}")
        im = art(tab_icon, tab_icons[branch])
        im = im.crop(im.getbbox())
        path = out / "icons" / f"branch-{branch}.webp"
        im.save(path, "WEBP", quality=90, method=6)
        branch_icons[branch] = f"{SITE}/icons/branch-{branch}.webp?v={version(path.read_bytes())}"
    return urls, overlays, side / plate, branch_icons


def tooltip(skill, text, maximum):
    """The tooltip's text as HTML: the lobby's line for a rank as fill_skills_properties writes
    it (description + d[0][0] + postfix + d[0][2]; no "Current value": the page has no player),
    then, for a skill of several ranks, its value at the last rank (d[last][0] + postfix +
    d[last][2], the lobby's text at full rank) after `maximum`; the markup (<green>) as the
    movie's stylesheet draws it."""
    said = text(skill["description"])
    if skill["postfix"]:
        postfix = text(skill["postfix"])
        line = lambda d: str(d[0]) + postfix + str(d[2])
        said += line(skill["rows"][0])
        if skill["ranks"] > 1:
            said += "\n" + maximum + line(skill["rows"][-1])
    said = html.escape(said.strip())
    said = said.replace("&lt;green&gt;", '<span class="skill-value">').replace("&lt;/green&gt;", "</span>")
    said = re.sub(r"&lt;/?\w+&gt;", "", said).replace("\n", "<br>")
    # A value stays on its label's line.
    return re.sub(r'\s*<span class="skill-value">\s*', '&nbsp;<span class="skill-value">', said)


def skills_of(raw):
    """The tree's skills, each as the page needs it (texts as keys)."""
    config = load(raw, "gameplay/skills/skills_tree.options")
    if not config:
        die("no gameplay/skills/skills_tree.options")
    tree = []
    for entry in config["skills_tree"]:
        ui, pre, ranks = entry["ui"], entry.get("preconditions") or {}, entry["levels_amount"]
        column, tier = ui["position"]
        branch = entry["id"] // 100 - 1
        if not 0 <= branch < len(BRANCHES):
            die(f"skill {entry['id']}: branch {branch} has no tab in the exe")
        rows = ui.get("skill_descriptions") or []
        if ui.get("postfix") and len(rows) != ranks + 1:
            die(f"skill {entry['id']}: {len(rows)} skill_descriptions for {ranks} ranks")
        tree.append({"id": entry["id"], "branch": branch, "column": column, "tier": tier, "ranks": ranks,
                     "needs": pre.get("branch_points", 0), "excludes": pre.get("antiskills") or [],
                     "icon": ui["icon"], "name": ui["name"], "description": ui["description"],
                     "postfix": ui.get("postfix"), "rows": rows})
    return tree


def page(lang, tree, texts, urls, overlays, disc, script_url, branch_icons):
    """The wiki page's body in `lang`."""
    i = 0 if lang == "en" else 1

    def text(key):
        found = texts[lang].get(key, texts["en"].get(key))
        if found is None:
            die(f"{key}: in no localization")
        return found

    word = lambda key: WORDS[key][i]
    esc = html.escape
    columns = max(s["column"] for s in tree)
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={lang}">{word("wiki")}</a> / {word("title")}</p>',
           f'<div class="wiki-heading"><div><h1>{word("title")}</h1></div></div>',
           f'<div class="skill-trees" style="--disc:{disc:.4f}">']
    # The exclusions: hovering a skill locks what it excludes in red, as the lobby does.
    # (A hovered skill with a mouse; an opened one, tapped; a focused one, keyboard.)
    style = lambda state: ",".join(
        f'.skill-trees:has([data-skill="{s["id"]}"]{state}) [data-skill="{x}"] .skill-lock-red'
        for s in tree for x in s["excludes"])
    if style(""):
        out.append(f'<style>{style(":is(.open,:focus-visible)")}{{display:block}}'
                   f'@media(hover:hover){{{style(":hover")}{{display:block}}}}</style>')
    excluded = {x for s in tree for x in s["excludes"]}
    overlay = lambda key: f'<img class="skill-{key.split("-")[0] if key.startswith("pips") else key}" src="{overlays[key]}" alt="" loading="lazy">'
    for branch in sorted({s["branch"] for s in tree}):
        skills = sorted((s for s in tree if s["branch"] == branch), key=lambda s: (s["tier"], s["column"]))
        tiers = sorted({s["tier"] for s in skills})
        # A tier's threshold: the least any of its skills needs; a skill that
        # needs more than the branch's other ranks add up to is out of reach.
        needs = {t: min(s["needs"] for s in skills if s["tier"] == t) for t in tiers}
        out.append(f'<section class="skill-branch"><h2><span class="glyph" style="--icon:url(\'{branch_icons[branch]}\')" aria-hidden="true"></span>{esc(text(BRANCHES[branch]))}</h2>'
                   f'<div class="skill-grid" style="--columns:{columns}">')
        for t in tiers:
            out.append(f'<div class="skill-tier" style="grid-row:{t}"><b>{NUMERALS[t - 1]}</b> · {points(lang, needs[t])}</div>')
        for s in skills:
            name = text(s["name"])
            reach = sum(o["ranks"] for o in skills if o is not s and o["needs"] < s["needs"])
            locked = s["needs"] > reach
            # PerkTipRenderer: the name, a line, the text; one line unless the skill has one rank.
            wrap = " wrap" if s["ranks"] <= 1 else ""
            tip = f'<div class="skill-tip{wrap}"><b>{esc(name)}</b><p>{tooltip(s, text, word("max"))}</p></div>'
            badge = f'<span class="skill-or">{word("or")}</span>' if s["excludes"] else ""
            out.append(
                f'<div class="skill{" locked" if locked else ""}" tabindex="0" data-skill="{s["id"]}" '
                f'style="grid-area:{s["tier"]}/{s["column"] + 1}">'
                f'<span class="skill-disc"><img src="{urls[s["id"]]}" alt="" loading="lazy">'
                f'{overlay("over")}{overlay("lock") if locked else ""}{overlay("lock-red") if s["id"] in excluded else ""}'
                f'{overlay(f"pips-{s['ranks']}") if s["ranks"] else ""}{badge}</span>'
                f'<span class="skill-name">{esc(name)}</span>{tip}</div>')
        out.append("</div></section>")
    out.append('</div>\n<div class="skill-float" role="tooltip" hidden></div>')
    out.append(f'<script src="{script_url}" defer></script>')
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--stock", required=True)
    ap.add_argument("--patches", required=True, help="the folder with the list's resources.db.patch_<n>")
    ap.add_argument("--list", default=str(LIST))
    ap.add_argument("--with", dest="with_patch", metavar="PATCH",
                    help="the next release's resources.db.patch_<n>, not in the list yet")
    ap.add_argument("--dbgrab", required=True)
    ap.add_argument("--ffdec", required=True, help="ffdec-cli.jar")
    ap.add_argument("--out", default=str(SRP / "servers" / "web-server" / "skills"))
    ap.add_argument("--work", help="keep the extracted files here and reuse them (while working on the page)")
    args = ap.parse_args()
    rows = offered(args.list, args.with_patch)
    out = Path(args.out)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(args.work or tmp)
        raw = work / "raw"
        if not (args.work and raw.is_dir()):
            raw.mkdir(parents=True, exist_ok=True)
            extract(args.dbgrab, args.stock, rows, args.patches, WANTED, raw)
        for sub in ("script", "art"):
            shutil.rmtree(work / sub, ignore_errors=True)
        tree = skills_of(raw)
        texts = {lang: load(raw, f"localization/{name}/localization")["strings"] for lang, name in LANGS.items()}
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        urls, overlays, disc, branch_icons = icons(args, raw, work, tree, out)
    shutil.copy(HERE / "skills.js", out / "skills.js")
    script_url = f"{SITE}/skills.js?v={version((out / 'skills.js').read_bytes())}"
    for lang in LANGS:
        (out / f"skills.{lang}.html").write_text(page(lang, tree, texts, urls, overlays, disc, script_url, branch_icons))
    served = ["skills.js"] + [f"icons/{f.name}" for f in sorted((out / "icons").iterdir())]
    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/skills-tab/build.py; do not edit. The files the skills page names.\n"
        "files! {\n" + "".join(f'    "{f}" => "{TYPES[f.rsplit(".", 1)[1]]}",\n' for f in served) + "}\n")
    write_built_from(out / "built-from.tsv", rows, "the skills (srp-026g/scripts/skills-tab/build.py)")
    print(f"{out}: {len(tree)} skills in {len({s['branch'] for s in tree})} branches")


if __name__ == "__main__":
    main()
