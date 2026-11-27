# Workshop: module merge, module prices, spare parts and module sources (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Static research plus Rust. Nothing was run in the game. VAs: exe base 0x200000000. AS = lobby movie ActionScript
(package `survarium.craft`). PROVEN = read in the exe, AS or game data. LIKELY = inferred.
CHOICE = a server rule that no source gives.

## 1. Module merge 45 10

Request `45 10`: `u32 x4` (module, 3 donors), `u8 pay` (1 = gold button, 0 = the free button). PROVEN: AS
`ModuleUpgradePanel.upgradeModule` (array of the module and `maxDonors` = 3 donors, `uint(free ? 0 : 1)`), C++ 0x187CD0.

Reply `83 45 10`: `u8 result, u32 module uid, [result not 0 and not 2: u8 n (<= 4), n x (u16 group, u16 grade)]`
(0x190C33..0x190D8B, PROVEN: result 0 skips the list, result 2 shows "cannot_craft_item"). The client looks the module
up BEFORE the list is applied and keeps its old modifier count, so the server sends this reply first and the item
records after it (LIKELY order, matches the code). Result 1 = success, 0 = the roll failed ("neutral" label).

Item records afterwards: `83 45 0F` carries the module (new modifiers), every weapon a consumed module sat on, and the
donors with count 0 (record count 0 = removal, 0x1910FD PROVEN); then 8B; 87 when something installed
changed (the lobby character is rebuilt on a version bump only).

### Rules (code: `crates/game-data/src/merge.rs`)

