# Facts about the 0.26g client

> **Version scope: client 0.26g only** (build 2777, build string "0.26g0",
> January 2015, no PDB). Every address, message id, data value and constant
> here is 0.26g's. The 0.21d facts are in CLIENT-FACTS-021D.md; do not mix
> them: no 0.21d message is wire-compatible with 0.26g.

> **Status, 2026-10-12.** The facts on this page come from reading the exe
> and `resources.db`, from scripted clients and from tests; each says how
> sure it is (PROVEN, LIKELY, GUESS). Since that day the server is live and
> real clients play on it (SRP-026G.md); a fact that live play confirmed or
> corrected says so, and `srp-026g/WORK.md` has the day's findings. The
> players' exe is the full patch (section build, 50 ms hash interval); its
> upload code runs in every live match. It does not have the battery rule
> patch of that evening yet (the table of patch sites below).

Addresses are virtual addresses in `survarium.exe` 0.26g (image base
0x10000; `.text` VA 0x11000 = file 0x400, so file offset = VA − 0x10C00 in
`.text`). Each fact keeps the tag its note gives it: **PROVEN** (read in the
code or data at the address given), **LIKELY**, **GUESS**. A line with no tag
is PROVEN in its note. Detail: `srp-026g/docs/notes/` (start with
its `README.md`); "lobby.md §3" below points there. The server: SRP-026G.md.
Matches: MATCH-SYNC-026G.md.

## The exe and how its functions were named

| Fact | Value | Note |
|---|---|---|
| Build | "0.26g0" (string 0x9DAE98), build 2777, compiled 2015-01-14 | README.md |
| File | `downloads/v026g/extracted/Survarium/game/binaries/x86/survarium.exe`, 12 894 840 bytes, sha256 `ac33d993…f153` (full value in `patch_026g.py`) | client_patch.md "Exes used" |
| Layout | image base 0x10000, 7 sections, no ASLR flag (DLL characteristics 0x8000). Only 0x5D zero bytes after `.text` (0x86F5A3–0x86F5FF) | client_patch.md §3, §3.1, §3.3 |
| Symbols | **No PDB** (none for any public build after 0.21d) | README.md, RE-TOOLKIT.md |
| `resources.db` | 2015-01-17, 6 285 533 906 bytes, 78 961 entries, CRC 0x4A01F332 | data.md, client_patch.md §0 |
| OpenSSL | 1.0.1h (string 0x8B3F90) | login_chat.md §3 |

How names were found:
- **Own strings.** Assert and `__FUNCSIG__` strings name functions directly
  (244 FUNCSIG strings), e.g. `on_match_packet_received` 0x27A300 (string
  0xA08908).
- **Matching to 0.21d.** `tools/re-kit/v026g/functions_matched.txt`, made by
  `tools/re-kit/common/scripts/match_functions.py`. Line format:
  `<0.26g VA> <length> <0.21d name(s)>  # <confidence>; <method>: …; 021d <VA> len <length>`.
  22 478 of 35 070 0.21d functions matched (16 956 high, 5 522 medium).
  Library code is near 93 % (Scaleform); **game code only about 30 %, network
  code about 18 %**: 0.26g game code was recompiled with other settings.
  Expect a few percent of medium rows to be wrong. Detail: client_patch.md
  "Function map".
- **By hand.** The list in client_patch.md §5.
- Data dumps: `tools/re-kit/v026g/data/`.

## Start-up and command line

- **Every stock 0.26g client stops at start** with "Resources archive is
  corrupted" (string 0x9DEF00). `vostok::crypt::verify_sign` 0x13310 takes
  the "survarium.com" certificate from the exe's own Authenticode signature,
  and 0x12A10 checks its chain for today. The certificate ran out on
  2015-06-06. Patch: 0x12A10 `83 EC 2C` → `31 C0 C3`. **PROVEN live** (the
  only live-proven fact here). The signature blob must stay readable:
  verify_sign takes the certificate out of it first and fails without it,
  also in the default build since patch revision 12, which skips the check
  of the `resources.db` file table with its key (client_patch.md §0.1).
  client_patch.md §0.
- The game mounts one archive, `../../resources.db`, at `resources`
  (query_db 0xE7D90, its one call of query_mount_archive at 0xE7E3E); a
  later mount at the same path wins file by file. Since patch revision 12
  the default build mounts each `resources.db.patch_<n>` there (n = 1..255, in order) after it
  (README patch 27). Not run in a game. client_patch.md §0.2.
- The login server key is **`-server=host:port`** (key object 0xA9C550), not
  `-client=`. With no key and no replay key the game stops: "Game server
  hasn't been specified" (0x9FA990). There is no default address.
  login_chat.md §1.
- **One client per Windows session**: mutex
  `survarium_or_its_installer_is_already_running` (string 0x9DAECC), checked
  in 0x124D0. client_patch.md §1.

