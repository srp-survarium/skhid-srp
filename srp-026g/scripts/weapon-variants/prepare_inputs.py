#!/usr/bin/env python3
"""Gather the pack's pinned inputs (inputs.sha256), in three groups:

  compose  the files packs are composed over: the stock resources.db and release/content.tsv's base
           patches. The live guns' configs as the edit lists start from them, the dictionary, the
           guns' models, materials and diffuse textures, the scope fits of the guns (configs and
           models).
  players  the players' files: the stock db and every patch of release/patches.tsv. The lobby
           movie (the weapon atlas: free cells, the live guns' icons) and the live guns' camouflage
           masks (_aoac, client patch 44) with their .options template.
  donor    0.69d0's files: each skin's model and its gun's, their materials, albedo and metalness
           maps; the TOZ-122 "Leather"'s decor model and its maps.
  Both builds' AKM and SKS "camo_st" textures (common.CALIBRATION) are in compose and donor too:
  check_pack.py measures paint.py on them.

usage: python3 -I prepare_inputs.py --stock <stock resources.db> --patches <folder of the patches>
           --game-data-bin <srp-game-data> --donor <0.69d0 volumes: a .split list>
           --donor-port <a vostok-port that reads 0.69d0's volumes> --out <new folder> [--pin]

--pin computes the inputs from variants.tsv and the files and writes inputs.sha256; without it the
extracted bytes are checked against it. No server data or release working folder is read.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile

from common import (AOAC_OPTIONS, CALIBRATION, DICTIONARY, HERE, MOVIE, ROOT, SCOPES, WEAPONS, bcw, sha, variants)


def layers(stock, patches, rows):
    """srp-game-data's layers: the stock db and the patches `rows` names, each checked by its sha256."""
    out = ['--db', str(stock)]
    for name, checksum in rows:
        path = Path(patches) / name
        if sha(path.read_bytes()) != checksum:
            raise ValueError(f'{path}: not the patch the manifest names (sha256 {checksum})')
        out += ['--patch', str(path)]
    return out


def compose_rows():
    """release/content.tsv's base rows: what packs are composed over."""
    rows = []
    for line in (ROOT / 'release' / 'content.tsv').read_text().splitlines():
        cols = line.split('\t')
        if cols[0] == 'base':
            rows.append((cols[1], cols[2]))
    return rows


