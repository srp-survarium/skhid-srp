#!/usr/bin/env python3
"""retexture.py: a 0.69d0 material made again as 0.26g's: a 0.26g template material (materials.tsv)
whose four texture slots take the donor's textures, slot for slot, and nothing else changed.

usage: python3 -I retexture.py <template material> <donor leaves> <out material>

<donor leaves>: `vostok-port dump-db-config --leaves` of the 0.69d0 material. Each 0.26g slot of
SLOTS takes the donor's texture of its 0.69d0 slot: albedo the diffuse, normal the normal,
smoothness 0.26g's roughness slot (its `_smap`, a gloss), metalness 0.26g's fresnel (its `_fmap`);
pbr_026.py converts the textures' values. Every other value stays the template's, a stock scope's:
the rifle scopes @1..@4 made them with vostok-port build-material-patch, which carried 0.69d0's
values over the template's, and the made materials had constant_roughness_min and _max both 0.2 (the
template's 0 and 1), so 0.26g's gloss texture had no effect (its g-buffer pass reads the roughness
slot as min + (max - min) * texture).
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

SLOTS = {'texture_diffuse': 't_albedo', 'texture_normal': 't_normal',
         'texture_roughness': 't_smoothness', 'texture_fresnel': 't_metalness'}


def donor_slots(leaves_text):
    """The donor material's texture of each 0.69d0 slot (`dump-db-config --leaves`)."""
    donor = {}
    for line in leaves_text.splitlines():
        key, _, value = line.partition('\t')
        parts = key.split('.')
        if len(parts) >= 2 and parts[-1] == 'value' and value.startswith('string:"'):
            donor[parts[-2]] = value[len('string:"'):-1]
    return donor


def made(template, donor):
    """`template` (bytes) with each slot of SLOTS set to the donor's texture (`donor`: 0.69d0 slot ->
    texture); a slot either side lacks is an error."""
    buf = bytearray(template)
    for ours, theirs in SLOTS.items():
        if not donor.get(theirs):
            raise ValueError(f'the donor material binds no {theirs}')
        off = ce.find(buf, f'material/g_stage/effect/{ours}/value')
        _, i, c, t, _ = ce.rec(buf, off)
        if t != ce.T_STR or not ce.value_of(buf, off):
            raise ValueError(f'the template binds no {ours}')
        text = donor[theirs].encode()
        ce.REC.pack_into(buf, off, ce.append(buf, text + b'\0'), i, c, t, len(text) + 1)
    return bytes(buf)


def main():
    template, leaves, out = (Path(a) for a in sys.argv[1:4])
    out.write_bytes(made(template.read_bytes(), donor_slots(leaves.read_text(encoding='utf-8'))))


if __name__ == '__main__':
    main()
