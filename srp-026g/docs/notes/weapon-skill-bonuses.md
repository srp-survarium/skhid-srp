# Skills on the VSSK rifles

The owner (2026-11-06): the SV-1367 Dusk was a recoloured VKS "Vychlop" with three free bonuses
at the same price. "There are skills people almost never play with: let's have Dusk one and
Vychlop the other." Prices stay 165 000 for both.

## The rules

- Dusk (`vssk_camo`, 2250) loses its three built-ins: chambering -15 % (2054), aiming time
  -10 % (2074), reliability -25 % (2094, wear only; wear is off).
- Each gun gets one rarely taken skill as a built-in, the skill's whole effect:
  - Vychlop (`vssk`, 365): **Bear arms** (skill 112). The gun's own movement penalty (-5 %) is
    gone. It is built-in 1114 (movement speed +5 %, the gear's row): the cook adds it to the gun's
    `parameters/movement_speed_modifier`, -0.05 + 0.05 = 0. With the skill too, nothing more.
  - Dusk: **Accuracy** (skill 106, hip fire, at its 5 ranks). Hip-fire spread is 10 % lower. It is
    built-in 2120, which multiplies the gun's `dispersion/from_the_hip_multiplier` by 0.9 (4 ->
    3.6). It stacks with the skill: 0.9 x (1 - 0.02 a rank).
- Why the whole skill, not one rank: Bear Arms has one rank, so its rank 1 is all of it. Skill
  106's rank 1 is -2 %, which nobody would notice; -10 % keeps the two even.
- The other stats are unchanged: the two guns are the same at level 15 apart from these.
- Owned guns: a built-in belongs to the item's config, not to the saved item. Every owned Dusk and
  Vychlop gets the change with the content patch, with no migration. A Dusk can now roll
  chambering and aiming time on an upgrade or reroll, since their types are no longer taken by a
  built-in. Its own modifications stay as they are (none can be reliability, chambering or
  aiming, because those types were held by built-ins).
- Card: Vychlop shows its own "Movement speed: -5 %" line, and under it the built-in
  "Movement speed: +5 %". Dusk shows "Dispersion off the hip: -10 %" (property 101, as the gear's
  dispersion rolls show it; no new text).

## Why these two (live data, 2026-11-05)

Accounts at level 10 or more that opened the lobby in the last 14 days (194), by skill, from the
srp.db backup of rev50's release:

| skill | taken by | on a gun? |
|---|---|---|
| 214 Heavyweight | 1 | no: a player modifier (a weapon registers none) |
| 111 Terminator | 8 | no: a kit layout |
| 213 Sprint | 11 | no: player |
| 207 Paratrooper | 12 | no: player |
| 211 Unstoppable | 14 | no: player |
| 113 Deep Pockets | 15 | no: a kit layout |
| 205 Second wind | 17 | no: player |
| 110 Equanimity | 19 | no: player |
| **106 Accuracy** (hip fire) | 28 | yes: the gun's `from_the_hip_multiplier` |
| 105 Iron lungs | 29 | no: player (index 23); the gun's breath keys are its sway |
| 203 Grenadier | 32 | no: player |
| **112 Bear arms** | 36 | yes: the gun's own `movement_speed_modifier` |

The medical skills (301..342, 0..17 accounts) are left out. They have been in the tree only since
rev50 (2026-11-05), and their ranks are packed bits of kit modifiers 43 and 45, which a gun cannot
add to.

## How

- A weapon never registers player modifiers: its slot 0x1AF330 is a bare `ret`
  (`weapon-rolls.md`). Only the gun in hand adds its own movement node, which Bear Arms cancels
  (`alt-weapon-stats.md`). So each pick changes one of the gun's own parameters, through the
  match's item cook (`items.md`: type 1 adds, type 2 multiplies, and an int becomes a float first).
- Data: `weapon-skill-bonuses-1.json` appends modification 2120 (type 42, its own, drop weight 0:
  never rolled). Its row is in `modifications.tsv`. `weapon-stats.tsv` has two new built-in columns,
  `movement_bonus` (ids 1110..1119) and `hip_bonus` (2120..2129); a gun with no
  `default_modifications` gets the list (Vychlop had none).
- Server: nothing. It reads the defaults from its game data; rolls already leave out a type an
  item holds, built-ins included.
- A simulation change: the `next` row of `simulation_changes.tsv`.
- Release: the content patch only.
