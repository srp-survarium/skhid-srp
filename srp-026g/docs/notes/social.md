# 0.26g social: chat, friends, squad, clan, misc pushes

Version scope: Survarium 0.26g (build 2777, `survarium.exe` from downloads/v026g) compared with 0.21d (build 2010, PDB symbols). Static analysis only. 0.26g VAs are from the dumpbin disassembly; 0.21d VAs/symbols are from `srp-021d/docs/notes/lobby_spec.md` and the 0.21d labelled disassembly.

Tags: **PROVEN** = read in the 0.26g disassembly at the cited VA (and, where named, matched to the 0.21d symbol/VA). **LIKELY** = strong inference (names, call graph) not fully traced. **GUESS** = plausible, unverified.

Conventions: `u8/u16/u32` little endian; `string` = `u8 len` + `len` bytes, no NUL (reader memcpy 0x424580 then NUL; writer 0x26AB90 writes only the low byte of strlen). Readers never bounds-check (as in 0.21d): a short message reads past the buffer, and a string longer than the destination buffer overflows it. "buf N" below = destination buffer size including the NUL, so the safe maximum is N-1.

Status: complete for the areas listed; open points in section 7.

---

## 1. Messaging is on the lobby connection

PROVEN: there is no separate messaging socket or messaging sign-in. `lobby_client::on_message_received` 0x274A00 routes ids 0xA2..0xA4 to the messaging dispatcher 0x26ABE0 (`messaging_client_process_messagess.cpp`), which logs `messaging_client received unknown message:%d` for anything else (0.21d: `messaging_client::on_packet_received` 0x19AD5A, ids 0xC9/0xCB/0xCC). Client→server messaging ids are 0x4C..0x50, created with the same lobby message factory 0x37A0E0 (0x50 through 0x378080) and sent on the lobby connection 0x379640.

The messaging_client object is lobby_client+0x1E56C4 (constructor 0x26C170). Fields used below:

| offset | meaning |
|---|---|
| +0x0C..+0x14 | friend list vector, element 0x5C bytes |
| +0x1C..+0x24 | ignore list vector, element 0x5C bytes |
| +0x2C..+0x34 | player-search result vector, element 0x5C bytes |
| +0x40..+0x48 | pending important messages, element 0xA8 bytes (message_id at +0xA0) |
| +0x4C..+0x70 | u32 channel_id[10], indexed by channel number (see 1.6) |

Friend/ignore/search element (0x5C): +4 u32 account_id, +8 char name[64], +0x48 char clan_abbr[16], +0x58 u32 status.

### 1.1 Channel enum (`message_channel_enum`) — PROVEN

From the chat tab descriptors (0x2093A0 general, 0x209583 team/match, 0x209FF0 squad, 0x20A0D0 clan, 0x20A1A0 clan team; channel arrays at 0xA74A1C, 0xA74A78, 0xA74A88, 0xA74A98, 0xA74AA8, 0xA74AB8) and the 0x4C sender switch (table 0x26AF8C):

| ch | name | tab | channel_id sent in 0x4C |
|---|---|---|---|
| 0 | (none) | — | 0; not censored on receive |
| 1 | general (`st_chat_channel_general`) | general | channel_id[1] = language room (1 = russian/ukrainian client, 2 = any other) |
| 2 | system | general, team | 0 |
| 3 | clan (`st_chat_channel_clan`) | clan | channel_id[3] = clan_id; **not sent if 0** |
| 4 | private | general | 0 |
| 5 | match (`st_chat_channel_match`) | team, squad | lobby_client+0x1E57A4 (match id); not sent if -1 |
| 6/7 | team (`st_chat_channel_team`) | team | match id; channel byte = 6 + (lobby_client+0x1E57AC != 0); not sent if match id -1 |
| 8 | squad (`st_chat_channel_squad`) | squad, team | channel_id[8] = squad_id; not sent if 0 |
| 9 | clan team (`st_chat_channel_clan_team`) | clan team | channel_id[9] = clan team id; not sent if 0 — **NEW** |

0.21d had 0..8 with the same meanings (lobby_spec 5.4); channel 9 is new.

### 1.2 0xA2 chat text (server → client) — was 0xC9

Handler 0x26B6F0 (0.21d `process_incoming_text_message` 0x19B858). PROVEN layout:

| # | type | field | note |
|---|---|---|---|
| 1 | u8 | sender_type (`client_type`) | 5 = account, 8 = stats_processor, 9 = clan service (see below) |
| 2 | u32 | sender_id | if sender_type == 5 and sender_id is in the **ignore list** (+0x1C, lookup 0x26C010), the handler returns here: nothing else is read, message dropped (as in 0.21d) |
| 3 | string | sender_name | stack buf 64 → ≤ 63 |
| 4 | string | sender_clan_abbr | **NEW, only when sender_type == 5**. Stack buf 16 → ≤ 15. Shown as `[%s] %s` by add_message 0x208AD0 (`abbr`) |
| 5 | u8 | channel | 1.1 |
| 6 | string | body | stack buf 256 → ≤ 255 |

Crash hazards: name > 63 overflows into the abbr buffer; abbr > 15 overwrites the saved sender_type and channel, then the body pointer slot (stack corruption). For sender_type ≠ 5 the abbr buffer is uninitialised stack and is still passed to add_message (GUESS: harmless if it happens to start with NUL; send players as type 5).