| rule | source |
|---|---|
| rarity of a module = grade of the modifier pair of group 312 in its record, 0 when absent | PROVEN 0x200249A00..0x200249A2A: the item record's pair list is scanned for group `dict+0x3CB8` (= `module_rarity_modifier` 312), the grade byte goes to Flash `moduleRarity` |
| rarity enum: 0 grey, 1 green, 2 blue, 3 orange, 4 gold, 5 legendary | PROVEN AS `ModuleRarityEnums` (the "purple" of the 0.69 note is rarity 3; `modules_rarity` in game data drops 0..2 and gives 3 only to its last row) |
| donors: same rarity as the module; for an orange or gold module an orange, gold or legendary donor; a module installed on a weapon is listed only with "show installed" ticked (bound but detached modules are listed) | PROVEN AS `ModuleUpgradeInventory.filterByRarity` / `checkDonorFilter` (`attachedToInstanceId`). 0.69 note: "Modules of max quality (purple ones) now can be used to upgrade other modules" |
| exactly 3 donors, all different, all modules of the account (donor may be installed on a weapon: the client only warns) | PROVEN AS (`MAX_DONORS` 3, `checkDonorFilter`, `showInstalledModulesWarning`) |
| result rarity: orange and gold -> legendary, legendary cannot be merged, else +1 | PROVEN AS `getNextRarity`, `showMaxUpgradeWarning` |
| gold price = what the gold button shows: `premium_to_craft[min(rarity - 1, 2)]` of the module's dictionary row: grey 0, green 10, blue 25, orange and gold 50; the free button costs nothing | PROVEN, see "Prices the panel shows" below |
| donors are consumed, the module keeps its uid and dict id; its drop modifier grades are drawn again from the table of the new rarity (blue `second`, orange `pve`, legendary `fourth` = grades 10 / 10), other pairs (override) kept | consumed donors CHOICE; the redraw PROVEN in direction (section 7: the panel shows the new rarity's grade bounds as `next_min_value` / `next_max_value` before the merge, 0x2001BCF60; 0.53 notes "obtain rarer modules with improved characteristics"); which table: the one holding only that rarity (LIKELY) |
| success chance | `craft::CRAFT_CHANCE` = 50 % (free button) / 100 % (gold button): fan wiki "Workshop" (survarium.fandom.com, snapshot 2022-05-27), row "Same Rarity": success 50 % silver / 100 % gold, failure 0 % / 0 %. The client carries only ONE pair, push 9B -> `ModuleUpgradePanel.setCraftChances([free, pay])` (AS: `freeChance` = [0], `payChance` = [1], text `value + " % " + chance_value`) and the same pair feeds `CraftItem` (weapon/armour craft), so the server sends it in the handshake (`login::handshake`) and rolls exactly the shown values (lobby generator, `CraftData::roll_success`). The values are not in the client or the game data; that the 2022 wiki table holds for 0.69d is LIKELY, unchecked |
| missed roll | client result codes PROVEN: `setCraftResult(item, success: Boolean)` shows `craft_result_success_label` or `craft_result_neutral_label` only; reply result 1 = success, 0 = neutral (no list), 2 = refused. There is NO "module lost" outcome in the client. CHOICE (no source): a missed roll consumes the donors and the gold (the button was paid) and leaves the module unchanged; the module's rarity stays |
| mixed-rarity combinations | wiki rows "Blue + White or Green + Blue" (success 25 % / 50 %, failure 25 % / 0 %) and "Green + White" (12 % / 25 %, failure 67 % / 25 %): NOT APPLICABLE in 0.69d. `filterByRarity` lets only same-rarity donors through (orange and gold are interchangeable, both count as the "same" row), and the panel can show one pair of numbers only. LIKELY the page text ("upgrade a blue or green item with an item of a lower rarity ... chance to lose your modification", rubles, 6 h timer) describes the old pre-0.43 item workshop; the "lose your modification" failure has no counterpart in the client's merge |

Refusals (`result 2`, nothing charged, nothing changed): duplicate or unknown uids, a non-module, mismatching donor
rarity, legendary module, price row missing or not enough gold.

### Prices the panel shows (PROVEN)

The client takes both prices from the item's own dictionary row (game data), not from any server message:

1. Opening the module upgrade sends `workshop_item_opened(module uid)` (handler 0x20028DB70 -> 0x20028CA90); the client
   passes the row's `details_to_craft` (dict +0x2BE, 3 x u16) and `premium_to_craft` (dict +0x2C4, 3 x u16) to
   `root.fill_item_craft_data`. Lobby notices 94 of kind 0x3A / 0x3B / 0x3C (modules) or 0x24 / 0x25 / 0x26 (other items)
   would replace or scale the gold row (switch 0x20018A13A); the server sends none of them, so the dictionary rows apply.
2. The module panel (`ModuleUpgradePanel.updateCraftPrice`) shows `premium_to_craft[min(rarity - 1, 2)]` on the gold
   button; a grey module (rarity 0, no group 312 pair) reads index -1, shown as 0. So the gold button shows 0 for grey,
   10 green, 25 blue, 50 orange / gold (all 416 modules: `premium_to_craft` 10 / 25 / 50), and the server charges
   exactly that (`merge::gold_price`). Grey modules merged with the gold button cost nothing at 100 %.
3. The free button has no price and no buy check; `details_to_craft` is 0 on all 416 modules. The server charges nothing
   for it.
4. Weapon / armour upgrade (`CraftItem`, 45 07) takes the same rows by the item's rank; these are the dictionary values
   that `CraftData::craft_price` charges (rank 3 uncraftable).

OPEN: whether the original server also charged 0 for a grey merge (the client shows 0 and lets the click through;
nothing else is known).

Push 9B is one pair for the module panel AND `CraftItem`, so the weapon/armour upgrade (45 07) also rolls
50 % with spare parts and 100 % with gold. A missed 45 07 roll charges the price and answers result 0 with no offer.

Open: whether the 2022 wiki chances hold for 0.69d (nothing in the client or data confirms them); what a missed roll
does to the donors in the real server (consumed here: CHOICE); what the donor's own modifiers become (nothing: they
are consumed). A merge redraws the drop modifiers (section 7).

## 2. Prices

* Consumables (grenades, mines, traps, medkits): prices of the fan-wiki Items page (snapshot 2022-05-27) fill items
  that have no faction-shop price: RGD-5 30, smoke grenade RDG-M 50, PMN-3 150, MON-50 200, bandages 20, Scavengers
  medkit 50 silver (full rule and conflicts: store.md section 5). RGO and 7 more pocket items without any source carry our own price (CHOICE, store.md 5a).

* Modules: 8A `a` = spare-parts price = dictionary `cost` (currency 4). 8A `c` = gold price = `a / 20` (integer
  division: 200 spare parts = 10 gold, 100 = 5; `shop::module_gold_price`): CHOICE, no source. The
  0.69d client and game data hold no gold price of modules (`cost` is one number with currency 4;
  `detach_price_in_gold`, `premium_to_craft` are other things); the real server
  sent one in 8A `c`. With `c = 0` the workshop would offer a 0-gold "shop" entry for every sold module and buy
  missing mounts for owned scopes with gold (`60 01` action 3). A module whose `cost` is 0
  (207 of 416) is not sold (`for_sale` 0, `c` 0, buy actions refused): it comes from drops. The client's own cost field
  in the buy action is not trusted.
