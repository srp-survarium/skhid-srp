# Booster buffs (the owner, 2026-10-26)

Four boosters made stronger, each on top of what it does. Edit lists in `scripts/content-edit/`:
`radio-respawn-1`, `rations-stamina-1`, `protein-aim-1`, `artefact-master-1` (applied in that order).

## The rule

- **Radio** (`respawn_booster`): "respawn +60%". Its respawn bonus is replaced: respawn speed
  +60 %, the time x 1/1.6 (was the time -20 %).
- **Rations** (`regeneration_booster`): "energy costs -30%", beside their regeneration +15 %.
- **Anomalous Force Protein** (`weight_booster`): "aiming speed -15%", beside its weight limit
  +5 kg.
- **Artifact-master** (`artefact_booster`): "collection time -60%". Its own bonus is changed
  (was -25 %).
- Each card shows the old effects and the new one together.

## How a booster works (PROVEN, Unicorn: `client-patches/check_booster_modifiers_026g.py`)

- Carried, not used: no key and no timed effect. Every 0.26g consumable buff of this kind is a
  booster, so the new bonuses work the same way: while it is in a pocket.
- Its config's `default_modifications` are applied to its `parameters/<name>_modifier` floats by
  the client (`item_config_cook::translate_query` 0x2F1750; a modifier of type 1 adds). The
  config reader 0x2C83E0 reads the 68 names of the table 0x928448; a misspelt name is read by
  nothing.
- At every spawn the booster makes one node for each nonzero float (0x33F690, no filter) and links
  it into its index's list in the kit (0x33F960 -> 0x33F990).
- The getter 0x2D5AE0 gives base x max(0, 1 + the sum of the list) for these four indexes (none
  is in the additive table 0x927E08). Other sources (skills, gear, artefacts) add to the same sum.
- One pocket a type (`one-pocket-a-type.md`): one booster of a kind counts once.
- The match's simulation computes all of it in every game: a `next` row in
  `crates/session/src/v026g/simulation_changes.tsv`. srp computes none of it (the server bots
  carry the radio and get its respawn from the clients).

## The values

| booster | modification | parameter (index) | card line |
|---|---|---|---|
| Radio | 1220 (only the radio has it), -0.2 -> -0.375 | `respawn_time_modifier` (60) | new line 135 "Respawn speed": +60% (was line 124 "Respawn time": -20%) |
| Rations | 1094 kept; new 1165, -0.30 | `stamina_spending_speed_modifier` (19) | "Regeneration: +15%", "Energy costs: -30%" (line 116) |
| Protein | 1124 kept; new 1085, -0.15 | `aim_time_modifier` (49) | "Weight limit: +5kg", "Aiming speed: -15%" (line 102) |
| Artifact-master | 1134 -> new 1135, -0.25 -> -0.60 | `artefact_search_time_modifier` (7) | "Collection time: -60%" (line 108) |

- The new modifications are made like the rolled ones of their type (1164, 1084, 1134), with
  `drop_weight` 0: they never roll. 1134 itself is a roll of gear, so it is not changed.
- They are added at the end of the table: the client sizes it by the file's count and finds an id
  by a linear scan (0x2CA000, 0x2F1818, 0x229F23). An item holds up to 8 default modifications.
- Line 135 is made like 124 (a percentage), "higher is better", text `st_srp_respawn_speed` in the
  six localization tables (Respawn speed, Geschwindigkeit der Wiederbelebung, Szybkość odrodzenia,
  Скорость возрождения, Швидкість відродження). Line 124 stays as 0.26g has it.

## What each does in a match

- **Respawn** (the respawn rule's `on_event` 0x316AA0): delay = `respawn_time` x max(0, 1 + sum),
  rounded up to the 50 ms grid. The HUD shows the seconds left rounded up.

  | mode (`respawn_time`) | nothing | 0.26g radio (-0.2) | radio now (-0.375) |
  |---|---|---|---|
  | Team Deathmatch (8 s) | 8 s | 6.4 s | 5 s |
  | battery modes, Research (10 s) | 10 s | 8 s | 6.25 s (HUD 7) |

  Read as "respawn time -60 %" (-0.6) it would be 3.2 s and 4 s. The ragdoll, the death camera and
  the death fade handle a respawn at any of these times (`ragdoll.md` section 15).
- **Energy costs** (index 19): the jump's cost (`player_stamina::jump` 0x329710), the can-jump
  test (0x2D6420) and the sprint's drain per second (0x329A70), each x max(0, 1 + S19) x
  max(0, 1 + S67) (67: the skill 201's own index). Only sprint and jumps: a hit's stamina loss
  (66) and breath holding (61) are other indexes. A jump of 10 costs 7; sprint 10/s drains 7/s.
- **Aim time** (index 49): scope-in and scope-out (0x2E1B50, 0x2E1BF0) take the weapon's
  `aim_time` x max(0, 1 + S49): 0.4 s -> 0.34 s. Springs, gloves, weapons and the skill add to
  the sum (`artefacts.md`, "Aim time at its limit").
- **Collection time** (index 7): an artefact container's use (`use_execute` 0x31C780) finishes
  after ceil(`artefacts_search_time_sec` x 1000 x max(0, 1 + S7)) ms: 5 s -> 2 s (was 3.75 s),
  10 s -> 4 s (was 7.5 s).

## Release

- The four rows of `release/content.tsv`; `release/build-content` packs the five gameplay files
  (the three boosters, `modifications.options`, `static_game_parameters.options`) and the six
  localization tables.
- Not run in a game.
