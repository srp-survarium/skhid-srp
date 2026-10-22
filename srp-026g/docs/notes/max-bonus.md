# Best bonus in gold

The rules the code follows. The owner, 2026-10-22: "if you get the max value for a bonus, it should
be golden instead of green; so +5% damage is golden; +4% is green". In the same change: the upgrade
and reroll windows show it too, and in the reroll window a bonus shows its range on hover.

Code, in `flash/lobby_menu/src` ([lobby-source.md](lobby-source.md)): `survarium/item/ModificationRolls`
(the table, `style`, `rangeTip`), the bonus lines of `ItemFactory`, `InventoryList` and `ItemCard`,
`MessageRerollItem` and `ModificatorListRenderer` (the hover). The check:
`survarium::v026g::modifications::tests::the_lobby_knows_the_values_each_bonus_rolls`.

## What "max" is

- Each bonus type rolls five strengths, ids `xxx0`..`xxx4`, drop weights 10, 8, 6.4, 5.12, 4.096
  ([items.md](items.md) §1). **The max is the strongest, `xxx4`.** For example, damage (2000..2004)
  +1..+5 %, so +5 % is gold; armour (1000..1004) +2, +3, +4, +5, +8, so +8 is gold.
- An item's mask picks the types it takes, never the strengths: any item that takes a type rolls
  all five. So the max does not depend on the item, its kind or its level.
- Types that share a property (weight, aim time, reliability, recoil, reload, anomaly armour) roll
  the same five values.
- Only bonuses that roll: drop weight above 0, a type some item takes. The rest stay green: pockets,
  slots, ammo bags, boosters, and the premium items' silver, experience and reputation bonuses. The
  golden weapons' +10 % silver (1230) never rolls, so it stays green.
- The value decides, not where the bonus came from: a default modification at the max is gold too.

## Where it shows

- Gold instead of green on every bonus line: item cards and tooltips, the item's list of
  modifications, and the craft and reroll windows (the item's own bonuses, the craft's new one,
  the reroll's three offers). One function colours them all (`ModificationRolls.style`).
- A bonus that is a loss stays red. A defect (the repair window's lines) never goes gold.
- An item tooltip's sum of a property is gold when it reaches the max.
- The compare view shows bars, not bonus text: unchanged.

## The reroll window's range

- Hovering a bonus in the reroll window (the item's bonuses or the three offered) shows its five
  values on one line, weakest first: `Damage: +1%  +2%  [+3%]  +4%  +5%`.
- The others grey, the current value white and in brackets; no gold in the tip (owner
  2026-10-22: a gold max there confused). The brackets mark it without relying on colour; plain
  ASCII, as the lobby's font lacks symbols.
- No tip for a bonus that does not roll (the golden weapons' silver bonus).

## One source of truth

- The movie gets no modification id: the exe hands it `{state, icon, props:[{prop_id,
  prop_value}]}` (0x229A40), and adding a field means an exe patch.
- So the movie keeps a table by `prop_id`: the values each property rolls, weakest first
  (`ModificationRolls.ROLLS`).
- The Rust test builds the same table from the server's game data and fails when they differ,
  printing the table to paste. A content patch that changes the modifications fails it until the
  table follows.
