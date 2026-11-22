#!/usr/bin/env python3
"""prepare_inputs.py: the files make_pack.py makes Artifact Hunt's content of, in one folder.

usage: python3 -I prepare_inputs.py --donor <0.46c2 game folder> --stock <0.26g resources.db>
           --patches <folder of resources.db.patch_<n>> --vostok-port <program> --out <new folder>
           [--pin --dbgrab <program>]

Writes <out>/donor/<game path> (0.46c2's files, from its resources.NN.db volumes), <out>/live/<game
path> (0.26g's files as players have them: the newest of the patches, else the stock db) and
<out>/live.list (every path the stock db and the patches hold), and checks each file against
inputs.sha256.

--pin makes the lists instead of checking them (after a release that changed a map, or a change
of what is ported): inputs.sha256, from the 27 maps' files and the dependency closure of ASSETS
in the donor (every material, texture, particle and sound a config of it names, that 0.26g has
not); stock_forms.tsv, from the stock db's own materials and particles (dbgrab): what a ported
material or particle may hold (make_pack.py, "Assets").

`vostok-port` is resource-porter's (stat, extract, list); `dbgrab` is tools' (get <prefix>).
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

LEVELS = ('01', '02', '03', '04', '05', '06', '07', '08', '09')
# 0.46c2's weather of a config, 0.26g's config and project names of it (level_08 has no rain:
# its third weather is "before the rain" in both builds).
WEATHERS = (('day', 'day', 'day'), ('evn', 'evn', 'evening'), ('rain', 'rain', 'rain'))
LEVEL_08_RAIN = ('rain', 'prerain', 'prerain')
# What the mode needs that 0.26g lacks (artefact_hunt.md, "Data"): the roots of the closure.
ASSETS = ('gameplay/victory_items/artefact.options',
          'gameplay/effects/effect_artefact_wood_shine.options',
          'models/item/artefacts/artefact_shining_forest.model/',
          'models/item/storage_device/box_atlant8c.model/',
          'particles/fx/artifacts/wood_shain.particle',
          'particles/fx/artifacts/wood_shain_start.particle',
          'particles/fx/artifacts/wood_shain_end.particle')
VOICES = ('english', 'french', 'german', 'polish', 'russian', 'spanish', 'ukrainian')
LINES = ('art_spawned', 'art_is_our', 'enemy_got_art', 'we_lost_art', 'enemy_loose_art', 'art_on_our_base',
         'art_on_enemy_base')
CONFIGS = ('.material', '.particle', '.options', '/settings', '/export_properties', '.sound_collection',
           '.raw_options', '/exported_primitives')
# The material stages make_pack.py converts (stock_forms.tsv keeps what 0.26g's own hold).
STAGES = (('forward', 'forward_default'), ('g_stage', 'g_stage_default'))


def maps():
    """(level, 0.46c2 weather, 0.26g config weather, 0.26g project weather) of the 27 maps."""
    for level in LEVELS:
        for weather in WEATHERS:
            yield (level,) + (LEVEL_08_RAIN if level == '08' and weather[0] == 'rain' else weather)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Archives:
    """Archives laid one over another, the last the newest (vostok-port stat, extract, list)."""

    def __init__(self, program, archives):
        self.program, self.archives, self.lists = program, list(archives), None

    def paths(self):
        if self.lists is None:
            self.lists = [set(subprocess.run([self.program, 'list', '--db', str(a)], check=True, capture_output=True,
                                             text=True).stdout.split('\n')) for a in self.archives]
        return set().union(*self.lists)

    def read(self, path):
        self.paths()
        for archive, listed in zip(reversed(self.archives), reversed(self.lists)):
            if path in listed:
                with tempfile.TemporaryDirectory() as folder:
                    out = Path(folder) / 'file'
                    subprocess.run([self.program, 'extract', '--db', str(archive), '--path', path, '--output',
                                    str(out)], check=True, capture_output=True)
                    return out.read_bytes()
        sys.exit(f'no archive has {path}')


def strings(value, out):
    if isinstance(value, str):
        out.add(value)
    elif isinstance(value, dict):
        for child in value.values():
            strings(child, out)
    elif isinstance(value, list):
        for child in value:
            strings(child, out)


def closure(donor, live, roots):
    """The donor files `roots` need that 0.26g has not: every file under a model's folder, and
    what a config's strings name (a material, a texture with its options, a particle, a model, a
    sound with its options and loudness, a loudness preset)."""
    listed, out, queue = donor.paths(), {}, list(roots)
    while queue:
        path = queue.pop()
        if path.endswith('/'):
            queue += [p for p in listed if p.startswith(path)]
            continue
        if path in out or path in live or path not in listed:
            continue
        out[path] = donor.read(path)
        if not path.endswith(CONFIGS):
            continue
        names = set()
        strings(bcw.loads(out[path]), names)
        for name in names:
            name = name.removeprefix('resources/')
            if '/' not in name:
                name = f'preset/{name}'
            for candidate in (f'material_instances/{name}.material', f'textures/{name}.dds', f'textures/{name}.options',
                              f'particles/{name}.particle', f'models/{name}.model/', f'sounds/single/{name}.high.ogg',
                              f'sounds/single/{name}.high.rms', f'sounds/single/{name}.raw_options',
                              f'sounds/spl/{name.removeprefix("preset/")}.spl'):
                if candidate.endswith('/') and any(p.startswith(candidate) for p in listed):
                    queue.append(candidate)
                elif candidate in listed:
                    queue.append(candidate)
    return out


def stock_forms(dbgrab, stock):
    """stock_forms.tsv: of the stock db's materials, each (stage, effect) of STAGES: the keys its
    effect and its stage hold and the combinations of `use_` switches drawn; of its particles,
    each node type's properties."""
    rows = set()
    with tempfile.TemporaryDirectory() as folder:
        for prefix in ('material_instances/', 'particles/'):
            subprocess.run([dbgrab, str(stock), 'get', prefix, folder], check=True, capture_output=True)
        for path in sorted(Path(folder).iterdir()):
            config = bcw.loads(path.read_bytes())
            if path.name.endswith('.material'):
                for stage, body in config['material'].items():
                    effect = body.get('effect') if isinstance(body, dict) else None
                    if not isinstance(effect, dict) or (stage, effect.get('effect_id')) not in STAGES:
                        continue
                    rows.update(('stage_key', stage, key) for key in body)
                    rows.update(('effect_key', stage, key) for key in effect)
                    on = sorted(key for key, flag in effect.items()
                                if key.startswith('use_') and isinstance(flag, dict) and flag.get('value') is True)
                    rows.add(('combination', stage, ' '.join(on)))
            elif path.name.endswith('.particle'):
                def walk(value):
                    if isinstance(value, dict):
                        if 'particle_entity_type' in value:
                            rows.update(('particle_property', value['particle_entity_type'], key)
                                        for key in value.get('properties') or {})
                        for child in value.values():
                            walk(child)
                walk(config)
    text = ('# What 0.26g\'s own files hold (the stock resources.db; prepare_inputs.py --pin): a ported material\n'
            '# keeps the keys its stage and effect have here and a combination of switches drawn here; a ported\n'
            '# particle the properties its node types have here.\n'
            'kind\tof\twhat\n' + ''.join('\t'.join(row) + '\n' for row in sorted(rows)))
    (HERE / 'stock_forms.tsv').write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--patches', required=True, type=Path)
    parser.add_argument('--vostok-port', required=True)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--pin', action='store_true')
    parser.add_argument('--dbgrab')
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    donor = Archives(args.vostok_port, sorted(args.donor.glob('resources.*.db')))
    patches = sorted(args.patches.glob('resources.db.patch_*'), key=lambda p: int(p.name.rsplit('_', 1)[1]))
    live = Archives(args.vostok_port, [args.stock] + patches)
    if not donor.archives or not patches:
        sys.exit('no resources.NN.db in --donor, or no patch in --patches')
    files = {}
    if args.pin:
        if not args.dbgrab:
            sys.exit('--pin needs --dbgrab')
        for level, theirs, config, _ in maps():
            path = f'gameplay/game_modes/level_{level}_{theirs}_ah.options'
            files['donor/' + path] = donor.read(path)
            project = bcw.loads(files['donor/' + path])['mode']['match_making_params']['map_name']
            files[f'donor/projects/{project}/client_project'] = donor.read(f'projects/{project}/client_project')
            path = f'gameplay/game_modes/level_{level}_{config}_4.options'
            files['live/' + path] = live.read(path)
            project = bcw.loads(files['live/' + path])['mode']['match_making_params']['map_name']
            files[f'live/projects/{project}/client_project'] = live.read(f'projects/{project}/client_project')
            # the Batteries project, whose props the boxes' stands are (make_pack.py, STANDS)
            path = f'projects/level_{level}/{project.rsplit("/", 1)[1][:-2]}_2/client_project'
            files['live/' + path] = live.read(path)
        roots = list(ASSETS) + [f'sounds/collections/vo/{voice}/artifact_hunt/{line}.sound_collection'
                                for voice in VOICES for line in LINES]
        for path, data in closure(donor, live.paths(), roots).items():
            files['donor/' + path] = data
        lines = ['# sha256 and the input each file is (prepare_inputs.py --pin): donor/ 0.46c2\'s, live/ 0.26g\'s as\n'
                 '# players have it (the stock db and the patches up to %s), at its game path\n' % patches[-1].name]
        lines += [f'{sha256(data)}  {path}\n' for path, data in sorted(files.items())]
        (HERE / 'inputs.sha256').write_text(''.join(lines))
        stock_forms(args.dbgrab, args.stock)
    else:
        for line in (HERE / 'inputs.sha256').read_text().splitlines():
            if line and not line.startswith('#'):
                want, path = line.split(None, 1)
                source, game_path = path.split('/', 1)
                files[path] = (donor if source == 'donor' else live).read(game_path)
                if sha256(files[path]) != want:
                    sys.exit(f'{path} is not the file inputs.sha256 names (sha256 {want}): another build, or a '
                             f'release changed it (--pin)')
    for path, data in files.items():
        target = args.out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'live.list').write_text('\n'.join(sorted(live.paths() - {''})) + '\n')
    print(f'{args.out}: {len(files)} files, {sum(map(len, files.values())) // 1024} KiB')


if __name__ == '__main__':
    main()
