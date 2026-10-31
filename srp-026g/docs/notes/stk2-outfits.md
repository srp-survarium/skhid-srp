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

| id | item | slot | level | config copied (card, sounds; numbers its own, see Stats) | model | icon |
|---|---|---|---|---|---|---|
| 1864 | Leather bomber jacket | torso | 6 | Hunter's jacket (83, 6) | `bandit_01_torso_s1` + bare forearms grown from the native hands' wrist | [1, 9] |
| 1865 | Track pants | legs | 6 | Hunter's pants (84, 6) | `bandit_01_legs_s1` | [2, 9] |
| 1866 | Sneakers | boots (`data_full`) | 6 | Hunter's boots (85, 6) | `bandit_01_boot` | [3, 9] |
| 1867 | Novice's hood | helmet (masks go under it) | 4 | Mounting helmet (330, 4) | `neutral_01_helmet`, grown down round the neck | [4, 9] |
| 1822 | Novice's field jacket | torso | 4 | "Marodeur" jacket (347, 4) | `neutral_01_torso_s1` (canteen, knife, pouch) | [19, 6] |
| 1868 | Novice's field jacket with pouches | torso | 4 | "Marodeur" jacket (347, 4) | `neutral_01_torso_v2_s1` (two belt pouches) | [5, 9] |
| 1869 | Novice's backpack | back | 4 | "Marodeur" container (355, 3) | `neutral_01_back` | [6, 9] |
| 1870 | Novice's jeans | legs | 4 | Field pants (47, 4) | `neutral_01_legs_s1` | [7, 9] |
| 1871 | Novice's boots | boots (`data_full`) | 4 | Military boots (39, 4) | `neutral_01_boot` | [8, 9] |
| 1872 | Novice's fingerless gloves | gloves (`data_full`) | 4 | "Marodeur" gloves (1105, 4) | `neutral_01_hand` | [9, 9] |
| 1873 | Loner's jacket with vest | torso | 6 | Paratrooper bulletproof vest (754, 6) | `neutral_03_torso_s1` + four pouches | [10, 9] |
| 1874 | Loner's camo pants | legs | 6 | Paratrooper pants (755, 6) | `neutral_03_legs_s1` + pouch, holster, nuts bag | [11, 9] |
| 1875 | Loner's boots | boots (`data_full`) | 6 | Army boots (62, 6) | `neutral_03_boot` (the left boot the right one mirrored) | [12, 9] |
| 1876 | Loner's fingerless gloves | gloves (`data_full`) | 6 | Rough gloves (327, 5) | `neutral_03_hand` | [13, 9] |

- **No faction, sold as the knit cap, and dropped (the owner, 2026-10-31):** every item's
  `parameters.faction` is 0, as the Novice's knit cap's: the Independent traders' (the item
  tables show them so). First drop only; then "let's make them buyable the same way the
  beanie is": each has a `prices::EXCEPTIONS` row `(0, 0)`, as the cap, so seller 0 sells it
  to everyone at no rank, at its price (Stats, below); the 14 names are in
  `prices::UNRELEASED` until the release that ships the packs. They also drop after
  matches: each has its level's drop weight, its category is in the generic drop table, and
  the table's level rows roll its item level (`prices::tests::the_stk2_outfits_are_sold_to_everyone_at_no_rank`;
  on the built server data, `drops::tests::the_stk2_outfits_drop_and_are_sold_to_everyone`).
  Drops are rolled after a match and prices are the lobby's, so nothing a match computes
  changes (no `simulation_changes.tsv` row).
- **Levels (the owner, 2026-10-31):** the Novice's set 4, the bandit's and the loner's 6.
- Each item's config is a stock piece's of its slot (card, sounds, rollable modifications)
  with its own numbers (Stats, below), model, icon, texts and faction (none); camouflage
  and decals off (the donor textures have no masks); no item takes another slot.

## Stats (the owner's, 2026-10-31)

