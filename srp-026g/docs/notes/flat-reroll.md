# Flat modification change price (the owner, 2026-10-25)

"Remove modification roll payment progression; it should stay flat." Code:
`survarium::v026g::modifications::Reroll::price`, `session::v026g::items_rules`
(`window_of`, `reroll`).

- **Rule:** every change of a modification costs the item level's price from
  `reroll.options`, the first change's: the 1st, the 5th and the 100th alike.
- **Before:** each change of the item made the next cost 1.1 times more
  (`reroll_modificator_multiplicator`, srp's guess at it). A level 10 item:
  2400, 2640, 2904, ... 3514 the 5th, 5659 the 10th silver; 12, 13, 15, ... 18,
  28 gold.
- **Unchanged:** the price table by item level (level 10: 2400 silver or 12
  gold), silver or gold, no wait (`reroll_time` 0), paid when the three are
  offered, the balance check before it, Cancel keeps the old one and gives
  nothing back.
- **Shown = taken:** the window's two prices (`83 46 06`, `83 46 08`) are the
  server's, from the same function; the lobby movie and the exe only show
  them. No movie, exe or content change.
- **Old items:** an item's change count (`g26_items.reroll`) stays and still
  counts up; it no longer prices anything. No migration: an item changed many
  times pays the flat price from now on.
- **Release:** the server only.
