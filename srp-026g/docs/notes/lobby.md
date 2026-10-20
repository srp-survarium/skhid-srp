# 0.26g lobby protocol vs 0.21d

Version scope: client 0.26g (build string "0.26g0", build 2777, compiled Jan 14 2015, NO PDB) vs client 0.21d (build 2010, April 2014, PDB). Everything below is 0.26g compared with 0.21d.

Tags: **PROVEN** = read in the 0.26g disassembly at the VA given (and matched to the 0.21d PDB symbol/VA given). **LIKELY** = strong inference (UI strings, data flow). **GUESS** = weak inference. Unknowns are marked "unknown".

Baseline: `srp-021d/docs/notes/lobby_spec.md` (abbrev. "spec021d §x").

Work files (scratch, not deliverables): a full `dumpbin /disasm` of the 0.26g .text with every string immediate annotated was made in the session scratchpad (`d026a.txt`, 3 M lines). Re-make it with `dumpbin /disasm:nobytes /section:.text <exe>` and annotate pointers into .rdata.

Sibling notes written for this comparison (same folder): `lobby_requests.md` (every client→server lobby request, 0x83/0x84 bodies), `social.md` (chat 0xA2..0xA4 and 0x4C..0x50, squad 0xA1/0x4A, clan 0x9A/0x49, 0x9F, 0xA5..0xAA), `results.md` (0x93, 0x97, 0x98, 0x99, 0x9B), `transport.md` (the UDP transport the lobby now uses), `login_chat.md` (login reply / browser). This file is the overview, the id map and the core economy/profile layouts; it summarises the others in §4–§5.

---

## 0. The big picture (read first)

| # | Finding | Tag | Evidence |
|---|---|---|---|
| B1 | **The lobby no longer runs over TCP.** `tcp_packet_client.cpp` (0.21d 0x829D30) is gone. `lobby_client::connect` 0x26F650 builds the sign-in message and passes it to 0x379090 = `vostok::network::match_client::connect` (string "vostok::network::match_client::connect", `.\match_client.cpp`). The lobby connection is the **UDP match transport** (`udp_match_client` / `udp_match_connection`, now with `sequence_number<unsigned char>` and `math::u128` acks: strings at 0xA2D358 etc.). The sign-in message is the connect payload. | PROVEN | 0x26F650 → 0x379090 (call at 0x26F886) |
| B2 | **Messaging (chat/friends) is merged into the lobby connection.** No separate messaging socket and no 0xC3/0xCA messaging sign-in. Messaging handler `messaging_client::process_incomming_packet` 0x26ABE0 is called from the lobby handler for ids 0xA2..0xA4. Messaging client sends ids 0x4C..0x4F through the same connection object (`[messaging+4]+8`). | PROVEN | 0x274A14..0x274A2B, 0x26AEB5..0x26AEBD |
| B3 | **All message ids are renumbered.** Client→server lobby ids are 0x2E..0x4B (+ messaging 0x4C..0x4F); server→client ids are 0x80..0xAA. The old two-level `0x36 <type>` client_status is **flattened**: every old 0x36 sub-type is now its own top-level id. | PROVEN | dispatcher 0x274A00, jump table 0x275138 |
| B4 | **The server never sees `0x36`, `0x21 <type>` etc. any more**: an 0.21d server is not wire compatible at any message. | PROVEN | — |
| B5 | Browser HTTP step (`%s&type=%d&local_ip=%s&login_ip=%s&ver=%s`, 0.21d 0x80D320) is gone from the exe; the lobby host/port comes from the login reply (+0x1E573C u16 port, +0x1E573E host, copied in 0x26F650 from the 0x50-byte login result struct). | LIKELY (string absent; connect args traced) | 0x26F66C..0x26F670, 0x26F872..0x26F886 |
| B6 | Outgoing message API: `msg = 0x37A0E0(id)` (eax = client+8), payload bytes appended to the vector at `msg+0x2BC` (data +4, size +0xC, resize 0x7AB30), `0x379640(client+8, msg)` sends. Booleans are written `00/FF` as in 0.21d. | PROVEN | e.g. 0x26F69B..0x26F6F1 |
| B7 | Reader: same raw `buffer_reader` (start +0, cursor +4, size +8), **no bounds checks** (as 0.21d R2). Strings are still `u8 len + bytes` copied with memcpy 0x424580 + NUL, no capacity check. | PROVEN | 0x1F10F0 (r_string) |
| B8 | **New: some payloads are LZMA2-compressed** (0x8A prices, 0x95 shop items): `u32 packed_size` + raw LZMA2 stream (7-zip `Lzma2Decode`, 0x3EE810, wrapper 0x357EB0; `compressor_lzma2.cpp`). Decoder dictionary prop byte is chosen by the client as the smallest `p` (0..40) with `(2|(p&1)) << (p/2+11) >= unpacked_size` (0x357DD0). | PROVEN | 0x271540, 0x271650 |

The transport layer itself (UDP header, acks, ordering) is not covered here; see the transport notes. For the server: the lobby port is a UDP match-style endpoint whose first (connect) packet carries message 0x2E.

---

## 1. Dispatchers and id tables

### 1.1 Sign-in phase: `lobby_client::sign_in_on_message_received(u8 id, reader&)` 0x26FAB0 (PROVEN)
0.21d equivalent: `lobby_client::sign_in_on_packet_received` 0x19D2F2.
- id **0x80** (= old 0x30 connection_successful): `u8 client_state` → lobby_client+0x1E5790, `u8 flag` (≠0 → bool; passed on; meaning unknown). Then 0x26FA00 swaps in the main handler 0x274A00, calls `root.close_sync`, `show_match_making(false)`, `allow_match_making_settings(true)`; 0x26C320 subscribes the chat channels (sends 0x4F); and the client immediately sends **0x32** (see §4). Reconnect counter +0x1E5780 reset.
- any other id: logged "received unknown SignIn message:%d" (0xA07658) and ignored.

### 1.2 Main handler `lobby_client::on_message_received(u8 id, reader&)` 0x274A00 (PROVEN)
0.21d: `network_client::on_lobby_packet_received` 0x1A1930.
- ids 0xA2..0xA4 → messaging 0x26ABE0.
- else `jmp [(id-0x80)*4 + 0x275138]` **with no range check**: an id < 0x80 or > 0xAA jumps through garbage → crash. Table entries for **0x81, 0x86, 0xA0 are 0 → jump to NULL → crash**. (0.21d sent unknown ids to `read_squad_info`; 0.26g crashes.)
- After most readers it calls `lobby_menu::on_client_status_received(u8 id)` 0x21D720 (0.21d 0x16F090), which dispatches on `id-0x85` (table 0x21D940/0x21D968) and logs "Unknown Client status received. type = %d" for ids without UI work. The byte table 0x21D968 (read 2026-10-12), id → case: 0x85 → 0 (the queue state), 0x87 → 1, 0x89 → 2, 0x8B → 4, 0x8C → 5, 0x8D → 6, 0x8E and 0x96 → 7, 0x95 → 8; 0x8A, 0x8F..0x94, 0x97, 0x99, 0x9A, 0x9C..0x9E → 3 (return, nothing); **0x86, 0x88, 0x98 and 0x9B → 9**, the default at 0x21D87B, which only writes that line and returns (0x21D90B..0x21D93A). So **every `9B` (craft tables) and every `98` (drop) writes "Unknown Client status received. type = 155" / "152" in the client's log: the client's own flaw, harmless, no server fault.** It is in the log of every sign-in (the first live one too, 09:07:21.798) and stood 5 ms before the crash of 2026-10-12 only because `9B` is sent four messages before `99` (results.md §4.1).

### 1.3 Server → client id table (0.26g → 0.21d)

| 0.26g id | 0.26g reader | meaning | 0.21d equivalent | status |
|---|---|---|---|---|
| 0x80 | 0x26FAB0 | signed in | 0x30 | changed (+u8) |
| 0x81 | — (NULL entry) | unknown; never send | — | crash if sent after sign-in |
| 0x82 | inline 0x274A46 | connect to match server | 0x33 | changed (+u32) |
| 0x83 | inline 0x274E92 | operation permitted | 0x34 | changed (new ops) |
| 0x84 | inline 0x274F14 → 0x21D9D0 | operation denied | 0x35 | same layout |
| 0x85 | 0x270100 | client state | 0x36 00 | changed |
| 0x86 | — (NULL entry) | never send | — | crash |
| 0x87 | 0x270260 (+0x1F1070) | account + all profiles + equipped items | 0x36 01 + 0x36 02 (merged) | **rewritten** |
| 0x88 | inline 0x274BCE | squad member account/profile | 0x36 10 | rewritten (same body as 0x87 core) |
| 0x89 | 0x270400 | inventory | 0x36 03 | changed (item descr) |
| 0x8A | 0x271540 | prices | 0x36 04 | changed (LZMA2) |
| 0x8B | inline 0x274C89 | money | 0x36 05 | same |
| 0x8C | 0x270570 | level/exp + skills | 0x36 06 | changed |
| 0x8D | 0x26E450 | service prices | 0x36 08 | changed (6 × u32) |
| 0x8E | 0x271740 | reputations | 0x36 09 | changed (+u8) |
| 0x8F | 0x271300 | ranked total + nickname | 0x36 0A | same |
| 0x90 | 0x271D90 | player Elo/ranking | 0x36 0B | same |
| 0x91 | 0x271EB0 | ranking list | 0x36 0C | same |
| 0x92 | 0x271DF0 | quests | 0x36 0D | same layout |
| 0x93 | 0x271A30 | victory-screen player: a full result (discarded) + an account body (§2.5) showing his equipment | 0x36 0E | rewritten (results.md) |
| 0x94 | 0x2714D0 | account services (premium) | 0x36 0F | changed (loyalty gone) |
| 0x95 | 0x271650 | premium shop / bundles | 0x36 11 | changed (LZMA2, 0xD0 records) |
| 0x96 | inline 0x274E21 | unlocked factions mask | 0x36 12 | same |
| 0x97 | 0x2718C0 | match statistics | 0x36 13 | rewritten (results.md) |
| 0x98 | 0x271AE0 | drop (`read_drop`) | — | NEW |
| 0x99 | 0x2769B0 | daily bonus | — | NEW |
| 0x9A | 0x271FD0 | clan info (`read_clan_info`) | — | NEW |
| 0x9B | 0x2769F0 | craft chances / prices | — | NEW |
| 0x9C | 0x271480 | clan-team battle size limits (4 × u8) | — | NEW |
| 0x9D | 0x271340 | tutorial state / awards | — | NEW |
| 0x9E | 0x2713A0 | video tutorial link | — | NEW |
| 0x9F | 0x271860 | online stats | 0x38 | same layout; 0 minutes no longer quits the game (social.md) |
| 0xA0 | — (NULL entry) | never send | (0x39) | crash |
| 0xA1 | 0x274720 | squad info | 0x3A | same layout; 2nd byte = commander index (social.md) |
| 0xA2 | 0x26B6F0 | chat text | 0xC9 (messaging) | changed: + clan tag (social.md) |
| 0xA3 | 0x26ABE0 inline | friendship status | 0xCB (messaging) | changed: + clan tag; result replies gone (social.md) |
| 0xA4 | 0x26BBF0 | important message | 0xCC (messaging) | changed: + clan tag, new types (social.md) |
| 0xA5 | inline 0x27507E | admin text (2 strings) | — | NEW |
| 0xA6 | inline 0x2750C0 | admin text clear | — | NEW |
| 0xA7 | inline 0x274F95 | `u32, str, u16`, read and ignored | — | NEW (no effect) |
| 0xA8 | inline 0x274FF8 → 0x28A400 | premium discount store / post reward | — | NEW |
| 0xA9 | inline 0x274FBA → 0x28AB20 | premium discount | — | NEW |
| 0xAA | inline 0x2750EB | forced sign-out to login, shows `st_login_menu_status_sign_in_already_online` | — | NEW |

