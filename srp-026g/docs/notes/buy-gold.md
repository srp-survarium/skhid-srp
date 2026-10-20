# "Get Gold" opens the exchange

The rules the code and the tests follow. Code: `lobby/src/SrpBuyGold.as` and the lobby
feature `buy-gold` (`lobby/features/buy-gold/`: each `goToStore`, and the premium window's
gold button, call `SrpBuyGold.open()`; [lobby-source.md](lobby-source.md)); the test:
`client-patches/test_build_lobby.py`. The owner's rule: 2026-10-20 ("it leads to a dead official
website, instead it should just open gold trading menu").

- **What:** every way the lobby had to the online store (`go_to_store`, a browser window) opens
  srp's exchange window instead, on the buy side: Pay silver, Receive gold.
- **Where:** the "Get Gold" button of the "Insufficient Gold Rubles to complete the purchase."
  dialog, whichever window showed it: buying (the buy dialog, an item's card), repairing, rerolling,
  crafting, buying or extending premium, the premium discount; and the premium window's gold
  button, which asked "Online store will open in a browser window. Continue?" and now opens the
  exchange at once. The top bar's gold and silver boxes already opened it (revision 23's exchange).
- **Over the dialog it came from:** the exchange shows at once, over any window still open (the
  premium window, for one); that window stays visible under it, as under its own dialogs, and is
  active again when the exchange closes.
  The player presses Buy (or Repair, …) again: the dialog does not retry by itself.
- **The amount:** the exchange opens as from the top bar; it is not filled with the gold missing.
- **Texts:** unchanged. "Get Gold" fits buying gold with silver.
- **Not enough silver for the gold either:** typing more than the silver buys gives the most it
  does (`exchange.md`).
- **Server:** nothing changes; the exchange's `47 04` is the one already live.
- **Release:** a new `lobby_menu.swf` in a new `resources.db.patch_<n>`, built with every other
  lobby change from source by `build_lobby_026g.py` ([lobby-source.md](lobby-source.md)).
