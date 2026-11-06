# Equipment from 0.31a2 and 0.46c2 (pack equipment-046)

The owner's pick of 0.69d equipment that live Skhid (rev 38) lacks: 38 pieces.
35 ship in the content pack `equipment-046@4` (`scripts/equipment-046`); 3 are
left out (below). `@8` adds four more the owner picked on 2026-11-05 (the
Veles Mask, Stalker's laced boots, "Shturm" gloves, Reinforced padded boots:
below), each with its donor's quirk, makes the NVD open goggles with a bonus
of its own, and puts the Veles and Sniper sets at level 12 from their 0.31a2
numbers: 39 pieces. (`@5` to `@7`, drafts of PR #1360, were never released.)
Every number here is a proposal: balance is the owner's call.

## What ships

- Donors: **0.31a2** for the 24 pieces it has (0.29c's format, the same as
  equipment-029's donor), **0.46c2** for the 15 only it has. Their models are
  0.26g's: no vertex conversion.
- 0.46c2 differences, converted in `make_pack.py`: configs in 16-byte records
  (read by `research-mode/bcw.py`, written in 0.26g's 24-byte ones); its items
  inside `gameplay/game.options` without surface names (`part_name` taken from
  the model's LOD0 surface); LOD1/LOD2 surfaces dropped (0.26g's skinned models
  have none); the material stage key `use_foliage_sun_backlighting` dropped.
- Both donors: surfaces keep only the vertex and index chunks (0.31a2 adds
  chunk 110, 0.46c2 110 and 113; no stock surface has them); data type 5 ->
  native 4; `damage_protection` (0.31's) dropped: 0.26g reads `hit_params`.
- Parameters, defect masks, repair and card layout come from a native piece of
  the same kind (`template_id`); protection on all seven native hit types of the
  donor's body regions; the card shows armour, isolation, weight (a backpack:
  weight).
- Masks are worn over the native head (`remove_head_model` dropped), as the
  other mask ports, except the two closed shells over the whole head
  (`covers_head` in `items.tsv`): the Armored mask and the Spectrum-M keep
  their donor's `remove_head_model`. Each is one shell open only at the neck
  (no face skin, no eye holes); over the native head 62 and 49 of their
  vertices lay inside it, up to 5.5 and 1.4 cm, and the skull showed through
  both (the owner, in game, 2026-10-30; fixed in @4). The NVD
  is goggles on straps over the native head: its goggles' rear rims touch the
  face (up to 1 cm inside it, round the eyes), as goggles do.
- The Veles Mask is a gas mask with goggles on three straps round the head,
  open at the back (renders): not a closed shell, so it is worn over the
  native head. Its donor takes the head's place and draws the stock stand-in
  head `renesanse_novice_combat_head` beside it (as the stock "Keeper" mask
  does); over the native head both go (`prepare_inputs.on_native_head`: a
  donor's `..._head` additional model goes with its `remove_head_model`).
- Made on the stand-in, it is fitted onto the native head (`make_pack.
  fit_to_native_head`, resource-porter mask_fit's outward push: rays from
  inside the skull, 4 mm clearance, each push spread to its neighbours). As
  built in @5, 6 native skull points showed through it (up to 4 mm: the
  forehead over the goggles' frame, the back straps sunk in the skull) and
  118 of its 1,144 vertices lay inside the head. Only what can be seen moves:
  the outer surface inside the head and the skin through the outermost
  surface; the inner rim the respirator seals on the face with stays in the
  face, as it lay in the stand-in's (pushing it, a first try, swelled the
  whole front). 548 vertices move, at most 1.0 cm; no skull point shows
  through (`check_pack.py` checks every mask over the native head, 1 mm
  allowed for rims resting on the skin: the NVD's goggle rims). There is no
  eye-hole move or neck tuck: the stand-in is on the native skeleton and the
  mask ends at the chin, above every collar. 0.26g has one head mesh
  (`base_character_head`); faces differ by texture only.
- A donor sound 0.26g lacks (the pants' `material/crouch_wear`) takes the
  template's sound in its place.
- Shaders: 0.26g ships its shaders compiled per switch combination and mesh
  kind, so a skinned piece keeps to the combinations the released character
  materials draw (`character_switches.tsv`): the Watchman helmet's `use_olta`
  (a static-mesh combination: with it the lobby hung, the character never
  loading, @2's in-game run) and the Watchman boots' emissive map and flicker
  are switched off; every other material draws as the donor's.
- Shared assets the released game has are reused, a material only when it is
  the donor's and complete: 0.26g's Stalker backpack and NVD materials are
  placeholders (no textures shipped, the backpack's vertex colours off: they
  drew neon in @2's run), so the donor's ship as `..._eq046`, with their
  textures.
- Camouflage: each diffuse map's implied `_aoac`, `_camo`, `_camo_nmap`,
  `_camo_smap` maps ship with it (@2 lacked the `_camo` ones: a camouflage on
  the Bandit's jacket "MK II" failed to cook).
- The Sniper vest's bright plates and OZK-UV's glossy rubber are the donor's
  own: their materials and maps ship unchanged, and 0.31a2's maps of models
  0.26g has are the same pixels as 0.26g's.
- Ids: 0.69d's own dictionary ids (988-1012, 1092-1108, 144, 145): none is
  taken by the stock db, a released patch, equipment-029 or live revision 50.
  The premium copy takes 1863 (0.69d's 352 is the stock Bandit jacket premium).
- Icons: the donors' own item icons, in the free equipment-atlas columns 7
  (rows 1-22) and 8 (rows 1-13 and, @8, 16-19; equipment-069 has 14-15).
- The declared tables carry the pieces too: their armour, weight, levels,
  rollable kinds and built-in bonus in `content-edit/armor.tsv`, their price in
  `content-edit/items.tsv`, both as the pack makes them.
- Texts: `st_srp_equipment046_<0.69d name>_name/_desc` in the ten catalog TSVs:
  the donors' names (German, Polish, Russian, Ukrainian), English renamed to
  the existing sets' names where 0.26g has the set (Watchman, Stalker's,
  Scout's, "Marodeur", Bandit's), German typos fixed (`-hanfschuhe`,
  `Gutschein` for Token); Serbian, Czech, Hungarian, Finnish, Belarusian
  translated in the stock wording. The donors have no descriptions: each
  language's "description not available yet".
- Built-in bonuses are already at the second-strongest strength
  (`built-in-bonuses-1` leaves them; `check_pack.py` checks it).

## The pieces (armour / isolation, kg, price, drop weight)

Levels map 0.31a2's onto 0.26g's by the shared models (0.31 level 3 -> 3,
10 -> 15), proportionally between (the owner, 2026-10-30: 0.31 level 7 ->
3 + 4 x 12/7 = 9.9 -> 10). Shop rank by srp's rule (`prices::seller_level`).
Prices are the level's median of the piece's kind (the owner's price
analysis, 2026-10-30).

- **Black Market "Heavy armored"** (0.31 level 10, `sturm_10`) -> **level 15**
  (the owner, 2026-10-30), the Black Market's first level-15 set; Black Market
  rank 5; drop 10.
  - Vest 88/20, 14 kg, 100,000, 1 ammo bag + 2 pockets (Zubr 90/50, 20 kg).
  - Pants 88/20, 4 kg, 50,000, ammo bag + pocket size.
  - Helmet 65/20, 2.5 kg, 71,250 (brain only; Nomad's 60/30; 0.95x the
    level-15 helmet median of 75,000, as the vest).
  - Armored mask 50/15, 1.2 kg, 23,760 (no native level-15 mask).
  - Gloves 88/20, 0.6 kg, 25,000, recoil -8 %. Boots 88/20, 1.8 kg, 25,000,
    energy costs -8 %.
  - Backpack for ammo: 4 kg, 47,500, 2 ammo bags + 1 pocket.
  - Five pieces + mask 24.1 kg (Zubr 34.6, Nomad 17.3): less armour and
    isolation than Zubr, lighter.
- **Veles and Sniper at level 12** (@8; the owner, 2026-11-06: "veles numbers
  you did are incorrect", "it has numbers in 0.30 patches", "i feel sniper
  vest is incorrect", both sets at level 12 and clearly different from the
  level-12 Molot, Omicron and Leshy). Until @7 they were level 10, each piece
  a third of the way from its level-9 to its level-12 native peers (@2):
  peer medians, not their donor, so the Sniper set had 27-28 isolation and
  the Veles helmet and mask 2.9 and 1.8 kg. Now from their 0.31a2 numbers
  (0.31a2, level 7, is the build nearest 0.30, where both sets came in):
  - 0.31a2: Veles vest 70/10 8 kg, pants 70/8 2.8 kg, helmet 70/7 1.2 kg,
    mask 70/12 0.6 kg, Sorcerer gloves 70/2 0.7 kg, Sorcerer boots 70/2
    1 kg, Token 3 kg; Sniper vest 65/7 6 kg, pants 65/6 1.5 kg, helmet 60/4
    1 kg, gloves 65/1 0.2 kg, boots 65/1 0.5 kg (isolation = `ano_armor`,
    the radiation `damage_protection`). The Spectrum-M is 0.46c2's only:
    its numbers stay, at level 12 with its set.
  - The scale: the pieces whose models both builds have (130 of them) give
    0.26g's body armour about 0.9 of 0.31's (0.31 level 8 -> 0.26g 12:
    Omicron, Leshy, Molot, Combat 0.87-0.94), helmets and masks less (0.27-0.65,
    0.38-0.59), and 0.26g's isolation 2-20 times 0.31's, unevenly by faction
    (Leshy 1.7x, Omicron 4x, Molot 4x on vests). Scaled by those medians the
    two sets come out where @2 had them (Sniper vest 57/22): the owner's
    "incorrect". So each piece keeps its donor's character against the
    level-12 sets instead, and its donor weight: the Sniper high armour,
    almost no isolation, light (75 armour, 2-5 isolation, the lightest of
    each slot); the Veles heavy armour all round (78, above Molot's 75 and
    below level 15's 80-90), little isolation (5-15), heavier than the
    Sniper; the Veles Mask, a gas mask, high isolation (the owner) at a
    mask's armour (0.31's 70 x 0.44 = 30): 30/65, between level 8's
    "Smaragd" (65) and level 14's gas masks (70).
  - Level 12: drop 40, prices the live level-12 medians of the kind
    (vest 73,500, pants 36,750, helmet 45,500, mask, gloves and boots 18,380,
    backpack 45,985), Fringe and Army shop rank by srp's rule; built-in
    bonuses as they were; the Veles Mask none.

  | piece | before (@4, live): level, armour / iso, kg, price | after (@8) |
  |---|---|---|
  | Veles Vest | 10, 67 / 17, 8.2, 53,830 | 12, 78 / 15, 8, 73,500 |
  | Veles Pants | 10, 67 / 17, 2, 26,920 | 12, 78 / 12, 2.8, 36,750 |
  | Veles Helmet | 10, 50 / 13, 2.9, 35,580 | 12, 60 / 10, 1.2, 45,500 |
  | Veles Mask (new) | (@7: 10, 45 / 20, 1.8, 14,005) | 12, 30 / 65, 0.6, 18,380 |
  | Sorcerer Gloves | 10, 67 / 15, 0.3, 13,460 | 12, 78 / 5, 0.7, 18,380 |
  | Sorcerer Boots | 10, 67 / 17, 1.2, 13,460 | 12, 78 / 5, 1, 18,380 |
  | Token | 10, 3.3 kg, 39,670 | 12, 3 kg, 45,985 |
  | Sniper Vest | 10, 57 / 27, 8, 59,500 | 12, 75 / 5, 6, 73,500 |
  | Sniper Pants | 10, 57 / 28, 1.8, 29,750 | 12, 75 / 5, 1.5, 36,750 |
  | Sniper Helmet | 10, 35 / 28, 1.5, 35,580 | 12, 55 / 5, 1, 45,500 |
  | Sniper gloves | 10, 60 / 28, 0.25, 14,880 | 12, 75 / 2, 0.2, 18,380 |
  | Sniper Boots | 10, 60 / 22, 1.1, 15,300 | 12, 75 / 2, 0.5, 18,380 |
  | Spectrum-M Thermal vision | 10, 27 / 37, 0.5, 14,880 | 12, 27 / 37, 0.5, 18,380 |

  Against the level-12 sets, slot by slot (armour / isolation, kg; built-in);
  each piece trades armour, isolation and weight otherwise than each of them,
  none strictly better or worse (also `balance_audit.py`'s same-region peers):

  | slot | Veles (@8) | Sniper (@8) | Molot UM-2 | Omicron | Leshy / Master |
  |---|---|---|---|---|---|
  | vest | 78 / 15, 8; ammo bag | 75 / 5, 6; pocket | 75 / 20, 15; 2 ammo bags | 70 / 60, 11.5; pocket, pocket size | 65 / 25, 7; health regen +12 % |
  | pants | 78 / 12, 2.8; ammo bag | 75 / 5, 1.5; pocket size | 75 / 20, 3.5; stamina regen +8 % | 70 / 60, 2.5; artefact slot | 65 / 25, 2; pocket |
  | helmet | 60 / 10, 1.2 | 55 / 5, 1 | 55 / 30, 4.5 | 40 / 60, 1.8 | 20 / 25, 0.15 (Master's hood) |
  | mask | 30 / 65, 0.6 | 27 / 37, 0.5 (Spectrum-M) | - | 35 / 60, 0.8 | - |
  | gloves | 78 / 5, 0.7; spread -8 % | 75 / 2, 0.2; aim -8 % | 75 / 20, 0.4; recoil -8 % | 70 / 60, 0.5; artefact search -20 % | 65 / 25, 0.3; aim -8 % |
  | boots | 78 / 5, 1; stamina regen +8 % | 75 / 2, 0.5; energy -8 % | 75 / 10, 2 | 70 / 60, 1.6; energy -8 % | 65 / 25, 1; energy -8 % |
  | backpack | Token 3 kg; artefact slot, pocket size | - | - | Life support M-1 5 kg; health regen +12 %, stamina regen +8 % | - |
- **Army "OZK-UV"** (`anom_03`, 0.31 level 3) -> level 3, Army rank 0,
  drop 130: vest 25/40 5.5 kg 9,450 (pocket size); pants 25/40 2 kg 4,730
  (pocket); gloves 25/40 0.6 kg 1,190 (artefact search -20 %); boots 25/40
  1 kg 1,190 (the level-3 medians).
- **Missing pieces of sets 0.26g has** (the set's level and protection):
  - Fringe Stalker (level 9): Stalker's helmet 35/25 1.6 kg 26,250; Stalker's
    backpack 4.5 kg 35,000 (ammo bag + pocket); NVD (mask) 11,880: 30/20
    0.8 kg until @8, then open goggles (below).
  - Fringe Warrior gloves (level 9) 60/20 0.3 kg 11,880 (recoil -8 %).
  - Scavenger Pathfinder boots (level 9) 65/20 1.3 kg 13,130 (movement +4 %);
    Saboteur boots (level 5) 45/20 1 kg 4,460 (energy -8 %). Scavengers rank 5.
  - Army Scout's gloves (level 8) 50/40 0.3 kg 8,660 (aim -8 %), Army rank 3.
  - Black Market Watchman (level 9, rank 5): boots 60/30 1.3 kg 13,130
    (movement +4 %); helmet 30/35 1.4 kg 26,250.
  - Black Market "Marodeur" gloves (level 4, rank 4) 40/30 0.4 kg 2,620
    (reload +12 %; between the level-3 and level-5 prices); Biker pants
    (level 5, the Corsair set's model family, rank 5) 55/5 2 kg 8,930
    (movement +4 %).
- **@8, the owner's pick of 2026-11-05**, each with its donor's quirk (the
  owner: "make sure they fit properly and are well balanced with their
  quirks"): 0.69d0's own numbers set where the piece stands against its live
  peers of the same level and slot; its built-in bonus is the donor's, at the
  second-strongest strength (`built-in-bonuses-1`), unless that bonus makes
  no sense for the slot (the owner: "how boots can give reload speed"), when
  it is one of the slot's family (boots: movement, stamina spending or
  regeneration; gloves: recoil, reload, aim; masks: breathing) unlike the
  set's other piece of the slot. Prices: the live level medians of the kind
  in revision 50. 0.26g has no `slowdown` (0.69's 1 % movement penalty of
  some boots): carried weight slows the wearer instead.
  - **Veles Mask** (998, 0.31a2 `edge_mask_7`, `edge_keeper_07_mask`): with
    the Veles set at level 12 (above), 30/65 0.6 kg 18,380, drop 40. (@6 and
    @7 had 45/20 1.8 kg at level 10, after 0.69d's 50/20 0.6 kg; the owner:
    "it should be high isolation since it is a mask".) No built-in bonus:
    0.69d's recoil -3.5 % (the whole Veles set has recoil) is no mask's; @6
    gave it energy costs -8 % (1163), which the owner, having seen it in
    game, found unnecessary (2026-11-06). Template the "Smaragd" respirator
    (121): a gas mask, the donor's respirator sounds.
  - **Stalker's laced boots** (1098, 0.46c2 `edge_boots_6`,
    `edge_stalker_02_boot_new`): the boots 0.46c2 and 0.69d gave the Stalker
    set in place of the fur-topped Stalker's boots (143), which live has; both
    stay. 0.69d: light (5/8 0.8 kg against the fur boots' 10/5 1 kg). Here
    level 9, 45/25 0.8 kg 13,130, drop 70: less armour (the peer band's
    floor, 15 under the Stalker's boots' 60), more isolation, the lightest
    level-9 boots (the others 60-65 armour, 1.2-1.3 kg). Bonus: 0.69d's recoil
    -3.5 % (boots don't steady a gun) becomes energy costs -8 % (1163): light
    boots; the Stalker's boots have movement +4 %.
  - **"Shturm" gloves** (1108, 0.46c2 `renesanse_gloves_6`,
    `renesance_combat_hand`): the gloves of the Army Shturm set's own model
    family (the Shturm vest and pants, Vityaz-S helmet, Assault mask);
    0.46c2's "Assault gloves", a name live has (`ren_gloves_1`, the set's
    gloves so far, the novice model). 0.69d: heavy, 69/1 0.5 kg against the
    Sniper gloves' 54/5 0.3 kg. Here level 9, 70/2 0.5 kg 13,130, drop 70:
    the most armour of the level-9 gloves (the others 60), almost no
    isolation (they 10-25), twice their weight. Bonus: the donor's recoil
    (-2.8 %), kept, at -8 % (1153), as the Assault gloves'.
  - **Reinforced padded boots** (1097, 0.46c2 `scavenger_boots_6`,
    `scavengers_05_boot`): the boots of the Scavengers' Reinforced set
    (Reinforced padded jacket and pants, level 5, which had none); 0.46c2's
    "Reinforced boots", a name the Army's level-12 boots have. 0.69d: 10/2
    0.8 kg against the Saboteur boots' 5/2 0.6 kg (twice the armour,
    heavier, 1 % slower). Here level 5, 55/10 1.3 kg 4,460, drop 110: their
    set's protection, more armour and less isolation than the Saboteur boots
    (45/20 1 kg), the heaviest level-5 boots (the Black Market's 55/10 1.1 kg).
    Bonus: 0.69d's reload +10 % (boots don't reload) becomes stamina
    regeneration +8 % (1103): the set's other boots have energy -8 %
    (Saboteur) and movement +4 % (Pathfinder).
  - **NVD** (145, live since @1): open goggles on a head harness (renders),
    so no mask's protection: 0.69d has 7/8. Here 10/5 0.3 kg (was 30/20
    0.8 kg), the same price. In exchange a bonus no other gear has: scope sway
    -25 % (`breath_vibration_modifier`, 1211, its family's strongest: it pays
    for the protection given up). The engine reads it: the aiming booster's
    1210 (-20 %) reaches the player the same way (`armor::add_modifiers`,
    `player_params_modifiers_container::apply_modifier`, armour-params.md);
    no armour piece had 1210/1211 (`armor.tsv` had no column for it; its new
    `sway` column is the NVD's alone). Benchmark the Fringe respirator (102):
    the Assault mask's band (40 armour) is a closed mask's.
  - Names in every language without a duplicate of a live item name.
  - `balance_audit.py`: the Veles Mask against the Protective mask and the
    Spectrum-M; the laced boots against the Stalker's, Watchman and
    Pathfinder boots; the Shturm gloves against the five level-9 gloves; the
    padded boots against the Saboteur and Black Market level-5 boots; the
    NVD against the Assault mask: each better in one way, worse in another.
  - Fit, offline renders (`private/renders/equipment-046-more/`, the
    porter's rasteriser in solid colours: no textures hide a gap): the Veles
    Mask before (@5) and after (@6, the same mesh as @8) and on its donor's stand-in head, front,
    side and back; with the Veles helmet and over four collars; the laced
    boots under six pants (`data_long`); the padded boots under four
    (`data_medium`); the Shturm gloves on five jackets (`data_full`), the live
    Assault gloves and Stalker's boots as references. Nothing clips.
- **Premium**: Bandit's jacket "MK II" (1863), a premium copy of the Bandit's
  jacket (351, level 3) with its own model: the plain piece's combat values and
  default, +5 % experience and +5 % Black Market reputation as its first two
  modifications (`items.tsv` `like` and `start`, the lobby's `Equipment046.as`), drop 0,
  500 gold: premium gold prices follow level (the owner, "proportional",
  2026-10-30: level 3 -> 500, 6 -> 1,100, 9 -> 1,400, 12 -> 1,700, 14 ->
  1,900; the stock level-3 premium jacket is 500 too).
- `balance_audit.py`: 38 regular pieces, 79 comparisons with native,
  equipment-029 and each other at the same level, kind, regions and slots;
  none strictly better or worse. No exact peer: the Armored mask (no level-15
  mask), the Token (no level-10 backpack) and the "Marodeur" gloves (no
  level-4 gloves).

## Left out

- `blackmarket_helmet_3_summer17` (Summer helmet), `leagues_back_2` (Medic's
  Bag), `leagues_back_5` (Soldier's Backpack): only in 0.69d. resource-porter's
  0.69d converter (`face_items_069d.rs`) builds mask-slot items fitted on the
  native head; its surface conversion (`v069d_skinned`) would carry the meshes,
  but a backpack or a reskinned helmet needs a builder of its own (model folder,
  0.69d PBR materials and textures on 0.26g templates, back and helmet configs)
  and a pack of its own. Not done here.

## Needs a look in game

- Fit: the Armored mask and the Spectrum-M in place of the head (@4), with the
  Heavy armored and Sniper helmets on them; the NVD's goggles on the face;
  the Veles helmet's and vest's feathers; the Sniper set's ghillie parts.
- @3 again: the Watchman helmet (the lobby loads with it), the Stalker's
  backpack and NVD (the donor's materials), the Watchman boots (no emissive).
- The 39 icons in the lobby (0.46c2's are scaled from 128x128 pictures).
- @8: the Veles Mask textured and animated (its straps when the head turns,
  the inner rim from the side, with the Veles helmet), the laced and padded
  boots walking and crouching, the "Shturm" gloves in first person (0.46c2's
  gloves have no hud model, as the other 0.46c2 gloves here), the NVD's
  scope sway in a scope.

## Decisions

- Names (the owner, 2026-10-30): as proposed. Watchman for 0.46c2's "Sentry"
  boots and helmet, "Backpack for ammo" (0.31's), and the masks keep "NVD"
  and "Spectrum-M Thermal vision" although they are cosmetic (no night or
  thermal vision).

- The Token gets +1 pocket size besides its artefact slot (the owner,
  2026-10-30; 0.31 had only the artefact slot): a level-10 Wanderer container
  (`edge_back_5`, 1040 + 1170).

- The three 0.69d-only pieces (Summer helmet, Medic's Bag, Soldier's Backpack)
  are ported later, in their own pack with a new resource-porter builder (the
  owner, 2026-10-30).

## Open questions for the owner

- Drop weights and built-in bonuses as proposed?
- @8's names: "Shturm" gloves and Reinforced padded boots for 0.46c2's
  duplicates, Stalker's laced boots beside the Stalker's boots?
- The NVD's scope sway -25 %: enough for 20 armour and 15 isolation given up?
