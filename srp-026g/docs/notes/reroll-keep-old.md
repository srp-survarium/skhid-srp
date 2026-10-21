# Keeping the old modification after a reroll

The owner, 2026-10-21: "if you don't like the roll, you should be able to cancel it", for the
modification reroll of the Upgrade window; the price is paid anyway. Code:
`session::v026g::items_rules` (`reroll`, `choose_modifier`), `lobby-server` `items.rs`,
`requests::KEEP_OLD_MODIFICATION`; the lobby: `MessageRerollItem`, `RerollChoice`.

- **How a reroll goes (the game's, unchanged):** the player picks the modification to change and
  pays (silver or gold, `46 07`); the server takes the price and offers three in its place
  (`83 46 07`); the window shows the item's modifications (the old one marked) and the three; the
  player takes one (`46 08`), which replaces the old one (`83 46 08`).
- **New:** while the three are offered, the window's Cancel button reads "Keep old" and takes none
  of them: the item keeps its modification.
- **Pending:** nothing of the item changes until the answer. The server holds the offer (the old
  one and the three) with the item, saved with it.
- **Price:** taken once, when the three are offered. Keeping the old one gives nothing back; the
  change counts like any other: the next one costs 1.1 times more, silver waits six hours.
- **No second roll while one is pending:** asking again (silver or gold, any modification) shows
  the same three and takes nothing; a craft of the item is refused until the answer.
- **One answer:** a choice or "keep old" ends the offer; a second answer is refused. The old and a
  new one can never both be on the item: a choice replaces the old one in its place.
- **The window closed, the game left, a server restart:** the window can not be closed while the
  three are offered (its close button is off, as in the game). If the game goes away anyway, the
  offer is kept and shown again when the window is opened for the item, paid for; until then the
  item has the old one. (Not discarded: the player paid for it, and keeping it gives nothing
  more.)
- **What stays locked:** the modification changed is the item's only one that may be changed
  again, kept or not (the game's rule: one modification of an item, any number of times).
- **Wire:** "keep old" is `46 08 <item> <old> 3` (the place after the three; the exe sends Flash's
  value as it is, 0x29FF7A). The answer is `83 46 08 <item> <old> <old> <costs> <wait>`: the
  client changes the old one into itself (nothing) and fills the window again. No exe change.
- **Texts (the button):** Keep old, Не міняти, Не менять, Behalten, Zostaw stary.
- **Release:** the lobby movie (the content patch) and the server, together: an old movie never
  sends "keep old"; an old server refuses it as a choice that is none (the window is filled
  again, the offer still there).
