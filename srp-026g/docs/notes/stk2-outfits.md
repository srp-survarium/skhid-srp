# STK2 outfits: bandit, Novice, loner (packs stk2-bandit, stk2-novice, stk2-loner)

The owner (2026-10-30): "let's port what wasn't ported yet" of the STK2 characters export
(`___surva_stk2_characters_port.7z`). Built by resource-porter
`examples/build-stk2-outfits-026` (`src/stk2_outfits_026.rs`, its
`docs/stk2-outfits-026.md`); one content pack per outfit. srp needs no code: it reads each
item's faction (none), level, slots and drop weight from its config in the server game data.

## What the export holds

- `bandit_01` (STK2's `bandit_01` textures), `neutral_01` (STK2's Novice,
  `neutral_01_novice` textures), `neutral_03` (STK2's loner, `neutral_01` textures),
  `duty_03`. The textures come from the STK2 dump (`/mnt/toshiba/surv-builds/STK2.7z`).
- `_s1`/`_s2`/`_s3` are one mesh three times (only the names differ): the names of the
  native variants, never cut. They are built here as the game draws them (sleeves per
  glove class, trousers per boot class), one item per look. `_hud` hands are the
  third-person ones. `neutral_01_torso_v2` is a second look of the Novice jacket.

## Already ported, not built again

- `bandit_01_head_01`'s balaclava (1810), `bandit_01_head_02`'s (1823), the Novice's knit
  cap (1821, of STK2's `neutral_01_novice` head), the whole `duty_03` set (the Duty and
  Freedom exoskeletons).

## Skipped (not equipment)

- Donor faces: `bandit_01_head_03`, `neutral_03_head`, the face under the knit cap
  (`neutral_01_head`), the faces under the balaclavas.
- `bandit_01_hand`, `bandit_01_hand_v2`: bare hands (`hand` with a wristwatch and a bracelet,
  `hand_v2` without).

## The items

| id | item | slot | level | copies (stats, weight, price, drop weight, bonuses) | model | icon |
|---|---|---|---|---|---|---|
| 1864 | Leather bomber jacket | torso | 6 | Hunter's jacket (83, 6) | `bandit_01_torso_s1` + the native forearms | [1, 9] |
| 1865 | Track pants | legs | 6 | Hunter's pants (84, 6) | `bandit_01_legs_s1` | [2, 9] |
| 1866 | Sneakers | boots (`data_full`) | 6 | Hunter's boots (85, 6) | `bandit_01_boot` | [3, 9] |
| 1867 | Novice's hood | helmet, covers the mask | 4 | Mounting helmet (330, 4) | `neutral_01_helmet`, grown down round the neck | [4, 9] |
| 1822 | Novice's field jacket | torso | 4 | "Marodeur" jacket (347, 4) | `neutral_01_torso_s1` (canteen, knife, pouch) | [19, 6] |
| 1868 | Novice's field jacket with pouches | torso | 4 | "Marodeur" jacket (347, 4) | `neutral_01_torso_v2_s1` (two belt pouches) | [5, 9] |
| 1869 | Novice's backpack | back | 4 | "Marodeur" container (355, 3; no back piece is level 4) | `neutral_01_back` | [6, 9] |
| 1870 | Novice's jeans | legs | 4 | Field pants (47, 4) | `neutral_01_legs_s1` | [7, 9] |
| 1871 | Novice's boots | boots (`data_full`) | 4 | Military boots (39, 4) | `neutral_01_boot` | [8, 9] |
| 1872 | Novice's fingerless gloves | gloves (`data_full`) | 4 | "Marodeur" gloves (1105, 4) | `neutral_01_hand` | [9, 9] |
| 1873 | Loner's jacket with vest | torso | 6 | Paratrooper bulletproof vest (754, 6) | `neutral_03_torso_s1` + four pouches | [10, 9] |
| 1874 | Loner's camo pants | legs | 6 | Paratrooper pants (755, 6) | `neutral_03_legs_s1` + pouch, holster, nuts bag | [11, 9] |
| 1875 | Loner's boots | boots (`data_full`) | 6 | Army boots (62, 6) | `neutral_03_boot` (the left boot the right one mirrored) | [12, 9] |
| 1876 | Loner's fingerless gloves | gloves (`data_full`) | 6 | Rough gloves (327, 5; no gloves are level 6) | `neutral_03_hand` | [13, 9] |

- **No faction, drop only (the owner, 2026-10-31):** every item's `parameters.faction` is 0,
  as the Novice's knit cap's: the Independent traders' (the item tables show them so). No
  one sells them: srp sells gear of no faction only through a named `prices` exception, and
  none names them (nor a bundle or the start kit). They drop after matches: each has its
  donor's drop weight, its category is in the generic drop table, and the table's level rows
  roll its item level (`prices::tests::the_stk2_outfits_are_sold_by_no_one`; on the built server data, `drops::tests::the_stk2_outfits_drop_and_are_sold_by_no_one`).
  Drops are rolled after a match, so nothing a match computes changes (no
  `simulation_changes.tsv` row).
- **Levels (the owner, 2026-10-31):** the Novice's set 4, the bandit's and the loner's 6.
  Each piece copies a stock piece of its slot at that level (any faction, the nearest look:
  a jacket for the Novice, a vest for the loner); where none is at that level (a back piece
  at 4, gloves at 6), the nearest one's numbers at the owner's level.

- Each item is its donor's config whole (parameters, protection, weight, price, drop
  weight, built-in bonuses, card, sounds) with only its model, icon, texts, faction (none)
  and, where set above, level; camouflage and decals off (the donor textures have no masks).
