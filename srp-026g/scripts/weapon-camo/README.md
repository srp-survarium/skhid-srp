# weapon-camo: the textures that let a weapon wear a camouflage

The rules and the engine: `srp-026g/docs/notes/weapon-camo.md`. The exe side is client patch 44
(`client-patches/weapon_camo_026g.asm`).

A camouflage reaches a weapon in three pieces, all made by `release/build-content` from the
content manifest (`release/content.tsv`), never by hand:

1. `<diffuse>_aoac.dds` (+ `.options`) beside each weapon diffuse: the content packs
   `weapon-camo@<version>` (the stock and 0.29a3 guns, `guns.tsv`) and `weapons-031-camo`,
   `weapons-046-camo`, `weapons-069-camo` (the ported guns, `../weapons-0xx/camo.tsv`), which
   `weapon_camo.py --pack` makes of each table by one rule;
2. `parameters/camo_available` and a `data` table (`camo_scale`) in the weapons' configs:
   `../content-edit/weapon-camo-1.json`, then `weapon-decals-1.json` (`decal_available` and
   `data/decals`: decals, client patch 47), the manifest's `edit` rows, applied in order;
3. the server reads both from its game data built of that patch (camouflage into slot 2 of a weapon).

A scope's variant of a weapon (`models/weapons/attach`, `coverage`) draws the weapon's own textures
on a part of its UVs, so it wears the weapon's masks (2026-11-20: every one of 2771 surfaces; the
scoped Mosin has a texture of its own, which gets the Mosin's mask).

## Which parts are painted

One rule for every gun (`choose()`), part by part. A part is what one UV island's texels cover on
the weapon (`parts_of()`: halves mirrored over one another in UV are one part), measured on the
model: its area, its length along the bore, where it is between butt and muzzle, and the radius of
the tube it is, if it is one (`tube()`: its corners on a circle about an axis along the bore at each
place along it, so a barrel may taper; its facets near that circle, so a box is none; corners all
around the axis, so a slide or a receiver cover, round above and open below, is none).

1. Under 3 cm²: bare (pins, screws, triggers, sight blades).
2. Not metal by 0.69d0's metalness map of the same UVs (polymer, wood, bakelite): painted.
3. A metal tube under 1.4 cm of radius, not at the butt (a wire stock's struts are painted): a rod,
   bare (barrels and their pieces between sight bases, gas tubes, pistons, cleaning rods).
4. Metal of 10 cm² and more: the body, painted (receivers, covers, slides, rails, metal stocks);
   but a slim tube (under 2 cm, twice as long as wide: a heavy barrel, a suppressor, a magazine
   tube) only where a painted version of the weapon paints it (the VSS's, the Val's and the
   9A-91's suppressors).
5. Smaller metal parts (3 to 10 cm²): painted where a painted version of the weapon repaints half
   of the part.

The painted versions are the references: 0.69d0's hand-painted textures of the same weapon (its
`_camo` and event textures), our premiums and 0.28a2's. 0.69d0 has no camouflage on weapons and so
no masks to take; what it has is those textures and its metalness maps, on the same UVs as ours:
our weapon meshes are 0.69d0's (`references.py` finds, for each texture of ours, the 0.69d0 surfaces
whose UV points lie on ours, whatever the textures are called: 174 of 181 rows on 2026-11-20; the
old Uzi, the SPAS-12, the TOZ-66, the M4A1 and three more have other meshes or layouts and are
judged by size and shape alone). A texture with no metalness map counts as metal throughout.

