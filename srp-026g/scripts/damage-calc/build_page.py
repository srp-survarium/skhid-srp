"""Builds the calculator page: template.html with the data, engine, rules and UI inlined.

    python -I build_page.py <calc-data.json> <out.html> [<icons dir>] [<revision>]
    python -I build_page.py --site <calc-data.json> <out dir> <icons dir> <maps dir> <revision>

--site writes the Skhid website's copy (srp-026g/servers/web-server/damage/):
damage.html, with {{LANG}} and <!--SITE-HEADER--> for the server to fill,
calc.js (the data, engine, rules and UI), fonts/ and maps/ (the top-down
renders, assets/maps/README.md); the maps' names and minimaps come from the
Maps tab's build in the same folder (srp-026g/scripts/maps-tab: run it
first). Nothing inline runs and nothing comes from elsewhere: the site's Content-Security-Policy allows its own files only.
Every file the page loads is named with ?v=<its hash>, so a browser may keep
it for good and still gets the new one after a rebuild.

<revision>: the Skhid patch revision the configs were read at (stock 0.26g
when left out); the page says so in its header.

assets/ (next to this script) gives the HUD doll, the modification icons and
the font; the icons dir holds the item icons as <config name>.webp
(assets/tools/regenerate.sh). Without it the page has no item pictures.
"""
import base64
import hashlib
import json
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
assets = os.path.join(here, "assets")
site = sys.argv[1] == "--site"
if site:
    del sys.argv[1]
    maps_dir = sys.argv.pop(4)
data_path, out_path = sys.argv[1], sys.argv[2]
icons_dir = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3] else None
revision = sys.argv[4] if len(sys.argv) > 4 else None


def version(blob):
    return hashlib.sha256(blob).hexdigest()[:12]


def read(name):
    with open(os.path.join(here, name), encoding="utf-8") as f:
        return f.read()


def uri(path, mime):
    with open(path, "rb") as f:
        return f"data:{mime};base64," + base64.b64encode(f.read()).decode()


with open(data_path, encoding="utf-8") as f:
    data = json.load(f)

# The HUD doll (hud.swf sprite 119): its bitmaps, where each sits, and the
# click zones traced from them. The doll's zone ids -> hit locations
# (character's side; arm_* is the hand).
ZONES = {"brain": "brain", "face": "face", "front": "front", "back": "back"}
for side, word in (("l", "left"), ("r", "right")):
    for zone in ("arm", "forearm", "hip", "foot"):
        ZONES[f"{zone}_{side}"] = f"{word}_{zone}"


def zones(path):
    svg = open(path, encoding="utf-8").read()
    return [{"part": ZONES[m.group(1)], "transform": m.group(2), "d": m.group(3)}
            for m in re.finditer(r'<path id="([a-z_]+)" class="zone"[^>]*?transform="([^"]*)" d="([^"]*)"', svg)]


doll = os.path.join(assets, "hud-doll")
with open(os.path.join(doll, "placement.json"), encoding="utf-8") as f:
    placement = json.load(f)
data["hud_doll"] = {
    "frame": placement["frame"],
    "art": {name: uri(os.path.join(doll, name + ".png"), "image/png")
            for name in ("arm", "body", "head", "leg", "outline")},
    "parts": [{"name": p["name"], "art": os.path.basename(p["file"]).removesuffix(".png"),
               "w": p["w"], "h": p["h"], "matrix": p["matrix"]} for p in placement["parts"]],
    "zones": {"front": zones(os.path.join(assets, "doll", "doll_front.svg")),
              "back": zones(os.path.join(assets, "doll", "doll_back.svg"))},
}
# The recovery skills' icons, as the lobby's skill renderer draws them: the
# skills page's own (srp-026g/scripts/skills-tab/build.py, run first).
skill_icons = os.path.join(here, "..", "..", "servers", "web-server", "skills", "icons")
data["skill_icons"] = {str(s["id"]): uri(os.path.join(skill_icons, f"{s['id']}.webp"), "image/webp")
                       for s in ((data.get("recovery") or {}).get("skills") or {}).values()
                       if os.path.isfile(os.path.join(skill_icons, f"{s['id']}.webp"))}
data["mod_icons"] = {name.removeprefix("mod_icon_").removesuffix(".png"): uri(os.path.join(assets, "mod_icons", name), "image/png")
                     for name in os.listdir(os.path.join(assets, "mod_icons"))}
# Top-down maps: the scale and heights go in the page, the images ship beside
# it as maps/<key>.webp and load when a map is picked.
with open(os.path.join(assets, "maps", "maps.json"), encoding="utf-8") as f:
    maps = json.load(f)["maps"]
def map_name_uk(key):
    """The game's name for the map, with the version the English name gives."""
    base = key.removeprefix("v021d_")
    uk = (data.get("level_names", {}).get(base) or {}).get("uk")
    return uk and (uk + (" (v0.21)" if base != key else ""))


data["maps"] = {key: {"name": m["name"], "name_uk": map_name_uk(key),
                      "image": ("/damage/maps/" if site else "maps/") + m["image"], "size": m["image_size_px"],
                      "mpp": m["metres_per_pixel"], "p2w": m["pixel_to_world_affine"], "w2p": m["world_to_pixel_affine"],
                      "bounds": m["world_bounds"], "play": m.get("play_area_px"),
                      "elevation": {k: m["elevation"][k] for k in ("grid", "cell_m", "heights")} if m.get("elevation") else None}
                for key, m in maps.items()}