* Vouchers (category 27) and boosters (category 23): the dictionary's own `cost` and `currency` (1 gold, 0 silver):
  premium vouchers 1d/2d/3d/7d = 50/100/150/200 gold, repair 50, rename 250, drop card 100, PvE ticket 50 gold;
  weight/speed/... boosters 500 silver; the 24 h experience/money/reputation boosters 5 gold. Cost 0 = not sold.
* Items with no real price (no wiki row, no dictionary cost, no own price of store.md 5a) are not sold, and
  `buy_total` refuses any record that is not `for_sale`.
* CHOICE: ammo not on the wiki page has unit price 0 and stays free; premium weapons
  without a wiki row keep `50 * level` gold in their (hidden) record; sell base `b` = price / 4 for silver items.

## 3. Spare parts and module sources after a match

PROVEN negative: message 97 carries NO spare parts and no item drops. Its fields are money, reputation, experience per
reward block, badges, per-weapon `weapon_upgrades_points` (0x2001423D1), nothing for `PlayerInfo.craftParts`.
Spare parts reach the client through 8B (field 4), received items through `83 46 00` with flag 1 (popup per record, the
handler adds no row for dict 925 `details`, 0x2001937D2..0x19392D). The drops of a match with a results
screen go as `83 45 0A` before the 97 instead (victory-screen extra rewards panel, events.md 3.2).

PROVEN data: `gameplay/drops/drops.options` -> `drop_after_combat` (89 rows; copy of the used rows in
`data/drops_069d.json`, script `crates/data-extract`):

* dict 925 `details`: amount `min..max` by match level 1..5 = 3-9, 5-15, 7-21, 9-27, 10-30, `premium_factor` 0.5;
* dict 214 `generic_money`: 100-200 .. 500-1000 by match level (silver; NOT applied: the existing silver reward stays);
* dict 1872 `random_module_chest`: 1 per won match; dict 2176..2180 `rare_weapon_level_N` chests with chance
  5 % / 2.3 % / 0.7 % / 0.4 % / 0.04 % by match level, `ignore_loss_if_premium`;
* `max_drop_items_count` 16; event currency rows (951, 1138, 1139) need `service_needed` 23 (an event): applied by `events.md` while an event runs.

Modules therefore come from the chest (`random_module_chest`), whose opening (`46 06`) belongs to the chest work;
`modules_drop_options` (rarity and modifier pair tables) describes what that opening rolls.

Implemented (`crates/game-data/src/rewards.rs`, `servers/lobby-server/src/play.rs`): after a local match with a
won/lost marker (a leaver or a match without the marker gets nothing, official 2022-01-27) the lobby rolls the table at
match level 1 (the level the options blob carries, options.rs), credits spare parts and adds the chests, answers
`83 46 00 flag 1` (details record + chest records) and 8B. CHOICES: amount uniform in `min..=max`; premium account:
`amount * (1 + premium_factor)` (LIKELY, the AS premium popup says +50 % details). Official facts used: "the more frags
you get, the more spare parts" (event text 2021-10-15, an extra part per kill only during that event: not applied; and
the client reports no kills in local matches anyway, the 97 numbers are synthetic).

NOT applied: the official short-match multiplier (0.62b: <5 min x3, 5-10 x2, 10-13 x1.5): it is not in the 0.69d game
data or exe (`reward_factor` at 0x987260 is read at 0x2E0843 from another config, not a match table); `drop_card_free`
(its `prob` array is indexed by something the data does not name).

## 4. Other changes

* The n4 module elements of a weapon record carry the installed module's modifiers (`module_modifiers`,
  `module_rarity`, up to 4 pairs), so the weapon shows the merged rarity.
* A bought module gets the client's own list (premium pairs, rarity gold, override pairs), every module
  record adds its type's `override_modifiers`, and the match options carry the pairs too (without them a 40-round
  magazine holds 30 in the match). Details and the exe
  addresses: `match-modifiers.md` section 2.

## 5. Workshop save `60 01` (modules)

