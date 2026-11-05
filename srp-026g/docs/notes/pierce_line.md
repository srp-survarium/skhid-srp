# Pierces: what wall a weapon's bullet gets through, and its best round on the card

The owner, 2026-11-05 (PR #1340, decided after testing it in the game): the weapon card says the
hardest wall material its bullet gets through and the damage it keeps there; the stats a round
changes show the weapon's best round ("99% of the time people use the best ammo"), a shotgun's by
kind, Buckshot and Slug (its players switch between them).

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
- **No round is listed:** held Shift the card shows main's rows plus Bullet speed and Pierces in
  their places, with the same best values.
- **A shotgun of both kinds** (from the data: among its rounds one of more than one pellet,
  `data/buck_shot`, the round's card line 165, and one of a single projectile): Damage, Armor
  penetration, Bullet speed and Pierces each its label alone over two rows, `• Buckshot <the best of
  its pellet rounds>` and `• Slug <the best of the others>`, the words alone (no calibre or shot
  size), bulleted and stepped in 16 px under the stat, at 0.8 of a row; the same held Shift. Its
  native Damage and Armor penetration bars keep the weapon's own number. A kind compares with the
  other shotgun's same kind; a shotgun beside any other weapon gets no colour on these four stats
  (bars included), and no rows are padded. A shotgun of one kind (the KS-23M: buckshot only) is any
  weapon: its best round.
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
  dictionary's pairs; each round's descriptor lines 21, 22, 163, 165 and its 174 entry for this
  weapon, `PierceLine.forWeapon`); `roundStats` says what each of the four stats is for a round
  (damage and armour piercing the weapon's card number x the round's, whole). Bullet speed and
  Pierces are its detail rows, after Minimum damage: the best round's, compared by value
  (`PierceLine.score` = rank x 1000 + percent). A shotgun of both kinds (`kindsOf`) has the four as
  a value-less row over a Buckshot and a Slug `child` row each (ids `<stat>/buckshot`, `/slug`: the
  other card's same kind compares, nothing else), which `ItemDetailsView` steps in by
  `SUBROW_INDENT` 16 after the row's first layout (configUI puts the label at the native column)
  and gives `SUBROW_STRIDE` 0.8 of a row. `ItemCompare` gives the native Damage and Armor
  penetration bars the weapon's best round (`WeaponDetails.bestRoundFactor`; 1 for such a
  shotgun), whole numbers as the card shows them (`Util.getNumberString` cuts a fraction's float
  tail: 34 x 1.2 read 40.79), and no colour when either card is such a shotgun
  (`WeaponDetails.splitByKind`).
- Pellets come from the round's card line 165, which ammo-cards-1.json gives the three smallshot
  rounds (12mm buck, buck2, 23 mm) and ammo.tsv writes from `data/buck_shot`: a pellet round added
  later needs its line there, as that list says, or it counts as a slug.
- The bullet, `•`, is in every full Calibri of the lobby movie (as `·`, `–`, `…`; not `★`, `▸`).
- Kept out (the owner, after seeing a per-round listing in the game, "messy"): rows of each round;
  a 4-column table would not fit the card's line in any case.

## Rows that change with the round, unchanged (the owner's call)

A round also scales, by its own `data` keys (ammo_data::load 0x332440): the full-damage and
minimum-damage distances (`k_max_damage_distance`, `k_min_damage_distance`: Full damage up to,
Minimum damage from, the Distance bar), the minimum damage (`k_min_damage_amount`), stopping power
(`k_power`), stamina damage (`k_stamina_damage`), and the spread (`dispersion`; pellets'
`buck_dispersion`). Their rows still show the weapon's own value.

## Live data (patch 49, players' files)

Best round per weapon (a shotgun of both kinds shows it by kind): 82 Concrete, 11 Plywood (the
shotguns' slugs), 7 Brick, 2 Stone (the VSSK and
its camo copy, 12.7x54 VPS: Stone · 36%), 1 Tinplate. The 12.7x54 PT on the VSSK (P 0.7 = stone's
0.7) stops at stone: Concrete · 100%. By kind, from the data: 16 shotguns (Benelli M4, IZh-27 and
its two premium copies, Jackhammer, MP-153, the two Neosteads, Remington 870, Saiga-12 and its
premium, SPAS-12, TOZ-34, TOZ-66, Vepr-12 and its premium); the KS-23M fires buckshot only.

## Checked

- `scripts/weapon-details/WeaponDetailsHarness.as` (Ruffle, `DETAILS RESULT pass`), in every
  language: any weapon but a shotgun of both kinds, normal and held Shift, each stat its best
  round's from different rounds (speed 7N10, material 7N22) and no round's own row; a shotgun of
  buckshot and slugs, each stat a value-less row over its best `• Buckshot` and `• Slug`, stepped in
  (its label column checked at +16, its spacing at 0.8 of a row), fitting their line; in English
  the bars with the best round (armour piercing 35 x 1.2 = 42, damage 34 x 1.2 = 41) and a
  shotgun's own; kind against kind between two shotguns (more damage of buck2, more of the slug's
  damage kept), a shotgun beside another weapon neutral on the four stats and the bars, nothing
  padded; a shotgun of one kind its best; a harder material over more damage kept (VPS Stone · 36%
  over PT Concrete · 100%), the same material by percent, equal and Ctrl neutral, a missing side a
  dash, no row without the data, no row on the ammo card, no bar.
- `content-edit/test_content_edit.py`: the lines by weapon, P = A not through (0.2 x 1.0 at brick's
  0.2), the VSSK's VPS through stone at 36 % and PT stopped at stone, no weapon or unpaired round
  touched, tiers out of order refused.
- Not seen in the game.
