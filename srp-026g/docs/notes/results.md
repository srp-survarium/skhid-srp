# 0.26g match results, player detail, drops, daily bonus, craft tables (vs 0.21d)

Version scope: client **0.26g build 2777** (Jan 2015, no PDB) vs **0.21d build 2010** (Apr 2014, PDB). VAs are image VAs (image base 0x10000) of each build. Static analysis only.

Tags: **PROVEN** = read in 0.26g code at the cited VA (and matched to the cited 0.21d symbol/VA where one exists); **LIKELY** = strong inference; **GUESS** = weak.

Conventions: all integers little-endian. `str(N)` = `u8 len` + `len` bytes, copied with no length check into an N-byte buffer and NUL-terminated (so at most N-1 bytes are safe; a longer string overflows the next field). `bool` on the wire is any non-zero byte. Every server->client body below follows the 1-byte lobby message id (the UDP match-transport framing is in `transport.md`). Readers use `buffer_reader` (`[r+4]` = cursor) with **no bounds checks**: a short message reads past the end.

Shared reader: the **item descriptor** `0x1F1350(reader, dest, items_dict, mode)` -> `0x1F13D0` (PROVEN 0x1F1350..0x1F14F3):

| # | wire | dest | note |
|---|---|---|---|
| 1 | u32 item_id | +0x00 | |
| 2 | u16 dict_id | +0x04 | **if 0, nothing more is read** (0x1F137A) |
| 3 | u32 condition_or_stack | +0x08 | condition is x10000 (UI shows `int(v*0.0001+0.5)`), stack items: count |
| 4 | u32 | +0x0C | only if **not stack and mode == 0** |
| 5 | u32 | +0x10 | only if **not stack** |
| 6 | u8 n_mods, n x {u32 modifier_id, bool} | vector +0x14, inline cap **8** | n > 8: **fatal assert "buffer overflow"** (`buffer_vector<inventory_item_descr::modification>::resize`, 0x1F1534) |
| 7 | u8 n_upg, n x {u32, u32, u64, u8} | vector +0x60, inline cap **3** | n > 3: **fatal assert** (`buffer_vector<upgrade>::resize`, 0x1F1655) |

"stack" = `items_dict[dict_id].is_stack` (byte +0x22A of the dict entry, 0x1F1393). An unknown dict_id (lookup 0x1F0090 returns NULL) is treated as stack (0x1F139F) - the server and client then disagree about fields 4/5 and the rest of the message is misparsed. Descriptor size in memory 0xB4.

---

## 1. 0x97 match stats (old `0x36 13`)

PROVEN path: case @0x274E39 -> `read_match_stats` 0x2718C0 -> `match_stats` reader 0x26E610 (into lobby_client+0x1E57DC, "S") + `player_result_full` reader 0x26EDD0 (into a stack local, "F") -> `lobby_menu_logic::call("show_match_statistic")` 0x2812E0 (runs the Flash-callback handler 0x29EDA0: `root.show_victory_screen`, lobby_menu+0x313A = 1) -> fill 0x29AB20 (`root.fill_victory_screen`) -> **client sends `3C 00`**.
0.21d: `read_match_stats` 0x19E6B1, `match_stats` 0x19C78F, `player_result_full` 0x19C93C, `fill_match_statistic` 0x177271.

Body = `match_stats` + **the recipient's** `player_result_full` (same as 0.21d).

### 1.1 `match_stats` (0x26E610), PROVEN layout

| # | wire | S+ | meaning (Flash key, from 0x29AB20) | vs 0.21d |
|---|---|---|---|---|
| 1 | u32 | 0x8BC | match_id (`match_id`) | same |
| 2 | u32 | 0x8C0 | **never read by the client** (only reset/written) | **NEW** |
| 3 | u8 | 0x8D8 | winner team. `match_result` = 5 if own brief.team == winner else 0 | same |
| 4 | u8 | 0x8D9 | team 0 score -> `team1_score` if own team 0 else `team2_score` | same |
| 5 | u8 | 0x8DA | team 1 score (mirror) | same |
| 6 | u32 | 0x8C4 | team 0 param -> `team1_param` if own team 0 else `team2_param` (meaning GUESS: mode-specific score, e.g. points) | **NEW** |
| 7 | u32 | 0x8C8 | team 1 param (mirror) | **NEW** |
| 8 | u8 | 0x8DB | `game_mode` (raw number to Flash) | **NEW** |
| 9 | str(64) | 0x000 | best player name | same |
| 10 | str(15) | 0x100 | best player **clan tag**; title shown as `"[tag] name"` (0x29ACE0) when non-empty | **NEW** |
| 11 | u16 | 0x8CC | best player score (`title_0_value_0`) | same |
| 12 | str(64) | 0x040 | best shooter name | same |
| 13 | str(15) | 0x10F | shooter clan tag | **NEW** |
| 14 | u8 | 0x8D6 | shooter accuracy % (`title_1_value_0`) | same |
| 15 | u16 | 0x8CE | shooter shots (`title_1_value_1`) | same |
| 16 | str(64) | 0x080 | best support name | same |
| 17 | str(15) | 0x11E | support clan tag | **NEW** |
| 18 | u16 | 0x8D0 | support value (`title_2_value_0`) | **CHANGED**: 0.21d had two u16 (saved, healed); `title_2_value_1` string no longer exists in 0.26g |
| 19 | str(64) | 0x0C0 | best explorer name | same |
| 20 | str(15) | 0x12D | explorer clan tag | **NEW** |
| 21 | u16 | 0x8D2 | explorer artifacts (`title_3_value_0`) | same |
| 22 | u16 | 0x8D4 | explorer items (`title_3_value_1`) | same |
| 23 | u8 n | 0x8D7 | player rows. Array is 20 x 0x60 at S+0x13C (ends at S+0x8BC). **No cap check** | same cap 20 |
| 24 | n x brief | 0x13C + k*0x60 | see 1.2 | changed |

Flash also gets (computed, PROVEN 0x29AB20): `statistics_score` = own brief score; `team_1_players_count` / `team_2_players_count` = number of rows with team 0 / team 1 (**absolute**, not relative to own side, unlike the scores); `map_name` = localized lobby_menu+0x313C, which is set from the client's own last match info (0x22052A, called via 0x27BEA0 in the static-match-info path, LIKELY) - **not** from this message.

### 1.2 `player_result_brief` (0x26E4E0), PROVEN layout, stride 0x60 (0.21d: 0x50, 0x19C69C)

