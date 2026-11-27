# The event: event currencies, the exchange window, drops (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Static research (exe, lobby movie, game data, official notes) plus Rust. Nothing was run against the real client. Tags: PROVEN =
read in the exe (address), the lobby movie's ActionScript (class / method) or a game file; LIKELY = inferred; CHOICE = a server rule
no evidence settles; OPEN = not established, not implemented. Addresses are VAs (image base 0x200000000). Related: `actions.md`
section 7 (the `48 28` request), `store.md` (94 records), `progression-audit.md` (Reward enum), `items.md` (chests).

Code: `crates/game-data/src/events.rs` (data and rules), `data/event_drops_069d.json` (`crates/data-extract`),
`servers/lobby-server/src/events.rs` (record, drops, exchange), `crates/accounts` (schema v17 `event_wallet`). Tests:
`crates/accounts/tests/events.rs`, `servers/lobby-server/tests/events.rs`, unit tests of the two modules.

## 1. Summary

| Question | Answer | Status |
|---|---|---|
| Currencies | three items: 951 `global_event_currency` "Snowflakes", 1138 `global_event_currency2` "Spruce trees", 1139 `global_event_currency3` "Stars" | PROVEN |
| Wallet | 8B fields `event1`, `event2`, `event3` (wire positions 3, 5, 6 of 7); per-account table `event_wallet` (schema v17) | PROVEN layout, implemented |
| Earning | `drop_after_combat`: 72 rows (result x place x mode), +25 % premium; +1 each with a worn winter hat; nothing else in the data | PROVEN data, implemented for online matches |
| Exchange | `48 28 u16 source, u16 target`; three rows: 951 x 750 -> 2202, 1138 x 1500 -> 2203, 1139 x 2500 -> 2200 | PROVEN data and request, implemented |
| Reply | `83 46 00` (flag 1, the item record) + 8B + `83 48 03` (request-lock ack) | LIKELY (the lock ack is PROVEN) |
| Event record | `94` record type 23 (`actionTime` = seconds left, `ui` = texts, second body non-empty) opens the window | PROVEN reading, LIKELY body bytes |
| Schedule | in no file; `[[events]]` entries of the config file (`config/srp.example.toml`, docs/notes/config.md), none = no event | CHOICE, implemented |
| Quests / achievements / chests | no event-currency rows (`quests.options` has them but the exe does not load it) | PROVEN absent |

## 2. The currencies

* The lobby movie's `PlayerInfo` sets `globalEventCurrencyDictId = 951`, `globalEventCurrency2DictId = 1138`,
  `globalEventCurrency3DictId = 1139` and the static `globalEventName = "new_year"` (PROVEN). `Reward.EVENTCURR1/2/3` = enum values
  8 / 9 / 10 (`RewardRenderer` picks the icon `<globalEventName>_event_curr1..3`), so a reward row of type 8..10 means these
  currencies (progression-audit.md section 8). The text colours per event (`eventCurrencyTextColors`) exist for `new_year`,
  `halloween`, `saint_patrick`, `summer`, `victory_day`: the client knows five events; this build's current one is New Year 2022.
* Items 951 / 1138 / 1139 (`items_069d.tsv`, category 29, stackable, icons 24 / 25 / 26 of the item atlas; the voucher icon folders
  of the five events carry `voucher_1_24..26`). Localised names (client `localization`, English): `st_global_event_currency_name`
  "Snowflakes", `..2_name` "Spruce trees", `..3_name` "Stars", `st_global_event_currency2_descr` "Exchange them for unique rewards!".
* 8B (reader 0x20018E4D3, PROVEN): 7 x u32 stored at +0x798690.. in the order silver (+0), gold (+4), **event1 (+8)**, spare parts
  (+0xE... `+0x7986A4`), **event2 (+0x1C)**, **event3 (+0x20)**, league (+0x7986A8). `set_account_money` (0x2001C0370) hands them to
  Flash as `generic_money`, `premium_money`, `global_event_currency`, `details`, `global_event_currency2`, `global_event_currency3`,
  `league_currency` (strings at 0x2009699E0.. / 0x2009699F0, 0x200969AE0, 0x200969AF8). The three event fields and the league field
  are the "new in 0.69d" ones next to silver, gold and spare parts. `protocol::s2c::lobby::Money` uses this order.
