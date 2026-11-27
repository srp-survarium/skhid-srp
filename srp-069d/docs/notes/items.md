# Items: uids, item use, chests, presets, 48 02 (0.69d lobby)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Addresses are exe VAs (image base 0x200000000). PROVEN = read from the exe or the lobby AS3; LIKELY = inferred;
CHOICE = server rule with no client evidence.

## 1. Item uids start at 100001

* Cause (AS3): for ammo and pocket slots (13..17) the lobby movie calls `createBaseItem(uid)` first, treating the uid as
  a dictionary id. Uids 1, 2, 3 ... would collide with real dictionary ids (uid 44 = dict 44 armour legs) -> no tab for the
  category -> null -> crash (static analysis).
* Rule: `accounts::UID_BASE = 100001` (the largest dictionary id in `data/items_069d.tsv` is 2204). New profiles, `add_item`
  and the state importer (`accounts::import`) start there.
* Migration: schema v4 -> v5 (`SCHEMA_V5_UP`, one step with the presets tables) shifts every uid of every account that has an item below 100001 by +100000 in
  `items`, `slots`, `item_mods`, `attachments` (weapon and module), `bound`, `profiles.next_uid`. Two steps through negative
  values avoid primary key clashes. Test `v4_database_uids_are_renumbered` builds an old-numbered file and compares the
  result with the new-numbering profile (another account is untouched, second open changes nothing).
* `Profile::lift_uids` does the same for an imported state.
* Pockets (ammo and pocket slots) are always on.
* PROVEN statically (working with the stock client LIKELY): an unknown id makes
  `createBaseItem` return an empty object (no throw), so `getItemData` falls back to `getInventoryItem(uid)` with the real category;
  dictionary ids are u16 <= 2204, so uids >= 65536 can never collide. Rules: every uid >= 65536 (UID_BASE 100001); the 89 that
  carries a stack comes before the 87 that names it; only categories with a pocket tab (10, 24, 11, 34, 55, 54) in pockets:
  category 23 is allowed by `profile_slots_restrictions` but has no tab (same null crash, LIKELY), so `QUICK_CATS` does not
  contain it (refused by `equip.rs`); 91 before the popup opens. Ammo slots 8 / 9 use the same fallback with these uids.

## 2. Using consumables

* `45 03 u32 item` (sender 0x20018D7B0): reply `83 x 45 03 <u32 item>`; the client then decrements the stack by 1 and erases
  the record at 0 (0x19086D, PROVEN). `Inventory45::Used` encodes it.
* `90` (0x20018E3C5, PROVEN): `u8 n, n x { u32 uid, u8 amount }`: count - amount (floor 0), erase at 0, else refresh the row.
  Encoder `ItemCounts90`.
* `86` is NOT item use: inline 0x20018E340 -> `set_playing_order_tip(u8 kind, u32 arg)`
  (0x2001CB2C0, log "arrived playing order tips"), a matchmaking warning text (kinds 0..3 of the table at 0x2009674F0).
  Encoder `PlayingOrderTip` only; nothing sends it.
* "usable" in the client (ItemFactory.as ~1006): stack, category 27 (vouchers), voucher type 0, 3 or 4 = account rename,
  premium access (1/2/3/7 days), new player booster. Everything else has no use button.
* Server: `items::consume` is the real count-down (`Profile::consume`: count, removal at 0 with slot / modifier /
  attachment clean-up, rounds placed in ammo and pocket slots capped to what is left) and pushes 90 (+ 87 when slots changed).
* Premium vouchers are consumed by `premium::use_voucher` (store.md 2). Boosters are enabled with 45 0C (not 45 03,
  `boosters.rs`, store.md 4); the rename voucher is spent by the 48 05 flow (`rename.rs`, store.md 4b, the client sends
  the voucher uid there). Drop cards never come through 45 03 (PROVEN: `ItemFactory` makes only vouchers of
  `voucher_type` 0, 3 or 4 usable; `drop_card` 153 / `drop_card_free` 1132 are type 1): they are spent only by the treasure hunt
  (`46 07 00` -> `83 46 07`).

## 3. Chests and item search

