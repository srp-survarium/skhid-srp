#!/usr/bin/env python3
"""Build the wiki's icons from the lobby (lobby_menu.swf) as the players have it.

    python3 -I build_icons.py --json <config json dir> --images <lobby images dir>
                              --item-icons <calculator icons dir> --swf <lobby_menu.swf> --ffdec <ffdec-cli.jar>
                              --flash <srp-026g/flash/lobby_menu/src> --out <servers/web-server/wiki/icons>

damage-calc/build_site.py runs it after the calculator's icons, so every
release makes them again from its own patches. Writes <out>/ anew:

- items/<items_dict name>.webp: the lobby icon of every item of a category
  the shop's equipment and weapon filters name, as the calculator's build cut
  it (assets/tools/build_icons.py, 171x83);
- mods/<ui_desc.icon>.webp: every 38x38 cell of the `modifiers` atlas
  (character 3279, IconContainer("modifiers", 38, 38, vertical): column-major)
  that holds an icon; each icon a modification shows must be one;
- groups/<item_category>.webp: the shop filter button of each of those
  categories (FilterItem: itemIcon.gotoAndStop(300 + icon), FiltersWindow's
  equipmentFilters() and weaponFilters() name each filter's icon and category;
  FFDec exports the frames);
- files.rs: those files, for the server's include (servers/web-server/src/icons.rs).

The modification and filter icons are light glyphs with a dark drop shadow.
The site tints them in its own ink (CSS masks), so they are written white,
with the glyph's brightness as their alpha: the shadow drops out.
"""
import argparse
import base64
import io
import json
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

import numpy
from PIL import Image

MOD_ATLAS = "3279_LobbyMenu_ModifierIcons.png"
MOD_CELL = 38
# FilterItemCatRenderer's `itemIcon` (lobby_menu.swf sprite 1125): frame
# 300 + icon draws a filter's icon. FFDec exports its frames as SVG.
FILTER_SPRITE = 1125
FILTER_FRAME = 300
TYPES = {"webp": "image/webp"}


def load(json_dir, name):
    return json.loads((json_dir / name).read_text(encoding="utf-8"))


def rows(table):
    return table if isinstance(table, list) else list(table.values())


def filters(flash):
    """{item_category: filter icon} of the shop's equipment and weapon filters."""
    constants = dict((name, int(value)) for name, value in re.findall(
        r"public static const (\w+):int = (\d+);", (flash / "survarium/constants/ItemCategory.as").read_text()))
    source = (flash / "survarium/controls/FiltersWindow.as").read_text()
    found = {}
    for which in ("equipmentFilters", "weaponFilters"):
        block = re.search(rf"function {which}\(\) : Array\s*\{{\s*return \[(.*?)\];", source, re.S)
        if not block:
            raise SystemExit(f"FiltersWindow.as: no {which}() list")
        for icon, category in re.findall(r'"icon":(\d+),.*?"sucat":ItemCategory\.(\w+)', block.group(1), re.S):
            found.setdefault(constants[category], int(icon))
    return found


def glyph(image):
    """White, with the glyph's brightness (over its brightest) as alpha."""
    pixels = numpy.asarray(image.convert("RGBA"), dtype=numpy.float64)
    light = pixels[..., :3] @ [0.299, 0.587, 0.114]
    alpha = pixels[..., 3]
    top = light[alpha > 128].max(initial=0) or 255
    out = numpy.full(pixels.shape, 255, dtype=numpy.uint8)
    out[..., 3] = numpy.rint(alpha * numpy.minimum(1, light / top))
    return Image.fromarray(out, "RGBA")


def webp(image):
    data = io.BytesIO()
    image.save(data, "WEBP", lossless=True, quality=100, method=6, exact=True)
    return data.getvalue()


def frame_image(frames, frame):
    """The one bitmap a filter frame's SVG embeds."""
    svg = (frames / f"{frame}.svg").read_text(encoding="utf-8")
    found = re.findall(r'xlink:href="data:image/png;base64,([^"]+)"', svg, re.I)
    if len(found) != 1:
        raise SystemExit(f"sprite {FILTER_SPRITE} frame {frame}: {len(found)} bitmaps, not one")
    return Image.open(io.BytesIO(base64.b64decode(found[0])))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    for name in ("json", "images", "item-icons", "swf", "ffdec", "flash", "out"):
        ap.add_argument(f"--{name}", required=True, type=Path)
    args = ap.parse_args()
    out = args.out
    shutil.rmtree(out, ignore_errors=True)
    files = {}

    def write(path, data):
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(data)
        files[path] = TYPES[path.rsplit(".", 1)[1]]

    categories = filters(args.flash)
    dictionary = load(args.json, "gameplay__db_static_dictionaries.json")["items_dict"]
    # The calculator names an icon by its config's file name, or by the
    # dictionary's name where file names clash (assets/tools/build_icons.py).
    stems = {}
    for item in dictionary:
        stems.setdefault(item["cfg_name"].rsplit("/", 1)[-1].removesuffix(".options"), []).append(item)
    missing = []
    for item in sorted(dictionary, key=lambda item: item["name"]):
        if item["item_category"] not in categories:
            continue
        stem = item["cfg_name"].rsplit("/", 1)[-1].removesuffix(".options")
        source = args.item_icons / f"{item['name'] if len(stems[stem]) > 1 else stem}.webp"
        if source.is_file():
            write(f"items/{item['name']}.webp", source.read_bytes())
        else:
            missing.append(item["name"])
    if missing:
        raise SystemExit(f"no lobby icon for {', '.join(missing)}")

    atlas = Image.open(args.images / MOD_ATLAS).convert("RGBA")
    cells = atlas.height // MOD_CELL
    shown = {row["ui_desc"]["icon"] for row in rows(
        load(args.json, "gameplay__items__modifications__modifications.options.json")["modifications"])}
    cut = set()
    for index in range(cells * (atlas.width // MOD_CELL)):
        column, row = divmod(index, cells)
        cell = atlas.crop((column * MOD_CELL, row * MOD_CELL, (column + 1) * MOD_CELL, (row + 1) * MOD_CELL))
        if cell.getchannel("A").getbbox() is not None:
            write(f"mods/{index}.webp", webp(glyph(cell)))
            cut.add(index)
    if shown - cut:
        raise SystemExit(f"modification icons {sorted(shown - cut)} are no cell of {MOD_ATLAS}")

    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(["java", "-jar", str(args.ffdec), "-format", "sprite:svg", "-selectid", str(FILTER_SPRITE),
                        "-export", "sprite", tmp, str(args.swf)], check=True, capture_output=True)
        frames = Path(tmp) / f"DefineSprite_{FILTER_SPRITE}"
        for category, icon in sorted(categories.items()):
            write(f"groups/{category}.webp", webp(glyph(frame_image(frames, FILTER_FRAME + icon))))

    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/wiki/build_icons.py; do not edit. The wiki's icons.\n"
        "files! {\n" + "".join(f'    "{path}" => "{kind}",\n' for path, kind in sorted(files.items())) + "}\n")
    total = sum((out / path).stat().st_size for path in files)
    counts = {kind: sum(path.startswith(kind + "/") for path in files) for kind in ("items", "mods", "groups")}
    print(f"{out}: {counts['items']} items, {counts['mods']} modification and {counts['groups']} group icons, "
          f"{total / 1024:.0f} KB")


if __name__ == "__main__":
    main()
