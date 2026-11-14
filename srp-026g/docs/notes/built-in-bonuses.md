# Built-in bonuses

An item's built-in bonuses are its `default_modifications`: the client adds them from the
config, the server counts them as held (a piece never rolls a kind it comes with,
`Modifications::candidates`).

The owner's rule (2026-10-29; the bonus cells of content-edit `armor.tsv`, once `built-in-bonuses-1.json`):

- Equipment only (items_dict categories 1-7: helmet, mask, body armour, back, pants, gloves,
  boots; premium copies too). Weapons keep their presets.
- Each built-in bonus of a kind that rolls (a row of it has a drop weight and changes more than
  wear) is raised to that kind's second-strongest strength (the rows by drop weight, commonest
  first): e.g. movement +4 %, energy costs -8 %, regeneration +12 %, weight limit +4 kg, armour +5.
  It only raises: a stronger one keeps its value (Pilgrim's regeneration +15 %, Nomad's movement
  +5 % and aiming -10 %).
- Why: a built-in +1 % lost to any roll's +5 %, and a kind the piece comes with cannot roll, so
  nobody wore those pieces.
- Kinds that never roll (item slots, ammo bags, artefact container, pocket size and capacity,
  boosters) and wear-only ones (reliability) stay.
- Held in `scripts/content-edit/armor.tsv` (a bonus cell is the modification's id); the content-edit
  test `test_each_built_in_bonus_that_rolls_is_one_of_its_kinds_two_strongest` checks every cell
  against `modifications.tsv`, so a new item's cell must follow it.
- The exoskeleton suits come after it (docs/notes/exo-suits.md): the Duty suit gets the weight
  limit +5 kg and regeneration +15 %, the kinds' top rows.
- Owned pieces get it at once (defaults come from the config).

## Their order

The card lists an item's built-in bonuses as its `default_modifications` has them, so every armour
piece and booster keeps them in one order (2026-11-14, the owner: "it should be sorted everywhere
properly", ammo bags on top): what it carries (ammo bags, item slots, pocket capacity, artefact
container), then the body (weight limit, movement, energy costs, stamina and health regeneration),
the gun (accuracy, aim, reload, recoil), the rest. It is the order of the bonus columns of
`STATS['armor']` and `STATS['boosters']` in `content_edit.py`: the table edit sorts each item's list
(kinds without a column after them, as they were) and `--check` reports a list out of it.
