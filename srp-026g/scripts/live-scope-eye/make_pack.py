#!/usr/bin/env python3
"""make_pack.py: the live scope variants' reticles back at the screen's centre (pack live-scope-eye,
edit list live-scope-eye-1.json; srp-026g/docs/notes/live-scope-eye.md).

usage: python3 -I make_pack.py --live <folder> --compose <vostok-port> --out <packs folder>/live-scope-eye-<version>

--live     the players' files (srp-game-data get of the stock db and the list's patches): gameplay/items/
           scopes/, the weapons' configs, the dictionary, models/weapons/ (prepare_live.sh).
--compose  a resource-porter vostok-port with `compose-model`.
--out      the pack's folder (a new one).

A scope's view (hide_weapon_on_aim) is its aimed model, drawn at the weapon model's scope_point, seen
from its `aim` locator. Patch 19 set each scope's eyepiece along the bore before that locator; where
the locator is turned about x (the AK family's, the VSS's, the Val's, the Saiga's: 0.004 to 0.008 rad)
the view is tilted and the reticle shows off the screen's centre, where the bullets go (seen in the
game on the rifle scopes' AK-74M variants: 10 px above at x4, 16 px at x6, 2026-11-01). Each such
variant gets its aimed model again, moved down (and aside, for a turn about y) onto the tilted line of
sight, at the reticle's distance (`models/weapons/attach/fit6/<aimed model>_<weapon model>`: one a
pair of aimed model and weapon model), and its config names it (the edit list's replace_string). Only
the aimed model changes: the weapon, its sights, its model and the scope's outside look stay as they
are. Each variant of ours whose weapon model's aim locator is turned is taken, but a collimator's (its
view is the weapon's own sights) and those that set a model with an unturned eye (the rifle scopes' @2).
"""
import argparse
import hashlib
import json
import math
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

REDDOT, GRID = 'scope_reddot_s', 'scope_aimgrid_s'


class lib:  # the few readers of the rifle scopes' scopes_lib this needs, as they are there
    @staticmethod
    def leaves(path):
        buf = bytearray(Path(path).read_bytes())
        out = {}
        for p, t, v in ce.walk(buf):
            if t == ce.T_STR:
                out[p] = v.decode('utf-8', 'replace')
            elif t in (ce.T_INT, ce.T_FLOAT):
                out[p] = ce.as_number(t, v)
            elif t == ce.T_BOOL:
                out[p] = bool(v & 1)
            elif t > ce.T_STR:
                out[p] = list(struct.unpack('<%df' % (t - 4), v))
        return out

    @staticmethod
    def locator(props, name):
        i = 0
        while f'/locators[{i}]/name' in props:
            if props[f'/locators[{i}]/name'] == name:
                return i, props[f'/locators[{i}]/position'], props.get(f'/locators[{i}]/rotation')
            i += 1
        return None

    @staticmethod
    def model_dir(root, model):
        for suffix in ('.model', '.skinned_model'):
            d = Path(root) / 'models' / (model + suffix)
            if d.exists():
                return d
        return None

    @staticmethod
    def surfaces(model):
        props = lib.leaves(model / 'settings')
        out, i = [], 0
        while f'/lod_hierrarchy/LOD0/surfaces[{i}]' in props:
            out.append(props[f'/lod_hierrarchy/LOD0/surfaces[{i}]'])
            i += 1
        return out

    @staticmethod
    def bbox_centre(model, surface):
        data = (model / 'render' / surface / 'converted_model').read_bytes()
        kind, size, n = struct.unpack_from('<3I', data, 0)
        stride = (size - 4) // n
        raw = np.frombuffer(data, dtype=np.uint8, count=stride * n, offset=12).reshape(n, stride)
        p = raw[:, 0:12].copy().view('<f4').reshape(n, 3).astype(np.float64)
        return (p.min(axis=0) + p.max(axis=0)) / 2


class rs:
    @staticmethod
    def weapon_models(live):
        out = {}
        d = lib.leaves(Path(live) / 'gameplay/db_static_dictionaries')
        i = 0
        while f'/items_dict[{i}]/name' in d:
            cfg = d[f'/items_dict[{i}]/cfg_name']
            if cfg.startswith('gameplay/items/weapons/wpn_'):
                model = lib.leaves(Path(live) / cfg).get('/object/model')
                if model:
                    out[d[f'/items_dict[{i}]/name']] = (cfg, model)
            i += 1
        return out

    @staticmethod
    def compose(port, parts, materials, out):
        args = [str(port), 'compose-model', '--output', str(out)]
        for model, t, chosen in parts:
            spec = f'{model}=%.6f,%.6f,%.6f' % tuple(t)
            if chosen:
                spec += ':' + ','.join(chosen)
            args += ['--part', spec]
        subprocess.run(args, check=True, capture_output=True)

    @staticmethod
    def files_of(directory, root):
        return {str(p.relative_to(root)): p.read_bytes() for p in sorted(Path(directory).rglob('*')) if p.is_file()}


