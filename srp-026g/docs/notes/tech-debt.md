# Tech debt

Known shortcuts and special cases to revisit; one bullet each, with where it lives and why it was taken.

- **Golden weapons' silver bonus is a granted modification, not a config default** (2026-10-22):
  special cases in srp (`golden::starting_modifications`, `give_golden_silver`, `golden::check`) and
  the lobby (`ItemFactory.startingModifiers` for shop entries and bundle contents), so that it can be
  rerolled and takes a place. See "Tech debt" in [golden_weapons.md](golden_weapons.md).
- **The lobby keeps its own copy of the values each bonus rolls** (2026-10-22): the movie gets no
  modification id, so `ModificationRolls.ROLLS` (by property) decides gold and the reroll window's
  range. A test (`modifications::tests::the_lobby_knows_the_values_each_bonus_rolls`) fails when it
  differs from the game data and prints the table to paste. See [max-bonus.md](max-bonus.md).