| # | wire | +off | meaning (Flash member of `players[]`) |
|---|---|---|---|
| 1 | str(64) | 0x00 | nickname. `name` = `"[tag] name"` if tag non-empty. Row whose nickname == own nickname (lobby_client+0x1E55D0) gets `color` = 0xF1E545 |
| 2 | str(16) | 0x40 | **NEW clan tag** (<= 15 chars) |
| 3 | u32 | 0x50 | player id (`id`); own row = first row with id == F.id (0x271910; default row 0) |
| 4 | u16 | 0x54 | score (`score`) |
| 5 | u16 | 0x58 | kills (`kills`) |
| 6 | u16 | 0x5A | deaths (`deaths`) |
| 7 | u8 (stored u32) | 0x5C | team 0/1. `team` = 1 same side as own row, 2 otherwise |
| 8 | u16 | 0x56 | level (`rank`) |

Wire order is the 0.21d order with the tag string inserted after the name.

### 1.3 `player_result_full` (0x26EDD0), PROVEN layout, **completely redesigned**

In-memory size 0x298 (ctor 0x26ECE0). Reader arg = items dictionary (lobby_client+0x1E56C0 -> +0x53A4).

| # | wire | F+ | meaning (Flash key) |
|---|---|---|---|
| 1 | u32 | 0x000 | player id (selects own brief row) |
| 2 | 3 x **score component** (variable, below) | 0x0F0 + i*0x68 | `score_components[]` |
| 3 | 3 x **bonus block** (33 B each, below): daily, premium items, premium | 0x228, 0x24C, 0x270 | see 1.4 |
| 4 | u16 | 0x294 | `elo_before` |
| 5 | u16 | 0x296 | `elo_after` (both in `statistics_rating`) |
| 6 | u32 | 0x0E0 | `quest_money` |
| 7 | u16 | 0x0E4 | `quest_rep` |
| 8 | u8 | 0x0E6 | `faction_id` (reputation faction, in `statistics_reputation`) |
| 9 | 3 x u16 | 0x0E8 | play time in seconds per profile column -> `profile_play_time` "%d:%02d" (0x29BD15) |
| 10 | u8 nA, nA x u32 | count 0x014, data 0x004 | list never read by the UI (as in 0.21d). Room for **4**; **no check** (see hazards) |
| 11 | item descriptor, mode 1 | 0x018 | the **dropped item** (`dropped_item`: `item_id`, `dict_id`, `condition_or_stack`, `modifiers`). `allow_drop_card` = item_id != 0 or dict_id != 0 |
| 12 | u8 | 0x0CC | `rarity_level` of the drop |
| 13 | f32 | 0x0D0 | drop chance, base (`st_item_drop_chance_base`, shown `+%.1f%%` of v*100) |
| 14 | f32 | 0x0D4 | drop chance from match result (`st_item_drop_chance_match_result`) |
| 15 | f32 | 0x0D8 | premium drop bonus; shown only if > 0; label `st_item_drop_chance_by_premium` if > 12.5 else `st_item_drop_chance_team_premium` (constant 12.5 @0xA2DA88 - odd for a fraction, kept as read) |
| 16 | f32 | 0x0DC | penalty (`st_item_drop_chance_penalty`, `-%.1f%%`). Total `value` = D0 + D4 + D8 - DC (Number) |

**Score component** (reader 0x26E960, record 0x68 bytes; PROVEN):

| wire | +off | meaning |
|---|---|---|
| str(64) | 0x00 | `name`: a **localization key** (looked up via 0x1FF0F0) |
| bool | 0x64 | show `amount` |
| 3 x { f32 amount; **if amount != 0.0**: u32 money, s16 exp, s16 rep } | f32 @0x58+4i, u32 @0x40+4i, s16 @0x4C+2i, s16 @0x52+2i | per profile column i: `amount` (truncated to int), `money`, `experience`, `reputation`. **Conditional wire fields**: for amount == 0.0 (or -0.0) the 8 bytes are absent; NaN counts as non-zero |

Columns i = 0..2 are the **local profile slots** (name from the local profile list +0x58 + i*0x704, i < profile count). Column i is shown only if some component has amount[i] != 0 (mask built at 0x29AB80). Profiles 4+ cannot be reported.

### 1.4 Bonus blocks (reader 0x26EA50, 0x24 bytes each; PROVEN)

Each block on the wire is 3 x { u32 money, u16 rep, u16 exp, u8 bonus_money, u8 bonus_rep, u8 bonus_exp } (11 bytes per column, interleaved per column). Memory: money u32[3] +0x00, rep u16[3] +0x0C, exp u16[3] +0x12, bonus u8[3] +0x18/+0x1B/+0x1E.

| block (F+) | money | rep | exp | u8 fields |
|---|---|---|---|---|
| daily (0x228) | `daily_money` | `daily_rep` | `daily_exp` | `daily_bonus_money` / `daily_bonus_rep` / `daily_bonus_exp` |
| premium items (0x24C) | `premium_items_money` | `premium_items_rep` | `premium_items_exp` | `premium_items_bonus_money` / `_rep` / `_exp` |
| premium (0x270) | `premium_money` | `premium_rep` | `premium_exp` | read but **unused** |

Each Flash key is an array of up to 3 values (one per profile column in the mask), built by 0x29F090 (u32), 0x29EF30 (u16), 0x29EFE0 (u8), 0x29EE80 (s16), 0x29EDD0 (f32->int). The u8 "bonus" values are GUESS: percentages.

Wire size of F: 4 + components + 99 + 4 + 4 + 2 + 1 + 6 + 1 + 4*nA + descriptor + 1 + 16.

### 1.5 What 0.21d had and 0.26g removed (PROVEN by string absence in 0.26g)

No `statistics_0..12` (assists, healed, saved, headshots, batteries, artifacts, killstreak, accuracy), no `weapon1_dict_id`/`weapon2_dict_id`, no `items_drop[]` (4 x {dict,count}), no `match_progress`, no `level`, no batteries-stolen x100, no rating before/after as the only rating. The 0.21d write of full+0x48 to profile P+0x9E0 is gone (0x2718C0 does not touch profiles). Drops became **one** full item instance + chance breakdown.

### 1.6 After the screen: `3C 00`

0x2719A2 creates message **0x3C** with payload `00` (inline copy of 0x37A0E0 + sender 0x379640). The same `3C 00` is sent after "Quest completed: %s" (0x22633B) and after operation_permitted 0x4B (0x274EED, followed by `3E 00`). LIKELY = "query quests" (0.21d `21 0D`). srp must answer it (or ignore it harmlessly).