| Option | What it does | Tag | Note |
|---|---|---|---|
| `-server=host:port` | login server | PROVEN | login_chat.md §1 |
| `-splash_screen=0` | no splash (0.21d: `-no_splash_screen`) | used in the shortcut | |
| `-localization=ukrainian` | language. The client maps russian, english, ukrainian, german, polish to its chat room and language byte | used in the shortcut; mapping PROVEN | login_chat.md §5.6 |
| `-minimum_resources_memory_size=<MiB>` | The floor of the resources arenas: option object 0xA83080, a float read at 0xE9966; default 0x150 = 336 MiB. PROVEN (read 2026-10-12) | used in the shortcut (768) | |
| `-mt_allocator=<MiB>` | The global multithreaded allocator's arena: option object 0xA80150, read at 0xE0FF5; without the key 16 + 360 = 376 MiB, with it `value << 20`. PROVEN (read 2026-10-12). A different arena from the line above. Upstream ran out of it loading a map (live, Wine) and passes 768 | **not in the shortcut yet**; in `make_client_026g.py` and the web page | `upstream_compare.md` §10 |
| `-account_name=`, `-account_password=` | fill in the login (key objects 0xAA4910, 0xAA4B40) | PROVEN; that the launcher passes them LIKELY | login_chat.md §1 |
| `-spectator` | "connect as spectator" | PROVEN it exists | login_chat.md §1 |
| `-hash_check_interval_in_ms=N` | world hash interval; default 100 (0x27CC4F), option object 0xAA19F8 | PROVEN; the `=N` form LIKELY, not tried | match.md §14 |
| `-force_send_hashes` | lifts only the "connection is up" test at 0x27CD3C | PROVEN | match.md §14 |
| `-debug_hash_mismatch` | flag 0x1F1D32C: 128 history items, parses 0xF6, connect byte 0xFF | PROVEN | match.md §1.1, §2.1 |
| `-reject_inputs` | drop late or early relayed inputs instead of moving them (option 0xAA2290) | PROVEN | match.md §5.1 |
| `max_ping_time_in_ms` | option 0xAA1DF8, default 250, 250..1000; sets the event horizon | PROVEN; whether a player can set it is open | match.md §2.1 |
| `-join_net_messages` | several messages in one datagram (option 0xAA6E20); off by default | off by default LIKELY | transport.md §2.5 |
| `max_unacknowledged_packets_count` | default 256 (0xAEFA44), hint "32 - 1024" | PROVEN | transport.md §2.5 |
| `qos`, `match_bandwidth_in_kbits` | Windows qWAVE on the client's own socket; nothing on the wire | LIKELY client-only | transport.md §8 |

The window: 0.26g shares `Documents\survarium\user.cfg` with 0.21d and needs
the `HIGHDPIAWARE` compatibility flag.

## Login (`login_chat.md` §3, §4)

One TCP connection, then TLS on the same connection. No browser server, no
presence ping, no sign-out message.

| Step | Bytes | Where |
|---|---|---|
| 1, client, plain | `01`, `u8 n`, name[n], version[12] = "0.26g0" + zeros, `u8` EULA flag, `u32` queue id. At most 0xA7 bytes; the name buffer is 100 bytes | 0x3807D0 |
| 2, server, plain | one byte: `0B` → TLS; `0A` → wrong name or password; else shown as that code | 0x3804F3 |
| 3, client, TLS | `02` + 40 hex chars (remembered login) or `03` + 40 hex chars + `u8 len` + password (typed, ≤ 47 chars) | 0x380160 |
| 4, server, TLS | `08`, `u32` session, `u32` lobby IP, `u16` lobby port (11 bytes); the client reads up to 82 | 0x37FB10 |

- **The digest** is the plain lowercase hex SHA-1 of the password (SHA-1
  0x376120, `%08x` 0x3761F0). A remembered login sends only the digest, so
  the server must check passwords by the digest alone.
- **Certificate check:** the client verifies the server against the file
  `../../resources/ssl/survarium_login_server.crt` (path string 0xA21E98,
  relative to `binaries\x86`). The callback 0x37C2B0 just returns
  "preverified"; there is no host-name check. The shipped file is another
  key and ran out on 2015-05-09, so srp's certificate must be copied over it
  (that this works as it does for 0.21d: LIKELY). A bad certificate shows
  "game: SSL certificate verification failed" (0x9FF578).
- TLS method still SSLv23: LIKELY (not traced).
- **The IP** on the wire is `u32::from(ip).to_le_bytes()` (127.0.0.1 =
  `01 00 00 7F`): the client passes it through `htonl` (WS2_32 ordinal 8 at
  [0xAEFFF4]). The port is raw little-endian.
- **Status codes** (switch at 0x21A6C0, table 0x21BD04; code N shows status
  N − 6 for the bans):

  | Code | Meaning |
  |---|---|
  | 0x08 | connection info (go to the lobby) |
  | 0x0A | invalid user name or password |
  | 0x0B | name accepted (step 2 only) |
  | 0x0C | attempt interval violated |
  | 0x09, 0x0D..0x11 | not handled: plain "error connection" |
  | 0x12..0x1F | banned, by reason: bad_language, insult, faschism, politics, flood, advertising, discuss_admins, reallife_treating, account_trading, incorrect_info (0x1B), administrator (0x1C), nickname, cheating (0x1E), teamkilling |
  | 0x20 | access level restriction |
  | 0x21 | already signed in |
  | 0x22 | invalid version |
  | 0x23 | registration not confirmed (new) |
  | 0x24 | EULA check failed |
  | 0x25 | added to the online queue (`25`, 1 byte, `u32` queue id, `u32` place, `u32` total) |
  | 0x26..0x2C | unknown, never read by the client |
  | 0x2D | the client's own failure code |

