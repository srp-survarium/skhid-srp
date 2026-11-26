# Premium store, premium account, vouchers and boosters (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Static research plus Rust. Nothing was run in the game. PROVEN = read in exe / AS / game data; LIKELY = inferred;
OPEN = not established, so not implemented.

## 1. How the client shows premium

* Premium status reaches the client in push 94, record kind 0: `f38` = seconds of premium left (PROVEN: the builder
  0x2001C56B0 switches on `[rec+0x24]` (jump table 0x1C5EB4 / bytes 0x1C5EC8); kind 0 stores `[rec+8]`, sets the
  premium flag, and 0x2001C5E2E calls `LobbyMenu.set_premium_left(seconds, st_hour_short, st_hours, st_days)` via
  0x144C40; AS `set_premium_left`/`getPremiumLabel` use `secondsPerHour` 3600 / `secondsPerDay` 86400). Without a kind-0
  record there is no premium. Kinds 1..7 / 0x29 = service flags (`root.show_all_presets_service {service_dict_id,
  actionTime}`), kinds 0x18 and 0x23 = active boosters (`root.set_active_boosters`, fields dictId / counter / cost /
  show_details / autobuy): record 0x23 decoded, see section 4.
* Server: `crates/accounts` schema v5 table `premium (account_id, until)`, `Profile::premium_until`
  (`premium_left(now)`, `extend_premium`); `servers/lobby-server/src/premium.rs` builds the 94 at login and after a
  change.
