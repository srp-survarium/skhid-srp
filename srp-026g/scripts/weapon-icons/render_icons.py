#!/usr/bin/env python3
"""Render the twenty revision-43 weapons in true orthographic right-side profile.

Requires numpy and Pillow. Inputs are the immutable content packs listed in
sources.tsv and a native lobby SWF for style reference. Writes transparent
171x83 icons and a before/after contact sheet. pack_icons.py makes new pack
versions containing exactly these icon replacements.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import renderer
import magazine_pose
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit


def sha(data):
    return hashlib.sha256(data).hexdigest()


LABELS = dict(ots14='Groza', scar_h='SCAR-H', vhs_2='VHS-2', fn_minimi='FN Minimi',
              mp5a1='MP5', svu_a='SVU-A', l85a2='L85A2', fort_401='Fort-401',
              jackhammer='Jackhammer', cz_75='CZ 75', mp443='MP-443',
              beretta93r='Beretta 93R', glock_17='Glock 17', colt_m1911='Colt M1911',
              akmsu='AKMSU', famas='FAMAS', g3sg1='G3SG/1', mauser_m712='Mauser M712',
              ks_23m='KS-23M', ppsh_drum='PPSh-41 Drum')


def matched_icon(parts, size, reference):
    """Keep this weapon's donor brightness, including its material finish.

    A shared rifle/pistol mean makes tan paint and pale metal as dark as
    black weapons. Match the same weapon instead, on opaque pixels only;
    retain the retail-style camera, light rig, framing and shadow.
    Measure the finished icon because highlights clip and a shadow changes
    edge coverage: exposure is not an exact power-law after rasterization.
    """
    target = renderer.mean_luma(np.asarray(reference))
    exposure = 1.0
    for _ in range(8):
        icon = renderer.render(parts, renderer.RIGHT, size, exposure=exposure)
        canvas = Image.new('RGBA', (171,83))
        canvas.paste(icon, ((171-size[0])//2, (79-size[1])//2))
        canvas = renderer.with_shadow(canvas)
        actual = renderer.mean_luma(np.asarray(canvas))
        if abs(actual-target) <= 1.0:
            return canvas, exposure, target, actual
        exposure *= (target / max(actual, 1e-6)) ** 2.2
    raise ValueError(f'Exposure did not converge: target {target:.2f}, rendered {actual:.2f}')


def comparison(rows, rgba, out):
    sheet = Image.new('RGB', (760, 10*112+166), (32,35,38))
    draw = ImageDraw.Draw(sheet)
    draw.text((12,8),'RETAIL 0.26 REFERENCES (original icons)',fill='white')
    for i,(name,r,c) in enumerate((('AK-12',3,3),('AK-74M',10,2),('A-545',16,2),('PM',17,1))):
        draw.text((12+i*185,28),name,fill='white')
        pixels=rgba[(r-1)*91:(r-1)*91+83,(c-1)*171:c*171]
        icon=Image.fromarray(np.clip(np.round(pixels),0,255).astype(np.uint8))
        sheet.paste(icon,(10+i*185,45),icon)
    for x in (0,380):
        draw.text((x+20,140),'DONOR RETAIL',fill='white')
        draw.text((x+205,140),'SIDE PROFILE',fill='white')
    for i,(name,before,after) in enumerate(rows):
        x,y=(i//10)*380,166+(i%10)*112
        draw.text((x+12,y),LABELS[name],fill='white')
        for j,img in enumerate((before,after)):
            sheet.paste(img,(x+10+j*185,y+20),img)
    sheet.save(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--packs', type=Path, required=True)
    ap.add_argument('--lobby-swf', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--large', action='store_true', help='also render 684x332 inspection images')
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    _, tags = renderer.swf_tags(args.lobby_swf.read_bytes())
    _, _, argb = renderer.atlas(tags)
    # Flash bitmaps are premultiplied ARGB; references need straight RGB.
    alpha = argb[:, :, :1].astype(float) / 255
    rgba = np.dstack([argb[:, :, 1:] / np.maximum(alpha, 1e-9), argb[:, :, 0]])
    refs = [rgba[(r-1)*91:(r-1)*91+83, (c-1)*171:c*171] for r,c in renderer.NATIVE_REFERENCE]
    target_luma = float(np.mean([renderer.mean_luma(p) for p in refs]))
    pistol_refs = [rgba[(r-1)*91:(r-1)*91+83, (c-1)*171:c*171] for r,c in ((5,1),(6,2),(13,2))]
    pistol_luma = float(np.mean([renderer.mean_luma(p) for p in pistol_refs]))
    rows, record = [], []
    with (HERE / 'sources.tsv').open() as f:
        sources = list(csv.DictReader(f, delimiter='\t'))
    for source in sources:
        packdir = args.packs / f"{source['id']}-{source['from']}"
        data = (packdir / 'pack.json').read_bytes()
        assert sha(data) == source['sha256'], f'{packdir}: source pack hash differs'
        pack = json.loads(data)
        icons = next(o['icons'] for o in pack['operations'] if o['op'] == 'atlas_icons_add' and o['symbol'] == 'LobbyMenu_WeaponIcons')
        by_cell = {(i['row'], i['col']): i for i in icons}
        items = next(o['entries'] for o in pack['operations'] if o['op'] == 'dictionary_add' and o['table'] == 'items_dict')
        rendered = {}
        for item in items:
            if item['item_category'] not in (8,13,14,15,16,17):
                continue
            key = item['name']
            config = (packdir / 'files' / item['cfg_name']).read_bytes()
            values = {p: v for p,t,v in content_edit.walk(config)}
            cell = (values['/ui_desc/icon[0]'], values['/ui_desc/icon[1]'])
            icon_path = by_cell[cell]['source']
            model = 'models/' + renderer.config_strings(config)['object.model'] + '.skinned_model'
            parts = renderer.rifle_parts(packdir / 'files', model)
            magazine_pose.apply(packdir / 'files', model, parts)
            view = renderer.RIGHT
            pistol = item['item_category'] == 17
            size = (114,55) if pistol else (171,73)
            before = Image.open(packdir / icon_path).convert('RGBA')
            canvas, exposure, reference_luma, rendered_luma = matched_icon(parts, size, before)
            target = args.out / f'{key}.png'
            canvas.save(target)
            rendered[icon_path] = target
            # Larger actual render, not an upscaled thumbnail.
            if args.large:
                large = renderer.render(parts, view, (684,332), exposure=exposure, super_sample=2)
                large.save(args.out / f'{key}-large.png')
            rows.append((key, before, canvas))
            record.append(dict(name=key, pack=source['id'], cell=cell, model=model,
                               reference_icon=icon_path,
                               reference_sha256=sha((packdir / icon_path).read_bytes()),
                               reference_luma=round(float(reference_luma),6),
                               rendered_luma=round(float(rendered_luma),6),
                               exposure=round(float(exposure),6), sha256=sha(target.read_bytes())))
            print(f'{key}: cell {cell}, exposure {exposure:.2f}, brightness {rendered_luma:.2f} (donor {reference_luma:.2f})', flush=True)
        assert len(rendered) == len(icons), f'{packdir}: missed weapon icon'
    assert len(rows) == 20, f'Expected 20 weapons, got {len(rows)}'
    comparison(rows, rgba, args.out/'comparison.png')
    (args.out/'rendered.json').write_text(json.dumps(record,indent=2)+'\n')
    (args.out/'lighting.json').write_text(json.dumps(dict(reference_lobby_sha256=sha(args.lobby_swf.read_bytes()),
        exposure_reference='same weapon original donor icon in sources.tsv input pack',
        metric='mean RGB luma (0.299, 0.587, 0.114), alpha > 200; finished icon within 1/255',
        retail_rifle_luma=target_luma, retail_pistol_luma=pistol_luma, camera=renderer.RIGHT),indent=2)+'\n')


if __name__ == '__main__':
    main()
