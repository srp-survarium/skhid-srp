#!/usr/bin/env python3
"""A ported map's pictures in the movies built from source: its HUD minimap
(srp-026g/flash/hud) and its loading picture (srp-026g/flash/lobby_menu).

    ui_images.py KEY --minimap MINIMAP.png [--loading PICTURE]

KEY is the map's project_name (the HUD minimap key, e.g. cs_mp_rembasa).
MINIMAP.png is resource-porter's tools/playable-map-pack/render_minimap.py
output (512 x 512 RGBA, the 0.26 HUD's style); its <KEY>.json beside it gives
the frame (size, posX, posY), written into Minimap.as's _minimapProps by hand.
Training Camp's is framed on its crater wall:

    render_minimap.py --map 212 --frame-outline srp-026g/scripts/stalker-maps/training-arena.tsv

PICTURE is cut to the lobby's 1920 x 798 (its middle band, scaled to cover).

Writes hud/bitmaps/minimap_<KEY>.png (the render as it is, a lossless bitmap)
and lobby_menu/bitmaps/Level_<KEY>.jpg (a baseline JPEG), the bitmaps the
build adds bound to the classes of the same name in each movie's src/
(tools/actionscript/flash_tree.py).
"""
import argparse
import json
from pathlib import Path
import shutil

from PIL import Image

FLASH = Path(__file__).resolve().parents[2] / 'flash'
LOADING = (1920, 798)


def cover(image, size):
    w, h = size
    scale = max(w / image.width, h / image.height)
    image = image.resize((round(image.width * scale), round(image.height * scale)), Image.LANCZOS)
    left, top = (image.width - w) // 2, (image.height - h) // 2
    return image.crop((left, top, left + w, top + h))


def bitmap(movie, cls, suffix):
    if not (FLASH / movie / 'src' / f'{cls}.as').exists():
        raise SystemExit(f'{movie}: no class src/{cls}.as to bind the bitmap to')
    return FLASH / movie / 'bitmaps' / f'{cls}{suffix}'


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('key')
    ap.add_argument('--minimap', type=Path, required=True)
    ap.add_argument('--loading', type=Path)
    a = ap.parse_args()
    minimap = Image.open(a.minimap)
    if minimap.mode != 'RGBA' or minimap.size != (512, 512):
        raise SystemExit(f'{a.minimap}: {minimap.mode} {minimap.size}, not 512 x 512 RGBA')
    out = bitmap('hud', f'minimap_{a.key}', '.png')
    shutil.copyfile(a.minimap, out)
    print(f'hud: {out.name}')
    if a.loading:
        out = bitmap('lobby_menu', f'Level_{a.key}', '.jpg')
        cover(Image.open(a.loading).convert('RGB'), LOADING).save(out, 'JPEG', quality=92, subsampling=0)
        print(f'lobby_menu: {out.name}')
    props = json.loads(a.minimap.with_suffix('.json').read_text())
    print(f"Minimap.as: _minimapProps.{a.key} = size {props['size']}, posX {props['posX']}, posY {props['posY']}")


if __name__ == '__main__':
    main()
