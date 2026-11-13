# Weapon stats in a match (hold Alt)

The owner, 2026-11-01: "pressing alt should show the stats affected by everything gloves + skills +
whatever else too".

## The owner's rules

- **Alt held** over a weapon's card in the lobby: its stats as they will be in a match, with
  everything that applies. **Released:** the weapon's own stats, as before.
- **Each stat a modifier changes reads "original -> current"**: original is the weapon's own base
  value, before its own rolls (the owner, on #1033); current the value with everything applied. A
  stat nothing changes shows its one value.
- **One line per stat**, in the card's own rows and bars; the arrow is plain `->` (no glyph risk).
  Held Alt, a value too wide to sit beside its label (the recoil rows) takes the line under it: two
  lines for that stat, accepted by the owner on #1033.
- **Never "x -> x"**: a changed value is shown to the first decimal at which the two differ (aim time
  0.2 s with a -2 % roll: `0.2 -> 0.196s`, where the card's two decimals read 0.19).
- **The current value's colour says whether the change is good** (the owner, on #1033): green when
  it is better for the player, red when worse, by the stat's direction (less aim time, reload,
  chambering, draw, kick, spread is better; a kick range is smaller when its ends come nearer 0).
  The card's own green and red, those of the compare view. The original and the arrow keep the
  neutral value colour; a stat with no change keeps its usual colour; Ctrl (neutral colours)
  leaves the change neutral too (my pick).
- **One short line says what is on:** "With kit and skills".
- **The hint line names Alt** (the owner, on #1033): `st_srp_weapon_shift_hint`, one short line in
  every language, "Shift: details · Alt: in a match" (the longer "Alt: with kit and skills" does not
  fit the card's line in most languages; the line under the heading says it once Alt is held).
- Real formulas only: each as the match's code applies it (below).

## What a match adds (my picks)

- **The kit:** the selected kit (the one the paper doll shows): its armour's modifications (built-in,
  rolled and defects; every slot from the head to the boots, the mask and backpack included) of a
  player modifier the weapon uses. A weapon's card shows the kit's effects wherever the weapon is
  (inventory, shop, the kit), as if it were in that kit.
- **The skills:** the account's skills, which srp sends as the kit's own 68 floats (the exe forwards
  them to the movie, below).
- **The weapon's own modifications** (rolls, built-in, defects): as the card already applies them.
- **The compared weapon** (the second card) gets the same kit and skills, so the comparison holds.
- **Left out, and why:**
  - artefacts: found in a match, never part of a lobby kit;
  - drugs and boosters: used in a match, for seconds;
  - injuries, stamina and the stance: the match's state. Spread is shown as standing, walking,
    crouching or aiming (their skill weight is 1; running, prone from the hip, jumping and low
    stamina 0, `default.player` `character_dispersion_skill_influence`);
  - aiming speed (index 1, spread recovery): no skill or item gives it in 0.26g;
  - breath vibration (gear 1210/1211) and breath holding (skill 105): where they meet the card's
    sway rows is not traced;
  - the dispersion bar (`base_dispersion`): no player modifier reaches it (PROVEN: its only readers
    are 0x331225, 0x2451F4 and 0x2E2173, and none of the 97 modifier-getter call sites is in their
    functions).

## The formulas

A player modifier's sum S = the kit's float (skills) + its armour's modifications of it
(`armor::add_modifiers` 0x33F3A0). `apply_modifier` 0x2D5AE0: B x max(0, 1 + k x S), k = 1 at
every site below (multiplicative indices; the additive list 0x927E08 has none of them).

| card stat | S | in a match | where |
|---|---|---|---|
| Aim time (133) | 49 `aim_time`: skill 101, gloves 1080..1084 | x max(0, 1 + S) | `instant_aim_start` 0x2E1B88 (PROVEN) |
| Reload (138) | 13 `reload_speed`: skill 108, gear 1140.. | / max(0, 1 + S) | the reload animation's time scale (length / `reload_time`, ctor 0x30E000) x max(0, 1 + S), 0x30E210; shotgun, pistol, double barrel the same (PROVEN) |
| Chambering (160) | 21 `chamber_a_round`: skill 103 | / max(0, 1 + S) | time scale length / `chamber_a_round_time` (0x30E2B0) x max(0, 1 + S), 0x30E55B (PROVEN) |
| Draw, holster (142, 143) | 22 `change_weapon_speed`: skill 104 | / max(0, 1 + S) | time scale length / `show_time` (`hide_time`), 0x30D850 / 0x30DE20, x max(0, 1 + S) at 0x30DF7B, shared by both (PROVEN) |
| Kick ranges (146..151) and their variation | 18 `recoil`: gear 1150.. | x max(0, 1 + S) | each axis of a shot, `weapon_recoil_calculator::on_shot` 0x331D95 (PROVEN) |
| Hip-fire spread (152) | 56 `dispersion_from_hip`: skill 106; 0 `dispersion`: gear 1070.. | x max(0, 1 + S56) x max(0, 1 + S0) | the stance's spread from the hip 0x331776, the whole target 0x3313C1 (PROVEN; weight 1 standing) |
| Spread per shot (153) | 0 `dispersion`: gear 1070.. | x max(0, 1 + S) | `dispersion_calculator::fire` 0x331150 (PROVEN; weight 1 standing) |

The weapon's own reload rolls (2040.., `parameters/reload_time` type 2) stay as the card already
shows them (time x (1 - sum)); the match's cook gives `reload_time` x (1 + sum), a longer reload
(DERIVED: an item with a default or own modification or upgrade gets class 0x43 in
`make_item_config_request` 0x2F14C0, `item_config_cook` 0x2F1750's output, and the weapon is built on
that resource, 0x30AAB3; the state query's own target, the functor 0x30C160, is not read). Not changed here: the owner gave it to its
own fix (`fix/reload-rolls`).

## Movement speed under Bear Arms

The owner, 2026-11-05: "when pressing alt to see final numbers, if you have bear arms, you should see
movements speed: -5% -> 0%", "on weapons that it fixes". Held Alt, the weapon's own movement line (a
built-in bonus line: `WeaponMovement`, prop 105 from `parameters/movement_speed_modifier`) reads
"Movement speed: -5% -> 0%" on exactly the weapons Bear Arms cancels it on, the current value green
(the bonus lines' green; neutral with Ctrl, as the stats' changes). Every other bonus line, and a
bonus movement line, keeps its text.

- **The game's rule** (PROVEN, read in the exe): when a weapon is put in hand,
  `weapon_core::initialize` 0x2E2A30 (from `weapon::initialize` 0x2449E0, the vtable+14h of
  `survarium::weapon` 0xA04174, the only class whose activation reaches it: every firearm, either
  weapon slot) adds the weapon's movement node (+610h, value at +61Ch) to the kit's container unless
  `apply_modifier(62, 0, 1)` (Bear Arms, skill 112; additive, so 0 + S62) is above 0 **and** the
  value is not above 0 (0x2E2C5C..0x2E2C9C). `weapon_core::deserialize` 0x2E3490 makes the same
  test at 0x2E37A4. So Bear Arms cancels every firearm's penalty, whichever slot; a bonus always
  counts; a weapon without a movement line has nothing to cancel.
- **S62 in the lobby:** the kit's own float 62 (`root.set_kit_modifiers`, the floats the Alt view
  already has) plus the armour's modifications of 62 (none in 0.26g). The weight callback's Bear
  Arms flag is made from the same floats (`weight_ui_026g.c`: `apply_modifier(62, 0, 1)`), so the
  card and the speed's tooltip (`SpeedParts`) agree; no new exe call.
- **Where:** every card with the Alt view is an `ItemInfoRenderer` (shop, inventory, kit slots, the
  craft window's two cards); it refills its bonus lines when Alt (or Ctrl) changes.
- **Checked:** `WeaponDetailsHarness` `bearArmsChecks` (Ruffle): a -5 % weapon with Bear Arms on and
  off, a +3 % weapon, Ctrl, another built-in line beside it, no floats, Alt pressed and let go over
  an open card.

## Medicines

The owner, 2026-11-04: "Alt on medkits and painkillers should show final numbers, after the skills
we implemented." Held Alt over a medicine's card (category 10: bandages, the five medkits,
antitoxin, painkiller), each line a medical skill changes reads "original -> current", coloured as
the weapon's. No effects line and no hint: the card has neither. The ranks come from the kit's
floats 43 and 45 (packed, `docs/notes/medical-skills.md`), parsed as the exe's `medical_skills` does
(a refused value of either is no skill); Medical Training is also float 57. Every rule is the exe's
(`client-patches/rust/patches/src/medical_skills.rs`, `vostok/medical.rs`):

| card line | in a match |
|---|---|
| Activation (19) | x max(0, 1 + S33) (set_active 0x33D45A), then Fast Bandaging x (1 - 0.05 x rank) on a medicine that heals (its card has a healing line: `data/influences`; antitoxin too, not the painkiller) |
| Duration (18) | + 1.5 s a rank of Potent Painkiller on a pain protector (the painkiller) |
| Pain protection (17) | c = (1 - p) x max(0, 1 + S58); Potent Painkiller: max(0, c - 0.01 x rank), a percentage point a rank; shown 1 - c (10 % -> 13 % at rank 3) |
| Healing lines (135, 136, 137, 13) | x max(0, 1 + S57) (medkit::tick 0x33D790; Medical Training +0.08 a rank), at most 100 % of the pool |

Left out: S34 (`drug_activity_time`, the duration's weight): no skill or item gives it. No skill
gives S33 or S58 either; they are there as the exe applies them.

The owner, 2026-11-06: "when I press alt on painkillers I should see improvements from medical
skills". The skills that act while a painkiller is active, or while a medicine heals, change no
line of the card, so held Alt each one with a rank adds a line under the card's own, a share of the
game's own value, "100 -> current %", coloured as the others (ranks from float 45, parsed with
float 43 as above):

| line (`st_srp_medicine_*`) | on | in a match |
|---|---|---|
| Sprint stamina (`sprint_stamina`) | a pain protector | Numb Legs: the sprint's stamina x (1 - 0.2 x rank) while it is active (100 -> 60 % at rank 2) |
| Slow from bullets (`hit_slow`) | a pain protector | Unflinching: the hits' stopping power x (1 - 0.1 x rank) while it is active (100 -> 60 % at rank 4) |
| Sight sway (`sight_sway`) | a pain protector | Concentration: the sight's sway x (1 - 0.05 x rank) while it is active (100 -> 80 % at rank 4; in a match added to the kit's own sway bonuses) |
| Healing crouched (`crouched_healing`) | a medicine that heals | Take Cover: each heal tick x (1 + 0.1 x rank) while crouched, after Medical Training (100 -> 120 % at rank 2) |