* The client skips the ids 951 / 1138 / 1139 when it adds item records (items.md section 3): they never live in the inventory, only in the wallet.
  `chests::grant` credits them to the wallet like silver, gold and spare parts.
* Server: `accounts::Profile::event: [u32; 3]`, `Currency::Event1..3`, table `event_wallet (account_id, slot, amount)` (only non-zero
  rows are written); `build::money_8b` sends it; `Profile::spend` / `credit` work like the other wallets (saturating at u32).

## 3. How players earned them

Searched in every decoded `.options` file (4 805 files, ints 951 / 1138 / 1139 anywhere): the only hits are `drops.options`
(72 rows), `event_exchange_action_items.options` and `quests.options`.

### 3.1 Match drops (PROVEN data, implemented for online matches)

`gameplay/drops/drops.options` `drop_after_combat` has 72 rows for the three currencies, each
`{game_mode_mask, dict_id, amount, place_in_match [n, n], service_needed 23, premium_factor 0.25}` plus `win_needed` (winners),
`loss_needed` (losers) or neither:

| `game_mode_mask` | rows | meaning |
|---|---|---|
| 503 (every mode bit but 3 and 9) | 24 winner rows + 24 loser rows | by place 0..7; winner place 0: 13 / 14 / 15, place 7: 3 / 4 / 5; loser place 0: 10 / 11 / 12, place 7: 1 / 2 / 3 |
| 8 (bit 3) | 24 rows without a result flag | place 0: 15 / 16 / 17, place 7: 4 / 5 / 6 |

* Mode bits (LIKELY): the client's `GameMode` constants are 0 gather, 1 team deathmatch, 2 capture point, **3 guns race**,
  4 paintball, 5 search and destroy, 6 slaughter, 7 PvE, 8 domination, 9 artifact hunt; the mask 8 rows are exactly Guns Race
  (`arena_01`, a mode with snowball and icicle weapons), so bit `n` = mode id `n`. (The achievement `mode_mask` of
  `achievements.rs` is a different table.) `ammo_boosters_drop` uses 247 = every bit but 3.
* `place_in_match` (LIKELY): the player's place in his own team by score, 0 = best, the same index the ranked league uses
  (`MatchFacts::team_place`). Places above 7 have no row: nothing is paid (CHOICE for the reading "no row = 0"). Equal scores share the
  better place (CHOICE). Without a stats source's counters (`Ended::stats` None) the place is unknown and nothing is paid (CHOICE: a
  place is part of every row, so nothing is invented).
* `service_needed 23` (PROVEN meaning of the number, see section 5): the rows apply only while the event record is running.
* Premium: `round(amount x 1.25)` (`premium_factor 0.25`; official: "+25 %" event currency, Steam news 0.62b, 0.66c, 0.69b). Rounding
  is CHOICE (the spare-parts code rounds).
* Worn items (PROVEN data, LIKELY timing): `event_exchange_action_items.options` `put_on_bonus_items`: the winter hats 1168, 1910, 1964,
  2054, 2201 (`newyear_2017..2022_helmet`) give `bonus_amount` 1 of each currency; the client text is "Use winter hat to receive more
  symbols". Server: +1 each, added flat after the premium factor, only when the match paid something; the hat is read from the
  profile's worn slots at payout (CHOICE: not at match start).
* `item_to_drop_after_win` 2201 (the 2022 winter hat; the faction battle table has the same key): no chance or count in any file.
  OFFICIAL (web evidence below): one 2201 on the account's FIRST PvP win of the event (`events::add_to_drops`
  -> `MatchDrops::once`, profile claim `event_first_win:<start>`, granted in the reward transaction).
* Local matches (training with bots) pay nothing (`LOCAL_MATCH_PAYS`), so nothing here applies to them.

Not paid because no data says so: kills, headshots, objectives, daily quests, achievements, chests. `quests.options` has 486 awards
`{type 5, param 23, param_2 = 951 / 1138 / 1139, amount 0, prior, premium_coef 1.5}` but the exe never loads that file (quests.md
section 1: the old quest system is dead data), so they are not a 0.69d mechanic (PROVEN absence in the daily quest data).

### 3.2 Shown in the match results

