# What one client's bytes can do to another: the relay trust map (0.26g)

**Version scope:** client 0.26g (build 2777, "0.26g0", no PDB), srp as on `main` of 2026-10-22.
Every address is a VA (image base 0x10000). Tags as in the sibling notes: **PROVEN** (read in
the code at the address given), **LIKELY**, **GUESS**.

This note is the map of every message on the match link (both directions) and of every lobby
message that carries one player's data to other players: its bytes, where they come from, what
srp checks, what the client does with bytes it did not expect, and whether that is covered. It
does not re-derive the layouts: those are PROVEN in `transport.md` (the transport), `match.md`
(the match messages), `static_info.md` (0xE4), `login_chat.md` and `social.md` (chat, friends,
squads, clans), `lobby.md` (the lobby's own messages). The new findings of the audit of
2026-10-22 are in `match.md` section 35 and are cited here as "audit".

Coverage words:

- **COVERED**: srp refuses or re-makes such bytes before any other client sees them (the check
  named).
- **PATCHED**: the client itself is fixed by an exe patch (the patch named).
- **OPEN**: a client can reach another client with bytes that hurt it (the risk given).
- **N/A**: the server makes the bytes itself; no client chooses them.

Three branches act on what this note found:

- **PR #282** `fix/match-world-checks` (match.md section 36 there). `Game::uploaded` drops an
  upload that `snapshot.rs::Sections::fault(body, places)` faults. A fault is a world that:
  - does not walk from the front (`Sections::walk`; the fallback from the end no longer counts);
  - has a mask bit at or past the roster size;
  - has a section over `SECTION_CAPS` (bullets 0x2400, statistics 0xC00, objects 0x2000,
    physics 0x4000, rules 0x800, effects 0xC00, counted after the u16 length);
  - has a player state over `PLAYER_STATE_CAP` 0x1000.

  Such uploads are saved as `unloadable` and counted in `uploads_refused`. Also in #282:
  `Game::vouched_recent` lets `recent` start a game only when a second game reported the same
  CRC, or when only one game simulates (`usable_snapshot` and `newest_snapshot` both go through
  it). `check_candidates` forgets `recent` when no agreement comes while another game reported
  that tick. Tests: `a_world_no_game_could_load_goes_to_nobody` and
  `an_upload_another_game_reported_otherwise_is_nobody_s_to_start_from`. All 2203 kept real
  worlds pass.
- **PR #286** `exe/relayed-input-crash-guards`:
  - patch 50, at 0x2D8D19 in `process_quick_slots`: bit 26 from a hand with no weapon takes
    no slot;
  - patch 51, at 0x3217CC in `apply_profile_input` (a jump to a check at 0x844DD0): a kit
    choice is taken only when that kit's player has a weapon in slot 7 or 10.

  Checks: `check_relay_guards_026g.py` (Unicorn: all 72 combinations of hand and bit, and the
  choices 0, 1, 2, 3 and 0xFF) and `selftest_026g.py` (1857 pass). A patch protects a game only
  when every client of the match has the release. Still open: the assumption that a kit nobody
  has played yet behaves the same; it needs the in-game test.
- **PR #288** `fix/name-tag-charset` (pending merge): the characters of account names and clan
  names and tags. Its checks:
  - a rename: lobby-server `v026g/dispatch.rs::name_refusal` -> `session::v026g::names::name_char`;
  - a sign-up: `SessionStore::find_or_create` -> `names::shown_name`;
  - names already saved: `names::made_safe`, run on load from `persist::AccountRecord::repair`
    and `v026g::account::Account::repair`; it strips `< > & " ' : @` and control characters;
  - clan names and tags: `clans::name_fault`, which bans `@` and refuses staff-like tags
    (`admin`, `vg`: `STAFF_TAGS`);
  - tests: `names::tests::*`, `dispatch::tests::a_name_is_checked_by_the_clients_reasons`,
    `clans::tests::names_are_checked_by_the_games_own_characters`.

  The chat code that puts names into HTML is ActionScript, in
  `srp-026g/flash/lobby_menu/src/survarium/chat/` (`LobbyChat.as`, `Chat.as`).

Two-vote initial world (#282's first plan): parked. In the battery modes, stale `first_finder`
bytes make two games' initial worlds differ. The candidate fix is an exe reset at 0x3348A0,
which is waiting for the owner.

## 1. Topology: who sends what to whom

```
                  +-----------------+
   client A  ---->|  login server   |  TCP+TLS: sign-in, session id
             <----|  (login-server) |  (login_chat.md section 4)
                  +-----------------+
                          |
                          | session id -> account (session crate)
                          v
                  +-----------------+     chat 4C, important 4D, friends 4E, squad 4A,
   client A  ---->|  lobby server   |---> clan 49, kits 46 ...: parsed, checked, and
             <----|  UDP, v026g/    |     RE-MADE by srp before anyone else sees them
                  |  (social.rs ...)|     (A2, A3, A4, A1, 88, 9A, 93, 97, 91)
                  +-----------------+
                          |
                          | match formed: accounts -> match options (0xE4)
                          v
                  +-----------------+     inputs DC/DD -> EB/EC   (bytes passed on)
   client A  ---->|  match server   |---> kit pick DE  -> F3      (byte passed on)
             <----|  UDP, v026g/    |     world E0 chunks -> E8   (bytes passed on)
                  |  game.rs        |     everything else: made by the server
                  +-----------------+
                          |
                          v
                      client B, C ...
```

The three paths on which a client's own bytes reach another client's game are the ones with
"passed on" above. Everything else another client sees is made by srp from its own state (an
account, the match clock, the roster), so a client can only put garbage into it through that
state, which srp keeps with its own rules.

```
  A's game                       srp                                  B's game
  --------                       ---                                  --------
  DC/DD input  ---------------> parse, window, dedup, rotation  ----> EB/EC relay
                                (tick and player re-written)          change_input 0x1FAB50
  DE kit pick  ---------------> parse, window, dedup  ------------->  F3 relay
                                                                      0x321790 bounds it
  E0 chunks    ---------------> reassemble, frame, vote  ---------->  E8 world
   (srp patch)                  (initial: first one; later: CRC)      0x2D0B70 -> 0x324D80
  4C chat      ---------------> lossy UTF-8, flood limit, re-made --> A2 line (Scaleform)
  account name ---------------> any text at first sign-in, cut ----->  names in A2/A3/A4/
                                to 63 bytes                           A1/88/E4 ...
```

The 0.21d chat server (`lobby-server/src/messaging_server`, TCP, first byte 0xC3) is not on any
0.26g path: the 0.26g client carries chat inside its UDP lobby link (login_chat.md section 5.1).

## 2. The match link, client -> server

Layouts and lengths: match.md section 1.1, transport.md section 4. All C->S messages have a fixed
length (the patched client's E0 chunk excepted); `udp_transport::wire::parse` and
`match_msgs::parse_client` refuse any other length (`ParseError::Length`), an id outside
0xD8..0xE1 (`ParseError::UnknownId`), and a precise time whose ms is 50 or more
(`ParseError::Milliseconds`).

### 0xD8 connect

```
+----+--------------+-------+
| D8 | u32 session  | u8 dbg|
+----+--------------+-------+
```
- Source: the client. Goes to: the server only.
- Checks: `v026g/server.rs::Server::receive` places the address by the session the lobby
  registered (`registry.rs`); an unknown one gets no place, and after `STRAY_PATIENCE` 0xFD.
- To others: nothing. **N/A** (no other client sees it).

### 0xD9 port probe, 0xDA ready

```
+----+--------------+        +----+
| D9 | u32 session  |        | DA |
+----+--------------+        +----+
```
- Source: the client. Server only: 0xD9 makes the server send 0xE4 once (stage `Connecting`),
  0xDA starts the sync (stage `Loading`) (`game.rs::Game::receive`). Repeats change nothing.
- To others: nothing. **N/A**.

### 0xDB time reply

```
+----+--------+--------+-----------+
| DB | u16 f1 | u16 f2 | u16 cl_ms |
+----+--------+--------+-----------+
```
- Source: the client. Server only: `game.rs::time_reply` (round trip, clock lag; a sample too far
  off is not used). It steers the 0xE7 corrections of that client alone.
- To others: nothing. **N/A**.

### 0xDC / 0xDD input  (relayed as 0xEB / 0xEC)

```
0xDC:  +----+------+-----------+-----------+------------------+
       | DC | tick | half dx   | half dy   | u32 actions_mask |
       +----+------+-----------+-----------+------------------+
0xDD:  +----+------+----+-----------+-----------+------------------+
       | DD | tick | ms | half dx   | half dy   | u32 actions_mask |
       +----+------+----+-----------+-----------+------------------+
        actions_mask bit 31 = the per-input flag (node+0xD, reader 0x318400)
```
- Source: the client. Passed on: rotation and mask byte for byte; tick and player are the
  server's (`relay` in `game.rs`).
- Checks (`game.rs::input`): the time resolved against the match clock, not ahead by more than
  `TOO_NEW_MS`, not older than `INPUT_MAX_AGE_MS`; one input per player per exact time
  (`inputs_seen`), so no two relays of one player ever share a time; both halves numbers under
  1024 (`PlayerInput::rotation_is_sane`; refused ones go nowhere and the owner gets 0xF1);
  late ones moved or discarded (`place`, match.md section 16).
- What the client does with it: the reader stores it unchecked (0x318400, PROVEN); the apply
  (0x2D86E0) and its parts read the bits (audit item 1, PROVEN):
  - bits 12, 13 and 25: a slot from the table 0x927DF0 = {7, 10}, always in bounds;
  - **bit 26 with a non-weapon in hands**: slot `[which*4 + 0x927DEC]` with `which` 0xFF reads
    0x9281E8 = 0x2F, slot 47 of 23, and calls through what is there (match.md section 20);
  - bits 14..19 and 23: slots the inventory itself returns (0x2FE7F0, 0x2FE8B0), bounded;
  - bits 5 and 22: the inventory's own current index (+0x170), bounded;
  - bit 24: a 1.0 hit on the player's own "brain" (0x2D9AE0), the suicide key of patch 34;
  - bits 8, 21 and 28: a flag byte, a use test with a null check (0x2D84C0), a ray query with a
    cooldown; bits 0..4, 6, 7, 9..11, 20, 27: booleans for the movement and the item;
  - bits 29 and 30: set by no stock builder (0x241250 sets 0..23 and 25..28, patch 34 adds 24)
    and tested by no code found (LIKELY: no `test`/`shr` of them in 0x1E0000..0x340000).
  - Rotations: `yaw += dx` (0x2D8AD0), `dy` to the item in hands. Two relays merged into one node
    add their halves (0x2C5B60 -> 0x2C6640); srp never relays two for one time, and one under
    1024 stays a number (PROVEN, match.md section 20).
- Status: rotations **COVERED** (`rotation_is_sane`); bit 26 **PATCHED by PR #286** (patch 50,
  0x2D8D19; needs every client on the release); every other bit bounded in the client (no check
  needed);
  bits 29/30 harmless (LIKELY), a server refusal would only mark a modified client.

### 0xDE kit pick  (relayed as 0xF3)

```
+----+------+---------+
| DE | tick | u8 kit  |
+----+------+---------+
```
- Source: the client. Passed on: the byte; tick re-written, player the server's.
- Checks (`game.rs::profile_input`): time window, one per (time, value) (`profiles_seen`), late
  ones moved or discarded.
- Client: 0x321790 takes the value only if it is under 3 and the place has an object for that
  kit (PROVEN, match.md section 23). A kit with no weapon used to end every game at the next
  spawn (null +0xF47C, `player::respawn` 0x1F2540).
- Status: **COVERED** for the value (the client bounds it) and for empty kits (only kits that
  pass `rules::check_play` go in with state 0: `lobby-server play.rs::match_profile`);
  the exe side **PATCHED by PR #286** (patch 51, 0x3217CC -> 0x844DD0: a choice of a kit with
  no weapon in slot 7 or 10 is not taken; needs every client on the release).

### 0xDF hash report

```
+----+------+---------+
| DF | tick | u32 crc |
+----+------+---------+
```
- Source: the client. Server only: a vote for that tick (`game.rs::hash`, `majority`).
- A liar's vote is one of several: a world needs more than half of the reports, and with two
  players in the simulation both (`majority`). It can make a 1-to-1 disagreement, which resyncs
  somebody (`judge`): an annoyance, no crash. **N/A** for crashes.

### 0xE0 leave, and 0xE0 with a payload: a world chunk (srp's client patch)

```
leave:  +----+
        | E0 |
        +----+
chunk:  +----+---------------+------------+-----------+-------------------------------+
        | E0 | u32 item_time | u16 offset | u16 total | data[min(480, total-offset)]  |
        +----+---------------+------------+-----------+-------------------------------+
        total <= 0x8000; offset a multiple of 480 below total (match_msgs/upload.rs)
```
- Source: the patched client (client_patch.md sections 3.2, 3.4). Passed on: the whole body, as
  the state of 0xE8 to every client that is started from it.
- Checks today: `upload.rs::Chunk` (header, offset, total, length); `Reassembler`; then
  `game.rs::uploaded`: the first upload is the initial world (`first_snapshot`; refused when it
  decodes and has kills, deaths or events: `not_initial`); a later one only when asked (0xF6)
  and for a tick near the clock; it becomes the snapshot when its CRC is the majority's
  (`check_candidates`), and `recent` meanwhile when nothing contradicts it.
- PR #282 adds, for every upload, initial or later: `Sections::fault` (front walk, mask bits
  under the roster size, `SECTION_CAPS`, `PLAYER_STATE_CAP`), refused ones saved as
  `unloadable` and counted in `uploads_refused`; and `vouched_recent` for `recent`.