---

## 2. 0x93 player match stats (old `0x36 0E`) - meaning changed

PROVEN: case @0x274DF6 -> 0x271A30: `player_result_full` (0x26EDD0, same layout as 1.3) is read into a stack local and **discarded**; then an **account/profiles block** is read with 0x1F1070 (the same reader used by 0x87 @0x27027D and 0x88 squad member profile @0x274BF1) and only that is passed to `fill_victory_screen_player` 0x29E2C0 (Flash keys: `name`, `level`, `state`, `item_slots`, `artifact_slots`, `dictId`, `slotId`, `condition_or_stack`, `parameters`, `base_rarity_level`, `modifiers`, `rarity_level`, `items`).
0.21d: `read_player_match_stats` 0x19E81F -> `fill_player_statistic` (per-player stats panel).

So in 0.26g clicking a player on the victory screen shows **that player's profiles/equipment**, not his statistics. Body = `player_result_full` (any valid one; zeroed is fine) + 0x1F1070 block (u8 -> +0x45EC, str(64) -> +8, str -> +0x48, u32 -> +0x45F4, then 0x1F1120: u8 profile count (+0x57), u8 (+0x1564), per profile u8, u32 id, str name, ... - full layout belongs to the 0x87 analysis).

---

## 3. 0x98 read_drop (NEW)

PROVEN 0x271AE0 (`bool lobby_client::read_drop(buffer_reader&)`, string at 0xA07798):

| # | wire | meaning |
|---|---|---|
| 1 | item descriptor, **mode 0** (inventory form) | the dropped item |
| 2 | u32 | drop-card item_id (inventory item that was used) |
| 3 | u32 | new count of that drop card (absolute, written to inventory_item+8) |
| 4 | u8 | rarity_level of the dropped item (UI only) |

Effects:
1. Inventory (vector lobby_client+0x1EA978..+0x1EA97C, stride 0xB4) is searched for the dropped **item_id**: if present, its +8 (count) += descriptor.condition_or_stack (0x271C19); else it is appended (0x26E8C0) **unless dict_id is 0xD6 or 0xD7** (0x271B94): 214 `generic_money` and 215 `premium_money` of `items_dict` (PROVEN by the dictionary). A dropped silver or gold is not an inventory item: the server adds it to the balance and pushes 0x8B.
2. Drop card lookup by item_id (0x277AA0): found -> count := field 3; not found -> log "Drop card with id %d was not found in inventory" (0xA07768), no crash.
3. 0x288520 refreshes inventory UI (`root.setup_inventory_data`), then 0x29F140 -> `root.fill_victory_screen_drop_item` {`dict_id`, `item_id`, `condition_or_stack`, `rarity_level`, `modifiers`}.

Trigger: Flash `use_drop_card` (handler 0x288D00, slot +0x740) -> **client `46 03 <u32 a> <u32 b>`** (0x270AE0). **a = the match's id, b = the drop card's item id** (PROVEN 2026-10-12: `VictoryScreen.onTreasureMapClick` calls `use_drop_card(this._match_id, PlayerInfo.getVoucherId(1))`, and 0x288D00 pushes Flash argument 1 and then 0, so 0x270AE0 writes argument 0 first; the earlier guess had them the other way round). The lobby takes the proven order first and still tries the other. A messaging line (see 5.3) can also make the client send `42 00 <u32>` / `43 00` ("query drop").

Hazard: 0x29F222 calls the dict lookup and dereferences `[eax+0x22A]` **without a NULL check** -> dict_id 0 or unknown in 0x98 = **crash**.

---

## 4. 0x99 daily bonus and 0x9B craft tables (NEW)

### 4.1 0x99 (PROVEN 0x2769B0 -> 0x2773A0 into lobby_client+0x1EE614 "B" -> 0x229520 `set_daily_bonus_data`)

| # | wire | B+ | meaning |
|---|---|---|---|
| 1 | u8 n | 0x07 | `current_day` |
| 2 | n x u8 | 0x00.. | per-day progress; UI always reads 7 bytes B+0..B+6 into `progress[7]`. **n > 7 overwrites B+7 and the descriptor; no check** |
| 3 | u8 is_stack (bool) | - | **sent on the wire here, and FIRST** (no dictionary lookup): `mov dl,[eax]` at 0x2773D1 |
| 4 | u32 item_id | 0x08 | reward item (descriptor, see note): `mov eax,[ecx]` at 0x2773D9, one byte on |
| 5 | u16 dict_id | 0x0C | `reward_item.dict_id`: `mov dx,[ecx]` at 0x2773EA |
| 6 | u32 condition_or_stack | 0x10 | raw to Flash |
| 7 | if !is_stack: u32 (+0x14), u32 (+0x18) | | mode 0 |
| 8 | u8 n_mods (<= 8) x {u32, bool}; u8 n_upg (<= 3) x {u32, u32, u64, u8} | | fatal assert if exceeded |
| 9 | u8 | 0xBC | `completed` (bool) |
| 10 | u8 bool | - | if set, calls Flash callback `show_daily_reward_window` |

Note the descriptor order differs from 0x1F1350: here `u8 is_stack, u32 item_id, u16 dict_id`, then 0x1F13D0 (called at 0x277407 with the flag and mode 0), and dict_id 0 does **not** stop the read.

**Corrected 2026-10-12 (this table had `u32 item_id, u8 is_stack, u16 dict_id`; the flag is first). It was the cause of the first live crash.** srp sent the reward of 50 gold as `01 00 | 00 00 00 00 | 01 | D7 00 | 32 00 00 00 | 00 | 00 | 00 | 00` (17 bytes). The client read is_stack = `00`, item_id = 0x01000000, dict 0xD7, and so took the stack for an item that is none: `u32 condition` = 50, `u32` (+0x14) = the four bytes meant as the two counts and the two flags, `u32` (+0x18) = **4 bytes past the message's end**, then its count of modifications, the modifications, its count of upgrades, the upgrades and the two flags, all from whatever lay behind the message. No reader checks the end (the `buffer_reader` is a begin, a cursor and a length nobody compares).