- Sign-out sends nothing (0x385860). The only UDP ping left is the queue
  ping `03 u32 queue_id` every 10 s (0x38AB30).

## Transport: reliable UDP (`transport.md`)

Matches **and the lobby** run on one transport (`udp_match_connection`).

| Fact | Value | § |
|---|---|---|
| Header | 3 bytes: `u8 seq, u8 rseq, u8 ack0`; 0–17 more ack bytes **at the end** of the datagram, written in reverse | §2.1 |
| Sequence numbers | 8 bits, half range 128 | §1 |
| Ack window | 128 bits as 7-bit groups; n tail bytes carry 7n + 6 bits; bit 7 = "another ack byte follows" | §2.2 |
| Single or multi | bit 0 of the last ack byte: 1 = multi | §2.2 |
| Datagram | at most 0x1FC = 508 bytes, at most 252 messages | §1 |
| Message | `id [x] [len] payload`; x = one byte, order id (flags 0x40) or unique id (0x20) | §3.1 |
| Lengths | fixed ids carry no length. Variable ids in a multi datagram: `u8` (≤ 254) or `FF u16`, payload only | §3.1 |
| Multipart | `FF id x part [count]`; part 0 holds 500 − n_tail bytes, later ones 501 − n_tail; at most 255 parts | §5 |
| Low-level ids | `FA` start disconnect, `FB` confirm, `FC` keep-alive, `FD` instant disconnect; **`FE` crashes** | §3.4 |
| Keep-alive | `s r ack0 FC` | §3.4 |
| Timeout | 5000 ms (match: 120 000 under a debugger); idle keep-alive every 100 ms (match) or 500 ms (lobby) | §4.1, §6 |
| Who ticks the lobby link | the logic thread, every 500 ms, in every state: lobby, loading, match, window not active (`on_waiting_for_render`). Silent only while that thread is held up. After 5 s of server silence: `root.show_sync`, a reconnect with the same session every 5 s, the login screen after the fifth failure | §6.1 |
| Connect | no handshake; the first message is the connect (0xD8 match, 0x2E lobby). First match datagram `00 FF 00 D8 <u32 session> 00` (LIKELY) | §7 |
| Ports | 0xE3 `{u16 min, u16 max}`; the client probes each with 0xD9 and then sends to a random one. Use min = max | §7 |

What breaks the client or the link:
- A **header-only datagram** (no message) fails the parse: the whole
  datagram is dropped, its acks too (0x3FAF55). §2.3.
- A datagram whose `rseq` is **newer than the client's last sent seq** is
  dropped whole (0x3F1C40). Only echo a sequence the client sent. §2.3.
- The server's first sequence must not be 0xFF (the client's initial value).
  Start at 0. §1.
- A fixed-length id in a single datagram must match its length **exactly**.
  §3.1.
- The client **stops sending** once its next sequence is 128 ahead of the
  newest acked one, keep-alives too (0x3FC2C3). Ack every datagram. §2.5.
- 256 unacked reliable packets disconnect it (0x3FC146). §2.5.
- Never have 128 unique ids outstanding: an id 128 behind counts as new.
  §3.3.
- A resend is fixed (no jitter) and each id has a **lifetime**; after it the
  sender drops the message silently (0x3FE470). §4.
- Plain messages are not deduplicated: a resent copy is delivered again.
  §3.3.

Match ids are plain, except E4, E5, E6, E7, E8 and F6, which are unique and
share one counter. **No match id is ordered.** Full table of lengths,
lifetimes and resend times: transport.md §4. In short: E5, E6, E7, ED, EE,
DB, DF, E1 are sent once; E8 lives 1.75 s; relays, roster changes and
discards (E9..F2) live 250 ms; inputs DC/DD live 500 ms.

The **lobby** table (vtable 0xA07FA8; transport.md §4.1):
- every lobby message is variable length and unique; client ids 0x2E..0x50,
  server ids 0x80..0xAA; resend 1000 ms;
- most server ids are also **ordered** (flags 0x60: 83, 84, 85, 87..99,
  9B..9E, A3, A5, A6, A8, A9), on their own counter from 0. The client
  holds a message until its predecessor arrives;
- 80, 81, 82, 9A, A0, A1, A2, A4, AA are unique only: **not ordered against
  the rest**;
- 0x9F is sent once, plain, with no x byte;
- an order id more than 127 ahead is dropped silently, its datagram still
  acked, and the ordered channel then waits for ever;
- nothing expires by time: an unfinished split message blocks every later
  order id.

## Lobby: server → client (`lobby.md`)

Dispatcher `lobby_client::on_message_received` 0x274A00, jump table
0x275138. Sign-in phase handler 0x26FAB0. Reads have **no bounds checks**;
strings are `u8 len` + bytes copied with no capacity check.