- Ids: 1822 is the id resource-porter's Novice port reserved for its field jacket (never
  shipped); the rest follow the ported ids (1823 the Army balaclava, 1824-1862 the 0.69d
  decals, 1863 equipment-046's MK II jacket). Icons: column 9 of the equipment atlas (free
  in the released movie) and the jacket's reserved [19, 6].
- `model_type` (the trousers a boot wears): all three `data_full`, so full trousers go
  over them: the sneakers (15 cm), the loner's boots (one low shoe, one wrapped in a rag to
  29 cm; the Military boots they copy are `data_medium`) and the Novice's (26 cm, worn under
  the jeans as in STK2: the top of the shaft is painted black where the jeans covered it,
  and tucked trousers would show it, as the exo boots did).

## Picks for the owner to change

1. Donors at the owner's levels above (stats, weight, price and drop weight).
2. The Novice jacket of 2026-10-19 (1822, not shipped then) had the owner's numbers:
   level 2, 25 armour, 10 isolation, 4.5 kg, drop 140, pocket capacity +1; here it copies
   the "Marodeur" jacket at level 4.
3. Two Novice jackets (`torso_s*` and `torso_v2_s*` differ only in the belt kit), or one.
4. The hood covers the mask slot (a mask comes off when it goes on), as the Marauder,
   Molot and Zubr helmets; fitting it over masks would mean moving its rim off the face.
5. Names and texts (new; STK2 has none): "Loner" for STK2's neutral stalker.

## Fits (offline, bind pose)

- Against every stock and released piece of the class, and each other's: gloves' cuffs
  inside wrist-length sleeves; the boots inside full hems; trousers lengthened to 12 cm
  over low boots where they end higher (the Novice's and the loner's end at 23-29 cm over
  their own boots), into medium and tall boots and cut inside them; jackets' waist over every trousers, collar over the native neck and every
  mask, sleeves over wrist-length gloves and into gauntlets (cut inside them); the hood
  over the native skull.
- The Novice jeans' waist is black in the STK2 map (hidden by STK2's jacket); it is
  painted over with the denim below it, reflected.

## Version 2 (the owner's local test, 2026-10-30)

- "The Novice jeans have a hole at the top; you can see behind the model": they ended at
  0.94-0.96 m, where STK2's long jacket covered them, and the background showed between
  them and shorter jackets (no body is drawn under trousers). A band now closes them up
  to the native legs' top ring (1.04 m, which every stock torso covers). Rendered against
  a magenta background with every stock `torso_s1` and the four new torsos from six
  views, the background showing in the waist band went from 75,854 to 61,927 pixels: the
  same as stock trousers (Linen pants 63,039, Jeans 63,717; what is left is between the
  arms and the body).
- "With the bomber, any gloves clip into it": its bare forearms were the donor's (from
  `bandit_01_hand`) and stood through the gloves' cuffs. Now each variant draws the native
  body's forearms (`base_character_torso_s1`/`_s2`/`_s3`, native skin), as the stock
  bare-armed torsos (the Leather sleeveless jacket) do, for which every stock glove is
  made.
- "The hood is not modelled at the bottom, compared to the Scavenger hood": the stock
  Scavenger hood's cowl (below 1.58 m, its own skin over the neck and shoulders) is joined
  under it, on a patch of the hood's own canvas.
- "The icons are too small", "the boots from another angle", "the sneakers a bit
  smaller": icons are fitted to the stock icons' fill of their slot (81 px high, boots
  76, gloves 77), the boots seen level with their black shaft openings capped for the
  icon, the sneakers 15 % smaller.
- Packs `@2` (all three; the loner's changed only where it is fitted to the other
  outfits).

## Version 3 (the owner, on version 2)

- The jeans' new waist band was a smear (the top texel row repeated) and tapered to the
  native ring ("too tight", wasp-waisted). It now rises straight up at the jeans' own
  width, its denim reflected up from below the old top. Girth (width x depth, cm; the
  perimeter): at 0.95 m v1 37.6 x 22.6 (open), v3 37.6 x 28.9, 104, the stock Jeans
  37.8 x 29.8, 106; at the hip (0.88) 112 against the stock Jeans' 106 and Linen pants'
  107, at mid-thigh 63 against 55 and 62. One leg's section at 1.00-1.03 m went from 74-69
  (tapering) to 81 (straight). Looser than stock below the band; not tighter anywhere.
