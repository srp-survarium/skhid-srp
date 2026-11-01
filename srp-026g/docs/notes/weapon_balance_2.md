# Weapon balance 2: AK-12 and Socom16

The owner's rules and what they change. Data: `gameplay/items/weapons/wpn_ak_12.options`,
`wpn_ak_12_premium.options` and `wpn_socom16.options`; the change ships as a
resources.db patch (rows of `scripts/content-edit/weapon-stats.tsv`, the weapon table).

## AK-12

- **The request (the owner, 2026-10-20):** "0.27a (released 3 months after 0.26a where
  ak12 was introduced) nerfed ak12 damage 55 » 50 and armour penetration 45 » 40.
  I think it should be done here".
- **Decision (the owner, 2026-10-20), shown the AK-12 in each build below:** "50 35
  for ak-12": damage 50 (0.27's), armour piercing 35, between 0.27d0's 40 and
  0.28a2's 30. 0.28a2 rebalanced the whole game; only the AK-12's own change is
  taken. Both rifles.
- **Retuned (the owner, 2026-10-21):** "ak12 to have 40 penet": piercing 40 (0.27d0's).
- **Values:** both rifles `bullet_damage` 0.55 -> 0.5, `bullet_pierce` 0.45 -> 0.4
  (f32 kept); the card's damage (prop 1) 55 -> 50 and armour piercing (prop 3)
  45 -> 40. Unchanged: rate of fire 650, magazine 30, damage distances 20 / 50,
  weight (0.27d0's other changes, distances 30 / 60, wear, repair cost, are not
  taken).
- **Premium gives silver only (the owner, 2026-10-20):** the premium AK-12 is the
  plain one plus +10 % silver (default modification 1230). Its 2044 (reload time
  -15 %) and 2094 (reliability +25 %) defaults go. Kept as the item's own: name,
  description, icon, model (camo), `base_rarity_level` 3, `drop_weight` 0 (never
  dropped). The other premium weapons are not changed here. Superseded by
  [golden_weapons.md](golden_weapons.md) (2026-10-21): every golden weapon, this one
  included, has no default bonus and base rarity 0; the silver bonus is its own
  modification.
- **In the builds at hand** (plain / premium; the card shows them x100):

  | build | damage | piercing |
  |---|---|---|
  | 0.26g stock (no premium AK-12) | 0.55 / - | 0.45 / - |
  | live, revision 25 | 0.55 / 0.55 | 0.45 / 0.45 |
  | 0.27d0 (no 0.27a at hand) | 0.55 / 0.50 | 0.40 / 0.40 |
  | 0.28a2 | 0.50 / 0.45 | 0.30 / 0.30 |
  | 0.28d0 | 0.45 / 0.45 | 0.30 / 0.30 |
  | 0.31a2 | 0.35 / 0.35 | 0.55 / 0.55 |
  | 0.46c2 | 0.26 / 0.26 | none (`material_pierce` 6: another scale) |
  | **here** | 0.50 / 0.50 | 0.40 / 0.40 |

## Socom16

- **Request (the owner, 2026-10-20):** "and also Socom16 75 → 50" (0.28a2's cut).
- **Decision (the owner, 2026-10-20), shown the history below:** "seems like socom is
  nerfed too much, let's make it 60" (0.31a2's value).
- **Retuned (the owner, 2026-10-21):** "socom16 is fine if it has 70 damage": 70.
- **Values:** `bullet_damage` 0.8 -> 0.7 (live is 0.26g's 0.8, not 0.75: 0.27d0 had
  already cut it to 0.75; weapon balance 1 did not touch it); the card's damage
  (prop 1) 80 -> 70. Piercing stays 0.45: 0.28a2 also cut it to 0.35, but the owner
  named the damage only.
- **In the builds at hand:**

  | build | damage | piercing |
  |---|---|---|
  | 0.26g stock, live revision 25 | 0.80 | 0.45 |
  | 0.27d0 | 0.75 | 0.45 |
  | 0.28a2 | 0.50 | 0.35 |
  | 0.28d0 | 0.50 | 0.35 |
  | 0.31a2 | 0.60 | 0.40 |
  | 0.46c2 | 0.65 | none (`material_pierce` 6) |
  | **here** | 0.70 | 0.45 |

## A-545

- **Request (the owner, 2026-10-21):** "check whether devs ever nerfed a-545 TTK"; shown its
  history (0.28a: damage 50 -> 45, piercing 30 -> 25, rate of fire 900 -> 800; 0.29a+ another
  damage system), yes to 0.28a's nerf.
- **Values:** `bullet_damage` 0.5 -> 0.45, `bullet_pierce` 0.3 -> 0.25, `rounds_per_minute`
  900 -> 800; the card's props 1 (damage), 3 (piercing), 4 (rate of fire) to match.
  0.28a2's recoil change (left/right 1.8 -> 2.0) is not taken.
- **TTK** (torso, full-damage range): no armour 0.067 -> 0.150 s (2 -> 3 shots); armour 0.7
  0.200 -> 0.300 s (4 -> 5 shots).

## Who computes it, release

- Damage, piercing and reload time in the client's lockstep simulation (a `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`); srp reads the premium's defaults
  for wear, bonuses and rolls (`item_params`).
- **Release:** the resources.db patch, made by `scripts/content-edit/content_edit.py`
  from the edit list (with the three files from the live server game data) and
  `release/build-patch`; the server game data built again with that patch; the `next`
  row. The damage calculator and the wiki rebuild per release.
