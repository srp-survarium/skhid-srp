# "Get Gold" and the exchange: removed

The owner, 2026-11-08: "Completely remove gold from the game, only silver should be left."
Revision 23's exchange window and the "Get Gold" buttons that opened it (2026-10-20) are gone from
the lobby movie with gold (the owner, 2026-11-08; [silver-only.md](silver-only.md)). The rules the lobby follows now, with no gold:

- **Balance:** the top bar shows silver only; the gold box is hidden and the silver box opens
  nothing (`StatusPanel`, `topPanelLeftContainer_56`). `PlayerInfo` keeps no gold balance.
- **Prices:** every price is silver, a premium item's too (`isPremium` no longer picks gold or
  multiplies by the exchange rate; selling gives `cost / 4`, times the condition). The buy dialog,
  the shop's cards, the bundles and the ammo dialog check silver (`MessageAsk`, `ItemCard`,
  `ItemControlPanel`, `ItemRendererInv`, `InventoryList`, `AmmoShareLite`).
- **Windows with a gold option** show the silver one only: a modification's change (`MessageRerollItem`), the upgrade (`MessageCraftItem`: the server sends
  the chances it rolls in the silver column), the repair (`MessageRepair`: no quick fix), the
  rename (`MessageEnterName`: paid in silver at 0x8D's rename price). Each sends payment 0. The
  skills reset has no window: it is free and the Reset button sends it at once, payment 0
  (`CharWindow.rerollChanges`, silver-only.md).
- **Premium time** is gone with premium itself (the owner, 2026-11-09; [premium.md](premium.md)): its
  windows are out of the movie.
- The exe's callbacks that hand over gold values (`set_premium_exchange_rate`,
  `setup_item_craft_prices_premium`, `set_craft_premium_cost`, `set_reroll_cost`'s second value)
  stay and take nothing.
- The exchange window's symbol (`SrvMessageExchangeSkinned`) stays in the movie's graphics, bound
  to an empty class; nothing opens it. Its texts (`st_srp_exchange_*`) are gone.
- **Server:** the exchange's `47 04` is refused.
