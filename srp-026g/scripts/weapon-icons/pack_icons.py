#!/usr/bin/env python3
"""Version the three weapon packs with the checked side-profile icons only.

usage: pack_icons.py --packs <input packs> --icons <render_icons.py output> --out <pack folder>
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil
from PIL import Image

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--packs', type=Path, required=True)
    ap.add_argument('--icons', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    records = json.loads((args.icons / 'rendered.json').read_text())
    assert len(records) == 20 and len({r['name'] for r in records}) == 20
    with (HERE / 'sources.tsv').open() as f:
        sources = list(csv.DictReader(f, delimiter='\t'))
    for source in sources:
        src = args.packs / f"{source['id']}-{source['from']}"
        dst = args.out / f"{source['id']}-{source['to']}"
        assert digest(src/'pack.json') == source['sha256'], f'{src}: unexpected pack'
        assert not dst.exists(), f'{dst}: refusing to replace a pack'
        pack = json.loads((src/'pack.json').read_text())
        replacements = [r for r in records if r['pack'] == source['id']]
        atlas = next(o for o in pack['operations'] if o['op']=='atlas_icons_add' and o['symbol']=='LobbyMenu_WeaponIcons')
        by_cell = {(i['row'],i['col']): i['source'] for i in atlas['icons']}
        assert {tuple(r['cell']) for r in replacements} == set(by_cell)
        for row in replacements:
            png = args.icons / (row['name']+'.png')
            assert digest(png) == row['sha256'], f'{png}: changed since render'
            with Image.open(png) as im:
                assert im.size == (171,83) and im.mode == 'RGBA'
        shutil.copytree(src,dst)
        for row in replacements:
            shutil.copyfile(args.icons/(row['name']+'.png'),dst/by_cell[tuple(row['cell'])])
        pack['version'] = source['to']
        pack['description'] += '; perpendicular lobby icons matched to retail 0.26 references'
        pack['notes'].append('Lobby weapon icons: scripts/weapon-icons/render_icons.py; orthographic profile, stock 0.26 framing and lighting references. No other content changes.')
        pack['notes'].extend(f"icon sha256 {r['sha256']} {by_cell[tuple(r['cell'])]}" for r in replacements)
        (dst/'pack.json').write_text(json.dumps(pack,indent=2)+'\n')
        changed = {str(p.relative_to(dst)) for p in dst.rglob('*') if p.is_file() and digest(p)!=digest(src/p.relative_to(dst))}
        assert changed == {'pack.json'} | set(by_cell.values()), changed
        print(f"{source['id']}@{source['to']}\t{digest(dst/'pack.json')}\t{len(replacements)} icons; other files identical")


if __name__ == '__main__':
    main()