`parts.tsv` (beside each table) is the last resort, a line a texture: `+u,v` paints the part at
that UV point, `-u,v` leaves it bare, over the rule; a texture with no points stays plain (the
ported guns' cartridges, sights and muzzle devices with textures of their own: 17 textures). The
picker page writes the lines. The picks (2026-11-20, ten textures), each said in its file: bare,
the double barrels of the TOZ-34, the TOZ-66 and the Lancaster (two barrels in one part are no
tube), the SPAS-12's magazine tube, the SKS's cartridges, the MP7's stock rods, the SVU-A's muzzle
device; painted, the Neostead's two magazine tubes (its top; its premium paints them), the
TOZ-66's forend and receiver, the barrel jackets of the two PPShs.

## The tables

`guns.tsv` and the packs' `camo.tsv`, one texture a line (the header says the columns): the texture,
its painted versions (`<painted>@<the plain texture it was painted over>`; `28:` 0.28a2's, `69:`
0.69d0's), the model surfaces whose UVs use it, the premium textures of the same UVs that get the
same mask, the plain texture for painted versions without an `@`, and the metalness map (`69:`).
`references.py` writes the `69:` columns and adds the surfaces of every weapon config's model, and
says which textures those models draw that no table has: barrels, sights, suppressors and
attachments with textures of their own (never painted), the 12-gauge shell, the SV-98's net.

```bash
# 0.69d0's weapon files (a vostok-port that reads its compressed volumes)
vostok-port extract-bundle --db <0.69d0 .split> --prefix textures/weapons/ --prefix models/weapons/ \
    --prefix material_instances/weapons/ --output v069          # v069/overlay: the folder below
python3 -I references.py game v069/overlay <vostok-port> guns.tsv ../weapons-031/camo.tsv \
    ../weapons-046/camo.tsv ../weapons-069/camo.tsv --write
```

## Making the packs

A new version of each pack when its table, its parts.tsv or the tool changes: its folder under
`$SKHID_PACKS_DIR` (`private/content-packs`) and its row in `release/content.tsv` (the name
`<id>@<version>`, the sha256 of its pack.json), in the same PR.

```bash
# the players' files (the stock resources.db and every offered patch, as the game mounts them),
# without the masks earlier versions of the packs added (each is an add_file again)
L="--db $SKHID_STOCK_DB"; for p in ~/.cache/skhid-release/patches/resources.db.patch_*; do L="$L --patch $p"; done
srp-game-data files $L | cut -f1 | grep -E '^(models/weapons/|textures/weapons/|textures/character/human/camo/|material_instances/|gameplay/items/weapons/)|black_market_01_torso_aoac.options$' \
    | grep -vE '^textures/weapons/.*_aoac\.' >paths
srp-game-data get $L --out game --paths paths
# guns.tsv's 28: textures, from 0.28a2's resources.db
srp-game-data files --db <0.28a2 resources.db> | cut -f1 | grep '^textures/weapons/' >donor-paths
srp-game-data get --db <0.28a2 resources.db> --out donor --paths donor-paths

python3 -I weapon_camo.py guns.tsv game donor $SKHID_PACKS_DIR/weapon-camo-<version> --v069 v069/overlay \
    --pack weapon-camo@<version> --bare-masks
python3 -I weapon_camo.py ../weapons-031/camo.tsv game donor $SKHID_PACKS_DIR/weapons-031-camo-<version> \
    --v069 v069/overlay --pack weapons-031-camo@<version> --bare-masks      # and -046, -069
python3 -I weapon_camo.py guns.tsv game donor - --v069 v069/overlay --previews previews \
    --camo game/textures/character/human/camo/premium/city_camo.dds    # look at every preview
```

The pack adds each `_aoac` (an `add_file`; a `replace_file` pinned by its sha256 for one the game
folder already has) and prints pack.json's sha256 for the row. `--bare-masks`: a texture with
nothing painted still gets an `_aoac`, one that paints nothing, so every surface of a camouflaged
weapon finds its mask and one a release painted earlier is bare again. It prints, per texture, the
parts, how many are painted and why, and the painted share of the surface.

To pick by hand:

```bash
python3 -I weapon_camo.py guns.tsv game donor - --v069 v069/overlay --picker picker.html --camos camos \
    --camo game/textures/character/human/camo/premium/city_camo.dds
```

Open `picker.html` in a browser (about 50 MB, the textures and meshes inside; the 3D view loads
three.js r128 from cdnjs). The texture on the left, the weapon in 3D on the right, both showing
the camouflage as the game's bake makes it (overlay with the aoac's shading, tiled 4 times).
Click a part on either to paint it or leave it bare (a part of another texture of the same weapon
switches to it); drag turns the weapon, the wheel zooms, right-drag moves it. Grey surfaces have
no _aoac (barrels, sights of their own). The camouflage list is every camouflage item of the game
data (`--camos`, 50); "premium texture" shows a painted version beneath, "Rule" and "None" set a
whole texture, "Reset" goes back to the rule with parts.tsv's picks. Copy or download the lines
into `parts.tsv` and build again.

`--previews` writes, per texture: the plain texture, a painted version, the mask, the aoac and the
bake the game makes (`bake()`, checked against the game's own bake of the AK-12 with the City
camouflage: the same tiling and shading). Look at every preview before a release.
weapon-camo-1.json names the configs whose surfaces use a texture of `out/` (made from the
survey of models, materials and configs; regenerate it when guns.tsv gains a weapon).

## Result

2026-11-20, the players' files of revision 68 (packs `weapon-camo@4`, `weapons-031-camo@3`,
`weapons-046-camo@3`, `weapons-069-camo@4`): 186 textures, 207 `_aoac` files with the premium copies,
all 103 weapon configs.

| table | textures | parts | painted | plain textures |
|---|---|---|---|---|
| `guns.tsv` | 96 | 11344 | 3208 | none |
| `../weapons-031/camo.tsv` | 20 | 2653 | 584 | 2 (sights) |
| `../weapons-046/camo.tsv` | 38 | 3549 | 674 | 12 (sights, barrels, muzzle devices, cartridges; the Colt's trigger group by the rule) |
| `../weapons-069/camo.tsv` | 32 | 3083 | 553 | 4 (cartridges, the loose shell) |

Of the painted parts about 2100 are the body, 1750 not metal and 1200 small parts or slim tubes a
painted version paints. Measured on the models (each gun's whole surface, hidden faces too): the
painted share of a gun was 68 % on average with the masks before (the hand-picked parts of
`weapon-camo@1`, the ported guns' large parts) and is 81 %; 91 guns are painted on more, 11 on less
(barrels above all: the double-barrel shotguns, the TT, the TOZ-122, the KS-23M; the Kedr, the
FN FNC, the Colt and the PP-2000 by a few points). Of what the
painted versions repaint (88 guns have one) the masks painted 75 % before and 88 % now. Each gun
drawn with the masks before and after beside its painted version: the page
`private/renders/weapon-camo-coverage/index.html` of the main checkout (software renders; not seen
in the game).