- What the client does with it: section 3 below (0xE8).
- Status:
  - frame, mask and caps (audit 3a, 3b, 3c): **COVERED by PR #282**;
  - an unvouched `recent` world (audit 3d): **COVERED by PR #282** (`vouched_recent`), except
    while one game is the only one simulating. That is **OPEN** and cannot be closed: there is
    nobody to vouch;
  - the initial world, which one client gives: frame and caps **COVERED by PR #282**; its
    contents **OPEN** (the two-vote check is parked, see the top of this note).

### 0xE1 keep-alive

```
+----+
| E1 |
+----+
```
- Server only; srp reads nothing from it. **N/A**.

### Transport messages from the client (0xFA, 0xFB, 0xFC)

```
+------+------+------+----+          s = seq, r = remote seq, a = ack byte(s)
| s    | r    | a..  | FC |          (transport.md sections 2, 3.4)
+------+------+------+----+
```
- Server only (`udp-transport`). 0xFD and 0xFE are never sent by srp to a client except 0xFD as a
  kick (transport.md section 3.4). **N/A**.

## 3. The match link, server -> client

All made by srp; the ones whose content comes from a client are 0xE4 (from accounts), 0xE8 (a
client's world), 0xEB/0xEC (a client's input) and 0xF3 (a client's kit pick). Lifetimes and the
id range: transport.md section 4; the client drops an id outside 0xE3..0xF6 and crashes on 0xE2
and 0xFE (transport.md sections 3.4, 4). srp never sends 0xE2, 0xED, 0xEE or 0xFE
(`match_msgs.rs`, match.md section 13).

### 0xE3 ports, 0xE5 set time, 0xE6 ping, 0xE7 clock offset

```
E3: +----+--------------+--------------+     E5: +----+-----+----------+--------------+
    | E3 | u16 min_port | u16 max_port |         | E5 | gen | u16 echo | u32 match_ms |
    +----+--------------+--------------+         +----+-----+----------+--------------+
E6: +----+-------+-------+                   E7: +----+---------------+
    | E6 | u16 a | u16 b |                       | E7 | i16 offset_ms |
    +----+-------+-------+                       +----+---------------+
```
- Server-made from the server's own clock and config. **N/A**.

### 0xE4 static info (match options and the 20 profiles)

```
+----+----------+------------------------------------------------------------+
| E4 | u8 local | LZMA2 raw stream -> <= 0x578A0 bytes:                      |
|    |  index   |   20 x { team, str name<=63, str clan<=14, protection,     |
|    |          |          kits[<=3] { name, slots, state ... }, items ... } |
|    |          |   then the match options                                   |
+----+----------+------------------------------------------------------------+
                                         (static_info.md section 6, byte exact)
```
- Server-made from accounts. A player chooses what is in them only through the lobby: names,
  clan tags, kit names, items, kit slots, upgrades.
- Checks: `match_msgs/static_info.rs::check_profile` refuses a team over 1, a text over its
  buffer, more kits or items than the client holds, an unknown dict id, unknown modifications,
  too many modifications or upgrades, and more (encode time); the lobby keeps the account so:
  `rules::move_items` (`Dictionary::fits` for each slot), `items_rules::make_safe` (unknown
  modifications, ranks, upgrades out of their slots), `rules::check_play` for state 0
  (match.md section 23).
- Status: **COVERED**.

### 0xE8 world

```
+----+-----+------------+---------------------------------------------------------------+
| E8 | gen | compressed | state: u32 time, then the body (raw, or LZMA2 into 0x8000)    |
+----+-----+------------+---------------------------------------------------------------+

body (game_state_history_item::serialize 0x324BE0, read by 0x324D80):
+------------------+------------------------------------+
| u32 active mask  | per set bit, ascending: a player   |   player block (0x33FEE0):
|                  | block, no length of its own        |   +---------+-----------+----+-------+
+------------------+------------------------------------+   | u16 len | state[len]| u8 | anim..|
| u16 len | bullets       cap 0x2400                    |   +---------+-----------+----+-------+
| u16 len | statistics    cap 0x0C00                    |    state cap 0x1000; anim read with
| u16 len | game objects  cap 0x2000                    |    a 0x7000 limit (0x395D00)
| u16 len | physics world cap 0x4000                    |
| u16 len | game rules    cap 0x0800                    |
| u16 len | effect players cap 0x0C00                   |
+---------+---------------------------------------------+
  caps: the fixed buffers of the history item (constructor 0x324510 -> 0x324523..0x324680,
  0x3246D0, 0x3247D0); none has an allocator (audit item 3a)
```
- Source: a client's upload (0xE0 chunks), byte for byte. srp adds the generation, the time and
  the compression (`game.rs::send_world`; it compresses only a body that fits 0x8000).
- Checks: section 2, 0xE0.
- What the client does (audit item 3, PROVEN unless tagged):
  - 0x2D0B70 -> 0x324D80 reads the mask and **puts the block of bit k into player slot k with
    no bound** (0x324DC0..0x324E15: `slot = item + k * 0x9054`); the item has 20 slots
    (0x324510: 20 x 0x9054, ending at item+0xB4690). A bit 20..31 makes 0x33FEE0 write and
    release references (`[slot+0x2050]`) in the section buffers that follow or past the item:
    memory corruption (crash LIKELY).
  - Each section is copied whole into its buffer (`memcpy` 0x424580 after a resize 0x7AB30).
    **A length over the buffer's capacity goes to `mutable_buffer::reallocate` (0x7AB70), which
    with no allocator raises the fatal assertion "mutable buffer overflow" (0x9E3398), then calls
    through a null pointer**: the game dies. The same holds for a player state over 0x1000
    (0x33FD80 -> 0x7AB30).
  - The reader of the body does not test its end (0x324D95.., 0x318400: `mov eax,[reader+4]`,
    read, advance): a length past the end reads past the buffer.
  - What each section's own reader does with crafted contents (objects in list order with no
    length or id, client_patch.md section 3.14; physics; bullets) was not traced.
