"""make_fog_pack.py STOCK_DB ORIGINAL_DB OUT: the pack v011e-radar-fog, 0.11e's atmosphere programs for exe patch
v011w_render: its additive geometry fog in 0.26g's own additive fog program, and its scattering tables,
sky, skylight, clouds and post as the twin families the patch compiles (make_mie_rayleigh_texture_v011w,
atmosphere_v011w, skylight_v011w, atmosphere_clouds_alpha_blend_v011w, atmosphere_clouds_modulate_v011w,
complex_post_process_blend_v011w_026g: 0.11e's post on 0.26g's inputs, under the plain key: shaders/masks lists no define of a family it does not name);
and 0.11e's light cones: its forward_volume_cone_base for vertex inputs 1..6 (forward_volume_cone_base_v011w_<n>,
which the patch gives a marked cone material's passes) and the falloff its cones read and 0.26g has none of,
textures/fx/sphere_falloff (from ORIGINAL_DB, 0.11e's resources.db; 0.26g's effect binds it by that name).

0.11e fogs the geometry in two passes of its effect_atmospheric_scattering: technique 6 multiplies the
frame by 1 - min(depth, D) * 0.1 / D (`atmospheric_scattering_on_geometry_mul.ps`), technique 7 adds the
in-scattered light read from the Rayleigh and Mie tables (`..._add.ps`), D the weather's
`atmosphere_inscatter_distance`. 0.26g compiles the same two techniques (3 and 4) but draws one pass of
its own (5, `..._temporary.ps`); exe patch v011w_render draws 3 and 4 on a 0.11 weather's scene.
0.26g's multiplicative program is 0.11e's with 0.26g's constant layout; its additive one is another
equation, and so are its tables and sky. This pack gives 0.26g's additive file 0.11e's program and adds
0.11e's tables and sky as new families, all compiled from source for 0.26g's layout
(srp-026g/shaders/v011w.sh build; `v011w.sh check` proves the sources are 0.11e's, byte for byte).
Nothing else draws technique 4, and only the patch's twin techniques name the new families, so no other
map changes. Needs what v011w.sh needs (Wine, fxc, dbgrab).
"""
import hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHADERS = REPO / 'srp-026g/shaders'
STOCK_DB_SHA = 'c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c'
STOCK_SHA = 'd9334ae5a9c103f5a9b0569d2bb7f5d26705ce68451e02efc503b8d84a952a8a'   # 0.26g's add.ps file
SPHERE_FALLOFF = {'textures/fx/sphere_falloff.dds': '7cb7da5821e77a34e0b0f579ad08c169f20a392b5459d26a154632b49ba8413f',
                  'textures/fx/sphere_falloff.options': '382274d7d0b81c72a86a2a1e88d74f8c457329b92d1fa4786ef5aa3571f11d27'}


def main():
    stock_db, original_db, out = map(Path, sys.argv[1:4])
    assert not out.exists(), 'the output folder must be new'
    dbgrab = os.environ.get('DBGRAB', str(REPO / 'tools/re-kit/common/dbgrab/target/release/dbgrab'))
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([dbgrab, str(stock_db), 'get', 'atmospheric_scattering_on_geometry_add.ps/', f'{tmp}/stock'],
                       check=True, stdout=subprocess.DEVNULL)
        [stock] = Path(tmp, 'stock').iterdir()
        assert hashlib.sha256(stock.read_bytes()).hexdigest() == STOCK_SHA, 'not 0.26g\'s additive fog program'

        def build(*args):
            lines = subprocess.run([str(SHADERS / 'v011w.sh'), 'build', str(stock_db), f'{tmp}/built', *args],
                                   check=True, capture_output=True, text=True).stdout.split()
            return [Path(line).relative_to(f'{tmp}/built').as_posix() for line in lines]
        [path] = build()
        twins = (build('--twin', 'make_mie_rayleigh_texture', 'atmosphere', 'skylight') + build('--post-twin')
                 + build('--twin-of', 'atmosphere_clouds', 'atmosphere_clouds_alpha_blend', 'atmosphere_clouds_modulate')
                 + build('--cone'))
        for name in [path, *twins]:
            (out / 'files' / name).parent.mkdir(parents=True, exist_ok=True)
            (out / 'files' / name).write_bytes(Path(tmp, 'built', name).read_bytes())
        for name, digest in SPHERE_FALLOFF.items():
            subprocess.run([dbgrab, str(original_db), 'get', name, f'{tmp}/original'], check=True, stdout=subprocess.DEVNULL)
            data = Path(tmp, 'original', name.replace('/', '__')).read_bytes()
            assert hashlib.sha256(data).hexdigest() == digest, f'not 0.11e\'s {name}'
            (out / 'files' / name).parent.mkdir(parents=True, exist_ok=True)
            (out / 'files' / name).write_bytes(data)
        twins += list(SPHERE_FALLOFF)
    pack = {
        'format_version': 1, 'id': 'v011e-radar-fog', 'version': '12', 'target_build': '0.26g',
        'description': "0.11e's atmosphere for exe patch v011w_render: its additive geometry fog in 0.26g's own "
                       "additive fog program (technique 4 of effect_atmospheric_scattering, which only the patch "
                       "draws), its scattering tables, sky, skylight, clouds and post as the twin families the patch compiles, "
                       "and its light cones (their program for vertex inputs 1 to 6 and textures/fx/sphere_falloff)",
        'requires': [], 'base_db_sha256': STOCK_DB_SHA,
        'operations': [{'op': 'replace_file', 'path': path, 'base_sha256': STOCK_SHA}]
                      + [{'op': 'add_file', 'path': name} for name in twins],
        'server': None,
        'notes': ['Built by srp-026g/scripts/v011-weather/make_fog_pack.py: 0.11e\'s programs compiled from '
                  'srp-026g/shaders/v011w/ for 0.26g\'s layout (v011w.sh build), each after the include records '
                  'of the 0.26g blob it replaces or twins.'],
    }
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(hashlib.sha256((out / 'pack.json').read_bytes()).hexdigest(), out)


if __name__ == '__main__':
    main()