- "The hood has unrelated parts": the grafted Scavenger cowl is gone. The hood's own
  shell hem grows down round the neck to the collar line (1.47 m, 1.44 in front, 6 mm off
  the native neck) and its flap tips are joined across the throat with a seam down the
  middle, the canvas reflected across the edge it grows from, the skin blending from the
  hood's into the Scavenger hood's hem's (its reference for the neck and shoulders). The
  hood's own hem is dark in the STK2 map, so the grown part continues dark; under
  jackets only the collar line shows.
- Glove icons at the stock angle (the stock glove icons' long axis 25-41 degrees above
  the horizontal, median 32; ours 15 before, 32 now), the back of the hand to the camera,
  the wrist capped and the cuff's black band in the glove's leather colour; the boot caps
  in the rim's shadowed colour; the sneakers 28 % under the boots' stock fill.
- Packs `@3`.
- Glove icons, again (the owner: "the hand is looking inward"; the black should go): the
  camera looks at the back of the hand square on (along the hand's own plane normal), the
  outline's long axis on the stock 32 degrees, the fingers down to the left; the cuff's
  black band and inside are left out of the icon render, not recoloured (the in-game mesh
  keeps them). Packs `stk2-novice@4`, `stk2-loner@4` (their icons only); `stk2-bandit@3`.
- "One boot has a part of the trousers": the export's left loner boot carried the bottom
  of the camo pants' leg (a wrapped cuff up to 29 cm, on the pants' part of the map, cut
  from the legs with the boot); first its faces above 13 cm with no mirror image on the
  right boot were dropped, then (it left a torn top) the whole left boot is the right
  shoe mirrored. The sneakers and the Novice's boots are
  mirror images, with nothing extra. Checked with the Loner pants, the Novice jeans and
  stock full trousers (the loner's boots are `data_full`): no gap but a sliver at the back
  heel under the highest stock hems (13 cm; the shoes are 15.5 cm high).
- Boot icons: each boot cut where its black shaft top begins, instead of a cap.
- Packs `stk2-bandit@4`, `stk2-novice@5`, `stk2-loner@5`.
- Then (the owner: the cut gloves and boots looked severed, with sawtooth edges; the left
  loner shoe's top was torn): every glove and boot icon keeps the whole piece up to its own
  cuff or top; of the black faces only those facing into the piece (lining, the inside of
  an opening) are left out, and the outer ones take their mesh neighbours' colour. The
  left loner boot is the right shoe mirrored (bones swapped Left/Right), with the same
  finished collar; the loner's shoes' icon 20 % under the boots' fill, as the sneakers'.
  Packs `stk2-bandit@5`, `stk2-novice@6`, `stk2-loner@6`.
- The Novice glove's cuff still rose in a dark spike (its black end band and inside, kept
  and recoloured, stand past the cloth): the glove icons now leave out the faces wholly on
  that band or inside the cuff (all three corners black), so the glove ends along the
  band's edge loop, smooth. The sneakers' icon is back to its first render (the owner's
  pick: the camera a little above, the collar and tongue whole), 28 % smaller. Packs
  `stk2-bandit@6`, `stk2-novice@7`, `stk2-loner@7`.
- No faction and drop only, the levels 4 and 6 with their new donors (above); the trousers'
  icons cut at the shin as the stock ones (from the waist down to the stock icons' width to
  height, 77 by 81); the Novice glove's cuff edge smoothed. Packs `stk2-bandit@7`,
  `stk2-novice@8`, `stk2-loner@8`.
- The backpack is not fitted: its straps lie on the stock torsos as stock straps do and
  sink into the bulky ones (Zubr UM-4) as theirs do.
- Renders of every piece with stock pieces of each class were checked; nothing was seen
  in the game or through the animation clips.

## In-game check left

- Each outfit whole, front, side and back, in the lobby and a match.
- Jackets with gauntlets (Chemical protection gloves, Omicron gloves) and wrist gloves; the
  bomber's bare forearms with and without gloves.
- Trousers with low (Leather boots), medium (Military boots) and tall (Stalker's boots)
  boots; the Novice's jeans and the loner's pants with low boots (lengthened).
- The sneakers, the loner's and the Novice's boots under stock trousers (the Novice's are
  tall for `data_full`: their shafts are pulled in under every hem).
- The collars with masks (gas masks, balaclavas) while aiming and looking down; the hood
  over the head.
- The backpack on slim and bulky jackets; first-person hands of both gloves.
- The 14 icons in the shop and inventory.

## Release

- Three `pack` rows in `release/content.tsv`: `stk2-bandit@7`, `stk2-novice@8`,
  `stk2-loner@8` (their pack.json sha256s). `release/build-content` composes them; the
  release PC keeps the folders in `$SKHID_PACKS_DIR` (`stk2-bandit-7`, `stk2-novice-8`, `stk2-loner-8`), built as
  resource-porter's `docs/stk2-outfits-026.md` says, byte for byte.
- The item texts are in the ten catalog TSVs (`client-patches/localization`).
