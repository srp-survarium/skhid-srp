"""make_chemical_probes_pack.py BAKED OUT: the pack v011w-chemical-probes, Chemical Plant v0.11's environment
probes rendered for its evening and rain.

0.11 never had these weathers (Chemical Plant shipped day only), and the game has no probe baker (the
editor made them). BAKED holds, per weather (evening, rain) and probe (the day project's object index),
`<weather>/p<index>.dds` and `<weather>/p<index>_diffuse.dds`: each cube drawn in the game at the probe
(six 90-degree faces of the lit frame before post, probe light off as the editor bakes), encoded as 0.26g
reads a probe (m * (c/a)^2 / 4, c = K * sqrt(L), K measured on native Chemical Plant's own cubes), 128 px
DXT5 with six mips, and its 16 px diffuse cube made by resource-porter's probe_irradiance (example
probe_diffuse). The rig and its numbers: private notes of the Chemical v0.11 weathers work.
The files go to textures/ported/v011w/environment_probes/level_04/<weather>_p<index>[_diffuse].dds with
0.26g's own probe .options; v011w-chemical-weathers-1.json points the projects' probes at them.
"""
import hashlib, json, sys
from pathlib import Path

STOCK_DB_SHA = 'c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c'
FOLDER = 'textures/ported/v011w/environment_probes/level_04'
# 0.26g's probe options (every native probe has these bytes; its diffuse cubes the second)
OPTIONS = {'': 'bddf1a56e0ce053a', '_diffuse': 'c1b64224300ba7c4'}
VERSION = '1'


def main():
    baked, out = Path(sys.argv[1]), Path(sys.argv[2])
    assert not out.exists(), 'the output folder must be new'
    ops = []
    for weather in ('evening', 'rain'):
        for cube in sorted((baked / weather).glob('p*.dds')):
            if cube.stem.endswith('_diffuse'):
                continue
            for kind in ('', '_diffuse'):
                src = cube.with_name(cube.stem + kind + '.dds')
                options = (baked / f'probe{kind}.options').read_bytes()
                assert hashlib.sha256(options).hexdigest().startswith(OPTIONS[kind]), f'not 0.26g\'s probe{kind} options'
                for name, data in ((f'{weather}_{cube.stem}{kind}.dds', src.read_bytes()),
                                   (f'{weather}_{cube.stem}{kind}.options', options)):
                    path = f'{FOLDER}/{name}'
                    (out / 'files' / path).parent.mkdir(parents=True, exist_ok=True)
                    (out / 'files' / path).write_bytes(data)
                    ops.append({'op': 'add_file', 'path': path})
    pack = {
        'format_version': 1, 'id': 'v011w-chemical-probes', 'version': VERSION, 'target_build': '0.26g',
        'description': "Chemical Plant v0.11's environment probes for its evening and rain (a recreation: 0.11 "
                       "shipped the map day only), rendered in the game for its v0.11 layout",
        'requires': [], 'base_db_sha256': STOCK_DB_SHA, 'operations': ops, 'server': None,
        'notes': ['Built by srp-026g/scripts/v011-weather/make_chemical_probes_pack.py from cubes baked in the '
                  'game (read it for how).'],
    }
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(hashlib.sha256((out / 'pack.json').read_bytes()).hexdigest(), out, len(ops), 'files')


if __name__ == '__main__':
    main()