def player_rows():
    """release/patches.tsv: the patches players have."""
    rows = []
    for line in (ROOT / 'release' / 'patches.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            cols = line.split('\t')
            rows.append((cols[0], cols[1]))
    return rows


class Game:
    """One layer set of the released game, read through srp-game-data into a scratch folder."""

    def __init__(self, binary, layer_args, work):
        self.binary, self.layers, self.work = binary, layer_args, Path(work)
        self.work.mkdir(parents=True, exist_ok=True)
        listing = subprocess.run([str(binary), 'files', *layer_args], check=True, capture_output=True, text=True)
        self.files = {line.split('\t')[0] for line in listing.stdout.splitlines()}
        self.got = set()

    def get(self, paths, out=None):
        paths = sorted(set(paths) - (set() if out else self.got))
        missing = [p for p in paths if p not in self.files]
        if missing:
            raise ValueError(f'not in the game: {missing[:5]}')
        if not paths:
            return
        target = Path(out) if out else self.work
        with tempfile.TemporaryDirectory(dir=target.parent if out else self.work.parent) as tmp:
            listing = Path(tmp) / 'paths'
            listing.write_text(''.join(p + '\n' for p in paths))
            subprocess.run([str(self.binary), 'get', *self.layers, '--paths', str(listing), '--out', tmp + '/x'],
                           check=True, capture_output=True)
            shutil.copytree(tmp + '/x', target, dirs_exist_ok=True)
        if not out:
            self.got |= set(paths)

    def config(self, path):
        self.get([path])
        return bcw.load(self.work / path)

    def folder(self, prefix):
        return sorted(p for p in self.files if p.startswith(prefix))


class Donor:
    """0.69d0's volumes, one file at a time through a vostok-port that reads them."""

    def __init__(self, port, split, work):
        self.port, self.split, self.work = port, split, Path(work)
        self.listing = None

    def get(self, path):
        target = self.work / path
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run([str(self.port), 'extract', '--db', str(self.split), '--path', path,
                            '--output', str(target)], check=True, capture_output=True)
        return target

    def config(self, path):
        return bcw.load(self.get(path))


def textures(material):
    """A 0.26g material's texture_* values (non-empty)."""
    effect = material['material']['g_stage']['effect']
    return {k: v['value'] for k, v in effect.items() if k.startswith('texture') and v.get('value')}


def pin(args, work):
    rows = variants()
    compose = Game(args.game_data_bin, layers(args.stock, args.patches, compose_rows()), work / 'compose')
    players = Game(args.game_data_bin, layers(args.stock, args.patches, player_rows()), work / 'players')
    donor = Donor(args.donor_port, args.donor, work / 'donor')
    groups = {'compose': {DICTIONARY}, 'players': {MOVIE, AOAC_OPTIONS}, 'donor': set()}
    dictionary = compose.config(DICTIONARY)
    bases = sorted({row.base for row in rows})
    models = {}
    for base in bases:
        path = f'{WEAPONS}/wpn_{base}.options'
        groups['compose'].add(path)
        models[base] = compose.config(path)['object']['model']
    # The scope fits of the live guns (category 22 rows whose srp_scope_fit names one) and their
    # weapon models (the fit's modification sets object/model).
    fit_models = []
    for item in dictionary['items_dict']:
        if item['item_category'] != 22 or not item['cfg_name'].startswith(SCOPES + '/fit'):
            continue
        config = compose.config(item['cfg_name'])
        weapons = config.get('srp_scope_fit', {}).get('weapons', [])
        if any(w in bases for w in weapons):
            groups['compose'].add(item['cfg_name'])
            for modifier in config['modifications'][0]['modifiers']:
                if modifier['path'] == ['object', 'model']:
                    fit_models.append(modifier['value'])
    for model in sorted(set(models.values())) + fit_models:
        files = compose.folder(f'models/{model}.skinned_model/')
        groups['compose'] |= set(files)
        settings = compose.config(f'models/{model}.skinned_model/settings')
        for surface in settings['material_settings'].values():
            material = f"material_instances/{surface['material_name']}.material"
            groups['compose'].add(material)
            if model in models.values():
                diffuse = textures(compose.config(material)).get('texture_diffuse')
                if diffuse:
                    groups['compose'] |= {f'textures/{diffuse}.dds', f'textures/{diffuse}.options'}
                    groups['players'].add(f'textures/{diffuse}_aoac.dds')
    # paint.py's check: the premium textures both builds ship, and their guns'.
    for ours, theirs, gun, gun69, metal69 in CALIBRATION:
        groups['compose'] |= {f'textures/{ours}.dds', f'textures/{gun}.dds'}
        for path in (theirs, gun69, metal69):
            donor.get(f'textures/{path}.dds')
            groups['donor'].add(f'textures/{path}.dds')
    # Only the files that exist: a material may name a texture the game never shipped.
    for name in ('compose', 'players'):
        game = compose if name == 'compose' else players
        groups[name] = {p for p in groups[name] if p in game.files}
    # The donor: each skin's model and its gun's, every LOD0 surface's material, its albedo and
    # metalness (the TOZ-122 "Leather": its decor too, with the normal and smoothness maps).
    for row in rows:
        donor_models = [row.donor_model, row.donor_base_model]
        if row.name == 'toz_122_premium':
            donor_models.append(row.donor_model + '_decor')
        for model in donor_models:
            kind = 'model' if model.endswith('_decor') else 'skinned_model'
            settings_path = f'models/{model}.{kind}/settings'
            settings = donor.config(settings_path)
            groups['donor'].add(settings_path)
            for surface in settings['lod_hierrarchy']['LOD0']['surfaces']:
                for leaf in ('converted_model', 'export_properties'):
                    path = f'models/{model}.{kind}/render/{surface}/{leaf}'
                    donor.get(path)
                    groups['donor'].add(path)
                material = f"material_instances/{settings['material_settings'][surface]['material_name']}.material"
                effect = donor.config(material)['material']['g_stage']['effect']
                groups['donor'].add(material)
                maps = ['t_albedo', 't_metalness'] + (['t_normal', 't_smoothness'] if kind == 'model' else [])
                for key in maps:
                    value = effect[key]['value']
                    if value:
                        donor.get(f'textures/{value}.dds')
                        groups['donor'].add(f'textures/{value}.dds')
            for leaf in ('bind_pose', 'export_properties') if kind == 'skinned_model' else ('export_properties',):
                path = f'models/{model}.{kind}/render/{leaf}'
                donor.get(path)
                groups['donor'].add(path)
    return groups


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ('stock', 'patches', 'game-data-bin', 'donor', 'donor-port', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--pin', action='store_true', help='compute the inputs and pin them')
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new directory')
    args.out.mkdir(parents=True)
    work = args.out / '.work'
    if args.pin:
        groups = pin(args, work)
    else:
        groups = {}
        for line in (HERE / 'inputs.sha256').read_text().splitlines():
            group, path = line.split('  ', 1)[1].split('/', 1)
            groups.setdefault(group, set()).add(path)
    compose = Game(args.game_data_bin, layers(args.stock, args.patches, compose_rows()), work / 'c')
    players = Game(args.game_data_bin, layers(args.stock, args.patches, player_rows()), work / 'p')
    compose.get(groups['compose'], args.out / 'compose')
    players.get(groups['players'], args.out / 'players')
    donor = Donor(args.donor_port, args.donor, work / 'donor')
    for path in sorted(groups['donor']):
        target = args.out / 'donor' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(donor.get(path), target)
    shutil.rmtree(work)
    keys = {f'{group}/{path}' for group, paths in groups.items() for path in paths}
    if args.pin:
        lines = sorted(f'{sha((args.out / key).read_bytes())}  {key}' for key in keys)
        (HERE / 'inputs.sha256').write_text(''.join(line + '\n' for line in lines))
        print(f'{len(keys)} inputs pinned in inputs.sha256')
    else:
        sums = dict(reversed(line.split('  ', 1)) for line in (HERE / 'inputs.sha256').read_text().splitlines())
        for key, expected in sums.items():
            if sha((args.out / key).read_bytes()) != expected:
                raise ValueError(f'{key}: extracted bytes differ from the pinned input')
        print(f'{len(keys)} pinned inputs verified in {args.out}')


if __name__ == '__main__':
    main()
