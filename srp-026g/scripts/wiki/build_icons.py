#!/usr/bin/env python3
"""Build the wiki's icons from the lobby (lobby_menu.swf) as the players have it.

    python3 -I build_icons.py --json <config json dir> --images <lobby images dir>
                              --item-icons <calculator icons dir> --swf <lobby_menu.swf> --ffdec <ffdec-cli.jar>
                              --lobby <srp-026g/flash/lobby_menu> --out <servers/web-server/wiki/icons>

damage-calc/build_site.py runs it after the calculator's icons, so every
release makes them again from its own patches. Writes <out>/ anew:

- items/<items_dict name>.webp: the lobby icon of every item of a category
  the shop's equipment and weapon filters name, and of every artefact
  (`gameplay/items/artefacts/`), as the calculator's build cut it
  (assets/tools/build_icons.py, 171x83);
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
- maps/<level>_<weather>.webp: the picture the rooms draw a map with, small (128x72): the
  library bitmap the lobby's table tables/survarium/rooms/MapRenders.tsv names for the level
  (match_making_params.map_id) and weather (map_weather_id), by the rule of the rooms' own
  RoomsLook.renderClass, which reads the same table; one for every level and weather that has a
  picture, so the site's match lists and the rooms cannot part;
- modes/<frame>.webp: each frame of the loading screen's mode icon, SrvGameModeIcons (a config's
  `ui.icon` n shows frame n + 1, LobbyGameLoad.setMode): the movie's own frames as FFDec exports
  them, srp's (SrvGameModeIcons.as: a frame constant and the bitmap class `art` gives it) from the
  lobby's bitmaps/<class>.png, the ones the movie is built with; and modes/twist-<lobby mode>.webp,
  the frame SrvGameModeIcons.forTwist shows for a free-for-all's twist (its lobby mode number),
  in their colours;
- tiles/: the pictures of the wiki index's tiles that the sets above have too small for a tile
  (about 196x116 CSS pixels): tiles/maps.webp and tiles/maps@2x.webp, level 1's day picture
  (the Maps tile's, as maps/1_1) covering 220x124 and 440x248 from the 1920-wide bitmap; and
  tiles/quests.webp, the four factions' emblems side by side as the lobby's factionIconsBig has
  them (66x66 cells, IconContainer("factionIconsBig", 66, 66, ..., 2, 2): the middle column
  is their normal state), at their own size: the quest window's faction tab draws a 38x51
  bitmap, which no tile could show sharp;
- camos/<row>_<column>.webp and decals/<row>_<column>.webp: each cell of the camouflage (72x72) and
  decal (82x82) atlases that a camouflage's or decal's config (item_category 25, 26) names by its
  `ui_desc.icon` [row, column], from 1: the lobby's own cut (ItemFactory.createBaseItem:
  IconContainer("camo", 72, 72, ...) and ("decals", 82, 82, ...), iconId = (column - 1) * rows +
  row - 1, column-major); one file a cell, so items that share a cell share it (the 1,000 number
  decals' one icon), lossy (`photo`); a named cell with nothing drawn in it stops the build;
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
# The artefacts' configs (the wiki's artefacts page, src/artefacts.rs).
ARTEFACTS = "gameplay/items/artefacts/"
# FilterItemCatRenderer's `itemIcon` (lobby_menu.swf sprite 1125): frame
# 300 + icon draws a filter's icon. FFDec exports its frames as SVG.
FILTER_SPRITE = 1125
FILTER_FRAME = 300
# The quest window's renderers and the child each draws an icon with:
# (class, instance name, folder, side in pixels it is written at).
QUEST_ICONS = (("SrvQuestSideRenderer", "questIcon", "quests", 64),
               ("SrvQuestFactionTab", "icon", "factions", 96))
TYPES = {"webp": "image/webp"}
# The cosmetics' atlases (LobbyMenu.as: IconContainer("camo", 72, 72, ...), ("decals", 82, 82, ...)),
# by item_category: (folder, the bitmap's class in FFDec's export `<id>_<class>.png`, cell side).
COSMETICS = {25: ("camos", "LobbyMenu_CamoIcons", 72), 26: ("decals", "LobbyMenu_DecalsIcons", 82)}


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


def photo(image):
    """A camouflage's or decal's icon: cloth and print, lossy (a third of the lossless size, the
    same at 72 pixels), its alpha whole."""
    data = io.BytesIO()
    image.save(data, "WEBP", quality=90, alpha_quality=100, method=6)
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


# The loading screen's mode icon (LobbyGameLoad: modeIcon.gotoAndStop(ui.icon + 1)), and the side
# in pixels each frame is written at.
MODE_ICONS = "SrvGameModeIcons"
MODE_SIDE = 64


def mode_icons(swf_path, ffdec, lobby, tmp):
    """({frame: picture}, {twist lobby mode: frame}) of SrvGameModeIcons: the movie's own frames
    (FFDec's export of the sprite; srp's frames show frame 1 with its art hidden) and srp's, each
    the bitmap class its `art` gives for its constant, from the lobby's bitmaps; the twists' frames
    by forTwist's cases (`case 9: return RACKET - 1;`: lobby mode 9 shows frame RACKET)."""
    source = (lobby / "src" / f"{MODE_ICONS}.as").read_text(encoding="utf-8")
    frames = {name: int(value) for name, value in re.findall(r"public static const (\w+):int = (\d+);", source)}
    own = {frames[name]: cls for name, cls in re.findall(r"case (\w+):\s*return new (\w+)\(\);", source)}
    body = source.split("function forTwist", 1)[1].split("\n      }\n", 1)[0]
    twists, cases = {}, []
    for case, returned in re.findall(r"case (\d+):|return (\w+) - 1;", body):
        if case:
            cases.append(int(case))
        elif returned:
            twists.update((number, frames[returned]) for number in cases)
            cases = []
    if not own or not twists:
        raise SystemExit(f"{MODE_ICONS}.as: no frames of srp's or no twists read")
    _, tags = swf.split(swf_path.read_bytes())
    ids = {name: cid for cid, name in swf.symbol_classes(tags)}
    if MODE_ICONS not in ids:
        raise SystemExit(f"lobby_menu.swf: no {MODE_ICONS}")
    subprocess.run(["java", "-jar", str(ffdec), "-format", "sprite:png", "-zoom", "2", "-selectid", str(ids[MODE_ICONS]),
                    "-export", "sprite", str(tmp / "modes"), str(swf_path)], check=True, capture_output=True)
    exported = next(d for d in (tmp / "modes").iterdir() if re.fullmatch(rf"DefineSprite_{ids[MODE_ICONS]}(_.*)?", d.name))
    pictures = {int(path.stem): Image.open(path).convert("RGBA") for path in exported.glob("*.png")}
    for frame, cls in own.items():
        pictures[frame] = Image.open(lobby / "bitmaps" / f"{cls}.png").convert("RGBA")
    missing = sorted(set(twists.values()) - set(pictures))
    if missing:
        raise SystemExit(f"{MODE_ICONS}: forTwist shows frames {missing}, which it has not")
    return pictures, twists


