# weapon-camo: the textures that let a weapon wear a camouflage

The rules and the engine: `srp-026g/docs/notes/weapon-camo.md`. The exe side is client patch 44
(`client-patches/weapon_camo_026g.asm`).

A camouflage reaches a weapon in three pieces, all made by `release/build-content` from the
content manifest (`release/content.tsv`), never by hand:

1. `<diffuse>_aoac.dds` (+ `.options`) beside each weapon diffuse: the content pack
   `weapon-camo@<version>` (the manifest's `pack` row), which `weapon_camo.py --pack` makes from
   `guns.tsv` and `parts.tsv` (the painted parts);
2. `parameters/camo_available` and a `data` table (`camo_scale`) in the weapons' configs:
   `../content-edit/weapon-camo-1.json`, then `weapon-decals-1.json` (`decal_available` and
   `data/decals`: decals, client patch 47), the manifest's `edit` rows, applied in order;
3. the server reads both from its game data built of that patch (camouflage into slot 2 of a weapon).

## Making the pack

A new pack version when guns.tsv, parts.tsv or the tool changes: its folder under
`$SKHID_PACKS_DIR` (`private/content-packs`) and its row in `release/content.tsv` (the name
`weapon-camo@<version>`, the sha256 of its pack.json), in the same PR.

```bash
# the players' files (the stock resources.db and every offered patch, as the game mounts them)
L="--db $SKHID_STOCK_DB"; for p in ~/.cache/skhid-release/patches/resources.db.patch_*; do L="$L --patch $p"; done
srp-game-data files $L | cut -f1 | grep -E '^(models/weapons/|textures/weapons/|textures/character/human/camo/)|black_market_01_torso_aoac.options$' >paths
srp-game-data get $L --out game --paths paths
# guns.tsv's 28: textures, from 0.28a2's resources.db
srp-game-data files --db <0.28a2 resources.db> | cut -f1 | grep '^textures/weapons/' >donor-paths
srp-game-data get --db <0.28a2 resources.db> --out donor --paths donor-paths

python3 -I weapon_camo.py guns.tsv game donor $SKHID_PACKS_DIR/weapon-camo-<version> --pack weapon-camo@<version>
python3 -I weapon_camo.py guns.tsv game donor - --previews previews \
    --camo game/textures/character/human/camo/premium/city_camo.dds    # look at every preview
```

The pack adds each `_aoac` (an `add_file`; a `replace_file` pinned by its sha256 for one the
players' files already have) and prints pack.json's sha256 for the row. Version 1 (2026-10-21,
the players' files of revision 26, 0.28a2's for the donors): 112 `_aoac` textures, 224 files,
pack.json `8e93b030…a132e2`. Version 3 (2026-11-01, weapons batch 1's four guns added to guns.tsv, their large parts
painted, sights bare; @2 was never released): 130 textures; built over the stock db and the base
patches of `release/content.tsv` (the game folder of those and the weapons-031 pack's files), so
the earlier `_aoac` are `add_file` as the composer expects, not `replace_file` of a live patch's.

Which parts are painted: `parts.tsv` (beside guns.tsv) names them per texture, a UV point on each;
a texture with no line gets the suggestion, the large parts its premium painted. To choose:

```bash
python3 -I weapon_camo.py guns.tsv game donor - --picker picker.html --camos camos \
    --camo game/textures/character/human/camo/premium/city_camo.dds
```

Open `picker.html` in a browser (about 50 MB, the textures and meshes inside; the 3D view loads
three.js r128 from cdnjs). The texture on the left, the weapon in 3D on the right, both showing
the camouflage as the game's bake makes it (overlay with the aoac's shading, tiled 4 times).
Click a part on either to paint it or leave it bare (a part of another texture of the same weapon
switches to it); drag turns the weapon, the wheel zooms, right-drag moves it. Grey surfaces have
no _aoac (barrels, sights of their own). The camouflage list is every camouflage item of the game
data (`--camos`, 50); "premium texture" shows the premium beneath, "Suggestion" and "None" set a
whole texture, "Reset" goes back to parts.tsv. Copy or download the lines into `parts.tsv` and
build again.

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

Result (2026-10-21, live data of revision 24): 91 textures, 112 `_aoac` files, all 83 weapon configs,
every texture from parts.tsv (22 picked by the owner, the rest after them by the same rule: the
body, not barrels, sights, rails, bolts or triggers). Before parts.tsv, the suggestion painted:

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
