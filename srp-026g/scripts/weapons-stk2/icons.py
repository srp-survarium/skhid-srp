#!/usr/bin/env python3
"""The STK2 AK-74s' lobby icons: right-side profiles of their models as the other ported guns'
(../weapon-icons/README.md), their brightness the live AK-74M's retail icon's (cell [10, 2]):
the same gun's finish, darkened STK2 metal and polymer.

    python3 -I icons.py --models <tree> --lobby-swf <lobby_menu.swf> --out <folder> [--large]

<tree>: files at their game paths with the guns' models, materials and textures (donor.py's
tree, or the port's bundle overlay). Writes <name>.png (171x83, the porter manifest's
`lobby_icon.file`) per gun of guns.json, and with --large a 684x332 inspection render.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'weapon-icons'))
import render_icons  # noqa: E402
import renderer  # noqa: E402

AK_74M_CELL = (10, 2)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--models', type=Path, required=True)
    ap.add_argument('--lobby-swf', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--large', action='store_true')
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    _, tags = renderer.swf_tags(args.lobby_swf.read_bytes())
    _, _, argb = renderer.atlas(tags)
    alpha = argb[:, :, :1].astype(float) / 255
    rgba = np.dstack([argb[:, :, 1:] / np.maximum(alpha, 1e-9), argb[:, :, 0]])
    r, c = AK_74M_CELL
    reference = Image.fromarray(np.clip(np.round(rgba[(r - 1) * 91:(r - 1) * 91 + 83, (c - 1) * 171:c * 171]),
                                        0, 255).astype(np.uint8), 'RGBA')
    for name, spec in json.loads((HERE / 'guns.json').read_text())['guns'].items():
        parts = renderer.rifle_parts(args.models, f'models/{spec["model"]}.skinned_model')
        canvas, exposure, target, actual = render_icons.matched_icon(parts, (171, 73), reference)
        canvas.save(args.out / f'{name}.png')
        print(f'{name}: exposure {exposure:.4f}, luma {actual:.2f} (the AK-74M icon {target:.2f})')
        if args.large:
            renderer.render(parts, renderer.RIGHT, (684, 332), exposure=exposure,
                            super_sample=2).save(args.out / f'{name}-large.png')


if __name__ == '__main__':
    main()