GONE from 0.21d: 0x36 07 (had no handler anyway), 0x37 ping answer (unless 0xA7), messaging 0xCA sign-in reply, the 0x36 01 / 0x36 02 split.

---

## 2. Server → client layouts (core: account, profiles, inventory, economy)

Offsets "lc+X" are lobby_client offsets in 0.26g. All integers little-endian. Type `str` = `u8 len` + bytes, no NUL.

### 2.1 0x80 signed in (old 0x30) — PROVEN 0x26FBAB
`u8 client_state` (→ lc+0x1E5790; same enum 0 surf / 1 matchmaking / 2 in match / 3 unknown), `u8 flag` (≠0 = true). The flag is only **echoed back**: right after 0x80 the client sends `32 00 <flag 00/FF>` (0x26FBEA..0x26FC47). Meaning unknown. 0x26FA00 then swaps handlers and calls `show_match_making(false)` / `allow_match_making_settings(true)` unconditionally. Send `80 00 00`.
After it the client sends 0x32 and 0x4F (see §4).

### 2.2 0x82 connect to match server (old 0x33) — PROVEN 0x274A46 (0.21d 0x1A1DEC)
| type | field | dest |
|---|---|---|
| str | host (stack buffer 64 → **≤ 63**) | stack |
| u16 | port | |
| u32 | match_id | lc+0x1E57A4 |
| u8 | team | lc+0x1E57AC |
| u32 | **NEW**: **match session**: what the client sends the match server as `session` in 0xD8 and 0xD9. PROVEN: stored at 0x274A83, reloaded at 0x274AB5 as the 4th argument of `connect_to_match`, passed at 0x21C090..0x21C0AD to `match_client::connect` 0x27A1B0, written into 0xD8 at 0x27A203; 0xD9 reads the same field (network_client+0x1EE7EC, 0x27B864). The field has no other writer (constructor: -1). The login session is used only for the lobby sign-in `2E`. Seen live 2026-10-12: with 0 here a real client sent 0xD8 with session 0. (Earlier misread as a game time: the "game time is %d" of the log line is a local clock value, 0x21BEE4..0x21BF16.) | lc+0x1EE60C |

Sets client_state = 2, then `network_client::connect_to_match(host, port, match_id, match_session)` 0x21BEB0, then chat channel resubscribe (0x26C2E0 → 0x4F). The handler has no condition (0x274A46..0x274AF8).

**`connect_to_match` does nothing when `match_id` equals the `Match id` of the last static info the client took** (PROVEN: `mov eax,[this+0x1F1C48]` the match client, `mov ecx,[eax+0x577BC]` = static info +0x577B0 (the static info sits at match client +0xC, constructors 0x2791F0 / 0x27A070), `cmp ecx,[ebp+0x10]`, `jz` to the function's end, 0x21BEBC..0x21BED5). No log line, no 0xD8; the client is already in state 2 and shows "matchmaking is over, waiting for the server" for good. That field is written by the static info reader (0x1F0E7A) and reset to −1 only by the constructor (0x3183D2), which runs once, when the game starts. Otherwise the function logs `[R] connect_to_game_server: %s: %d game time is %d`, sets the match client's delegate, zeroes the match message state (+0x1F1C64) and calls `match_client::connect` 0x27A1B0, which makes a new UDP connection object each time (0x379090): nothing of an earlier connection is in the way. Seen live 2026-10-12: a player whose match connection broke pressed Play, was sent `82` for the match it had been in (same id as its static info) and never contacted the match server until its game was started again. So the static info must carry another id than the lobby's (static_info.md; `registry::static_id`). A match id of −1 would also be dropped by a client that has played no match yet (srp's ids never have the top bit).

**Requests the client does not send while lc+0x1E57A4 (the match id) is not −1** (PROVEN, each sender starts with `cmp [lc+0x1E57A4],-1; jnz` to its end): `46 00` 0x2706F0, `47 ..` 0x270BE0 / 0x270D00 / 0x271040 / 0x272F70, `48 ..` 0x271100 / 0x271270. The id is set by `82` and by `85` status 2, and set to −1 only by `85` status 0 or 1 (§2.4): a client back from a match buys, moves and repairs nothing until one of those comes.

### 2.3 0x83 operation permitted / 0x84 operation denied — PROVEN
- 0x83: `u8 op` then an op body. Handled ops: 0x30 → 0x276810, 0x46 → 0x275520 (`process_succeeded_inventory_action`), 0x47 → 0x276360, 0x49 → 0x275C80 (sub-type byte 1..0x19, tables 0x2762D0/0x276300), 0x4B → the client re-queries 0x3C and 0x3E. Every other op: nothing read. (0.21d: ops 0x24/0x29/0x2B, 0x1A1D7C.) Bodies: §4.
- 0x84: `u8 op`, `u8 skipped`, `str text` → `lobby_menu::on_operation_denied_received` 0x21D9D0 (0.21d 0x16F29F). Same layout as 0.21d 0x35. Per-op behaviour: §4.

### 2.4 0x85 client state (old 0x36 00) — PROVEN 0x270100 (0.21d `read_status_info` 0x19D7DD)
| type | field | dest | when |
|---|---|---|---|
| u8 | status | lc+0x1E5790 | always |
| u32 | order_id | lc+0x1E57A8 | status 1 or 2 |
| u32 | place (0 = match found) | lc+0x1E579C | status 1 |
| u32 | queue_total | lc+0x1E57A0 | status 1 |
| u32 | current_wait_s | lc+0x1E5794 | status 1 |
| u32 | average_wait_s | lc+0x1E5798 | status 1 |
| u16 | **NEW** search step (seconds): UI shows the range `max(0, min − w) .. max + w`, `w = current_wait / this` (`div` at 0x22565E), `min`/`max` being the client's own level range (`matchmaking.md` §1, §2). **Must be ≠ 0**: integer divide-by-zero crash (path 0x21D7C0 → 0x2252D0 → 0x22565E, reached for status 1 when place ≠ 0 and the squad is ready) | lc+0x1E57B0 | status 1 |
| 20 × u16 | **NEW** matchmaking "search properties": entry i shown as `{level: i, value}` via `root.set_mm_search_properties` (0x225643) | lc+0x1E57B2 (40 bytes, memcpy) | status 1 |
| u32 | match_id | lc+0x1E57A4 | status 2 |
| u8 | team | lc+0x1E57AC | status 2 |
| str | status text, **only if bytes remain**. Buffer lc+0x1E60C4, 128 bytes (**≤ 127**), copied by 0x26E1D0 with no capacity check (overflow runs into lc+0x1E6144 and then the account struct lc+0x1E6344) | | optional |

The status-1 field order equals 0.21d plus the two new trailing fields. Status 0 sets order/match = −1, team = 3 and zeroes the four counters; status 1 sets match −1, team 3; status 2 zeroes the counters. UI 0x21D750: 0 → (if not in a match) `game::switch_to_lobby`, `show_match_making(false)`; 1 → `show_match_making(true)` + `set_match_making_status` 0x2252D0 (`st_mm_match_found` when place == 0, `st_squad_not_ready`, `set_mm_current_time`, `set_mm_search_properties`, `set_mm_search_range`); `queue_total` and `average_wait` are no longer shown (0.21d `set_mm_place`/`set_mm_average_time` not called). Play button: `update_play_button_state` 0x21FD40 (0.21d `update_play_button_lock` 0x1769A7).

### 2.5 The account body, `deserialize` 0x1F1070 (used by 0x87 and 0x88) — PROVEN
This replaces 0.21d's per-profile `player_profile::deserialize_static` 0x1F5C52 and the 0x36 01 profile list. One `survarium::player_profile` struct (strings "survarium::player_profile::serialize_dynamic", `.\player_profile.cpp`; lc+0x1E6344, ≈ 0x4640 bytes, ctors 0x2F73F0/0x2F7C50: 3 sub-profiles of 0x704, 0x45 = 69 items of 0xB4) holds **up to 3 profiles and a shared item table**:

| type | field | dest (struct-relative) | note |
|---|---|---|---|
| u8 | team (zero-extended to u32; ctor default 3 = `game_team_id` undefined, 0x2F7CAF) | +0x45EC | LIKELY team (the 0.21d profile's team byte moved here); compared between players (0x1F2B25, 0x234964) |
| str | name / nickname (**≤ 63**) | +0x08 | 64-byte buffer |
| str | second name (**≤ 14**: buffer +0x48..+0x56; +0x57 is the profile count and is overwritten right after) | +0x48 | GUESS clan tag |
| u32 | unknown | +0x45F4 | read by 0x284E7C, 0x2F4E2A, 0x332BAA. GUESS account/player id |
| u8 | P = profile count (**≤ 3**; profile i lives at +0x58 + i·0x704, a 4th overwrites the item table; no check) | +0x57 | |
| u8 | M = item count (**≤ 69**: table +0x1568..+0x45EC, 0xB4 each; no check) | +0x1564 | |
| P × profile | below | +0x58 + i·0x704 | |
| M × item | `inventory_item_descr`, mode 0 (§2.6) | +0x1568 + j·0xB4 | |

Profile record (Q = +0x58 + i·0x704):

| type | field | dest | note |
|---|---|---|---|
| u8 | state (ctor default 2, 0x2F7BF3) | Q+0x700 | 2 → passed to Flash as 2; anything else → the UI computes 0 (complete) or 1 (incomplete + `hint_text`) from the slots (0x222010, 0x2F7DA0). **2 = a premium account's profile (PROVEN in the UI):** `ProfileSelector.setupProfiles` (`lobby_menu.swf`) shows a state-2 tab highlighted, with the tip `st_premium_profile_hint` = "Available with premium account". Which profiles the original server marked is not known (LIKELY the third); srp marks none: every account uses all three kits, premium or not (the owner's choice, 2026-10-17; `Account::kit_usable`) |
| u32 | profile_id | Q+0x6FC | matched against the active id (§2.7) |
| str | profile name (**≤ 63**); empty → localized default name per index (table 0xA7482C) | Q+0x00 | |
| u32 | slot mask, bits 0..22 | | |
| per set bit k ascending: `u16 item_index`, `u16 slot_amount` | | Q+0x40 + 4k | Clear bit → 0xFFFF (empty). **item_index is an index into this account's M-item table** (0x2225B6: `table + index·0xB4`), not an item id; an index ≥ M reads stale memory (no check). slot_amount is shown as condition_or_stack for stack items (0x2226DA) |
| u128 | modifier mask (16 bytes) | | was u32 with 20 bits |
| per set bit b (0..67) ascending: f32 | | added to Q+0x9C + 4b | 68 modifiers (`player_params_modifiers_enum` grew from 20 to 68; names not recovered) |

Gone from the 0.21d profile body: team, the is_local bool, the new-player-protection counter, the per-slot `inventory_item_descr` with `MODE[k]` (0.21d table 0x877168). Items are now listed once in the account item table.