| Id | Meaning | Reader |
|---|---|---|
| 0x80 | signed in: `u8 state, u8 flag`. Send `80 00 00` | 0x26FAB0 |
| 0x82 | go to the match server: host (≤ 63), `u16` port, `u32` match id, `u8` team, `u32` game time | 0x274A46 |
| 0x83 / 0x84 | operation permitted / denied (`u8 op`, `u8`, text) | 0x274E92 / 0x274F14 |
| 0x85 | client state (queue status) | 0x270100 |
| 0x87 | own account: up to 3 kits, item table, level, experience, autobuy | 0x270260 |
| 0x88 | a squad mate's account | 0x274BCE |
| 0x89 | inventory | 0x270400 |
| 0x8A | prices (LZMA2) | 0x271540 |
| 0x8B | money: `u32` silver, `u32` gold | 0x274C89 |
| 0x8C | level and skills | 0x270570 |
| 0x8D | six service prices | 0x26E450 |
| 0x8E | reputations + `u8` quest faction | 0x271740 |
| 0x8F | ranked total + nickname | 0x271300 |
| 0x90, 0x91 | rating, rating list | 0x271D90, 0x271EB0 |
| 0x92 | quests | 0x271DF0 |
| 0x93 | a results-screen player's gear | 0x271A30 |
| 0x94 | premium | 0x2714D0 |
| 0x95 | shop and bundles (LZMA2) | 0x271650 |
| 0x96 | unlocked factions mask | 0x274E21 |
| 0x97 | match results | 0x2718C0 |
| 0x98, 0x99, 0x9B | drop, daily bonus, craft tables | 0x271AE0, 0x2769B0, 0x2769F0 |
| 0x9A | clan info (7 kinds) | 0x271FD0 |
| 0x9C | clan-team size limits | 0x271480 |
| 0x9D, 0x9E | tutorial state, video link | 0x271340, 0x2713A0 |
| 0x9F | online stats: players, games, minutes to maintenance | 0x271860 |
| 0xA1 | squad info | 0x274720 |
| 0xA2, 0xA3, 0xA4 | chat text, friend lists, important message | 0x26B6F0, 0x26ABE0, 0x26BBF0 |
| 0xA5 / 0xA6 | admin text: two strings (first ≤ 63; second LIKELY ≤ 63) → `root.set_admin_text(a, b)` / one skipped byte, clears it | 0x27507E / 0x2750C0 |
| 0xA8, 0xA9 | premium discount store, discount | 0x274FF8, 0x274FBA |
| 0xAA | forced sign-out, shows "already online" | 0x2750EB |

Reading rules that matter (lobby.md §3 has the full hazard list H1..H14):

| Rule | Tag | Evidence |
|---|---|---|
| **Never send 0x81, 0x86, 0xA0 or 0xA7.** 0x81, 0x86, 0xA0: the handler's table entry is 0. 0x86 and 0xA7: the transport's own table jumps to address 0 before any handler runs | PROVEN | lobby.md H1; transport.md §4.1 |
| An id outside 0x80..0xAA is dropped by the transport ("invalid message type", 0x3FA424) before the handler's unchecked table | PROVEN | transport.md §4.1 |
| **Until the client has read 0x80, every other message is thrown away.** 0x80 is not ordered against the pushes, so send it alone and wait for its ack | PROVEN | H2, H13 |
| **0x89 (inventory) before 0x87 (account).** Every item in 0x87's table is looked up in the inventory by item id; a miss is a NULL dereference | PROVEN | H3, 0x270340 |
| **0x89 has a tail:** after the items, three lists (`u8 count`, count × {`u32`, `u16` dict id}): money spent on ammo, on consumables, on repair. With nothing spent the message ends `00 00 00` | PROVEN | lobby.md §2.9, 0x26EF40 |
| 0x87: at most 3 kits and 69 items; kit slots hold **indexes** into the item table, not item ids; none is checked | PROVEN | H4 |
| Every dict id must exist in the client's `items_dict`: the lookup returns NULL and the UI dereferences it | PROVEN | H5 |
| A modification with state ≠ 0 must exist in the modifications table. At most 8 modifications and 3 upgrades per item (fatal assert) | PROVEN | H6 |
| 0x85 with state 1: the new `u16` search step must not be 0 (divide by zero at 0x22565E) | PROVEN | H7 |
| Strings: names ≤ 63, clan tag ≤ 14 or 15, status text ≤ 127, 0x82 host ≤ 63 | PROVEN | H8 |
| 0x8A / 0x95: `u32` packed size + raw LZMA2 that unpacks to exactly n × 8 / n × 0xD0 bytes | PROVEN (format) | H9 |
| 0x8E must end with a non-zero quest faction, or the client asks to choose one | LIKELY | H11 |
| 0x82 is not ordered against 0x85: a 0x85 state 1 read after the 0x82 resets the match id. Send 0x82 only after the "match found" 0x85 is acked | PROVEN (handlers), LIKELY (effect) | H14 |
| **0x9F repetition.** 0x9F is the one send-once lobby message. Play stays off until the first one (minutes ≤ 30 disables Play; initial value 0), so a lost one leaves Play off. Send it again and again (srp: every 5 s), with minutes ≥ 60. 0 minutes only shows the maintenance notice (0.21d quit the game) | PROVEN | social.md §4 |
| The client never asks for its account, money or friend lists: push 0x87, 0x89, 0x8B, `A3 07`, `A3 08` after every change | PROVEN (no sender exists) | lobby_requests.md §3, social.md §1.3 |
| `A3` with an action other than 7, 8, 9 is read as a player list. The 0.21d `u8 result` reply crashes the client | PROVEN | social.md §1.3 |
| A Play (`2F`) is answered by a 0x85 (state 1) or by `84 2F`; `83 2F` does nothing | PROVEN | lobby_requests.md §4.1 |

**Sign-in push order** (LIKELY; built from the hazards, lobby.md §3). Among
the ordered ids only:

