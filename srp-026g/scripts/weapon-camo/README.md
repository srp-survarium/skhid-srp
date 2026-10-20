# weapon-camo: the textures that let a weapon wear a camouflage

The rules and the engine: `srp-026g/docs/notes/weapon-camo.md`. The exe side is client patch 44
(`client-patches/weapon_camo_026g.asm`).

A camouflage reaches a weapon in three pieces, all in one content patch:

1. `<diffuse>_aoac.dds` (+ `.options`) beside each weapon diffuse that has a camouflaged premium
   texture: `weapon_camo.py` with `guns.tsv` and `parts.tsv` (the painted parts);
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

Which parts are painted: `parts.tsv` (beside guns.tsv) names them per texture, a UV point on each;
a texture with no line gets the suggestion, the large parts its premium painted. To choose:

```bash
python3 -I weapon_camo.py guns.tsv game donor - --picker picker.html \
    --camo game/textures__character__human__camo__premium__city_camo.dds
```

Open `picker.html` in a browser (about 15 MB, the textures inside). Click a part to paint it or
leave it bare; "premium texture" shows the premium beneath, "Suggestion" and "None" set a whole
texture, "Reset" goes back to parts.tsv. Copy or download the lines into `parts.tsv` and build again.

`--previews` writes, per texture: the plain texture, a premium donor, the mask, the aoac and the
bake the game makes (`bake()`, checked against the game's own bake of the AK-12 with the City
camouflage: the same tiling and shading). Look at every preview before a release.
weapon-camo-1.json names the configs whose surfaces use a texture of `out/` (made from the
survey of models, materials and configs; regenerate it when guns.tsv changes).

## guns.tsv

One texture a line: the texture, its donors (`28:` from 0.28a2; none for a weapon with no premium), the model surfaces whose UVs use
it, the premium textures of the same UVs that get the same aoac, and the plain texture the donors
were painted over when it is not ours (0.28a2 changed the Vityaz's, the AK-74N's and the
Lancaster's). A donor is usable only on our UVs: the same plain texture in its build, or a model
whose mesh UVs equal ours.

Result (2026-10-20, live data of revision 24): 91 textures, 111 `_aoac` files, all 83 weapon configs.
Painted by the suggestion (no parts.tsv yet), share of the texture's parts:

| texture | donors | parts painted | of the parts |
|---|---|---|---|
| uzi | uzi_camo_st, 0.28a2 uzi_camo | 6 of 81 | 50 % |
| sks | sks_camo_01, sks_camo_st | 3 of 132 | 57 % (the wooden stock) |
| akmn | akmn_camo, akmn_camo_st | 9 of 138 | 66 % |
| izh_27_lancaster | lancaster_camo_st, 0.28a2 lancaster_camo | 2 of 83 | 21 % (both stocks) |
| ak74_stock_wood (AK-74N) | ak74_stock_wood_camo_st, 0.28a2 _camo | 3 of 10 | 83 % |
| ak_12 | ak_12_camo | 7 of 254 | 53 % |
| the other 20 | one each | 2–13 parts | 49–96 % (stocks, magazines and barrel covers: few parts, most painted) |
| 65 with no premium (the rest, 0.29a3 ports and pistols included) | none | 3–16 parts | 40–100 % (the large parts; a barrel too on the TOZ-66) |

Not in guns.tsv: textures of barrels, sights, suppressors and attachments of their own, the
12-gauge shell, the SV-98's net, the premium-only `_camo` textures, the mp7's glow (no texture).
The Vepr-12's magazine has no _aoac (its premium repainted under 1 % of it).