- The first time in a game's life the bytes behind are LIKELY zeros (a fresh buffer): 0 modifications, 0 upgrades, and nothing is seen but a wrong item in the window. That is why the first sign-in of the session (09:07:21) passed.
- The second `99` of the same game (09:22:07, its reconnect to the restarted server) found old data there. The minidump: the reader on the stack at 0x9A7EFB08 is `{begin 0x72D61D3C, cursor 0x72D61DA1, length 0x11}`, so **101 bytes were read of a message of 17**: 9 + 12 + (1 + 5 x 5) + (1 + 3 x 17) + 2, that is **5 modifications and 3 upgrades** nobody sent (the 5 is the count 0x229FD0 held in `esi`, saved by 0x229A40 at 0x9A7EAC0C; the 3 is what the 101 leaves). The message's bytes themselves are heap and not in the dump.
- `set_daily_bonus_data` 0x229520 then gives the reward to Flash: 0x229838 calls 0x229FD0 for the modifications (the vector at B+0x1C), which calls 0x229A40 for each `{u32 id, bool state}`. 0x229A40 looks the id up in the modifications the game holds (`[[game+0x9C]+0x53A4]`: the list at +0x148 with its count at +0x14C when the state is set, the one at +0x140 / +0x144 when it is not; entries of 0x2C bytes; 158 of them there) and **reads `[entry+4]` (`ui_desc`) with no check that it was found**: `xor esi,esi; mov eax,[esi+4]` at 0x229AC2/0x229AC4 = the crash. Here: state 0, an id that is no modification, 158 entries searched (eax = edx = 0x9E).

So: **a `99` whose reader runs past its end is a crash waiting for its second time**, and a modification id the game does not know in a `99` (or any descriptor given to 0x229FD0: also 0x28838C, 0x29D950, 0x29F35B, 0x29F717) is one at once. Any second `99` would have done it: `43 00`, the one after a match, the next sign-in. The reconnect only happened to be the second.

### 4.2 0x9B craft chances/prices (PROVEN 0x2769F0 -> 0x29F980)

| # | wire | to Flash |
|---|---|---|
| 1 | 12 x f32 (3 rows x 4) | `root.setup_item_craft_chances` (flat 12 Numbers) |
| 2 | 16 x u16 | `root.setup_item_craft_prices_general` |
| 3 | 16 x u16 | `root.setup_item_craft_prices_premium` |

Fixed 112 bytes, no counts, no hazards. **Meaning (PROVEN in the UI, `MessageCraftItem.updateCraftChance`):** a row for each difference in rank between the item and its donor (0, 1, 2); its four values are success and failure in silver, then success and failure in gold, in percent; the two price tables are indexed by the item's level. The values are `gameplay/items/craft/craft.options` (items.md).

---

## 5. Requests from the client

### 5.1 Ask for match stats (old `21 13 <match_id>`) - PROVEN

0.26g sender 0x26FF90: **`41 00 <u32 match_id>`**. Called only from `lobby_menu::on_stats_message_arrived` 0x225810 ("query match stats for %d", 0xA01A08).
Trigger: chat text message **0xA2** (messaging merged into the lobby connection) parsed at 0x26B6F0: `u8 sender_type, u32 sender_id, str name, [type 5: extra str], u8 kind, str(256) body`. For sender_type **8**: kind 3 -> clan system message (0x26B940), kind 4 -> additional drop (5.3), any other kind -> if body contains `#mstats` -> parse `[N]` (strncpy_s 16 bytes, then 0x428CB2(buf,0,0), LIKELY `strtol(..., base 0)`: avoid leading zeros/`0x`) -> send `41 00 N`. Missing brackets -> N = 0.

Differences: no "lobby connected" check and no pending path: the 0.21d string "query match stats on connected for %d" is absent in 0.26g, and the sign-in message `2E` is just `u32 session, bool` (0x26F69B..0x26F6F1) - **`pending_match_stats_id` no longer exists**. 0x97 can also be pushed unsolicited: it opens the screen by itself (1, 0x271958).

### 5.2 Victory-screen player request (old `21 0E <player> <match>`) - PROVEN layout, LIKELY arg meaning

Flash `victory_screen_request_player_data` (handler 0x29ED70, slot +0x6A0; slot order matches the name table at 0x9C5798..: `close_victory_screen` 0x29ED60 clears +0x313A, `show_match_statistic` 0x29EDA0) -> 0x26FF00: **`3D 00 <u32 arg0> <u32 arg1>`**; by the 0.21d AS3 arg0 = player id (brief id), arg1 = match_id. Reply: **0x93** (section 2).