### 2.6 `inventory_item_descr` — PROVEN 0x1F1350 / 0x1F13D0 (0.21d `inventory_item_descr::deserialize` 0x1F5B53)
0xB4 bytes in memory (was 0x3C).

| type | field | dest | condition |
|---|---|---|---|
| u32 | item_id (instance) | +0x00 | always (**order swapped vs 0.21d**: id first, then dict) |
| u16 | dict_id | +0x04 | always. **0 → nothing else is read** (6-byte empty entry) |
| u32 | condition_or_stack | +0x08 | dict_id ≠ 0. Stack items: a count. Other items: condition; UI shows `round(v·0.0001)` (0x287FE7), so 1 000 000 = 100 |
| u32 | repair_time | +0x0C | only non-stack items **and** mode 0 |
| u32 | reroll_time | +0x10 | only non-stack items (any mode) |
| u8 | n1 | | `buffer_vector<inventory_item_descr::modification>`: overflow = fatal "buffer overflow" assert (0x1F1534). inline capacity **8** (storage +0x20..+0x60, item ctor 0x2F7C10) — PROVEN |
| n1 × {`u32 modification_id`, `bool state`} | | +0x14.. (8 B each) | **state = 0: a modification; state ≠ 0: a defect** (items.md §2). The id is looked up in the table of its kind (modifications dict+0x140, 0x229F0D; defects dict+0x148, 0x229AA0; 0x2C each) and **the row is read even when it was not found → crash** (0x229AC2), for both states (corrected 2026-10-12: "state 0: no lookup" was wrong). rarity_level = min(3, base_rarity_level + count(state == 0)) |
| u8 | n2 | | `buffer_vector<survarium::upgrade>`, inline capacity **3** (storage +0x6C..+0xB4, 0x2F7C10); overflow = fatal assert (0x1F1655) — PROVEN |
| n2 × {`u32 upgrade_id`, `u32`, `u64`, `u8 param`} | | +0x60.. (0x18 B each) | NEW. Flash `upgrades: [{id, param}]` (0x288218, 0x2882DB). `upgrade_id` = the cosmetic's item id, the u32 = its dict id, the u64 = the decal editor's value, `param` = the slot (0, 1 decals, 3 camouflage): items.md §2 (PROVEN 0x275671) |

`is_stack` comes from the dictionary (`dictionary_item+0x22A`, as in 0.21d). **The dictionary lookup changed:** `items_dictionary::item_by_id` 0x1F0090 is now a binary search that logs "There is no dictionary item with id[%u]" and returns NULL. The reader treats NULL as is_stack = 1, but most UI paths dereference the result without a check (e.g. 0x2226C9, 0x287E54, 0x2F7E1D), so **an unknown dict_id still crashes**, only later. Every lobby reader found uses mode 0.

### 2.7 0x87 own account (old 0x36 01 + 0x36 02) — PROVEN 0x270260
| type | field | dest |
|---|---|---|
| … | account body §2.5 | lc+0x1E6344 |
| P × u32 | per-profile revision (LIKELY; 0.21d P+0x9DC) | lc+0x1EA940 + 4i |
| u32 | active profile_id | lc+0x1EA964 |
| u8 | autobuy flag (now per account) → `root.set_autobuy` 0x227760 | lc+0x1EA974 | **Corrected 2026-10-12, see premium.md §3: a number, bit 0 autobuy, bit 1 auto-repair.** |
| u8 | level (account level = skill points unlocked) | lc+0x1EA975 |
| u32 | exp_total | lc+0x1EA968 |
| u32 | exp_level_start | lc+0x1EA970 |
| u32 | exp_next_level | lc+0x1EA96C |

Then 0x270320 re-copies **every account item from the inventory vector by item_id** (std::find 0x277AA0). A miss yields a NULL source that 0x270390 dereferences → **crash. Every item in the 0x87 item table must already be in the inventory (0x89): send 0x89 before 0x87.** (PROVEN 0x270340..0x2703F1.) Then the active profile index (lc+0x1EA976) becomes the profile whose id equals the active id (0x274B4E). UI: `setup_player_profiles` 0x221E80 (`name`, `level` = equipment level computed by 0x2F79A0, `state`, `hint_text`) and 0x222330 (`select_player_profile`, `fill_player_profile_items{itemId, slotId, condition_or_stack}`, ammo bags, quick slot size, `update_profile_slots_count`, `set_quest_reward_multiplier`). If lc+0x1EAAA8 (in a squad) is set, the squad UI 0x2277C0 refreshes too.

No direct send was found in 0x222330 (0.21d sent `21 02` per profile and `29 04` for the active one).

### 2.8 0x88 squad member account (old 0x36 10) — PROVEN 0x274BCE
`account body §2.5` (into a temporary lobby_player_profile, ctor 0x2F73F0), then `u32 revision` → `on_squad_member_profile_arrived` 0x222FE0. **There is no leading profile_id any more.**

