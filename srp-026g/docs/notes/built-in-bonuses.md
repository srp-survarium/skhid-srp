# Built-in bonuses

An item's built-in bonuses are its `default_modifications`: the client adds them from the
config, the server counts them as held (a piece never rolls a kind it comes with,
`Modifications::candidates`).

The owner's rule (2026-10-29, content-edit `built-in-bonuses-1.json`):

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
- Computed from modifications.options and the item configs at build time (`content_edit.py`
  `promote_rolled_defaults`), so new items follow it.
- The exoskeleton suits come after it (docs/notes/exo-suits.md): the Duty suit gets the weight
  limit +5 kg and regeneration +15 %, the kinds' top rows.
- Owned pieces get it at once (defaults come from the config).
