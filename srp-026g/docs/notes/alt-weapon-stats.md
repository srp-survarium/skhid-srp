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
  - the dispersion bar (`base_dispersion`): no player modifier reaches it (its readers 0x331220,
    0x2E2173 call no `apply_modifier`); LIKELY;
  - movement speed (the weapon's built-in line, Bear Arms): a bonus line of the card, not a stat.

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
(LIKELY: the cooked config is the one 0x30B33E reads). Not changed here: the owner gave it to its
own fix (`fix/reload-rolls`).

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
- **Checked:** `scripts/weapon-details/WeaponDetailsHarness.as` (Ruffle), `check_weight_ui_026g.py`
  (Unicorn). Not run in a game.
