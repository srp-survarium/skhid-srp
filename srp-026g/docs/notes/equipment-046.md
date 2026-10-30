# Equipment from 0.31a2 and 0.46c2 (pack equipment-046)

The owner's pick of 0.69d equipment that live Skhid (rev 38) lacks: 38 pieces.
35 ship in the content pack `equipment-046@4` (`scripts/equipment-046`); 3 are
left out (below). Every number here is a proposal: balance is the owner's call.

## What ships

- Donors: **0.31a2** for the 23 pieces it has (0.29c's format, the same as
  equipment-029's donor), **0.46c2** for the 12 only it has. Their models are
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
- Ids: 0.69d's own dictionary ids (988-1012, 1092-1105, 144, 145): none is
  taken by the stock db, a released patch or equipment-029. The premium copy
  takes 1863 (0.69d's 352 is the stock Bandit jacket premium).
- Icons: the donors' own item icons, in the free equipment-atlas columns 7
  (rows 1-22) and 8 (rows 1-13).
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
- **Level 10** (the owner, 2026-10-30), off level 9 where Fringe already has
  Stalker and Warrior. Each piece moved by a third of the step from its level-9
  to its level-12 native peers (armour, isolation, weight, price: vests and
  pants Stalker -> Leshy and Shturm -> Molot UM-2, gloves Stalker -> Master's
  and Assault -> Reinforced, boots Stalker -> Reinforced, helmets Pathfinder
  -> Combat, backpacks Watchman -> Life support M-1; the Spectrum-M the body's
  +2/+2, as no Army mask has level 12). Fringe and Army rank 5; drop 60;
  built-in bonuses kept.
  - **Fringe "Veles"** (`keeper_07`, 0.31 level 7): vest 67/17 8.2 kg 53,830
    (ammo bag); pants 67/17 2 kg 26,920 (ammo bag); helmet 50/13 2.9 kg
    35,580; Sorcerer gloves 67/15 0.3 kg 13,460 (spread -8 %); Sorcerer boots
    67/17 1.2 kg 13,460 (stamina regeneration +8 %); Token backpack 3.3 kg
    39,670 (1 artefact slot, +1 pocket size: the owner, 2026-10-30).
  - **Army "Sniper"** (`sniper_07`, 0.31 level 7): vest 57/27 8 kg 59,500
    (pocket); pants 57/28 1.8 kg 29,750 (pocket size); helmet 35/28 1.5 kg
    35,580; gloves 60/28 0.25 kg 14,880 (aim -8 %); boots 60/22 1.1 kg 15,300
    (energy -8 %); Spectrum-M thermal vision (mask) 27/37 0.5 kg 14,880.
- **Army "OZK-UV"** (`anom_03`, 0.31 level 3) -> level 3, Army rank 0,
  drop 130: vest 25/40 5.5 kg 9,450 (pocket size); pants 25/40 2 kg 4,730
  (pocket); gloves 25/40 0.6 kg 1,190 (artefact search -20 %); boots 25/40
  1 kg 1,190 (the level-3 medians).
- **Missing pieces of sets 0.26g has** (the set's level and protection):
  - Fringe Stalker (level 9): Stalker's helmet 35/25 1.6 kg 26,250; Stalker's
    backpack 4.5 kg 35,000 (ammo bag + pocket); NVD (mask) 30/20 0.8 kg 11,880.
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
- **Premium**: Bandit's jacket "MK II" (1863), a premium copy of the Bandit's
  jacket (351, level 3) with its own model: the plain piece's combat values and
  default, +5 % experience and +5 % Black Market reputation as its first two
  modifications (`srp_premium`, the lobby's `Equipment046.as`), drop 0,
  500 gold: premium gold prices follow level (the owner, "proportional",
  2026-10-30: level 3 -> 500, 6 -> 1,100, 9 -> 1,400, 12 -> 1,700, 14 ->
  1,900; the stock level-3 premium jacket is 500 too).
- `balance_audit.py`: 34 regular pieces, 62 comparisons with native,
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
- The 35 icons in the lobby (0.46c2's are scaled from 128x128 pictures).

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
