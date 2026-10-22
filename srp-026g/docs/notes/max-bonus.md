# Best bonus in gold

The rules the code follows. The owner, 2026-10-22: "if you get the max value for a bonus, it should
be golden instead of green; so +5% damage is golden; +4% is green". In the same change: the upgrade
and reroll windows show it too, and in the reroll window a bonus shows its range on hover. Later the
same day: "silver received +10% should be golden too (as well as other premium bonuses), since they
are highest level achievable".

Code, in `flash/lobby_menu/src` ([lobby-source.md](lobby-source.md)):
`survarium/item/ModificationRolls` (the table, `style`, `isPremium`, `rangeTip`), the bonus lines of
`ItemFactory`, `InventoryList` and `ItemCard`, `MessageRerollItem` and `ModificatorListRenderer` (the
hover). The check:
`survarium::v026g::modifications::tests::the_lobby_knows_the_values_each_bonus_rolls` and
`the_premium_bonuses_are_the_most_their_property_gives`.

## What "max" is

- Each bonus type rolls five strengths, ids `xxx0`..`xxx4`, drop weights 10, 8, 6.4, 5.12, 4.096
  ([items.md](items.md) §1). **The max is the strongest, `xxx4`.** For example, damage (2000..2004)
  +1..+5 %, so +5 % is gold; armour (1000..1004) +2, +3, +4, +5, +8, so +8 is gold.
- An item's mask picks the types it takes, never the strengths: any item that takes a type rolls
  all five. So the max does not depend on the item, its kind or its level.
- Types that share a property (weight, aim time, reliability, recoil, reload, anomaly armour) roll
  the same five values.
- Only bonuses that roll: drop weight above 0, a type some item takes. The rest stay green:
  pockets, slots, ammo bags, boosters.
- **The premium bonuses are gold too** (owner 2026-10-22): the golden weapons' +10 % silver (1230),
  the premium armour's +5 % experience (1240) and +5 % reputation (1250 Scavengers, 1260 Black
  Market). They never roll ([premium_armour.md](premium_armour.md)), and nothing gives more.
- The value decides, not where the bonus came from: a default modification at the max is gold too.

## Where it shows

- Gold instead of green on every bonus line: item cards and tooltips, the item's list of
  modifications, and the craft and reroll windows (the item's own bonuses, the craft's new one,
  the reroll's three offers), and the shop's entries and bundle contents of the golden and
  premium items (`ItemFactory.startingModifiers`). One function colours them all
  (`ModificationRolls.style`).
- A bonus that is a loss stays red. A defect (the repair window's lines) never goes gold.
- An item tooltip's sum of a property is gold when it reaches the max.
- The compare view shows bars, not bonus text: unchanged.

## The reroll window's range

- Hovering a bonus in the reroll window (the item's bonuses or the three offered) shows its five
  values on one line, weakest first: `Damage: +1%  +2%  [+3%]  +4%  +5%`.
- The others grey, the current value white and in brackets; no gold in the tip (owner
  2026-10-22: a gold max there confused). The brackets mark it without relying on colour; plain
  ASCII, as the lobby's font lacks symbols.
- The craft (upgrade) window's new bonus after a combine shows the same tip on hover.
- No tip for a bonus that does not roll (the premium bonuses: silver, experience, reputation).

## One source of truth

- The movie gets no modification id: the exe hands it `{state, icon, props:[{prop_id,
  prop_value}]}` (0x229A40), and adding a field means an exe patch.
- So the movie keeps a table by `prop_id`: the values each property rolls, weakest first
  (`ModificationRolls.ROLLS`).
- The Rust test builds the same table from the server's game data and fails when they differ,
  printing the table to paste. A content patch that changes the modifications fails it until the
  table follows.
- The premium bonuses are not in `ROLLS` (they do not roll): the movie knows them by
  `ItemFactory.BONUS_UI`, the rows the shop already shows them by, held to the data by
  `golden::tests::the_lobby_shows_the_bonuses_of_the_data`. A bonus with one of those properties at
  that value or more is gold (`ModificationRolls.isPremium`). The second test fails when a
  modification gives one of those properties more, or rolls it.