Dispatch after reading (PROVEN, 0x26B7F4..0x26B923):
- sender_type 8, channel 3 → clan system line 0x26B940 (below).
- sender_type 8, channel 4 → `lobby_menu::on_additional_drop_arrived(body)` 0x225950: body `[a][b]` (tokens between `[` and `]`, strtol); b == 0 or 2 → client sends `42 00 <u32 a>`; b == 1 → client sends `43 00`. NEW (drop queries; ids are in the other agent's list).
- sender_type 8, any other channel: if body contains `#mstats` → `on_stats_message_arrived` 0x225810: N = number between the first `[` and `]` (≤ 15 chars), client sends `41 00 <u32 N>` (old `0x21 13 N`); otherwise dropped. Same rule as 0.21d 0x1765A5, new query id.
- sender_type 9, channel 3 → clan system line 0x26B940. NEW.
- everything else: censor (option byte 0xA70EC7; never for channel 0) → `add_message(channel, body, name, abbr)` 0x208AD0 → `add_to_recent_list(name)` 0x209330.

Clan system line 0x26B940 (NEW, PROVEN): body = `<k>\x0E<arg1>[\x0E<arg2>]` (separator byte 0x0E, k one decimal digit, atoi). The shown sender is the localised `st_clan_system_message_sender`; the line goes to the received channel (3):

| k | text built (each `st_` localised) |
|---|---|
| 0 | `<arg1> st_clan_member_joined` |
| 1 | `<arg1> st_clan_member_left` |
| 2 | `<arg1> st_clan_member_kicked <arg2>` |
| 3 | `st_clan_level_up_1 <arg1, first 7 chars> st_clan_level_up_2` |
| other | body shown unchanged |

### 1.3 0xA3 friendship status (server → client) — was 0xCB, CHANGED

Inline in 0x26ABE0: `u8 action`, then by action; afterwards `lobby_menu` 0x21D990(action) refreshes Flash: 6 → `root.fill_players_search` (0x28BBF0), 7 and 9 → `root.set_friends_list` (0x28B540), 8 → `root.set_ignored_list` (0x28B910), any other → nothing. PROVEN.

| action | reader | body | store |
|---|---|---|---|
| 7 query_friend_list | 0x26C540 | `u16 n`, n × {`u32 id`, `string name (≤63)`, `string clan_abbr (≤15)` **NEW**, `u8 status`} | replaces friend list (+0x0C); status → +0x58, passed raw to Flash `status` |
| 9 update_friends_status | 0x26C4A0 | `u16 n`, n × {`u32 id`, `u8 status`} | if n ≠ friend count held: **returns without reading and without re-querying** (0.21d re-queried with C4 07). An unknown id stops the loop silently |
| 8 query_ignore_list | 0x26C620 | `u16 n`, n × {`u32 id`, `string name (≤63)`} | replaces ignore list (+0x1C) |
| **any other** (6 find_players, and 0..5) | 0x26C6C0 | `u16 n`, n × {`u32 id`, `string name (≤63)`, `string clan_abbr (≤15)`} | replaces the search list (+0x2C) |

**Breaking change:** in 0.21d actions 0..5 carried `u8 result` (`'4'` = OK). In 0.26g every action other than 7/8/9 is parsed as a search list, so an old-style `A3 00 34` makes the client read `u16 n` from `34 ??` and run off the buffer (crash). The client also never asks for its lists (there is no 0x4E 07/08/09 sender: PROVEN from the five 0x4E call sites), so the server must **push** `A3 07` and `A3 08` at sign-in and after every change, and `A3 09` for status changes (n equal to the friend count the client holds). PROVEN (layouts, no query sender) / LIKELY (the real server pushed full lists).

Overflow: name > 63 runs into clan_abbr of the same 0x5C element, abbr > 15 into status; for the last element of the vector, > 19 abbr chars write past the element (heap). Counts are unchecked u16 (vector resize 0x26C790).

### 1.4 0xA4 important message (server → client) — was 0xCC, CHANGED

Reader 0x26BBF0 (0.21d 0x19B964). PROVEN layout:

| # | type | field | note |
|---|---|---|---|
| 1 | u8 | sender_type | |
| 2 | u32 | sender_id | now **used**: it is the inviter id the client sends back when answering clan / clan-team invites (types 7, 10) |
| 3 | string | sender_name | buf 64 → ≤ 63 |
| 4 | string | sender_clan_abbr | **NEW, always present**, buf 16 → ≤ 15 |
| 5 | u8 | type | `important_message_types`, table below |
| 6 | string | text | buf 256 → ≤ 255 |
| 7 | u32 | message_id | if already pending (vector +0x40, lookup 0x26C0B0) the whole message is ignored; else a 0xA8-byte record is kept |

Then `lobby_menu::important_message_arrived(message_id, type, text, name, abbr)` 0x226020, then the messaging side (table 0x26BDA0): types 1, 4, 5 → auto-answer accept (`50 <id> 00`); type 3 → auto-accept and `4A 02 <sender_name>` (0x2745D0); types 0, 2 and ≥ 6 → no auto answer from here.

Types (lobby_menu 0x226020, PROVEN unless marked):

| type | meaning | client action |
|---|---|---|
| 0 | friend request | Flash `root.add_important_message` with `st_add_to_friends_request`, sender `[abbr] name`; waits for the user |
| 1 | quest | text `#+q` (new) / `#!q` (failed) / `#q!` (completed); queries 0x3E / 0x40, 0x39, 0x3E / 0x3C via 0x26FE20 (other agent); auto-accept |
| 2 | squad invite | `st_add_to_squad_request`; waits |
| 3 | squad invite accepted | no UI; auto-accept, then `4A 02 <sender_name>` |
| 4 | reputation level unlocked | text `#ru…`; queries 0x40, 0x39 (0x2265F6, 0x226606; **corrected 2026-10-12**: the exe has no writer for 0x3A, `coverage.md` §1.2); auto-accept |
| 5 | drop arrived | log only; auto-accept |
| 6 | item repaired | text `#ir[<item id>]` (PROVEN: the bracket parser 0x227120; the item's `repair_time` is set to 0, 0x22693A); inventory refresh (0x288520). Not auto-answered (GUESS: stays pending; server should not expect `50`) |
| 7 | **clan invite** NEW | `st_add_to_clan_request` + `%s "%s"` (LIKELY text = clan name); waits |
| 10 | **clan team invite** NEW | `st_add_to_clan_team_request`, `[%s] %s`; waits |
| 13 | **referral reward** NEW | Flash nickname/level/reward/reward_type parsed from text; answers itself (`50 id 00`) and queries 0x37 |
| 14 | **skill point reward** NEW | skill_level/reward_type/reward_id/reward_amount from text; answers itself and queries 0x37 | **Corrected 2026-10-12, see premium.md §7: the text is `[skill_level][reward_type][reward_id][reward_amount]`.** |
| 8, 9, 11, 12, ≥ 15 | — | nothing; record stays pending |

User answer (`important_message_read` 0x28C5E0, PROVEN): removes the Flash entry. Accept (0): type 2 → `4D` type 3 `membership accepted` to the inviter's name; type 7 → `49 0A <u32 sender_id> 00`; type 10 → `49 1A <u32 sender_id> 00`. Decline (1): type 7 → `49 0A <sender_id> 01`; type 10 → `49 1A <sender_id> 01`. Always followed by `50 <u32 message_id> <u8 answer>` (0x26B1B0), which **fatally asserts** `found != m_important_messages.end()` if the id is not pending (0x26B211).

### 1.5 Client → server messaging

Every sender first checks the lobby connection (lobby_client+0x1E55C8 → state at +0xB00A0+0x154 is 0 or 4); otherwise it does nothing (0x4C prints `not connected to messaging server...` as a channel-2 `System` line).

| new id | old | payload after the id | sender (0.26g) / callback |
|---|---|---|---|
| 0x4C | 0xC1 | `u32 channel_id`, `string receiver`, `u8 channel`, `string body (≤255)` | 0x26AD30 (chat input 0x208A71/0x208AB5); local echo is shown before sending; channel_id rules in 1.1. PROVEN |
| 0x4D | 0xC2 | `u32 0`, `string target_name`, `u8 type`, `string text` | 0x26AFB0: `add_friend` (picked from search list, type 0, text ""), `add_friend_name` (type 0), `invite_to_squad` (type 2, text `invite`), squad invite accepted (type 3, `membership accepted`). PROVEN |
| 0x4D (long) | — | `u32 0`, `string target_name`, `u8 type`, `string text`, **`u32 extra`** | 0x26B0A0, NEW: type 7 clan invite / type 10 clan-team invite; sent only after the server permits it (`83 49 0F` / `83 49 19`, 3.4), with target, text and extra copied from that reply. PROVEN |
| 0x4E | 0xC4 | `u8 action`, then: 2 remove_friend `u32 id`; 3 add_ignorable `u32 id`; 5 remove_ignorable `u32 id`; 4 add_ignorable_by_name `string`; 6 find_players `string` | 0x26B470 `remove_friend`, 0x26B510 `add_ignore` (skipped if already ignored), 0x26B5C0 `remove_ignored`, 0x26B3E0 `add_ignore_name`, 0x26B660 `find_players`. Actions 0, 1, 7, 8, 9 are never sent. PROVEN |
| 0x4F | 0xC5 | `10 × u32 channel_id[0..9]`, `u8 language` | 0x26C320, 1.6. CHANGED |
| 0x50 | 0xC6 | `u32 message_id`, `u8 answer` (0 accept, 1 decline) | 0x26B1B0 (built with 0x378080). PROVEN |

### 1.6 0x4F channel subscription — was 0xC5, CHANGED

0x26C320 copies the 40 bytes at messaging+0x4C, then appends one byte (PROVEN):

| index | initial (ctor 0x26C170) | updated by |
|---|---|---|
| 0 | 0 | — |
| 1 | general room: 1 if `g_language` is `russian`/`ukrainian`, else 2 | — |
| 2 | 0xFFFFFFFF | — |
| 3 | 0 | clan_id from 0x9A (0x2721FF full info when the clan is registered, 0x27273D daily_message). **Not reset** when 0x9A full arrives with clan_id 0 |
| 4 | 1 | — |
| 5 | 0 | match id (0x26C2E0, from 0x82 connect_to_match) |
| 6 / 7 | 0 | match id in slot 6 if team == 0, in slot 7 if team ≠ 0 (0.21d kept both 0 for team 3) |
| 8 | 0 | squad_id from 0xA1 (0x2747B7) |
| 9 | 0 | clan team id from 0x9A clan_team_change (0x272D48) — NEW |

Trailing **u8 language** (NEW; 0.21d had only slot 1): `russian` 1, `english` 0, `ukrainian` 2, `german` 3, `polish` 4, anything else 1 (0x26C36E..0x26C47F).

Sent at sign-in (0x26FBE2, right after 0x80 is read and before the client's `32`), and whenever one of the slots above changes value. Match slots are only rewritten when the new match id is not -1 and differs, so they are never cleared after a match (0x26C2E0).

---

## 2. Squad

### 2.1 0xA1 squad info (server → client) — was 0x3A, layout UNCHANGED

`lobby_client::read_squad_info` 0x274720 (string at 0xA079D8; 0.21d 0x19EE6D). The squad block is lobby_client+0x1EA9B0, zeroed (0xFC bytes) on every 0xA1. PROVEN:

| type | field | stored |
|---|---|---|
| u32 | squad_id | +0x1EAAA4. 0 → removes the squad chat tab (0x20A070) and **nothing else is read** (message = `A1 00 00 00 00`). A new non-zero id adds the squad tab (0x209FF0). If it differs from messaging channel_id[8] the client re-sends 0x4F |
| u8 | n | +0x1EAAA8. **Unchecked; the array has 3 slots** (0x50 each, +0x1EA9B0..+0x1EAAA0). n = 4 writes member 3 over revision/squad_id/n itself |
| u8 | commander_index **(meaning now PROVEN)** | +0x1EAAA9; a non-commander's Play is gated on member[commander_index].ready (2.2). 0.21d stored it but did not use it |
| n × | member (0x50): `u32 account_id` (+0), `u32 profile_id` (+4), `u32 profile_revision` (+8), `string name` (+0x10, buf 64 → ≤ 63), `bool online` (+0xC), `bool ready` (+0xD) | |
| bool | i_am_commander | +0x1EAAAC |
| u32 | revision | +0x1EAAA0 (log `squad info arrived rev. %d`) |

Then `lobby_menu::fill_squad_info(squad, i_am_commander)` 0x2277C0 (PROVEN):
- `root.show_squad`, `root.remove_squad_player` 0..2, `root.set_squad_commander_mode(n == 0 ? true : i_am_commander)`, then per member `root.set_squad_player(index, …, name, ready)`.
- For each member whose profile_id is not one of the player's own profiles (profile ids at +0x754 + k×0x704): if the cached squad-mate panel has a different profile_id or revision, the client sends **`34 00 <u32 account_id> <u32 profile_id>`** (0x26FE70). 0.21d sent `21 10 <profile_id>`; the reply is 0x88 (CONTEXT; other agent).
- Then `update_play_button_state` 0x21FD40.

### 2.2 Play gating, `lobby_menu::update_play_button_state` 0x21FD40 — CHANGED

PROVEN, in this order (each failure logs a `PlayBtn disabled reason is: …` line):
1. lobby_menu+0x3138 (`pending_for_match_making_answer`) set → disabled.
2. client_state (lobby_client+0x1E5790, from 0x80/0x85) is 2 or 4 → disabled; state 1 with `place` (+0x1E579C) == 0 → disabled.
3. minutes_to_maintenance (lobby_menu+0x30D8, from 0x9F) ≤ 30 → disabled (`state=%d place=%d maintenance=%d`). Same rule as 0.21d (`> 30`).
4. **Clan team mode** (NEW) when lobby_client+0x1EE6D4 == 1 and in_clan_team (+0x1E56BC, from 0x9A type 4) is set. battle_cnt = clan-team members with role byte (elem+4) ≤ 1; team_ready_cnt = those with ready (elem+5) set (0x2742B0); min_cnt = +0x1EE5F1, max_cnt = +0x1EE5F2 (from 0x9C).
   - team commander (+0x1E56BD): enabled iff team_ready_cnt ≥ battle_cnt − 1 **and** min_cnt ≤ battle_cnt ≤ max_cnt (`commander=%d team_ready_cnt=%d battle_cnt=%d min_cnt=%d max_cnt%d team_commander_ready=%d`).
   - team soldier: enabled iff team_commander_ready == 0, where team_commander_ready = ready byte of the first clan-team member whose role byte is 0 (0x2742E0) (LIKELY role 0 = commander).
5. **Squad mode** when n > 0:
   - i_am_commander: enabled iff (number of members with ready set, the commander included, 0x2749D0) ≥ n − 1 (`I AM COMMANDER, BUT NOT ALL SQUAD SOLDIERS IS READY %d but %d`). 0.21d required == n − 1.
   - not commander: enabled iff member[commander_index].ready == 0 (`I AM SOLDIER AND COMMANDER IS READY`). **NEW**: 0.21d never gated a soldier.
6. Otherwise enabled.

So a server that marks the commander ready when the commander presses Play locks the soldiers' Play (they cannot cancel/ready once the squad is queued), and a commander can only press Play once every soldier is ready.

### 2.3 Squad actions (client → server) — were 0x2A 01..04, now 0x4A

PROVEN (senders 0x274580..0x2746D0; all require order/match id lobby_client+0x1E57A8 == -1, as in 0.21d):

| message | meaning | sent by |
|---|---|---|
| `4A 01` | destroy squad | `disband_squad` (0x2A5CE0); also 0x2A4D40 (tutorial start) when commander |
| `4A 02 <string name>` | add the player who accepted | automatically on important message type 3 (0x2745D0; also needs the connection state 0/4) |
| `4A 03 <u32 account_id>` | drop member (m+0 of 0xA1) | `remove_from_squad` (0x2A5D00) |
| `4A 04` | leave squad | `leave_squad` (0x2A5CC0); also 0x2A4D40 when not commander |

Invite flow is the 0.21d one on new ids: `invite_to_squad` → `4D` type 2 text `invite` → server relays `A4` type 2 → target accepts → target's client sends `4D` type 3 `membership accepted` to the inviter and `50 id 00` → server relays `A4` type 3 to the inviter → inviter's client auto-answers `50 id 00` and sends `4A 02 <name>` → server pushes new 0xA1 to every member.

No operation_permitted (0x83) case exists for op 0x4A (byte table 0x2751FC handles only 0x30/0x46/0x47/0x49/0x4B), so the server answers squad actions only with 0xA1.

### 2.4 How a squad mate's 0x88 is filed (added 2026-10-12) — PROVEN

The squad window keeps a panel per mate (lobby_menu+0x698, +0x69C). A panel holds copies of the mate's account body (0x4634 bytes each, from panel+0x18; the current one's index is the byte at panel+0xD2B4) and at **panel+4 the kit id it asked for**.

- `fill_squad_info` (0x227C59..0x227CE5), for each member of 0xA1 whose kit id is none of the player's own: it compares the member's revision (m+8) with the panel copy's first-kit revision (`[panel + i·0x4634 + 0x4614]`) and the member's kit id (m+4) with the **copy's first kit id** (`[panel + i·0x4634 + 0x76C]`). If either differs it writes the kit id to panel+4 and sends `34 00 <account> <kit id>`.
- `on_squad_member_profile_arrived` (0x222FE0) takes panel 0 when `[panel0+4]` equals the **first kit id of the arrived body** (`cmp edx,[esi+0x754]`), else panel 1, and stores the body there (0x284B10; the 0x88's trailing revision is compared at body+0x45FC).

So the body of a 0x88 must carry, **as its first kit, a kit whose id is the kit id 0xA1 gave for that mate**, and its trailing revision must be 0xA1's revision for the mate. Otherwise the answer lands in the wrong panel (always panel 1) and the client asks again on every 0xA1 and 0x87. A kit id is not unique across accounts in srp (each account's kits are 1, 2, 3), so srp sends a mate as **one kit, the one it plays, under the id `0x40000000 | account id`** (in 0xA1 and in 0x88), with a revision that moves when that kit changes or another kit is played (`servers/lobby-server/src/v026g/social.rs`).

---

## 3. Clan (all NEW in 0.26g)

### 3.1 lobby_client clan fields (PROVEN)

| offset | meaning |
|---|---|
| +0x1E5610 | char clan_name[64] |
| +0x1E5650 | char own clan_abbr[16] (also used as the abbr of the local chat echo) |
| +0x1E5660 / +0x1E56A0 | name / abbr last sent for validation (49 08 / 49 09) |
| +0x1E56B0 / +0x1E56B4 | member id pending for rank-up confirmation / kick confirmation |
| +0x1E56B8 / +0x1E56B9 | registration state flags (0x27242A) |
| +0x1E56BC / BD / BE | in_clan_team, i_am_team_commander, **my own ready flag** (0x9A type 4; corrected 2026-10-12, see section 8) |
| +0x1EAAB0 | clan members, 0x48 each, room for 105 (+0x1EC838 is next) |
| +0x1EC838 | ranks, 0x22 each, room for 10 |
| +0x1EC98C | clan_id |
| +0x1EC990 | u32 member count |
| +0x1EC994 | u32 money collected (max of received values) LIKELY |
| +0x1EC998 | u16 count of members with status byte 0 |
| +0x1EC99A | u16 (from full / member_act) |
| +0x1EC99C | u8 rank count |
| +0x1EC99D | clan info received / name reserved flag |
| +0x1EC99E | registration paid flag (set once) LIKELY |
| +0x1EC99F | clan registered flag ("full" clan vs registration in progress) |
| +0x1EC9A0 | clan-team members, 0x48 each, room for 100 (+0x1EE5C4 is the count) |
| +0x1EE5C0 | clan_team_id |
| +0x1EE5F0..F3 | 4 bytes from 0x9C: [1] min_cnt, [2] max_cnt for a clan-team battle |

### 3.2 0x9A read_clan_info (server → client) — NEW

`bool lobby_client::read_clan_info(buffer_reader&)` 0x271FD0. First byte `u8 clan_message_type` (switch table 0x272F4C); a value > 6 reads nothing more. Every type returns true, then 0x21D720(0x9A). PROVEN unless marked.

**type 0 — clan_message_type_full** (0x27200A):

| type | field | note |
|---|---|---|
| u32 | clan_id | **0 = no clan**: nothing else read; the client clears members, ranks, clan team, name/abbr, removes the clan and clan-team chat tabs, `set_clan_window_state(0)` |
| string | clan_name | stack buf 64 → ≤ 63; copied to +0x1E5610 (cut to 63) only if registered |
| string | clan_abbr | stack buf 80; copied to +0x1E5650 cut to 14 chars (0x1F5360) only if registered |
| u8 | registered | == 1 → +0x1EC99F |
| u8 | n_ranks | +0x1EC99C, **unchecked, room for 10** |
| n_ranks × | rank (0x22): `string name` (buf 32 → ≤ 31), `u8 permissions` (+0x20), `u8 is_default` (== 1, +0x21) | permissions: **PROVEN (corrected 2026-10-12, 0x2A11A0)** bit 0 = battle_commander, 1 = daily_message, 2 = change_rank, 3 = kick, 4 = invite (the reverse of the 49 10 codes) |
| u32 | n_members | +0x1EC990; loop counter is a u8, so n ≥ 256 never ends; **room for 105** |
| n_members × | member (0x48): `u32 account_id` (+0), `string name` (+8, buf 64 → ≤ 63), `u8 rank` (+7), then **if registered**: `u8 level` (+5), `u8 online` (+6, == 1 → `online`); **else**: `u8 status` (+4; status 0 counted in +0x1EC998) | Flash `root.set_clan_players_list` (0x2A03E0 registered: name/rank/default_rank/level/status; 0x2A01E0 registration: name/status) |
| u16 | ? | +0x1EC99A |
| u8 | flag | == 1; registered: window state 4 if set else 3 (0x2A1030); registration: selects the state handling below |
| if registered: | `u8 level`, `u32 a`, `u32 b`, `u32 c`, `u32 d` | `root.set_clan_progress` (0x2A1480) with the 5 values. GUESS: level, exp, exp_next, … |
| if not registered: | `u8 paid` (== 1 sets +0x1EC99E once), `u32 money` (kept as the max seen, +0x1EC994), `u8 reg_state` | flag 0 → window state 2. flag 1: reg_state ≥ 3 → window state 1; reg_state 0 / 1 / 2 → window state 0, sets +0x1E56B9 / +0x1E56B8 / neither and shows `st_clan_name_is_not_valid` / `st_clan_abbr_is_not_valid` / `st_clan_name_and_abbr_are_not_valid` (0x21FC20 switches on reg_state), +0x1EC99D cleared. UI: `set_clan_registration_paid`, `set_clan_creation_money_collected`, `set_clan_ready_players`, `set_clan_creation_enabled` |

Registered clan: adds the clan chat tab (0x20A0D0) and, if clan_id ≠ channel_id[3], re-sends 0x4F. Ends with `root.set_clan_name` / `set_clan_manage_permissions` (0x2A1100).

**type 1 — daily_message** (0x272652): `u32 clan_id` (changes the tab and channel_id[3] like type 0; 0 → stop), **`wstring text`** = `u16 count` + count × UTF-16LE units (reader 0x277180, stack buf 492 bytes → ≤ 245 units). Converted to UTF-8 (≤ 511 bytes: Cyrillic fits, CJK can overflow), shown with `root.set_clan_daily_message` and, if non-empty, as a channel-3 line from `st_clan_daily_message`.

**type 2 — member_act** (0x272818): `u32 clan_id` (0 → stop), `u32 n_members` + n × {`u32 account_id`, `string name (≤63)`, `u8 rank`, `u8 level`, `u8 online`} (always the registered layout), `u16` (+0x1EC99A), `u8 flag` (window state 4 if 1 else 3). Refreshes the member list and permissions. Same caps as type 0.

**type 3 — rank_change** (0x2729F0): `u32 clan_id` (0 → stop), `u8 n_ranks` + n × {`string name (≤31)`, `u8 permissions`, `u8 is_default`}. Refreshes `set_clan_ranks` and permissions.

**type 4 — clan_team_change** (0x272B71): `u32 clan_id` (0 → stop), `u8 n` (+0x1EE5C4, **room for 100**) + n × {`u32 account_id` (+0), `string name` (+8, ≤ 63), `u8 role` (+4), `bool ready` (+5)}, then `u8 in_team`; if non-zero: `bool in_clan_team` (+0x1E56BC), `bool i_am_team_commander` (+0x1E56BD), `bool team_commander_ready` (+0x1E56BE), `u32 clan_team_id` (+0x1EE5C0; a change adds the clan-team chat tab 0x20A1A0 and re-sends 0x4F with channel_id[9]). If in_team == 0: removes the clan-team tab, clears the three flags and the id. Then UI 0x2A1680.

**type 5 — clan_startup_info** (0x272DBD): `u32 clan_id` (0 → stop), `u8 flag` (0 → stop), `string clan_name` → +0x1E5610, `string clan_abbr` → +0x1E5650. **Read directly into the fields with no cut**: name > 63 runs into the abbr, abbr > 15 into the validation buffers. Does not touch the chat tabs or channel ids.

**type 6 — clan_disbanded** (0x272E7F): `u8 flag`; non-zero → system message `st_clan_was_disbanded` (0x220790). Clears nothing: LIKELY the server follows with `9A 00 <u32 0>`.

### 3.3 Clan actions (client → server), id 0x49

0x49 is the account/clan action message (it also replaces old 0x29). Sub-type is the first byte. UI names come from the lobby_menu callback table (names at 0x9C56C8, delegates registered by 0x281787.., name index = delegate slot + 8, checked against the friend and squad callbacks). Guards: "conn" = connection state 0/4 required; "order" = lobby_client+0x1E57A8 == -1 required. PROVEN layouts; meanings LIKELY from the callback names.

| sub | payload after sub | sender | sent from | meaning |
|---|---|---|---|---|
| 00 | `u32 profile_id`, `string name` | 0x26FD00 | `profile_rename` | rename profile (other agent) |
| 01 | `u8 on` | 0x273000 | `autobuy_changed` | autobuy (old 29 03, other agent) |
| 02 | `u32 profile_id` | 0x26FC90 | `profile_changed`, 0x2A45D0 | set active profile (old 29 04, other agent) |
| 03 | `string nickname` | 0x2730E0 | `request_nickname_validation` | nickname check (other agent) |
| 04 | `string nickname`, `u32` | 0x274310 | `change_player_nickname` | (other agent) |
| 05 | `string`, `u8`, `u8` | 0x274470 | `go_to_knowledge_base` handler region 0x28CAB0 | (other agent) |
| 06 | `string clan_name`, `string clan_abbr` | 0x273150 (order) | `reserve_clan_name` | reserve name + abbr |
| 07 | — | 0x2731F0 (order) | `create_clan` | create the reserved clan |
| 08 | `string clan_name` | 0x273240 | `request_clan_name_validation` | validate name (reply 83 49 08) |
| 09 | `string clan_abbr` | 0x2732B0 | `request_clan_abbreviation_validation` | validate abbr (reply 83 49 09) |
| 0A | `u32 inviter_account_id`, `u8 answer` (0 accept, 1 decline) | 0x273320 (conn) | `important_message_read`, type 7 | answer a clan invite |
| 0B | `string target_name` | 0x2733D0 (conn, order) | after `83 49 0F` | clan invite was sent to target |
| 0C | `u32 value` (Flash arg) | 0x273580 (conn, order) | `buy_clan_registration` | pay registration |
| 0D | — | 0x273610 (conn) | `system_message_clicked` ids 215 / 218 (confirm of `cancel_clan_registration`, `disband_clan`, `leave_clan`) | leave / disband / cancel registration (server decides by role) |
| 0E | `u8 what` | 0x273680 (conn) | `clan_members_tab_opened` / `clan_window_opened` (0), `clan_team_window_opened` (4), lobby_menu 0x21D2E0 (4) | request a 0x9A of type `what` |
| 0F | `string target_name` | 0x2734F0 (conn, order) | `invite_player_to_clan` | ask to invite (reply 83 49 0F) |
| 10 | `u8 rank_index`, `u8 value` (0/1), `u8 permission` (**order corrected 2026-10-12**: 0x273710 writes its arguments rank, value, code) | 0x273710 (conn) | `update_invite_permission` 0, `update_kick_permission` 1, `update_change_rank_permission` 2, `update_daily_message_permission` 3, `update_battle_commander_permission` 4 | rank permission |
| 11 | `u32 account_id`, `u8 up` | 0x2737F0 (conn) | `rank_down_member` (up 0); system message 216 confirm (up 1, id from +0x1E56B0) | change member rank |
| 12 | `u32 account_id` | 0x2738A0 (conn) | `rank_up_member` | rank-up request (reply 83 49 12 u8 → non-zero opens the confirm, 0x21FB20) |
| 13 | `u32 account_id` | 0x273930 (conn) | system message 219 = confirm of `kick_member` | kick |
| 14 | `string rank_name` | 0x2739C0 (conn) | `clan_create_rank` (after a local swear filter, `st_swear_clan_string`) | add rank |
| 15 | `u8 rank_index` | 0x273A50 (conn) | `clan_remove_rank` | remove rank |
| 16 | `wstring text` (u16 count + UTF-16LE, censored 0x268B70) | 0x273AE0 (conn) | `update_daily_message` | set daily message |
| 17 | — | 0x273C70 (conn) | `create_clan_team` | |
| 18 | — | 0x273CE0 (conn) | `request_to_join_clan_team` | |
| 19 | `string target_name (≤63)` | 0x273460 (conn, order) | `invite_player_to_clan_team` | ask to invite (reply 83 49 19) |
| 1A | `u32 inviter_account_id`, `u8 answer` | 0x273D50 (conn) | `important_message_read`, type 10 | answer a clan-team invite |
| 1B | — | 0x273E00 (conn, order) | `leave_clan_team` | |
| 1C | — | 0x273E70 (conn, order) | `disband_clan_team` | |
| 1D | `string member_name`, `u8 status` | 0x273EE0 (conn, order) | `change_status_clan_team_member` | |
| 1E | — | 0x273F90 (conn, order) | `oust_commander` | |
| 1F | `string member_name` | 0x274000 (conn, order) | `remove_team_member` | |
| 20 | `u8 ready` | 0x274120 (conn, order) | `83 30` permitted, clan-team soldier: 1 if not yet ready, else 0 | team soldier ready toggle |
| 21 | `u8 ready` | 0x274090 (conn, order) | `83 30` permitted, clan-team commander | team commander ready toggle |
| 22 | `string member_name` | 0x2741B0 (conn, order) | `set_another_clan_team_commander` | |
| 23 | — | 0x274240 (conn, order) | `clan_match_closed` | |
| 24 | `u8` | 0x274510 | `video_tutorial_hide_clicked` | (other agent) |

Every sub 0x00..0x24 has exactly one sender; nothing above 0x24 is sent. `checkbox_tournament_match_clicked` and `start_friend_message` send nothing directly (LIKELY UI only).

### 3.4 operation_permitted 0x83 for op 0x49 — clan parts

Handler 0x275C80 (`u8 sub`, `sub-1` indexes byte table 0x276300 / jump table 0x2762D0). PROVEN; subs without a case (5, 7, 0A, 0C..0E, 10, 11, 13..18, ≥ 1A) read nothing more.

| reply | body after `83 49 <sub>` | client |
|---|---|---|
| 01 | `u8` → +0x1EA974 | autobuy UI (other agent) |
| 02 | `u32 profile_id` → +0x1EA964 | active profile (other agent) |
| 03 | `string nickname`, `bool ok`, if !ok `u8 reason` 0..5 → `st_short_clan_string`, `st_few_alpha_clan_string`, `st_already_exist_clan_string`, `st_space_symbol_clan_string`, `st_wrong_symbol_clan_string`, `st_string_pattern_error` | nickname validation (other agent) |
| 04 | `u32` | nickname change (other agent) |
| 06 | — | +0x1EC99D = 1 (name reserved) |
| 08 | `string clan_name`, `bool ok`, if !ok `u8 reason` 0..4 (same strings, no 5) | applied only if the name equals the one asked (+0x1E5660); `root.handle_clan_name_validation` |
| 09 | `string clan_abbr`, `bool ok`, if !ok `u8 reason` 0..4 | same with +0x1E56A0; `handle_clan_abbreviation_validation` |
| 0B | `string target_name` | system message `st_clan_invite_1 <name> st_clan_invite_2` |
| 0F | `u8 refused` (1 → stop); else `u32 extra`, `string target_name`, `string text` | client sends `4D` long: target, type 7, text, extra; then `49 0B <target>` |
| 12 | `u8 confirm` | non-zero → rank-up confirm dialog (0x21FB20) |
| 19 | `u32 extra`, `string target_name`, `string text` (no refused byte) | client sends `4D` long type 10; system message `st_clan_invite_1 <name> st_clan_team_invite_2` |

Invite flows (LIKELY, from the code paths): clan: `49 0F <name>` → `83 49 0F 00 <u32> <name> <text>` → client `4D` (type 7, extra) → server relays `A4` type 7 (sender_id = inviter) → target `49 0A <inviter_id> <answer>` + `50 <msg_id> <answer>` → server sends `83 49 0B <name>`? (the client sends `49 0B` itself right after the 4D; the reply shows the "invite sent" text). Clan team: `49 19` → `83 49 19 …` → `4D` type 10 → `A4` type 10 → `49 1A`.

---

## 4. 0x9F online stats — was 0x38, layout UNCHANGED, maintenance CHANGED

Reader 0x271860 → `lobby_menu::set_online_stats` 0x225E80 (0.21d `read_online_stats` 0x19E651 / `set_online_stats` 0x1768A2). PROVEN:

| type | field | UI |
|---|---|---|
| u32 | players_online | `root.set_players_online` |
| u32 | games_online | `root.set_games_online` |
| u32 | minutes_to_maintenance | lobby_menu+0x30D8 |

- < 60 → `root.set_warning_text("<st_server_maintenance> :%dm")` (same as 0.21d).
- **== 0 → system message `st_maintenance` (0x21E680); the game no longer quits** (0.21d called `game::exit_and_launch_launcher`).
- ≤ 30 → Play disabled (2.2). Then `update_play_button_state`.

Initial value 0 → Play stays disabled until the first 0x9F. **Send ≥ 60** (31..59 shows the warning). 0x9F is the lobby's one send-once message (transport.md §4.1: no x byte, never resent), so a lost one would leave Play off: srp sends the counts again every 5 s, changed or not (`servers/lobby-server/src/v026g/core.rs`).

---

## 5. Other server pushes

| id | handler | body | effect | status |
|---|---|---|---|---|
| 0x9C | 0x271480 | `u8 players_to_register`, `u8 min_cnt`, `u8 max_cnt`, `u8` → +0x1EE5F0..F3 | byte 0: the players a clan needs to be registered (`set_clan_ready_players(n, total)` 0x2A0880, `set_clan_creation_enabled` 0x2A0F90); bytes 1, 2: clan-team battle size limits used by Play gating (2.2); byte 3 has no reader | PROVEN |
| 0x9E | 0x2713A0 | `bool show` (+0x1EE611), `wstring url` (u16 count + UTF-16LE, stack buf 504 → ≤ 251 units) | UTF-8 (WideCharToMultiByte CP 65001) into +0x1E6144, `root.show_video_tutorial_link(show)` | PROVEN |
| 0xA5 | inline @0x27507E | `string a` (stack buf 64 → ≤ 63), `string b` (LIKELY ≤ 63) | `root.set_admin_text(a, b)` (0x228DC0) | PROVEN |
| 0xA6 | inline @0x2750C0 | `u8` (skipped) | `root.set_admin_text("", "")` | PROVEN |
| 0xA7 | inline @0x274F95 | `u32`, `string`, 2 bytes | read and ignored (only a NUL is written into a stack buffer at the length); **no** on_client_status_received call | PROVEN |
| 0xA8 | inline @0x274FF8 | `u32 seconds`, `u32`, `u32`, `u32`, `u32`, `f32` | `0x28A400(seconds / 3600, …)`: `root.set_premium_discount_store`, `root.show_premium_post_reward` | PROVEN (layout); meanings GUESS | **Corrected 2026-10-12, see premium.md §2: seconds, money, experience, reputation, time left, discount.** |
| 0xA9 | inline @0x274FBA | `f32 discount` | `root.show_premium_discount` (0x28AB20) | PROVEN |
| 0xAA | inline @0x2750EB | — | forced sign-out: disconnects (0x376BB0, 0x26F8C0) and shows login status 8 = **`st_login_menu_status_sign_in_already_online`** (table 0x21F2A4) | PROVEN |

Login status table (0x21EEA0, for reference): 0 error_connection, 1 invalid_user_or_password, 2 sign_in_attempt_interval_violated, 3/4 no text, **5 NULL entry (crash)**, 6 access_level_restriction, 7 user_not_confirmed, 8 sign_in_already_online, 9 invalid_version, 10 EULA window, 11 online queue, 12+ `st_ban_reason_*` (bad_language, insult, faschism, politics, flood, advertising, discuss_admins, reallife_treating, account_trading, incorrect_info, administrator, nickname, cheating, teamkilling).

---

## 6. What changed vs 0.21d, and what srp must change

Paths are relative to the srp copy (srp-026g, same layout as srp-021d).

| message | 0.21d | 0.26g | status | srp change (file: function) |
|---|---|---|---|---|
| chat transport | separate messaging TCP connection, 0xC3 sign-in | on the lobby UDP connection, no sign-in | CHANGED | `servers/lobby-server/src/messaging_server/mod.rs`: `run` — fold the chat room into the lobby connection; route ids 0x4C..0x50 to it |
| text S→C | 0xC9 | 0xA2, + `string abbr` after the name **for sender_type 5 only** | CHANGED | `messaging_server/message.rs`: `ServerMessage::Text` serialize — new id, add abbr (≤ 15) when sender_type == 5 |
| clan system line | — | 0xA2 type 8/9 channel 3, body `k\x0Eargs` | NEW | only if clans are implemented |
| friendship S→C | 0xCB | 0xA3; lists 7 and 6 gain `string abbr`; action 9 not re-queried; **all other actions are search lists** | CHANGED | `message.rs`: `write_accounts`, `ServerMessage::FriendList/FoundPlayers` serialize (add abbr), drop `FriendshipResult` (`u8 result` crashes the client); `mod.rs`: `friendship_reply` — push full `A3 07` / `A3 08` after each change and at sign-in |
| important S→C | 0xCC | 0xA4, + `string abbr` after the name; types 7, 10, 13, 14 new | CHANGED | `message.rs`: `ServerMessage::Important` serialize; `mod.rs`: `Request::message`, `important_message` |
| text C→S | 0xC1 | 0x4C, same layout; channel 9 new | CHANGED (id) | `message.rs`: `parse_frame`; `mod.rs`: `receives` (channel 9, clan 3) |
| important C→S | 0xC2 | 0x4D; long form with trailing u32 for types 7/10 | CHANGED | `message.rs`: `parse_frame` (accept the optional u32) |
| friendship C→S | 0xC4 | 0x4E; actions 2/3/4/5/6 only | CHANGED (id) | `message.rs`: `parse_friendship` |
| subscription | 0xC5, 9 × u32 | 0x4F, 10 × u32 + u8 language | CHANGED | `message.rs`: `ClientMessage::Subscription` (`[u32; 10]` + u8); `mod.rs`: `Member::channel_ids`, `subscribe`, `language_name` (u8: 0 en, 1 ru, 2 uk, 3 de, 4 pl; slot 1 still 1/2) |
| answer | 0xC6 | 0x50, same layout | CHANGED (id) | `message.rs`: `parse_frame` |
| `#mstats` | `21 13 N` | `41 00 <u32 N>` | CHANGED | lobby query handler (other agent's list) |
| squad info | 0x3A | 0xA1, same layout; byte 2 = commander index (now used) | CHANGED (id, gating) | `crates/survarium/src/v021d/lobby.rs`: `squad_status` (id, write commander index); `lobby_server/mod.rs`: `squad_update` |
| squad member profile query | `21 10 pid` | `34 00 <u32 account> <u32 pid>` | CHANGED | `lobby_server/mod.rs` query dispatch (other agent) |
| squad actions | 0x2A 01..04 | 0x4A 01..04, same payloads | CHANGED (id) | `lobby_server/mod.rs`: `squad_answer` |
| Play gating | commander needs ready == n−1; soldier free | commander needs ready ≥ n−1; soldier locked while member[commander_index].ready | CHANGED | `lobby_server/mod.rs`: `play_answer` / `queue_status` — keep the commander's ready bit meaningful |
| online stats | 0x38, 0 quits the game | 0x9F, 0 only shows `st_maintenance` | CHANGED | `lobby.rs`: `online_stats` (id); keep minutes ≥ 60 |
| clan info | — | 0x9A types 0..6 | NEW | optional: send `9A 00 00 00 00 00` (no clan) at sign-in for a clean clan window (LIKELY not required: the client starts with clan_id 0) |
| clan actions | — | 0x49 06..23 | NEW | answer with `84 49 <sub>` (denied) or ignore; see 3.3 |
| clan-team limits | — | 0x9C | NEW | only for clan teams |
| video tutorial | — | 0x9E | NEW | optional |
| admin text | — | 0xA5 / 0xA6 | NEW | optional |
| A7 | — | ignored | NEW | never needed |
| premium store / discount | — | 0xA8 / 0xA9 | NEW | optional (other agent: premium) |
| forced sign-out | — | 0xAA | NEW | send when the same account signs in elsewhere |

---

## 8. Clans: what was traced on 2026-10-12 (corrections and additions)

From the handlers and the lobby SWF's ActionScript (`CreateClanWindow`, `ManageClanWindow`, `TournamentWindow`, `ClanInviteRenderer`, `MatchMakingSettings`). All PROVEN unless marked.

**0x9A type 0 / 2, the fields that were unknown**
- The `u16` (+0x1EC99A) is the **index of the receiving player in the member list**: `set_clan_manage_permissions` 0x2A11A0 takes `members[u16].rank`, that rank's permission byte, and unpacks bit 4 invite, 3 kick, 2 change_rank, 1 daily_message, 0 battle_commander.
- The `u8 flag` after it: registered clan: 1 = window state 4 (`CLAN_MANAGE_STATE`: the player commands the clan; the leave button reads "Disband", the settings tab is on), 0 = state 3. Registration: 1 = state 1 (the founder: invites, pays, registers), 0 = state 2 (an invited player who accepted: pays, leaves).
- Registered tail `u8 level, u32 a, u32 b, u32 c, u32 d` is `set_clan_progress(minimum = b, maximum = c, current = d, players = a, level)` (0x2A1480): `a` is the most members the clan may have ("The list of players n/a possible"), b..d the experience bar.
- Registration: member `status` 0 accepted, 1 rejected, 2 pending (`statusMap`); `paid` + `money` drive `set_clan_registration_paid` / `set_clan_creation_money_collected` (0x2A0EC0); the cost is 0x8D's sixth u32 (`set_clan_creation_cost`, 0x2250C9); the players needed are 0x9C's first byte. The Register button is on iff name reserved && paid && accepted >= needed (0x2A0F90). `reg_state` 0 / 1 / 2 = the name / the abbreviation / both are taken (flags +0x1E56B8 name ok, +0x1E56B9 abbreviation ok, the same two the `83 49 08` / `83 49 09` answers set; the Reserve button is on when both are).
- Default ranks carry the key suffix as their name and `is_default` 1: Flash shows `st_clan_rank_<name>` (commander, assistant, warlord, soldier exist). Rank 0 is the commander's: its checkboxes are never editable. A new rank is offered only under 10 ranks.
- A clan's tag is shown beside the player's name only in window state 3 or 4 (registered).

**0x9A type 4**
- Roles (`TournamentWindow`): 0 commander, 1 lineup, 2 reserve, 3 applications. The window needs the list grouped by role with the commander first (`_hasCommander = list[0].status == 0`; a separator is drawn where the role changes).
- The third bool of the tail (+0x1E56BE) is the receiving player's **own ready state**: on `83 30` both the commander (`49 21`) and a soldier (`49 20`) send the opposite of it (0x276810). A soldier's Play gating reads the commander's `ready` from the member list (0x2742E0).
- Who is offered what: "create a team" / "become commander" needs the battle_commander permission (`PlayerInfo.isTeamCommander`); "set another commander" the clan's commander (window state 4); move to lineup / reserve and remove the team's commander; oust is offered to everyone on the commander's row (the server decides).
- Clan match mode is offered only in clan window state 3 / 4 and outside a squad.

**Play in clan mode** (`play_button_clicked` 0x28C320): state 0 → `30 <kit>` (no pending flag is set: `83 30` or `84 30` is the whole answer); state 1 with a place → `31 <order> 00` as in any queue. So a waiting clan team's players leave through `31`.

**Invites (0xA4 type 7 and 10)**, `important_message_arrived` 0x226F77 / 0x226372: the sender line is `[<abbr>] <text>` (text alone when the abbr is empty) and a clan invite's body `st_add_to_clan_request "<sender_name>"`. So the server puts the **inviter's name in `text`**, the **clan's name in `sender_name`** and the clan's abbreviation in the abbr; `sender_id` is the inviter's (sent back in `49 0A` / `49 1A`). The `u32` of `83 49 0F` / `83 49 19` is only handed back in the long `4D` (0x26B0A0): its meaning is the server's (srp: the clan id / the clan-team id).

**Clan system line 2**: `<arg1> st_clan_member_kicked <arg2>` reads "<kicker> expelled from the clan <kicked>" (the Russian text).

**Names**: `gameplay/clans/clan_name.options` has `clan_name_regex` (Cyrillic, Latin, digits, є і ї ä ö ü ß, `@`, `#`, blank, `-`) and `clan_name_letters_replacement` (look-alike Cyrillic and German letters for Latin ones); the abbreviation is "3 to 6 characters" (`st_clan_abbreviation_info`). The client refuses an empty name and a swear word itself before `49 08`.

**Not in the client** (srp's own, `crates/session/src/v026g/clans.rs`): the price, the numbers of 0x9C, who may act on whom, the clan's experience and levels, where a new rank goes.

**The numbers' defaults (2026-10-12).** Price 50,000 silver (`SRP_CLAN_COST`) and at most 30 members (`SRP_CLAN_MAX_MEMBERS`): the owner's choice; the only source is the Russian wiki of mid 2015 (0.29 to 0.30: 250,000 silver, 15 members), nothing for 0.26. They were 1,000 and 100 before. Players needed to register: the code's default is 1 (so a test or a lone operator can register), the game's own text says 5 (`st_clan_creation_info_description`: "you need to enlist at least 5 players"), and `deploy/env-026g.example` sets `SRP_CLAN_PLAYERS_REQUIRED=5` for the live server.

## 7. Open questions

- 0xA2 abbr for sender_type ≠ 5: the client passes an uninitialised stack buffer to add_message; whether it can print garbage is not traced (GUESS harmless). srp should send players as type 5 with an abbr string (empty is fine).
- 0xA4 type 6 (item repaired) and types 8, 9, 11, 12: no auto-answer and no UI path that answers them; pending records grow. Unknown whether the real server expected `50`.
- ~~0x9A unknown fields~~, ~~0x9C byte 0~~: settled, section 8. 0x9C byte 3 (+0x1EE5F3) has no reader.
- 0xA5 second string buffer size (assumed 64).
- 0xA8 fields 2..6 meanings (premium discount store), see the premium notes.
- `83 49 0F` `u32 extra` copied into the 4D long form: clan id? (GUESS).
- Clan invite completion: what the server sends after `49 0A` accept (LIKELY a fresh `9A 00` for both and a clan system line `0\x0E<name>` on channel 3).
- `83 30` permitted with clan-team flags sends `49 20/21 <ready>` — the order of `83 30` vs these toggles is not traced further (the 0x30 sender belongs to the other agent).
- The friend `status` values in 0.26g (passed raw to Flash; 0.21d guess 0 lobby / 1 in game / 2 offline / 3 away) are not re-checked.

## 9. What the sender is told when the server can not do what it asked (2026-10-12)

Code: `servers/lobby-server/src/v026g/social.rs`; the client's rules as code: `crates/survarium/src/v026g/lobby/client_rules.rs`; tests: `servers/lobby-server/src/v026g/tests/core_tests/feedback_tests.rs`. All PROVEN unless marked.

### 9.1 The ways the client can show a text of the server's

- **`84 <op> 00 <key>`** (`lobby_menu::on_operation_denied_received` 0x21D9D0). The text is localized first (0x1FF0F0), then the handler jumps by `op - 0x2F` (`cmp eax,0x1B; ja 0x21DDED` at 0x21DA0B; byte table 0x21DED8, jump table 0x21DEB8):

  | op | what happens |
  |---|---|
  | `2F`, `30` | message box, and the Play button is reset (`st_not_logged_id` signs out) |
  | `46`, `47` | message box, then the inventory is drawn again (0x229400) |
  | `48`, `49` | **message box and nothing else** (0x21DCF1, 0x21DD6F): `root.show_system_message(0, "debug", text)` → `MessageBox` with the text and OK (`Message.as`), whatever the client was doing |
  | `4A` | returns at once (0x21DEA8) |
  | `31`..`45`, and everything above `4A` (so `4B`..`50`) | only the log line `Unknown (operation denied) type received %d` (0x21DDED) |

  So `84 4C`, `84 4D`, `84 4E` (the chat and friends ops themselves) and `84 4A` (squads) show nothing. `84 49` is the one message that shows a key of the game at any time with no side effect.
- **`A2` chat line.** The body is shown as it is (censored), never localized: a key sent there is printed as a key. Only the clan system lines (§1.2) are built from keys, and only from their four fixed ones.
- **`A4` important message**: fixed texts by type (§1.4); none is an error.
- `83`: no case for `4A`, `4C`..`50` (§2.3).

### 9.2 The texts the game has

Searched: the 0.26g localization (`tools/re-kit/v026g/data/localization/`, all six languages), the exe's strings, the lobby SWF's ActionScript and the Flash key map (`flash_movies_localizations.options`).

- **`st_player_not_found`** ("Player not found." / «Игрок не найден.» / «Гравця не знайдено.»): in every language; **referred to by nothing in the client** (not in the exe as ASCII or UTF-16, not in the ActionScript, not in the Flash key map). So it is a key the server sent, and `84` is the only message that localizes a key of the server's. LIKELY the messaging server's answer for a name it did not know; which op carried it is not in the client [srp: `49`, see 9.1].
- `st_spam_filter_block_notification` + `st_spam_filter_block_notification_fin`: used by the exe only (0x20875F, 0x20877A), in the client's own spam filter (9.3).
- There is **no** key for "the player is offline", "the player ignores you", "already a friend", "the request was sent", nor for any squad refusal (not the commander, squad full, not invited, already in a squad). The squad keys are labels (`st_squad`, `st_leave_squad`, `st_disband_squad`, `st_remove_from_squad`, `st_invite_to_squad`, `st_show_squad_list`, `st_hide_squad_list`, `st_chat_channel_squad`), the two requests (`st_add_to_squad_request`, `st_add_to_friends_request`) and two the client shows by itself (`st_squad_not_ready` in the queue window, 0x225418; `st_squad_list_locked_hint`, Flash).

### 9.3 Chat flood: the notice is the client's own

`0x2083F0(text, channel, &wait_ms)`, called from the chat input's send (0x2086E8) **only for channels 1, 5, 6 and 7** (general, match, team: `cmp eax,1 / 5 / 6 / 7` at 0x2086C6..0x2086D8). State in the chat object: +0x28 muted-until, +0x2C CRC-32 of the last long line, +0x30/+0x34 head/tail of a ring of four times at +0x38. The clock is milliseconds (owner +0x584C).

1. now < muted-until → wait = the rest (0x20840B).
2. A line of more than 20 characters (`cmp eax,0x14`): its CRC-32 (0x1C180); if it equals the last long line's and now < (time of the last line) + 10,000 → wait = the rest (0x208490..0x2084C7). A line of 20 or fewer clears the stored CRC.
3. The time goes into the ring; when the ring holds three and newest − oldest < 2,000 ms → muted for `10 − 5·(channel ≠ 1)` seconds: **10 s on the general channel, 5 s on a match channel**, and this third line is blocked too (0x20850C..0x208540).

When wait ≠ 0 the client prints, on its system channel (2) from the sender `System` (0x9FE380), `"%s %2.1f %s"` (0x9FE374) of `st_spam_filter_block_notification`, wait/1000 and `st_spam_filter_block_notification_fin`, and **does not send the line** (the sender 0x2089D0 is reached only with wait 0, 0x208731).

So: on the general, match and team channels a stock client lets through at most two lines in any two seconds and tells its player itself. srp's own limit (10 lines at once, then one a second; [srp's own choice]) is never reached by a line that filter let through (test `a_stock_clients_own_spam_filter_is_stricter_than_the_servers_limit`, which runs the filter as code against the lobby). The server can not make the client print the notice (nothing but the client's own filter reaches 0x20875F), so a line over srp's limit is dropped with no word: that only happens to a client that is not stock, or on the private, clan, squad and clan-team channels, where the client has no filter and the game no notice.

### 9.4 What srp sends, case by case

| request | case | srp | why |
|---|---|---|---|
| `4C` private | the name is no account, or the player is not in the lobby | `84 49 00 st_player_not_found` | the game's one text for it; it has no "offline" text. Once per name in 10 s [srp's choice: each line would open its own window] |
| `4C` private | the player ignores the sender | nothing | an ignore is not given away; no text |
| `4C` private | the player's own name | nothing | no text |
| `4C` any | over srp's limit | nothing | 9.3 |
| `4D` type 0 (friend request) | the name is no account | `84 49 00 st_player_not_found` | |
| `4D` type 0 | the player is offline | the request waits for its sign-in | as 0.21d |
| `4D` type 0 | already a friend, already asked, or the player ignores the sender | nothing | no text for any of them |
| `4D` type 2 (squad invite) | the name is no account, or the player is not in the lobby | `84 49 00 st_player_not_found` | |
| `4D` type 2 | the player ignores the sender | nothing | |
| `4D` type 2 | squads are off (`SRP_SQUADS=off`) | `84 49 00 <SRP_SQUADS_OFF_TEXT>`, by default the sentence "Squads are switched off on this server." | the game has no text for it (9.2); a text that is no key is shown as it is (`lobby_requests.md` §4.2). The invite's button is the client's and stays. Not yet seen in a real game |
| `4A 02` | squads are off | the squad as it is (`A1`: none), no text | the invite was already refused; this is a client that adds a player all the same |
| `4D` type 2 | the sender's squad has `SRP_SQUAD_SIZE` players | `84 49 00 <SRP_SQUAD_FULL_TEXT>`, by default "Squads are limited to <size> players." | 9.8; the invite is not passed on |
| `50` (answer 0 to a squad invite) or `4D` type 3 after it | the inviter's squad filled since the invite | `84 49 00 <SRP_SQUAD_FULL_TEXT>` to the player who accepted, once; nothing to the inviter | 9.8; the acceptance is not kept nor passed on |
| `4D` type 3 (the client's own "membership accepted") | can not be passed on | nothing | not the player's act |
| `4E 04` (ignore by a typed name) | the name is no account | `A3 08` (the list as it is), then `84 49 00 st_player_not_found` | |
| `4A 02` | the squad is full (two invites accepted before either was added) | `84 49 00 <SRP_SQUAD_FULL_TEXT>` to the inviter and to the player who was not added; the inviter gets the squad as it is (`A1`) | 9.8; the one squad refusal that is told, with `84 49` (`84 4A` would be dropped) |
| `4A 01..04` | refused (not the commander, not invited, in no squad) | the squad as it is (`A1`), no text | `84 4A` is dropped by the client and the game has no text (9.2) |
| `3D` | the match's report is gone | nothing; but see 9.5 | 0x93 is always a player's kits (0x271A30): there is no "nothing to show" answer |

0.21d (`servers/lobby-server/src/messaging_server/`) says nothing in any of these cases; 0.26g's srp now says what the game has a text for. The bytes: `84 49 00 13 "st_player_not_found"`.

### 9.5 The rows of an open results screen (`3D`)

`VictoryScreen.reguestPlayerInfo` asks once per row (`victory_screen_request_player_data(id, match_id)`, then it keeps the answer in `playersCache`); with no answer the row's panel simply does not open, and nothing waits. srp chose to **keep the report while a screen of it may be open** rather than answer with an empty body: the last 64 reports are kept as before, and past them any report whose screen an account was sent (after the match, at sign-in, or by `41`) in the last two hours [srp's choice] (`session::v026g::rewards::Results`, `SHOWN_FOR_SECONDS`). A restart of srp is no gap: a lobby link is made only by a sign-in (`core.rs` `first_datagram`), so no client's results screen outlives srp. `41` for a match not kept stays unanswered: the client sends it only after a `#mstats [N]` line (0x225810), which srp never sends.

### 9.6 One general chat room for every language: the owner's rule

**The owner wants one general room for all locales (English, Ukrainian, Russian, any other), as on 0.21d. Do not split it.**

- The client's side: `4F` slot 1 is 1 for a `russian` or `ukrainian` client and 2 for any other, and the last byte is the language (§1.6); a general line goes out with that slot's id as its `channel_id` (§1.1). `A2` carries **no** room id: the client shows a line by its channel byte alone (§1.2). So the rooms exist only if the server splits by them.
- srp's side: the ten ids are stored for each client (`dispatch.rs`, `Subscribe`); a general line goes to every client whose slot 1 is set, whatever its value; the `channel_id` of `4C` and the language byte are not used (`social.rs` `chat`).
- Test: `the_general_channel_is_one_room_for_every_language` (an English, a Ukrainian, a Russian and a German client, each with its own room id and language byte, each hears the three others, byte for byte).

### 9.7 Lines about the search (since 2026-10-13, `queue_chat.rs`)

The game has no text for "somebody joined the search", so these lines are the
server's own (the owner's rule was game-native text only; this is a
deliberate exception for these lines, at the owner's request).

- An `A2` line of sender type 5 (an account), id 0, an empty name (default;
  `SRP_QUEUE_CHAT_NAME` gives one), no clan tag, channel 1 (general); the body
  is shown as it is (§1.2). Id 0 is in no ignore list.
- **A join or a leave** goes to every client that listens to the general
  channel (`4F` slot 1 set): `Alice joined the search (3 waiting)`,
  `Alice left the search (2 waiting)`; a squad is one line, naming it:
  `Alice, Bob joined the search as a squad (2 waiting)`, `Bob left the search
  with their squad (0 waiting)`. A leave is said for every way out of the
  queue (Leave, a closed link, the squad changed) and only for a join that was
  said. A join is said at most once in 30 s for a player, so a Play/Leave loop
  is not repeated.
- **A match** (`Match found: Alice, Bob vs Carol`, team 1 against team 2) goes
  to everybody who listens to the general channel, like a join; over 255 bytes
  it is two lines (one for each team).
- `SRP_QUEUE_CHAT=off` turns all of it off.
- **Not verified in a live client:** what the client draws for an empty sender
  name (`add_message` 0x208AD0 puts it in the line's `name` field; a name equal
  to `st_clan_system_message_sender` or `st_clan_daily_message` skips it, but
  those are the clan's lines) and whether `add_to_recent_list("")` (0x209330)
  is harmless. If it shows a stray colon or a blank recent contact, set
  `SRP_QUEUE_CHAT_NAME` to a short name.

### 9.8 The size of a squad (`SRP_SQUAD_SIZE`, since 2026-10-17)

The owner's rule (2026-10-17): squads of two, not of three. Squads were
switched off because squads of three put strong players on one side; two
are fine, with no change to how the sides are made (`matchmaking.md` §8.5).

- **The setting.** `SRP_SQUAD_SIZE` is 2 or 3, default 3 (the client keeps
  three members, `MAX_SQUAD_MEMBERS`; it is today's behaviour, and the live
  server sets 2 with `SRP_SQUADS=on`). Anything else is logged and 3 is
  used. It is given to the store's squads when the lobby starts
  (`session::Squads::set_size`, `Core::new`). It counts only with squads
  on: with `SRP_SQUADS=off` no invite goes out at all (9.4).
- **When an invite is refused.** A `4D` type 2 from a player whose squad
  already has `SRP_SQUAD_SIZE` members: the commander or any member. A
  player in no squad always has room (an invite starts a squad of two).
  The invite is not passed on: the invited player sees nothing.
- **What the player sees.** The message box of `84 49 00 <text>` (9.1),
  as for squads off: `SRP_SQUAD_FULL_TEXT`, a key of the game or a
  sentence of up to 255 bytes (a longer or empty one is ignored), by
  default "Squads are limited to 2 players." (the number is the size).
  The game has no text of its own for it (9.2). The client's invite
  button stays.
- **Races.** Invites go out while the squad has room, so two can be out at
  once (Alice invites Bob and Carol before either is in):
  - The second acceptance (`50` answer 0, or the `4D` type 3 that follows
    it) comes after the first was added: it is not kept nor passed on to
    the inviter, and the player who accepted is told the same text, once.
  - Both acceptances came before either was added: the inviter's client
    sends two `4A 02`. `Squads::add` checks the size under its lock, so
    only the first is added; the second is refused (`SquadError::Full`)
    and its acceptance is used up (a place freed later does not take that
    player on it). The inviter gets its squad as it is (`A1`) and the
    same message box, and so does the player who was not added.
- **Leaving frees the place.** A member leaving (`4A 04`), dropped (`4A
  03`), or the squad disbanded (`4A 01`): the next invite goes out. A
  squad of one is none (`squads.rs`), so its last player is
  in no squad and starts a new one.
- **A smaller size while squads are bigger** (only across a restart, which
  ends every squad anyway): a squad keeps its members and takes nobody new
  until it is under the size.

Tests: `session::squads` (`a_squad_of_two_takes_nobody_more`,
`two_invites_added_from_two_threads_make_one_squad_of_two`,
`a_member_leaving_frees_its_place`, `the_size_is_two_or_three`) and the
lobby's `core_tests::squad_size_tests`.

## Correction (2026-10-12): the order of an accepted squad invite

`important_message_read` 0x28C5E0 sends the two messages of an accept in this order: `4D` type 3 "membership accepted" (the sender 0x26AFB0, called at 0x28C698), then `50 <id> 00` (0x26B1B0, called at 0x28C7E1). PROVEN. Where this note says the acceptance is passed on "only for an invite that was accepted", read: it is passed on once both have come, whichever came first (`social.rs`: `Request::accepted_text`). Until then srp dropped the first and every first accept was lost (12 of 12 live). A squad invite that waits already is sent again under its id when the inviter invites again (the client skips an id it holds, 0x26BCDA).
