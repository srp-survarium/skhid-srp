# Tech debt

Known shortcuts and special cases to revisit; one bullet each, with where it lives and why it was taken.

- **Golden weapons' silver bonus is a granted modification, not a config default** (2026-10-22):
  special cases in srp (`golden::starting_modifications`, `give_golden_silver`, `golden::check`) and
  the lobby (`ItemFactory.startingModifiers` for shop entries and bundle contents), so that it can be
  rerolled and takes a place. See "Tech debt" in [golden_weapons.md](golden_weapons.md).
