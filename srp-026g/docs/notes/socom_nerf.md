# SOCOM 16 nerf (the owner, 2026-10-25)

"Nerf SOCOM: 10 damage, 5 penetration" and "move SOCOM to lvl 15". The SOCOM 16
(`wpn_socom16`, dict id 366) is the only SOCOM in the game data. The edit list is
`scripts/content-edit/weapon-stats.tsv` (its rows, the weapon table).

## The rule

- Damage 70 -> 60, armour piercing 45 -> 40 (10 and 5 off, in the lobby card's units).
- Equipment level 12 -> 15.
- Nothing else changes: falloff, rate of fire, magazine, recoil, price, no shop.

## Values

| | config | card | before | after |
|---|---|---|---|---|
| damage | `parameters/bullet_damage` | prop 1 | 0.7 (70) | 0.6 (60) |
| armour piercing | `parameters/bullet_pierce` | prop 3 | 0.45 (45) | 0.4 (40) |
| equipment level | `parameters/item_level` | "Equipment level" | 12 | 15 |
| matchmaking level | `parameters/match_making_level` | - | 12 | 15 |

- The card shows `x100` of `bullet_damage` / `bullet_pierce` from its own texts (`ui_desc/props_list`),
  so both texts change with the values.
- Distance falloff stays: full damage to 30 m, down to `min_damage_amount` 0.6 of it at 100 m
  (60 -> 36). The ammo's factors stay (7.62x51 M59: piercing x1.2, so 48 at the muzzle).
- `match_making_level` equals `item_level` in every 0.26g item, so both move to 15.
- The match simulation changes (`simulation_changes.tsv` `next` row).

## Level 15: what it changes

- 0.26g has no player-level gate. The shop gates by the seller's reputation rank in srp's price
  list (`prices.rs`), never by the item's config. The SOCOM has no faction and is not sold
  (drop only, as in 0.26a), so no buy check changes.
- The card's "Equipment level" reads `item_level` (`ItemFactory`: `description.level`).
- Drops: it now drops as a level-15 item (with the AK-12, ASh-12, VSSK, PKP), not at 12.
- Matchmaking: a kit carrying it is a level-15 kit.
- Craft and reroll prices go by item level, so they are level 15's.
- srp reads all of these from the server game data built with the patch, so client and server agree.
  Owned SOCOMs stay and take the new values.

## The SVD at level 15 (the owner, 2026-10-25)

"also move the SVD to level 15". The SVD (`wpn_svd`, Renaissance) is the only SVD; it has no
premium copy. Its rows are in `scripts/content-edit/weapon-stats.tsv`.

- `item_level` and `match_making_level` 12 -> 15. Damage, piercing and the rest stay.
- Shop: srp sells it at Renaissance rank `item_level` - 5, so rank 7 -> 10 (the last). The
  price list is built from the server game data, so the lobby's lock and the buy check agree.
- Drops as a level-15 item; a kit carrying it is a level-15 kit.
- Craft 14,000 -> 20,000 silver (70 -> 100 gold); a modification change 3,200 -> 5,000 silver
  (16 -> 25 gold).
- Owned SVDs stay. Not a simulation change: the level only picks who meets whom and what it costs.