Each number inside the range of the other armour of its slot and level in the released
game data (non-premium, still dropped, every faction), shown as [min/median/max]; where a
slot has fewer than two pieces at the level (backs at 4, gloves at 4 and 6), the nearest
levels below and above too. `armour::tests::the_stk2_outfits_are_inside_their_slot_and_levels_stock_range`
holds them there on the built server data. Armour: against bullets, blades and blasts;
isolation: fire, shock, toxins and radiation (one number on every stock piece, so the
jackets' warmth is isolation). Every stock armour piece has reliability 1, wear 0.01 a
match and 0.004 a use, repair 10 % of the price: so do these. Prices are the stock ones of
the slot and level (two tiers: Scavengers' and Renaissance's, Black Market's and Edge's;
gloves cost as boots); drop weight is the level's (120 at 4, 100 at 6).

| id | item | armour | isolation | weight (kg) | bonuses | price | drop | why |
|---|---|---|---|---|---|---|---|---|
| 1864 | Leather bomber | 55 [50/55/60] | 5 [5/10/20] | 7 [6/7/10] | pocket +1 | 20900 | 100 | decent armour, low isolation (leather) |
| 1865 | Track pants | 50 [50/55/60] | 10 [10/10/20] | 1.6 [1.6/1.7/2.1] | stamina use -8 %, movement +4 % | 10450 | 100 | light and mobile: the level's least |
| 1866 | Sneakers | 50 [50/55/60] | 10 [10/15/20] | 1.0 [1.0/1.1/1.2] | movement +4 % | 5230 | 100 | light and mobile |
| 1867 | Novice's hood | 20 [20/22.5/25] | 20 [5/17.5/30] | 1.0 [1.0/1.3/1.6] | none (no stock helmet has one) | 8080 | 120 | light head armour, some isolation; the head only |
| 1822 | Novice's field jacket | 40 [40/40/40] | 30 [10/20/30] | 8 [8/8.5/9] | pocket +1 | 12350 | 120 | the level's top isolation, a pocket |
| 1868 | ... with pouches | 40 | 20 | 8.5 | pocket +1, ammo bag +1 | 13650 | 120 | the pouches an ammo bag, paid in isolation and weight |
| 1869 | Novice's backpack | - | - | 4.5 (levels 3, 5: 4.5/5/8) | pocket +1, pocket size +1 | 7920 | 120 | capacity, as the level-3 and level-5 packs; price between the level-3 (6650) and level-6 (10450) packs' |
| 1870 | Novice's jeans | 40 [40/40/40] | 20 [15/22.5/30] | 1.4 [1.2/1.4/1.6] | pocket size +1 | 6180 | 120 | middling |
| 1871 | Novice's boots | 35 [30/32.5/35] | 10 [5/7.5/10] | 0.9 [0.8/0.85/0.9] | stamina use -8 % | 3090 | 120 | middling, sturdier than the Military boots |
| 1872 | Novice's fingerless gloves | 35 (levels 3-5: 20/42.5/50) | 25 [10/25/50] | 0.35 [0.3/0.375/0.8] | artefact search -20 % | 2620 | 120 | light; bare fingertips |
| 1873 | Loner's jacket with vest | 60 chest and back, 30 forearms [50/55/60] | 5 [5/10/20] | 9.5 [6/7/10] | pocket +1 | 23100 | 100 | the most chest armour of the level-6 torsos; the sleeves the jacket's |
| 1874 | Loner's camo pants | 55 [50/55/60] | 15 [10/10/20] | 1.9 [1.6/1.7/2.1] | ammo bag +1, pocket size +1 | 11550 | 100 | pouch and holster, nuts bag |
| 1875 | Loner's boots | 55 [50/55/60] | 15 [10/15/20] | 1.1 [1.0/1.1/1.2] | stamina use -8 % | 5780 | 100 | middling |
| 1876 | Loner's fingerless gloves | 50 (levels 5, 8: 45/50/50) | 15 [10/20/60] | 0.3 [0.3/0.3/0.35] | reload +12 % | 5230 | 100 | light, a step above the level-5 gloves |

Bonuses, each at its stock value (modification id, and stock pieces carrying it): pocket +1
1030 (jacket 83 at 6, 824 at 5, pants 349 at 4, packs 355, 1357, 753); ammo bag +1 1050
(jacket 48 at 4, 58 and 754 and pants 59, 755 at 6); pocket size +1 1170 (pants 46 at 3,
827 at 5, packs 355, 1357); movement +4 % 1113 (boots 35 at 4, 85 at 6); stamina use
-8 % 1163 (pants 47 at 4, 84 at 6, boots 39 at 4, 62 at 6); reload +12 % 1143 (gloves
1105 at 4, 327 and 829 at 5); artefact search -20 % 1133 (gloves 42 and 989 at 3). No
stock bonus makes steps quieter. Stock pieces at 4-6 carry one bonus (backs two): 1868,
1865 and 1874 carry two, the owner's call ("we can add some stock bonuses too").

Sets against a typical stock set of their level (the slots' medians): the Novice's at 4
matches it in armour (hood 20, gloves 35 against 23 and 42) and weighs 16.2 kg against
17.4; the bandit's at 6 is the least protected (55/50/50 armour, 5/10/10 isolation against
55/55/55 and 10/10/15), 9.6 kg against 9.8, and pays with mobility (movement +8 %, stamina
use -8 %, a pocket); the loner's at 6 is the armoured one (60 on the chest), 12.8 kg
against 10.1.
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

1. Two Novice jackets (`torso_s*` and `torso_v2_s*` differ only in the belt kit), or one.
2. Names and texts (new; STK2 has none): "Loner" for STK2's neutral stalker.

## Fits (offline, bind pose)

- Against every stock and released piece of the class, and each other's: gloves' cuffs
  inside wrist-length sleeves; the boots inside full hems; trousers lengthened to 12 cm
  over low boots where they end higher (the Novice's and the loner's end at 23-29 cm over
  their own boots), into medium and tall boots and cut inside them; jackets' waist over every trousers, collar over the native neck and every
  mask, sleeves over wrist-length gloves and into gauntlets (cut inside them); the hood
  over the native skull and over every mask (below).
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
- No faction and drop only (since: sold as the knit cap, above), the levels 4 and 6 with their new donors; the trousers'
  icons cut at the shin as the stock ones (from the waist down to the stock icons' width to
  height, 77 by 81); the Novice glove's cuff edge smoothed. Packs `stk2-bandit@7`,
  `stk2-novice@8`, `stk2-loner@8`.
- The trousers' icons cut at the ankle (the owner: "cut at the ankles"), as the stock ones
  show them: 93 % of the waist-to-floor height, fitted to the stock 81 px, the width
  following (41-45 px), not cropped to a ratio; the open waist's inside (an unlit black
  band over the front) left out of them. The Novice jeans' grown waist ends in a 4 cm
  waistband painted in a free strip of their map (lighter denim, stitched seams, the belt
  line under it). Packs `stk2-bandit@8`, `stk2-novice@9`, `stk2-loner@9`.
