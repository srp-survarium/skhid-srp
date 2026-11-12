#!/usr/bin/env python3
"""A ported map's pictures in the movies built from source: its HUD minimap
(srp-026g/flash/hud) and its loading picture (srp-026g/flash/lobby_menu).

    ui_images.py KEY --minimap MINIMAP.png --loading PICTURE

KEY is the map's project_name (the HUD minimap key, e.g. cs_mp_rembasa).
MINIMAP.png is resource-porter's tools/playable-map-pack/render_minimap.py
output (512 x 512 RGBA, the 0.26 HUD's style); its <KEY>.json beside it gives
the frame (size, posX, posY), written into Minimap.as by hand once. PICTURE
is cut to the lobby's 1920 x 798 (its middle band, scaled to cover).

Writes images/minimap_<KEY>.{jpg,alpha} and images/Level_<KEY>.{jpg,alpha}
(colour premultiplied by alpha as JPEG, alpha as one zlib byte a pixel:
flash_tree.py's DefineBitsJPEG3, as the game's own minimaps) and names them in each movie.json's "images".
"""
import argparse
import json
from pathlib import Path
import zlib

from PIL import Image

FLASH = Path(__file__).resolve().parents[2] / 'flash'
LOADING = (1920, 798)


def write(movie, cls, image):
    folder = FLASH / movie / 'images'
    folder.mkdir(exist_ok=True)
    alpha = image.getchannel('A') if image.mode == 'RGBA' else Image.new('L', image.size, 255)
    # Premultiplied, baseline, no chroma subsampling: as the game's own minimaps.
    rgb = Image.composite(image.convert('RGB'), Image.new('RGB', image.size), alpha) if image.mode == 'RGBA' else image.convert('RGB')
    rgb.save(folder / f'{cls}.jpg', 'JPEG', quality=92, subsampling=0)
    (folder / f'{cls}.alpha').write_bytes(zlib.compress(alpha.tobytes(), 9))
    spec_path = FLASH / movie / 'movie.json'
    spec = json.loads(spec_path.read_text())
    spec.setdefault('images', {})[cls] = f'images/{cls}'
    spec['images'] = dict(sorted(spec['images'].items()))
    spec_path.write_text(json.dumps(spec, indent=2) + '\n')
    print(f'{movie}: {cls} {image.size}')


def cover(image, size):
    w, h = size
    scale = max(w / image.width, h / image.height)
    image = image.resize((round(image.width * scale), round(image.height * scale)), Image.LANCZOS)
    left, top = (image.width - w) // 2, (image.height - h) // 2
    return image.crop((left, top, left + w, top + h))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('key')
    ap.add_argument('--minimap', type=Path, required=True)
    ap.add_argument('--loading', type=Path, required=True)
    a = ap.parse_args()
    minimap = Image.open(a.minimap).convert('RGBA')
    if minimap.size != (512, 512):
        raise SystemExit(f'{a.minimap}: {minimap.size}, not 512 x 512')
    write('hud', f'minimap_{a.key}', minimap)
    write('lobby_menu', f'Level_{a.key}', cover(Image.open(a.loading).convert('RGB'), LOADING))
    props = json.loads(a.minimap.with_suffix('.json').read_text())
    print(f"Minimap.as: _minimapProps.{a.key} = size {props['size']}, posX {props['posX']}, posY {props['posY']}")


if __name__ == '__main__':
    main()
