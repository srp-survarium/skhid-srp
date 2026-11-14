#!/usr/bin/env python3
"""Write maps.json for the damage-calculator map images.

    make_json.py WORK_DIR OUT_DIR TABLE_RS [--only KEY ...]

TABLE_RS is srp-026g/crates/survarium/src/v026g/maps/table.rs (maps_dict ids
per level; that file is gone since the server derives the table from the game
data, so take it from git history). With --only, OUT_DIR/maps.json is updated
in place: the named maps are written (added or replaced) and the others kept
as they are; the patch-added maps (build_all.PATCHED) need no TABLE_RS. Elevation grids come from WORK_DIR/out/<key>-ground.npy (terrain
model heights) with WORK_DIR/out/<key>-height.npy (top surface) where no
terrain was rendered; of a map with no terrain model at all (the STALKER
ports), from WORK_DIR/out/<key>-solid.npy (the top of what is solid: no
foliage).
"""
import json
import re
import sys
from pathlib import Path

import numpy as np
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_all import NATIVE, PATCHED, PORTED, TARGET  # noqa: E402

GRID = 64
WEATHER = {1: 'day', 2: 'rain', 3: 'evening', 4: 'night', 5: 'prerain'}
MODE = {2: 'battery (gather victory items)', 3: 'battery with charging stations (match_time_vi_spawn)',
        4: 'team deathmatch', 8: 'free for all'}
# Where a mode's configs are under gameplay/game_modes/, when in a folder of their own.
CONFIG_DIR = {8: 'ffa/'}


def elevation(work, key, size):
    """The grid, and whether it is of terrain models."""
    g = np.load(work / 'out' / f'{key}-ground.npy').astype(np.float64)
    h = np.load(work / 'out' / f'{key}-height.npy').astype(np.float64)
    terrain = bool(np.isfinite(g).any())
    if not terrain:
        g = np.load(work / 'out' / f'{key}-solid.npy').astype(np.float64)
    g = np.where(np.isfinite(g), g, h)
    W = g.shape[0]; c = W // GRID
    blocks = g[:c * GRID, :c * GRID].reshape(GRID, c, GRID, c).swapaxes(1, 2).reshape(GRID, GRID, -1)
    import warnings
    with warnings.catch_warnings():
        warnings.simplefilter('ignore')
        med = np.nanmedian(blocks, axis=2)
    if np.isnan(med).any():
        idx = ndimage.distance_transform_edt(np.isnan(med), return_distances=False, return_indices=True)
        med = med[tuple(idx)]
    return [[round(float(v), 1) for v in row] for row in med], terrain