if icons_dir:
    data["icons"] = {name.rsplit(".", 1)[0]: uri(os.path.join(icons_dir, name), "image/webp")
                     for name in os.listdir(icons_dir) if name.endswith(".webp")}

if revision:
    data["stamp"] = f"Skhid revision {revision}"
    data["stamp_uk"] = f"Схід, ревізія {revision}"
engine = read("engine.js").split('if (typeof module !== "undefined")')[0]
if site:
    # The page's fonts, served by the site: the ones Google Fonts gives the
    # standalone page, PT Sans, PT Sans Narrow and PT Mono, plus Carlito.
    FONTS = [("PT Sans", 400, "normal", "pt-sans-regular"), ("PT Sans", 700, "normal", "pt-sans-bold"),
             ("PT Sans", 400, "italic", "pt-sans-italic"), ("PT Sans Narrow", 400, "normal", "pt-sans-narrow-regular"),
             ("PT Sans Narrow", 700, "normal", "pt-sans-narrow-bold"), ("PT Mono", 400, "normal", "pt-mono-regular"),
             ("Carlito", 700, "normal", "carlito-bold")]
    fonts = {file: open(os.path.join(assets, "fonts", file + ".woff2"), "rb").read() for _, _, _, file in FONTS}
    faces = "\n".join(f'@font-face {{ font-family: "{family}"; font-weight: {weight}; font-style: {style}; '
                       f'font-display: swap; src: url("/damage/fonts/{file}.woff2?v={version(fonts[file])}") format("woff2"); }}'
                       for family, weight, style, file in FONTS)
    # The Maps tab's build (srp-026g/scripts/maps-tab, run first): every
    # map's name from the game's modes and its HUD minimap, in its order.
    tab = os.path.join(out_path, "..", "maps", "maps.js")
    if os.path.isfile(tab):
        with open(tab, encoding="utf-8") as f:
            tab_maps = json.loads(re.search(r"const MAPS = (.*);\n", f.read()).group(1))["maps"]
        order = [t["key"] for t in tab_maps if t["key"] in data["maps"]]
        data["maps"] = {key: data["maps"][key] for key in order + [k for k in data["maps"] if k not in order]}
        for t in tab_maps:
            if t["key"] in data["maps"]:
                data["maps"][t["key"]].update(name=t["name"]["en"], name_uk=t["name"]["uk"], minimap=t["minimap"])
    images = {}
    for key, m in data["maps"].items():
        name = m["image"].rsplit("/", 1)[1]
        images[name] = open(os.path.join(maps_dir, name), "rb").read()
        m["image"] = f"/damage/maps/{name}?v={version(images[name])}"
    template = read("template.html")
    head, body = template.split("<style>", 1)
    style, rest = body.split("</style>", 1)
    title = re.search(r"<title>.*?</title>", head).group(0)
    markup = rest.split('<script id="calc-data"')[0]
    script = ('"use strict";\n// Built by srp-026g/scripts/damage-calc/build_page.py --site; do not edit.\n'
              f"const DATA = {json.dumps(data, ensure_ascii=False, separators=(',', ':'))};\n"
              f'{engine}\n{read("rules.js")}\n{read("ui.js")}')
    page = ('<!doctype html>\n<html lang="{{LANG}}">\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width,initial-scale=1">\n'
            f"{title}\n<style>\n{style.replace('/*__FONT__*/', faces)}</style>\n<!--SITE-HEADER-->\n"
            f'{markup.strip()}\n<script src="/damage/calc.js?v={version(script.encode())}"></script>\n')
    for sub_dir in ("fonts", "maps"):
        os.makedirs(os.path.join(out_path, sub_dir), exist_ok=True)
    with open(os.path.join(out_path, "damage.html"), "w", encoding="utf-8") as f:
        f.write(page)
    with open(os.path.join(out_path, "calc.js"), "w", encoding="utf-8") as f:
        f.write(script)
    for sub_dir, blobs in (("fonts", {k + ".woff2": v for k, v in fonts.items()}), ("maps", images)):
        for name, blob in blobs.items():
            with open(os.path.join(out_path, sub_dir, name), "wb") as f:
                f.write(blob)
    print(f"{out_path}: damage.html {len(page) / 1024:.0f} KB, calc.js {len(script) / 1024:.0f} KB, "
          f"{len(fonts)} fonts, {len(images)} maps")
    sys.exit(0)
# '</' would end the inline <script>.
payload = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/")
font = uri(os.path.join(assets, "fonts", "carlito-bold.woff2"), "font/woff2")
page = (read("template.html")
        .replace("/*__FONT__*/", f'@font-face {{ font-family: "Carlito"; font-weight: 700; src: url("{font}") format("woff2"); }}')
        .replace("/*__DATA__*/", payload)
        .replace("/*__ENGINE__*/", engine)
        .replace("/*__RULES__*/", read("rules.js"))
        .replace("/*__UI__*/", read("ui.js")))
with open(out_path, "w", encoding="utf-8") as f:
    f.write(page)
print(f"{out_path}: {len(page) / 1024:.0f} KB")
