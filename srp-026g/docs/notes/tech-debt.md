# Tech debt

Known shortcuts and special cases to revisit; one bullet each, with where it lives and why it was taken.

- **Premium armour's experience and reputation bonuses are granted modifications, not config defaults**
  (2026-10-22): special cases in srp (`like` and `start` in `scripts/content-edit/items.tsv`,
  `golden::starting_modifications`, `golden::check`) and the lobby (`ItemDictId.PREMIUM_ARMOUR*`,
  `ItemFactory.startingModifiers` for shop entries and bundle contents, `ItemFactory.BONUS_UI`), so
  that they can be rerolled and take a place. See [premium_armour.md](premium_armour.md). (The
  golden weapons' silver bonus had the same; they are plain weapons since 2026-11-14,
  [golden_weapons.md](golden_weapons.md).)
- **The lobby keeps its own copy of the values each bonus rolls** (2026-10-22): the movie gets no
  modification id, so `ModificationRolls.ROLLS` (by property) decides gold and the reroll window's
  range. A test (`modifications::tests::the_lobby_knows_the_values_each_bonus_rolls`) fails when it
  differs from the game data and prints the table to paste. See [max-bonus.md](max-bonus.md).
- **The lobby knows the premium bonuses by `ItemFactory.BONUS_UI`** (2026-10-22): they never roll,
  so `ROLLS` leaves them out; `ModificationRolls.isPremium` makes a bonus of their property at their
  value gold. A test (`modifications::tests::the_premium_bonuses_are_the_most_their_property_gives`)
  fails when a modification gives that property more or rolls it. See [max-bonus.md](max-bonus.md).