def main():
    work, out, table = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    only = sys.argv[sys.argv.index('--only') + 1:] if '--only' in sys.argv else None
    rows = re.findall(r'MapMode \{ id: (\d+), config: "([^"]+)", name: "([^"]+)", mode: (\d+), level: (\d+), '
                      r'weather: (\d+), project: "([^"]+)", has_project: (true|false)', table.read_text()) \
        if table.is_file() else []
    maps = json.loads((out / 'maps.json').read_text())['maps'] if only else {}
    for key, m in {**NATIVE, **PORTED, **PATCHED}.items():
        if only and key not in only:
            continue
        size = m['size']; W = TARGET; mpp = 2 * size / W
        x0, z1 = m['posX'] - size, m['posY'] + size
        if key in PORTED or key in PATCHED:
            modes = [dict(map_id=i, config=f'{CONFIG_DIR.get(mo, "")}{key}_day_{mo}.options', name=f'{key}_{mo}', mode=mo,
                          mode_name=MODE[mo], weather='day', project=f'{key}/day_{mo}')
                     for i, mo in m['map_ids'].items()]
        else:
            lvl = int(key[-2:])
            modes = [dict(map_id=int(i), config=cfg, name=n, mode=int(mo), mode_name=MODE.get(int(mo), mo),
                          weather=WEATHER.get(int(w), w), project=pr)
                     for i, cfg, n, mo, lv, w, pr, hp in rows if int(lv) == lvl and hp == 'true']
        check = json.loads((out / f'check_{key}.json').read_text()) if (out / f'check_{key}.json').exists() else None
        heights, terrain = elevation(work, key, size)
        maps[key] = {
            'name': m['name'],
            'name_key': m.get('name_key') or f'st_level_{key[-2:]}_name',
            'origin': ('ported: Skhid content pack maps-021d-7 (released in revision 20), geometry from the '
                       'pack overlay + the 0.26g db' if key in PORTED else
                       f"skhid patch: {m['origin']}, geometry from the 0.26g db with the released patches over it"
                       if key in PATCHED else 'native 0.26g'),
            'image': f'{key}.webp',
            'image_size_px': [W, W],
            'metres_per_pixel': round(mpp, 6),
            'pixels_per_metre': round(1 / mpp, 6),
            'world_bounds': {'x_min': round(x0, 4), 'x_max': round(m['posX'] + size, 4), 'z_min': round(m['posY'] - size, 4), 'z_max': round(z1, 4)},
            'orientation': 'image right = +x, image up = +z (top edge is z_max); same as the in-game HUD minimap',
            'pixel_to_world_affine': [round(mpp, 9), 0.0, round(x0, 4), 0.0, round(-mpp, 9), round(z1, 4)],
            'world_to_pixel_affine': [round(1 / mpp, 9), 0.0, round(-x0 / mpp, 6), 0.0, round(-1 / mpp, 9),
                                      round(z1 / mpp, 6)],
            'hud_minimap_props': {'size': size, 'posX': m['posX'], 'posY': m['posY']},
            'project_name': m.get('project_name', key),
            'source_project': f"projects/{key}/{m.get('project', 'day_2')}/client_project",
            'map_ids': sorted(r['map_id'] for r in modes),
            'modes': modes,
            'elevation': {
                'grid': GRID,
                'cell_m': round(2 * size / GRID, 4),
                'note': 'heights[row][col] in metres (engine y), row 0 = north edge (z_max), col 0 = west '
                        '(x_min); cell centre = bounds origin + (col+0.5, row+0.5) * cell_m. '
                        + ('Median terrain height per cell (terrain models; top surface where no terrain). '
                           'Not floor heights inside multi-storey buildings.' if terrain else
                           'Median height per cell of the top solid surface, without foliage (the map has no '
                           'terrain model): a roof counts as ground.'),
                'heights': heights,
            },
        }
        ver = {}
        if check:
            ver['object_overlay'] = {
                'image': f'check_{key}.png',
                'objects_plotted': check['points'], 'outside_image': check['outside'],
                'height_test': check.get('height_test'),
                'control_mirrored_z': check.get('control_mirrored_z'),
                'control_shift_5m': check.get('control_shift_5m'),
                'meaning': 'respawn points / battery containers / battery spawners / charging stations of every '
                           'mode project; floating = object stands > 0.5 m above all rendered geometry at its '
                           'pixel (impossible if the transform is right); the controls run the same test with a '
                           'deliberately wrong transform'}
        al = work / 'out' / f'align_{key}.json'
        if al.exists():
            ver['native_minimap_alignment'] = json.loads(al.read_text())
        ms = work / 'out' / f'measure_{key}.json'
        if ms.exists():
            ver['building_measurement'] = json.loads(ms.read_text())
        if ver:
            maps[key]['verification'] = ver
    doc = {
        'format': 'srp damage-calculator maps v1',
        'coordinates': 'Engine world metres. Horizontal plane is x/z, y is up. Distance = hypot(dx, dz) '
                       '(add dy from elevation for slant distance).',
        'pixel_convention': 'px, py are continuous image coordinates in pixels of image_size_px with (0,0) at the '
                            'top-left corner of the image (pixel centres at +0.5). x = a*px + b*py + c, '
                            'z = d*px + e*py + f for pixel_to_world_affine [a,b,c,d,e,f]; world_to_pixel_affine '
                            'is the inverse in the same form (px = a*x + b*z + c, py = d*x + e*z + f). If the '
                            'image is shown scaled, scale px, py by image_size_px / displayed size first.',
        'derivation': 'Each image is an orthographic top-down render of the map geometry (LOD0 render models of '
                      'the day_2 client_project, from the pristine 0.26g resources.db) into exactly the square '
                      'frame of the native 0.26g HUD minimap: hud.swf (build 2777) survarium.controls.Minimap '
                      '_minimapProps {size, posX, posY} covers 2*size metres around (posX, posY), drawn with '
                      'pixel = ((x - posX + size), (posY - z + size)) * width / (2*size) (Minimap.setProps / '
                      'updatePlayerPos, the engine passes (x, -z)). Because the render rasterizes world '
                      'coordinates directly, the scale is exact by construction; the native minimap bitmap '
                      '(drawn by the developers for the same frame) lines up with the rendered buildings, and '
                      'levelmap_min/max in level_01/02/04/05 client_projects equal the same frames.',
        'maps': maps,
    }
    text = json.dumps(doc, indent=1)
    # keep number rows (elevation) on one line each
    text = re.sub(r'\[\s*(-?[0-9.]+(?:,\s*-?[0-9.]+)*)\s*\]', lambda mm: '[' + re.sub(r'\s+', '', mm.group(1)).replace(',', ', ') + ']', text)
    (out / 'maps.json').write_text(text + '\n')
    print(out / 'maps.json', (out / 'maps.json').stat().st_size)


if __name__ == '__main__':
    main()
