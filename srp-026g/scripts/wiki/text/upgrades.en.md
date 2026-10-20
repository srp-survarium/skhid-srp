# Upgrades, rerolls, repairs
section: progress
summary: More modifications, other modifications, fixing wear.

See [Modifications](modifications) for every bonus and its chances; each item page lists what it can roll.

## Upgrade

- Put a second copy of the item in as the donor. The donor is used up either way.
- Outcome: one modification more, one less, or nothing. At most {{mods.max}}.
- The chances depend on how many more modifications the item has than the donor:

{{table:upgrade_chances}}

- A damaged item can't be upgraded.
- After an upgrade: **Continue** upgrades the same item again, **Exit** closes the window.

## Change a modification

- Pick one modification of the item; it is the only one you can change on it, as often as you like.
- You are offered {{mods.reroll_choices}} to choose from: of the kinds the item's other modifications leave free.
- Paying in silver: once in {{mods.reroll_wait}}.
- Each change costs {{mods.reroll_growth}} the last.

{{table:upgrade_prices}}

## Repair

- Items wear with use, matches and deaths.
- A full repair costs the item's repair share of its price, times the wear: each item page shows it from 0.
- Defects come as an item wears: [Modifications](modifications). Each is fixed on its own.