* `83 46 06` (reader 0x2001930D8..0x200193626, PROVEN reads): `u32 chest item uid, u16 chest dict id, u8 n, n x item record`.
  The client removes one unit of the chest (stack: count - 1, erase at 0; else erase), adds the n records (same uid = count
  added, new uid = new item) and calls `0x248F50(chest dict id, rewards)`. `Shop46::Chest`.
* `83 46 07` (0x200192E55): `u8 a, u8 n, n x item record`, records merged like above, then `root.set_items_search_result`.
  Meaning of `a` unknown. `Shop46::Search`.
* `5A 02 u8 flag, [flag 0: u32]` / `5A 03 u8 u8` (item search opened / roulette change): decoded (`Roulette5A`). PROVEN
  (builder 0x200196F80): flag 0 is followed by the `a` of the last `AA` state (also sent after every faction level-up).
* Data: `data/chests_069d.json` (`crates/data-extract`): the 55 chest items of `game.options` vol 75 (category 31:
  `extraction_type`, `drop_type`, `items_count`, `items`) and the 29 tables of `gameplay/drops/chest_drops.options` (vol 67; per table groups
  `{drop_weight, categories, items_dict_ids, rarity [min,max]}`; group weights sum to 100, PROVEN by the data). Loaded as `GameData::chests`.
