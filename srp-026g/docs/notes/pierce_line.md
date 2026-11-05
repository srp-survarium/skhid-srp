# Material penetration on the weapon and ammo cards

The owner, 2026-11-05: each weapon card and ammo card gets one row saying what wall its bullet goes
through. Wording as retail Survarium's (0.46c2 to 0.69d0: `st_material_pierce`, "Material
penetration", a plain material name, no number and no bar).

## The owner's rules

- **One row:** `Material penetration: <material>`, a detail row (label left, value right), no bar,
  no number, no explanation; neutral colour (a material is no better or worse than another beside it).
- **The material:** the hardest wall material the bullet goes through keeping at least half its speed
  and penetration (the pierce factor k >= 0.5, below).
- **A weapon:** its compatible rounds all agree: that material; they differ: `Varies`.
- **A round:** every weapon that fires it agrees: that material; else `Varies`.
- **From the game data players have**, never a table in code: a retune, a new gun or a new round
  follows by itself (below).
- English and Ukrainian (every language of the dictionaries has the texts).

## The words

| tier: material (`armor`) | key | English | Ukrainian |
|---|---|---|---|
| `glass` (0: glass, foliage, cloth, water, paper, plastic) | `st_srp_pierce_glass` | Glass, foliage | Скло, листя |
| `metal_car` (0.08: car bodies, gas tanks) | `st_srp_pierce_car` | Car | Автомобіль |
| `wood_thin` (0.10: thin wood, hard glass, rubber) | `st_srp_pierce_plywood` | Plywood | Фанера |
| `metall_thin` (0.12: thin metal, slate) | `st_srp_pierce_tinplate` | Tinplate | Бляха |
| `brick` (0.20: brick, thick metal, trees) | `st_srp_pierce_brick` | Brick, steel | Цегла, сталь |
| `concrete` (0.23) | `st_srp_pierce_concrete` | Concrete | Бетон |
| `stone` (0.7; no bullet reaches it in the current data) | `st_srp_pierce_stone` | Stone | Камінь |
| the rounds (or weapons) differ | `st_srp_pierce_varies` | Varies | Різне |

The row's label: `st_srp_inv_material_pierce`, "Material penetration" / "Пробиття матеріалів". The same
words are the damage calculator's (PR #1334). Ground, gravel, asphalt, tanks (`armor` 2 to 20) stop
every bullet.

## The game's rule (0.26g exe, static reading)

- A bullet's penetration P = the weapon's `parameters/bullet_pierce` x the round's `data/k_arp`
  (`survarium::bullet::bullet` 0x32BA60: bullet +74h = ammo data +24h x weapon +520h; PROVEN).
- Each wall it meets: `survarium::bullet::check_collision` 0x32D4E0 puts the triangle's material in
  bullet +4Ch and tries the pierce first (`try_pierce` 0x32D960), the ricochet (`try_reflect`
  0x32DE40) only when it fails (PROVEN).
- `try_pierce`: A = the material's `physic/armor` (material +4Ch, `game_material::load_from_config`
  0x323A10). t = clamp((P - A) / A, 0, 1), t = 1 when A = 0; k = 1 - (1 - t)^2. k <= 0 stops the
  bullet; else its damage (+70h), P (+74h) and speed are multiplied by k, its direction turned by a
  random angle up to the round's `pierce_dispersion_angle` x the material's `k_pierce_dispersion`.
  No thickness and no angle of incidence: every wall of a material alike (PROVEN).
- So k >= 0.5 needs P >= 1.293 A (t >= 1 - sqrt(0.5)); the content build computes it in single
  precision, as the game.
- After a wall, the bullet's damage and P are also re-derived from its new speed
  (`update_bullet_position` 0x32EE90: min(k D, the distance falloff at k x speed), found for the
  calculator): the damage left can be below k D. The card's rule stays k >= 0.5, "half its speed and
  penetration", not a damage figure.

## How it reaches the card

- `scripts/content-edit/pierce-line-1.json`, the last edit row of `release/content.tsv`: UI property
  174 (not comparable) and, on each round the item dictionary pairs with a weapon
  (`items_compatibility`), a `ui_desc/props_list` line 174 naming the key above for each of those
  weapons: `<weapon dict_id>:<key> ...` (the 5.45x39 7N10: `3:st_srp_pierce_concrete ...
  220:st_srp_pierce_brick ...`, 220 the A-545; at most about 400 bytes, a string the exe hands the
  movie as it is, 0x28709C). `content_edit.py` (`pierce_lines`) computes it from the files as the
  rows above leave them (weapon-stats.tsv, ammo.tsv, packs), the tiers' armour read from
  `game_materials/game.materials` by material name (refused when a material is gone or the tiers'
  armour no longer rises).
- **The weapons get no line:** a weapon config cannot grow. The client rebuilds an upgraded item's
  config in a fixed arena of 565 entries (`tables.rs` `MAX_CONFIG_ENTRIES`, exe 0x358AA0), and the
  Remington 700s, the SV-98, the KS-23M and the Neosteads are within 3 entries of it (a line is 3);
  the first build with weapon lines stopped there (SV-98 566). A round's config is small.
- `release/build-content` (`pierce_check`) computes every line again from the final files and stops
  when one differs, is missing, or is on a round outside the list's files: a row added after it that
  retunes a gun, a round or a material, or adds one, cannot leave a stale card.
- The lobby movie (`survarium.item.PierceLine`) reads the line once into {weapon id: key}. A round's
  card (`AmmoDetails`, last row): the key its weapons agree on, else Varies. A weapon's card
  (`WeaponDetails`, after the bullet speed): the key each of its rounds (`ItemFactory.getList`, the
  dictionary's pairs, as the bullet speed row) gives it, else Varies. A dash beside a compared item
  that has the row; no row when neither has.

## Live data (patch 49, players' files)

84 weapons one material (61 Concrete, 21 Tinplate, 2 Brick), 19 Varies (the shotguns: buckshot glass,
slug car; the 9x19 guns: Para/7N21 tinplate, 7N31 brick; the A-545: 7N10 brick, 7N22
concrete). Rounds that vary: 5.45x39 7N10, .45 ACP, 12mm slug, 12mm buck2.

## Checked

- `scripts/weapon-details/WeaponDetailsHarness.as` (Ruffle, `DETAILS RESULT pass`): the row in every
  language on a weapon and a round, each tier's text, Varies, the dash, no row without the line, no
  bar, the label fitting one line beside its value.
- `content-edit/test_content_edit.py` (each round's line by weapon, no weapon or unpaired round
  touched, tiers out of order refused); `content_edit.py` on players' files, aggregated as the movie
  does, reproduces the counts above.
- Not seen in the game.
