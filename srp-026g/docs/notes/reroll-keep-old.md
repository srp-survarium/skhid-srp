# Keeping the old modification after a reroll

The owner, 2026-10-21: "if you don't like the roll, you should be able to cancel it", for the
modification reroll of the Upgrade window; the price is paid anyway. Code:
`session::v026g::items_rules` (`reroll`, `choose_modifier`), `lobby-server` `items.rs`,
`requests::KEEP_OLD_MODIFICATION`; the lobby: `MessageRerollItem`, `RerollChoice`.

- **How a reroll goes (the game's, unchanged):** the player picks the modification to change and
  pays (silver or gold, `46 07`); the server takes the price and offers three in its place
  (`83 46 07`); the window shows the item's modifications (the old one marked) and the three; the
  player takes one (`46 08`), which replaces the old one (`83 46 08`).
- **New:** while the three are offered, the window's Cancel button takes none of them: the item
  keeps its modification. It is the game's own Cancel, as in every other window (the owner,
  2026-10-23: "replace keep old with generic cancel, like all other options"): the stock
  `cancel` text in every state of the window, no text of srp's.
- **Pending:** nothing of the item changes until the answer. The server holds the offer (the old
  one and the three) with the item, saved with it.
- **Price:** taken once, when the three are offered. Keeping the old one gives nothing back; the
  next one costs the same (flat since 2026-10-25, [flat-reroll.md](flat-reroll.md); no wait
  since 2026-10-22).
- **No second roll while one is pending:** asking again (silver or gold, any modification) shows
  the same three and takes nothing; a craft of the item is refused until the answer.
- **One answer:** a choice or Cancel ends the offer; a second answer is refused. The old and a
  new one can never both be on the item: a choice replaces the old one in its place.
- **The window closed, the game left, a server restart:** the window can not be closed while the
  three are offered (its close button is off, as in the game). If the game goes away anyway, the
  offer is kept and shown again when the window is opened for the item, paid for; until then the
  item has the old one. (Not discarded: the player paid for it, and keeping it gives nothing
  more.)
- **What stays locked:** nothing since 2026-10-22, see below. (The game's rule was one
  modification of an item, any number of times: the first change picked which.)
- **Wire:** Cancel (keep the old one) is `46 08 <item> <old> 3` (the place after the three; the exe sends Flash's
  value as it is, 0x29FF7A). The answer is `83 46 08 <item> <old> <old> <costs> <wait>`: the
  client changes the old one into itself (nothing) and fills the window again. No exe change.
- **Texts (the button):** none of srp's: the game's `cancel`, in its five languages. (Until
  2026-10-23 it read "Keep old" while the three were offered.)
- **Release:** the lobby movie (the content patch) and the server, together: an old movie never
  sends the keep-old answer; an old server refuses it as a choice that is none (the window is filled
  again, the offer still there).

## Any modification may be rerolled

The owner, 2026-10-22: "it should be possible to roll every single slot of equipment and not be
locked into one". Code: `items_rules::reroll`; the lobby: `MessageRerollItem.updateModifiers`.

- **Rule:** any of the item's own modifications may be changed, each any number of times. The
  game's lock (the first change picks the one) is gone, on the server and in the window.
- **Same as before:** the price by item level (the same for every change since 2026-10-25,
  [flat-reroll.md](flat-reroll.md)); paid once, when the three are
  offered; one offer at a time (asking for another modification meanwhile shows the same three,
  free); keep old; no craft while one is offered; refused for a damaged item or one in repair;
  golden weapons' and premium armour's bonuses roll like any other.
- **The one changed last:** still kept (`reroll.modification`) and sent (`83 46 06`/`08`,
  `was_rerolled_before`); the window selects it when it opens, locks nothing and greys out
  nothing.
- **Text:** the window's rule line is srp's own (`st_modifier_reroll_description` says "only one
  modifier"): "You can change any modifier of an item, any number of times. You then choose one
  of three random modifiers.", in the lobby's five languages.
- **Exe:** no lock. `reroll_item_modifier` 0x29FE80 sends `46 07` with `item.mods[index]` of the
  row clicked, no check (0x29FED0..0x29FEFE); the window's data 0x29FC8D..0x29FD1A only sets
  `was_rerolled_before` (the window selects that row) and `reroll_available` (the window does
  not read it). The lock was the window's alone. No exe change.
- **Match:** nothing changes there: the item's modifications work as before.
- **Release:** the server and the lobby movie. With an old movie the lock stays in the window
  only; with an old server the new window's other rows are refused (`84 46`).

## The item's stats on hover

The owner, 2026-10-23: "when in modification, when hovering over the weapon, it should show its
stats". The lobby: `MessageRerollItem.configUI`, `updateModifiers`.

- **Rule:** hovering the item's picture (top left of the window) shows the lobby's item tip
  (`SrvItemHintRednerer`, as on a profile's items): name, stats with its modifications applied,
  the modifications. Weapons and armour alike.
- **After a change:** the tip's modifications are the ones the window was filled with last (on
  opening, after a choice or Cancel), with the item kind's own: the stats follow the new one.
  While the three are offered the item is unchanged, so is the tip.
- **The Upgrade (craft) window:** its slots already show the item's tip, compared with the other
  slot's (craft-menu.md); unchanged.