The other medical skills (Metabolism, Adrenalin, Strong Bones, Brothers in Arms, the pockets)
do nothing a medicine's card shows.

## Grenades and traps

The owner, 2026-11-05: "let's add them, let's also make sure alt modifies things (I think throw
speed?)". Held Alt over a grenade's or a trap's card (categories 24 and 11: the RGD-5, the RDG-M
smoke, the trap), each bar a modifier changes reads "original -> current", as the medicines'. The
cards' bars are `throwable-cards-1` (presentation only): damage is on each body hit (the RGD-5's
`splash_max_damage` at the centre on each of its six splash bodies, the trap's on the foot), so
prop 1's scale (150) stays the weapons'. Every rule is the exe's (`apply_modifier` 0x2D5AE0;
none of the three indices is on the additive list 0x927E08):

| card line | in a match |
|---|---|
| Throw speed (169, `data/throw_power`, m/s) | the grenade's velocity: the throw's forward axis x throw_power x max(0, 1 + S27) (`grenade_set_core::throw_grenade` 0x30A250, call 0x30A2DE; to `set_linear_velocity`): skill 203 Grenadier +0.1 a rank, 2 ranks (17 -> 18.7 -> 20.4 m/s) |
| Damage (1), stopping power (132), stamina damage (167) | the grenade's splash min and max damage, stamina damage and stopping power x max(0, 1 + S11) (`grenade_set_core::splash_hit_params` 0x30A350); the trap's hit x max(0, 1 + S11) (`booby_trap_core::hit_victim` 0x304A40) |
| Radius (23) | x max(0, 1 + S12) (0x30A350; the trap's splash 0x3055F0, unused: `is_explosive` 0) |

Unchanged in a match: armour penetration (`splash_pierce`, the trap's `armor_piercing`), the fuse
(`ttl_ms`), the trap's defuse time (no `apply_modifier` reads S9 `engineer_use_time`). Only S27 has
a source today (Grenadier); S11 and S12 are there as the exe applies them (no skill, roll or item
gives them). Pocket counts (S15, S17, `pocket_units` 166) are the paper doll's, not a card bar.

## Armour: isolation

The owner, 2026-11-10: "alt on items ... should work and be affected by resistance skill in
anomalous skill tree"; "alt on equipment for isolation". Held Alt over a piece with an Isolation
line (prop 8, `st_inv_ano_armor`: armour; a piece without the line has nothing to show), the
line reads "original -> current" with Resistance (skill 502, `artefact-skills.md`), green. Every
other line of the piece is its own; as on every Alt card, the original is the piece's base, before
its own rolls (an isolation roll and the skill: `30 -> 60`).

| card line | in a match |
|---|---|
| Isolation (8) | min(100, points + 5 x rank), rank 0..5 from the kit's float 40, bits 0..2 |

- **What the line is:** the piece's `hit_params/<part>/hit_types/<toxic, radiation, fire,
  shock>/armor` x100, one number for every part it covers (`content_edit.py` `ano_armor`; every one
  of the 249 armour configs with `hit_params` keeps the four equal over all its parts: 585 rows, 0
  differ, revision 56; the card's line is its own text, `ui_desc/props_list`).
- **The match** (PROVEN, `armour-params.md`, `artefact-skills.md`): a part's armour A against a
  hit's type is the sum over the worn pieces that cover the part (0x33F2DD); the hit keeps
  D x (1 - clamp(A - P, 0, 1)) (0x32A07E). Resistance is our hook at 0x32A083
  (`artefact.isolation`, `artefact_skills.rs` `Skills::armour`): A + 0.05 a rank for the four
  anomalous types, before the piercing and the clamp. So no point past 100 stops more (the cap;
  P for an anomalous hit is the zone's `max_armor_piercing`, `armour-params.md`).
- **Once a part, not once a piece:** the +5 a rank is added to the part's sum, whatever covers it
  (a bare part too). A card's "30 -> 35" is true of the parts that piece covers; where two pieces
  cover one part the skill counts once, so the cards' currents do not add up. A kit total (per
  body part) would be the place for the whole of it; there is none in the lobby.
- **The rank** is parsed as the exe's `Skills::parse` (an exact integer below 2^14, the rank at
  most 5, Keeper's draw at most 6; otherwise no skill). The 5 points a rank are the patch's
  `Resistance::ARMOUR`, typed in `MatchEffects` as the medical ranks' rules are: the movie gets
  no number of an exe skill.
- **Left out:** modifier 8 `anomaly_damage` (x max(0, 1 + S8) on an anomalous hit before armour,
  0x332890): no skill or item gives it, and it is no line of a piece. Forest Protection (523)
  is bullets inside an anomaly, not isolation.
- **Checked:** `WeaponDetailsHarness` `isolationChecks` (Ruffle): ranks 0, 1, 5, other skills'
  bits beside it, refused values, the compared card, the colour, the cap, an own roll, a piece
  without the line, no floats. Not run in a game.
- **The harness, 2026-11-10:** `throwableChecks` and `medicineChecks` follow the cards of
  `one-pocket-a-type.md` (Pocket slots first, the weight last, a missing stat as 0). It found two
  faults of those cards held Alt, fixed with it: a painkiller beside a medkit with two skill lines
  had ten rows for a card of nine and lost the second skill line (the skill lines keep their rows
  now, and a stat the item has none of goes first: `ItemCompare.fitted`), and a stat only the
  compared item has went through the skills as the item's own (a medkit beside a painkiller read
  "Pain protection 0 -> 3%"). The whole harness passes.

## How

- **Exe** (`weight_ui_026g.c`): before each weight callback (`update_player_items_weight`
  0x2221E0, from `on_profile_changed` and each inventory action) it also calls
  `root.set_kit_modifiers` with the selected kit's 68 floats, by index. UI only.
- **Movie:** `KitModifiers` (parsed once), `MatchEffects` (the sums and the formulas),
  `LocalPlayerProfile.heldMatchEffects`; `ItemCompare` (the bars), `WeaponDetails` (the rows),
  `LabelDiffIndicator` (a changed value's text; a value too wide for its label's line takes the
  line under it); Alt is `MainScene.isAlt` (VK 164/165 from the exe's DIK map, 0x1FE8EA, 0x1FE8C6),
  cleared when the window loses focus. Texts `st_srp_weapon_with_kit_skills`, `st_srp_weapon_with_kit`
  (an exe that sends no floats: the kit alone).
- **Release:** the lobby movie and the dictionaries (content) and the exe (the floats). Nothing a
  match computes: no `simulation_changes.tsv` row.
- **Checked:** `scripts/weapon-details/WeaponDetailsHarness.as` (Ruffle; the medicines in
  `medicineChecks`: the seven live cards, no skill, each skill alone and all at their most ranks, refused ranks,
  no floats, Alt pressed over an open card, the combo skills' lines on every live card at each rank
  alone and both full sets, their colours and every language's line beside its value; the grenades and the trap in `throwableChecks`: the three
  live cards, Grenadier ranks 1 and 2, S11 and S12, a sum below -1, no floats, Alt pressed over an
  open grenade card), `check_weight_ui_026g.py` (Unicorn). Not run in a game.
