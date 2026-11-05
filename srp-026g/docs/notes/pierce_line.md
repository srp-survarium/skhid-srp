# Pierces: what wall a weapon's bullet gets through, and its rounds on the card

The owner, 2026-11-05 (PR #1340, decided after testing it in the game): the weapon card says the
hardest wall material its bullet gets through and the damage it keeps there; the stats a round
changes show the weapon's best round, and Shift lists each round.

## The owner's rules

- **The rule is the game's:** a bullet gets through a material when its penetration P is above the
  material's armour A (`try_pierce` stops it at P <= A; A = 0 lets every bullet through). No other
  threshold. The VSSK with 12.7x54 VPS (P 0.84) gets through stone.
- **The value:** the material and the damage kept after it, `Concrete · 52%`: k = 1 - (1 - clamp((P -
  A) / A, 0, 1))^2 against that material, 100 % at A = 0, rounded to a whole percent. Computed per
  weapon and round at the content build (P = bullet_pierce x k_arp, single precision).
- **Weapon card only.** The ammo card keeps its own armour piercing stat, no Pierces row.
- **The card (normal view):** one row, `Pierces: <material> · <percent>`, of the weapon's best round:
  the harder material, then the higher percent. Compared with the other card's best, in the card's
  better / worse colours (harder wins, same material: higher percent wins, equal neutral); a weapon
  without it shows a dash, neutral. Ctrl neutral, as every row.
- **The other stats a round changes** show the weapon's best round too, each its own best (the rounds
  can differ per stat), compared best against best as today: Damage (the bar: the weapon's damage x
  the highest k_damage x pellets of its rounds), Armor penetration (the bar: x the highest k_arp),
  Bullet speed (the fastest round).
- **Held Shift:** under Bullet speed and under Pierces, one row per round the weapon fires,
  `<round name>  <value>`, neutral; the shorter list of two compared cards padded with dashes so their
  rows stay aligned. Damage and Armor penetration list no rounds (below).
- English and Ukrainian, and every other dictionary.

## The words

| tier: material (`armor`) | key | English | Ukrainian |
|---|---|---|---|
| `glass` (0: glass, foliage, cloth, water, paper, plastic) | `st_srp_pierce_glass` | Glass | Скло |
| `metal_car` (0.08: car bodies, gas tanks) | `st_srp_pierce_car` | Car | Автомобіль |
| `wood_thin` (0.10: thin wood, hard glass, rubber) | `st_srp_pierce_plywood` | Plywood | Фанера |
| `metall_thin` (0.12: thin metal, slate) | `st_srp_pierce_tinplate` | Tinplate | Бляха |
| `brick` (0.20: brick, thick metal, trees) | `st_srp_pierce_brick` | Brick, steel | Цегла, сталь |
| `concrete` (0.23) | `st_srp_pierce_concrete` | Concrete | Бетон |
| `stone` (0.7) | `st_srp_pierce_stone` | Stone | Камінь |

The row's label `st_srp_inv_material_pierce`: "Pierces" / "Пробиває" (retail's "Material penetration"
with `Concrete · 52%` beside it does not fit the card's line). The value `st_srp_pierce_value`:
`%1$s · %2$s` (the material, `st_srp_unit_percent`). The material words are the damage calculator's
(PR #1334). Ground, gravel, asphalt, tanks (`armor` 2 to 20) stop every bullet.

## The game's rule (0.26g exe, static reading)

- P = the weapon's `parameters/bullet_pierce` x the round's `data/k_arp` (`survarium::bullet::bullet`
  0x32BA60: bullet +74h = ammo data +24h x weapon +520h); its damage +70h = bullet_damage x k_damage
  (0x2E1CF0). PROVEN.
- Each wall: `check_collision` 0x32D4E0 puts the triangle's material in bullet +4Ch and tries the
  pierce first (`try_pierce` 0x32D960), the ricochet (`try_reflect` 0x32DE40) only when it fails.
- `try_pierce`: A = the material's `physic/armor` (material +4Ch, `game_material::load_from_config`
  0x323A10). t = clamp((P - A) / A, 0, 1), 1 when A = 0; k = 1 - (1 - t)^2. k <= 0 (P <= A) stops the
  bullet; else its damage, P and speed are multiplied by k, its direction turned by a random angle up
  to the round's `pierce_dispersion_angle` x the material's `k_pierce_dispersion`. No thickness, no
  angle of incidence. PROVEN.
- After a wall the bullet's damage and P are also re-derived from its new speed
  (`update_bullet_position` 0x32EE90, found for the calculator): the damage left can be below k x D.
  The card's percent is k, the try_pierce factor, not that figure.

## How it reaches the card