- Status: frame, mask bits and buffer caps **COVERED by PR #282** (`Sections::fault`); crafted
  contents within them **OPEN** where no second game vouches (section 6, gaps 1 and 2).

### 0xE9 player entered, 0xEA player left

```
+----+------+---------+
| E9 | tick | u8 place|        (0xEA the same)
+----+------+---------+
```
- Server-made from the roster (`game.rs::roster_change`). **N/A**.

### 0xEB / 0xEC input relay

```
0xEB:  +----+------+--------+--------+--------+------------------+
       | EB | tick | player | half dx| half dy| u32 actions_mask |
       +----+------+--------+--------+--------+------------------+
0xEC:  +----+------+----+--------+--------+--------+------------------+
       | EC | tick | ms | player | half dx| half dy| u32 actions_mask |
       +----+------+----+--------+--------+--------+------------------+
```
- tick, ms and player: the server's. dx, dy, mask: the sender's 0xDC/0xDD. Status: as 0xDC.

### 0xED / 0xEE batched relay

- Never sent by srp (match.md section 13). **N/A**.

### 0xEF / 0xF0 move own input, 0xF1 / 0xF2 discard own input, 0xF4 / 0xF5 the same for a kit pick

```
EF: | EF | from tick | to tick |          F0: | F0 | tick | ms | tick | ms |
F1: | F1 | tick |                          F2: | F2 | tick | ms |
F4: | F4 | from tick | to tick |          F5: | F5 | tick |
```
- Server-made, to the owner only, about the owner's own input (`game.rs::place`, `answer_owner`).
  **N/A**.

