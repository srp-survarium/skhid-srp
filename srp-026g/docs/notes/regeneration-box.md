# The Regeneration box

The owner (2026-11-10): "remove autobuy and put regeneration (similar to speed and weight) as
the next column"; then "it should just be regen and then stamina regen".

## What it shows

- **Where:** the character panel's bottom row, left of Speed and Weight, where the Autobuy box
  was (premium.md §3): `Regeneration | Speed | Weight`. The same kind of box as Speed's
  (`KitStatIndicator`: the weight's frame and a copy of the weight's own symbol).
- **The box's value:** health regeneration with the selected kit and the account's skills, as a
  share of the game's base rate: `100%` with nothing, `115%` with one +15 % bonus. Green above
  100 %, coral below (Speed's colours).
- **The tip, two sections** (the Speed and Weight tips' card and rows):
  1. "Health regeneration: N%", then one row for each thing that changes it;
  2. "Stamina regeneration: N%", then its rows.
  A row is an item by its name (armour head to boots, then the pockets), then the skills; none at
  0; green faster, red slower. A section with no rows shows its heading alone (100%).
- **Not shown:** seconds or health a second (the base rates are not given to the lobby: below);
  the regeneration's delay and limit (the Regeneration booster's -2 s and +100 %, Metabolism's
  -0.5 s a rank: regeneration-booster.md, medical-skills.md); artefacts (Larkspur, Spring: found in
  a match, not part of a kit). Sprint stamina use stays in the Speed tip: another number (what a
  sprint takes, modifiers 19 and 67), not this one (what comes back, modifier 3).

## The rule

Both rates are `base x max(0, 1 + S)`, S the sum of one player modifier over the kit
(`apply_modifier` 0x2D5AE0 with weight 1; neither index is in its additive list 0x927E08):

| | modifier | base | where the match computes it | confidence |
|---|---|---|---|---|
| health | 2 `health_regeneration_speed` | each pool's `regeneration_speed` (`human_hit_params.options`: body, head, arms, legs 0.03/s, pain 0.1/s) | `body_part::regenerate` 0x32A4B0 (re-kit; Unicorn run of 2026-10-23, items.md §1) | PROVEN |
| stamina | 3 `stamina_regeneration_speed` | the stamina's regeneration speed (`player_stamina` +8) | `player_stamina::regenerate` 0x3298C0: `mov esi,3`, `call 0x2D5AE0`, then `stamina += ms x 0.001 x rate`, held to 0..maximum (read 2026-11-10, re-kit `abi_inputs.tsv`) | PROVEN |

The one percentage holds for every health pool (each pool's own base times the same factor), so
the box needs no base rate.

What S sums, and where the lobby reads each part:

| part | in the match | in the lobby | confidence |
|---|---|---|---|
| the skills | the kit's own float of the index (srp writes the skills' sums there) | `root.set_kit_modifiers`' 68 floats (`KitModifiers.at(2)`, `at(3)`), sent before each weight callback (`weight_ui_026g.c`) | PROVEN that the getter starts from the kit's float (0x2D5AE6); today no skill adds to 2, and Breath (204, +4 % a rank) is the one skill of 3 |
| armour (head to boots, the backpack) | each worn piece's modifications of the parameter become nodes of the kit's list (`armor::armor` 0x33EAA0, `add_modifiers` 0x33F3A0) | the item's modifications as the cards show them: property 103 (health) and 104 (stamina), built-in, rolled and defects (`LocalPlayerProfile.kitItems`) | PROVEN for a vest, trousers and a backpack (items.md §1); LIKELY the same for the other armour slots (the same class) |
| a booster in a pocket | a kit modifier of each parameter that is not 0, while carried (`survarium::booster` 0x33F690 / 0x33F990) | the same properties of the pocket's item | PROVEN (regeneration-booster.md, one-pocket-a-type.md) |

- **Numbers come from the game data only:** every value is a modification's `prop_value` or a
  kit float. ActionScript holds the two property ids (103, 104), the two modifier indices
  (`PlayerModifier`), the two icons of the modifier atlas (9 and 10, the modifications' own
  `ui_desc/icon`) and Breath's skill id for its row's name. A retune of a bonus or of the skill
  needs no change of the movie.
- **Seconds are not shown** because the lobby is not given `human_hit_params.options` or the
  stamina's base rate; the percentage is exact without them.
- A weapon's modifications are not read: no weapon has either parameter.

## Code

- `survarium/player`: `RegenerationParts` (one rate's sources and total), `RegenerationExplanation`
  (the box's word, value and tip: `IKitStat`), `KitStatIndicator` (the box; Speed's and this one's,
  was `SpeedIndicator`), `WeightIndicator` (owns the row's boxes), `KitItems` (`regeneration`,
  `staminaRegeneration`), `LocalPlayerProfile.showWeight` / `kitItems`.
- Without the kit's modifiers from the exe (`set_kit_modifiers` not yet called) the box is hidden,
  as Speed's is without its numbers.
- Texts: `st_srp_regeneration_label` (the box; each language's stock word of the bonus,
  `st_health_regen_correction_perc_name`), `st_srp_regeneration_health`,
  `st_srp_regeneration_stamina` (the stock `st_stamina_regen_correction_perc_name`).
- UI only: nothing a match computes changes; no `simulation_changes.tsv` row.

## Checks

- `scripts/weight-ui/check_ui.sh` (CPU Ruffle): the sums, the rows and their colours and icons,
  the box's value and style; in the inventory's profile, in all ten languages, the three boxes
  side by side inside the panel and apart from the boots slot, each word apart from its value,
  no Autobuy box, no box without the kit's modifiers.
- Not played.