- The ankle cut made the trousers' icons too long and thin beside the stock ones, which
  stop a hand below the knee. Measured: each stock trousers model rendered with the icon
  camera and its silhouette fitted to its icon (27 of the 53 fit, IoU over 0.85) is cut at
  0.30-0.50 m, median 0.40 m (the knee at 0.48-0.50 m), 0.62 of the waist-to-floor height.
  Ours are cut at 0.62 and fitted to 81 px: 55-64 px wide opaque, the stock median 61 by
  80. Packs `stk2-bandit@9`, `stk2-novice@10`, `stk2-loner@10` (the trousers' icons only).
- The stock icons' faint dark edge (the owner: "retail pants have this light black effect at
  the corners", "very slight"): every stock icon sits on a black halo, even all round, its
  alpha 60 at 2 px from the body, 34 at 4, 19 at 6, 10 at 8, 4 at 10 (248 icons of the
  released atlas), which is the item's alpha blurred (sigma 5 px) at 0.6. All 14 icons now
  have it at 0.55, a shade fainter; the cells' top and bottom rows stay clear as on stock,
  nothing else reaches a cell edge. Packs `stk2-bandit@10`, `stk2-novice@11`,
  `stk2-loner@11` (their icons only).
- The owner's stats (Stats, above), approved as proposed. Packs `stk2-bandit@11`,
  `stk2-novice@12`, `stk2-loner@12`, with the two fixes below.
- "The hands are too big ... they don't fit the original palms" and "the sweater or
  something is still seen" (the owner's screenshots: a pink band between the bomber's hem
  and the Loner's pants): the forearms were the native torso's, its sweater's sleeves (57
  to 62 mm round the arm at the top, against the hands' 31 mm wrist), and the faces taken
  for them let the sweater's waist through below the hem (they were picked along the
  forearm's direction, which in the bind pose also runs down to the hips). They are now
  grown from the native hands' wrist ring (its own vertices and weights first, so the skin
  goes on from the hand's) up to 5 cm below the elbow, 1.3 times the wrist at the top,
  with the hands' skin; nothing of the native torso is left in the bomber. Checked in the
  game's clips (aiming, sprinting): with no gloves and the Rough, Black Market and
  researcher's gloves (the cuffs cover the forearms), and over the Loner's pants, the
  jeans, the track pants and three stock trousers from the side and from below (the hem
  over every waist, no gap). Pack `stk2-bandit@12`.
- "It shines through ... not in the elbow, the waist" (from below, aiming): the bomber is
  a short loose jacket with no body inside (its own T-shirt ends 0.98 to 1.15 m up, its
  hem 0.92), and the native torso's waist faces that had filled it by mistake were gone;
  from below, between the hem and the trousers, the background showed. It is now closed
  inside its hem: a floor from the hem's inner side in to the native hips, both ways, with
  the hem's leather (skin blending from the hem's to the hips'); every trousers' top comes
  through it. Counted from below (six views, aiming, sprinting and crouching, against a
  probe body inside): see-through pixels at the waist 27-669 before, 0-35 after, with the
  Loner's pants, the jeans, the track pants and three stock trousers; the cuffs, armpits
  and open front show none. Pack `stk2-bandit@13`.
- "Let's make them buyable the same way the beanie is": sold by seller 0 to everyone at
  no rank at their prices, and still dropped (srp's `prices`, no pack change).
- "The sweater is seen through the leather jacket": the bomber's bare forearms were the
  native body's (`base_character_torso_s1`/`_s2`/`_s3`) with the native torso's material,
  whose map is a red Christmas sweater; they now take the native hands' material, mapped to
  the plain skin of its map. Rendered in the game's clips (aiming, crouching, sprinting):
  skin below the pushed-up sleeves, nothing through the leather. The bomber's own T-shirt
  at the open front is STK2's and stays.
- "The hood shouldn't take the mask slot": masks go under it, and it protects the head
  only (no face region), as the stock helmets that leave the mask free. Grown, it is pushed
  out over every mask (stock, the STK2 and 0.69d ones, gas masks) where a mask's point
  behind the face comes through it or within 3 mm, up to 15 mm: 868 points of 15 masks,
  239 vertices moved, up to 16.7 mm; nothing of them comes through it after. The bulky
  helmet masks' neck guards, hoses and filters (Black Market anomaly mask, Edge assault
  head, Sturm, sniper and combat heads: 30 to 62 mm out) are left: they stand as far out of
  the stock Scavenger hood (31 to 57 mm, which the 0.69 balaclavas, buff and keffiyeh also
  stand 23 to 59 mm out of).
- The backpack is not fitted: its straps lie on the stock torsos as stock straps do and
  sink into the bulky ones (Zubr UM-4) as theirs do.
- Renders of every piece with stock pieces of each class were checked; the bomber and the
  hood with masks also in poses of the game's clips (resource-porter's
  `examples/pose-stk2-outfits-026.rs`).