PACK_ID = 'live-scope-eye'
VERSION = '1'
TURNED = 1e-6


def affected(live):
    """[(variant, its config path, weapon, weapon model, aimed model)] of every scope variant (not a
    collimator) whose weapon model's aim locator is turned."""
    weapons = rs.weapon_models(live)
    out = []
    for cfg in sorted((Path(live) / 'gameplay/items/scopes').glob('*.options')):
        d = lib.leaves(cfg)
        if '/srp_scope_fit/scope' not in d or d.get('/data/hide_weapon_on_aim') is False:
            continue
        change = next((d[k.replace('/path[1]', '/value')] for k in d if k.endswith('/path[1]') and d[k] == 'model'), None)
        for i in range(16):
            w = d.get(f'/srp_scope_fit/weapons[{i}]')
            if w is None:
                break
            model = change or weapons[w][1]
            mdir = lib.model_dir(live, model)
            rot = lib.locator(lib.leaves(mdir / 'render/export_properties'), 'aim')[2] or [0.0, 0.0, 0.0]
            if max(abs(x) for x in rot) > TURNED:
                out.append((cfg.stem, cfg, w, model, d['/data/aimed_model']))
    return out


ON_AXIS = 0.0015
DONE = 0.00005


def line_of_sight_shift(live, model, aimed):
    """(the scope-frame translation of the aimed model, the turn (x, y), the reticle's distance, why):
    the translation None where none is made.

    The reticle (the eyepiece's red dot, else its grid) as the weapon draws the aimed model: at
    scope_point, turned half about Y (and as scope_point itself turns: the VSS's is turned half about
    Y). The view of an aim locator turned by +x looks down (seen in the game: the AK-74M's reticles
    above the centre), by +y aside the same way; a roll (about z) turns it about its own centre.
    - a reticle on the bore before the eye (within ON_AXIS): set along the bore, the turn not taken
      into account (the AK family's, the Saiga's): moved by the turn at its distance;
    - a reticle on the turned line of sight already but for ON_AXIS (patch 19 set the A-545's there,
      the Ash-12's most of the way): moved the rest;
    - else (the A-91's, SR-3M's, Val's, VSS's, Vepr's, RPD's: their reticles 1 to 2 cm off the eye in
      the files): not made, as we do not know how the game shows those (to be seen in the game)."""
    props = lib.leaves(lib.model_dir(live, model) / 'render/export_properties')
    _, sp, sp_rot = lib.locator(props, 'scope_point')
    _, aim, rot = lib.locator(props, 'aim')
    sp, aim, rot = np.array(sp), np.array(aim), np.array(rot or [0.0, 0.0, 0.0])
    turn = (sp_rot or [0.0, 0.0, 0.0])[1]
    c, s = math.cos(turn), math.sin(turn)
    ry = np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    adir = lib.model_dir(live, aimed)
    centre = lib.bbox_centre(adir, REDDOT if REDDOT in lib.surfaces(adir) else GRID)
    reticle = sp + ry @ (centre * np.array([-1.0, 1.0, -1.0]))
    distance = aim[2] - reticle[2]
    if distance <= 0:
        sys.exit(f'{aimed} on {model}: the reticle is not before the eye')
    tilt = np.array([-distance * math.tan(rot[1]), -distance * math.tan(rot[0]), 0.0])
    off = reticle - aim
    off[2] = 0.0
    if np.abs(off[:2]).max() < ON_AXIS:
        moved, why = tilt, 'on the bore: the turn added'
    elif np.abs((tilt - off)[:2]).max() < ON_AXIS:
        moved, why = tilt - off, 'on the line of sight but for the rest'
        if np.abs(moved).max() < DONE:
            return None, rot[:2], distance, 'on the line of sight already'
    else:
        return None, rot[:2], distance, f'{off[0] * 1000:+.1f}, {off[1] * 1000:+.1f} mm off the eye: not made'
    # back into the aimed model's own frame
    return np.linalg.inv(ry) @ moved * np.array([-1.0, 1.0, -1.0]), rot[:2], distance, why


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--compose', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    work = Path(tempfile.mkdtemp())
    try:
        build(args, work)
    finally:
        shutil.rmtree(work)


