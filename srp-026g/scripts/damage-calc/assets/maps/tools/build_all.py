#!/usr/bin/env python3
"""Build the damage-calculator map set: render, shade, check, webp, maps.json.

    build_all.py WORK_DIR OUT_DIR [--only KEY ...] [--rerender]

WORK_DIR holds extract_level.py output (lv/<key>/), the decoded HUD images
(ex/hud/images, given by --hud-images) and the raw client_projects (db/).
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

HERE = Path(__file__).resolve().parent
PY = sys.executable
TARGET = 1536            # image side in px

# Native 0.26g HUD minimap frames: hud.swf (build 2777) survarium.controls.Minimap
# _minimapProps {size, posX, posY} and the _levels bitmap per project_name.
NATIVE = {
    'level_01': dict(size=130, posX=12.558, posY=-25.774, bitmap='967_minimap_rudnya_minimap_rudnya.png', name='Rudnya'),
    'level_02': dict(size=110, posX=21.079, posY=105.098, bitmap='499_minimap_school_minimap_school.png', name='School'),
    'level_03': dict(size=100, posX=14, posY=-42.196, bitmap='976_minimap_base_minimap_base.png', name='Vostok Radar Station'),
    'level_04': dict(size=115, posX=15, posY=43, bitmap='911_minimap_zavod_minimap_zavod.png', name='Chemical Plant'),
    'level_05': dict(size=90, posX=-1.096, posY=-14.211, bitmap='955_minimap_fort_minimap_fort.png', name='Tarakanovsky Fort'),
    'level_06': dict(size=100, posX=30, posY=109, bitmap='975_Map_06_Map_06.png', name='Mamayev Kurgan'),
    'level_07': dict(size=100, posX=0, posY=0, bitmap='974_Map_07_Map_07.png', name='Vector Laboratory'),
    'level_08': dict(size=92, posX=0, posY=35, bitmap='972_Map_08_Map_08.png', name='Cologne'),
}

# Live ported maps (Skhid revision 20, pack maps-021d-7). Their project_name is
# level_03 / level_06, so the in-game HUD uses the native level_03 / level_06
# frame; the renders use the same frames (no native play-area outline: the
# native bitmaps show the 0.26g layout, not 0.21d's).
PACK = Path('/mnt/hdd/Projects/release-20/packs/maps-021d-7/files')
PORTED = {
    'v021d_level_03': dict(size=100, posX=14, posY=-42.196, bitmap=None, name='Vostok Radar Station (v0.21)',
                           name_key='st_level_10_name', project_name='level_03', map_ids={166: 2, 152: 4}),
    'v021d_level_06': dict(size=100, posX=30, posY=109, bitmap=None, name='Mamayev Kurgan (v0.21)',
                           name_key='st_level_11_name', project_name='level_06', map_ids={175: 2, 153: 4}),
}


# Live maps a released skhid patch added, with their own minimap in the
# patch-22 hud.swf (Minimap _minimapProps / _levels by project_name): London
# (patch 14) and the 0.11e ports (patch 22, modes 2 and 4 only). Their inputs
# are extracted with extract_level.py --patch, their raw client_projects (as
# the game overlays them) put in WORK_DIR/db like the stock ones.
PATCHED = {
    'level_09': dict(size=140, posX=0, posY=-5, bitmap='971_Map_09_Map_09.png', area='alpha:0.4', name='London', origin='London, patch 14',
                     name_key='st_level_09_name', modes=(2, 3, 4), map_ids={181: 2, 182: 3, 183: 4}),
    'v011e_level_03': dict(size=100, posX=14.23, posY=-42.196, bitmap='985_minimap_bv11.png', area='line', origin='0.11e port, patch 22',
                           name='Vostok Radar Station (v0.11)', name_key='st_level_13_name', modes=(2, 4),
                           map_ids={208: 2, 209: 4}),
    'v011e_level_04': dict(size=140, posX=23.302, posY=43.049, bitmap='984_minimap_zv11e.png', area='line', origin='0.11e port, patch 22',
                           name='Chemical Plant (v0.11)', name_key='st_level_12_name', modes=(2, 4),
                           map_ids={206: 2, 207: 4}),
    # 0.69d's Rudnya (pack rudnya-069d, patch 40): project_name level_01, so the HUD
    # draws native Rudnya's minimap in its frame; the layout is the same.
    'v069d_level_01': dict(size=130, posX=12.558, posY=-25.774, bitmap='967_minimap_rudnya_minimap_rudnya.png',
                           origin='0.69d port, pack rudnya-069d@3 (patch 40)', name='Rudnya (v0.69)',
                           name_key='st_level_14_name', project_name='level_01', modes=(2, 3, 4),
                           map_ids={216: 2, 217: 3, 204: 4}),
}


def project_files(work, key):
    if key in PORTED:
        return [PACK / 'projects' / key / f'day_{n}' / 'client_project' for n in (2, 4)]
    modes = PATCHED[key]['modes'] if key in PATCHED else (2, 3, 4)
    return [work / 'db' / f'projects__{key}__day_{n}__client_project' for n in modes]


def run(*args):
    r = subprocess.run([PY, '-I', *map(str, args)], check=True, capture_output=True, text=True)
    return r.stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('work', type=Path); ap.add_argument('out', type=Path)
    ap.add_argument('--hud-images', type=Path, required=True)
    ap.add_argument('--only', nargs='*'); ap.add_argument('--rerender', action='store_true')
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    rd = a.work / 'out'
    for key, m in {**NATIVE, **PORTED, **PATCHED}.items():
        if a.only and key not in a.only:
            continue
        size = m['size']; ppm = TARGET / (2 * size)
        if a.rerender or not (rd / f'{key}-albedo.npy').exists():
            print(run(HERE / 'render_topdown.py', a.work / 'lv' / key, key, size, m['posX'], m['posY'], ppm, rd), flush=True)
        png = rd / f'{key}.png'
        native = a.hud_images / m['bitmap'] if m['bitmap'] else 'none'
        print(run(HERE / 'shade.py', rd, key, native, png, '--ppm', ppm, '--area', m.get('area', 'alpha')), flush=True)
        projects = project_files(a.work, key)
        print(run(HERE / 'check_overlay.py', png, key, size, m['posX'], m['posY'], a.out / f'check_{key}.png',
                  *projects, f'--height={rd / (key + "-height.npy")}'), flush=True)
        im = Image.open(png).filter(ImageFilter.GaussianBlur(0.5))
        for q in (80, 75, 70, 65, 60, 55, 50):
            im.save(a.out / f'{key}.webp', 'WEBP', quality=q, method=6)
            if (a.out / f'{key}.webp').stat().st_size <= 400_000:
                break
        print(key, 'webp q', q, (a.out / f'{key}.webp').stat().st_size, flush=True)


if __name__ == '__main__':
    main()