### 0xF3 kit pick relay

```
+----+------+--------+---------+
| F3 | tick | player | u8 kit  |
+----+------+--------+---------+
```
- tick and player: the server's; the kit byte: the sender's 0xDE. Status: as 0xDE.

### 0xF6 upload request

```
+----+------+
| F6 | u8   |    the patched client uploads its event-horizon world (upload.rs)
+----+------+
```
- Server-made. A stock client skips it (match.md section 1.2). **N/A**.

### Transport messages to the client (0xFA, 0xFB, 0xFC, 0xFD; parts)

- Server-made (`udp-transport`). A split message's parts each hold a packet of the client's
  pool until the message is whole or 128 newer unique ids have come; an empty pool is a null
  write (transport.md section 5, match.md section 27): srp paces its own parts. **N/A**.

## 4. The lobby: what carries one player's data to another

All of these are re-made by srp: a client's text is parsed (`lobby/requests.rs`: `u8 len` +
bytes, read as lossy UTF-8; wide strings as lossy UTF-16) and written again by
`lobby::Writer::str(text, limit)` (cut to the buffer at a character boundary, `lobby::cut`) or
`Writer::wide(text, units)`. The client's readers copy without a bound into fixed stack and heap
buffers (social.md sections 1.2..1.4: a name over 63 runs into the tag, a tag over 15 over the
saved type and channel), so the limits are the check.

