#!/usr/bin/env python3
"""make_pack.py: 0.69d's camouflage maps for the released vests and trousers whose donor had none
(the owner, 2026-11-26; ../../docs/notes/equipment-069.md, Camouflage).

usage: python3 -I make_pack.py --bundle <folder> --out <packs folder>/armour-camo-069d-<version>
       python3 -I make_pack.py --check <live folder> --pack <pack folder> [--pack ...]

--bundle  resource-porter's `vostok-port build-camo-maps-069d` output (manifest
          ports/survarium-069d-camo-maps-to-026g.json): overlay/, port-manifest.json and
          camouflage-report.json, every file pinned by BUNDLE's sha256.
--out     the pack's folder (a new one): pack.json and files/, for the manifest's `pack` row.
--check   the camouflage guard over a composed game: <live folder> holds every armour config,
          model settings and material of the released game at their game paths (`srp-game-data get`),
          each --pack's files are laid over it in order; every config with camo_available must have
          `<diffuse>_aoac.dds` (live or in a pack) for the surface each data section wears. Prints the
          ones that have not and exits 1 if any.

The game bakes a camouflage into a surface only when `<diffuse>_aoac.dds` loads beside its diffuse
(weapon-camo.md, "The engine"). Seven camouflage-capable pieces had none, their donors having none:
equipment-046's Sniper vest and pants (renesanse_sniper_07, 0.31a2) and eq046_edge_torso_7
(edge_keeper_07_torso), and equipment-029's edge_torso_8 and edge_legs_8 (edge_fighter_02),
renesanse_torso_8 and renesanse_legs_8 (renesanse_vdv) and scavenger_torso_6_premium
(scavenger_05_sniper_torso_prem). 0.69d ships the four camouflage maps of each at the same paths; the
porter checks that a surface drawing each map has 0.69d's mesh and UVs (the paintings differ, the
layouts do not) and converts them as it converts a ported piece's. The pack adds them; nothing else
changes (as the weapons' *-camo packs beside their weapons' packs).
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

PACK_ID = 'armour-camo-069d'
VERSION = '1'
# The porter's bundle, by sha256 (resource-porter #101, feat/camo-maps-069d a533b98).
BUNDLE = {
    'camouflage-report.json': 'a8161eea831158aa586004ec8b9e673efa3fbd998e60c22728e5e84d0476139c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_aoac.dds': 'f40fb621ae32633bdc0339d8c7af5c9c78a5933d3b11608f11595e1d81a88e1c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo.dds': '98f7cac27cb03fff4261bba40f6eea66fa50e5032295c2d60a62619266c561a8',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo_nmap.dds': '07faffd81a1bfd15f709217a4de15479a1f849d52f9a1454de12e39fb8003922',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo_smap.dds': '33a811e58f18b2e0409e4255f76e05489daece9eac8126374b2076e9c442904f',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_legs_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_aoac.dds': 'cc69444b7a68f54b7dea480ce998a2326ecda9758222bc2e324e572435af2aba',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo.dds': 'c4094f12cad50e913a421b27188b3f5a09a0d7d0bfc62c13d1c4f66890c9ba37',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo_nmap.dds': 'ec6dc7737a12bf65083610ddef5f0c0fc3fb772af53bd6a9ec27c248cc8fffcb',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo_smap.dds': '642fd8a5ed64ef489cd365303be62f6985dfb24c47d6598ef37291ee2161f542',
    'overlay/textures/character/human/edge_fighter_02/edge_fighter_02_torso_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_aoac.dds': 'ef57a70fde43f7ba857d7cf44d2b74fe0388a9136abd9193c8e0219cf621da85',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo.dds': 'fd3a0b60b9b26f865810db02a0c532da13b2ff8f6f8fce3241a51d49f203ea17',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo_nmap.dds': '33507c764e81fb643804f24e1e54f3426d8958cb543228fcc2d8e34023a785c9',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo_smap.dds': '64ded7fb1770915dd53c87b639234b6b3ba835ccea8969a12b310d4ad38752de',
    'overlay/textures/character/human/edge_keeper_07/edge_keeper_07_torso_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_aoac.dds': 'd5ade96603e2d862dfeb1bd34c47f939b884967a20de367b9cdfa44a2409aaf4',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo.dds': '91307d1ccc7878b4dc1703d5de681b892f506ba8dbf2028878cf0ece9fb207f4',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo_nmap.dds': 'dc5e1f1e2956bd804ff024ff7dbcab62a1342c151b99e836c9d37714ab2632f7',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo_smap.dds': 'b2a3afa0030ed4119595891e15507637f5dbbccaef16886529908876b6e239ed',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_legs_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_aoac.dds': 'b4dd43776fd7d30c79534f9a6fa4cba7e53eaef0391f19cb23c750091e3474d5',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo.dds': '14e6377edbde3a02cba9987d22c48e05573362f0219f6fae30d51f5042bed0d5',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo_nmap.dds': 'dc5e1f1e2956bd804ff024ff7dbcab62a1342c151b99e836c9d37714ab2632f7',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo_smap.dds': '9386fa01c585227708d3987ce32b0da6c2953dfaa7e05d59cd5695fdf927d70b',
    'overlay/textures/character/human/renesanse_sniper_07/renesanse_sniper_07_torso_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_aoac.dds': '3a85fd286b5de6c9d46f0cfc8cb52312eed22efeb991c88bcc50a488e866b906',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo.dds': '07686091fac16f612428f077fa16e33e7002e55bab72127c70f4c936fc3e65e1',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo_nmap.dds': 'f39b973bf45db0c66e09f87a6ed220e971f1c87d78a07d38f1948040e0b50984',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo_smap.dds': 'c3a3643be69ea1a86721d517a9deb0b67f652abb9976db0cc91d8a30b31cdf7a',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_legs_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_aoac.dds': '2aecea78aa26ab2c883e1603a941aad77e37ef712c48ee25143323847984935b',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo.dds': '07686091fac16f612428f077fa16e33e7002e55bab72127c70f4c936fc3e65e1',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo_nmap.dds': 'f39b973bf45db0c66e09f87a6ed220e971f1c87d78a07d38f1948040e0b50984',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo_smap.dds': 'c3a3643be69ea1a86721d517a9deb0b67f652abb9976db0cc91d8a30b31cdf7a',
    'overlay/textures/character/human/renesanse_vdv/renesanse_vdv_torso_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_aoac.dds': 'cff148a260df14001441ef5e23bc6f729fe8fb7526d2f5b7aebfb7564846741a',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_aoac.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo.dds': 'a8a7d6a057394cc6a5bc66cb3ac9ec323c2f3b0869a5218de4a52f56971226e7',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo_nmap.dds': 'dfa5dd2947b536c7965f1d28de6da4ec1fba2e05848ec2bd85dc415d1ec583a7',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo_nmap.options': '6788ca0ae343af6ad0e862e9c37271d62b053dce82aabb6062f675b43deba87c',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo.options': '3e9c726f7109d5995cbb23199f35149eb5096e94a6b29b70d94debbfa757848c',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo_smap.dds': '967713e33e68dc28ac11ea9310d1ddb469e61416249a850219cef7f4d8bb4ab4',
    'overlay/textures/character/human/scavenger_05_sniper/scavenger_05_sniper_torso_prem_camo_smap.options': '766600edd7cb8e031f8d9420ac57bad10e2f78f1c4ce8a6f4dcec07259c84ef0',
    'port-manifest.json': '47a7bad092a331eea5140b2965915d34a9d70862da0ce5fdb36fd2ddc96576bb',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def make(bundle, out):
    if out.exists():
        sys.exit(f'{out} exists')
    on_disk = {p.relative_to(bundle).as_posix() for p in bundle.rglob('*') if p.is_file()}
    if on_disk != set(BUNDLE):
        sys.exit(f'the bundle is not the pinned one: {sorted(on_disk ^ set(BUNDLE))[:4]}')
    files = {}
    for path, digest in sorted(BUNDLE.items()):
        data = (bundle / path).read_bytes()
        if sha(data) != digest:
            sys.exit(f'{path}: not the pinned bytes')
        if path.startswith('overlay/'):
            files[path.removeprefix('overlay/')] = data
    report = json.loads((bundle / 'camouflage-report.json').read_text())
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': "0.69d's camouflage maps (_aoac, _camo, _camo_nmap, _camo_smap) for the released vests "
                       'and trousers whose donor had none: ' + ', '.join(r['texture'].split('/')[-1] for r in report),
        'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
    }
    for path, data in files.items():
        target = out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(f'{out}: {len(files)} files; pack.json {sha((out / "pack.json").read_bytes())}')


def check(live, packs):
    """Every camouflage-capable config's worn surfaces have their diffuse's _aoac."""
    def find(path):
        for pack in reversed(packs):
            if (pack / 'files' / path).exists():
                return pack / 'files' / path
        return live / path if (live / path).exists() else None

    configs = sorted({p.relative_to(root / 'files' if root in packs else root).as_posix()
                      for root in [live, *packs]
                      for p in (root / 'files' if root in packs else root).glob('gameplay/items/armour/*/*.options')})
    missing, checked = [], 0
    for path in configs:
        config = bcw.loads(find(path).read_bytes())
        if not config.get('parameters', {}).get('camo_available'):
            continue
        for key, section in config.items():
            if not (key == 'data' or key.startswith('data_')) or 'base_model' not in section:
                continue
            settings = bcw.loads(find(f"models/{section['base_model']}.skinned_model/settings").read_bytes())
            material = settings['material_settings'][section['part_name']]['material_name']
            effect = bcw.loads(find(f'material_instances/{material}.material').read_bytes())['material']['g_stage']['effect']
            aoac = f"textures/{effect['texture_diffuse']['value']}_aoac.dds"
            checked += 1
            if find(aoac) is None:
                missing.append(f'{path}: {section["base_model"]} {section["part_name"]}: no {aoac}')
    for line in sorted(set(missing)):
        print(line)
    print(f'{len(configs)} armour configs, {checked} worn surfaces of camouflage-capable ones checked, '
          f'{len(set(missing))} without a mask')
    return not missing


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--bundle', type=Path)
    p.add_argument('--out', type=Path)
    p.add_argument('--check', type=Path, metavar='LIVE')
    p.add_argument('--pack', type=Path, action='append', default=[])
    a = p.parse_args()
    if a.check:
        sys.exit(0 if check(a.check, a.pack) else 1)
    if not (a.bundle and a.out):
        p.error('--bundle and --out, or --check')
    make(a.bundle, a.out)


if __name__ == '__main__':
    main()