def build(args, work):
    rows = affected(args.live)
    if not rows:
        sys.exit('no scope variant with a turned eye')
    made, files, edits, report, skipped = {}, {}, {}, [], []
    for name, cfg, weapon, model, aimed in rows:
        key = (aimed, model)
        if key not in made:
            shift, rot, distance, why = line_of_sight_shift(args.live, model, aimed)
            print(f'{name} on {weapon}: {why}')
            if shift is None:
                made[key] = None
                skipped.append(f'{name}\t{weapon}\t{model}\t{why}')
                continue
            new = f'weapons/attach/fit6/{aimed.split("/")[-1]}_{model.split("/")[-1]}'
            out = work / 'models' / (new + '.model')
            adir = lib.model_dir(args.live, aimed)
            rs.compose(args.compose, [(adir, shift, [])], {}, out)
            files.update(rs.files_of(out, work))
            made[key] = new
            report.append(f'{aimed}\t{model}\t{new}\t{rot[0]:.5f},{rot[1]:.5f}\t{distance:.4f}\t'
                          + ','.join('%.5f' % x for x in shift))
        if made[key] is None:
            continue
        edits.setdefault(made[key], {}).setdefault(aimed, []).append(f'gameplay/items/scopes/{name}.options')
    # a variant config naming one aimed model for two weapon models of other eyes cannot be told apart
    per_cfg = {}
    for new, by in edits.items():
        for aimed, cfgs in by.items():
            for c in cfgs:
                per_cfg.setdefault(c, set()).add(new)
    for c, news in per_cfg.items():
        if len(news) > 1:
            sys.exit(f'{c}: its weapons need {sorted(news)}; one aimed model a config')
    operations = [{'op': 'add_file', 'path': p} for p in sorted(files)]
    pack = {'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
            'description': 'the live scope variants\' reticles back at the screen\'s centre: an aimed model on the '
                           'tilted line of sight of each weapon model whose aim locator is turned '
                           '(srp-026g/docs/notes/live-scope-eye.md)',
            'operations': operations, 'server': {'items': []},
            'notes': ['made by srp-026g/scripts/live-scope-eye/make_pack.py']}
    for p, data in files.items():
        t = args.out / 'files' / p
        t.parent.mkdir(parents=True, exist_ok=True)
        t.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    # replace_string's `files` is one glob: an entry a config
    replaces = [{'files': c, 'from': aimed, 'to': new}
                for new, by in sorted(edits.items()) for aimed, cfgs in sorted(by.items()) for c in sorted(set(cfgs))]
    about = (f"The live scope variants' reticles at the screen's centre (pack live-scope-eye; srp-026g/docs/notes/"
             f"live-scope-eye.md): {len(per_cfg)} variant configs name an aimed model on the tilted line of "
             "sight of their weapon model's turned aim locator. Made by scripts/live-scope-eye/make_pack.py.")
    lines = ['{', ' "_about": ' + json.dumps(about, ensure_ascii=False) + ',', ' "replace_string": [']
    lines += ['  ' + json.dumps(e, ensure_ascii=False) + (',' if i < len(replaces) - 1 else '') for i, e in enumerate(replaces)]
    lines += [' ]', '}']
    (HERE.parent / 'content-edit' / 'live-scope-eye-1.json').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    (HERE / 'aimed.tsv').write_text('# made by make_pack.py: each aimed model made again, for a weapon model with a turned eye\n'
                                    '# aimed model\tweapon model\tnew aimed model\taim turn (x, y rad)\treticle distance (m)\t'
                                    'shift (scope frame, m)\n' + '\n'.join(report) + '\n', encoding='utf-8')
    (HERE / 'variants.tsv').write_text('# made by make_pack.py: each variant whose aimed model is made again\n# variant\tweapon\t'
                                       'weapon model\taimed model (- left as it is)\n' + '\n'.join(f'{n}\t{w}\t{m}\t{made[(a, m)] or "-"}' for n, _c, w, m, a in rows) + '\n',
                                       encoding='utf-8')
    digest = hashlib.sha256((args.out / 'pack.json').read_bytes()).hexdigest()
    print(f'{args.out}: {len(files)} files, {sum(1 for v in made.values() if v)} aimed models, {len(per_cfg)} variant configs; pack.json sha256 {digest}')


if __name__ == '__main__':
    main()
