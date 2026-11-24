# Store action tabs, offers, event exchange, favourite-weapon click, specifics (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Sources: the exe, the game data and the lobby movie. Tags: PROVEN = read in the exe
(address) or in a game file; LIKELY; CHOICE (server rule no evidence settles); OPEN (not established, not implemented).
Addresses are VAs (image base 0x200000000). Related: `store.md` (premium store, 94 / 95), `items.md` section 6.

Code: `servers/lobby-server/src/actions.rs`, `crates/protocol/src/binconfig.rs` (the config writer), `crates/protocol/src/s2c/hero.rs`
(`BC`), `crates/accounts/src/hero.rs` (`buy_specific`). Tests: `servers/lobby-server/tests/hero_actions.rs`.

## 1. What it is

The store window (AS `survarium.shop.StoreScreen`, `StoreActions`) has tabs and, per tab, a list of action cards (bundles,
chests, roulette, a favourite-weapon discount) that are bought for gold or real money. Neither the tab list nor the cards are in any
game file (store.md section 3: no price, duration or content table exists in the client data). They were real-server data.
The Rust server therefore takes them from a SETTING that is EMPTY by default, and every piece around them is real.

## 2. Tab list: push 95 (PROVEN)

`95` ends with `u8 n2, n2 x {u32 id, str(64) name, u8 flag}` (reader 0x20018AA80, records at L+0x7A33D0). The consumer
(0x200175C48..0x200175D70) turns each into `root.set_shop_action_tabs {id, name, default_gold = flag}`, AS `StoreScreen.setTabData`.
Setting `LobbyConfig::store_tabs` (`actions::StoreTab`): default = one tab `{id 1, "Special offers",
default_gold 1}` (`actions::default_tabs`, CHOICE name / id; `StoreScreen.setTabData` shows `String(name).toUpperCase()`, plain
text, and the server tabs REPLACE the client's five built-in tabs `StoreScreen.onLocal`). Empty = no server tab. The shop
elements of 95 are the premium window's offers (store.md "Store offers and pictures"), not discount actions.

## 3. `5C 00` get_actions_data -> `BC` sub 0 (PROVEN layouts)

* Request (builder 0x200196130, log `ingame_shop_data_request tab %u`): `5C u8 0, u32 tab, u8 language, u8 steam (platform == 2)`.
  The client first looks the tab up in its cache (queue at L+0x7A3350, entries `{config, tab, expiry}`): a live entry is shown without any
  request; an expired or missing one is dropped and the request goes out. After a buy it sets the expiry of every cached tab to 0.
* Answer `BC` sub 0 (reader 0x200196540, log `read_ingame_shop_contents tab %u (%u bytes)`): `u8 0, u32 tab, u32 len, len bytes (a
  binary config), u8 more, [u32 weapon dict id, u32 web shop id if more]`. The blob is parsed by the engine's config reader (the same
  format as the `.options` files, see section 7) and handed to `root.set_actions_store_data` (0x20028E8A0). `more` != 0 marks the
  favourite-weapon card (`is_favorite_weapon`, `weapon_dict_id`, `web_shop_id`), which the server never sends.
* Keys read from the blob root (0x20028E9D3..): `currency_type`, `currency_sign`, `shop_items` (an indexed table; anything else skips
  the loop and shows an empty list). The mandatory key set per item is in section 11.
* Server: `actions::get_data` answers every `5C 00` (also for a tab it does not list, so the window never waits) with a blob built by
  `actions::tab_blob` (the PROVEN key set of section 11): root `currency_type` "usd" (a STRING: the name of the real-money
  table in every `cost`; CHOICE value), `currency_sign` "$", `shop_items` = the offers of that tab, each with `id`, `type` 0, `ttl`
  (seconds, 0 = no timer), `ttl_discount` 0, `label`, `description`, `s_img`, `m_img`, `l_img`, `cost {gold {price,
  discount_price, discount_value 0}, usd {price 0, discount_price 0, discount_value 0}}` (real-money price 0 hides that button)
  and `bundle {}` or `bundle {items [{dictId, amount}]}` (the full view's "Contains" block: `actions::shown_items`, dict 0 =
  premium hours; store.md "Offer details and the running contract"). With no offers the list is empty (PROVEN safe path: the loop is skipped).