def mode_square(image):
    """`image` cut to what it draws and scaled into a MODE_SIDE square, centred."""
    return fitted(image, image.getbbox() or (0, 0, image.width, image.height), MODE_SIDE)


# A map's picture in a match list: 16:9 as the rooms' cards, twice the 64x36 it is shown at.
MAP_SIZE = (128, 72)
# Levels are match_making_params.map_id, a byte's worth; none is above this.
MAP_LEVELS = range(1, 128)


def map_pictures(lobby, images):
    """{(level, weather): picture} by the rooms' rule (RoomsLook.renderClass) over the lobby's
    table MapRenders.tsv: ONE's picture whatever the weather; else LIKE's name, or `Level<level>`,
    with the weather's word, its day one for a weather it has none of. `images`: FFDec's export of
    the movie's bitmaps (`<character id>_<class name>.<type>`); a level with no picture is not
    there."""
    _, table = read_table(lobby / "tables" / "survarium" / "rooms" / "MapRenders.tsv")
    lists = {}
    for name, key, value in table:
        lists.setdefault(name, {})[int(key)] = value
    weathers, like, one = lists["WEATHERS"], lists.get("LIKE", {}), lists.get("ONE", {})
    exported = {}
    for path in images.iterdir():
        match = re.fullmatch(r"\d+_(\w+)\.(?:png|jpg|jpeg)", path.name)
        if match:
            # The original movie's bitmaps are exported under their name twice (`Level1_day_Level1_day`).
            name = match.group(1)
            half = name[:len(name) // 2]
            exported[half if name == f"{half}_{half}" else name] = path
    found = {}
    for level in MAP_LEVELS:
        for weather, word in weathers.items():
            if level in one:
                names = [one[level]]
            else:
                base = like.get(level, f"Level{level}")
                names = [f"{base}_{word}", f"{base}_day"]
            picture = next((exported[name] for name in names if name in exported), None)
            if picture is not None:
                found[level, weather] = picture
    missing = sorted(name for name in one.values() if name not in exported)
    if missing:
        raise SystemExit(f"MapRenders.tsv names pictures the movie has not: {', '.join(missing)}")
    return found


# The wiki index's tiles (servers/web-server/src/wiki.rs `sections`).
TILE_MAP = (1, 1)
TILE_MAP_SIZE = (220, 124)
FACTION_ATLAS = "3274_LobbyMenu_FactionIconsBig.png"
FACTION_CELL, FACTION_STEP, FACTION_COLUMN = 66, 68, 1
TILE_FACTIONS = range(1, 5)
TILE_GAP = 8


def faction_row(atlas):
    """The factions' emblems (rows TILE_FACTIONS of the atlas, its FACTION_COLUMN) in a row."""
    row = Image.new("RGBA", (len(TILE_FACTIONS) * (FACTION_CELL + TILE_GAP) - TILE_GAP, FACTION_CELL))
    for at, faction in enumerate(TILE_FACTIONS):
        left, top = FACTION_COLUMN * FACTION_STEP, faction * FACTION_STEP
        row.paste(atlas.crop((left, top, left + FACTION_CELL, top + FACTION_CELL)), (at * (FACTION_CELL + TILE_GAP), 0))
    return row


def covered(image, size):
    """`image` scaled to cover `size` and cut to it about its centre, as the rooms' cards are
    (RoomsLook.cover)."""
    scale = max(size[0] / image.width, size[1] / image.height)
    image = image.resize((max(size[0], round(image.width * scale)), max(size[1], round(image.height * scale))), Image.LANCZOS)
    left, top = (image.width - size[0]) // 2, (image.height - size[1]) // 2
    return image.crop((left, top, left + size[0], top + size[1]))


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
        if item["item_category"] not in categories and not item["cfg_name"].startswith(ARTEFACTS):
            continue
        stem = item["cfg_name"].rsplit("/", 1)[-1].removesuffix(".options")
        source = args.item_icons / f"{item['name'] if len(stems[stem]) > 1 else stem}.webp"
        if source.is_file():
            write(f"items/{item['name']}.webp", source.read_bytes())
        else:
            missing.append(item["name"])
    if missing:
        raise SystemExit(f"no lobby icon for {', '.join(missing)}")

    # The camouflages' and decals' cells, each once.
    for category, (folder, cls, side) in COSMETICS.items():
        found = [path for path in args.images.iterdir() if re.fullmatch(rf"\d+_{cls}\.png", path.name)]
        if len(found) != 1:
            raise SystemExit(f"{args.images}: {len(found)} exported bitmaps of {cls}, not one")
        atlas = Image.open(found[0]).convert("RGBA")
        rows_, columns = atlas.height // side, atlas.width // side
        cells = {}
        for item in dictionary:
            if item["item_category"] != category:
                continue
            config = load(args.json, item["cfg_name"].replace("/", "__") + ".json")
            icon = config.get("ui_desc", {}).get("icon")
            if not (isinstance(icon, list) and len(icon) == 2 and all(isinstance(n, int) for n in icon)):
                raise SystemExit(f"{item['name']}: ui_desc.icon is {icon!r}, no [row, column]")
            cells.setdefault(tuple(icon), item["name"])
        for (row, column), name in sorted(cells.items()):
            if not (1 <= row <= rows_ and 1 <= column <= columns):
                raise SystemExit(f"{name}: icon [{row}, {column}] is outside {cls}'s {rows_}x{columns} cells")
            left, top = (column - 1) * side, (row - 1) * side
            cell = atlas.crop((left, top, left + side, top + side))
            if cell.getchannel("A").getbbox() is None:
                raise SystemExit(f"{name}: icon [{row}, {column}] of {cls} is empty")
            write(f"{folder}/{row}_{column}.webp", photo(cell))

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
        pictures, twists = mode_icons(args.swf, args.ffdec, args.lobby, Path(tmp))
        for frame, picture in sorted(pictures.items()):
            write(f"modes/{frame}.webp", webp(mode_square(picture)))
        for number, frame in sorted(twists.items()):
            write(f"modes/twist-{number}.webp", webp(mode_square(pictures[frame])))

    small = {}
    pictures = map_pictures(args.lobby, args.images)
    for (level, weather), picture in sorted(pictures.items()):
        if picture not in small:
            data = io.BytesIO()
            covered(Image.open(picture).convert("RGB"), MAP_SIZE).save(data, "WEBP", quality=82, method=6)
            small[picture] = data.getvalue()
        write(f"maps/{level}_{weather}.webp", small[picture])

    whole = Image.open(pictures[TILE_MAP]).convert("RGB")
    for name, scale in (("maps", 1), ("maps@2x", 2)):
        data = io.BytesIO()
        covered(whole, (TILE_MAP_SIZE[0] * scale, TILE_MAP_SIZE[1] * scale)).save(data, "WEBP", quality=85, method=6)
        write(f"tiles/{name}.webp", data.getvalue())
    write("tiles/quests.webp", webp(faction_row(Image.open(args.images / FACTION_ATLAS).convert("RGBA"))))

    (out / "files.rs").write_text(
        "// Built by srp-026g/scripts/wiki/build_icons.py; do not edit. The wiki's icons.\n"
        "files! {\n" + "".join(f'    "{path}" => "{kind}",\n' for path, kind in sorted(files.items())) + "}\n")
    total = sum((out / path).stat().st_size for path in files)
    counts = {kind: sum(path.startswith(kind + "/") for path in files)
              for kind in ("items", "mods", "groups", "quests", "factions", "maps", "modes", "camos", "decals")}
    print(f"{out}: {counts['items']} items, {counts['mods']} modification, {counts['groups']} group, "
          f"{counts['quests']} quest and {counts['factions']} faction icons, {counts['maps']} map pictures, {counts['modes']} mode icons, {counts['camos']} camouflage and {counts['decals']} decal icons, "
          f"{total / 1024:.0f} KB")


if __name__ == "__main__":
    main()
