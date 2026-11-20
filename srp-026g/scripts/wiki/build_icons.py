#!/usr/bin/env python3
"""Build the wiki's icons from the lobby (lobby_menu.swf) as the players have it.

    python3 -I build_icons.py --json <config json dir> --images <lobby images dir>
                              --item-icons <calculator icons dir> --swf <lobby_menu.swf> --ffdec <ffdec-cli.jar>
                              --lobby <srp-026g/flash/lobby_menu> --out <servers/web-server/wiki/icons>

damage-calc/build_site.py runs it after the calculator's icons, so every
release makes them again from its own patches. Writes <out>/ anew:

- items/<items_dict name>.webp: the lobby icon of every item of a category
  the shop's equipment and weapon filters name, as the calculator's build cut
  it (assets/tools/build_icons.py, 171x83);
- mods/<ui_desc.icon>.webp: every 38x38 cell of the `modifiers` atlas
  (character 3279, IconContainer("modifiers", 38, 38, vertical): column-major)
  that holds an icon; each icon a modification shows must be one;
- groups/<item_category>.webp: the shop filter button of each of those
  categories (FilterItem: itemIcon.gotoAndStop(300 + icon); the lobby's table
  tables/survarium/data/ShopFilters.tsv, its EQUIPMENT and WEAPONS lists, names
  each filter's icon and category, as the movie has them; FFDec exports the
  frames);
- quests/<ui.icon>.webp: each frame of the quest list's icon, frame n + 1 for
  a quest's `ui.icon` n (QuestListRender: questIcon.gotoAndStop(quest_icon + 1);
  the exe's add_quest 0x28DE20 sends `quest_icon` from the quest's ui.icon,
  which its quest reader 0x321300 keeps at descriptor + 0xB8), a glyph;
- factions/<faction id>.webp: the quest window's faction tab emblem, frame
  `faction_id` of SrvQuestFactionTab's `icon` (ButtonQuestFaction sets its
  data's icon to faction_id; ButtonIconCenter: icon.gotoAndStop(icon)), in
  its colours;
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
import sys
import tempfile
from pathlib import Path

import numpy
from PIL import Image

# The movie build's reader of a lobby table (tools/actionscript/flash_tree.py).
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools" / "actionscript"))
from flash_tree import read_table  # noqa: E402
import swf  # noqa: E402

MOD_ATLAS = "3279_LobbyMenu_ModifierIcons.png"
MOD_CELL = 38
# FilterItemCatRenderer's `itemIcon` (lobby_menu.swf sprite 1125): frame
# 300 + icon draws a filter's icon. FFDec exports its frames as SVG.
FILTER_SPRITE = 1125
FILTER_FRAME = 300
# The quest window's renderers and the child each draws an icon with:
# (class, instance name, folder, side in pixels it is written at).
QUEST_ICONS = (("SrvQuestSideRenderer", "questIcon", "quests", 64),
               ("SrvQuestFactionTab", "icon", "factions", 96))
TYPES = {"webp": "image/webp"}


def load(json_dir, name):
    return json.loads((json_dir / name).read_text(encoding="utf-8"))


def rows(table):
    return table if isinstance(table, list) else list(table.values())


def filters(lobby):
    """{item_category: filter icon} of the shop's equipment and weapon filters (the lobby's ShopFilters)."""
    constants = dict((name, int(value)) for name, value in re.findall(
        r"public static const (\w+):int = (\d+);", (lobby / "src/survarium/constants/ItemCategory.as").read_text()))
    header, rows = read_table(lobby / "tables/survarium/data/ShopFilters.tsv")
    column = {name.split(":")[0]: at for at, name in enumerate(header)}
    found = {}
    for row in rows:
        if row[column["list"]] in ("EQUIPMENT", "WEAPONS"):
            category = row[column["sucat"]]
            if category not in constants:
                raise SystemExit(f"ShopFilters.tsv: {category} is no ItemCategory constant")
            found.setdefault(constants[category], int(row[column["icon"]]))
    if not found:
        raise SystemExit("ShopFilters.tsv: no EQUIPMENT or WEAPONS filters")
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


def renderer_child(svg, name):
    """The character id of the child named `name` on a sprite's first frame (FFDec's SVG)."""
    for use in re.findall(r"<use\b[^>]*>", svg):
        if re.search(rf'\bid="{name}"', use):
            found = re.search(r'characterId="(\d+)"', use)
            if found:
                return int(found.group(1))
    raise SystemExit(f"lobby_menu.swf: no child {name}")


def quest_icons(swf_path, ffdec, tmp):
    """{folder: (side, [each frame's picture])} of QUEST_ICONS, from the movie."""
    _, tags = swf.split(swf_path.read_bytes())
    ids = {name: cid for cid, name in swf.symbol_classes(tags)}
    missing = [cls for cls, *_ in QUEST_ICONS if cls not in ids]
    if missing:
        raise SystemExit(f"lobby_menu.swf: no {', '.join(missing)}")
    subprocess.run(["java", "-jar", str(ffdec), "-format", "sprite:svg", "-selectid",
                    ",".join(str(ids[cls]) for cls, *_ in QUEST_ICONS), "-export", "sprite", str(tmp / "renderers"),
                    str(swf_path)], check=True, capture_output=True)
    sprite_dir = lambda root, cid: next(d for d in root.iterdir() if re.fullmatch(rf"DefineSprite_{cid}(_.*)?", d.name))
    children = {folder: renderer_child((sprite_dir(tmp / "renderers", ids[cls]) / "1.svg").read_text(encoding="utf-8"), name)
                for cls, name, folder, _ in QUEST_ICONS}
    subprocess.run(["java", "-jar", str(ffdec), "-format", "sprite:png", "-zoom", "2", "-selectid",
                    ",".join(map(str, children.values())), "-export", "sprite", str(tmp / "icons"), str(swf_path)],
                   check=True, capture_output=True)
    found = {}
    for _, _, folder, side in QUEST_ICONS:
        frames = sprite_dir(tmp / "icons", children[folder])
        count = len(list(frames.glob("*.png")))
        found[folder] = (side, [Image.open(frames / f"{n}.png").convert("RGBA") for n in range(1, count + 1)])
    return found


def fitted(image, box, side):
    """`image` cut to `box` (its set's pictures' bounds, so they keep their sizes) and scaled
    into a `side` square, centred."""
    image = image.crop(box)
    image.thumbnail((side, side), Image.LANCZOS)
    square = Image.new("RGBA", (side, side))
    square.paste(image, ((side - image.width) // 2, (side - image.height) // 2))
    return square


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    for name in ("json", "images", "item-icons", "swf", "ffdec", "lobby", "out"):
        ap.add_argument(f"--{name}", required=True, type=Path)
    args = ap.parse_args()
    out = args.out
    shutil.rmtree(out, ignore_errors=True)
    files = {}

    def write(path, data):
        (out / path).parent.mkdir(parents=True, exist_ok=True)
        (out / path).write_bytes(data)
        files[path] = TYPES[path.rsplit(".", 1)[1]]

    categories = filters(args.lobby)
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
        for folder, (side, pictures) in quest_icons(args.swf, args.ffdec, Path(tmp)).items():
            boxes = [picture.getbbox() for picture in pictures if picture.getbbox()]
            box = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
            # A quest's icon is frame ui.icon + 1, a faction's frame faction_id.
            for frame, picture in enumerate(pictures, start=1):
                if folder == "quests":
                    write(f"quests/{frame - 1}.webp", webp(glyph(fitted(picture, box, side))))
                else:
                    write(f"factions/{frame}.webp", webp(fitted(picture, box, side)))

    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/wiki/build_icons.py; do not edit. The wiki's icons.\n"
        "files! {\n" + "".join(f'    "{path}" => "{kind}",\n' for path, kind in sorted(files.items())) + "}\n")
    total = sum((out / path).stat().st_size for path in files)
    counts = {kind: sum(path.startswith(kind + "/") for path in files)
              for kind in ("items", "mods", "groups", "quests", "factions")}
    print(f"{out}: {counts['items']} items, {counts['mods']} modification, {counts['groups']} group, "
          f"{counts['quests']} quest and {counts['factions']} faction icons, {total / 1024:.0f} KB")


if __name__ == "__main__":
    main()