**From the Rankings (srp's, 2026-10-20).** srp's changed lobby movie sends `3D 00 <0xFFFF_FF03> <place>` (`GEAR_BY_PLACE`) for "Show player's information" on a row of the Rankings; the server answers `93` with that account's kits as they are now, for any account (`player_gear.md`). No results row has a player id that high.

**How long srp can answer it (2026-10-12).** PROVEN in the 0.26g ActionScript: `VictoryScreen.reguestPlayerInfo` calls `victory_screen_request_player_data(uint(rendererData.id), this._match_id)` for a row not yet in `playersCache`, so arg0 is the row's player id and arg1 the match id; with no answer the panel does not open and nothing waits. 0x93 has no "nothing to show" form. So srp keeps a match's report for as long as a screen of it may be open: the last 64 reports, and past them any whose screen an account was sent in the last two hours (`rewards.rs`, `SHOWN_FOR_SECONDS`; `social.md` §9.5).

### 5.3 Drop queries (NEW) - PROVEN

0xA2 sender_type 8 kind 4 (0x26B7F4..0x26B84B) -> `lobby_menu::on_additional_drop_arrived` 0x225950 parses two `[...]` numbers (0x227120): first = value, second = type. type 0 or 2 -> **`42 00 <u32 value>`** (0x270000); type 1 -> **`43 00`** (0x26FE20); any other type sends nothing; logs "query drop: %s".

**`42 00` is not answered, on purpose** (traced 2026-10-12):
- 0x270000 has one caller, 0x2259D9, and 0x225950 has one, 0x26B84B: the client sends `42 00` only after that chat line of the statistics service. PROVEN. srp never sends the line, so a client of srp never sends `42 00`. Every drop srp gives comes another way: the match's in `97`, a drop card's in `98` (answer to `46 03`), the daily bonus's in `99` (which is also the answer to `43 00`).
- 0x270000 only writes `42 00 <value>` and queues it: no field is set, no timer started, nothing waits. PROVEN. So no answer is harmless.
- What the original server sent back is not in the client. `98` is LIKELY (the one other message that carries a drop; its handler ends in `fill_victory_screen_drop_item`, which fills the open results screen's treasure item), with the type-1 line and `43 00` -> `99` as the pattern. What `value` is (a drop's id, a match's) and what types 0 and 2 are is not in the client either: GUESS.
- So an answer would need srp to invent both the announcement and the meaning of its number. It is left out until a reason to announce a drop this way turns up; the lobby logs the request (`dispatch.rs`, `QueryDrop`).

### 5.4 Premium from the victory screen

`accept_premium_victory_screen` (0x2A5FB0, slot +0x980) -> 0x271040 **`47 02 <u32 arg0> <u32 arg1> <f32 0.0>`**, same sender as `buy_shop_item`/`accept_premium` (which pass arg1 = 0); sent only when lobby_client+0x1E57A4 == -1.

---

## 6. Crash hazards (all PROVEN code paths; impact LIKELY)

1. **0x97/0x93 list A** (F+0x14 count, room for 4 u32 at F+4): unchecked. The 5th u32 lands on the count byte itself, so a crafted value keeps the loop going; F is a stack local (0x97: esp+0x30 in a 0x2B8 frame, F ends exactly at the return address) -> **stack overwrite / return-address smash**. Always send nA = 0.
2. **0x97 rows** n > 20 overwrite S+0x8BC.. (match fields) and then lobby_client memory past S+0x8DC.
3. **Strings**: titles 64 / tags 15, brief name 64 / tag 16, component name 64 - u8 length up to 255 is copied unchecked.
4. **Brief team >= 2**: 0x29B682 does `inc byte [esp+0x48+team]` (team is u8 -> up to 255) inside the 0xB34 UI frame: team 2/3 hit padding, larger values corrupt locals (GFx object pointers at esp+0x4C/0x50) -> crash. Send 0/1 only.
5. **Drop descriptor** in 0x97 with unknown non-zero dict_id: reader misparses (treated as stack) and UI 0x29D825/0x29D848 dereferences the NULL lookup -> crash. dict_id 0 (no drop) is safe in 0x97 (UI skips at 0x29D7B1).
6. **0x98** dict lookup at 0x29F222 has no NULL/zero guard -> crash for dict_id 0 or unknown.
7. **Descriptors** (0x97, 0x98, 0x99): mods > 8 or upgrades > 3 -> fatal assert "buffer overflow".
8. **0x99** day count n > 7 overwrites the reward descriptor and beyond.
9. Score components: forgetting that the 8 bytes per column are omitted for amount == 0.0 desyncs the rest of 0x97/0x93.
10. **0x97 to a game that has loaded no match** (PROVEN, crash dump of 2026-10-13): the screen's builder reads the map's name from lobby_menu+0x313C (0x29ABE1) and gives it to `translate_text` (0x29AC04 -> 0x1FF0F0), which reads the string at once (0xF6C47). The field has one writer in the whole exe, the match loading screen (0x22052A), so it is NULL until the game has loaded its first match: access violation reading address 0. A game started anew after its player's match (the game was closed or died in it) is such a game. srp sends 0x97 only to a session the lobby has sent into a match (`Core::sent_to_match`, `show_results`); the others get the match's notices and no screen.

---

## 7. Summary: unchanged / changed / new vs 0.21d, and srp changes

| 0.26g | 0.21d | status | srp-021d code to change (same paths in srp-026g) |
|---|---|---|---|
| S->C 0x97 match stats | `36 13` | **changed**: id; +u32 unknown, +2 x u32 team params, +u8 game_mode after the scores; clan tag after each title name; support title has 1 u16; rows gain a tag string; full redesigned (1.3) | `crates/survarium/src/v021d/results.rs`: `write_match_stats` (new fields, tags, one support u16), `write_full` (rewrite: id, 3 components, 3 bonus blocks, elo, quest money/rep, faction, 3 play times, nA=0, drop descriptor, rarity, 4 f32), `match_stats` (id 0x97, no 0x36 prefix); `crates/session/src/reports.rs` `MatchReport::screen_for` |
| S->C 0x93 | `36 0E` | **changed meaning**: full (ignored) + account/profiles block of the clicked player | `results.rs` `player_match_stats`; `reports.rs` `player_details` must build the clicked player's profiles (0x87 writer) |
| S->C 0x98 read_drop | - | **new** | new writer; inventory/drop-card logic in the lobby (`servers/lobby-server/src/lobby_server/actions.rs`) answering `46 03` |
| S->C 0x99 daily bonus | - | **new** | new writer (lobby sign-in pushes) |
| S->C 0x9B craft tables | - | **new** | new writer (fixed 112 bytes) |
| C->S `41 00 u32` | `21 13 u32` | changed id/framing | `servers/lobby-server/src/lobby_server/mod.rs` query match (`MATCH_STATS` arm) |
| C->S `3D 00 u32 u32` | `21 0E u32 u32` | changed id/framing | same file (`PLAYER_MATCH_STATS` arm) |
| C->S `3C 00` after 0x97 | (`21 0D`) | new trigger | answer like the quests query |
| C->S `46 03 u32 u32`, `42 00 u32`, `43 00` | - | new | lobby handlers |
| `#mstats [N]` line | 0xC9 type 8 | same text; now 0xA2 sender_type 8 with kind != 3/4 | `results.rs` `stats_line` unchanged; the messaging writer must use 0xA2 framing |
| sign-in `pending_match_stats_id` | 0x26 offset 6 | **removed** | `servers/lobby-server/src/lobby_server/mod.rs` sign-in parse (field at line ~30/45) must go; push 0x97 after sign-in from `Reports::unseen` instead |

---

## 8. Open questions

- S+0x8C0 (u32 after match_id): read, never used. Meaning unknown.
- `team1_param`/`team2_param` and `game_mode` values: what the AS3 shows (needs the 0.26g lobby SWF).
- ~~Score component name keys~~: settled, §9. The meaning of the u8 "bonus" fields: the screen prints them as `(+N%)` beside the daily and equipment values (`VictoryScreen.updateTotalScore`), so percents (PROVEN use, the server's values unknown).
- Premium drop-bonus label threshold 12.5 vs chance values that are displayed x100.
- Descriptor u32 at +0x10 (non-stack, both modes) and +0x0C (mode 0): meaning (the 0x89 inventory analysis should settle it).
- 0x98: ~~what dict ids 0xD6/0xD7 are~~ (silver and gold, §3); Flash args of `use_drop_card`; the server reply to `42`/`43`.
- 0x93 trailing block: full layout = 0x87 profiles format (owned by the 0x87 analysis).
- Whether 0x3C really is "query quests" (only its call sites are proven).
- 0x9B row/column meaning.
- When the client opens the victory screen at match end in 0.26g (0.21d: `close_current_match(3)` -> `show_match_statistic`); not traced here.

---

## 9. Settled for the rewards code (added 2026-10-12)

Code: `crates/session/src/v026g/rewards.rs` (rules and the choices srp made are in its module text).

- **The client adds the end-of-match events itself** (PROVEN). 0.26g `shared_statistics::finish_match` is 0x2F4CF0 (called from the statistics handlers 0x26D9C0 and 0x278CB0 with the mask at core+0x18FD8). For each roster place: if its bit is in the mask given or in `m_active_in_match_players_mask` (+0x31C4): `events[11][0]++` (participating, 0x2F4D4A); if its team (profile +0x45EC) is the winner (+0x31DC): `events[12][0]++` (won, 0x2F4DE1), **whether or not it is still in the match**; if its protection value (profile +0x45F4) is 0: `events[27][0]++` (0x2F4E34). All three go to kit ("preset") 0. 0.21d's function (0x1DDB6F) also added event 16 for each battery of the winning team; 0.26g's does not. A snapshot uploaded before the end has none of them: the server adds 11 and 12 itself.
- **Event 25 is a grenade kill** (PROVEN): `on_player_killed` 0x2F4EB0 counts it at 0x2F514E when the weapon's category equals the first byte of the struct at [0x1F1DA1C], which 0x2CA5D0..0x2CA631 fills from `gameplay/item_categories`: `item_category_grenades` (24), then `item_category_booby_traps` (11). The same function counts events 0, 15 (headshot), 10 (carrier kill), 13 (melee), 8 (rescue), 9 (revenge), 4 (team kill) and 3 (suicide), so those keep their 0.21d numbers (PROVEN); it no longer counts 14 (silent kill). **Event 27**: given at the end of a match to a player whose protection value is 0 (above). 24 and 26 are battery events (solo delivery; stolen and brought home): snapshot_rules.md §3.1.
- **The three score components are the game's own** (PROVEN data): `gameplay/ui/victory_screen.options` has `categories` = `st_statistics_score_kills` (show_amount true, "Enemies killed"), `st_statistics_score_help` (true, "Help ally"), `st_statistics_score_game_events` (false, "Match Activity Rewards"), and `event_categories[28]`: events 0 and 4 → 0; 1, 7, 8 → 1; every other → 2. No string of that file is in the exe (it only reads what the server sends), so the file is LIKELY the server's own table shipped with the client.
- **How the screen adds up** (PROVEN, `VictoryScreen.as` `updateTotalScore`): total = the three components' money / experience / reputation over all columns + `quest_*` + the `premium_items` block + the `daily` block + the `premium` block **only if the client has premium** (`PlayerInfo.premiumState != 0`). Without premium the premium block is still printed (dimmed), beside the "activate premium" button. The premium line's tooltip is the constant `[50, 50, 50]` and the multiplier 1.5: premium is +50 %. So the components carry the plain values and the premium block the half.
- `match_result` 5 is a victory: the screen tests `match_result > 2`; anything else is "defeat" (there is no draw screen). `team1_param` / `team2_param` are read only when `game_mode == 4` and both team scores are 0, as the maximum of two "last players" bars whose values are the teams' row counts.
- **Send what the match changed of the account before 0x97** (PROVEN, `LobbyMenu.fill_victory_screen`): when 0x97 arrives the screen reads the experience bar from `PlayerInfo.experienceCurrent/Next` (0x8C) and the reputation bar from the quest window's factions (0x8E) for `faction_id`; an unknown faction (0) gives an empty bar, no crash.
- The premium drop-chance label: the `> 12.5` test at 0x29D51F is on the raw f32 (F+0xD8), which is then shown × 100 (0x29D5A2). A fraction can never pass it, so a fraction always gets the "Premium players in match" label (`st_item_drop_chance_team_premium`), never "Premium account" (`st_item_drop_chance_by_premium`). Since 2026-10-12 srp sends the premium account's own 0.25 there (§10): the number and the total are right, the label is the client's.
- 0x98 is the answer to a drop card only. The item a match dropped is inside 0x97; the inventory push (0x89) that follows brings the item itself. Sending 0x98 for it as well would count a stack twice (§3 effect 1).

---

## 10. What a match pays (added 2026-10-12)

Code: `crates/session/src/v026g/pay.rs` (every number, each with its tag) and `rewards.rs` (how they are applied). The sources are in `progression_sources.md`; two results screens of the owner's fix the numbers:

| | SHOT1 (0.26) | SHOT2 (LIKELY 0.27) |
|---|---|---|
| match level (`match_levels_dict`) | 1: `money_k` 1.47, exp / rep 1 | 15: `money_k` 10.17, exp / rep 1.7 |
| match | win 5:4, score 223, 17 kills, 4 helps | win 8:2, score 289, 19 kills, 8 helps |
| Enemies killed | x18: 551 / 41 / 17 | x19: 4830 / 96 / 32 |
| Help ally | 4: 58 / 4 / 4 | 8: 813 / 13 / 13 |
| Match Activity Rewards | 463 / 143 / 5 | 5064 / 270 / 11 |
| Daily bonus (+100 %) | - / 188 / 26 | (no such row) |
| Premium (+50 %) | 536 / 94 / 13 | 5353 / 189 / 28 |
| Total | 1608 / 470 / 65 | 9784 / 568 / 84, after 6276 of expenses |

(money / experience / reputation.)

**The rule.** Each cell of a row = its plain amount × the match level's multiplier, **rounded down after multiplying** (PROVEN by SHOT2: 8 × 10 × 10.17 = 813.6 → 813; 8 × 1 × 1.7 = 13.6 → 13; 19 × 25 × 10.17 = 4830.75 → 4830; 19 × 3 × 1.7 = 96.9 → 96; 19 × 1.7 = 32.3 → 32). The multipliers are used as the data's decimals (the file's f32 for 1.05 is a hair under 1.05).

| Row | Plain amount | Tag |
|---|---|---|
| Enemies killed | 25 silver, 3 experience, 1 reputation a kill | PROVEN for SHOT2's kind of kill. SHOT1's 17 kills are 375 / 41 / 17 before the multiplier: exactly 7 kills of 25 / 3 and 10 of 20 / 2. So there is a lesser kind of kill, **cause unknown**; srp pays every kill 25 / 3 / 1 (`pay::LESSER_KILL` is kept, unused) |
| Help ally | 10 / 1 / 1 an assist | PROVEN (both screens). A teammate cured or rescued is in the same row of `victory_screen.options` and is paid the same: LIKELY |
| Match Activity Rewards | fixed part 82.73 / 122.69 / 2.46 for a won match played to the end, and 7.038 / 0.615 / 0.077 for each point of other score | GUESS: the line through the two screens (SHOT1: 315 / 143 / 5 at other score 33; SHOT2: 498 / 159 / 6.5..7 at 59). Both are wins, so the split of the fixed part is unknown |

- **Other score** = the score of the events of the row itself (every event that is no kill and no help; `event_categories` value 2), with the weights the match was played with. For a player whose helps are all assists that is "score − 10 a kill − 5 an assist", which is how both screens were read.
- **A match not won** (lost, drawn, or with no known result) and played to the end pays half of the fixed part and the same for each point; **a player who left early** gets nothing of the fixed part. The owner's choice (2026-10-12), until a lost match's screen is seen.
- **Rates** (srp's): the rows' silver × the percent of the match's level (`pay::MONEY_RATES`, the owner's choice of 2026-10-14: 150 to level 5, ten more a level, 220 from level 12; 120 at every level from 2026-10-13, 80 before; `SRP_MONEY_RATE` sets one percent for every level); their experience × `SRP_EXPERIENCE_RATE` (default 2 since 2026-10-14; 1, the original's pace, before); their reputation × `SRP_REPUTATION_RATE` percent (default 120). §13.
- **Daily bonus** = the rows' experience and reputation once more, no silver; **premium** = half of the rows, of all three, not of the daily bonus; total = rows + daily + premium (PROVEN: SHOT1's 188 + 188 + 94 = 470, 26 + 26 + 13 = 65; SHOT2's 379 + 189 = 568).
- **The money total is net of expenses in the client** (PROVEN, `VictoryScreen.updateMoney`): it adds the autobuy and repair lists of the inventory push (0x89) to the sum of the rows, quests and bonus blocks. 0x97 carries the gross rows; SHOT2's 9784 = 10707 + 5353 − 6276.
- **The "x18" of SHOT1** against 17 kills on its scoreboard: the row's events are 0 (kill) and 4 (team kill), so the 18th is LIKELY a team kill, which paid nothing. It does not explain the lesser kind of kill (ten of them). srp's amount counts kills only [choice].
- **Score weights** (`static_info.rs` `SCORE_WEIGHTS`): kill 10, assist 5, stolen battery 35 (the wiki of February 2015; the stolen battery is event 17, LIKELY); the others are srp's 0.21d choices divided by ten.
- **Quests**: a quest's silver × `money_k` and its reputation × `reputation_k` of the match it was completed in (that the level matters is PROVEN, patch note 0.23h; these multipliers LIKELY), each rounded down; its reputation twice in one of the day's doubled victories (LIKELY, patch note 0.25a). A 0.26g quest awards no experience. The silver rate does not touch a quest's silver; `SRP_REPUTATION_RATE` does raise its reputation (× the percent, in the same rounding down, before the doubling).
- **Drop chance**, the four f32 of §1.3: base = the table's 40 % (with the play time bonus only when `SRP_DROP_PLAYTIME_BONUS=on`; off by the owner's choice of 2026-10-13, see §12); match result = 2.5 % for each battery the player's own team has at the end (modes 2 and 3; LIKELY); premium = 25 % for a premium account (LIKELY); penalty 0. The parts are cut so that their sum is at most 1, and the roll uses the sum.
- **A weapon or armour found after a match** arrives at 40 % condition (`SRP_DROP_CONDITION`; the value is the owner's choice) with one defect (`SRP_DROP_DEFECT`), drawn by drop weight from the defects the item may have (`Modifications::defect_candidates`). Source: patch note 0.24a, "all the items found after a match will be in poor condition with critical malfunctions". Stacks, vouchers and cosmetics are as before, and so is an item of a drop card.
- **Levels**: 100 is the highest; reaching it pays 250 gold (the wiki's table). **Rating** starts at 100 for a new 0.26g account (LIKELY); accounts made before keep theirs.
- **The seven days** (`bonus.rs`): a day counts when its three victories are made, a day missed or not made does not reset the run, and the reward comes with the third victory of the seventh such day (patch notes 0.24a, 0.24c). `SRP_DAILY_BONUS=off` turns the bonus off (no 0x99 is sent); `SRP_DAILY_BONUS_RESET_HOUR` moves the start of a bonus day.

## 11. What of the results outlives a restart (added 2026-10-12)

Until 2026-10-12 `rewards::Results` was in memory only. A restart (every update is one) forgot:
the results screens not shown yet, with their notices (a level reached, quests) and the clan
experience that is given when the screen is shown; the premium half that can still be claimed
from a screen (§5.4); and each account's play period, so the next match's drop chance was a
first game's again. The accounts always kept what a match gave them.

Now (`Results::attached`, `crates/session/src/v026g/persist/results.rs`):

- Table `g26_results (key TEXT PRIMARY KEY, body TEXT NOT NULL)`, schema step 6 of the `g26_*`
  tables. Additive: a database of before has no rows.
- `report:<match id>`: one finished match's `MatchReport` as JSON (serde; the lobby's results
  types in `survarium` and the reward types in `session` derive it; a kit's 68 modifiers are a
  list). The last 64 are kept, as in memory; a report that goes is deleted.
- `state`: who has not been shown which match, the notices of reports that went, the play
  periods (`account, when its last match ended, games`; one that ended a whole period before
  the newest is left out: it is over whenever its account plays next), the premium halves
  claimed.
- Written at `GameAccounts::flush` after the accounts (`GameAccounts::on_flush`): the saver's
  pass every 2 s, the flush after a match is settled, and the one at the end of srp. So a
  report is never kept ahead of what its match gave the accounts.
- Read at start (`GameRepository::load_results`). A row that does not read (other code, a
  damaged file, a struct that changed) is left out with a warning and cleaned up at the next
  save: a screen is lost, never anything an account was given.
- Not kept: which screens were open (a client's lobby link does not outlive srp, so no screen
  does).
- With `SRP_DB=off` nothing is kept, as before.

Tests (`rewards/tests.rs`): `a_restart_keeps_the_screens_not_shown_and_the_play_periods` (the
reports equal to the last number, the claim not paid twice, the third match's drop chance and
reward those of no restart), `the_game_of_the_play_period_is_counted_across_a_restart`,
`only_the_reports_kept_stay_in_the_repository`,
`rows_that_do_not_read_are_left_out_and_cleaned_up`,
`results_attached_to_accounts_in_memory_are_plain_ones`; `persist/results.rs`:
`the_rows_outlive_the_repository`, `a_database_of_before_gets_the_table`.

## 12. The economy changes of 2026-10-13 (the owner's choices)

What the live server's first two days showed (254 results of players who finished, 67 matches, match levels 0 to 4; 66 accounts that had played): a match paid 1,162 silver on average at the 80 % rate, quests 189 more; wear cost 44 silver a result at the shop's repair price, ammo 78 at the shop's price, defects 16. So the costs were about 12 % of the pay, because most players still used the free starting weapons and their free ammo. With bought gear they grow faster than the pay does (a level 15 weapon costs eleven times a level 3 one, the level's `money_k` is four times): a projection for a level 15 kit and 266 rounds a match (the live average) gave wear and ammo as about two thirds of the pay. 73 % of the results had a drop.

| Change | Before | Now | Where |
|---|---|---|---|
| Match silver | 80 % of the original | **120 %** | `pay::DEFAULT_MONEY_RATE`, `SRP_MONEY_RATE` |
| Wear of the weapons and armour played with | the game's (`degrade_per_match`, `degrade_per_use`), and a defect roll for a worn item | **none**; `SRP_ITEM_WEAR=on` brings the game's rule back | `pay::item_wear`, `rewards::Terms::wear` |
| Items worn before the switch | - | made whole once: full condition, defects off, repairs over (`g26_meta` key `gear_mended`) | `GameAccounts::from_env`, `items_rules::mend_all` |
| Ammo | the game's `base_cost` a clip | **about half**, each price set by hand and rounded down to a multiple of 5 | `prices::AMMO_PRICES` |
| The play time bonus to the drop chance | the table's 40 % times 2, 1.75, 1.5, 1.25 for the first four games of an hour of play (srp's GUESS at the data's `playtime_dependence`) | **not applied**; `SRP_DROP_PLAYTIME_BONUS=on` brings it back | `pay::drop_playtime_bonus`, `rewards::Terms::playtime_bonus` |

Not changed: a weapon or armour piece found after a match still arrives at 40 % with one defect (`SRP_DROP_CONDITION`, `SRP_DROP_DEFECT`), and stays so until its owner repairs it; the batteries' and premium's parts of the drop chance; quest silver, level rewards, the gold exchange; the prices of drugs, traps and grenades.

What follows from them:

- With wear off the auto-repair box has nothing to do after a match, and a repair is needed only for a found item (and a "reliability" modification, which lessens wear, does nothing).
- **The kit's ammo window shows the game's own price** for a stack the player owns, and limits what can be added by it: an inventory item's cost is the client's own `base_cost` (`InventoryList`: `item_data.cost = descrData.cost`), only the shop's items carry the price the server sent (`GangShop.setupShopItems`). PROVEN in the ActionScript. The server takes its own price, so the player is charged half of what that window shows; the shop, the results screen's "Ammunition" list and the balance show the real one. Only a change of the client's data would make the window agree.
- Selling ammo back pays a quarter of the game's price (the lobby shows it from the same `base_cost`); no price of the table is under that quarter (test `ammo_costs_the_owners_prices`).

Tests: `rewards/tests.rs` `with_wear_off_a_match_leaves_the_items_whole`, the bonus-off part of the play period test, `the_money_rate_scales_the_rows_silver_and_premiums_half_of_it` (srp's own terms); `v026g/mod.rs` `mending_makes_weapons_and_armour_whole_and_leaves_the_stacks`; `persist.rs` `a_first_time_is_once_in_the_databases_life`; `prices.rs` `ammo_costs_the_owners_prices`.

## 13. The pay changes of 2026-10-14 (the owner's choices)

What two days of the players' server showed is in `progression_pace.md`: the low tiers are the quick ones, a tier's whole kit costs 20 to 26 matches of silver up to tier 6 and 53 to 72 from tier 12, and for a whole kit the silver is what a player waits for at every tier (for a weapon alone, the rank).

| Change | Before | Now | Where |
|---|---|---|---|
| Match silver | 120 % of the original at every match level | **by the match's level**: 150 % at levels 0 to 5, then 160, 170, 180, 190, 200, 210, and 220 % from level 12 | `pay::MONEY_RATES`, `pay::MoneyRate`; `SRP_MONEY_RATE` set = that percent at every level |
| Reputation of a match's rows and of the quests completed in it | the original's | **120 %** (the owner: "increase reputation a bit for all"; the number is srp's reading of "a bit") | `pay::DEFAULT_REPUTATION_RATE`, `SRP_REPUTATION_RATE`, `quests::Scale::reputation_share` |
| Experience of a match's rows | the original's | **twice** (the owner: "x2 exp") | `pay::DEFAULT_EXPERIENCE_RATE`, `SRP_EXPERIENCE_RATE` |
| What `SRP_EXPERIENCE_RATE` multiplies | a match's experience and reputation | **its experience only**; the reputation has its own rate | `rewards.rs`, the rows' cells |

Not changed: quest silver, level rewards (so their silver and gold come twice as fast), the level table, prices, the drops.

What follows from them:

- The percent is the match's, as `money_k` is: every player of a match is paid at its level's percent, whatever gear each wears. No match of the first two days was above level 5, so until matches are, only the 150 % is felt.
- Past 220 % the model has the Renaissance rank arrive after the silver for tiers 12 to 15: more silver there would buy nothing sooner.
- **The game's own numbers for a quest are the data's**: the quest window's award and the "Mission accomplished" card are counted by the client and do not know the 120 %; the results screen's quest reputation, the reputation bar and the rank are the server's and do. (The card already differed from what is paid: it uses the multiplier of the player's own level, the pay the match's.)
- The daily bonus and premium are counted from the rows, so they follow all three rates: one of the day's first three victories now pays four times the original's experience.
- The account level was the longest road: at the original's pace level 20 took about 67 hours of play and the skill tree's 81 points about 750; twice the experience makes them about 38 and 390 (`progression_pace.md` §7).

Tests: `pay.rs` `the_silver_rate_rises_with_the_match_level`; `rewards/tests.rs` `the_owners_rates_raise_silver_by_match_level_experience_and_reputation`, the own-terms part of `the_money_rate_scales_the_rows_silver_and_premiums_half_of_it`; `quests.rs`, the raised part of the scale test.
