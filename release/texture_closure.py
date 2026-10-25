#!/usr/bin/env python3
"""texture_closure.py: every texture a content pack's particles draw is in players' files.

usage: python3 -I texture_closure.py --vostok-port <program> --db <stock resources.db>
                                     [--patch <resources.db.patch_n>]... --pack <pack folder>...

The particles checked are a pack's own (particles/...) and every particle its configs name (a
string of any .options file of the pack that is a particle players have: object/particle_on_spawn,
a damage zone's idle_particle, an effect's particle, ...). For each particle, each emitter (a
node whose properties name a Material) and its material instance (material_instances/<name>.material,
the pack's or players'), every texture the material names (a string value with a '/': its
texture slots, transparency, emission, distortion, normal; `test/test` is the engine's own) must
be textures/<name>.dds in the stock db, the patches (laid in the order given; the content build
passes the list's and the new one) or the pack.

A missing material or texture of an enabled emitter fails (exit 1): the game draws such a
billboard untextured, its colour ramp as a plain square (the Sponge's spawn, 2026-10-25: 0.26g's
own magic_smoke materials name textures no archive of 0.26g holds). Of a disabled emitter it is
only listed. ALLOWED holds what is proven harmless.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'srp-026g' / 'scripts' / 'research-mode'))
import bcw  # noqa: E402

# (material, texture): harmless although no archive holds the texture.
ALLOWED = {
    # 0.26g's own fx/anomaly/light_idle (the light emitters), fx/anomaly/hot_distortion and
    # fx/explosions/distortion_wave (distortions) name skin/skin_shadow as their transparency,
    # and no archive of 0.26g (or 0.69d0, the Sponge's retail) holds it; 0.26g's own particles
    # draw those emitters, enabled, on every map: the Lifebone's, Spring's and Onyx's emergence
    # (light and distortion) and carry particles, the electro anomaly, bonfires, lamps, torches
    # (light_idle), every grenade's explosion, grenade_exp (exp_distortion: distortion_wave).
    ('fx/anomaly/light_idle', 'skin/skin_shadow'),
    ('fx/anomaly/hot_distortion', 'skin/skin_shadow'),
    ('fx/explosions/distortion_wave', 'skin/skin_shadow'),
}


class Archives:
    """Players' files: the stock db and the patches over it, the last that has a path wins."""

    def __init__(self, vostok, layers):
        self.vostok, self.layers, self.where, self.cache = vostok, layers, None, {}
        self.tmp = tempfile.mkdtemp(prefix='texture-closure-')

    def listed(self):
        """the archives' listings, read once, when a pack needs them (a pack without particles never)"""
        if self.where is None:
            self.where = {}
            for layer in self.layers:
                listing = subprocess.run([self.vostok, 'list', '--db', layer], check=True, capture_output=True,
                                         text=True)
                for path in listing.stdout.split():
                    self.where[path] = layer
        return self.where

    def has(self, path):
        return path in self.listed()

    def read(self, path):
        if path not in self.cache:
            out = os.path.join(self.tmp, str(len(self.cache)))
            subprocess.run([self.vostok, 'extract', '--db', self.listed()[path], '--path', path, '--output', out],
                           check=True, capture_output=True)
            self.cache[path] = Path(out).read_bytes()
        return self.cache[path]


def strings(value, key=None):
    if isinstance(value, dict):
        for name, child in value.items():
            yield from strings(child, name)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child, key)
    elif isinstance(value, str):
        yield key, value


def emitters(node, out):
    """(name, enabled, material) of each node whose properties name a Material."""
    if isinstance(node, dict):
        props = node.get('properties')
        if isinstance(props, dict) and isinstance(props.get('Material'), str):
            out.append((props.get('Name', '?'), props.get('Enabled', True) is not False, props['Material']))
        for child in node.values():
            emitters(child, out)
    elif isinstance(node, list):
        for child in node:
            emitters(child, out)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--vostok-port', required=True)
    parser.add_argument('--db', required=True)
    parser.add_argument('--patch', action='append', default=[])
    parser.add_argument('--pack', action='append', default=[], type=Path)
    args = parser.parse_args()
    archives = Archives(args.vostok_port, [args.db] + args.patch)
    failed, listed = [], []

    for pack in args.pack:
        files = pack / 'files'

        def have(path):
            return (files / path).is_file() or archives.has(path)

        def read(path):
            return (files / path).read_bytes() if (files / path).is_file() else archives.read(path)
        particles = {str(p.relative_to(files))[len('particles/'):-len('.particle')]
                     for p in (files / 'particles').rglob('*.particle')} if (files / 'particles').is_dir() else set()
        for config in files.rglob('*.options'):
            try:
                value = bcw.loads(config.read_bytes())
            except ValueError:
                continue
            for _, text in strings(value):
                if '/' in text and have(f'particles/{text}.particle'):
                    particles.add(text)
        for particle in sorted(particles):
            path = f'particles/{particle}.particle'
            if not have(path):
                failed.append(f'{pack.name}: {path}: no such particle')
                continue
            for name, enabled, material in emitters(bcw.loads(read(path)), []):
                where = f'{pack.name}: {particle} emitter {name} ({"enabled" if enabled else "disabled"}), material {material}'
                mpath = f'material_instances/{material}.material'
                if not have(mpath):
                    (failed if enabled else listed).append(f'{where}: no material')
                    continue
                textures = sorted({t for _, t in strings(bcw.loads(read(mpath))) if '/' in t and t != 'test/test'})
                for texture in textures:
                    if have(f'textures/{texture}.dds'):
                        continue
                    note = f'{where}: no texture {texture}'
                    if (material, texture) in ALLOWED:
                        listed.append(note + ' (allowed: 0.26g\'s own effects draw it so)')
                    elif enabled:
                        failed.append(note)
                    else:
                        listed.append(note + ' (the emitter is disabled)')
        print(f'{pack.name}: {len(particles)} particles checked')
    for line in sorted(set(listed)):
        print('  --', line)
    for line in failed:
        print('  FAIL', line)
    if failed:
        sys.exit(f'{len(failed)} texture(s) or material(s) missing')
    print('every texture of every enabled emitter is in players\' files or the packs')


if __name__ == '__main__':
    main()