### 2.9 0x89 inventory (old 0x36 03) — PROVEN 0x270400 (0.21d 0x19DB20)
`u32 n`, n × `inventory_item_descr` mode 0 (§2.6) into the vector lc+0x1EA978 (0xB4 per item, heap). **Then three more lists are read from the same message** (0x270534 → 0x26EF40, the reader in esi, its only caller; **corrected 2026-10-12**: this note said 0x26EF40 "builds" the list. srp's `lobby::inventory` writer and `client_model` stopped after the items until the same day; they now write and read the three lists, `lobby::inventory_with_spent` for real rows): each `u8 count`, count × {`u32 value`, `u16 dict_id`} (8 bytes each in memory; 0x26EF40..0x26F051). They are the money the server spent for the player after a match: autobuy ammo, autobuy consumables, repair, in that order (handed to `fill_victory_screen_spend_money_detailed` 0x229120 → `VictoryScreen.setSpendMoneyDetailed(autobuy_ammo, autobuy_consulables, repair)`, rows `{dict_id, value}`; shown as "Ammunition", "Special equipment", "Repair cost"). Stack buffer_vectors of about 24, 18 and 27 entries (LIKELY, from the frame offsets at 0x2704BD..0x27052D); a larger count is the fatal "buffer overflow" assert (0x277070). **An 0x89 with nothing spent must end `00 00 00`.** Then `setup_inventory_data` 0x288520 (`dictId, faction_id, condition_or_stack, repair_time, reroll_time, is_attached_to, upgrades, modifiers, rarity_level`), which ends by raising the lobby's own UI event `inventory_content_filled` (0x2886F5; no script calls it): the tutorial's weapon step waits for it (premium.md §5.1).

### 2.10 0x8A prices (old 0x36 04) — PROVEN 0x271540 (0.21d 0x19DBDF)
`u16 n` (lc+0x1EA9A0), `u32 packed_size`, then `packed_size` bytes of **raw LZMA2** that unpack to exactly `n × 8` bytes of `price_item`. The record is the same 8 bytes as 0.21d: `u32 cost, u16 dict_id, u8 level, u8 faction_id` (UI 0x223190 reads +4/+6/+7). n = 0 → nothing more is read. The reader does not advance past the packed bytes, so this must be the last field.

### 2.11 0x8B money (old 0x36 05) — PROVEN inline 0x274C89
`u32 generic_money` (lc+0x1EA9A4), `u32 premium_money` (lc+0x1EA9A8) → `root.set_account_money{generic_money, premium_money}` 0x2238D0. The layout is unchanged. **The 0.21d dependency "send 0x0A before 0x05" (nickname) is gone.**

### 2.12 0x8C level and skills (old 0x36 06) — PROVEN 0x270570 (0.21d `read_player_skills` 0x19DC4D)
| type | field | dest |
|---|---|---|
| u8 | level | lc+0x1EA975 |
| u32 | exp_total | lc+0x1EA968 |
| u32 | exp_level_start | lc+0x1EA970 |
| u32 | exp_next_level | lc+0x1EA96C |
| u8 | n | lc+0x1EA98C |
| n × {`u16 skill_id`, `u8 points`} | heap (4-byte stride) | lc+0x1EA988 |

Changes:
- **no profile_id** (skills and level are per account);
- skill id is u16 (was u8);
- **no perks list**;
- the exp order is total/start/next (0.21d 0x36 06 was total/next/start).

UI `fill_char_info` 0x224420: `points_available = level − Σpoints`, `points_unlocked = level`, `experience_current = total − start`, `experience_next_level = max(0, next − start)`. `fill_skills` / `fill_skills_progress` come from the `skills_tree` config; skill modifiers `use_two_double_handed_weapons_modifier` and `disable_second_weapon_modifier` are applied.

### 2.13 0x8D service prices (old 0x36 08) — PROVEN 0x26E450 (0.21d 0x19E304)
6 × u32 (was 3) → lc+0x1EE5D8..+0x1EE5EC, UI 0x224ED0:

| # | use |
|---|---|
| 0, 1 | `root.set_reroll_cost(a0, a1)`: the skills reset in silver and in gold (PROVEN in the UI: `LobbyMenu.set_reroll_cost(value, value_prem)` → `CharWindow.reroll_cost` / `reroll_cost_premium`, the reset dialog's two boxes) |
| 2 | `root.set_profile_cost` (its Flash side, `StatusPanel.setProfileCost`, is an empty function: nothing shows it, and no client message buys a profile) |
| 3 | `root.set_change_name_cost` |
| 4 | `root.set_premium_exchange_rate` |
| 5 | `root.set_clan_creation_cost` |

### 2.14 0x8E reputations (old 0x36 09) — PROVEN 0x271740 (0.21d 0x19E4CD)
`u8 n` (lc+0x1EA994; initial 0xFF = "never received"), `n × 8` bytes of `player_reputation` (memcpy; the same 8-byte record: u32 faction, u16, u16 points. The points are read as a u16 at +6: PROVEN 0x225AB0; the faction thresholds go up to 35250, so they fit), then **NEW `u8 quest_active_faction`** (lc+0x1EA998) → `root.set_quest_active_faction`. 0 → the client shows `choose_faction_message` (0x225D4C, 0x21D428). UI 0x225AB0 also uses the 0x96 mask (lc+0x1EE610).

### 2.15 0x8F ranked total and nickname (old 0x36 0A) — PROVEN 0x271300
`u32 ranked_players_total` (lc+0x1EE5FC; shown later by the 0x9D handler via `ranking_set_players_total_count` 0x227700: its only caller is 0x274D8E, so a later `8F` alone never changes the page's count, and the page shows no row past the count it has. Seen live 2026-10-12; srp follows the `8F` of a first ranking page with a `9D` with `show` off), `str nickname` → lc+0x1E55D0 (buffer size not measured; GUESS 64). Then `root.set_account_name` 0x223840 and `root.set_local_player` 0x209AC0. The layout equals 0.21d.

**The ranking page pairs answers with requests by count alone** (`PlayerRanking.as`, PROVEN in the script): `request_players` pushes the row indexes it asks for onto `_requestList` and `fillPlayers` gives each row of a `91` to the next index shifted off it, whatever place the row itself names. The list is never cleared (`onActivate` clears the cache only). A `3B` that is never answered (a server restart, a lobby link that was made again) therefore leaves its indexes at the front for as long as the game runs, and every later page is filled shifted by that many rows (seen live 2026-10-12: the 4th player on the first line, the others one line down, each with its right place number). Only a restart of the game clears it, or enough unasked rows: a row that finds the list empty goes to index 0 (`shift()` of an empty array is `undefined`, 0 as an `int`), and opening the page empties its cache (`onActivate`). The request is cut to the count the page has (`_totalPlayers`), so a page opened before the count grew asks for the old number of rows. The exe is no obstacle to unasked rows (PROVEN): the `91` reader 0x271EB0 pushes the rows onto a vector with no limit but the count byte and calls `ranking_set_data` 0x2271B0 with no condition, which calls `root.ranking_set_data` → `fillPlayers`. **What srp does about both (2026-10-12):** (1) every lobby sign-in is followed by a `91` nobody asked for (`push::ranking_flush`): the ranking's top as a first page's answer, then its first row again up to 255 rows; it takes every waiting index off the list, answers a first page that was open and unanswered, and leaves index 0 with the right row. (2) When the count of ranked players changes, every client in the lobby and not in a match is sent `8F` and a `9D` with `show` off (which stores the same flags and does nothing else, 0x271340..0x27138F; the `9D` handler then hands the count to the page), so the page already has the count when it is opened. `8F` also runs `set_account_name` and `set_local_player` with the unchanged name (the status panel and the chat take it again; `LobbyMenu.as`). The page's rules are `client_rules::RankingPage`, and its tests replay both pages seen live.

### 2.16 0x90 / 0x91 / 0x92 — PROVEN, layouts unchanged
- 0x90 (0x271D90): `u32, u16, u16` → `root.ranking_set_player{place, nickname, ranking, ranking_diff}` 0x2274B0 (0.21d 0x36 0B).
- 0x91 (0x271EB0): `u8 n`, n × {`str name (≤63)`, `u32`, `u16`, `u16`} (0x48-byte `player_elo_stats`) → `root.ranking_set_data` (0.21d 0x36 0C).
- 0x92 (0x271DF0): `u32 n`, n × 20-byte `quest_instance` memcpy → sort 0x277250 → `clear_quests` / `add_quest` 0x28DDD0 (0.21d 0x36 0D). Field meanings LIKELY unchanged (not re-traced), except the byte at +0x10: `add_quest` passes `proposal = (byte != 1)` (`cmp byte [q+0x10], 1; setnz`: PROVEN), and the quest window lists a proposal apart (`QuestWindow.addQuest`). So 1 = a quest the player has.

#### 2.16.1 The matches now and JOIN, on `3B` / 0x91: srp's own, not the original server's

> **Removed 2026-10-17** (the owner's: every PvP match is a room's, §2.16.3). The lobby answers neither the list (2 000 000 000) nor a JOIN (from 2 100 000 000) any more: they get no answer, as any reserved start it does not know (`lobby::RESERVED_START`). What follows is kept for the record; what still holds of it is how srp's own requests ride on `3B` / 0x91.

**An extension of srp. It needs srp's changed lobby movie (`lobby_menu.swf`) and no change of the exe; a stock game never asks and is never sent it.** The movie's "matches now" window lists the matches being played, as the launcher's page does, each with a JOIN button. It asks with the ranking page's own request and reads the ranking's own reply, so the exe carries both as it is. The format is version M2 (M1, pull request 54, was the list without JOIN). M2 was later extended without a new header (2026-10-15): `join` became a code that says why JOIN is off, each row got the match level, the JOIN answer got the code as a 4th field, and near a maintenance a maintenance rule decides JOIN. Every change is additive: the first M2 movie reads `join == 1` as on and any other number as off, ignores fields past `key`, and ignores the answer's 4th field.

Requests: the movie calls `ExternalInterface.call("request_players_range", start, count)`, which the game sends as `3B 00 <start u32> <count u8>` (`lobby_requests.md`). Both arguments are `int`-typed (the game reads the raw low 32 and low 8 bits; a Number argument arrives as 0).

- `start == 2 000 000 000` (`MATCHES_START`): the list of the matches now. `count` is the most match rows wanted (the header row is not counted); the server lists at most `min(count, 32)`.
- `start == 2 100 000 000 + key`, `key` from 1 to 99 999 999 (`JOIN_START`, `JOIN_KEY_MAX`): the JOIN button of the match with that listing key. `count` is not read (the movie sends 1).
- Every `start >= 2 000 000 000` is reserved and never a ranking page. Any other of them (2 000 000 001 to 2 100 000 000, and 2 200 000 000 on) gets no answer at all.
- The stock page asks only for indexes below the count of ranked players (`PlayerRanking.request_players`), so no stock game sends such a start. Nothing of the ranking is read, sent or kept for a reserved start: not the `8F` / `9D` / `90` of a first page, not the count the server keeps of what the page has.

Reply to the list: one ordinary `91`, rating and change 0 in every row. The movie tells it from a ranking page by the first row's place (`PlayerRanking.fillPlayers`: `rows[0].place >= 2000000000` goes to the window and leaves `_requestList` and the page's cache alone).

- Row 0, the header, always there (an empty list is not no answer): `name = "M2;<total>"`, `place = 2 000 000 000`. `total` is the matches open now, which may be more than the rows that follow. (M1 had `M1;`; the movie takes either.)
- Row `i` from 1, a match, the oldest first: `place = 2 000 000 000 + i`, `name = "<level>;<mode>;<phase>;<team0>;<team1>;<score0>;<score1>;<played>;<join>;<key>;<match_level>"`. ASCII decimal numbers with `;` between them; 60 bytes at the most (43 for a real match), within the 63 of the name. The movie ignores fields after the ones it knows, so a later version may append; a movie of M1 reads an M2 row as it read an M1 row. Example: `1;4;2;4;3;88;78;380;2;17;3` (Rudnya, Team Deathmatch, in play, 4 v 3, 88 to 78, 380 s played, JOIN off because the teams are already balanced, key 17, match level 3).

| field | meaning |
|---|---|
| level, mode | of the match's `maps_dict` entry (`modes_maps.md`): level 1..8, mode 2, 3 or 4 |
| phase | 0 gathering (made; the countdown has not begun), 1 countdown, 2 playing. A match that has its result is not listed |
| team0, team1 | each team's players who have not left; before the countdown the ones still to come count too |
| score0, score1 | batteries home (modes 2, 3) or tickets left (mode 4), of a match in play and from a world no older than 60 s; `-` for both otherwise |
| played | whole seconds since play began; `-` before it has |
| join | a code (table below): 1 when **this asker** would be put into the match at once by its JOIN (the rule below), else why not. The rows are the asker's own |
| key | the lobby's listing key of the match, 1 to 99 999 999: given when the lobby makes the match (or first lists one it did not make) and kept for the match's life; not the match id. The keys of a lobby start at a number taken from the time, so a window left open across a restart of srp does not name another match by an old key |
| match_level | the match's level as its rewards take it (`MatchSetup::match_level`, the row of `match_levels_dict`, fixed when it is made), decimal; `-` when not known. The movie shows it with the game's word `level` |

No player name, account id or match id is in it.

The codes of `join` (and of the JOIN answer's 4th field). A movie takes a code it does not know as 0. The movie's texts are its own (five languages); the English ones:

| code | the server's reason (`JoinCode`) | movie text (en) |
|---|---|---|
| 0 | no reason given: a match this lobby did not make, a key no match has, the match server says nothing of the match | (nothing) |
| 1 | may join now (the balance rule, or the maintenance rule) | (JOIN on) |
| 2 | the sides would count no more alike with the asker (also: a player alone would make an even match uneven, which the balance rule waits to do) | Teams are already balanced |
| 3 | no side takes the asker: a side full, no roster place left, the sizes rule; under the maintenance rule the weaker side full | No free place in this match |
| 4 | another player is joining: another player's place is held in it, or the match server admits nobody while the match is in play with time left (a joiner on its way in) | Another player is joining |
| 5 | not in play yet | (the phase word, "waiting" / "starting") |
| 6 | 120 s or less left (`JOIN_CUTOFF`), or the match is over | Match ends soon |
| 7 | the match takes no late joiners: rosters that do not grow, fixed teams | This match takes no new players |
| 8 | the asker is in a match, sent to one, or has a place in one | You are already in a match |
| 9 | the asker is in a squad with others | Leave your squad first |
| 10 | the asker's game does not join matches being played, or the server refuses its Play for its revision | Update your game |
| 11 | the asker had a place in this match already | You already played in this match |
| 12 | maintenance rule: the match ends after the maintenance starts (or less than a minute before) | Ends after the maintenance |
| 13 | the asker waits in the queue (it pressed Play and is searching) | Leave the search first |

Of the reasons that apply the server says the first in this order (the owner's of 2026-10-15): the asker's own that block every match, which the player must act on first (13, 8, 9, 10: a queued player sees 13 on every row, also of a match that does not admit or whose sides count alike), then the match's own, which hold for everyone (7, 5, 6, 4), then the asker's place of old in this match (11), then the maintenance (12), then the placement (3, 2). A match the lobby did not make says 0. The movie shows every reason in the yellow of the "starting" word.

Reply to a JOIN: one `91` with one row, `name = "J1;<result>;<key>;<code>"`, `place = 2 100 000 000`, rating and change 0; for example `J1;full;17;2`. `code` is the table's above (`ok` has 1); a movie that knows it shows the code's text on its message line instead of the result's generic text for codes 2 to 13. `result` is the code's, for the first M2 movie, which reads only the first three fields: 1 `ok`; 2, 3, 4 `full`; 0, 5, 6, 7, 12 `gone`; 8, 9, 10, 11, 13 `busy`. It is decided at the click:

| result | meaning |
|---|---|
| `ok` | a place is held for the asker for 10 s; its Play now puts it into the match. The movie closes the window and presses Play (`play_button_clicked`) |
| `full` | the match does not take the asker at once now: another player is joining (its place is held, or it is on its way in), the sides count alike, no side takes it (codes 2, 3, 4) |
| `gone` | no match being played has the key, the match takes no late joiners, it is not in play yet or near its end, or it ends after a near maintenance (codes 0, 5, 6, 7, 12) |
| `busy` | the asker cannot join now: in the queue, in a match or with a place in one, in a squad with others, a game that does not join matches being played or that the server refuses Play for its revision, or it had a place in this match (codes 8 to 11, 13) |

The rule, for asker A and match X (`join.rs`, `Core::join_fit`; the owner's of 2026-10-14: JOIN only where the player goes in at once with no wait, by the balance rule, and only to make a match that is uneven by value more even, counting the player's own value; a 3 v 4 that counts alike takes nobody):

1. X admits now (`play.rs` `late_admission`: growing rosters, teams not fixed, the match server admits, more than 120 s left, no earlier joiner on its way in), and the queue has X.
2. A is in the lobby (not in the queue, not in or sent to a match, no place in one), in no squad with others, its game plays in growing rosters, and A never had a place in X. (With `SRP_MIN_PATCH_REVISION` set, an older game is never in the lobby: `patch_check.rs`.)
3. No other player's place is held in X now.
4. `matchmaking_policy::place_late(the sides now, [A alone: what A counts for as the queue counts it (Core::strength_of), not waited out], the late-join rule)` puts A on a side, and the sides then count strictly more alike than without A: `|apart after| < |apart before|`, apart being what side 0 counts less side 1 (players, strengths, lead; `matchmaking.md` §9.2).

The maintenance rule (the owner's of 2026-10-15). While a maintenance is planned and the minutes to it, the number `9F` carries (`SessionStore::minutes_to_maintenance`), are 30 or fewer, the clients have Play off; JOIN then still takes a player into a match that will be over before the maintenance starts, even where that makes the sides less even. Point 4 of the rule is then replaced:

- X's time limit ends (now + the match server's `admission_remaining`) at least 60 s (`MAINTENANCE_MARGIN`) before the maintenance starts; otherwise code 12.
- `matchmaking_policy::place_joiner(the sides now, A's worth, the late-join rule)` puts A on the side with fewer players; of two alike in that, the side A played on in that match before (only a room's late join knows it: `room_late_join` reads it from the match's seating; no other caller passes one); then the one that counts for less (strengths + lead); then side 0 (the owner's of 2026-10-19), if that side has fewer than `capacity` players and the match has room; otherwise code 3. No "more alike" check, no sizes rule, no patience, nothing of a dramatic lead.

Points 1 to 3 (the match's admission, the asker's checks, no other player's hold) stay, and so do the 10 s hold, Play from the hold (checked again by the same rule: a maintenance moved nearer refuses it, not queued) and never the queue. Outside the window nothing of it applies, and a match that ends after a maintenance more than 30 minutes away is not refused for that. The queue's own late joins (`fill_running_matches`) do not use it. The log's JOIN and Play-from-hold lines say `maintenance rule` when it placed the player (or refused it for 12 or its weaker side).

Client check (original 0.26g exe, 2026-10-15): the movie's Play press works while the button is off for the maintenance. The AS callback `play_button_clicked` (0x28C320) tests only the lobby state (0x270660), the clan mode (+0x1EE6D4) and the profile count before it sends `2F`; the maintenance minutes (lobby_menu+0x30D8) are read in only 6 places of the whole exe, all in `update_play_button_state` (0x21FE92, 0x21FFAC) and the `9F` handlers (0x21CB45, 0x225E92, 0x225F27, 0x225F78). So `ok` followed by the movie's `play_button_clicked` sends Play during the window.

Several at once, and one hold per match. The lobby is one thread that reads one message at a time, so clicks are decided one after another, each against the places held before it. The match server takes one batch of joiners at a time: a reservation closes its admission (`MatchRegistry::join_running_group`), and the game opens it again only once every admission has entered the match. A second player's place in the same match could never be taken within its 10 s, so while one place is held the match is `full` (and `join` 4) for everyone else; the queue's late joins (`fill_running_matches`) pass it by too. After the first joiner is in and the match admits again, the next asker is decided anew. A hold ends with its 10 s, a JOIN of the same player (granted or not: it replaces the hold), its Play, or the player's link.

Play (`2F`) with a place held: the hold is used up, whatever comes of it. The rule is checked again; if the match still takes the player it goes in at once by the queue's own late-join steps: `Matchmaking::append_player` (the checks of `append_batch`, for a player who is in no queue) seats it only once the match server reserved the place (`MatchHost::join_running`), then "match found" and `82` as for any Play, and the log's "late join" line. Never another match, never a new one: otherwise the Play is refused with `84 2F 00 st_cant_change_ready_state` ("Error changing readiness status.", the game's text for a Play-like press it could not carry out, which srp already gives a clan team's ready) and the player is not queued. A Play with no place held (or one whose 10 s ran out) is the ordinary Play, as before.

The log: one line for each JOIN, `<name>: JOIN match 0x… (key k): ok (team t); sides a v b, counting X v Y; with it c v d, counting X' v Y'` (`ok (team t, maintenance rule)` under the maintenance rule; or `full` / `gone` / `busy` with the reason and `code n`, and the sides now), and one for each Play with a place held: `<name>: Play with a place held in match 0x… (key k): in, team t; …` (`in, team t, maintenance rule; …`) or `refused (<reason>; code n); not queued`.

What the exe does with it (PROVEN in the original 0.26g exe, client trace of 2026-10-14):

- The sender of `3B`, 0x270070, writes `3B 00 u32 u8` as it is called: it has no condition of the ranking page's, so the start goes out whole.
- The `91` reader 0x271EB0 copies each row's name into a 64-byte field of its stack **with no check of the length: 63 bytes are a hard limit**. The writer cuts every name to 63 as it does a player's (`elo_rows`); the widest list row the fields' types allow is 60 bytes (tested), a JOIN row 19 for a key the lobby gives (21 for any u32).
- The rows reach the movie as `place` = uint (the u32 whole), `ranking` = uint from the u16, `ranking_diff` = signed int from the i16. Nothing else in the client looks at them: no cache, no compare of names, no log.
- The movie does not run during a match, so no game asks while it plays.

Where it comes from: the match server's registry (`MatchRegistry::live_matches`), the source of the launcher's page too. Which match is listed and which score is told is one rule for both (`session::v026g::matches_now`). A match the lobby did not make itself (the lobby was started again while it went on) is listed with a key and `join` 0, and its JOIN is `gone` (code 0). The match level comes from there too (`LiveMatch::match_level`); the lobby tells a match near its end from one with a joiner on its way in by the match server's time left (`LiveView::time_left`; zero once the match is over).

An srp from before M1 answers the reserved start as a ranking page past the last row: an empty `91` (`91 00`), with no header; the window takes no header within about 6 s as "no matches". An srp of M1 answers a JOIN with nothing; the movie takes no answer within 6 s as "no answer".

Code: `push.rs` `matches_now` / `join_answer` / `JoinCode`, `dispatch.rs` `reserved_ranking` / `matches_now`, `join.rs`, `play.rs` `play` / `fill_running_matches`, `session::Matchmaking::append_player`, `session::matchmaking_policy::place_joiner`. Tests: `lobby/tests/writers.rs` `the_matches_now_are_ranking_rows_with_reserved_places` and `a_join_is_answered_with_one_row_at_the_join_start`, `core_tests/matches_now_tests.rs`, `core_tests/join_tests.rs`, and two of `v026g_end_to_end.rs` (the list in the first; `a_join_from_the_matches_list_puts_a_player_into_an_uneven_match_at_once`).

#### 2.16.2 "Player vs Bots", on `3B` / 0x91: srp's own (contract B1/B2, 2026-10-16)

**An extension of srp, as §2.16.1: srp's changed lobby movie asks, a stock game never does.** One player plays against the match server's own bots (`player_bots.md` §8): the movie's match settings panel has a "Player vs Bots" choice with a team size and a map. The rules, the settings and the owner's decisions are `servers/lobby-server/src/v026g/pvb.rs`; this is the wire.

Requests (`request_players_range(start, count)`, `int` arguments, as §2.16.1):

- `start == 2 050 000 000` (`PVB_STATUS_START`), `count` 32: the status, asked when the panel opens.
- `start == 2 051 000 000 + size × 1000 + map_id` (`PVB_CHOOSE_BASE`; up to 2 051 999 999, `PVB_CHOOSE_LAST`), `count` 1: the choice of `size` players a side on the `maps_dict` id `map_id`.
- Both are reserved starts (§2.16.1): an srp without the mode answers neither, and the movie takes no answer within 6 s as "not offered".

Reply to the status: one `91`, rating and change 0 in every row, at most `count` rows (the header always):

- row 0, `place = 2 050 000 000`: `B1;<code>;<gear>;<max gear>;<sizes>;<default size>;<default map>;<running>;<max matches>`, `sizes` joined by `.`. Example: `B1;0;3;5;6.8.10;8;61;2;5` (allowed; the player's equipment level 3 of at most 5; sizes 6, 8, 10, 8 first; Rudnya batteries first; 2 bot matches of at most 5).
- row `i` from 1, `place = 2 050 000 000 + i`: a map offered, `<map_id>;<level>;<mode>;<free>`. `free` is how many more matches against bots may start now (0: none, the map's slot is busy); with the mode off it is 0. The maps are the server's own list (16: the day of each level 1 to 8 in batteries, mode 2, and in Team Deathmatch, mode 4; never London, a timed battery or another weather): the movie shows only these.

Reply to a choice: one `91` with one row at the start asked:

- `B2;ok;<size>;<map_id>`: the choice is held for the player for 10 s; the movie then presses Play (`play_button_clicked`).
- `B2;no;<code>;<gear>;<max gear>;<running>;<max matches>`: refused, with the numbers the movie may show (the gear for code 1, the matches for code 7). The first three fields are DESIGN.md's `B2;no;<code>`; the four numbers were appended (a reader of three fields reads it as before).

The codes (`PvbCode`; the first that applies is said, in this order: 6, 5, 4, 3, 2, 9, 1, 8, 7):

| code | meaning |
|---|---|
| 0 | allowed |
| 1 | the equipment level (the client's own, of the kits that can be played) is over the most (`SRP_PVB_MAX_GEAR`, 5) |
| 2 | in a squad with others |
| 3 | waiting in the queue |
| 4 | in a match, sent to one, with a place in one, or holding a JOIN place (§2.16.1) |
| 5 | the game is too old (does not play in growing rosters) |
| 6 | the mode is off (`SRP_PVB`) |
| 7 | as many matches against bots run, or are held for players who chose one, as allowed at once (`SRP_PVB_MAX_MATCHES`, 5) |
| 8 | a planned maintenance is 30 minutes away or less |
| 9 | in its clan's team (clan-team Play) |
| 10 | (a choice only) a size or a map the server does not offer |

Play (`2F`) with a choice held: the rule is checked again and the match is made at once with this player alone on team 0, the teams fixed, against `size` of the server's bots, with `size − 1` on its side (2N − 1 bots in all): `85` "match found" and `82` as for any Play. Refused: `84 2F 00 st_cant_change_ready_state`, not queued. A Play whose choice ran out within 30 s is refused the same way and not queued; later it is the ordinary Play. A choice ends with its 10 s, a new choice (granted or not), a JOIN, the Play, or the player's link.

A match against bots never takes a late joiner (its teams are fixed).

Code: `push.rs` `pvb_status` / `pvb_choose_ok` / `pvb_choose_no` / `pvb_choice` / `PvbCode`, `dispatch.rs` `reserved_ranking`, `pvb.rs`, `play.rs` `play` / `launch`, `session::Matchmaking::seat_solo`. Tests: `lobby/tests/writers.rs` `player_vs_bots_rows_are_ranking_rows_at_their_starts`, `core_tests/pvb_tests.rs`, `v026g_end_to_end.rs` `a_player_against_the_servers_bots_plays_a_match_to_the_results`. The contract as the movie's builder has it: `private/pvb-mode/CONTRACT-PVB.md` (not in this repository).

#### 2.16.3 Rooms (the players' "servers"), on `3B` / 0x91: srp's own (contract S1, 2026-10-16)

**An extension of srp, as §2.16.1: srp's changed lobby movie asks, a stock game never does. Always on: every PvP match is a room's (since 2026-10-17), and Play puts a player into one.** A player makes a room (a team size, a max level, a wait, an optional PIN, a pool of maps), others find it in a list or by its key and join; once two are in, the members vote on three cards from the pool for 15 s, then the room's wait runs; when it is over a match starts on the map voted for with everyone free in the room, and after it they are in the room again. The rules and the owner's decisions are `session::v026g::rooms` and `servers/lobby-server/src/v026g/rooms.rs`; the design canvas is the owner's (`rooms-design` in the session notes). This is the wire.

Requests: `request_players_range(start, count)` with `int` arguments (§2.16.1). Every start is from **2 060 000 000 to 2 079 999 999** (`ROOMS_FIRST`, `ROOMS_LAST`): above "Player vs Bots", and inside an `int` (the replay poll's `uint` start above 2^31 has not been seen to reach the server, so rooms stay below it). `count` is read only by the list; the movie sends 1 for the others.

| start | request | answer |
|---|---|---|
| 2 060 000 000 | the list of the rooms, `count` rows at most (the header counted) | list |
| 2 060 000 001 | the asker's room | state |
| 2 060 000 002 | leave it (the room's LEAVE) | state |
| 2 060 000 006 | Play (the movie's Play button): into the open room that lets the asker in whose level is nearest its own, however far (then the fullest; none: a new room made for it, of any max level), straight into its match if one is played; no search shown | state |
| 2 060 000 007 | leave the queue the asker waits in | state |
| 2 060 000 008 | go into the asker's room's match being played now, without waiting for its `J` seconds (a member who comes while the match is played sees the room first and goes in 15 s later, `LATE_JOIN_WAIT`) | state |
| 2 060 000 003 + card | vote, card 0 to 2 | state |
| 2 060 000 009 | the asker's create window's start: the settings of its last room made by hand (since 2026-10-20) | form |
| 2 060 000 010 + page | the maps a pool may hold, page 0 to 15 | pool |
| 2 060 000 100 + place + 20 × check | (the owner of a room with a PIN; an open room turns nobody out) take out the member at that place of the state's list, 0 to 19; `check` is that state's `serial` modulo 495, and a kick whose check is not the room's now does nothing (a member came or went since the list was drawn) | state |
| 2 060 010 000 + PIN | the PIN (0 to 9999) of the asker's next join, or that its next room asks for, kept 60 s | `S1;K;1` at 2 060 010 000 |
| 2 060 100 000 + key | join the room with that key, 1 to 99 999 (the room's code) | state |
| 2 060 200 000 + key | wait in that full room's queue (the owner's of 2026-10-17): from no room only, one queue at a time (a new one leaves the last), its PIN given just before; a room with a place free is joined at once. The first of a queue is put in as soon as a place frees (straight into a match being played), if it still may come: in the lobby, free, its equipment within the max level; one who may not is out of the queue. Joining or making a room, Play, the end of its game take it out | state |
| 2 061 000 000 + chunk × 2^20 + bits | entries `chunk × 20` to `chunk × 20 + 19` of the pool of the asker's next room, an entry in by its bit; chunk 0 to 7; kept 120 s | `S1;B;<chunk>;<entries in so far>` at 2 061 000 000 |
| 2 070 000 000 + code | make a room: `code = ((((size − 6) × 16 + max level) × 10 + wait index) × 2 + PIN) × 3 + (min side − 1)`; min side 1 to 3 (the players a side it needs before its vote begins: 1 v 1 to 3 v 3); size 6 to 10 (a room has no level to make: it is worked out from its players), max level 0 to 15 (the highest equipment level let in; 0: any; not under the maker's own), the wait 15, 20, … 60 s (index 0 to 9), PIN 1 when joining asks for the PIN the maker gave just before | state |

A room made by a create (not by Play) is its maker's next form: its size, min side, max level, wait and pool are kept with the account (srp.db, `g26_room_settings`), the PIN is not. A room cannot be changed once it is made (the owner's): there is no request for it. Making or joining a room leaves the asker's room first. A room with a PIN and none given before is refused (code 8). A pool sent before the create is the room's; with none sent (or older than 120 s) the room has the whole match pool. An empty pool is refused (code 8).

Answers: one `91`, rating and change 0 in every row, the first row at the start named in the table, row `i` at that start + `i`. Each row's name is ASCII fields with `;` between them, the first `S1`.

- **List**, at 2 060 000 000: `S1;L;<total>;<my key>` (`my key` 0: the asker is in no room), then a row a room: `<key>;<level>;<max level>;<size>;<players>;<phase>;<secs>;<map_id>;<pin>;<name>`. `level` the room's level now: the match level of its members (`matchmaking::match_level` of their levels, as a match's), so it follows who is in it. `phase` 0 waiting for a second player, 3 the vote, open until the match starts (its 15 s and the room's wait together, `secs` left), 2 its match being played (`secs` played); 1 (the wait after the vote, the map chosen) is not sent since 2026-10-19, kept for the movies of revision 24 and before. `map_id` the map being played, or before its match the card that leads the vote alone (0: none, a tie or no votes: the start draws one). `pin` 1: joining asks for a PIN. `name` its maker's (the room's name, whoever owns it later), no `;`, 32 bytes at most. At most 31 rooms.
- **State**, at 2 060 000 001: in no room, one row `S1;N;<code>;<queue key>;<place>` (the full room whose queue the asker waits in and its place, 1 the first; 0 and 0 when none); in one, `S1;R;<key>;<level>;<max level>;<size>;<wait>;<phase>;<secs>;<owner>;<pool>;<members>;<serial>;<pin>;<next map>;<min players>` (`next map` always 0 since 2026-10-19, no map being fixed before the start; kept for the movies of revision 24 and before; `min players` the players it needs before its vote begins, 2, 4 or 6), then a row a card `C;<i>;<map_id>;<votes>;<mine>`, a row a member `P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>`, then `J;<seconds>` while the asker waits to go into its room's match being played, or `J;-1;<why>` while it waits and the match takes nobody now (`why`: 1 another joiner on its way in, 2 a game of the match loading, 3 30 s or less left, 4 full, 5 a maintenance near, 6 the match ending; since 2026-10-20, a movie before it reads -1 as no row; none once it is in, when its kit or gear cannot play, or when nobody says why), then `W;<key>;<max level>;<players>;<size>;<name>` when a room of a lower max level would take the asker now (its one-click switch: a Join of that key; none while it plays a match) (in the order they came; `gear` the equipment level the game shows; `fit` whether it can play the next match as it is now: 0 yes, 1 its active kit cannot (no weapon, too little ammo: Play's own check), 2 its equipment is over the max level; `away` 1: its game is gone for now, its place kept until its session ends, about 90 s), and `L;<map_id>`, its last match's map (0: none yet). `owner` 1: the asker owns it. `serial` changes whenever a member comes or goes. 27 rows at most (the header, 3 cards, 20 members, `J`, `W`, `L`).
- **Form**, at 2 060 000 009 (since 2026-10-20; a movie before it never asks): `S1;F;<saved>;<size>;<min side>;<max level>;<wait>;<modes off>`, then a row a chunk of 20 pool entries, `E;<chunk>;<bits>`, every chunk the pool has (one at least). `saved` 1: the asker's own (its last room made by hand), 0: the defaults (8, 1, 0, 30, the match pool, every mode). `modes off` the modes left out, `,` between them, empty for none. Bit `b` of chunk `c`: the tile (level and weather) of pool entry `c × 20 + b` is in; an entry goes into the pool made when its tile is in and its mode is not out. Fitted to now (`session::v026g::room_settings`): a map or mode no longer offered dropped, one offered later as the defaults have it (an experimental one out), the size, min side and wait within their ranges (the nearest wait), a max level under the asker's equipment raised to it; a choice that leaves no entry in gives the default pool.
- **Pool**, at 2 060 000 010 + page: `S1;M;<total>;<page>`, then 30 entries of that page: `<index>;<map_id>;<level>;<weather>;<mode>;<extra>`. The entries are the match pool (`SRP_MATCH_MAPS`, every playable map of `SRP_MATCH_MODES` by default) in its order, then the experimental maps (`SRP_ROOMS_EXTRA_MAPS`, `extra` 1). An entry is a `maps_dict` id: a level, a weather and a mode; the create window shows a tile a level and weather, with its render (`LevelN_day` / `_evn` / `_rain` / `_prerain` of the lobby movie), and puts in every mode of it. `index` is the bit that names the entry.

The codes of `S1;N;<code>` (`RoomCode`): 0 none (no room, nothing went wrong); 1 (rooms are off) is no longer sent; 2 no room has the key; 3 a wrong PIN, or none given; 4 the room is full; 5 the asker is in a match, sent to one, or has a place in one; 6 as many rooms as allowed (`SRP_ROOMS_MAX`); 7 the game does not play in matches that take players; 8 a room asked for with no map, or out of range; 9 a maintenance is 30 minutes away or less; 10 the owner took the asker out of that room (it may not come back); 11 the asker is in its clan's team; 12 the asker's equipment level is over the room's max level (or, making one, its max level is under the asker's; or its room's match started while it was: it was taken out of the room); 13 the asker's kit could not play when its room's match started (no weapon, too little ammo): it was taken out of the room. A code that a Play or a kick left is said once, with the asker's next state.

What happens around it:

- **The vote** is always there (the owner's): three cards (all different while the pool has as many) are drawn when the room is made, and the next three as soon as a match starts, so its players vote during it; a member changes its vote by voting again. The card with the most votes is played; of several alike, or none voted for, one at random.
- **The vote and the wait, one phase** (the owner's of 2026-10-17 and 2026-10-19). Once the room's minimum (2, 4 or 6, set by its maker) are in it who can play (in the lobby and free, `fit` 0; one who cannot is marked red in the room and counts for nothing), the vote runs (phase 3; votes may change) for 15 s (`rooms::VOTE`) and the room's own wait (15 s to 1 min) together, 10 s more after a match (`rooms::AFTER_MATCH`: its players come back from the results screen first): 30 s at the least. It is open until the match starts, whose map is its winner then (most votes; a tie or no vote drawn at random). It always runs whole, a full room (2 × size) too: its players may still be changing their equipment. Fewer members than its minimum stop it; a member who stops counting for a while (changing its kit) does not. A vote that ends with fewer than its minimum who can play goes back to waiting. A match that could not be made gives the room its vote and wait again, whole, its votes kept.
- **A maintenance** ([`MAINTENANCE_WINDOW`] minutes or fewer to one; the owner's of 2026-10-17): no room starts another vote or match and no room is made (code 9). A room not playing is disbanded at once; one playing plays its match out and is disbanded when it ends. Each member is told at its next state request (`S1;N;9`).
- **The match.** The members free in the lobby, each with its active kit if Play would take it and its equipment level within the room's max level (checked again here, not only at the door), go: two sides of at most the room's size, squads together (a squad too large for the free places of one side is split), as even as can be (`rooms::sides`), each squad of two or more a squad in the match (its own squad id), in a match of teams that are **not** fixed, on the card's map, with none of the server's bots, its rewards any match's. Each gets `85` "match found" and `82` as for any Play, also one whose room window is hidden. A member that cannot go (in a match, no good kit) stays in the room. The match is made at the match server before anyone is told; if that fails, or fewer than one a side can go, the vote and wait run again, and a full room waits them out too (no new try every tick). The members who could not play it (`fit` 1 or 2) are taken out of the room once it started (code 13 or 12, said at their next state request; they may come back), the owner's of 2026-10-17.
- **Late.** A member free in the lobby while its room's match is played sees the room first and goes into it 15 s later (`LATE_JOIN_WAIT`; at once with `2 060 000 008`) if the match admits players (from its countdown until 30 s are left, `LOAD_MARGIN`; no other joiner on its way in) and no maintenance is 30 minutes away or less, on the side with fewer players, then the side it played on in that match before, then the one that counts for less (`place_joiner`, a side at most the room's size; the owner's of 2026-10-19); otherwise it waits for the next match. A place in the match is never taken again: one that left the match by its own word and joined the room again goes in as a new place, its state fresh (the owner's of 2026-10-19).
- **After the match** its players are in the lobby and in the room, with the map as `L`, and the next vote starts on the next tick.
- **Play.** Play with no bots choice held (and no place in a match being played, which it goes back to) goes into an open room (no PIN, a place free, its max level not under the player's equipment, not one that turned it out; the one nearest its level however far, then the fullest), or makes one for it (8 v 8, any max level, 30 s, the whole pool), and is answered with the lobby state (`85 00`); the movie then asks for the state and shows the room. In a room Play does nothing but that answer (the movie greys Play out there).
- A member whose session ends leaves its room; its link going silent keeps its place while the session is kept. A member that leaves its room's match by its own word (its game's 0xE0 on the match link) leaves the room as by Leave (no code; it may join again). An owner who leaves hands the room to the member who came first; a room nobody is left in is gone.

Code: `lobby/rooms.rs` (`survarium`: starts, decoding, writers), `session::v026g::rooms` (the rooms and their rules), `servers/lobby-server/src/v026g/rooms.rs` (the lobby's side), `play.rs` `play` / `launch` / `finish`, `session::Matchmaking::seat_sides_in_pool` / `append_player`. Tests: `lobby/rooms.rs`'s own, `session::v026g::rooms`'s own, `core_tests/rooms_tests.rs`.

### 2.17 0x94 account services (old 0x36 0F) — PROVEN 0x2714D0 (0.21d 0x19E358)
`u8 n`, n × 10 bytes {`u8 type`, `u8 skipped`, `u32 a`, `u32 b` (skipped)}. Only type 0 is used: premium = true, hours = a → `root.set_premium_state` 0x227D80 (`st_less_then_hour`, `%d %s` hours/days). **Types 1..4 (faction loyalty bonuses → `set_faction_bonus`) are no longer read: the loyalty display is gone.** The 0.21d 8-entry stack overflow hazard is gone with it.

### 2.18 0x95 premium shop / bundles (old 0x36 11) — PROVEN 0x271650 (0.21d 0x19E40E)
`u32 n` (lc+0x1EE5F8), `u32 packed_size`, then raw LZMA2 that unpacks to exactly `n × 0xD0` bytes of fixed records (lc+0x1EE5F4, heap). Strings are no longer on the wire; labels come from the client config (`ui_descr`, `text_descriptions`). Record, from UI 0x227F60 (offsets PROVEN, meanings LIKELY):

| off | type | use |
|---|---|---|
| +0x00 | u32 | id (Flash `id`) |
| +0x04 | u32 | ui descriptor key (passed to 0x2CAC10) |
| +0x0C | u32 | kind: 0 → a `set_premium_store` entry {id, value, money_type, label, value2, discount}; ≠ 0 → `setup_bundle_data` {bundle_id, bundle_name, bundle_descr, cost, currency_type, icon_row, icon_column, bundle_properties, bundle_items} |
| +0x10 | u32 | value / cost |
| +0x14 | u8 | k items (**≤ 5**) |
| +0x18 + j·0x24 | 0x24-byte item | +0 dict_id, +0xC amount, +0x14/+0x18 two u32 used in the discount computation, +0x20 u32 kind (== 1 → counted) | **Corrected 2026-10-12, see premium.md §1: +0x04 is a bundle id of `shop_bundles_dict`; the item's kinds are at +0x1C and +0x20.** |
| +0xCC | u32 | money_type / currency_type |

The sign-in `need_shop_items` flag still means "no shop array yet" (lc+0x1EE5F4 == NULL → FF).

### 2.19 0x96 unlocked factions mask (old 0x36 12) — PROVEN inline 0x274E21
`u8 mask` → lc+0x1EE610. The faction/progress UI 0x225AB0 then runs immediately (0.21d had no UI call).

### 2.20 0x9C clan-team battle limits (NEW) — layout PROVEN 0x271480, meaning LIKELY
4 × u8 → lc+0x1EE5F0..F3. Byte +1 is the minimum and byte +2 the maximum clan-team size allowed to press Play (compared in `update_play_button_state` 0x21FF12 / 0x21FF1B, only in clan-team mode, see social.md §2.2; log "battle_cnt=%d min_cnt=%d max_cnt%d"). If never sent, both are 0 and a clan-team commander can never press Play; ordinary squads are not affected. Bytes +0 and +3 are unknown (+0 is used at 0x2A0FB8).

### 2.21 0x9D tutorial / awards (NEW) — PROVEN 0x271340
`bool show`, `u32 a` (lc+0x1EE600), `u32 b` (lc+0x1EE604). If show: the tutorial state machine 0x283190 runs (`tutorial_state_choose_nickname`, `tutorial_state_click_inventory`, `tutorial_state_equip_drop`, `set_tutorial_rewards`). The handler then shows the ranking total (from 0x8F) and calls 0x2768A0, which sends client message 0x45 with the UI language (§4). `a` is a bit set (tested `& 0x22` at 0x21D40C). GUESS: a = tutorial flags, b = reward state. **Corrected 2026-10-12, see premium.md §5: bit n of `a` is tutorial step n; the awards are `unique_awards.options`. The lobby tutorial's states and the `89` its weapon step waits for after `46 0A`: premium.md §5.1. srp sends every step done since 2026-10-19: no tutorial (premium.md §5).**

### 2.22 0x9E video tutorial link (NEW) — PROVEN 0x2713A0
`bool` (lc+0x1EE611), `u16 n`, then `n × u16` UTF-16 chars. They are copied to a stack buffer of about 0x1EC bytes (**n ≤ ~245; no check → stack overflow**), then converted to UTF-8 into lc+0x1E6144 (0x200 bytes) → `root.show_video_tutorial_link(bool)`.

---

## 3. Ordering requirements and crash hazards (0.26g)

| # | Rule | Tag | Evidence |
|---|---|---|---|
| H1 | Server ids must be in 0x80..0xAA and not 0x81, 0x86, 0xA0. The main handler indexes its jump table with no range check, and those three entries are NULL. (0.21d sent unknown ids to `read_squad_info` instead.) | PROVEN | 0x274A39..0x274A3F, table 0x275138 |
| H2 | During sign-in only 0x80 is accepted; anything else is logged and ignored. | PROVEN | 0x26FAC7 |
| H3 | **Send 0x89 (inventory) before 0x87 (own account).** Every item in the 0x87 item table is looked up in the inventory by item_id and a miss is a NULL dereference. | PROVEN | 0x270340..0x2703F1 |
| H4 | Profile slots hold indexes into the account's own item table (≤ M−1), not item ids. ≤ 3 profiles and ≤ 69 items, both unchecked. | PROVEN | 0x1F1120, 0x2225B6, ctor 0x2F7C50 |
| H5 | Every dict_id must exist in `items_dict`: the lookup now returns NULL instead of running off the array, but the UI dereferences NULL. | PROVEN | 0x1F0090, 0x2226C9 |
| H6 | Every `{id, state}` of an item must exist in the client's table of its kind: modifications for state 0, defects otherwise (the row is read unchecked). The three modifications of `83 46 07` and the one of `83 46 09` too. ≤ 8 modifications and ≤ 3 upgrades per item (fatal asserts). | PROVEN | 0x229AA0, 0x2F7C10, 0x1F1534, 0x1F1655 |
| H7 | 0x85 status 1: the new u16 search step must be non-zero (integer divide-by-zero). | PROVEN | 0x22565E |
| H8 | Strings: the status text ≤ 127; the 0x82 host ≤ 63; account/profile names ≤ 63; the account second name ≤ 14; 0x9E link ≤ ~245 UTF-16 units. No capacity checks anywhere. | PROVEN | §2 |
| H9 | 0x8A / 0x95 packed payloads must unpack (raw LZMA2) to exactly `n·8` / `n·0xD0` bytes. Decoder failure handling was not traced (GUESS: the array stays partly unfilled). | PROVEN (format), GUESS (failure) | 0x271540, 0x271650, 0x3EE810 |
| H10 | Send 0x9C with sensible min/max sizes before offering clan-team play, or a clan-team commander's Play stays disabled. | LIKELY | 0x21FF12 |
| H11 | Send 0x8E with a non-zero trailing quest_active_faction, or the client asks to choose a faction. | LIKELY | 0x21D428, 0x225D4C |
| H13 | **Until the client has read 0x80, every other message is thrown away** (H2), and 0x80 (unique class) is not ordered against the pushes (transport.md §4.1): a push that overtakes a lost 0x80 is lost for good, and a lost 0x89 before a delivered 0x87 is H3's crash. srp sends the 0x80 alone and the rest only once the client has acked it (`servers/lobby-server/src/v026g/core.rs`). | PROVEN (H2, the classes) | 0x26FAC7, transport.md §4.1 |
| H14 | 0x82 (unique class) is not ordered against 0x85 either. A 0x85 with state 1 read **after** the 0x82 sets the match id back to −1 and the team to 3 (§2.4), so the match chat and the leave message lose their match. srp sends a 0x82 only once the 0x85 sent just before it ("match found") is acked (the ordered channel delivers in order, so every earlier 0x85 is then read), and no queue state after it. | PROVEN (the handlers), LIKELY (the effect in play) | 0x270100, 0x274A46 |
| H12 | Removed 0.21d hazards: the 0x0A-before-0x05 nickname order (money no longer shows the nickname), the 0x36 0F 8-entry stack overflow (types 1..4 ignored), the 0x36 06 "profile not listed → misparse" (no profile_id any more), and the unbounded quest-id lookup (0.26g's 0x2CA7B0 returns a found flag and 0x28DE43 skips unknown quests — LIKELY). | PROVEN / LIKELY | §2 |

**Recommended push order after 0x80 (LIKELY; built from H1..H11 and the UI calls):**
1. `80 00 00` (client_state 0).
2. `8F` ranked total + nickname.
3. `8B` money.
4. `96` mask, then `8E` reputations (+ quest_active_faction).
5. `8D` six service prices.
6. `94` (n = 0 or premium).
7. `8A` prices (LZMA2), `95` shop. For an empty shop the reader 0x271650 still allocates a 0-record array, reads the packed size and calls the decoder with a 0-byte output (PROVEN: it has no `n == 0` branch as 0x8A's has). srp sends the LZMA2 stream of no bytes, its end marker: `95 00000000 01000000 00` (what an LZMA2 encoder writes for empty input; that the decoder takes it cleanly is LIKELY, as it is for `95 00000000 00000000`).
8. `89` inventory.
9. `87` own account (profiles + items + level/exp).
10. `8C` level + skills.
11. `92` quests (n = 0).
12. `9C` squad limits, `9D` tutorial (show = 1, every step done: no tutorial), `9F` online stats (see the social notes for the maintenance semantics).
13. `85 00` client state 0.

---

## 4. Client → server (summary; full layouts in `lobby_requests.md`)

All PROVEN there, at the sender addresses given in that file. In short:
- **Sign-in `2E`** (6 bytes, the UDP connect payload): `u32 session`, `bool need_shop_items`. `just_left_match` moved to `31`; `pending_match_stats_id` is gone (`#mstats` now sends `41 00 <u32>` directly).
- After `80` the client sends `32 00 <bool>` (echo of the 0x80 flag; purpose unknown) and the chat subscription `4F`.
- **Queries** are separate ids with a leading `00`: `34` squad member (→ 0x88), `37` skills (→ 0x8C), `39` reputations (→ 0x8E), `3B` ranking page (→ 0x91), `3C` quests (→ 0x92), `3D` victory-screen player (→ 0x93), `3E` services (→ 0x94), `40` faction mask (→ 0x96), `41` match stats (→ 0x97), `42` drop (→ 0x98), `43` daily bonus (→ 0x99), `45 <lang>` (→ 0x9E). **There is no request for the profile/account (old `21 02`) or money (old `21 05`)**: the server must push 0x87 / 0x89 / 0x8B itself after every change.
- **Actions:** play `2F` (was 20), clan-team play `30` (new), leave queue `31` (was 27), inventory `46 00..0A` (00 = old 23 move, same body; 01..0A new: upgrades, use, drop cards, decals, reroll, craft), shop `47 00..06` (was 24), skills `48` (was 25: no profile id, u16 skill ids, no perks), profile/account/clan `49 00..24` (01 autobuy account-wide, 02 active profile, others rename/nickname/comments/clan), squad `4A 01..04` (was 2A, same bodies), quest faction / tutorial `4B 00..02` (was 2B).
- **Chat:** `4C` text (was C1), `4D` important message (was C2), `4E` friendship (was C4), `4F` subscription (was C5, now 10 × u32 + u8 language), `50` answer (was C6). See `social.md`.

---

## 5. Results, social, clan (summary; details in `results.md` and `social.md`)

- **0x97 match stats** (was 36 13): the header gains an unused u32, two u32 team params and a u8 game_mode; each title name and player row gains a clan tag (row 0x50 → 0x60, still ≤ 20 rows); `player_result_full` is completely redesigned (score components, daily/premium bonus blocks, Elo before/after, quest money/rep, per-profile play time, one dropped item). After showing it the client sends `3C 00`. (results.md §1)
- **0x93** (was 36 0E): a full result, discarded, then an account body (§2.5) used to show the clicked player's equipment. (results.md §2)
- **0x98 drop, 0x99 daily bonus, 0x9B craft tables**: new (results.md §3–4).
- **Chat 0xA2/0xA3/0xA4**: a clan tag (≤ 15) after the sender name; friendship replies other than lists 7/8/9 are now parsed as search lists, so **the 0.21d `u8 result` reply crashes the client**; the client no longer asks for its friend/ignore lists, so the server pushes `A3 07`/`A3 08`. (social.md §1)
- **Squad 0xA1**: same layout; the second byte is the commander index; a soldier's Play is locked while the commander is ready; the commander needs ≥ n−1 ready. (social.md §2)
- **Clan 0x9A** (7 sub-types) and **0x49 06..23**: new. (social.md §3)
- **0x9F online stats**: same layout; 0 minutes only shows `st_maintenance` (0.21d quit the game); Play still needs > 30. (social.md §4)

---

## 6. New 0.26g lobby features and the messages they need

| feature | server → client | client → server | needed for a usable server? |
|---|---|---|---|
| account-wide level/skills, 3 profiles per account | 0x87, 0x8C | `37`, `48`, `49 02` | **yes** |
| item condition/repair/reroll, upgrades, modifications with state, rarity | 0x89, 0x87 item fields | `46 01..0A`, `47 03..06` | item fields yes (send 0 / empty lists); actions can be denied (`84`) |
| compressed prices / bundle shop | 0x8A, 0x95 (LZMA2) | `47 02` | 0x8A yes (LZMA2 encoder needed); 0x95 can be n = 0 |
| crafting | 0x9B | `46 0A` (lobby_requests.md) | no |
| drops / drop cards | 0x98, 0x97 drop block | `42`, `46 03` | no |
| daily bonus | 0x99 | `43` | no |
| tutorial, video link | 0x9D, 0x9E | `4B 01/02`, `45` | no (send 0x9D show = 0) |
| clans and clan teams | 0x9A, 0x9C, 0xA4 types 7/10 | `30`, `49 06..23` | no (deny or ignore) |
| matchmaking search properties | 0x85 status 1 (u16 + 20 × u16) | — | yes (u16 ≠ 0) |
| change name / nickname check / comments | 0x8D prices | `49 00, 03..05` | no |
| premium discounts / post-match premium offer | 0xA8, 0xA9 | `47` variants | no |
| admin text | 0xA5, 0xA6 | — | no |
| forced sign-out | 0xAA | — | recommended (double login) |
| quest active faction | 0x8E trailing byte | `4B 00` | yes (non-zero) |
| **removed**: loyalty bonuses (0x36 0F types 1..4), perks, per-profile skills/level/autobuy, profile leveling (36 07), browser HTTP step, separate chat socket | | | |

---

## 7. What srp-021d must change (lobby core)

Paths in `srp-021d` (the srp-026g copy has the same layout). Request handling is listed in `lobby_requests.md` §6, chat/squad/clan in `social.md` §6, results in `results.md` §7.

| message | 0.21d | 0.26g | status | srp change (file: function) |
|---|---|---|---|---|
| transport | TCP, `u8`/`00 u16` frames | UDP match transport, connect packet carries `2E` | CHANGED | `servers/lobby-server/src/lobby_server/frame.rs` (whole file) and `mod.rs`: `run` — replace the TCP framing with the 0.26g UDP transport (transport.md); one connection now also carries chat |
| browser HTTP | lobby address via HTTP | from the login reply | CHANGED | `servers/browser-server` unused; `login-server` reply (login_chat.md) |
| ids | 0x30..0x3A, 0x36 xx | 0x80..0xAA flat | CHANGED | `crates/survarium/src/v021d/lobby.rs`: `status_writer(kind)` → write the single 0.26g id; every writer below |
| signed in | `30 st` | `80 st flag` | CHANGED | `lobby.rs`: `connection_successful` |
| connect to match | `33 host port match team` | `82 host port match team u32 match_session` | CHANGED | `lobby.rs`: `connect_to_match_server` (+ the match session) |
| client state | `36 00 …` | `85 …`, status 1 + `u16 step (≠0)` + `20 × u16` | CHANGED | `lobby.rs`: `client_state`; `lobby_server/mod.rs`: `queue_status` |
| profiles + contents | `36 01` list, `36 02` per profile, slot items inline | `87`: one player_profile (≤ 3 profiles, shared item table, slots = item indexes, u128 modifier mask) + revisions + active id + autobuy + level/exp | REWRITTEN | `lobby.rs`: `profiles` + `profile_contents` → one `account` writer; `write_item` → 0.26g descriptor; `crates/survarium/src/v021d/account.rs` model (account-level level/exp/skills/autobuy) |
| squad member profile | `36 10 pid body rev` | `88 body rev` (no pid) | CHANGED | `lobby.rs`: `squad_member_profile` |
| inventory | `36 03 n × descr(mode 2)` | `89 n × descr(0.26g)` | CHANGED | `lobby.rs`: `inventory`, `write_item` |
| item descriptor | `u16 dict, u32 id, u16 cond, [u32 amount], u8 k, k×u32` | `u32 id, u16 dict, [u32 cond, (u32 repair), u32 reroll], u8 n1, n1×{u32,bool}, u8 n2, n2×{u32,u32,u64,u8}` | CHANGED | `lobby.rs`: `write_item`; condition scale ×10000 |
| prices | `36 04 u16 n, n×8` | `8A u16 n, u32 packed, LZMA2(n×8)` | CHANGED | `lobby.rs`: `prices` + an LZMA2 (raw, no xz header) encoder dependency |
| money | `36 05` | `8B`, same body | ID ONLY | `lobby.rs`: `money` |
| skills | `36 06 pid lvl tot next start n×(u8,u8) m×u8` | `8C lvl tot start next n×(u16,u8)` | CHANGED | `lobby.rs`: `skills`; `crates/survarium/src/v021d/skills.rs` (u16 ids, per account) ; `lobby_server/mod.rs`: `skills_reply` |
| service prices | `36 08 3×u32` | `8D 6×u32` | CHANGED | `lobby.rs`: `service_prices(reroll, add_profile)` → six values |
| reputations | `36 09 n×8` | `8E n×8 + u8 quest_active_faction` | CHANGED | `lobby.rs`: `reputations` |
| base info | `36 0A u32 str` | `8F`, same body | ID ONLY | `lobby.rs`: `base_info` |
| Elo / list / quests | `36 0B/0C/0D` | `90/91/92`, same bodies | ID ONLY | `lobby.rs`: `player_elo`, `elo_list`, `quests` |
| services | `36 0F` premium + loyalty | `94`, only type 0 used | CHANGED (loyalty dropped) | `lobby.rs`: `services`; `crates/survarium/src/v021d/loyalty.rs` no longer shown |
| shop | `36 11` strings | `95 u32 n, u32 packed, LZMA2(n×0xD0)` | CHANGED | `lobby.rs`: `shop_items` (keep n = 0 → `95 00000000 00000000`) |
| faction mask | `36 12` | `96`, same | ID ONLY | `lobby.rs`: `unlocked_factions` |
| permitted / denied | `34` / `35` | `83` / `84` | CHANGED | `lobby.rs`: `bought`, `sold` (drop profile list), `quest_changed`, `autobuy_set` (now `83 49 01 on`), `operation_denied` (id), `premium_bought` |
| sign-in order | 0x0A before 0x05; 0x12 before 0x09; 0x03 before 0x02 | 0x89 before 0x87 (crash otherwise) | CHANGED | `lobby.rs`: `sign_in_push` (§3 order) |
| new pushes | — | 0x9C, 0x9D, 0x9E, 0xAA… | NEW | `lobby.rs`: new writers as needed (§6) |

---

## 8. Open questions

1. The 0x80 second byte and the client's `32 00 <flag>` reply: meaning and expected answer (lobby_requests.md §1.2).
2. Account body: the u8 at +0x45EC (LIKELY team), the second string at +0x48 (≤ 14; the clan abbreviation, static_info.md §3), the u32 at +0x45F4 (new-player protection, static_info.md §3). The profile state: 2 = premium profile (§2.5); which profiles the original server marked is open; srp marks none (§2.5).
3. Names and order of the 68 `player_params_modifiers_enum` bits (u128 mask): **now in static_info.md §3.1** (same reader 0x1F1070; that note also proves the account u32 at +0x45F4 is the new-player protection value and names the second string the clan abbreviation). Until the skill values are mapped, send mask 0 until the skill boosters are mapped (0.21d sent skill boosters here; in a match the player reads its modifiers only from here in 0.21d — re-check for 0.26g in the match notes).
4. ~~Upgrade record fields other than id and param (u32, u64).~~ The cosmetic's dict id and the decal editor's value (items.md §2).
5. ~~Whether the reroll-cost pair in 0x8D is (generic, premium).~~ It is (§2.13).
6. The 0x95 record fields at +0x04/+0x08 and the item sub-record beyond dict/amount; and whether `95 n=0` with packed size 0 is accepted cleanly by the LZMA2 call (the return value is ignored, so LIKELY harmless).
7. 0x9C bytes +0 and +3; 0x9D fields a/b.
8. The exact LZMA2 encoder settings: the client only sets the dictionary size from the unpacked size; any standard raw LZMA2 stream whose dictionary does not exceed it should decode (LIKELY). Not tested.
9. Behaviour when a lobby push arrives before the Flash movie is ready (0.21d spec §6 risk) — not re-checked.
10. The quest record (20 bytes) fields were not re-traced; the lookup of an unknown quest id now looks safe (0x28DE43), LIKELY.