1. `80 00 00`, alone, and wait for its ack.
2. `8F` ranked total and nickname.
3. `8B` money.
4. `96` faction mask, then `8E` reputations.
5. `8D` six service prices.
6. `94` premium.
7. `8A` prices, `95` shop.
8. `89` inventory.
9. `87` own account.
10. `8C` level and skills.
11. `92` quests.
12. `9C`, `9D` (show = 0), `9F`.
13. `85 00` client state 0.

Item descriptor (0x1F1350 / 0x1F13D0; lobby.md §2.6): `u32` item id, `u16`
dict id (0 = nothing more), `u32` condition × 10 000 or stack count; gear
only: `u32` repair time, `u32` reroll time; `u8 n1` × {`u32` modification,
`bool`}; `u8 n2` × {`u32`, `u32`, `u64`, `u8`}. Stack or gear is decided by
the client's own dictionary (`is_stack`), so the server must use the same.

## Lobby: client → server (`lobby_requests.md`)

- **Sign-in `2E`**: `u32` session, `bool` need_shop_items (6 bytes, the
  connect payload). 0x26F650.
- After 0x80 the client sends the chat subscription `4F` and `32 00 <flag>`
  (the echo of 0x80's second byte; what it asks for: GUESS).
- Booleans are `00` / `FF`. Queries are their own ids with a leading `00`
  the server must skip.

| Id | Meaning |
|---|---|
| `2F` / `30` | Play (`u32` kit id) / clan-team Play |
| `31` | leave the queue or "left the match": `u32` order, `bool` |
| `34`, `37`, `39`, `3B`, `3C`, `3D`, `3E`, `40`, `41`, `42`, `43` | queries → 0x88, 0x8C, 0x8E, 0x91, 0x92, 0x93, 0x94, 0x96, 0x97, 0x98, 0x99 |
| `45` | `u8` language → 0x9E (no leading `00`) |
| `46 00..0A` | inventory: 00 move items, 01..0A upgrades, use, drop card, decals, reroll, craft |
| `47 00..06` | shop: buy, sell, offer, repair, gold → silver, premium variants |
| `48 00` / `48 01` | spend skill points (`u8 n` × {`u16` skill, `u8` points}) / reset one branch (branch, payment) |
| `49 00..24` | kits and account (01 autobuy, 02 active kit), clans 06..23 |
| `4A 01..04` | squad: destroy, add, drop, leave |
| `4B 00..02` | quest faction, tutorial |
| `4C`..`50` | chat: text, important message, friendship, subscription, answer |

Ids the client never sends: 0x33, 0x35, 0x36, 0x38, 0x3A, 0x3F, 0x44. What
each request expects back (`83 <op>` bodies, `84` behaviour): §4.

## Chat, friends, squads (`social.md`, `login_chat.md` §5)

- Chat runs **inside the lobby connection**; there is no chat sign-in.
- `4F` subscription: 10 × `u32` channel ids + `u8` language (russian 1,
  english 0, ukrainian 2, german 3, polish 4, other 1). Slot 1 is the
  general room: 1 for a russian or ukrainian client, else 2. 0x26C320.
- `A2` text: `u8` sender type, `u32` id, name (≤ 63), **clan tag (≤ 15) only
  when the sender type is 5**, `u8` channel, body (≤ 255).
- `A4` important message: the clan tag is always there. Types 0 friend
  request, 1 quest, 2 squad invite, 3 squad accepted, 4 reputation level,
  5 drop, 6 item repaired, 7 clan invite, 10 clan-team invite, 13, 14
  rewards. `50` with an id that is not pending is a fatal assert (0x26B211).
- `A1` squad info: at most 3 members (unchecked). The second byte is the
  commander's index, now used: a soldier's Play is locked while the
  commander is ready; the commander needs ready ≥ n − 1. social.md §2.2.
- A squad mate's `88` must carry, as its first kit, the kit id `A1` gave
  for that mate. social.md §2.4.
- Clans (`9A`, `49 06..23`): layouts in social.md §3; srp refuses them.

## Results (`results.md`)

- `97` = match stats + the reader's own full result. Header: match id, an
  unused `u32`, winner team, two team scores, two `u32` team values, `u8`
  game mode; four titles each with a clan tag; up to 20 rows of 0x60 bytes.
  The full result was redesigned: 3 score components, 3 bonus blocks, rating
  before and after, quest money and reputation, play time per kit, one
  dropped item with its chances. §1.
- A score component's 8 bytes per column are **absent when the amount is
  0.0**. §1.3.
- Hazards: always send the unused list with count 0 (a 5th entry smashes
  the stack); rows ≤ 20; team 0 or 1 only; an unknown dict id in the drop
  crashes. §6.
- After the screen the client sends `3C 00`.
- Send what the match changed in the account **before** 0x97: the screen
  reads its bars from 0x8C and 0x8E when 0x97 arrives. §9.
- `93`: a full result (thrown away) + an account body: clicking a player
  shows his gear, not his statistics. §2.
- The client adds end-of-match events 11, 12 and 27 itself (`finish_match`
  0x2F4CF0); event 25 is a grenade kill; 24 and 26 are unknown. §9.

## Matches

See MATCH-SYNC-026G.md. In short (match.md §0, §2.1; all PROVEN):
- still lockstep: the client's server mode is hard-coded off (cook byte
  0x9261E64 := 0 at 0x2CBBC1);