Rules of `servers/lobby-server/src/workshop.rs` (exe `process_attach_module` 0x200287480). Actions run in order, the state is updated after each; a refused action is skipped
and logged (`60 01` has no refusal reply, the client re-reads the records from `83 45 0F`).

* Slot set of the weapon (`ModuleData::slot_set`): `weapons.<cfg>.slots` plus, recursively, the `slots_to_add` of every
  module in the weapon (installed, or a default still in its slot), read from the attach_params block that the
  module's `slots_to_attach_to` entry names for its slot (`exceptions[<weapon cfg>]` first); `accept_module` = the
  weapon slot's value, true for an added slot unless it says false, then `slots_to_override` may change it. Data:
  `module_attach`, `weapon_slots_accepting`, `module_detach_price` of `game_tables_069d.json`.
* ATTACH 0: the slot is in the set and accepts modules; the module was bought in this save (buy uid = the client's
  counter) or is owned and free or on this weapon, not bound to another weapon, attaches to the slot by its
  `slots_to_attach_to`, and is a default or a `module_to_item_links` module of the weapon. uid 0xFFFFFFFF = the
  weapon's default for the slot back (never an inventory instance). Side effects (0x200283040): the old module of the
  slot to the inventory (a default: marked removed); the modules in the new module's `slots_to_disable` slots (any
  `clear_only`) to the inventory unless the target slot hangs from them.
* DETACH 1 (no slot on the wire): the module leaves its slot; modules in slots it added go to the inventory (their
  slots are gone). `f24 = 1` destroys the module (record with count 0), `f24 = 0` keeps it. uid 0xFFFFFFFF = the default
  of that dict: the slot is marked "default removed".
* "Default removed" mark (`Profile::removed_defaults`): the records (89, `83 45 0F`, 87 builds) and the local-match
  options give the weapon no default n4 element for a marked slot. Any module leaving a slot that has a default marks
  it (the client shows the slot empty after a DETACH); ATTACH of the default clears it. Stored without a schema change
  as an `attachments` row with module uid 0 (`accounts::DEFAULT_REMOVED`; a slot holds a module or a mark).
* BIND 6: binds the module to this weapon (bound = weapon dict id) for free; refused for a module bound to another
  weapon. Its byte `f2C` is the action id of the buy that created the module, ignored. CHANGE_MODULE_BOUND 7: binds to
  this weapon for `detach_price_in_gold` of the game data (`modules_drop_options.modules_additional_infos`: 0 / 50 /
  100), refused when gold is short; `f24` = old binding, logged only. CHOICE: no charge when the module is not bound to
  another weapon.