The `83 45 0A` reader (0x200194130) feeds `root.set_item_search_drop` (0x2001345A0), not
`fill_match_drop_items`. Layout (PROVEN): `u8 flag`; flag 0 -> `u8 n`, n x {item record, `u32 amount`, `u32 bonus`} (the UI shows
`amount - bonus` and `bonus_amount`); flag 1 shows `st_reject_drop_for_inactivity`; flag 2 closes the window. Its use as an event drop
carrier is only LIKELY. `83 45 0B` feeds `set_dropped_pve_items` (pve.md). Awards outside a match use 8B and the
`83 46 00` received-item popup (one record per currency, dict 951 / 1138 / 1139). PROVEN: the client draws them: every flag-1 record goes to `root.add_important_message` type 24 (NEW_ITEM) at 0x2001938A8 with no id filter
("Received:\n<name> xN"); only the inventory add 0x200193CB0 skips these ids. The 97 has no event-currency field, but the victory
screen CAN show them: `83 45 0A` flag 0 also calls 0x200179AE0 = `root.fill_match_drop_items(items, match_id)` (match id = the last
match options'), and `fillVictoryExtraRewards` shows a `SrvVsExtraRewards` panel when that match's list is non-empty (PROVEN path;
order before the 97 LIKELY). Server (`play::reward_messages`): a match with a results screen sends its drops (spare
parts, event currencies, chests / items) as `83 45 0A` flag 0 BEFORE the 97 (amount = the record's +0xC: a stack's count, another
item's own value; bonus = the premium / worn-item part: `MatchDrops::parts_bonus` / `event_bonus`) and no `83 46 00` popup (both
messages add the records to the inventory, PROVEN 0x2001942C7 / 0x20019382E). Checked in the exe: the panel filter 0x200179B9E
tests the record's FIRST u16 (wire `bound`, decoded +0xA by 0x2001263D0; the dict is decoded +8), 0 for these records. Awards
(no 97) keep the popup. OPEN (in-game check): the panel shows; if it does not, the order is wrong.

## 4. The exchange

* Window (PROVEN, AS `survarium.action.ItemsExchangeEventWindow`): three rows from the client's own copy of
  `event_exchange_action_items.options` (`data/event_exchange_069d.json`): 1139 x 2500 -> 2200 (Barrett M82 NY 2022), 1138 x 1500 ->
  2203 (character decal), 951 x 750 -> 2202 (character decal). It reads the balance from `PlayerInfo.global_event_currency*` (8B). If
  `source_amount <= balance` it sends `48 28 u16 source, u16 target` and locks its buttons; else it opens the "you lack" payment dialog
  (`not_enought_payment`, `amount - balance`). A decal that is already owned is shown "received" with a dead button
  (`DecorateControl.getCamoDecalById(target).owned`); the weapon has no such check.
* Request (builder 0x2001EA4A0): sets the lock L+0x7A5759, which only `83 48` or `84 48` clears.
* Server (`events::exchange`): window must be open (a schedule exists and `now < exchange_end`); the row must be in the table; one
  profile transaction charges `source_amount`, grants `target_amount` of the target (`chests::grant`: the weapon as an inventory row, a
  decal as an item) and refuses a decal the account already owns (CHOICE, mirrors the client's button; nothing is charged).
* Reply: success = `83 46 00 flag 1` with the item record (the client adds it and shows the received-item popup, as for the other
  grants; LIKELY), 8B, then `83 48 03` (PROVEN lock clear). A refusal logs the reason and sends `83 48 03`; the two a
  player can cause first get the client's own text through `84 48 01 <key>` (`events::refusal_text`; localised at 0x200176430):
  too little currency `st_not_enought_items_for_exchange` ("Not enough money for exchange"), an owned decal `st_item_bought`
  ("You already have this item"); key choice CHOICE. The window's own
  buttons are re-armed by the client when the received-item message arrives (`ActionsList.updateBigRenderer` after `NEW_ITEM` /
  `CHEST_OPENED`, LIKELY the popup does it). PROVEN: the only real lock is the C++ byte
  L+0x7A5759, cleared by any `83 48 xx` or `84` (PROVEN), so the `83 48 03` of a refusal unlocks it; the Flash `lockButtons` looks
  for a child `eventUi` that the db 75 movie does not have (LIKELY a no-op). The `84 48` text is sent (above).
* Exchange is allowed after the event ended while the window record is still sent (`exchange_end`): the client text for a finished
  event is "exchange the symbols for the unique weapons" (`st_halloween_descr_1`).

## 5. The event record: `94` type 23

* PROVEN: the 94 builder (0x2001C56B0, jump table 0x1C5EB4 / byte table 0x1C5EC8: type 23 = case 2) turns a record of type 23 into
  an action of `root.set_actions_data` with `service_dict_id` 23, `useTime`, `actionTime` = the record's second u32 (`f38`, seconds
  left) and, when the record's FIRST body is not empty, `ui` = that body converted by the generic config converter (0x294190). The
  type-specific part (0x2001C63EE: skipped when the SECOND body is empty) adds `actionItemsData` (6 rows from the client's own
  table, `source_dict_id` / `source_amount` / `target_dict_id` / `target_amount`), `only_exchange` and `id` = 23. A record with both
  bodies empty is skipped entirely (0x2001C5A50).
* PROVEN (AS `ActionsList.openAction`): `FactionsBattleWindow.FACTIONS_BATTLE_ENABLED` is false in this build, so the action with
  `id` 23 shows the message named `<PlayerInfo.globalEventName>_event`: `new_year_event` -> `SrvNewYearEventMsg`, a skin of
  `ItemsExchangeEventWindow` (registered with its four siblings `halloween_event`, `saint_patrick_event`, `summer_event`,
  `victory_day_event`). Number 23 is the same one the drop rows call `service_needed`.
* The window: header = `ui[i].objectData.label` (upper-cased), title = `ui[i].objectData.description` for the first
  `TextComponent`; a `bottom=` marker splits the description into the title and a small text below. Both empty -> the client's own
  `st_halloween_title` ("Halloween Weapons!") and `st_halloween_descr_1`, which are wrong for New Year, so the server always sends a
  `ui` body. The countdown is `actionTime`; after it reaches 0 the timer hides and the buttons stay.
* Server: while `now` is in `[start, exchange_end)` every `94` (login, premium change, hero push) carries one record
  `{f30 1, f38 seconds to the event end (0 after it), type 23, data1 = ui blob, data2 = exchange rows}`.
  `login_calendar::with_calendar` adds it with the calendar record, so every `94` sender gets it.
* The second body (PROVEN): the status builder 0x2001C6350 LZMA2-decodes
  data2 (0x2001C64DE), then the type-23 branch 0x2001C6E19 reads the DECODED bytes with 0x2001B8EF0 into the lobby object +0x15F334:
  `u8 n` (<= 6) x `{u16 source_dict_id, u32 source_amount, u16 target_dict_id, u32 target_amount}`, `u8 k` (<= 32) x
  `{u16, u8 m (<= 4), m x {u16, u32}}`, `u8 only_exchange`. The builder makes `actionItemsData` from the rows and `only_exchange`
  from the flag; `ItemsExchangeEventWindow.fillItemData` draws the window's rows from `actionItemsData` (target item, source amount),
  not from the client's own options file. A body of `[0]` (an empty LZMA2 stream) makes the reader read past the decoded bytes. The server
  sends the `exchange_items` rows (`events::exchange_body`), k = 0 (no exe code reads +0x64 by offset), only_exchange = 1
  (`ActionsList.gameModeActionPresent(23)` false: no event game mode). `tests/client_94.rs` models the reader.
* LIKELY, to check in the game: the root of the `ui` body is an indexed table (`protocol::binconfig::encode_value`).
* Texts (CHOICE): default header "Holiday season event" (the client's `st_whats_new_051_ny_event`), default body "Exchange them for
  unique rewards!" (the client's `st_global_event_currency2_descr`); an `[[events]]` entry's `header` / `text` replace them
  (ASCII; the blob must stay under 0x800 bytes or the defaults are used). The real server sent its own texts: no file has them.

## 6. The schedule (CHOICE, no file has dates)

`LobbyConfig::events: Vec<game_data::events::Schedule>`, from the `[[events]]` entries of the config file: `event = "new_year"` (the only event the data has rows for), `start` / `end` (UTC dates; `end` is exclusive and
ends the drops), `exchange_end` (>= `end`, closes the window, default `end`), `header`, `text` (client `st_*` keys); entries must not
overlap, the one whose window is open now runs (`LobbyConfig::event_at`). srp logs at start which event runs until when, or "no event
scheduled". The commented example entry in `config/srp.example.toml` is the one to copy and date. No entry (the default): no record, no drops, `48 28` is refused. The faction battles and the login
calendar are handled the same way (an input, nothing invented).

## 7. Other event features found

| Feature | Evidence | Status |
|---|---|---|
| Faction battle (the "winter action") | `FACTIONS_BATTLE_ENABLED` false; the battle runs through the 8E status | already implemented (`faction-battle`), not an event record |
| Legacy "global event" with progress items (`set_global_event_data`, `set_global_event_unlocked_items`, `set_global_event_coins`, `set_global_event_multiplier`, `set_global_event_cost`, `GLOBAL_EVENT_REWARD` important message type 28, `SrvEventItemsProgressBar`) | the AS has the methods; the exe has no string `root.set_global_event*` (searched), so nothing calls them | dead in 0.69d, not implemented |
| Buying an action with the event currency (`5C 01` payment kinds `Silver, Gold, RealMoney, GlobalEventCurrency`, table 0x2009907E0) | the Flash event passes mode 1 or 2 only (actions.md section 4) | not reachable, not implemented |
| Event items (hats and weapons of the five events) | items 1168 / 1910 / 1964 / 2054 / 2201 (hats), 2200 (Barrett), 2202 / 2203 (decals) | in the item data; only the exchange / the hat bonus use them |
| Seasonal event quests | `quests.options` event awards, file not loaded by the exe | not a 0.69d mechanic |
| Event shop tabs | no tab list in any file (actions.md section 2) | OPEN (store tabs are a setting) |
| Other events' exchange rows | only the New Year 2022 rows exist in the data (`halloween`, `saint_patrick`, `summer`, `victory_day` windows exist, their tables do not) | OPEN |
| Event medkit / summer / "global_event_0" strings ("Summer Solstice", "Solar token", boost "x2 tokens") | client localization of an older event | not used by 0.69d code paths found |

## 8. Checks in the game

1. Start the server with an `[[events]]` entry covering now (the start log line says "RUNNING now"). Sign in: the lobby's action list should
   show the event action (with the countdown); opening it should show `ItemsExchangeEventWindow` with the three rows, the configured
   texts and the balances (all 0 until a match pays).
2. If the lobby crashes or the action does not appear at sign-in with the event on: the 94 record 23 body layout (section 5) is the
   suspect; remove the `[[events]]` entry.
3. Play an online match with a stats source's counters: the 8B wallet should grow by the row of the place and result (winter hat: +1 each;
   premium: +25 %), a received-item popup lists the currencies.
4. Credit a wallet in the database and exchange: the item arrives with the popup, the decal row turns "received", a second
   exchange of the decal is refused. Check whether the window's buttons unlock after a success and what a refusal looks like.

## Web evidence

Sources: the Steam news feed of appid 355840 and the web; labels OFFICIAL = developer post, FAN = fan site, NOT FOUND:
* New Year 2021/22: "December 24 - January 10, 12PM GMT" (OFFICIAL 0.69b, 2021-12-24, gid 4224938027302742632; start hour not
  stated); "Ranked matches are unavailable during the event"; 0.69c (2022-01-20) removed the holiday lobby. Events since 2020
  lasted about 10 days, New Year 15-17 days.
* Currency: from PvP matches, a higher place gives more, winners get more (Halloween 2020 / 2021), Guns Race more than other
  modes, premium +25 %, Winter Hat 2022 +1 each (OFFICIAL): matches the client rows we already use. Per-place amounts: the
  client data (no web table, NOT FOUND).
* Win item: NOT a chance. "You will receive Winter Hat 2022 for the first win in a PvP-match during the event!" (OFFICIAL):
  one item (2201) on the account's first PvP win of the event; the server does this (section 3.1).
* Exchange after the end: 2017 Halloween stayed open; January 2018 about 3 more days (OFFICIAL, old); 2019-2022 NOT FOUND.
* Window text candidate (OFFICIAL patch-note wording, not proven in-game): "Holiday Season Event" / "Play in PvP game modes,
  receive holiday currency (snowflakes, pine trees and stars) and exchange it for the equipment with unique holiday camos!
  The higher you place after a match, the more currency you get!"
* Faction War 2020: 10 days, tiers 10k / 20k / 40k / 60k / 80k reputation (= the client's 5 tiers), the winner's items -50 %,
  the others -25 % for about a week (OFFICIAL).