- the world is frozen until the first snapshot (`game_world_core` +0x18FDF,
  `move` 0x2D2E40 returns at once; only `deserialize` clears it at
  0x2D0FE7);
- the client never sends its world, which is why the patch uploads it;
- tick 50 ms; event horizon `max_ping_time_in_ms` + 250 = 500 ms; 11
  history items;
- message ids: client → server 0xD8..0xE1, server → client 0xE3..0xF6
  (0xE2 does not exist: never send it);
- every time on the wire except 0xE5's is **one byte**: tick mod 256,
  relative to the receiver's clock (±127 ticks = ±6.35 s);
- inputs carry two half-floats that must be relayed byte for byte;
- the world hash is boost CRC-32 (table built at 0x1C210, polynomial
  0x04C11DB7 reflected, init and final 0xFFFFFFFF) over the serialized item
  without its leading `u32` time; `hash()` 0x325150.

### What every client does with another player's input (`match.md` §20)

PROVEN in the exe on 2026-10-12 unless tagged.
- A relayed input is not checked: the reader 0x318400 stores the rotation's
  four bytes and the mask as they came, and `base_player` 0x2D86E0 applies
  them.
- The rotation is added to the player's yaw (player+0x490, 0x2D8AD0) and
  handed to the physics world; an infinity or a NaN stays in every world.
  The server refuses such an input (live since 2026-10-12 13:32 UTC).
- **Mask bit 26 (action 0x25, LIKELY "previous weapon") is a defect that
  reaches every client.** With an item in the player's hands that is in
  neither weapon slot (7, 10), `mov ebp,[eax*4+0x927DEC]` at 0x2D8D1C runs
  with eax = 0xFF and reads 0x9281E8 = 0x2F: slot 47 of 23. The client then
  tests `[inventory+0x1C8]`, 0x40 bytes past the end of the 0x188-byte
  inventory object, and calls through it when it is not 0. Bit 25 (action
  0x24) takes slot 7 in the same case and is safe. No fix is applied;
  OPEN-ISSUES.md 26-24.
- The kit byte of a profile input is checked (0x321790).
- **A game whose first world is a Team Deathmatch in play has the tickets
  of the start in its statistics** (`tickets_count` each, and a draw),
  whatever the world had. PROVEN (match.md §21): a world is applied
  statistics first (0x2D2B18), then the rules in order (0x2D2B42);
  `timelimit_rule_core::deserialize` 0x317FD0 sends the rules event 0
  ("the match started") when the state it reads is "in process" and the one
  it had was not (0x31802B..0x31803E); the tdm rule answers with 0x3161F0,
  which writes its tickets, still the constructor's, to the statistics; its
  own `deserialize` (0x3163D0) comes after and does not. A later world is
  taken as it is (the state is "in process" already), and so is every world
  in modes 2 and 3. The next death or player leaving puts the statistics
  right (0x3161CF, 0x316616). The per-team alive counts, stale too, are
  read only right after a recount (0x316470 from 0x3161D8 and 0x31661F).
