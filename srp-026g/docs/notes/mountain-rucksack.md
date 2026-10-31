# German mountain rucksack (pack mountain-rucksack)

The Bundeswehr mountain troops' rucksack (Gebirgsjägerrucksack), as Mil-Tec sells it
(14016001, olive, 25 l, 32 x 21 x 40 cm). No build of Survarium has it, and no usable
model of it exists, so it is modelled for 0.26g. It ships in the content pack
`mountain-rucksack@2` (`scripts/mountain-rucksack`). The item's numbers are proposals:
balance is the owner's call.

## The item

- Id 1877, `srp_mountain_rucksack`, back slot.
  - 1860, the id first asked for, is live: decals-069d took 1824-1862, and 1863 is taken too.
  - The STK2 outfits work in progress takes 1864-1876; 1877 is the next free id.
- No faction: the Independent traders', as the Novice's knit cap (1821). Level 14.
- Drop only (the owner, 2026-10-31: all the new equipment):
  - sold by no one. No `prices.rs` row names it, and the rule sells a piece of armour
    only by its config's faction; nothing else sells it;
  - it drops after matches: the generic drop table's back pieces (category 4, by level),
    at level 14 (match levels 14-20), drop weight 20 as the other level-14 backpacks;
  - tests: `prices::the_mountain_rucksack_is_sold_by_no_one`, and on data with the pack
    `drops::the_mountain_rucksack_drops_and_is_sold_by_no_one` (ignored; `SRP_GAME_DATA`).
- Everything else is the Assault fighter's backpack's (`ren_back_1`, 115), copied:
  - 8 kg, drop weight 20, `base_cost` 55,250 (what selling it back and repairs go by);
  - its built-in bonuses (1031, 1171), modification and defect masks, repair, sound.
- Name: "German mountain rucksack"; Russian "Горно-егерский рюкзак".
  - Ten catalog TSVs, keys `st_srp_mountain_rucksack_name` / `_desc`.
  - Description: one line, the frameless 25-litre rucksack of the Bundeswehr's mountain troops.
- Icon: model.py's render, equipment atlas row 16, column 8 (equipment-069 has rows 14
  and 15 of column 8).

## The model

- Built from nothing but `scripts/mountain-rucksack/model.py` (Blender, headless).
  - Its numbers come from the shop's measures and the stock torsos.
  - resource-porter's `vostok-port build-armour-model` converts its glTF and PNG maps.
- What it has:
  - the bag, with a PVC bottom band;
  - the lid, with its bound edge, over the top and the upper front;
  - two front straps over the lid, two D-rings each, a side-release buckle under the lid;
  - two side pockets, each with a flap, a strap and a buckle;
  - the carry handle;
  - padded shoulder straps, ladder-lock adjusters, webbing round the flanks to the bag's
    lower corners.
- Left out:
  - the waist strap, the drawstring collar and the sleeping-mat sleeve (inside);
  - the mat straps under the bottom.
- 2,984 triangles, one LOD0 surface, as 0.26g's skinned models draw.
  - The Assault fighter's backpack has 2,908.
- The material is the Assault fighter's backpack's, with this model's four maps:
  - diffuse, DXT1;
  - normal map, DXT5 (X in alpha, Y in green, +Y up the texture, as the stock maps);
  - gloss (`texture_roughness`), DXT1;
  - Fresnel, DXT1.
  - All 1024 x 1024 with mips down to 4 x 4, as the stock character maps.
  - Same switches as the template, so the same compiled shader.
- Maps baked from the mesh's own materials:
  - olive Cordura, webbing, black plastic buckles, steel D-rings, PVC;
  - the woven canvas, ribbed and stitched webbing and rounded edges in the normal map;
  - baked occlusion in the diffuse.

## Weights and fit

- The bag rides the back as one rigid piece, as the stock back pieces do: Chest 0.55,
  Spin_1 0.27, Spin 0.18.
  - These are the mean of the stock torsos' weights over the back panel's area.
- The shoulder straps move with the cloth under them.
  - Their weights are the stock torsos' own along the strap: Chest, the clavicle of their
    side, Neck over the shoulder, Spin_1, Spin and Hip under the arm.
  - They are the mean of the six nearest torso vertices on each of the 22 torso_s1 models.
  - Both ends take the bag's weights.
- The back panel and the strap paths follow the 22 stock torso_s1 models' outer surface
  (its 85th percentile), plus a centimetre.
- The porter's measure, against the 59 stock torso surfaces that leave the back free:
  - at most 5.8 % of the vertices sunk (the heavy tank jacket);
  - native back pieces go up to 48.5 % there;
  - above the native median on 2 torsos.
- The renders on all 22 torso_s1 models show no clipping from behind or from the side.

## Needs a look in game

- The straps when the arms move: aiming, reloading, sprinting, crouching.
  - Their weights are the torsos'. A torso that bulges far out (fur collars, the tank
    jackets) may show a strap sunk in, as native back pieces are.
- The colours and gloss in the game's light; the icon in the lobby.
- The lid's binding and the buckles at a distance (normal-map detail).

## Owner's decisions

- 2026-10-31: no faction (the Independent traders), never sold, drops after matches.

## Owner's picks to confirm

- Level and the Assault fighter's backpack's stats.
- Id 1877.
- Names: "German mountain rucksack", "Горно-егерский рюкзак".
- Olive Mil-Tec colours (no Flecktarn version).
