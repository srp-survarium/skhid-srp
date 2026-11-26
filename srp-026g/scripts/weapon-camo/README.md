# weapon-camo: the textures that let a weapon wear a camouflage

The rules and the engine: `srp-026g/docs/notes/weapon-camo.md`. The exe side is client patch 44
(`client-patches/weapon_camo_026g.asm`).

A camouflage reaches a weapon in three pieces, all made by `release/build-content` from the
content manifest (`release/content.tsv`), never by hand:

1. `<diffuse>_aoac.dds` (+ `.options`) beside each weapon diffuse. Since `remodel-069-1` every gun
   (all 103 weapon configs) draws its 0.69d0 remodel (`docs/notes/remodel-069.md`), whose textures
   are 0.69d0's: their masks are made inside the pack `weapon-modules` (`../weapon-modules/make_pack.sh`
   step 3, from `../remodel-069/camo.tsv`) and, for the textures the six weapons-069 guns brought,
   the pack `weapons-069-camo` (`../weapons-069/camo.tsv`). `weapon_camo.py` makes both by one rule
   and each table's `parts.tsv`. The tables of the guns' earlier models (`guns.tsv`, packs
   `weapon-camo`, `weapons-031-camo`, `weapons-046-camo`) are of textures no live gun draws any more:
   they follow the rule, and their packs stay as released;
2. `parameters/camo_available` and a `data` table (`camo_scale`) in the weapons' configs:
   `../content-edit/weapon-camo-1.json`, then `weapon-decals-1.json` (`decal_available` and
   `data/decals`: decals, client patch 47), the manifest's `edit` rows, applied in order;
3. the server reads both from its game data built of that patch (camouflage into slot 2 of a weapon).

The bake paints a weapon's first 8 LOD0 surfaces only (patch 44; #973 would make it 16): of the
remodels, the AKMSU's bolt, fire selector and wooden grip (11 surfaces; its bare body's receiver
cover), the HK416's magazine, the SCAR-H's front sight and the Glock 17's back cap stay plain
whatever their masks say. The decal UVs of `weapon-decal-uv` do not touch the masks: its bake reads
the diffuse, the mask and the camouflage through the material's own UVs and only writes into the
decal atlas. A fitted scope, muzzle device, grip or second sight (and the MPR45 rail) is a model of
its own (`weapons/modules_069`), not a surface of the gun: no bake, no mask, plain under a
camouflage (the owner's decision). The scope variants are retired (8d646466b).

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

## The review (2026-11-26)

The owner, of the rule alone: "it didn't do a good job, in some cases what we have now is better";
the right answer is where the game's own camouflaged versions paint. Every texture the live remodels
draw (237: `../remodel-069/camo.tsv` and `../weapons-069/camo.tsv`) was measured part by part
against its painted versions (0.69d0's `_camo` and premium textures, else its event skins; a part
counts as painted where one changed it: its mean blurred difference 0.02 and more, an untouched part
measures 0.005 and less), and so were the masks players had (weapon-modules@7's, each texture's
large parts) and the rule's. Each texture took whichever agreed better by area, then, under 95 %,
the parts the painted version decides clearly (a change of 0.03 and more, or 0.008 and less); a
texture with no painted version kept the masks players had but where the rule is clearly right (a
rod bare, large polymer or wood painted), and one players had plain (the cartridges) stays plain. The
choices are the `+`/`-` picks of the two `parts.tsv`. Agreement with the painted versions, by a
gun's area, on average: 67 % with the masks players had, 82 % by the rule, 95 % chosen. No painted
version: the MP-153, SOC Vepr, TT-33, Magnum, SV-98, AEK-919K, OTs-14, PM, CZ 75, Fort-17, MP-443
and TOZ-66. Only an event skin (30 guns, the A-545's, the SCAR-H's, the HK416's ...): it paints
where the skin does, barrels too.

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
python3 references.py game v069/overlay <vostok-port> guns.tsv ../weapons-031/camo.tsv \
    ../weapons-046/camo.tsv ../weapons-069/camo.tsv ../remodel-069/camo.tsv --write
```

(`references.py` imports `weapon_camo` from its own folder: no `-I`.) The game folder has the live
configs, so a weapons-069 texture that a remodel of another gun draws (the AKMN's magazine and
grip, the PPSh's body) gets that remodel's surfaces too, after its own.

## Making the packs

The remodels' masks are made by `../weapon-modules/make_pack.sh` (step 3, the same `weapon_camo.py`
call over `../remodel-069/camo.tsv` with `--v069`) into the next `weapon-modules`; the weapons-069
textures' by `../weapons-069/make_pack.sh` into the next `weapons-069-camo`. weapon-modules@8 is
@7's files with only its 410 mask files (205 `_aoac` and their `.options`) made again so, as step 3
alone makes them: the models, the modules table and every fit are @7's byte for byte.

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

2026-11-26, the players' files of revision 73 (packs `weapon-modules@8`, masks only, and
`weapons-069-camo@5`): the 237 textures the 103 guns' remodels draw, one `_aoac` each.

| table | textures | parts | painted | plain textures |
|---|---|---|---|---|
| `../remodel-069/camo.tsv` | 205 | 21638 | 6411 | 6 (cartridges, the MP7's glowing dots, the Colt's barrel, the Glock's back cap) |
| `../weapons-069/camo.tsv` | 32 | 3083 | 408 | 4 (cartridges, the loose shell) |

Agreement with the painted versions, by a gun's area (the review above): 67 % with the masks
players had, 82 % by the rule alone, 95 % chosen. Each gun drawn with its painted version, the
masks players had, the rule's and the chosen ones, its scores and the surfaces past the bake's 8th:
the review sheet of 2026-11-26 (software renders; not seen in the game).

2026-11-20, before the remodels were live (the PR's first version, of `guns.tsv` and the weapon
packs' tables, which no live gun draws now): 186 textures; the painted share of a gun 68 % with the
hand-picked masks, 81 % by the rule.
