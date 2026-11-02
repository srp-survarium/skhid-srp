# Weapon rolls that take effect

The owner, 2026-10-29: "let's fix!". Two weapon modifications rolled, showed on the card and did
nothing; a third worked but the card never showed what it changes. The edit list
`scripts/content-edit/weapon-rolls-1.json` makes the first two work with the same percentages, and
the lobby card shows all three (`flash/lobby_menu`, `WeaponDetails.as`, `ItemCompare.as`).

## How a weapon's modification applies

- The item cook (`item_config_cook`) finds each modifier's `path` in the weapon's config
  (`find_modifier_target` 0x2F1430) and skips one whose key the config lacks (0x2F1ABF), with no
  message. Per key it sums the values, then type 1 adds (0x357F80) and type 2 sets
  base x max(0, 1 + sum) (0x358010; an int value becomes a float first).
- The weapon reads the cooked config: its recoil (`weapon_recoil_parameters::load` 0x331840) and
  its `parameters/aim_time` (weapon+0x5F8, `on_weapon_config_loaded` 0x30AC7E).

## What changes

- **Recoil (prop 115):** 2080-2084 (-2, -4, -6, -8, -10 %) and the defect 3060 (+10 %, "DTK
  broken"). Their twelve modifiers targeted `recoil/max_first_shoot_*`, `*_queue_shoot_*` and
  `*_back_recoil`, keys no 0.26g weapon has: nothing applied. Now six modifiers, one per key the
  loader reads: `min/max_top_recoil`, `min/max_left_recoil`, `min/max_right_recoil`, type 2, the
  same value. -10 % is 0.9 x every kick bound; the defect 1.1 x. Every one of the 83 weapons has
  the six keys.
- **Aim time (prop 102):** 2070-2074 (-2..-10 %). They targeted `parameters/aim_time_modifier`
  (type 1), a player modifier: the armour and gloves register theirs with the player
  (`armor::add_modifiers` 0x33F3A0), a weapon never does (its slot 0x1AF330 is a bare `ret`). Now
  `parameters/aim_time`, type 2, the same value: -10 % is 0.9 x the weapon's aim time. The
  player's aim-time sum (skills, gloves, Spring) still multiplies the result (artefacts.md).
- **The VSSK camo** has 2074 as a default modification: its aim time 0.4 s -> 0.36 s, as its card
  always said.
- **Chambering (prop 119):** 2050-2054 (-5..-15 %) and the defect 3050 (+10 %) on
  `parameters/chamber_a_round_time`, type 2: already working; unchanged.
- **Reload (prop 114):** 2040-2044 and the defect 3040: already working, unchanged (checked
  2026-11-01, below).
- Dispersion, reliability and every other roll: unchanged. Ids, values, card texts and
  `lobby_info` unchanged: owned items keep what they have and now get its effect.
- A simulation change: the `next` row of `crates/session/src/v026g/simulation_changes.tsv`.
  The server reads modifications from its game data and simulates neither recoil nor aim
  time: no server code.

## Reload rolls (checked, no change)

- Data (rev 40): 2040-2044 are `parameters/reload_time`, type 2, **-0.05, -0.08, -0.10, -0.12,
  -0.15**; the card text (prop 114) is the opposite sign, +5..+15 %. The defect 3040 is +0.10
  (card -10 %). Every weapon has `parameters/reload_time`, so the cook applies them: -15 % is
  0.85 x the reload time.
- The weapon reads it with its other state times (`weapon_core_cook::query_weapon_states`
  0x30B2B0, block +10h). The reload state (0x30E000; the pistol and double-barrel ones alike)
  keeps clip / reload_time (`computed_reload_animation_time_scale` 0x30D750) and plays the clip
  at that x max(0, 1 + S13), the player's reload speed (`get_weapon_lexeme` 0x30E1F0 through
  0x2D5AE0): a reload lasts reload_time / max(0, 1 + S13). A tube reload (bolt and pump guns)
  splits reload_time over the rounds (0x30EE40); its start and end times take no roll.
- The exe's own 0x358010, 0x30D750 and 0x30E1F0 run in Unicorn: a 3.167 s reload is 3.009 s
  with 2040 and 2.692 s with 2044 (3.484 s with 3040), the card's base x (1 - 0.05) and
  x (1 - 0.15). Card and game agree.

## The card

- The kick rows (props 146-151: upward and sideways range, their variation) scale by
  max(0, 1 + the weapon's sum of 115).
- The stock aim-time bar (prop 133) takes the weapon's 102 rolls: 133 x max(0, 1 + sum)
  (`ItemCompare.MODIFIED_BY`).
- A chambering row (prop 160, `parameters/chamber_a_round_time`, carried by
  weapon-details-ui-1 on the 11 bolt and pump weapons that have it): the time with its rolls
  (`1.13s`), as every other row (the owner, 2026-10-29); lower is better. Beside a compared weapon
  with one, a weapon without one shows a dash, so the rows stay aligned. Label
  `st_srp_weapon_chamber`.
- Only the weapon's own modifications (with its defaults), never the player's.
