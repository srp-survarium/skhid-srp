# Buy from a modified item's right-click menu

The rules the code follows. Code: `InventoryList.showContextMenu` and `LobbyMenu.try_buy_item` in
`flash/lobby_menu/src` ([lobby-source.md](lobby-source.md)); README "Buy for modified items". The owner's rule: 2026-10-20
("if the item is modified, when pressing rmb 'buy' is no longer there. I wish it were there and it
would just buy a normal unmodified item").

- **What:** right-click on an owned weapon or armour piece with modifications lists **Buy**, as it
  does for one without.
- **What it buys:** a stock item of the same kind, unmodified and new (the shop's item: its price,
  its seller); the modified one stays as it is.
- **When Buy is listed:** as for an unmodified item: the shop sells the item and the account has
  unlocked it at the seller (`canBuy`). Stacks (ammo, consumables) as before.
- **The "already purchased" question** comes first, as for every buy of an item owned (the owner,
  2026-11-01: "it shouldn't just be camo, it should also be scopes and other things"; `items.md`
  §6). From 2026-10-20 to then a buy on an owned item (its right-click Buy, its card's Buy) was not
  asked (the owner, 2026-10-20: "'the item is already purchased' should not show up").
- **Only owned items change:** shop entries, bundles and a bundle's contents keep the old rule
  (Buy when not of a rarity), so the shop's and the bundles' menus are as before.
- **Not changed:** the shop list (a shop item whose owned copy is modified is still listed in the
  shop too); the item card's buttons; the server (it buys by dictionary id and gives a new item
  without modifications).
