#!/usr/bin/env python3
"""retexture.py: a material made again on a 0.26g template (vostok-port build-material-patch) given
the donor's textures, slot for slot: 0.69d0's PBR slots have other names than 0.26g's, so the
converter keeps the template's textures.

usage: python3 -I retexture.py <made material> <donor leaves>

<donor leaves>: `vostok-port dump-db-config --leaves` of the 0.69d0 material. Each 0.26g slot of
SLOTS takes the donor's texture of its 0.69d0 slot: albedo the diffuse, normal the normal,
smoothness 0.26g's roughness (its `_smap`), metalness 0.26g's fresnel (its `_fmap`). The material is
rewritten in place; a slot either side lacks is left as it is.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

SLOTS = {'texture_diffuse': 't_albedo', 'texture_normal': 't_normal',
         'texture_roughness': 't_smoothness', 'texture_fresnel': 't_metalness'}


def main():
    made, leaves = Path(sys.argv[1]), Path(sys.argv[2])
    donor = {}
    for line in leaves.read_text(encoding='utf-8').splitlines():
        key, _, value = line.partition('\t')
        parts = key.split('.')
        if len(parts) >= 2 and parts[-1] == 'value' and value.startswith('string:"'):
            donor[parts[-2]] = value[len('string:"'):-1]
    buf = bytearray(made.read_bytes())
    for ours, theirs in SLOTS.items():
        if not donor.get(theirs):
            continue
        try:
            off = ce.find(buf, f'material/g_stage/effect/{ours}/value')
        except KeyError:
            continue
        old = ce.value_of(buf, off)
        if old and old != donor[theirs]:
            if ce.replace_string(buf, old, donor[theirs]) < 1:
                sys.exit(f'{made}: {ours} not replaced')
            print(f'{made.name}: {ours} {old} -> {donor[theirs]}')
    made.write_bytes(bytes(buf))


if __name__ == '__main__':
    main()