### 0xA2 chat line  (from 0x4C)

```
4C (A -> srp): | 4C | u32 channel_id | str receiver | u8 channel | str body |
A2 (srp -> B): | A2 | u8 sender_type | u32 id | str name<=63 | [str tag<=15 if type 5] |
               | u8 channel | str body<=255 |
```
- Checks (`lobby-server v026g/social.rs::chat`): flood allowance (`CHAT_PER_SECOND`,
  `CHAT_BURST`); recipients by channel and subscription; ignore lists; sender_type is always 5
  (account) for a player's line, so the `#mstats`, drop-query and clan-system parsers (sender
  types 8 and 9, social.md section 1.2) never see a player's text; `lobby::chat_text` cuts name,
  tag and body to 63 / 15 / 255.
- Client: the body goes to Scaleform with `<` turned into `&lt;` (`styledChar`, lobby and HUD
  `Chat.as`); the **name and the clan tag go in as HTML unescaped** (`styled(clanPrefix +
  linked(name ...))`, `LobbyChat.as::get_styled_text`, `GameChat.as`). A tag starting with
  `[admin]` or `[VG]` is drawn in the staff colours (`LobbyChat.as`).
- Status: body and lengths **COVERED**; names and tags in HTML **COVERED by #288 (pending
  merge)**: no name or tag can hold `< > & " ' : @` or a control character, and no tag can
  be staff-like (the checks are at the top of this note and under "Account names").

