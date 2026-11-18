"""make_fog_pack.py STOCK_DB OUT: the pack v011e-radar-fog, 0.11e's additive geometry fog in 0.26g's own
additive fog program.

0.11e fogs the geometry in two passes of its effect_atmospheric_scattering: technique 6 multiplies the
frame by 1 - min(depth, D) * 0.1 / D (`atmospheric_scattering_on_geometry_mul.ps`), technique 7 adds the
in-scattered light read from the Rayleigh and Mie tables (`..._add.ps`), D the weather's
`atmosphere_inscatter_distance`. 0.26g compiles the same two techniques (3 and 4) but draws one pass of
its own (5, `..._temporary.ps`); exe patch v011w_render draws 3 and 4 on a 0.11 weather's scene.
0.26g's multiplicative program is 0.11e's with 0.26g's constant layout; its additive one is another
equation. This pack gives 0.26g's additive file 0.11e's program, compiled from source for 0.26g's layout
(srp-026g/shaders/v011w.sh build; `v011w.sh check` proves the sources are 0.11e's, byte for byte).
Nothing else draws technique 4, so no other map changes. Needs what v011w.sh needs (Wine, fxc, dbgrab).
"""
import hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHADERS = REPO / 'srp-026g/shaders'
STOCK_DB_SHA = 'c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c'
STOCK_SHA = 'd9334ae5a9c103f5a9b0569d2bb7f5d26705ce68451e02efc503b8d84a952a8a'   # 0.26g's add.ps file


def main():
    stock_db, out = map(Path, sys.argv[1:3])
    assert not out.exists(), 'the output folder must be new'
    dbgrab = os.environ.get('DBGRAB', str(REPO / 'tools/re-kit/common/dbgrab/target/release/dbgrab'))
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([dbgrab, str(stock_db), 'get', 'atmospheric_scattering_on_geometry_add.ps/', f'{tmp}/stock'],
                       check=True, stdout=subprocess.DEVNULL)
        [stock] = Path(tmp, 'stock').iterdir()
        assert hashlib.sha256(stock.read_bytes()).hexdigest() == STOCK_SHA, 'not 0.26g\'s additive fog program'
        built = Path(subprocess.run([str(SHADERS / 'v011w.sh'), 'build', str(stock_db), f'{tmp}/built'], check=True,
                                    capture_output=True, text=True).stdout.split()[-1])
        path = built.relative_to(f'{tmp}/built').as_posix()
        (out / 'files' / path).parent.mkdir(parents=True)
        (out / 'files' / path).write_bytes(built.read_bytes())
    pack = {
        'format_version': 1, 'id': 'v011e-radar-fog', 'version': '2', 'target_build': '0.26g',
        'description': "0.11e's additive geometry fog in 0.26g's own additive fog program (technique 4 of "
                       "effect_atmospheric_scattering, which only exe patch v011w_render draws, on the 0.11 "
                       "weathers' projects)",
        'requires': [], 'base_db_sha256': STOCK_DB_SHA,
        'operations': [{'op': 'replace_file', 'path': path, 'base_sha256': STOCK_SHA}],
        'server': None,
        'notes': ['Built by srp-026g/scripts/v011-weather/make_fog_pack.py: 0.11e\'s program compiled from '
                  'srp-026g/shaders/v011w/ for 0.26g\'s layout (v011w.sh build), after 0.26g\'s blob\'s '
                  'include records.'],
    }
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(hashlib.sha256((out / 'pack.json').read_bytes()).hexdigest(), out)


if __name__ == '__main__':
    main()
