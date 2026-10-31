# Medicines (the owner, 2026-10-31)

The bandages, the Medikit and three coloured medkits (blue, green, yellow), each made whole from one
table: `scripts/medicines/medicines.tsv`. `scripts/medicines/make_pack.py` makes the pack `medicines`
from it (its row in `release/content.tsv`); content-edit `medicines-1` points the bandages' and the
Medikit's dictionary rows at their new configs; the texts are in `client-patches/localization/*.tsv`.

## The owner's rules

- Green, yellow and blue medkits, from the builds that have them ("they should be present in some
  patches"); not the later medkit model (0.69 changed it): the old box, the stock medkit's look.
- All five stand on their own: none better than another in everything.
- One table for all of them ("we can just rebuild it completely for all items").
- Yellow heals intoxication and radiation, always completely; gradually is fine.
- A bullet hit stops a drug's use or its healing: an exe patch of its own (branch
  `feat/drug-hit-interrupt`), for every drug.

## The table (the owner's, 2026-10-31)

| | use | active | arms, legs | body, head | pain | other | kg | silver | sold by |
|---|---|---|---|---|---|---|---|---|---|
| Bandages | 1 s | 1 s | 100 % + injuries | 35 % | 35 % | | 0.1 | 10 | Scavengers r1 |
| Orange Medikit | 2 s | 3 s | 100 % + injuries | 100 % | 10 % (was 0) | | 0.2 | 20 | Scavengers r3 |
| Blue Military | 1 s | 1 s | 100 % + injuries | 60 % | 20 % | | 0.3 | 40 | Renaissance r1 |
| Green Black Market | 2 s | 2 s | 0 | 100 % | 100 % | | 0.3 | 40 | Black Market r3 |
| Yellow Science | 2 s | 3 s | 100 % + injuries | 100 % | 0 | intoxication and radiation | 0.3 | 40 | Fringe r1 |

- What each wins at: bandages light, cheap, a bit of everything; orange the full heal with a little
  pain; blue a big heal in 1 s; green the one full pain cure; yellow the full heal that cures
  intoxication and radiation.

## How the table becomes a config

- A drug heals `amount` of a pool a second over its active time, after its use time
  (`bandages.md`). Pools: body 1.0, head 0.4, each arm and leg 0.5, pain 1.0, `infection`.
- 100 % is 1 a second (0.26g's Medikit: the body full in 1 s); less is that share of the pool over
  the active time (bandages' 35 % of body in 1 s: 0.35; the Medikit's 10 % of pain in 3 s:
  0.0333). So the bandages are as before, the Medikit as before but its pain.
- Every config is 0.26g's `medkit.options` with the row's keys, icon cell, faction, levels, times,
  weight, heals, injuries and card, at `gameplay/items/medicines/<name>.options`; read back, it is
  the row and nothing else of the template differs. Also the stamina key spelt as the exe reads it,
  as every live item has it (`weapon-balance-1`). `parameters/heal_amount` stays the template's: no
  code or movie reads it.
- New paths: no live config's value changes its type (the release's type check refuses that; the
  stock Medikit's amounts are whole numbers, its 10 % is not). The old `gameplay/items/drugs/`
  bandages and medkit stay in the archive, named by nothing.
- The cards: use, active time and weight, and `medical-cards-1`'s rows (135 arms and legs, 136
  body and head, 137 pain), yellow's also 13 (intoxication, the antitoxin's); no "Heal amount"
  (12).
- Descriptions: the coloured ones one line of what they heal, as the bandages'; the Medikit keeps
  its own.
- Levels: `item_level` 6 for blue and yellow, so Renaissance and Fringe sell them from rank 1
  (srp's rule: the level - 5); green 3, Black Market rank 3. `match_making_level` 3 for the
  medkits, 1 for the bandages, as before: a kit with one is matched as before.
- Sellers: a config's `parameters/faction` (the coloured ones); srp sells a supply whose config
  names a faction from that faction, as gear (`prices.rs`); no stock supply names one. The bandages
  and the Medikit stay the Scavengers' (`EXCEPTIONS`).
- The match changes: the Medikit's pain heal (`simulation_changes.tsv` `next` row). The new items
  change no older game (none has one).

## Intoxication and radiation (PROVEN in the data and the exe's tables)

- The `infection` pool takes both toxic and radiation damage
  (`gameplay/hit_params/human_hit_params.options`: of a player's hit zones only `infection` and the
  hips are not invulnerable to radiation; toxic and radiation hits on the others go to `infection`
  through `bdb_coeff`). Below full it gives `poisoning`; `radiation_sickness` lasts 10 s
  (`affect_duration_sec`).
- The affects (`hit_affects_type_enum`, its names' table in the exe's `.data`): death 0, bleeding
  1, concussion 2, hand_damage 3, leg_damage 4, critical_poisoning 5, poisoning 6,
  radiation_sickness 7, blindness 8 (the medkit's own 3 on the hands and 4 on the legs agree).
- Yellow: `infection` 1 a second (full in 1 s), and `remove_affects` infection 6 and 7. That a
  radiation sickness sits on `infection` is LIKELY (not seen in a game): an in-game test in a
  radiation zone checks it.
- The antitoxin keeps its own place: the light cure (0.1 kg) that also halves toxic damage for
  10 s.

## The coloured medkits

| id | name | colour | from | sold by | icon cell |
|---|---|---|---|---|---|
| 390 | `military_medkit`, Military Medkit | blue | 0.27d0 (id 390) | 3 Renaissance | [16, 3] |
| 1196 | `blackmarket_medkit`, Black Market Medkit | green | 0.46c2 (id 1196) | 2 Black Market | [17, 3] |
| 391 | `science_medkit`, Science Medkit | yellow | 0.27d0 (id 391) | 4 Fringe | [14, 1] |

- 0.26g has no medkit model, in the world or in the hands: a medkit is its config, its dictionary
  row and its lobby icon (`LobbyMenu_AmmoIcons`, the "consumables" atlas; the match HUD has no item
  icons).
- Icons: 0.27d's atlas cells as they are (the same box as 0.26g's orange one); the green from
  0.46c2's single icon, placed in its cell where 0.27d placed the same box. The cells are free in
  the live movie (other packs draw in columns 4 and 5).
- Ids: the donor's own, free in 0.26g and every pack (0.46c2's 759 and 760 for the first two are
  the Rattle pack's).
- Faction: 0.46c2's names (The Renaissance Army / Fringe Settlers / Black Market Medkit).
- Names: 0.27d's ("Military Medkit", "Science Medkit"), the Black Market one and the other
  languages new, each from its table's word for the medkit and the Black Market.
- Not done: 0.46c2's Military perk (-20 % ranged damage for 2 s). A drug can protect
  (`data/damage_protection`, as the antitoxin's `toxic`), but it is not in the owner's table.
