# STK2 outfits: bandit, Novice, loner (packs stk2-bandit, stk2-novice, stk2-loner)

The owner (2026-10-30): "let's port what wasn't ported yet" of the STK2 characters export
(`___surva_stk2_characters_port.7z`). Built by resource-porter
`examples/build-stk2-outfits-026` (`src/stk2_outfits_026.rs`, its
`docs/stk2-outfits-026.md`); one content pack per outfit. srp needs no code: it reads each
item's faction, level, slots and drop weight from its config in the server game data.

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

| id | item | slot | faction | level | copies (stats, price, drop, bonuses) | model | icon |
|---|---|---|---|---|---|---|---|
| 1864 | Leather bomber jacket | torso | 2 Black Market | 3 | Thug's jacket (32) | `bandit_01_torso_s1` + the native forearms | [1, 9] |
| 1865 | Track pants | legs | 2 Black Market | 3 | Reinforced jeans (44) | `bandit_01_legs_s1` | [2, 9] |
| 1866 | Sneakers | boots (`data_full`) | 2 Black Market | 1 | Cossacks (24) | `bandit_01_boot` | [3, 9] |
| 1867 | Novice's hood | helmet, covers the mask | 1 Scavengers | 3 | Scavenger hood (892) | `neutral_01_helmet`, grown down round the neck | [4, 9] |
| 1822 | Novice's field jacket | torso | 1 Scavengers | 1 | Quilted jacket (31) | `neutral_01_torso_s1` (canteen, knife, pouch) | [19, 6] |
| 1868 | Novice's field jacket with pouches | torso | 1 Scavengers | 1 | Quilted jacket (31) | `neutral_01_torso_v2_s1` (two belt pouches) | [5, 9] |
| 1869 | Novice's backpack | back | 1 Scavengers | 1 | Backpack with container (49) | `neutral_01_back` | [6, 9] |
| 1870 | Novice's jeans | legs | 1 Scavengers | 1 | Linen pants (45) | `neutral_01_legs_s1` | [7, 9] |
| 1871 | Novice's boots | boots (`data_full`) | 1 Scavengers | 1 | Leather boots (37) | `neutral_01_boot` | [8, 9] |
| 1872 | Novice's fingerless gloves | gloves (`data_full`) | 1 Scavengers | 1 | Cut gloves (41) | `neutral_01_hand` | [9, 9] |
| 1873 | Loner's jacket with vest | torso | 1 Scavengers | 4 | Light armored vest (48) | `neutral_03_torso_s1` + four pouches | [10, 9] |
| 1874 | Loner's camo pants | legs | 1 Scavengers | 4 | Field pants (47) | `neutral_03_legs_s1` + pouch, holster, nuts bag | [11, 9] |
| 1875 | Loner's boots | boots (`data_full`) | 1 Scavengers | 4 | Military boots (39) | `neutral_03_boot` | [12, 9] |
| 1876 | Loner's fingerless gloves | gloves (`data_full`) | 1 Scavengers | 5 | Rough gloves (327) | `neutral_03_hand` | [13, 9] |

- Each item is its donor's config whole (parameters, protection, weight, price, drop
  weight, built-in bonuses, card, sounds) with only its model, icon and texts; camouflage
  and decals off (the donor textures have no masks). Sellers follow srp's rule
  (`prices::seller_level`): Scavengers and Black Market at reputation = item level.
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

1. Factions: bandit Black Market (the game's bandits: "Bandit's jacket" is theirs), Novice
   and loner Scavengers. The knit cap 1821 is in no faction at level 2 (the owner,
   2026-10-19); the Novice items here could follow it, but gear of no faction is sold only
   through srp's named exceptions (`prices.rs`), which is server code.
2. Levels and donors above. The Novice jacket of 2026-10-19 (1822, not shipped then) had
   the owner's numbers: level 2, no faction, 25 armour, 10 isolation, 4.5 kg, 3,300, drop
   140, pocket capacity +1; here it copies the Quilted jacket.
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

- Three `pack` rows in `release/content.tsv`: `stk2-bandit@3`, `stk2-novice@3`,
  `stk2-loner@3` (their pack.json sha256s). `release/build-content` composes them; the
  release PC keeps the folders in `$SKHID_PACKS_DIR` (`stk2-<outfit>-3`), built as
  resource-porter's `docs/stk2-outfits-026.md` says, byte for byte.
- The item texts are in the ten catalog TSVs (`client-patches/localization`).