* BUY 3 (gold, 8A `c`) / 4 (spare parts, 8A `a`): refused for a price of 0 or short money; a spare-parts buy is also
  refused when weapons + armour + modules held reach 89 `max_items` (the client's own "inventory full" rule).
* NOT charged, OPEN: the `+0x2A` cost of a DETACH that keeps the module (= `detach_price_in_gold` for child detaches
  and after the dialog toggle); the dialog shows 0 and its totals ignore it. Logged.
* B3 lists the weapon's own slots only (added slots need no B3 entry).

## 6. Where modules are sold

One sale path, served by section 5; the client has no module category anywhere else.

* PROVEN (AS `ModuleSelector`): the module picker of a weapon slot has the tabs `unbound_modules` ("Unbound"), `shop`
  ("Premium") and `bound_modules` ("Bound") and the craft toggle `craft_module_free`. The `shop` tab lists the unowned
  proposals (gold entry, 8A `c`); the craft toggle lists their spare-parts clones (8A `a`). When the slot has no owned
  unbound module (`show_drop_info`), the Unbound tab shows the `modules_drop_info` text ("... or buy them in the shop")
  and the shop proposals with their gold price. Buying sends BUY 3 / 4, BIND 6, ATTACH 0 in `60 01`.
* PROVEN negative: `Category.BASE_MODULE` (categories 41-53) is not referenced by the item shop (`ItemShop`,
  `ShopListBase`, `ShopListSimple`, `FactionItems`) or the gold store (`StoreScreen`, `StoreEquipment`; built-in tabs gold /
  bundles / premium / chests / recommended). Only a store action card can show a module as card content.
* OFFICIAL 0.51 (2017-12-29): "Modules with average characteristics are available for purchase in the Workshop when you
  modify weapons"; 0.60a (2019-09-19): modules are created for spare parts with "Create" in the workshop. A module's gold
  price: NOT FOUND (`c = a / 20` stays, section 2).

## 7. Module rarity and drop modifiers

Code: `crates/game-data/src/module_drops.rs`; data `data/module_drops_069d.json` (`crates/data-extract`,
`modules_drop_options` of game.options).

### What a module record carries

A module's record (89 / 87 / `83 45 0F`, n1 list) and the n4 element of its weapon carry the same pairs (at most 4):
`(312, rarity)`, `(first_drop_modifier, grade)`, `(second_drop_modifier, grade)`, then the type's `override_modifiers`
(`ModuleData::instance_mods`, match side: `match-modifiers.md`). The client reads the rarity from the 312 pair
(0x200249A00) and shows the other pairs as stat lines through the `modifications` table (grade 0..10, 10 the best:
e.g. group 403 aim time penalty 0.10 at grade 0, 0 at grade 10). Drop groups: 300..330 and 400..417 (206 module types
have a first, 156 a second drop group; 216 have none).

### The tables (PROVEN data; the client parses them, 0x2002DB560..0x2002DBEDB, and rolls nothing)

* Five 11 x 11 tables indexed `[first grade][second grade]` (0x2001BD0C0 reads the first group's bounds from the rows,
  the second's from the columns). `modules_rarity` gives the table value of a grade pair, -1 = none.
* Table value -> rarity enum: 0 green, 1 blue, 2 orange, 3 legendary. PROVEN: the upgrade panel's
  `fill_module_next_grade_max_values` (0x2001BCF60, on opening the panel and after `83 45 10`) maps the module's 312
  grade 0 / 1 -> table 1, 2 -> 2, 3 / 4 -> 3, 5 -> nothing (jump table 0x2001BD0A0) and shows that value's grade
  bounds as `next_min_value` / `next_max_value` = the rarity after the merge; the 0.53 notes (2018-08-03) list
  "Factory-made - green (default)", industrial blue, military orange, "Special - gold", "Mastercraft - purple ... the
  maximum positive and minimum (or nonexistent) negative modifiers" = table value 3, the single cell (10, 10); the
  client draws legendary in purple (0x8759AC). Grey and gold have no cells: gold is the bought module, grey one
  without a rarity pair.
* Weight tables: `modules_mods_pair_prob` (general: values 0 / 1 / 2), `..._second_rarity_prob` (blue cells only),
  `..._pve_prob` (orange cells only), `..._fourth_rarity_prob` (only (10, 10)). Shares of the general table: two drop
  groups green 70.9 %, blue 24.4 %, orange 4.7 %; one drop group (client column rule below) 42.3 / 33.8 / 23.9 %.
* One drop group: the client takes the bounds of value v from the table's last column holding v (9 for green / blue /
  orange, 10 for legendary) and the rows of that column holding v (0x2002DBDA3..0x2002DBE81, used when the record has 2
  pairs). The server draws such modules from those cells only, so the grade lies inside what the client shows
  (LIKELY rule).
* `used_weapon_dropping_modules_percent` 65 / `used_weapon_pve_dropping_modules_percent` 100 decide WHICH module drops
  (one for a weapon used in the match); not applied: the chest picks among its 186 modules uniformly (OPEN).

### Sources

| source | rarity rule | status |
|---|---|---|
| PvP win chest `random_module_chest` (1872), opened after the match or by `46 06` | general table | 0.51 notes: "random modules for each PvP win. Higher quality modules are more rare" |
| workshop gold buy (60 01 action 3) | gold (4) + `premium_modifiers` (grade 7) + override | PROVEN client list 0x2002DFD30 (match-modifiers.md); 0.51 "modules with average characteristics are available for purchase"; 0.53 "Special - gold" |
| workshop "Create" for spare parts (action 4) | general table | 0.60 notes: "Rarity of the module will be determined randomly" (table: LIKELY). The client shows its own gold list until the save's records replace it |
| `rare_modules_chest`, `_1`..`_4` (faction battle top rewards) | blue table | LIKELY ("rare" = the middle of "common, rare, legendary", 0.51 preview) |
| treasure hunt module entries | blue table; the `AA` page entry carries the pairs (`u8 m <= 4`, buffer 0x200181080) and the found module gets the same ones (seeded by the page, not stored) | carrier PROVEN (0x2002477E0); table LIKELY (module entries have `rare_level` 1; 0.51b "high quality modules (rare) ... in the Treasure Hunt") |
| PvE (team mission) drops | orange table (`Source::Pve`) | no PvE module drop exists yet (the 0.69d PvE chest tables hold no modules) |
| merge to green / blue / orange / legendary | drop grades redrawn: `pair` cells of green (CHOICE, grey modules only) / `second` / `pve` / `fourth` | direction PROVEN (panel bounds above), table LIKELY |
| starter items of a new account (modules 1703, 1768, 1771, 1772, 1867), rolled when the account is created (`progress::new_player`) | green `(312, 1)` + drop grades from the green cells of the general table (`ModuleDrops::green`; one-modifier modules from column 9) | LIKELY: the client has no state for an owned module without a rarity (no `st_rarity_0` name, no inventory filter entry, merge price index -1); the 0.53 notes call green "Factory-made" the default; random grades rather than fixed ones (no 0.69d source fixes them) |
| module seed of an old version-0 profile without modules (`login::seed_modules`) | green, as the kit | same as the kit |
| other grants (store bundles, quests, calendar, events, achievements, match drops, a shop buy) | green, as the kit (`chests::give_rarity`) | none of their 0.69d data lists a module |

No path creates a grey module: every new module gets a rarity pair. The panel price follows the rarity
(section 1): grey 0, green 10, blue 25, orange and gold 50 gold per merge. A grey module (no rarity pair) attaches,
shows no stats and merges to green for 0 gold.

## 8. Modifier reroll 45 05 / 45 06 and the upgrade pick 45 08

The window is the lobby movie's `RerollItem`; the server code is `workshop::reroll` / `workshop::reroll_pick`.

* `45 05 u32 item, u16 group, u16 grade, u8 pay` (0xFF gold, 0 spare parts): the reroll of one modifier. The price is
  the item's `premium_to_reroll` gold or `details_to_reroll` spare parts, charged here. Answer `83 45 05 u32 item,
  3 x (u16 group, u16 grade)` and 8B. Refused (unknown item or modifier, not enough money): the old pair three times
  and no stored offer, so the pick keeps the modifier.
* How the client shows the three rows (PROVEN): the reader (0x200190918) passes the rows in wire order to the list
  builder 0x2002477E0, which does not sort and only skips a row of the module-rarity group 312; `updateReroll` and
  `setRerollOptions` keep that order. Row 0 is a normal ("choose") row. When the reroll was paid with spare parts, rows
  1 and 2 are "premium" rows that cost gold; when it was paid with gold every row is "choose". The premium price the
  window shows is `premium_cost` = `premium_to_reroll` x a sale factor + 0.5, truncated (0x20028CF76..0x20028CFD8); the
  factor is 1.0 because this server sends no sale record, so the price is `premium_to_reroll`. There is no "keep" row:
  "keep" is a separate button that sends `45 12` (`modifier_choose_reject`).
* `45 06 u32 item, u16 old group, u16 old grade, u32 row, u32 gold` (PROVEN writer 0x20028D87C..0x20028D9DF, AS
  `RerollItem.doReroll`): `row` is the list row the player clicked, `gold` the premium price the client showed (0 for a
  "choose" row). The server maps the row to the stored offer, charges a premium row with its OWN price (never the
  client's number) and refuses it without enough gold. A row outside the list, a different old pair or no stored offer
  keeps the modifier. The answer is always `83 45 06 u32 item, old pair, new pair` (old = new when kept: the reader
  0x200190A9D only overwrites a pair, then refreshes the item and the reroll window), then 8B when gold was charged.
  Log line: "45 06 item I modifier (g, r) row R (client gold G, offer stored S): changed / kept ... N gold charged".
* `45 08 u32 item, u32 index` (the upgrade pick): without a stored offer, or for an item the profile no longer holds,
  the answer is `83 45 07` with result 2: the client's `fillCraftedItem(-2)` shows "cannot_craft_item" and returns the
  window to its default state (reader 0x200190E64..0x200190E9A). An `83 45 08` would always change the item's list
  (its reader appends the new pair when the old one is missing).
* `46 00` (a shop buy) when the profile cannot be loaded gets the usual refusal `83 46 00 00 00` too, so the shop unlocks.