* `servers/lobby-server/src/chests.rs` opens a chest for real: roll (`game_data::chests`, rng = the lobby's SplitMix seeded from time and OS randomness, `Lobby::roll`), one unit taken
  from the stored stack (no 90: the client takes it itself, 0x200193113..0x200193135), rewards stored (stack merge, new uids from UID_BASE), answer
  `83 46 06`, then 8B (money), 8C (experience), 8E (reputation) when they changed. Money dicts 214/215/925, the event currencies 951/1138/1139 and the league
  currency 1356 are skipped by the client when it adds the records (0x200192EE8..0x200192F24, 0x20019335A..0x20019339F, PROVEN): they are credited and listed in the record list (uid 0) for the reward window only. 1164 reputation and 1165 experience are
  NOT in that skip list (the client would add them as inventory items): they go to the profile (active faction / xp) and are not sent as records. PROVEN: the reward window (0x200248F50)
  lists every record with a non-zero dict id (amount shown 100, no special case for 1164 / 1165), so listing them would also add
  inventory rows (LIKELY a following full 89 would remove them again). CHOICE: they stay out of the
  window (credited, not sent as records).
  `random_module_chest` (1872, dropped after won matches) is covered: one module out of its 186 ids.
* Roll rules. PROVEN (data): table per chest = `drop_type`, group chosen by `drop_weight`, amount in `[min,max]`, `items_count` draws. LIKELY: extraction type 2 = give every listed item with its amount (no table);
  draws are without replacement (event chest: 6 draws, 6 groups = one set piece each; rare module chests 10/10). CHOICE: uniform item inside a group (the dictionary `drop_weight` is 110..160 sort data, 0 for modules);
  uniform amount in the range. The four rarity buckets of every group carry the same range, so `rarity_points` cannot change an outcome and is not stored.
* NOT openable (logged, no change, no answer: the chest animation keeps waiting): the four `rank_N_equip_chest` (groups without item ids: the 11 item-level columns would need a mapping to the
  dictionary levels 1..5 that no file states), premium ammo chests 1..3 (every group weight 0), pve mission chests rank 2x..5x and `pve_mission_1_chest_closed` (their `drop_type` is missing from `chest_drops.options`).
* Check in the game: open a seven-day chest (fixed pack), a daily quest chest (money / parts / reputation groups: wallet updates, no stray inventory item), a random module chest; the reward window shows the items; the stack count goes down by exactly 1 (no double decrement).
* `83 46 07` / `46 07` / `5A xx`: see 3a (implemented).
* Where the lobby lists chests: NOWHERE (PROVEN, lobby movie AS).
  `ItemFactory.getCategory` maps categories 25..31 (camo, decal, voucher, regular, gold, details, CHEST 31) to superCategory 5
  (`BASE_VOUCHER`); the inventory list (`InventoryListBase.applyInventoryFilter`, every branch) keeps an item only when
  `checkBaseOptions` passes, and its `isStacked` is false for every stack and every superCategory 5 item. The inventory tabs
  (`FilteredWindow.fillCategoryTabData`: all, armour pieces, weapon types, modules, all armour) and the item shop tabs have no
  superCategory 5 tab. The chest "open" entries (context menu, `ItemCard` button -> `open_chest` -> `46 06`) are therefore
  unreachable for a stored chest; 0.69d opens chests in their own windows (daily quests `4A 04`, PvE end screen `5F`, league
  `ChestOpen`, treasure hunt). The exe builds a Flash item for category 31 (only 0x19 / 0x1A / 0x20 are skipped, 0x2492D0), so
  the record itself is fine.
* Dropped chests are opened at once (`play::reward_messages`): the drop message (`83 45 0A` on the
  victory screen, `83 46 00` without one) lists the chest stack (the panel's "?" item; the client adds it), then one `83 46 06`
  per unit (`u32` the listed stack uid, chest dict, the rolled records) takes the unit out again, adds the contents and calls
  0x200248F50 = `root.add_important_message` type 25 `CHEST_OPENED` (the chest notification); then 8B / 8C / 8E. The `46` reply
  reader (0x200192DF0) switches on the sub byte with no pending-request check (the unsolicited `83 46 00` popup already uses
  it), and a sub-6 chest uid the client lacks only skips the unit removal (loop 0x200193103..0x200193190). That the 0.6x server
  gave the module itself is LIKELY (0.51b notes: modules drop after matches; `drop_after_combat` win item 1872 has prob 1.0).
  A chest the data cannot open stays in the inventory (logged).
* Chests given by a reward list are opened at once too (`chests::grant` / `chests::open_chests`): a faction battle tier
  (`rare_modules_chest_1..4`, 2033-2036), a configured store offer, a login calendar day or an event exchange that names a
  chest gives the rolled contents (same tables and module rolls as `46 06`), never the chest stack. The answer of that
  feature lists the contents (no `83 46 06`, no chest notification); every opened chest leaves a log line
  "granted chest dict N opened". A chest inside a chest is opened too.

## 3a. The treasure hunt (item search roulette): decoded and implemented

Server: `servers/lobby-server/src/hunt.rs`, data `data/weekly_drop_069d.json` (`crates/data-extract`, from
`gameplay/drops/drops.options` of volume 72), game-data `hunt.rs`, accounts schema v8 `hunt` + `hunt_entries`.
In the client the feature is called "stash maps" / "treasure hunt" (`st_treasure_*`); the drop card items are dict 153
(`drop_card`, 100 gold in the shop) and 1132 (`drop_card_free`).

PROVEN (exe, AS3):
* `AA` push. Sub 0 = state, reader 0x200188BD0 -> 0x200180FA0 (+ 0x200180F20): `str32 config name` (a key of
  `weekly_drop_configs`), `u32 a` (0 ends the message: no hunt), then `u32 b, u32 c, u8 d, u32 time_left, u8 level,
  u8 page, u8 max_level, u8 min_level, u8 n, n x { u16 dict, u8 not_found, u8 m, m x {u16, u16} }`. Stored at menu
  +0x7A4640: name +0, a +0x20, b +0x24, c +0x28, d +0x2C, time +0x30 (AS: `actionTime`, seconds, shown as minutes),
  level +0x34, page +0x35, max +0x36, min +0x37, the item list +0x38 (0x30 B per entry). Sub 1 = `str32, u32 a, [a != 0:
  u32 b, u32 c, u8 d]` (counts only, calls `root.update_items_search_data`).
  The UI builder 0x20024E260 (called when the counts change, from the buy handler 0x193A18 and sub 0): reads
  `weekly_drop_configs[name].drop_by_level[level][page]` and per config item `dict_id` / `min` (= shown amount) / `prob`
  (shown as a percent) / `rare_level` / `item_quality`; the entry `dict` the server sent is the concrete id of a
  `dict_id` list; `owned` = the entry's `not_found` byte is 0 (0x20024E896); all `owned` = AS `allOwned` = "all the
  stashes were opened" (search disabled). It then calls `root.set_items_search_data` with `ui[]` = the `ItemSearch`
  element {`shop_maps_count` b, `free_maps_count` c, `used_maps_count` min(d, 2 - c), `free_maps_max`,
  `max_free_drop_cards`}, the `Roulette` list and `CostGold` {`full_price` = `premium_money_to_open`}, plus
  `can_change_drop_level`, `min/current/max_roulette_level`.
* b and c are the stack counts of dict 153 and 1132: the `83 46 00` handler sets +0x24 / +0x28 from the merged record
  of dict 0x99 / 0x46C (0x19388E..0x1938A2). `ShopCards` draws 2 free slots: slot i is selected when i < used (d),
  enabled when i < c + used; the paid count shows as "+b".
* `46 07 u8` = `start_item_search(payment)` (`ActionBigStoreRenderer.onButtonClicked`): 0 = drop card
  (`Payment.VOUCHER_MAP`), 1 = gold (the AS first runs `BuyManager.handleTryBuy`). Answer `83 46 07 u8 a, u8 n, n x
  item record`: `a` = index of the found entry (the roulette stops there), records merged as in 46 06; the client
  sets `foundItem` to the first non-stack record's uid (0x192F40). Refusal = `84 x 46` generic denied (0x18F445,
  reader 0x20018FE10; handler 0x176BF8): `code 0, u8 != 0xFF, u32 reason` 0 failed ("st_opening_treasure_failed"),
  1 not enough drop cards, 2 not enough money, 3 all treasures opened; then `root.unblock_roulette`.
* `5A 03 i8 delta, u8 gold` (`RewardRerollWindow.changeRouletteLevel`): delta 0 = update the list, -1 / +1 = level;
  gold = the client has no card (`isVouchers = free > 0 || shop > 0`); the AS cost of a change is one card
  (`MAP_ACTION_COST`) or `full_price` gold. `5A 02 u8` is sent when the hunt window opens (`item_search_opened`) and
  when the found-item message closes and the list is not exhausted (`last_item_search_ok_clicked`).
* `st_reroll_window_max_level_reached` = "Maximum treasure tier is limited by your faction level";
  `st_treasure_maps_info` = the free-card rules (below).

Data (PROVEN): `weekly_drop`: `max_free_drop_cards_in_period 2`, `max_free_drop_cards 2`, `can_change_drop_level true`,
`drop_cards_to_open 1`, `premium_money_to_open 100`; levels 1..5 with 10 / 14 / 14 / 17 / 19 pages, `roulette_prob` of
the pages of a level add up to 1, per page 5..15 items `{dict_id (id or pool), min, max, prob, rare_level,
item_quality}`. The second config `weekly_drop_new_year_2018` (55 pages per level, no level change) is the 2018 event:
not used (OPEN: when to switch to it).

Server rules: PROVEN where above; CHOICE: period = a week from Monday 00:00 UTC (the name `weekly_drop`, the client
shows a countdown but no length is in the data); a new period gives a new page and resets `d` (free cards used),
held cards stay (`st_treasure_maps_info`: "won't disappear when Treasure Hunt resets"); a new hunt starts at the
highest level; the highest level is the active faction's level capped at 5; a page is picked by `roulette_prob`, a
search finds one not-yet-found entry weighted by `prob` and gives `[min, max]` units (uniform); free cards are used
before paid ones (client text); a free card drops after a finished match with the 20 % of the client text while
`held + used < 2` (a 15-entry `prob` list for dict 1132 sits in `drop_after_combat`; what its index means is not
provable: OPEN, not used); a refused search / level change costs nothing. Rewards use the chest granting code
(money dicts credited, items get uids from UID_BASE, 89 / 8B pushes); `item_quality` adds hidden modifiers (below).
PROVEN: `item_quality` is the item's rarity rank; for non-module items it is also the
number of hidden ("unknown") modifiers the hunt window promises (`unknown_mod_count`); for modules (categories 39, 41..53) the
window shows the `{u16, u16}` pairs of `AA` sub 0 instead. In the data it is 3 only for 74 weapon entries. CHOICE
(`hunt::with_hidden_mods`): a found non-module, non-stack item gets `item_quality` modifiers rolled by
the workshop rules (`CraftData::roll_mods`: allowed types from `available_modifications_mask`, one per type, grade by
`drop_weight`), stored with the item and sent in its `83 46 07` record; the real server's roll is in no file. A weapon with
3 modifiers is at rank 3 (the workshop maximum).
Order after a search: `83 46 07`, `90` (cards), 8B / 8C / 8E when they changed, `AA` sub 1 (counts); `5A 02` / level
change / login: `AA` sub 0. Check in the game: open the stash window (the list shows the page, the counts, the
two free slots), search with a card (roulette stops on the sent index, the item arrives, the card count drops), search
until "all opened", change the level / update the list (one card or 100 gold), and that the found entry is greyed after
the message closes (if it is greyed too early or not at all, the AA order after a search needs changing).

More server rules (CHOICE):
* Owned gear is not offered: when a page is made, a page entry that is a pool of ids never draws a weapon or armour piece
  the player already owns (a skinned weapon counts as its base weapon) while the pool has another id; when he owns every
  id of the pool, the plain draw stays (`hunt::new_page`, `HuntConfig::resolve_page_skipping`). Log line: "hunt page
  level L page P: N entries drew another pool id instead of owned gear".
* The weekly reset uses real dice everywhere: at sign-in, when the window opens, and after a finished match
  (`hunt::after_match`), so a player online over Monday 00:00 UTC whose next match ends before he opens the hunt gets a
  randomly drawn page, not always the first page and the first ids. Log line: "hunt: new page after a match".

## 4. Presets (58 requests, BB push)

* `58 <op>` (sender 0x2001EAB40 = `process_default_stored_preset_action`, PROVEN incl. which op is which via the log strings):
  0 create `str name, u32 profile`; 1 remove `u32 preset`; 2 rename `u32 preset, str name`; 3 load `u32 profile, u32 preset`;
  4 save `u32 profile, u32 preset`.
* `BB` (0x200195CA0, PROVEN): `u8 n (<= 100), n x { u32 id, u8 flag, str128 name }`; the client zeroes its table first, so
  BB is the full list. The flag is stored and never read (sent as 0). The client shows names only.
* Storage: tables `presets` / `preset_slots` (schema v5), `Store::{presets, create_preset, save_preset, rename_preset, remove_preset}`.
  A preset is the occupied slots (slot, uid, rounds).
* At most 10 presets, 100 while the premium account runs (PROVEN: BB passes 10 / 100 to `set_presets` at
  0x200195E4A / 0x200195E59, `PresetSelector.updateCurrMaxPresetsCount(premiumState > 0)` picks one; `MAX_PRESETS` /
  `MAX_PRESETS_PREMIUM`; presets made during premium stay after it). CHOICES: create / save snapshot
  the current equipment; load equips the slots the preset names whose item is still owned (slots it does not name keep their
  item; pocket cells included; a non-stack item moves out of its other slot; rounds capped to the stack),
  bumps the profile version and pushes 87; every op is answered with a fresh BB; BB is also pushed after the lobby-load
  handshake. A request naming another profile than 1, or an unknown preset, is ignored (BB still sent).
* Needs a game test: that BB after the handshake enables the preset UI, create / rename / load / save / remove behaviour, and
  that the buy-preset flow (`48 03`, cost `buy_preset_cost` 200000 / 500000) is not required (not implemented).

## 5. 48 02

`48 02 u32 profile` is sent 0.5 s after the lobby loads. Reply `83 x 48 02 <u32 profile id>` (reader 0x200192CDD, PROVEN: finds
the profile record with that id, makes it active, rebuilds the character). Server (`profiles.md`): switches the worn items to the named profile and answers with the current profile id;
an unknown or locked id is answered with the current id. Presets act on the current profile.

## 6. 5C 00 (in-game shop tab)

`5C u8 u32 u8 u8` (`get_actions_data` 0x200196130, log "ingame_shop_data_request tab %u"). PROVEN: the pending-tab queue at L+0x7A3350 is filled by push `BC` sub 0
(reader 0x200196540): `u8 0, u32 tab id, u32 len, len bytes (a binary config object), u8 c, [u32 a, u32 b if c]`; the config goes to 0x20028E8A0, which builds Flash
`root.set_actions_store_data` (0x20028E8A0..0x20028FC34; keys read from the config: `shop_items` (each: `type`, `label`, `description`, `ttl`, `ttl_discount`/`discount_ttl`, `currency_type`, `currency_sign`,
`cost`/`price`/`full_price`/`discount_price`/`discount_value`, `Cost`, `CostGold`, `bundle` with `items` of `{dict_id, amount}`, `bonuses`/`bonus_value`, images `image_small`/`image_medium`/`image_big`,
`web_shop_id`, `weapon_dict_id`, `is_favorite_weapon`, `Roulette`, `chest`, `drop_chance`)). Not 94/95/8A (95 = premium shop elements, a different list).
Server (docs/notes/actions.md): the tab list of 95 and the offers are a server setting that is empty by default, `5C 00` is answered with `BC` sub 0 (a binary config
with an empty `shop_items` when no offer is set), `5C 01` buys an operator offer with gold (`BC` sub 1). The offers themselves stay OPEN: no game file has them.

## `items.count` of a non-stack item is a dictionary id (storage convention, not a count)

The `count` column holds the u32 of item record +0xC of `87` / `89` as it goes on the wire
(`game_data::items::ItemDb::amount_field`): the unit count of a stackable item; for every other item its original dictionary id
(`base_item_dict_id` of a weapon that has one, else its own id). So most rows of an account show `count = dict_id`.
* The client needs it (PROVEN): for the weapon categories {8, 13, 14, 15, 16, 17, 36, 40} it reads +0xC as a dictionary id
  and takes that item's `parameters.sale_confirmation_level` (0x2002496F8 -> dictionary lookup 0x200123410); for modules it
  is `original_dict_id` (0x200249965); `condition_or_stack` takes +0xC only when the dictionary row's byte +0x306 is set, else
  the constant 100 (0x2002493C0).
* Server readers: `build::record` sends it unchanged; counts, consumption, pockets and boosters read it only for stackable
  items; a skinned weapon keeps its real dict in `skin_base` (`Profile::real_dict`), not here; `online::StartCharge::between`
  sums per dict but only takes what decreased at a match start (money, tokens, booster stacks), never a non-stack row.
* Nothing to change. Do not "fix" the column to 1: the client would read dict id 1 for a weapon's sale confirmation.

## Buy a locked faction item (`46 00`)

The client's rule (PROVEN, lobby movie `GangShop.updateItemInfo` / `setShopItems`): a shop tile is buyable (`canBuy`) when
it is a gold item (8A kind 1) or a league item (kind 7), or the account's level in the item's faction reaches the item's
level (`faction_level` of the tile = dictionary `parameters.item_level`, 0x20014485E; faction = `parameters.faction`). An item
of no faction compares the best of the four faction levels (`getMaxSellerLevel`). Levels come from the `8E` records and
count from 1. The server applies the same rule (`progress::shop_allows`).

A locked silver tile is still clickable (PROVEN):
* `canForceBuyPrem` = not gold, not league, not `canBuy`; the tile shows a gold lock and `ShopListBase.onListMouseClick`
  opens the trade dialog (`LobbyMenu.try_buy_item`).
* The dialog takes the price from `GangShop.getItemLockedCost` = the 8A `c` field (`premium_cost`, 0x2001C0111) of a locked
  item. With `c` > 0 it shows that gold price and `st_force_unlock_for_gold` ("To buy this item using silver you must
  reach the rank of [faction_level] in [faction] faction. You may also buy the item now using gold."), and Buy sends
  `46 00` with the last byte 1 (`buy_ok_clicked` 0x20024AEB0 passes `usePremiumMoney`).
* With `c` = 0 (our 8A) the dialog shows the plain silver price and sends `46 00` with the last byte 0.

Server (`shop::buy`): a locked item is refused. The answer is `83 46 00 00 00` (unlocks the shop window), 8B, then `84 46`
with a text key, shown as a system message (display 0x200176BF8, text path; it also clears UI locks 1, 2, 4):
* faction level too low: `st_item_sell_precondition_faction_<f>` ("Reputation with <faction> required"). All 11
  languages have the four keys. No client code reads them: LIKELY the original server's refusal text.
* no price, or a faction-free item above the best level: `st_cannot_buy_item` ("Cannot buy the item"; CHOICE of key).
* not enough money: `st_not_enough_money`.

OPEN: the gold unlock price of each item (8A `c`). No game file or web source gives it, so the server sends `c` = 0 and
does not sell locked items for gold.

## Sell a weapon

Selling a weapon frees its installed modules (they stay in the inventory, uninstalled, bindings kept); the reply is followed
by `83 45 0F` records of the freed modules so the client stops showing them attached. 0.63 dev note: "When selling a weapon its
modules are removed automatically" (ambiguous: freed vs deleted; freed chosen, nothing is lost for the player).