* Effect: the client's premium popup (AS `PremiumPopup.configUI`) lists "+50 %" for details (spare parts), silver
  ("revenue"), reputation, experience: PROVEN text. Server: `premium::MATCH_BONUS_PERCENT` = 50 on a paid match's
  experience, silver and reputation (`boosters::reward_percent`, section 4, shown in 97 as paid) and the spare-parts factor
  of the drop table (`premium_factor` 0.5). Official: premium also unlocks all 3 player profiles (0.63) and gives +25 %
  event currency (0.62b, 0.66c, 0.69b; the drop table's event rows carry factor 0.25 = the same): profiles in profiles.md;
  the +25 % on event currency is applied by `events.md` section 3.1.

## 2. Premium vouchers (implemented)

* `premium_access_voucher_1d / 2d / 3d / 7d` (dict 213 / 980 / 975 / 1021): game data `voucher_type` 3,
  `ttl_in_hours` 24 / 48 / 72 / 168, `cost` 50 / 100 / 150 / 200 gold (`currency` 1). PROVEN data
  (`data/vouchers_069d.json`, `crates/data-extract`).
* Activation: `45 03 item_use` (C++ 0x24BD10, `u32 item`; the handler reads the item's `voucher_type`). Server
  (`premium::use_voucher`): extends the expiry by the voucher's hours (from the running expiry, else from now),
  consumes one, answers `83 45 03 <item>`, pushes 94 and 8B. Official 0.69d: the 1 day voucher no longer activates
  automatically on receiving, so the player uses it by hand. Other `45 03` uses (consumables) are NOT touched: the
  call returns false and the dispatcher continues.
* Buying: through the ordinary item shop (`46 00`): the vouchers are in the 8A table with the dictionary price
  (kind 1 = gold). Whether the item shop lists category 27 rows is not proven (the premium store is the real place).
* Premium time is also sold directly: the premium window's four offers (`46 02`) and the
  7-day actions-tab card (`5C 01`) add their hours the same way a voucher does (from the running expiry, else from now);
  see "Store offers and pictures" below.

## 3. The gold store and premium offers: layouts and default offers (see "Store offers and pictures")

Exe (PROVEN layouts and consumers, reader addresses are RVAs + 0x200000000):

* Push 95 (reader 0x20018AA80): `u32 n` elements, each `u32 e0, u32 e4, u8, u8, u32 eC, u8 (+0x488), u8 m, m x 44-byte
  sub-records (m <= 20)`, then `u8 n2` tabs `{u32, str(64), u8}`. The elements (consumer loop 0x2001440A0..0x2001444E4) are premium-time offers
  and bundles, and `e4` is a `shop_bundles` BUNDLE id looked up with `bundle_item_by_id` 0x2002DFF60; see 3.1 and "Store
  offers and pictures".
* Premium store (0x200143F30 chunk): `root.set_premium_store` is built from the 94 triples `M x {u32, u32, u32}` (+0x7A3400)
  and the K list (+0x7A3418) matched against 94 records of type 8; `root.setup_bundle_data` reads, per bundle id of the
  dictionary (`shop_bundles`, 14 rows), `ui_descr.text_descriptions.name / description`, `parameters.faction.faction_id`,
  `item_level`/`faction_level`, `cost`, `currency_type`, `icon_row`, `icon_column`, `props_list {prop_id, prop_value}`,
  `bundle_properties`, `bundle_items {dict_id, amount}`. The mapping of every field: 3.1.
* Buy: `46 02 u32 offer id, u32 match id, f32 0.0` (0x187E90; the lobby's `accept_premium` 0x24B890 passes match id 0, the
  victory screen 0x290E20 its match id). No answer is read for it (`83 46` sub 2 has no case). Server
  (`premium::buy_offer`): gold taken, premium extended, 8B + 94; an unknown offer or too little gold changes nothing.
* push BC sub 0 / request 5C 00 (shop actions): decoded, see `actions.md` (tabs of 95, the tab blob, the buy answer
  `BC` sub 1, the keys of the UI call 0x20028E8A0). The offers are a server setting; the default is one tab with two cards.

### 3.1 Field meanings (PROVEN unless marked)

* 94 reader 0x200189F90: record `w0..w4, L1, L2, u8 type, u8 x`, then `u16`, `u32 M, M x {u32 a, u32 b, u32 c}` (vector +0x7A3400),
  `u32 K, K x u32` (vector +0x7A3418). Only the premium builder 0x200143F30 reads the two vectors.
* Triple `{a, b, c}`: `c` = discount in percent (shown "-c%"); `b` = the offer it applies to (a 95 element id for premium time, or
  a value of the K list for the gold discount); `a` = id of a discount action that must exist as a 94 record of type 8 whose `w3`
  = `a`; that record's `w1` = seconds left (the Flash timers count seconds). The largest `c` wins:
  `root.set_gold_discount(c, secs)` (the gold-info window shows "-c%" and counts down) or `root.set_premium_discount(c, secs)`
  (only a countdown on the premium button: `PremiumPopup` never reads the percentage). A type-8 94 record does nothing by itself.
* K list: the ids that count as gold offers for the gold discount (PROVEN as a set; what they stood for, e.g. web-shop gold packs: LIKELY).
* 95 element (reader 0x2001805A0): `+0` id, `+4` `shop_bundles` bundle id (name / description / icon; not
  an item id: an unknown bundle id asserts in the client), `+8` not read by these builders,
  `+9` 0 = premium-time offer / 1 = bundle, `+0xC` gold price, `+0x488` money type; sub-records (44 bytes on the wire, s0..s12).
  Premium offer -> `root.set_premium_store {id, value = +0xC, money_type, label / value2 = name / description, discount}` with
  `discount = trunc((full - value) / full * 100)`, `full = sum of trunc(s5 / s7 * s1)` over sub-records with `s11 == 1`.
  Bundle -> `root.setup_bundle_data {bundle_id, name, descr, faction, cost = +0xC, currency_type = +0x488, icon, props,
  bundle_items}`, where `bundle_items` = `{dict_id = s0, amount = s4}` of the sub-records with `s12 == 0` (from the 95, NOT
  from the dictionary). LIKELY: `s5` = premium hours (the level-up premium proposal 0x2001C1F90 compares it with days x 24),
  `s1 / s7` = price per `s7` hours, `s12 == 1` with `s0 == 0` = a premium-time line.
* Our server: `Notices94` / `Shop95` encode these layouts. The 95 carries four premium elements (`+9 = 0`)
  and `46 02` is handled (see "Store offers and pictures"); no triple, K list or type-8 record is sent (no discount runs).

Game data searched for a price / duration / bundle-content table: every `.options` / `.cfg` / `.lua` file that is not a
texture or a game-mode file of volume 00 .. 75 (212 listed, none with price rows besides `game.options`), `game.options`
of volumes 00 .. 75 (keys `cost`, `currency`, `premium_to_craft`, `detach_price_in_gold`, `shop_bundles`,
`exchange_currency_rates`, `buy_specific_cost`, `buy_preset_cost`, `pve_default_prices`), the 14 `gameplay/bundles/*.options`
(only `faction_data` and `ui_descr`), `event_exchange_action_items.options` (event currency exchange: 2500 / 1500 / 750 ->
1 item, no gold), `booster_parameters.options` (silver per match, section 4), `lobby_menu.options`, `static_game_parameters.options`.
RESULT: no price, duration or content table for premium time / bundles / gold packs exists in the client data. They were
real-server data. What a server must fill for a real store (nothing is invented here): per offer the 95 element fields
above (id, item/bundle dict id, discount value, money type, bundle items with amounts), the 94 triples / K list /
type-8 records for the premium store tab, and the BC sub 0 blob. The default offers take the 0.6x
web-shop prices (see "Store offers and pictures"). A wrong 95 element crashes the lobby (B5/B6 and the premium builder look
its `+4` up as a bundle id).

The premium vouchers (section 2) are bought in the ordinary item shop for the dictionary gold price.

## 4. Boosters (`servers/lobby-server/src/boosters.rs`)

Data (`data/vouchers_069d.json`, resolved from `game.options` `modifications` by group / grade):

| dict | item | kind | modifier (value) | price |
|---|---|---|---|---|
| 339 | weight_booster | per match (`use_variable_costs`) | stamina_health_increase_modifier +0.25 | `booster_use_costs` silver |
| 340 | speed_booster | per match | max_movement_speed_modifier +0.05 | same |
| 341 | regeneration_booster | per match | health_regeneration_speed_modifier +0.02 | same |
| 342 | respawn_booster | per match | respawn_time_modifier -0.30 | same |
| 343 | aiming_booster | per match | breath_vibration_modifier -0.25 | same |
| 344 | artefact_booster | per match | artefact_passive_power_modifier +0.30 | same |
| 853 / 857 / 861 | experience / money / rep `_24h_50_voucher` | per match (gold slots) | experience / money / reputation bonus +0.5 | 5 gold (8A price, kind 1) |

* `booster_use_costs` (`gameplay/items/boosters/booster_parameters.options`, volume 00, PROVEN data,
  `data/booster_costs_069d.json`): 6 rows x 3: `[250,500,750] [250,500,750] [500,1000,1500] [750,1500,2250] [1000,2000,3000] [1500,3000,4500]`;
  row = equipment level (the client indexes by a lobby byte), column = nth booster used (0..2). An online match charges the row the client shows: the highest equipment level of the player and his squad mates (`online::booster_level`, online.md section 8); a local Training match charges none.
  `equipment_use_costs` = 0 x 6: the match cost by equipment level (index = the level), the "repairs" group (type 2, dict
  0xFFFF) of the pre-match cost (the client shows 0 from this table). The server charges after an online match by the
  setting `LobbyConfig::equipment_use_costs` (default the patch 0.55 prices) as "Match costs" (`charge.rs`, online.md 9).
* The client UI object per active booster (0x2001C5191..0x2001C5568, PROVEN): `{dictId, cost, counter, show_details,
  autobuy}`; cost = silver from the table for `use_variable_costs` items, else the item's own `cost`; no cost is shown while
  the 89 inventory holds the item (stock is used first); at most 3 boosters.
* 853 / 857 / 861 are per-match boosters, not 24 h vouchers. PROVEN: their descriptions say "for one match" (`st_experience_booster_24h_description`
  "Increases experience gain for one match", silver / reputation alike); the lobby panel (`survarium.boosters.BoosterPanel`)
  has three silver slots (339..344) and three gold slots (8A kind 1); `booster_participation_costs` (0x2001BF460) adds every
  selected booster without stock to "Match costs" (`root.set_match_participation_cost`, 0x2001BFBC0: type 1, its cost and
  currency; 853 = 5 gold), with `n` of `booster_use_costs[row][n]` = the variable boosters without stock before it.
* 94 record type 0x23 (PROVEN consumer): `f40` = booster dict id. The server sends one per selected booster
  (`boosters::records`, dict id order). Type 0x18 (service boosters; the record type IS the
  service id, `f40` = the item's `default_modifications` pair `group<<16 | grade`, `f44` = counter) is only used
  by `new_player_booster` 1088 (`service_id` 24, 3 matches): a new account gets it (section "Newbie's blessing" below).
* Requests (no answer in the client, PROVEN): `45 0C dict` enable, `45 0D dict` disable, `45 0E old new` replace, `48 29 dict,
  u8` autobuy. The panel sends 45 0D when a selected slot is clicked again (`handleSlotClick` / `handleBoosterSelected`) and
  redraws its slots only from `root.set_active_boosters` (`setActiveBoosters` also calls `enableUI`). Server: enable only
  selects (three per slot group, silver / gold), disable unselects any booster; every request, refused or not, is answered
  with the 94. An online match start charges each selected booster in dict id order (a local Training match charges none): stock first, else its price as the client
  shows it (variable: silver of the table column `n`; others: 8A price in 8A currency); unaffordable ones are switched off.
  The selection stays for the next match. State: schema v8 table `boosters (account_id, dict_id, enabled, until, autobuy)`
  (`until` is not used).
* Effect on rewards: `boosters::reward_percent` = premium +50 % plus +50 % of each 853 / 857 / 861 the match carried (online:
  the ones its start charged, `Running.boosters`; local: the selected ones) on experience / silver / reputation, added per
  component (LIKELY additive); their reward modifiers are not put into the match options; used by `servers/srp/src/matches.rs` through
  `local_match::results_with_bonuses`, shown in the 97.
* Match side (weight, speed, regeneration, respawn, aiming, artefact), implemented. The match options carry
  no booster field and no booster item slot (the profile has slots 0..12, none for category 23); the match reads ONLY
  the per-player modifier block of the 5B profile (18-byte bit mask over 138 ids + one f32 per set bit, reader
  0x200125680, PROVEN) which it stores at `player[slot * 0x2A08 + 0x100 + 4 * id]`; gameplay reads it through the
  evaluator 0x20031E440 / 0x20031E160 (`profile float + a list of runtime modifiers`; ids in the additive list
  0x20098D1F0 add, the others scale `max(0, 1 + sum)`). PROVEN consumers: id 68 weight 0x200309F6A, id 54 respawn
  0x2003E7547, id 55 breath (aiming) 0x2003DEFC6 / 0x2003DF00C. The server therefore does what it does for skills: the
  enabled booster's `default_modifications` value (table above) is ADDED to the human's skill sum per modifier id
  (`HumanLoadout::extra_modifiers`, `boosters::match_modifier_ids`: 68 +0.25, 122 +0.05, 0 +0.02, 54 -0.30, 55 -0.25,
  40 +0.30). That booster items have no other match-side handler (the `booster` class and `booster_participation_costs`
  are lobby objects) is LIKELY (no booster-specific read found in the match code), and so is that the real server
  filled the same block (it is the only path the reader offers). The block is the same in an online match's player
  record (same reader), so the online builder only has to pass the same map.
  `5B 01` (Training with bots) charges no booster (the mode pays nothing, so nothing is used
  up; the selection stays for the next online match); its options still carry the selected boosters' modifiers (CHOICE).
  The online start charges first and builds the options after, so an unaffordable booster (switched off) does not reach
  that match; the tutorial charges nothing and carries no booster. Bots get none.
  Test: `local_match::options::tests::booster_modifiers_fill_the_profile_block` (mask, floats, rest byte-equal to the
  options without boosters); with no booster the options are byte-equal to the goldens.
* To check in the game: the booster panel shows the selected boosters and "Match costs" their cost; a selected slot clicked
  again is unselected; the match start takes the silver / gold; the results screen shows +50 % more.
  In a Training match with boosters enabled: weight booster = "Maximum stamina +25 %" (PROVEN: dict 339 = modification
  group 163 grade 2, `stamina_health_increase_modifier` +0.25, UI prop 177 "Maximum stamina" in %; modifier 68 is added to the
  stamina maximum at 0x200309F6A; no kg anywhere, 0.69d has no carried-weight modifier), speed = about 5 % faster running, regeneration =
  faster health regeneration, respawn = 30 % shorter respawn time (modes with respawns), aiming = 25 % less breath
  sway when scoped, artefact = 30 % stronger passive artefacts. Compare a match with and without.

## 4b. Rename voucher and drop card

* Rename (`rename_account_voucher` 152, 250 gold, `voucher_type` 0): implemented in `servers/lobby-server/src/rename.rs`.
  Request `48 05 str nickname, u8 face, u32 voucher uid` (builder 0x200290BC0, last field = voucher uid LIKELY), answer
  `83 48 05 u8 ok [u32 uid, u8 face]` (reader 0x200192B68, PROVEN: the client lowers the stack of row `uid`), then 8F. Rules:
  owns the voucher, `accounts::Store::rename_account` sets only the NICKNAME (column `nick`, NULL = login name; sign-in keeps the login name; <= 30 chars = the client's box `NICKNAME_MAX_CHARS`, no control / edge space, unique among nicknames case-insensitively). Shown everywhere via the nickname (87, 8F, chat, friends, clans, squads, results).
  PROVEN (lobby movie + exe): the box takes 30 characters; about 1 s after typing stops the client sends
  `48 04 str, u32 3`; empty, swear-filtered (filter 0x20025AA20) and unchanged names are answered locally and never sent;
  no character filter runs (`removeSpecialChars` / `BANNED_CHARS` exist but are never called). Reason bytes of a refused
  `83 48 04` (`rename::reason`): 14 non-printable, 15 already reserved, 21 too short, 22 too long, 23 invalid
  delimiters, any other = empty text. `48 05` third field: 0xFFFFFFFF in the tutorial, a voucher uid, or 0 = "pay with gold"
  at the price 8D `costs[3]` (`root.set_change_name_cost`); a refusal (`ok 0`) makes the client restore the old face itself.
  CHOICE: the gold rename costs the rename voucher's dictionary price, 250 gold (`rename::gold_price`,
  sent as 8D `costs[3]`; no client data gives the real price). `48 05` with uid 0 after the first nickname: the gold is
  taken, the name checked (a refused name gives the gold back, answer `ok 0`), then `83 48 05 01, u32 0, face`, 8B, 8F.
* Drop card (`drop_card` 153, 100 gold, `voucher_type` 1) and `drop_card_free` 1132: the client's `buy_drop_card` Flash event
  (0x20019DA67) calls 0x187A40 with dict id 0x99 = 153 and a count: it is an ordinary item purchase (`46 00`), served
  by the item shop. What a card does is the treasure hunt (UI keys `max_free_drop_cards`, text
  `st_not_enough_drop_cards`, 0x200188DC9): items.md section 3a.

## 5. Prices of grenades, mines, traps, medkits and boosters (fan wiki, Items page)

Source: survarium.fandom.com "Items", Wayback snapshot 2022-05-27 (three months after 0.69d), tables 0-3, in
`data/shop_069d.tsv` (`crates/data-extract`). Fan data: LIKELY close to 0.69d, not checked in the client.

Conflict rule: where a faction shop page of the same month already gives a price, that price is kept
and the Items-page value is written into the tsv `note` column; the Items page only fills items without a price.

| item (dict id) | Items page | faction shop page | in the shop table |
|---|---|---|---|
| RGD-5 grenade (5) | 30 | none | 30 (new) |
| Smoke grenade RDG-M (219) | 50 | none | 50 (new) |
| PMN-3 mine (734) | 150 | none | 150 (new) |
| MON-50 mine (736) | 200 | none | 200 (new) |
| Pack of bandages (66, "Bandages") | 20 | none | 20 (new) |
| Medikit (67, Scavengers medkit) | 50 | "?" | 50 (new) |
| Anesthetic (65) | 50 | 25 | 25, Items value noted |
| Antitoxin (82) | 50 | 20 | 20, Items value noted |
| Military Medikit (759) | 150 | 40 | 40, Items value noted |
| Science Medikit (760, Fringe Settlers medkit) | 300 | 25 | 25, Items value noted |
| Trap (68) | 20 | 20 | 20 |
| RGO (733) | "?" | none | 40, our own price (see 5a) |

Faction columns of the Items page differ from the dictionary for the smoke grenade (wiki Scavengers), PMN-3 (wiki Army),
MON-50 (wiki Fringe) and Science Medikit (wiki Independent); noted in the tsv, not used. The Black Market medkit (1196)
has no Items-page row (priced in 5a).

### 5a. Own prices of the pocket consumables without a source (CHOICE)

The pocket popup builds a tile only for an item whose 8A record has `for_sale` = 1, and an item without a price is not
sold, so 8 of the client's 19 pocket consumables never appeared. No 0.69d data, client value or wiki page gives their
price, so they carry our own silver price. In `data/shop_069d.tsv` these rows have `sold` 1, `source` = `choice` and the
reasoning in `note`. `ShopTable::is_choice` marks them, and they load like any other row. An operator changes a price in
the shop tsv of the `data_dir` (no switch). The silver price is the 8A `a`: the shop buy, the after-match pocket charge
("Special items", `charge.rs` through `ShopTable::pocket_cost`) and the sell base `a / 4` use it. The gold unlock (`46 09`)
does not depend on it; that price comes from `buy_specific_cost` by item level.

Comparison set (sourced prices): RGD-5 30 (level 1), smoke grenade 50 (1), trap 20 (2), Scavengers medkit 50 (1),
Renaissance Army medkit 40 (3), Fringe Settlers medkit 25 (2), anesthetic 25 (2), bandages 20 (2), antitoxin 20 (2), PMN-3
150 (3), MON-50 200 (4). Effect values are from `game.options` (volume 75).

| item (dict) | faction, level | silver | reasoning |
|---|---|---|---|
| RGO grenade (733) | Renaissance Army, 2 | 40 | impact fuse, but less damage than RGD-5 (150 vs 170, radius 5 vs 6); one level above RGD-5 30 |
| Anomaly Capacitor (1089) | Scavengers, 2 | 50 | utility throwable, like the smoke grenade (50) |
| Ammo crate (1091) | Renaissance Army, 2 | 50 | devices at 25 x level from the smoke anchor; team resupply 50 % for 30 s |
| Throwing Sensor (1101) | Scavengers, 3 | 75 | devices at 25 x level; 10 m scan for 10 s |
| Stationary Sensor (1090) | Renaissance Army, 4 | 100 | devices at 25 x level; 15 m scan for 30 s |
| Defense totem (1106) | Fringe Settlers, 4 | 100 | devices at 25 x level; 4 m regeneration aura for 30 s |
| C-4 (1133) | Renaissance Army, 4 | 250 | mines: PMN-3 150 (225 damage), MON-50 200 (200 damage); C-4 deals 300 and is remote-detonated |
| Black Market Medkit (1196) | Black Market, 3 | 40 | same rank as the Renaissance Army medkit (40): faster (0.3 s, 1.25 s), heals 40 % + 25 % stamina vs 45 % |
| Flash grenade (2072) | Scavengers, 1 | 40 | between RGD-5 30 and smoke 50 |

The flash grenade is not one of the 19 of the client's specifics table, so `46 09` cannot open it. It is level 1, so
every player has it open by faction level. Nut (1355) and Snowball (1884) are also outside the 19 and stay unlisted: they
are event specials. Listing one would need only a `choice` row like the ones above. Both would then open by faction level
alone (Nut level 1 = always; Snowball Renaissance Army level 2).

Unlisted pocket items cost nothing (CHOICE): `ShopTable::pocket_cost` charges 0 for an item whose
8A record is not `for_sale` (Nut and Snowball from an event drop, or any item without a 0.69d, wiki or `choice` price), so
using one in a match adds no "Special items" entry. Their dictionary default price (20 x level) is only a placeholder that
the client never shows. A 45 00 pocket move of such an item places only the units the player owns: missing units are not
bought (the popup has no tile for it, and the 46 00 buy refuses an unlisted item too).

A locked pocket item (faction level below the item's level and not opened with `46 09`) follows the same owned-units rule: a 45 00
move places the units the player owns and buys none, and the popup (source 200) places none (actions.md section 6). Once the
item is open, missing units are bought at the 8A silver price as usual.

Boosters (Anomalous Force Protein, Artifact-master, Motherwort extract, Radio, Rations, Tornado Energy Drink): the
page says "3.000" each (LIKELY 3000 silver, dot as thousands separator); the client dictionary `cost` is 500 (currency 0)
and stays the price (game data first). The wiki value is only noted in the tsv.

## 6. Gold prices elsewhere

See workshop.md section 2: module gold price = spare-parts price / 20 (CHOICE, no source), vouchers /
boosters priced from the dictionary,
items without a real price are not sold. The gold unlock price of a locked faction item (8A `c`) has no source: 0, not
offered (items.md, "Buy a locked faction item").

## Web evidence: store, contracts, clans, leagues

Labels OFFICIAL = developer post, FAN = fan site, NOT FOUND:
* Premium: lifetime premium "technically for 10 years", bonuses +50 % each (OFFICIAL 2019-06-20, gid 2415533798132485926);
  a web-shop bundle "2500 Gold and 6 months of Premium" (OFFICIAL 2020-04-24, no price); premium from 24 hours to 1 year (FAN);
  1 year = 5000 gold (FAN, old forum). The 30-day premium price: NOT FOUND.
* Gold packs (FAN, undated snapshot of the official shop): 100 / 315 / 540 / 1100 / 2300 / 3500 / 6000 / 12500 gold for
  $0.99 / 2.99 / 4.99 / 9.99 / 19.99 / 29.99 / 49.99 / 99.99; "A Month of Premium Weekends" $5.99. Real-money packs are
  not a server function here.
* Gold prices the developer stated (OFFICIAL): premium ammo 10 gold, the 7th daily login 20 gold, Team Missions invite /
  slot / respawn 50 gold, Treasure Hunt change 100 gold (both match the data), the Executioner set about 6050 gold.
* Discounts: weekend sales 20-70 %, "Personal Offers" (2019), web-shop weapon lotteries, faction-event price cuts (OFFICIAL).
  The favourite-weapon offer and any in-game action card contents: NOT FOUND, so the offers stay empty (operator setting).
* Silver for gold: 0.47d tiered 200..800 per gold (OFFICIAL 2017) CONFLICTS with the 0.69d data (250 flat): the data wins.
* Shutdown: from April 2022 the stores were off and every player received 50 000 gold and 500 000 silver (OFFICIAL
  2022-02-07). Not implemented as a starting grant.

## Web round 2

Premium prices [0.6x, OFFICIAL web shop text, Wayback 2021-05-07]: 3 days 150, 7 days 300, 14 days 500, 30 days 900, 90 days
2500 gold. The 7-day value conflicts with the client data's voucher (200 gold): the data wins for the vouchers. These
prices are the premium-window offers and the 7-day card ("Store offers and pictures").

## Store offers and pictures

PROVEN: the 95 element's +4 is a BUNDLE id of `game.options` `shop_bundles`,
not an item id; the premium bundles are 7 = 24 hours, 1 = 3 days, 2 = Week, 3 = Month, 4 = 3 months, 5 = 6 months, 6 = Year
(the label comes from the bundle). The voucher item ids 213 / 980 / 975 / 1021 there make the lookup assert (crash). The
premium window shows per offer only the label, the gold cost, a "saved" percentage and a tooltip: NO picture (the built
object has no image field). It sorts offers by id and preselects the 4th. Each offer needs one sub-record with the hours, or
the "saved" percentage divides by zero (base = the 1-day voucher price: 7 days -14 %, 30 days -40 %, 90 days -44 %).
14 days has no bundle label: it cannot be a premium-window offer.

### The default offers

Code: `servers/lobby-server/src/premium.rs` (`PremiumOffer`, `shop_elem`, `buy_offer`, `buy_card`), `actions.rs` (`default_tabs`,
`default_offers`, the tab blob), `contract.rs` (the Winner's Contract), `online.rs` (`pay` calls `contract::on_won_match`),
`crates/accounts` schema v20 table `contracts (account_id, started, won_mask)`. Settings: `LobbyConfig::premium_offers`,
`store_tabs`, `store_offers` (defaults below; an empty list removes them). Tests: `servers/lobby-server/tests/store_offers.rs`,
`tests/online.rs` (`the_winners_contract_pays_the_first_win_of_each_day`), `contract.rs` unit tests.

* Premium window (95 elements, `+9 = 0`, `+0x488 = 1`, one sub-record `{s0 0, s1 50, s5 hours, s7 24, s11 1, s12 1}`):

  | element id | bundle (`+4`) | label (client text of the bundle) | gold (`+0xC`) | hours | "saved" shown |
  |---|---|---|---|---|---|
  | 1 | 1 | 3 days | 150 | 72 | none (0 %) |
  | 2 | 2 | Week | 300 | 168 | 14 % |
  | 3 | 3 | Month | 900 | 720 | 40 % |
  | 4 | 4 | 3 months | 2500 | 2160 | 44 % |

  Prices: the 0.6x web shop (OFFICIAL, Wayback 2021-05-07). Element id = bundle id: CHOICE. The client preselects the 4th.
  `46 02 id, match id, f32` takes the gold and adds the hours (from the running expiry, else from now), then 8B and 94; an
  unknown id (e.g. a voucher item id) or too little gold changes nothing and sends nothing. Then the client's own
  "premium bought" notice `B1 0x14 [1][days]` (0x2001C3EA3, section "Premium notice" below).
* Actions tab: 95 tab `{id 1, "st_special_offer", default_gold 1}`, sent as its English text (CHOICE: name and id; server tabs replace the client's five
  built-in tabs, `StoreScreen.setTabData`). Its blob has two type-0 cards (all mandatory keys, `actions.md`):
  item 0 = card 1, 7 days of premium, 300 gold (the window's week price, 0.6x web shop), pictures
  `673_s/m/l_1563455165.png` (crown; item 0 uses `l_img`), `5C 01 1 01` adds 168 h of premium (the same as a voucher),
  answers `BC` sub 1, 8B and 94; item 1 = card 2, the Winner's Contract, 100 gold, picture `image_10.png` in all three
  keys (item 1 uses `s_img`), see below. Texts: the client's own (section "Client texts in the store").
* Winner's Contract (`contract.rs`): `5C 01 2 01` takes 100 gold and starts a contract (refused, result 0, while one runs or
  with less gold). 3 days of 24 h from the purchase (CHOICE: not calendar days). Every `94` while it runs carries record type
  0x16 WITH ONE BYTE IN ITS SECOND BUFFER (PROVEN: the client's status loop skips a default-case record whose two buffers
  are both empty before it reads the type, 0x2001C5A50..0x2001C5A5A; then `root.set_contract_data` never runs and
  `set_contract_exist(false)` hides the button) and the tail (its blob is a RAW LZMA2 STREAM of the config: `set_contract_data`
  decodes it with the 8A decoder; a plain config crashes the client at sign-in) `C = 3` statuses (1 = the day's win was paid), A = minutes left of the contract (rounded up), B =
  0xFFFFFFFF while today's win is open, else the minutes until the next day (on the last day: until the end), blob
  `{amount 20000, actionType 1, dictId 214}`. The first WON online PvP match of a contract day (random, ranked, clan battle;
  not local, custom training or PvE) pays 20 000 silver, sets the day's status and sends 8B, the `94` and `B1` type 0x16
  `[20000][0]` (in that order: the notice opens the contract window from the latest data). A second win the same day, a
  loss or a missed day pays nothing; a missed day stays 0 (LIKELY that the original also used 0; the client counts only
  status-1 days). After the 3 days no record is sent (the button disappears with the next `94`; the client asks `39 00` when
  its countdown ends) and a new contract may be bought. Premium does not raise the 20 000 (CHOICE).

## Premium notice and new-player booster

* `B1 0x14 [1][n]` (PROVEN): `n` is a number of DAYS, printed by a plain vsnprintf into `st_premium_bought_for_n_days`
  ("Days of Premium: %d! ", one signed-int placeholder). The client shows a non-clickable MESSAGE notification (Flash type 10,
  icon 2), logs marker 0x17 and acks with `51`; premium state is not touched (8B / 94 do that). Server: after 8B / 94 of a
  premium purchase it sends `[1][hours / 24]` (3 / 7 / 30 / 90 for the window offers, 7 for the 168 h card). OPEN
  whether the original also sent it for vouchers / gifts (no client evidence possible).
* 94 type 0x18 / `new_player_booster` 1088 (PROVEN): the client looks the item up by category 27, `voucher_type` 4,
  `service_id` == the record type (24), and a `default_modifications` pair == `f40`, so `f40` = `140 << 16 | 0` = 0x008C0000
  (sending 24 resolves nothing, shows nothing). `f44` = the shown counter (LIKELY matches left; `service_duration_matches` 3).
  Texts "Newbie's blessing" / "Protection from anomalies". In a match, modifier 76 (`new_player_damage_modifier` -0.15) calls
  `root.show_bless_info(..., 1088)` and reduces damage of hit types 4 / 5 / 7, scaled by options record +0x2D8 (we send 0).
  No grant in 4 807 decoded configs, the exe or the movie (NOT FOUND). A grant needs the 94 record above, item 1088 in the 89
  inventory (LIKELY needed by the booster panel), modifier 76 in the options modifier block. OPEN: +0x2D8 meaning, hit type
  names, panel without the inventory item.

## Premium notice and Newbie's blessing: server rules

* `B1 0x14 [1][days]` is sent after every premium purchase (window offers, the 7-day card;
  `premium::bought_text`). The text is the client's own localized `st_premium_bought_for_n_days`; the server sends only the
  number. Not sent for premium vouchers (CHOICE; the voucher use shows through 94).
* Newbie's blessing (CHOICE): a new account gets `new_player_booster` 1088 x 3 (`service_duration_matches`)
  in its inventory (`progress::new_player`); the 94 carries type 24, `f40` 0x008C0000, `f44` = units left; every online match
  except PvE uses one unit when the match server accepted it (`boosters::use_services`) and carries modifier 76 = -0.15 in the
  player's options block (`boosters::service_modifier_ids`). Local and PvE matches neither carry nor use it. To check in the game: the booster panel and the in-match blessing info.

## Client texts in the store

* The store texts are the client's own. The client shows card `label` / `description` and `95` tab names
  raw, so the server sends the VALUE of a client key in the player's language: `5C 00` byte 6 = the client's language
  index (1 russian, 2 english, 3 german, 4 polish, 5 ukrainian, 6 french, 7 spanish, 8 chinese, 9 portuguese, 10 turkish,
  0 other -> English; 0x2000915E0), values from the client's 11 language files (`data/store_texts_069d.json`,
  `game_data::texts`, made by `crates/data-extract`). A label that is not one of those keys (an operator's own
  text) is sent as it is.
* The cards: 7 days of premium for 300 gold (no client text names 14 days), label
  `st_premium_access_7d_voucher_league_desc` "Premium account for 7 days", description `st_gold_advert_1_hint`; the
  contract (100 gold, same rules) under the client's own name of its window `st_contract_with_profiteer` "Contract with
  Huckster", description `st_contract_complete_after_win_descr` "Win 1 match". The tab: `st_special_offer` "Special
  offer" in English (95 comes at login, before any language is known; OPEN: a language source for 95).

## Offer details and the running contract

PROVEN unless marked.
* The full view (`SrvShopFullRenderer` extends `ActionBigStoreRenderer`, opened by a card click with the card list hidden)
  has NO picture: its `addElement` has no `Background` case and its frame 1 covers only the left panel (placements x 0 ..
  ~600: texts, buttons, `setList`); the lobby room shows on the right by design. The "Contains" (`st_inside`) block is
  `setList` (title, `reward1..4` = `blockInsideSilverAll` = `RewardRendererBig`, an item tile list below). The exe fills it
  only from `bundle.items` (builder 0x20028E8A0, type 0: ui element `Content`, `list = [{dict_id, amount}]`); an empty bundle
  leaves the block empty.
* `ItemFactory._rewardsId = [214, 215, 925, 0]` -> silver / gold / spare parts / `Reward.PREMIUM`; `filterRewards` moves
  them to `reward1..4` (premium `amount` = hours, shown as `Util.getTimeLeftStringHours`, "7" + `st_days_short`, with the
  `premium` icon in gold); other dicts become item tiles. Server (`actions::shown_items`): the 7-day card lists `{0, 168}`,
  the contract `{214, 20000}` (its daily reward, the contract config's `amount` / `dictId`); operator items follow. These
  entries are display only for the premium and contract grants.
* A card cannot be shown as bought or disabled: the full view's buy buttons are always enabled outside the roulette-drop
  preset (`updateLayout` line 365), "PURCHASE COMPLETE" needs `complete`, which the builder never copies, and a refused
  `BC sub 1` runs an empty branch. So the contract card is left out of the tab while a contract runs
  (`actions::shown_to`); after a bought contract the server sends the tab again (`BC sub 0` has no request guard and
  redraws the open list; sending `5C 01` expired every cached tab, 0x200196F48). The client keeps a tab 300 s
  (0x2001967B5, f32 at 0x2009E60A4): after the contract ends the card is back with the next tab request, at most 5 min
  later. A second buy from a view drawn before the purchase is still refused silently (no client text exists).