### 0xA3 friend lists, search results; 0xA4 important message (from 0x4D, 0x4E)

```
A3 07: | A3 | 07 | u16 n | n x { u32 id | str name<=63 | str tag<=15 | u8 status } |
A3 08: | A3 | 08 | u16 n | n x { u32 id | str name<=63 } |
A4:    | A4 | u8 type | u32 id | str name<=63 | str tag<=15 | u8 kind | str text<=255 |
       | u32 message_id |
```
- Checks: `social.rs` (friendship, important, clan_important) and the `lobby::Writer::str`
  limits; the lists are built from the store, not from what a client sent.
  Kinds a player may send: friend request, squad invite and answer, clan and clan-team
  invites (permitted by `clans`); the rest are srp's own (quests, drops, rewards).
- Status: **COVERED** (lengths, kinds). Names and tags as 0xA2.

### 0xA1 squad info, 0x88 a squad mate's account, 0x9A clan info, 0x91 ranking, 0x93 / 0x97 results

- Server-made from accounts (social.md sections 2.1, 2.4, 3.2; results.md). Player-chosen parts:
  names (63), clan names and tags (`clans::name_fault`: letters, digits, a few symbols, no `<`),
  the clan's daily message (wide, `WIDE_TEXT` 245 units), kit contents (as 0xE4).
- Status: **COVERED** by the limits and the clan name rule (with #288, pending merge, it also bans
  `@` and staff-like tags through `STAFF_TAGS`); account names as 0xA2.

### Account names (every message above, and 0xE4)

- A name is made at the first sign-in from the login's part before `@`, any characters, cut to
  63 bytes (`session lib.rs::find_or_create`, `account::cap_name`); a rename (`49 03`/`49 04`,
  `names.rs`) is checked by the lobby.
- Status: lengths **COVERED**; characters **COVERED by #288 (pending merge)**:
  - a rename: lobby-server `v026g/dispatch.rs::name_refusal` -> `session::v026g::names::name_char`;
  - a sign-up: `SessionStore::find_or_create` -> `names::shown_name`;
  - names already saved: `names::made_safe` on load (`persist::AccountRecord::repair`,
    `v026g::account::Account::repair`), stripping `< > & " ' : @` and control characters;
  - clan names and tags: `clans::name_fault` (no `@`; `STAFF_TAGS` refuses `admin`, `vg`);
  - tests: `names::tests::*`, `dispatch::tests::a_name_is_checked_by_the_clients_reasons`,
    `clans::tests::names_are_checked_by_the_games_own_characters`.

  The movies these names reach are ActionScript in `srp-026g/flash/lobby_menu/src/survarium/chat/`
  and `srp-026g/flash/hud/src/survarium/chat/GameChat.as`; they still do not escape a name
  or tag.

## 5. Coverage table