* Setting `LobbyConfig::store_offers` (`actions::StoreOffer {id, tab, label, description, gold, items, ttl_secs, images, grant}`):
  default = `actions::default_offers` (the 14-day premium card and the Winner's Contract card, CHOICE).

## 4. `5C 01` buy_action -> `BC` sub 1 (PROVEN layouts)

* Request (builder 0x200196CC0, log `Buy action %d`): `5C u8 1, u32 data id, u8 mode`. Only mode 1 is sent: the client's payment kinds are
  indexed `Silver, Gold, RealMoney, GlobalEventCurrency` (table 0x2009907E0); the Flash event passes mode 1 or 2, and 2 sends nothing (the client
  only expires its cached tabs). So an action is bought with GOLD.
* Answer `BC` sub 1 (reader 0x200196540 tail, UI `root.set_buy_action_result {data_id, result, chest_item_index}`): `u8 1, u32 data id, u32 chest
  item id (0xFFFFFFFF = none), u8 result`. The client waits for it. A true result makes the client send its own internal message 0x37.
* Server (`actions::buy`): the offer must exist, mode 1, enough gold; then by the offer's `grant`: `Items` = one transaction spends
  the gold and grants the bundle (money items 214 / 215 / 925 are credited, the other items added; the records follow as `83 45 0F`,
  then 8B); `Premium { hours }` = gold taken and premium extended like a voucher, 8B + 94 (`premium::buy_card`);
  `WinnersContract` = gold taken and the contract started, 8B + 94 with the contract record, refused while one runs
  (`contract::buy`, store.md "Store offers and pictures"; the contract card is hidden while one runs and the tab is
  sent again after the purchase, store.md "Offer details and the running contract"). Every outcome answers `BC sub 1` (unknown offer, other mode, too little
  gold or a running contract = result 0, nothing changes). An offer with a chest (`chest item id`) is not made.

## 5. `46 08` favorite_weapon_discount_clicked

`46 08 u32 action, u8 tab` (builder 0x200197560, Flash handler 0x28FCE0): the player clicked the favourite-weapon discount card
(section 3, `more` tail). `83 46` has cases for subs 0, 1, 6, 7 only, so nothing waits for an answer (PROVEN). The
byte is the client global at 0x200AA2130 that `BC` sub 0 also compares with the tab. Server: logged, no state. The card itself needs
per-player weapon usage and a discount price, and the server has neither, so no such card is offered (OPEN).
PROVEN: the u32 is the 94 type-8 record id (the action), not a weapon. The card
is a `94` record of type 8 (builder 0x2001C6350): `data2 = u32 discount_item_dict_id, u32 discount_web_shop_id`, `id` = f30,
`actionTime` = f38 (seconds left), `ui` = data1; shown only when both ids are > 0 (`FavoriteWeaponActionItem`). A fresh cached
offer is shown without a request; no lock, no `83 46 08`. The window is filled by `BC` sub 0 with `more = 1` + `u32 weapon, u32
offer id` (LIKELY the real reply): header = name of `ui[4].list[0].dict_id`, sale text `ui[2].discount_value`, price
`ui[2].discount_price` (old price `full_price`), timer `discount_ttl`; buying is the ordinary `5C 01 offer 01` -> `BC` sub 1; the
percentage is display only (the offer's own price is charged). `B1` type 0x14 `[6][pct][icon]` = "Discount received! N%".
The server sends no type-8 record, so the card never shows.

## 6. `46 09` buy_usage_of_specific and push 91

* The pocket popup lists 19 consumables (static table 0x200976A90: dict 5, 65, 66, 67, 68, 82, 219, 733, 734, 736, 759, 760, 1089, 1090,
  1091, 1101, 1106, 1133, 1196). Push `91 u32 mask` (0x20018E48A -> 0x200247030, `root.set_specifics_info`): an entry is `opened` when
  the player's faction level reaches the item's level OR bit `i` of the mask is set. An unopened entry shows a price and a
  buy button: AS `SlotsSelector.callBuySpecific` calls `buy_usage_of_specific(dict id)` with `usePremiumMoney` true, i.e. the price is in
  GOLD; the price is `dataPrices[clamp(sellerLevel - 2, 0, len - 1)]`, `dataPrices` = the `buy_specific_cost` row the 8A `idx` names
  (`set_specific_prices`): `[[100, 150, 200], [150, 200, 300], [200, 300, 400]]` (`game.options`, PROVEN data; the server sends idx 0).
* Request `46 09 u16 dict` (0x24BBF0). No answer is read; the new `opened` state comes with a new `91`.
* Server (`actions::buy_specific`): refused (logged) for an item outside the 19, one already opened (bought, or open by faction level) or
  too little gold; else one transaction spends the gold and stores the bit (`specifics_opened`, schema v13), then `91` and `8B`.
  The price uses the item's level as `sellerLevel` (LIKELY).
* The mask the server sends carries only the bought bits; the client's faction levels do the rest, exactly as the client implements it.
* The `91` handler takes the faction levels from the stored `8E` records by position: record `f` is faction `f` (PROVEN, loop
  0x2002470B8..0x200247119 reads `[records + 12 f + 8]` for f = 1..4; the daily-quest `AB` reader 0x200188398 does the same). The
  server's `8E` therefore starts with a faction-0 record (0 points) before factions 1..4 (`build::factions_8e`). With only four
  records the client would read each faction's level from the next faction's points and faction 4 from past the end of the array
  (heap memory), and the popup would open items the server refuses. The readers that find a record by its faction field (`setup_player_progress` 0x2001C1A82 skips a
  faction outside 1..4, 0x20018ADE0, 0x20011B699) do not mind record 0. The popup ORs the `91` result with the `8E` level
  (`GangShop.getSellerUnlockedLevel`), so the 91 sent at sign-in before the 8E (levels then 1) under-opens nothing for long. A locked tile is greyed with a lock, the faction emblem and the
  rank; a click opens the gold dialog (`st_force_unlock_for_gold`).
* The popup builds a tile only for an item whose 8A record has byte e (`for_sale`) = 1. All 19 are listed: 8 of them (733, 1089, 1090,
  1091, 1101, 1106, 1133, 1196) have no 0.69d or wiki price and carry our own silver price (CHOICE, store.md section 5). The gold
  unlock price does not depend on that silver price. The server keeps a locked item that already sits in a pocket: nothing removes it
  (the client's 91 handler keeps no state and its lobby movie moves no item on `opened`).
* Where the client tests `opened` (PROVEN, lobby movie; every AS reference to `opened` is one of these methods): only the popup tiles.
  `SlotsSelector.filterItemsByCategories` sets it per tile, `ListSpecItem.updateSlotState` draws the lock, `handleListItemClick`
  opens the gold dialog for a locked tile (never a placement) and `startItemDrag` refuses a drag from the list when not opened. The
  tile is a shop item: the units the player owns play no part. Moving an item between pockets (`exchangeSlotData`), the pocket count
  stepper (`handleSlotCountChanged`), a drop on a paper-doll slot (`ItemPaperDollSlot.handleDropEvent`) and the preset load have no
  `opened` test. The exe keeps no `91` state (0x20018E48A passes the mask to 0x200247030, which only builds the AS array).
* The server enforces the client's rule (`actions::pocket_open`): a pocket consumable is open when its faction level is reached
  (`progress::faction_allows`: level of the item's faction from the `8E` points >= the item's level) or it is one of the 19 and was
  opened with `46 09`. Items outside the table (2072, 1355, 1884) open by faction level only: AS `SlotsSelector.getItemOpened` scans
  only the 19 entries `91` delivered and returns false for any other dict, so the popup's `opened` is the faction rule alone (PROVEN).
  * `45 00` (`equip::apply`), an entry that puts a locked consumable into a pocket slot 13..17:
    * from the popup (source 200) it is skipped, owned units or not (the client's locked tile cannot be selected or dragged);
    * from anywhere else (an inventory stack, another pocket, the same pocket with a new count) it is placed when the owned stack
      covers every pocket that holds it (the source pocket of a move counts as emptied). Units it would have to buy are refused
      (CHOICE: the popup, the only pocket buy UI, refuses a locked item; nothing proves what 0.69d's server did).
    * A refused entry is skipped before anything changes: no stack is created, nothing is charged, the source slot keeps the item,
      and the 87 that follows shows the real slots (the silent refusal of every invalid move). Taking an item out (to 100) always works.
  * `58` preset load (`presets::apply`): every cell is set, a locked consumable too; the load uses owned units only (the rounds are
    capped to the stack, nothing is bought).
  * No other path fills a pocket slot (the new-player kit's medkit 67 and RGD-5 5 are level 1, open at 0 points).
* Flash grenade (2072) is listed too (CHOICE price); it is not in the table of 19, so the mask cannot open it and `46 09` refuses it. It is
  level 1, so the client opens it by faction level. Nut (1355) and Snowball (1884), also outside the 19, stay unlisted (event specials).

## 7. `48 28` action_item_exchanged

AS `ItemsExchangeEventWindow` (an event window opened from a system message that carries `actionItemsData`) sends `48 28 u16 source, u16
target` after checking `source_amount <= owned` itself, then locks its buttons. The client sets a request guard (L+0x7A5759, 0x2001EA503) that
only an `83 48` or `84 48` reply clears. Data (PROVEN): `gameplay/event_rules/event_exchange_action_items.options`
(`data/event_exchange_069d.json`): 1139 x 2500 -> 2200 (Barrett New Year 2022), 1138 x 1500 -> 2203, 951 x 750 -> 2202. The sources are event
currencies: the client skips ids 951 / 1138 / 1139 when it adds item records, they are wallet values (8B `event1..3`).
Server (`actions::exchange` -> `events::exchange`): implemented, see `events.md` section 4 (event wallet, `83 46 00` item +
8B + `83 48 03`; refused without a running event window, a poor wallet, an unlisted row or an owned decal).

## 8. `52` links

`go_to_store`, `go_to_knowledge_base`, `link_clicked`, `whats_new_clicked`, `system_message_clicked` and the real-money `buy_action` send `52 str`
(builder 0x200195B10). The only answer is `S->C BA` (`/client-auth/%I64u/%s` with the login token), which makes the client open the
operator's web page in a browser. Without it nothing happens and nothing waits (PROVEN). This server has no web site to open, so it
answers nothing (CHOICE); a `BA` with an operator URL would be the extension.

## 9. Binary config writer (`protocol::binconfig`, PROVEN format)

16-byte records `u32 data | u16 type << 12 | u16 count | u32 name offset | u32 hash`, root at offset 0 whose `data` points at its children right behind
it; types 0 bool, 1 int, 2 float, 3 named table, 4 indexed table, 5 string. A named table's children are sorted by the CRC-16 of the name, kept in the high half
of `hash` (the lookup 0x2000CC0E0 binary-searches the 16-bit hash at record offset 0xE; CRC-16/ARC with init 0, table at 0x20F43DF60). The writer is
checked against hashes read from `hero_of_the_day.options` and round-trips through a reader (tests). Whether the engine also needs anything the files do not
show (the loader 0x2000CC680 is not traced) is untested in the game.

## 10. To check in the game

* Default: the store shows one tab "SPECIAL OFFERS" with two cards: the big crown card "PREMIUM 14 DAYS" (500 gold) and the small
  contract card "WINNER'S CONTRACT" (100 gold, the "CONTRACT / DAY 1 / DAY 2" picture). Check that both pictures draw (a blank slot
  = a wrong file name), the full view opens with the gold button only, buying the premium card adds 14 days to the premium
  button's timer, buying the contract shows the contract button next to the premium button; a second contract purchase is refused.
* The pocket popup lists unopened consumables with a gold price (100 / 150 / 200 for the first row), buying one opens it for good.

## 11. Store offers and pictures

PROVEN unless marked; completes sections 3 and 4:
* The blob's image keys are `s_img` / `m_img` / `l_img` (the exe renames them to `image_small` / `image_medium` /
  `image_big` for the movie); the value is the full file name with extension (e.g. `673_s_1563455165.png`), loaded from
  `../textures/ui_actions/` (the client's own 444 pictures, volumes 00 ..). Each card uses one: item 0 = large (`l_img`,
  580 x 687), items 1-4 and later pages = small (`s_img`, 600 x 210), item 5 = medium (`m_img`, 600 x 450); a bigger picture
  is cropped. A missing FILE leaves the slot blank (LIKELY); a missing KEY hits the engine's fatal assertion (crash).
* Mandatory keys (a missing one crashes): root `currency_type` (a STRING), `currency_sign`, `shop_items`; per item `id`,
  `type`, `ttl` (seconds, 0 = no timer), `ttl_discount`, `label`, `description`, `s_img`, `m_img`, `l_img`, `cost.gold` =
  `{price, discount_price, discount_value}` and `cost.<currency_type>` with the same keys (price 0 hides the real-money
  button); type 0 / 1 need a `bundle` table (may be empty) whose items use `dictId` / `amount`.
* `actions::tab_blob` sends every mandatory key with its type (section 3; test `tests/store_offers.rs` decodes the blob and
  checks each key and type).
* Pictures by theme (file names in `textures/ui_actions/`):
  premium alone = `673_*_1563455165.png` (golden crown, no text) and the 213 crown with "x4"; premium + gold 73, 74, 59 / 62 /
  65 / 221; gold packs 253..262; silver + gold 79, 88, 89; the only contract art = `image_10.png` (a signed "CONTRACT" with a
  "DAY 1 / DAY 2" board and gold bars, the 2016 gold contract, 774 x 238); silver coins `events_small_03.png` / `image_6.png`.