- 0xE8's handler loads the world inside the handler (`0x2D0B70(core,
  reader, 1, 8)`, 0x27C0D9 / 0x27C111), the same way for a first world and
  a later one. PROVEN.
- **A client that connects sends 0xD8 in its first datagram and again every
  500 ms until acked, and a keep-alive every 100 ms in between**
  (transport.md §4, §6); 0xFD drops its connection at once in any state
  (0x3FDEED). So a server must not answer "datagrams without a 0xD8" with
  0xFD before a second 0xD8 can have come (match.md §22). PROVEN.

## Match start, modes and maps (`static_info.md`, `modes_maps.md`)

- **0xE4** = `u8` local player index **first**, then a raw LZMA2 stream
  (no header, no property byte; dictionary 32 KiB, property 6) that decodes
  to at most 0x578A0 bytes of `match_options`. Stored (uncompressed) LZMA2
  chunks should do: LIKELY, never tried on the client. static_info.md §1.
- `match_options` (0x1F0E70): **always 20 profiles**, then `u8` map id,
  `u8` N (roster, ≤ 20), `u32` match id (also the anomaly seed), `u16`
  match level (logged only), 28 × `f32` score weights, 20 × `u8` squad ids.
  Byte layout: static_info.md §6.
- A profile: team, name (≤ 63), clan tag (≤ 14), protection `u32`, up to 3
  kits (state, id, name, 23 slots of `u16` item index + `u16` count, a
  128-bit modifier mask **high qword first**, then one `f32` per set bit of
  68), then up to 69 items. §3.1.
- **Kit state 0 = usable in the match**: a player resource is loaded only
  for state 0 (0x2CC684). §3.1.
- The 68 modifier names and their bit order (table 0x928448): §3.1. Bits
  0..19 are 0.21d's order (15 is now pocket_slots, 17 pocket_slot_size).
- Mode, map, timings and rules all come from the client's own data, picked
  by the map id (`maps_dict` → `gameplay/game_modes/*.options`). The server
  chooses only the map id and N.

| Mode | Rules | Data |
|---|---|---|
| 2 batteries | 93, 91, 88, 94 | 10 batteries |
| 3 timed batteries | 93, 91, 89, 94 | a battery every 60 ± 10 s |
| 4 Team Deathmatch (new) | 93, 91, 90, 94 | 100 tickets (150 on level_05, level_08) |

All modes: 900 s, countdown 20 s, respawn 8 s, waiting limit 5 min,
`wait_player_percent` 0.8.

- Map ids: mode 2 = 61..105 odd and 109; mode 3 = 62..106 even and 110;
  mode 4 = 117..140. 72 playable. **Never use** 11, 45, 107, 108, 114, 115,
  141..151. Table: static_info.md §4.
- **Start** (`timelimit_rule_core::logic_tick` 0x318100): the countdown
  begins when at least one player has entered and
  `popcount(entered) ≥ floor(N × 0.8)`. N = 1 or 2 starts on the first
  player. After 5 minutes of waiting the rule reports itself completed.
  modes_maps.md §10.
- **End and winner:** modes 2, 3: a team has all 10 batteries (+3000 ms),
  or time: more batteries wins, equal is a draw. Mode 4: a team has 0
  tickets and nobody alive (+3000 ms), or time: more tickets wins; a kill
  by a teammate costs no ticket. modes_maps.md §11.
- Mode 3 batteries need no patch: 0.26g serializes the next-battery time
  (+0x16C, 0x3149B0 / 0x314B90). Confirm live. client_patch.md §4.

## Game data (`data.md`)

- Same binary config format as 0.21d; srp's parser reads all of it.
- `db_static_dictionaries`: 384 items (0.21d 83), 29 categories, 23 kit
  slots, new `maps_dict` (89), `match_levels_dict` (21), bundles (14).
  Boosters, perks and skills dictionaries are gone; `item_level` moved into
  each item's config; `base_cost` is new. §2.
- Ammo's `base_cost` is **per clip** (PROVEN for the client's use; LIKELY
  for the server's price list). Sell = cost / 4. "Settled in step 4".
- Skills: `gameplay/skills/skills_tree.options`, 26 skills in two branches
  (101..113, 201..214), named modifiers, values as fractions. A row needs
  its `branch_points` spent in the rows above; exclusive pairs 111/113 and
  213/214. Skills 206 and 210 can never be bought. §3.
- `default.player`: carry limit 30 kg (0.21d 25), 3 ammo bags. §4.
- Items wear out (`degrade_per_use`, `_per_match`, `_per_death`); 158
  modifications, 20 defects; armour keeps only `armor` (no `reduce`). §4.
- Drops: four tables; the exe has no drop code, so drops are the server's
  alone. Quests: 369, type numbers renumbered. §5.
- Silver and gold are items 214 and 215.

## The client patch sites (`client_patch.md`)

`srp-026g/client-patches/patch_026g.py` checks the exe's sha256
and each site's original bytes before it writes. What each patch is for:
MATCH-SYNC-026G.md.

| Site | VA | Original → patch | Build | Tag |
|---|---|---|---|---|
| Certificate chain check | 0x12A10 | `83 EC 2C` → `31 C0 C3` | all | PROVEN live |
| Single-instance mutex | 0x124F9 | `74 09` → `90 90` | full | PROVEN |
| Default hash interval 100 → 50 ms | 0x27CC4F (the byte at 0x27CC53, file 0x26C053) | `C7 44 24 18 64 00 00 00` → `… 32 …` | full | PROVEN bytes; effect not tried live |
| Minimum video-memory check | 0x106411 (file 0xF5811) | `73 0F` → `EB 0F` | full | PROVEN bytes (`client_patch.md` §3.7) |
| Patch revision in the sign-in's version field (byte 7) | 0x380862 (file 0x36FC62), in `sign_in_on_connected` 0x3807D0 | `8A 0D 9E AE 9D 00` → `C6 44 24 27 01 90`; the relocation at 0x380864 → type 0 | full | PROVEN bytes (`client_patch.md` §6); run with a client 2026-10-12 (logged as revision 1) |
| Battery rule: an event after the completion is counted | 0x313A62 (file 0x302E62), in the container event handler 0x313A50 | `0F 85 D3 00 00 00` → six `90` | full, revision 3 | PROVEN bytes; the handler run outside the game (`client_patch.md` §3.9); in the players' download since 2026-10-13 12:22 UTC; not watched in a match to a team's last battery |
| Battery rule: the containers stay usable | 0x313B34 (file 0x302F34), same handler | `C6 81 60 01 00 00 00` → seven `90` | full, revision 3 | PROVEN bytes; the handler run outside the game (`client_patch.md` §3.9); in the players' download since 2026-10-13 12:22 UTC; not watched in a match to a team's last battery |
| A mount without a root is not announced as mounted | 0x46B0B (file 0x35F0B), in `vfs::mounter::finish` 0x469F0 | `33 C9 EB 03` → `EB 29 90 90` | full, revision 4 | PROVEN bytes; the crash made again and the built exe started (`client_patch.md` §3.10); in the players' download since 2026-10-13 15:36 UTC; not run by us past the sign-in screen |
| No journal by default: the default of `-record_journal` is "0" | 0x851CC8 (file 0x8410C8), the address in the option's initializer 0x851CC2 | `1C B5 9D 00` → `0C BF 9D 00`; the relocation stays | full, revision 4 | PROVEN bytes; the crash made again and the built exe started (`client_patch.md` §3.10); in the players' download since 2026-10-13 15:36 UTC; not run by us past the sign-in screen |
| First-sync hook, in `on_match_ready` 0x27AA20 | 0x27AEA1 | `8B 9B 48 1C 1F 00` → `E9 rel32 90` | full | PROVEN |
| Upload-request hook, 0xF6 case | 0x27A505 | `80 3D 2C D3 F1 01 00` → `E9 rel32 90 90` | full | PROVEN |
| Upload code, section build (default) | new section `.srp`, RVA 0xA415000, VA 0xA425000, file 0xC4AA00, 0x200 bytes | 8th section; the Authenticode blob moves to 0xC4AC00 | `--cave=section` | PROVEN by the self-test; whether the client minds an 8th section: LIKELY harmless |
| Upload code, fallback | over `on_hash_mismatch` 0x325A60 (0x47B bytes) | loses `-debug_hash_mismatch` | `--cave=hash-mismatch` | PROVEN (dead code once the hook is in) |
| Base relocation under the request hook | 0x27A507 | type → 0 | full | PROVEN |
| Transparent effects scroll as in 0.29 (README patch 28): `use_uv_scrolling` sets the configuration's CONFIG_USE_UV_SCROLLING bit, the bit sends `uv_scrolling_parameters0/1`; `scene_slomo` bound to 1.0 | 0x1D90DC and 0x1D94CB in `effect_fstage_default_materials::compile` 0x1D8F40, 0x14990F in `renderer_context`'s constructor | each a `jmp` into `.srp`+B00h and nops | full, revision 13 | PROVEN in the code, compile run with Unicorn (`client_patch.md` §3.26); with the retail `resources.db` no shader changes; not run in a game |

Builds: `--cert-only` (only the first row; what step 0 installed), the full
section build (**installed 2026-10-12**; the players' download had the one
with the video-memory check and the patch revision from 18:43 UTC that
day, sha256 `3c7d7bf0…9c9d`, and from 21:10 UTC that day to 12:22 UTC of
2026-10-13 the one that also holds the login address `skhid.link:1234`,
patch revision 2, sha256 `30e928dd…4883`), the full hash-mismatch build.

The two battery rule rows are patch revision 3 (`client_patch.md` §3.9;
why: `match.md` §26, the tenth battery of modes 2 and 3 cannot be put in).
Revision 3 was the players' download from 2026-10-13 12:22 to 15:36 UTC.
Made as the download is (`--server=skhid.link:1234`), the revision 3
section build is sha256 `ac947abd…6d7b`, and differs from the revision 2
exe of the download before it (`30e928dd…4883`) in those 13 bytes, the
revision byte and the PE checksum only.

The rows of `mounter::finish` and of `-record_journal` are patch revision 4
(`client_patch.md` §3.10; why: a game whose folder in Documents cannot be
made or written stopped at its start). In the players' download since
2026-10-13 15:36 UTC: made as the download is, the revision 4 section
build is sha256 `ae3ddf43…9f25` (`../client-patches/README.md` has the
downloads, row by row).

## Not proven against a client yet

The most important open points (each note ends with its own list):
- whether the push order is enough to reach the lobby;
- whether stored LZMA2 chunks decode in the client (0x8A, 0x95, 0xE4);
- what `32 00 <flag>` asks for and what 0x80's second byte means;
- the upload code running at all, and the client starting with an 8th
  section;
- the 0xDB clock field (LIKELY), the order of the rules inside a snapshot
  (LIKELY), score events 24 and 26;
- whether the original server moved late inputs (0xEF) or discarded them
  (0xF1).

## The launcher (`survarium_launcher.exe`, build 2777)

Proven by reading the exe on 2026-10-13 (`notes/launcher.md`, `notes/launcher_updater.md`), and by running the
patched one (`../client-patches/launcher/README.md`):

- Started from `game\binaries\x86` it makes no window: it has
  `survarium_updater.exe copy` copy both to `<install>\temp` and starts the
  copy. Only a path ending in `\temp\survarium_launcher.exe` gets a window
  (`run_as_copy` 0x1472A, the constructor's test at 0x14442).
- What PLAY starts is not in the exe. `file to execute`, `arguments`,
  `working folder`, `update server` and `launcher` are read from
  `HKLM\Software\Vostok Games\Survarium` through `registry_key::get_string`
  (0x14027); the game's version is read out of the file `file to execute`
  names (the mark `survarium_version_header_mark!!`, then `0.26g0`).
- PLAY runs `"<exe>" -localization=<english|russian|ukrainian|german|polish>
  <arguments>` with `CreateProcessW` and the launcher ends. The language is
  `HKCU\Software\Vostok Games\Survarium`, `language` (0 to 4), changed in
  the window without a restart.
- It has no network code. The page is an Internet Explorer control on
  `http://survarium.com/launcher/v2/survarium/?lang=<code>`, with the local
  `game\launcher\index-<code>.html` when that fails; buttons run
  `cmd.exe /c start <url>`.
- The updater is started with fourteen arguments, the second being the pipe
  `\\.\pipe\vostok_updater_pipe-Survarium`, which the launcher makes first.
  A message is a 16-bit length and that many bytes, the first its type; the
  launcher's table of 25 types has no bounds check (0, 24 and above 25 crash
  it). PLAY is enabled by updater state 14 alone.
- A 32-bit exe with "update" in its name and no manifest is taken for an
  installer by Windows, which asks for administrator rights before it starts
  (seen 2026-10-13). The original updater's manifest asks for them outright.
