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
description (`ui.name`, `ui.description`) and the value of each rank, from
`ui.skill_descriptions` and `ui.postfix` as the exe writes the lobby's
tooltip: per rank r, description + d[r][0] + postfix + d[r][2], and below the
last rank + "Current value:" + d[r][1] + postfix + d[r][3]. A rank's value
here is the one the tooltip calls current at it (d[r][1], the last rank's
d[r][0]). `ui.invert_sign` is read by nothing in the exe: the strings carry
their signs. The texts are the English and Ukrainian localization; a branch's
name is the label the exe gives its tab (BRANCHES).

The icons are the lobby's, from flash_movies/lobby_menu.swf through FFDec,
drawn as its skill renderer (SrvSkillRendererCh) draws them: what lies under
its iconContainer (a plate, and `background`'s child at frame branch + 1, the
branch's tint), the icon in the container's mask, what lies over it (the
rim); a skill that `survarium.char.MedicalSkillIcon` names (its `show` switch)
has that bitmap, fitted to the icon sets' box; any other is frame `ui.icon` + 1
of the icon set PerkIconContainer places at frame branch + 1 (the movie's
SkillListRenderer). The renderer's overSprite, lockSprite and lockSpriteRed
are the page's hover, locked and excluded states.

Writes <out>/skills.<lang>.html (the wiki page's body, the server adds the
site's header), <out>/icons/<id>.webp, <out>/files.rs (those files, for the
server's include) and <out>/built-from.tsv (the patches it was built from:
release/release.sh rebuilds the page, through release/build-site, when it
offers others). Needs pillow and java on PATH.
"""
import argparse
import hashlib
import html
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
TYPES = {"webp": "image/webp"}
# The page's own words; the rest is the game's.
WORDS = {
    "title": ("Skills", "Навички"),
    "wiki": ("Wiki", "Вікі"),
    "or": ("or", "або"),
    "excludes": ("Not with: {}", "Не разом з: {}"),
    "locked": ("Needs {} in the branch", "Потрібно {} у гілці"),
}
# The lobby's skill renderer, and its sprites for the states: hovered, locked (its tier not
# reached), excluded (another skill excluding it is hovered).
RENDERER = "SrvSkillRendererCh"
OVERLAYS = {"over": "overSprite", "lock": "lockSprite", "lock-red": "lockSpriteRed"}
SVG, FFDEC = "http://www.w3.org/2000/svg", "https://www.free-decompiler.com/flash"
SHAPES = {2, 22, 32, 83}
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
    """SrvSkillRendererCh's children as FFDec draws it: [(character id, instance name or None)],
    bottom to top."""
    root = ElementTree.fromstring(svg)
    frame = next(iter(root))
    return [(int(use.get(f"{{{FFDEC}}}characterId")), use.get("id")) for use in frame.iter(f"{{{SVG}}}use")
            if use.get(f"{{{FFDEC}}}characterId")]


def icons(args, raw, work, tree, out):
    """{skill id: the icon's address} and {overlay: its address}: each skill's lobby icon as
    the skill renderer draws it, as webp in <out>/icons/, and the renderer's state overlays."""
    movie = raw / "flash_movies__lobby_menu.swf"
    ffdec = ["java", "-jar", args.ffdec]
    subprocess.run(ffdec + ["-selectclass", "survarium.char.MedicalSkillIcon", "-export", "script",
                            str(work / "script"), str(movie)], check=True, capture_output=True)
    source = (work / "script" / "scripts" / "survarium" / "char" / "MedicalSkillIcon.as").read_text()
    bitmaps = {int(i): cls for i, cls in re.findall(r"case (\d+):\s*\w+ = (\w+);", source)}
    _, tags = swf.split(movie.read_bytes())
    ids = {name: cid for cid, name in swf.symbol_classes(tags)}
    for name in ("PerkIconContainer", RENDERER):
        if name not in ids:
            die(f"lobby_menu.swf: no {name}")

    def only_child(sprite, frame=0):
        shown = sprite_frames(tags, sprite)[frame]
        if len(shown) != 1:
            die(f"lobby_menu.swf: sprite {sprite} has {len(shown)} children on frame {frame + 1}, not one")
        return next(iter(shown))

    sets = [only_child(ids["PerkIconContainer"], f) for f in range(len(sprite_frames(tags, ids["PerkIconContainer"])))]
    # The renderer: what lies under its iconContainer, over it, and the states' sprites.
    subprocess.run(ffdec + ["-format", "sprite:svg", "-selectid", str(ids[RENDERER]), "-export", "sprite",
                            str(work / "renderer"), str(movie)], check=True, capture_output=True)
    layers = renderer_layers(next((work / "renderer").glob("*/1.svg")).read_text())
    named = {name: cid for cid, name in layers if name}
    for name in ("background", "iconContainer", *OVERLAYS.values()):
        if name not in named:
            die(f"lobby_menu.swf: {RENDERER} has no {name}")
    at = [cid for cid, _ in layers].index(named["iconContainer"])
    shapes = {struct.unpack_from("<H", swf.payload(r))[0] for k, r in tags if k in SHAPES}
    under = [cid for cid, name in layers[:at] if name in (None, "background")]
    over = [cid for cid, name in layers[at + 1:] if name is None and cid in shapes]
    # The background's one child, its frame branch + 1 the branch's tint.
    tint = only_child(named["background"])
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
    chars = {cid for _, cid, _ in wanted.values()} | set(under) | set(over) | {tint, named["iconContainer"]}
    chars |= {named[n] for n in OVERLAYS.values()} | set(sets)
    subprocess.run(ffdec + ["-format", "sprite:png,shape:png,image:png", "-zoom", "2",
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

    # The layers are concentric discs: each is centred on one canvas.
    under_art = {cid: art(cid) for cid in under if cid != named["background"]}
    over_art = [art(cid) for cid in over]
    side = max(im.width for im in [*under_art.values(), *over_art, art(tint)])

    def centred(im, canvas=None):
        canvas = canvas or Image.new("RGBA", (side, side))
        box = im.getbbox() or (0, 0, im.width, im.height)
        cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
        layer = Image.new("RGBA", canvas.size)
        layer.paste(im, (round(side / 2 - cx), round(side / 2 - cy)))
        return Image.alpha_composite(canvas, layer)

    def save(im, name):
        im = im.resize((ICON_SIZE, ICON_SIZE), Image.LANCZOS)
        path = out / "icons" / f"{name}.webp"
        im.save(path, "WEBP", quality=90, method=6)
        return f"{SITE}/icons/{name}.webp?v={version(path.read_bytes())}"

    mask = centred(art(named["iconContainer"])).getchannel("A")
    frame_size = art(sets[0]).size
    (out / "icons").mkdir(parents=True)
    urls = {}
    for skill, (kind, cid, frame) in sorted(wanted.items()):
        branch = skill // 100 - 1
        canvas = Image.new("RGBA", (side, side))
        for c in under:
            canvas = centred(art(tint, branch + 1) if c == named["background"] else under_art[c], canvas)
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
    overlays = {key: save(centred(art(named[name])), key) for key, name in OVERLAYS.items()}
    return urls, overlays


def plain(text):
    """A game string without its markup (<green>...)."""
    return re.sub(r"<[^>]*>", "", str(text)).strip()


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
        values = []
        if ui.get("postfix"):
            if len(rows) != ranks + 1:
                die(f"skill {entry['id']}: {len(rows)} skill_descriptions for {ranks} ranks")
            values = [(rows[r][1], rows[r][3]) if r < ranks else (rows[r][0], rows[r][2]) for r in range(1, ranks + 1)]
        tree.append({"id": entry["id"], "branch": branch, "column": column, "tier": tier, "ranks": ranks,
                     "needs": pre.get("branch_points", 0), "excludes": pre.get("antiskills") or [],
                     "icon": ui["icon"], "name": ui["name"], "description": ui["description"],
                     "postfix": ui.get("postfix"), "values": values})
    return tree


def page(lang, tree, texts, urls, overlays):
    """The wiki page's body in `lang`."""
    i = 0 if lang == "en" else 1

    def text(key):
        found = texts[lang].get(key, texts["en"].get(key))
        if found is None:
            die(f"{key}: in no localization")
        return found

    word = lambda key: WORDS[key][i]
    esc = html.escape
    by_id = {s["id"]: s for s in tree}
    columns = max(s["column"] for s in tree)
    out = [f'<p class="breadcrumb"><a href="/wiki?lang={lang}">{word("wiki")}</a> / {word("title")}</p>',
           f'<div class="wiki-heading"><div><h1>{word("title")}</h1></div></div>',
           '<div class="skill-trees">']
    # The exclusions: hovering a skill locks what it excludes in red, as the lobby does.
    style = [f'.skill-trees:has([data-skill="{s["id"]}"]:is(:hover,:focus)) [data-skill="{x}"] .skill-lock-red'
             for s in tree for x in s["excludes"]]
    if style:
        out.append(f'<style>{",".join(style)}{{display:block}}</style>')
    excluded = {x for s in tree for x in s["excludes"]}
    overlay = lambda key: f'<img class="skill-{key}" src="{overlays[key]}" alt="" width="64" height="64" loading="lazy">'
    for branch in sorted({s["branch"] for s in tree}):
        skills = sorted((s for s in tree if s["branch"] == branch), key=lambda s: (s["tier"], s["column"]))
        tiers = sorted({s["tier"] for s in skills})
        # A tier's threshold: the least any of its skills needs; a skill that
        # needs more than the branch's other ranks add up to is out of reach.
        needs = {t: min(s["needs"] for s in skills if s["tier"] == t) for t in tiers}
        out.append(f'<section class="skill-branch"><h2>{esc(text(BRANCHES[branch]))}</h2>'
                   f'<div class="skill-grid" style="--columns:{columns}">')
        for t in tiers:
            out.append(f'<div class="skill-tier" style="grid-row:{t}">{points(lang, needs[t])}</div>')
        for s in skills:
            name = text(s["name"])
            reach = sum(o["ranks"] for o in skills if o is not s and o["needs"] < s["needs"])
            locked = s["needs"] > reach
            edge = " start" if s["column"] == 1 else " end" if s["column"] == columns else ""
            tip = [f"<b>{esc(name)}</b>", f"<p>{esc(text(s['description']))}</p>"]
            if s["values"]:
                postfix = text(s["postfix"])
                tip.append('<ol class="skill-ranks">' + "".join(
                    f'<li><span>{NUMERALS[r]}</span><span class="skill-value">{esc(plain(f"{a}{postfix}{b}"))}</span></li>'
                    for r, (a, b) in enumerate(s["values"])) + "</ol>")
            if s["excludes"]:
                tip.append(f'<p class="skill-note">{esc(word("excludes").format(", ".join(text(by_id[x]["name"]) for x in s["excludes"] if x in by_id)))}</p>')
            if locked or s["needs"] != needs[s["tier"]]:
                tip.append(f'<p class="skill-note">{esc(word("locked").format(points(lang, s["needs"])))}</p>')
            badge = f'<span class="skill-or">{word("or")}</span>' if s["excludes"] else ""
            out.append(
                f'<div class="skill{" locked" if locked else ""}{edge}" tabindex="0" data-skill="{s["id"]}" '
                f'style="grid-area:{s["tier"]}/{s["column"] + 1}">'
                f'<span class="skill-disc"><img src="{urls[s["id"]]}" alt="" width="64" height="64" loading="lazy">'
                f'{overlay("over")}{overlay("lock") if locked else ""}{overlay("lock-red") if s["id"] in excluded else ""}'
                f'<span class="skill-numeral">{NUMERALS[s["tier"] - 1]}</span>{badge}</span>'
                f'<span class="skill-name">{esc(name)}</span>'
                f'<span class="skill-pips">{"<i></i>" * s["ranks"]}</span>'
                f'<div class="skill-tip" role="tooltip">{"".join(tip)}</div></div>')
        out.append("</div></section>")
    out.append("</div>")
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
        urls, overlays = icons(args, raw, work, tree, out)
    for lang in LANGS:
        (out / f"skills.{lang}.html").write_text(page(lang, tree, texts, urls, overlays))
    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/skills-tab/build.py; do not edit. The files the skills page names.\n"
        "files! {\n" + "".join(f'    "icons/{f.name}" => "{TYPES["webp"]}",\n'
                               for f in sorted((out / "icons").iterdir())) + "}\n")
    write_built_from(out / "built-from.tsv", rows, "the skills (srp-026g/scripts/skills-tab/build.py)")
    print(f"{out}: {len(tree)} skills in {len({s['branch'] for s in tree})} branches")


if __name__ == "__main__":
    main()