```
+-------+---------------------+--------+--------------------------------+--------------------+
| id    | what                | source | srp check (file::function)     | status             |
+-------+---------------------+--------+--------------------------------+--------------------+
| D8    | connect             | client | server.rs::receive, registry   | N/A (server only)  |
| D9 DA | port probe, ready   | client | game.rs::receive (stage)       | N/A (server only)  |
| DB    | time reply          | client | game.rs::time_reply            | N/A (own clock)    |
| DC DD | input               | client | game.rs::input                 | rotation COVERED;  |
|  ->EB |  rotation, mask     |        |  window, dedup, rotation_is_.. | bit 26 PATCHED #286|
|  ->EC |                     |        |                                | other bits bounded |
| DE    | kit pick            | client | game.rs::profile_input;        | COVERED; exe side  |
|  ->F3 |                     |        |  play.rs::match_profile        | PATCHED #286       |
| DF    | hash vote           | client | game.rs::hash, majority        | N/A (a vote)       |
| E0    | leave               | client | game.rs::receive (give_up)     | N/A (server only)  |
| E0+   | world chunk         | client | upload.rs, game.rs::uploaded,  | frame, mask, caps, |
|  ->E8 |                     |        |  Sections::fault, candidates,  | vouched: COVERED   |
|       |                     |        |  vouched_recent (#282)         | #282; contents of a|
|       |                     |        |                                | lone world OPEN    |
| E1    | keep-alive          | client | -                              | N/A                |
| FA-FC | transport           | client | udp-transport                  | N/A                |
| E3 E5 | ports, time         | server | -                              | N/A                |
| E6 E7 | ping, offset        | server | -                              | N/A                |
| E4    | static info         | server | static_info.rs::check_profile, | COVERED            |
|       |                     | (accts)|  rules::move_items, make_safe  |                    |
| E8    | world               | client | see E0+                        | see E0+            |
| E9 EA | roster change       | server | game.rs::roster_change         | N/A                |
| EB EC | input relay         | client | see DC DD                      | see DC DD          |
| ED EE | batched relay       | -      | never sent                     | N/A                |
| EF-F2 | move/discard own    | server | game.rs::place, answer_owner   | N/A                |
| F3    | kit relay           | client | see DE                         | see DE             |
| F4 F5 | move/discard kit    | server | game.rs::profile_input         | N/A                |
| F6    | upload request      | server | -                              | N/A                |
| FD    | kick                | server | udp-transport                  | N/A                |
| A2    | chat line           | client | social.rs::chat, chat_text     | COVERED; names     |
|       |                     |        |  + #288 names::name_char       | in HTML COVERED#288|
| A3 A4 | friends, important  | client | social.rs, Writer::str         | COVERED            |
| A1 88 | squad, squad mate   | server | Writer::str, limits            | COVERED            |
| 9A    | clan info           | server | clans::name_fault, wide limit  | COVERED            |
| 91-97 | ranking, results    | server | Writer::str, limits            | COVERED            |
| names | account names       | client | cap_name (63 bytes); #288:     | length COVERED;    |
|       |                     |        |  shown_name, made_safe         | markup COVERED#288 |
+-------+---------------------+--------+--------------------------------+--------------------+
```

## 6. The open gaps, ranked

Re-ranked on 2026-10-22, with PR #282, PR #286 and PR #288 in.

1. **The initial world comes from one client, and its contents are its word** (OPEN). PR #282
   checks its frame, mask and caps (`Sections::fault`). What its opaque sections hold (objects,
   physics, bullets, player states past their head) is checked by nothing, and every game of
   the match starts from it. Reach: all games. The planned fix, two games' initial worlds with
   the same CRC, is parked: in the battery modes, stale `first_finder` bytes make two honest
   initial worlds differ. The candidate fix is an exe reset at 0x3348A0, which waits for the
   owner. Whether crafted contents crash a game was not traced (match.md section 35, 3d).
2. **A world from the only game that simulates starts another game unvouched** (OPEN,
   inherent). `vouched_recent` (PR #282) takes `recent` only when a second game reported the
   same CRC, or when one game is all that simulates. In the second case nobody can vouch, and
   the structural checks are all such a world gets. Reach: the game started from it, not all.
   It cannot be closed on the server.
3. **Patches 50 and 51 protect only the games that have them** (PATCHED by PR #286). The bit-26
   slot (match.md section 20) and the kit-choice guard against the null +0xF47C respawn
   (section 23) work only when every client of the match is on that release. srp's own kit
   rule (`check_play`, since 2026-10-12) covers the respawn for any client meanwhile; bit 26 has
   no server cover (the server cannot tell it from a stock input). Still open in #286: the
   assumption about kits nobody has played yet, which needs the in-game test.
Closed since the audit:
- **Names and clan tags as HTML in other players' chat** (low; no crash found): COVERED by #288
  (pending merge). `names::name_char`, `names::shown_name`, `names::made_safe` and
  `clans::name_fault` with `STAFF_TAGS` (section 4, "Account names"). The movies
  (`srp-026g/flash/lobby_menu/src/survarium/chat/`, `GameChat.as`) still put names in unescaped,
  so the server's character rule is the whole check.
- **A section over its buffer** (audit 3a: a fatal "mutable buffer overflow" in every game that
  loads it): COVERED by PR #282 (`SECTION_CAPS`, `PLAYER_STATE_CAP`). The scripted test
  client's ballast now fills the bullets to under their cap and the rest into the physics
  (`scripted.rs`).
- **A mask bit at or past the roster** (audit 3b: writes past the 20 player slots): COVERED by
  PR #282 (`Sections::fault`).
- **A frame that does not walk** (audit 3c: the reader has no end): COVERED by PR #282 (the front
  walk alone; the fallback from the end no longer counts).
- **An unvouched `recent` world while another game reported otherwise** (audit 3d): COVERED by
  PR #282 (`vouched_recent`; `check_candidates` forgets it).
- **Bit 26 from a hand with no weapon**: PATCHED by PR #286 (patch 50, 0x2D8D19).
- **A kit choice with no weapon in slot 7 or 10**: PATCHED by PR #286 (patch 51, 0x3217CC ->
  0x844DD0), beside srp's own kit rule.

