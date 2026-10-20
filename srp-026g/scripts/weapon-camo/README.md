# weapon-camo: the textures that let a weapon wear a camouflage

The rules and the engine: `srp-026g/docs/notes/weapon-camo.md`. The exe side is client patch 44
(`client-patches/weapon_camo_026g.asm`).

A camouflage reaches a weapon in three pieces, all in one content patch:

1. `<diffuse>_aoac.dds` (+ `.options`) beside each weapon diffuse that has a camouflaged premium
   texture: `weapon_camo.py` with `guns.tsv`;
2. `parameters/camo_available` and a `data` table (`camo_scale`) in those weapons' configs:
   `../content-edit/content_edit.py` with `../content-edit/weapon-camo-1.json`;
3. the server reads both from its game data built of that patch (camouflage into slot 2 of a weapon).

## Making the patch

```bash
# the players' files: the stock resources.db with every offered patch over it, flattened
for db in resources.db resources.db.patch_12 ... resources.db.patch_<live>; do
  for p in models/weapons/ material_instances/weapons/ textures/weapons/; do dbgrab $db get $p layer; done
  cp -f layer/* game/                                  # later patches over earlier ones
done
dbgrab resources.db get black_market_01_torso_aoac.options game     # the stock _aoac .options
dbgrab <0.28a2 resources.db> get textures/weapons/ donor            # guns.tsv's 28: textures
dbgrab server-game-data.db get gameplay/items/weapons/ live         # the live weapon configs

python3 -I weapon_camo.py guns.tsv game donor out --previews previews \
    --camo game/textures__character__human__camo__premium__city_camo.dds
python3 -I ../content-edit/content_edit.py ../content-edit/weapon-camo-1.json live folder
cp -r out/textures folder/
release/build-patch folder <revision>
```

`--previews` writes, per texture: the plain texture, a premium donor, the mask, the aoac and the
bake the game makes (`bake()`, checked against the game's own bake of the AK-12 with the City
camouflage: the same tiling and shading). Look at every preview before a release.
weapon-camo-1.json names the configs whose surfaces use a texture of `out/` (made from the
survey of models, materials and configs; regenerate it when guns.tsv changes).

## guns.tsv

One texture a line: the texture, its donors (`28:` from 0.28a2), the model surfaces whose UVs use
it, the premium textures of the same UVs that get the same aoac, and the plain texture the donors
were painted over when it is not ours (0.28a2 changed the Vityaz's, the AK-74N's and the
Lancaster's). A donor is usable only on our UVs: the same plain texture in its build, or a model
whose mesh UVs equal ours.

Result (2026-10-20, live data of revision 24): 26 textures, 46 `_aoac` files, 43 weapon configs.

| texture | donors | painted |
|---|---|---|
| uzi | uzi_camo_st, 0.28a2 uzi_camo | 52 % |
| sks | sks_camo_01, sks_camo_st | 49 % (the wooden stock) |
| akmn | akmn_camo, akmn_camo_st | 74 % |
| izh_27_lancaster | lancaster_camo_st, 0.28a2 lancaster_camo | 16 % (both stocks) |
| ak74_stock_wood (AK-74N) | ak74_stock_wood_camo_st, 0.28a2 _camo | 70 % |
| akm_stock_wood, ak_12, aks74u, ak74n, kiparis, vityaz, neostead, rem700, saiga_12s, ak74_stock_plastic, vepr12, vepr_main, vepr_sec, vsk_9a91, vssk, vssk_brl, vss, vss_stock_wood, sr3m, sr3_mag, stock_val | one each | 25–84 % |

Not covered: the Vepr-12's magazine texture (its premium repainted under 1 %), the AK-12's magazine
(our AK-12 has no magazine surface), the 0.29a3 weapons, weapons with no premium.