## In-game check left

- Each outfit whole, front, side and back, in the lobby and a match.
- Jackets with gauntlets (Chemical protection gloves, Omicron gloves) and wrist gloves; the
  bomber's bare forearms with and without gloves.
- Trousers with low (Leather boots), medium (Military boots) and tall (Stalker's boots)
  boots; the Novice's jeans and the loner's pants with low boots (lengthened).
- The sneakers, the loner's and the Novice's boots under stock trousers (the Novice's are
  tall for `data_full`: their shafts are pulled in under every hem).
- The collars with masks (gas masks, balaclavas) while aiming and looking down; the hood
  over the head with and without a mask (balaclava, gas mask, a bulky helmet mask).
- The backpack on slim and bulky jackets; first-person hands of both gloves.
- The 14 icons in the shop and inventory.

## Release

- Three `pack` rows in `release/content.tsv`: `stk2-bandit@13`, `stk2-novice@12`,
  `stk2-loner@12` (their pack.json sha256s). `release/build-content` composes them; the
  release PC keeps the folders in `$SKHID_PACKS_DIR` (`stk2-bandit-7`, `stk2-novice-8`, `stk2-loner-8`), built as
  resource-porter's `docs/stk2-outfits-026.md` says, byte for byte.
- The item texts are in the ten catalog TSVs (`client-patches/localization`).