Ruled out in the audit (each in `match.md` section 35): the other action bits (all bounded in
the client); a rotation merge to infinity (one relay per player per time); the kit byte (the
client bounds it); relayed ticks and players (the server writes them); the 0xE8 compression
(server-made, at most 0x8000); lobby text lengths (cut to the buffers); the `#mstats`, drop-query
and clan-system parsers (never a player's line); the 0.21d chat server (not a 0.26g path).

## 7. Sequences

### 7.1 Handshake and first sync (match.md sections 2.2, 18, 27)

```
  client                                server
    |  D8 {session, dbg}  ----------------> |  placed by session
    |  <---------------- E3 {min, max}      |
    |  D9 {session} (each port) ----------> |
    |  <---------------- E4 static info     |  built from accounts, check_profile
    |  ... loads the level ...              |
    |  DA ready  -------------------------> |  generation := 0xFF on the client
    |  E0 chunks (initial world, patched) ->|  first upload = initial world
    |  <---------------- E5 {gen 0, time}   |  again until answered
    |  DB  -------------------------------> |
    |  <---------------- EB/EC/F3/E9 ...    |  the replay since the world, kept until acked
    |  <---------------- E8 {gen 0, world}  |  only after the replay is acked
    |  <---------------- E9 {tick, own}     |  it enters 300 ms ahead
    |  DC/DD/DE/DF ...  ------------------> |  play
```

### 7.2 An input, in time, late, moved or discarded (match.md sections 5, 16)

```
  A                       server                              B, C
  | DC {t, dx, dy, m} -->|                                     |
  |                      | window, dedup, rotation             |
  |                      |-- rotation bad: --> F1 {t} to A     |  (nothing to B, C)
  |                      |-- in time: ------------------------>| EB {t, A, dx, dy, m}
  |                      |-- late, move (SRP_LATE_INPUTS=move):|
  |  <-------- EF {t, t2}|------------------------------------>| EB {t2, A, ...}
  |                      |-- late, discard (default):          |
  |  <-------- F1 {t}    |                                     |  (nothing)
```

### 7.3 A world: upload, vote, 0xE8 (match.md sections 13..15; PR #282)

```
  uploader U            server                        others        joiner / resynced J
  | <---- F6 (asked)    |                               |                |
  | E0 chunks ------->  | reassemble (upload.rs)        |                |
  |                     | Sections::fault    [PR #282]  |                |
  |                     |  (walk, mask, caps; refused:  |                |
  |                     |   unloadable)                 |                |
  |                     | CRC -> hashes[t][U]           |                |
  |                     | <------------------- DF {t, crc} (each)        |
  |                     | majority(t) == crc ?          |                |
  |                     |   yes: snapshot := world      |                |
  |                     |   no : dropped, recent cleared|                |
  |                     |   none yet: recent := world   |                |
  |                     | vouched_recent: a 2nd CRC, or |   [gap 2]      |
  |                     |  U is the only one simulating |                |
  |                     |------------- E5, replay, E8 {gen, world} ----->|
```

### 7.4 A resync (match.md sections 18, 30)

```
  J (out of step)       server                         others
  | DF {t, crc_J} ----> | majority(t) != crc_J          |
  |                     | judge: J is wrong             |
  |                     | fresh snapshot (<= 700 ms)?   |
  |                     |   no: F6 to a source -------->| E0 chunks back
  | <--- E5 {gen+1} --- |                               |
  | DB --------------->  |                               |
  | <--- replay ------- | the others' inputs since the world, kept until acked
  | <--- E8 {gen+1} --- | once the replay is acked
  | DF {t2, crc} ----->  | agrees: done (restore count cleared)
```