- `scripts/content-edit/pierce-line-1.json`, the last edit row of `release/content.tsv`: UI property
  174 (not comparable: no bar) and, on each round the item dictionary pairs with a weapon
  (`items_compatibility`), a `ui_desc/props_list` line 174 `<weapon dict_id>:<rank>:<key>:<percent>
  ...`, one entry per weapon (the 12.7x54 VPS: `... <VSSK's id>:6:st_srp_pierce_stone:36 ...`; the
  rank the tier's place in the list, 0 glass .. 6 stone, so the order is the data's; at most 505
  bytes, a string the exe hands the movie as it is, 0x28709C). `content_edit.py` (`pierce_lines`)
  computes it from the files as the rows above leave them (weapon-stats.tsv, ammo.tsv, packs), the
  tiers' armour read from `game_materials/game.materials` by material name (refused when a material
  is gone or the tiers' armour no longer rises).
- **The weapons get no line:** a weapon config cannot grow. The client rebuilds an upgraded item's
  config in a fixed arena of 565 entries (`tables.rs` `MAX_CONFIG_ENTRIES`, exe 0x358AA0), and the
  Remington 700s, the SV-98, the KS-23M and the Neosteads are within 3 entries of it (a line is 3);
  the first build with weapon lines stopped there (SV-98 566).
- `release/build-content` (`pierce_check`) computes every line again from the final files and stops
  when one differs, is missing, or is on a round outside the list's files: a row added after it that
  retunes a gun, a round or a material, or adds one, cannot leave a stale card.
- The lobby movie: `WeaponDetails` reads the weapon's rounds once (`ItemFactory.getList`, the
  dictionary's pairs; each round's descriptor lines 21, 22, 163 and its 174 entry for this weapon,
  `PierceLine.forWeapon`). Bullet speed and Pierces are its detail rows (best round, compared by
  value, `PierceLine.score` = rank x 1000 + percent); held Shift each round's under them.
  `ItemCompare` gives the native Damage and Armor penetration bars the weapon's best round
  (`WeaponDetails.bestRoundFactor`), whole numbers as the card shows them (`Util.getNumberString`
  cuts a fraction's float tail: 34 x 1.2 read 40.79).

## Shift: rows under each stat, not one table

A 4-column "by ammo" table (Damage | Pen | Speed | Material) does not fit: a detail row is one native
label and one right-aligned value field (about 250 px unscaled; Scaleform resolves the game's font
only on the movie's own fields), and a material alone ("Ziegel, Stahl · 100%", "Rétegelt lemez ·
19%") takes most of the value column. Per-round blocks of all four stats (a caption and four rows a
round) made the AK-74M's Shift card 1369 px unscaled (main: 890), past the 1200 the harness keeps it
in (900 at the card's 0.75); four sub-row lists, about 1250. Rows under Bullet speed and Pierces only:
1026. Damage and Armor penetration are native bars at the card's top, with no room under them for
rows: Shift shows their best round only.

## Rows that change with the round, unchanged (the owner's call)

A round also scales, by its own `data` keys (ammo_data::load 0x332440): the full-damage and
minimum-damage distances (`k_max_damage_distance`, `k_min_damage_distance`: Full damage up to,
Minimum damage from, the Distance bar), the minimum damage (`k_min_damage_amount`), stopping power
(`k_power`), stamina damage (`k_stamina_damage`), and the spread (`dispersion`; pellets'
`buck_dispersion`). Their rows still show the weapon's own value.

## Live data (patch 49, players' files)

Best round per weapon: 82 Concrete, 11 Plywood (the shotguns' slugs), 7 Brick, 2 Stone (the VSSK and
its camo copy, 12.7x54 VPS: Stone · 36%), 1 Tinplate. The 12.7x54 PT on the VSSK (P 0.7 = stone's
0.7) stops at stone: Concrete · 100%.

## Checked

- `scripts/weapon-details/WeaponDetailsHarness.as` (Ruffle, `DETAILS RESULT pass`), in every
  language: the best round per stat from different rounds (speed 7N10, material 7N22); each round's
  rows held Shift, neutral, padded beside a weapon of one round, aligned, fitting their line, the card
  under 900; the bars with the best round (armour piercing 35 x 1.2 = 42, damage 34 x 1.2 = 41); a
  harder material over more damage kept (VPS Stone · 36% over PT Concrete · 100%), the same
  material by percent, equal and Ctrl neutral, a missing side a dash, no row without the data, no
  row on the ammo card, no bar.
- `content-edit/test_content_edit.py`: the lines by weapon, P = A not through (0.2 x 1.0 at brick's
  0.2), the VSSK's VPS through stone at 36 % and PT stopped at stone, no weapon or unpaired round
  touched, tiers out of order refused.
- Not seen in the game.
