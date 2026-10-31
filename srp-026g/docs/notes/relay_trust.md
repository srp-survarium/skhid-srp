# What one client's bytes can do to another: the relay trust map (0.26g)

**Version scope:** client 0.26g (build 2777, "0.26g0", no PDB), srp as on `main` of 2026-10-25.
Every address is a VA (image base 0x10000). Tags as in the sibling notes: **PROVEN** (read in
the code at the address given), **LIKELY**, **GUESS**.

The messages that carry one client's bytes to other clients, what srp checks of them, and what
the client does with bytes it did not expect. Layouts are in `transport.md`, `match.md`,
`static_info.md`, `social.md` and `lobby.md`; the audit's findings are `match.md` section 35.

Coverage words:

- **COVERED**: srp refuses or re-makes such bytes before any other client sees them (the check
  named).
- **PATCHED**: the client itself is fixed by an exe patch (the patch named).
- **OPEN**: a client can reach another client with bytes that hurt it (the risk given).
- **N/A**: the server makes the bytes itself; no client chooses them.

Fixes not on main: #282 (world checks) and #286 (patches 50, 51) are open PRs; #288 (name
characters) was closed.

## 1. Topology

```
  A's game                       srp                                  B's game
  --------                       ---                                  --------
  DC/DD input  ---------------> parse, window, dedup, rotation  ----> EB/EC relay
                                (tick and player re-written)          change_input 0x1FAB50
  DE kit pick  ---------------> parse, window, dedup  ------------->  F3 relay
                                                                      0x321790 bounds it
  E0 chunks    ---------------> reassemble, vote  ----------------->  E8 world
   (srp patch)                  (initial: first one; later: CRC)      0x2D0B70 -> 0x324D80
  4C chat      ---------------> lossy UTF-8, flood limit, re-made --> A2 line (Scaleform)
  account name ---------------> any text at first sign-in, cut ----->  names in A2/A3/A4/
                                to 63 bytes                           A1/88/E4 ...
```

Everything else another client sees is made by srp from its own state (an account, the match
clock, the roster).

## 2. The actions mask of an input (0xDC/0xDD, relayed as 0xEB/0xEC)

The reader stores the mask unchecked (0x318400); `base_player::apply_new_input` 0x2D86E0 and its
parts read the bits (PROVEN unless tagged):

- bits 12, 13 and 25: a slot from the table 0x927DF0 = {7, 10}, always in bounds;
- **bit 26 with a non-weapon in hands**: slot `[which*4 + 0x927DEC]` with `which` 0xFF reads
  0x9281E8 = 0x2F, slot 47 of 23, and calls through what is there (match.md section 20). The
  only unbounded read of the mask;
- bits 14..19 and 23: slots the inventory itself returns (0x2FE7F0, 0x2FE8B0); bits 5 and 22:
  its current index (+0x170). Bounded;
- bit 24: the suicide key (patch 34); bits 8, 21, 28: a flag byte, a null-checked use test
  (0x2D84C0), a ray query with a 3000 ms cooldown; the rest: movement and item booleans;
- **bits 29 and 30: set by no stock code and read by none found** (no `test`, `bt` or `shr` of
  them in 0x1E0000..0x340000). LIKELY harmless; a server refusal would only mark a modified
  client.

Rotations: srp relays at most one input per player per time (`inputs_seen`) and only halves
under 1024 (`PlayerInput::rotation_is_sane`), so no merge (0x2C5B60) makes one infinite.

## 3. The 0xE8 world

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
  0x3246D0, 0x3247D0); none has an allocator (match.md 35, item 1)
```

The body is a client's upload (0xE0 chunks) byte for byte; srp adds the generation, the time
and the compression. On main, `game.rs::uploaded` takes a later upload only when asked and near
the clock, and uses one that does not read as a world as it is (saved as `unreadable`). A
section over its cap, a mask bit at 20 or above, or a length past the body's end is not refused
(match.md 35, items 1 to 3).

## 4. The lobby

srp re-makes every lobby message another player sees: a client's text is parsed as lossy UTF-8
(`lobby/requests.rs`) and written again by `lobby::Writer::str` (cut to the client's buffer at
a character boundary, `lobby::cut`) or `Writer::wide`. The client's readers copy into fixed
buffers with no bound (social.md 1.2..1.4), so these limits are the check. A player's chat line
(0xA2, room chat on channel 10 too) is always sender type 5, so the `#mstats`, drop-query and
clan-system parsers never see it; `social.rs::chat` limits the rate (`CHAT_BURST`,
`CHAT_PER_SECOND`). The movies escape `<` in a body (`styledChar`), not in a name or clan tag
(`LobbyChat.as::get_styled_text`, `GameChat.as`); a clan tag `admin` is drawn in the staff
colours. Clan names and tags keep the game's characters (`clans::name_fault`: no `<`); an
account name may hold `<` (`find_or_create` takes the login's part before `@`;
`dispatch.rs::name_refusal` refuses only blanks, control characters and length).

## 5. Coverage table

```
+-------+---------------------+--------+--------------------------------+--------------------+
| id    | what                | source | srp check (file::function)     | status             |
+-------+---------------------+--------+--------------------------------+--------------------+
| DC DD | input               | client | game.rs::input: window, dedup, | rotation COVERED;  |
|  ->EB |  rotation, mask     |        |  rotation_is_sane; a late one  | bit 26 OPEN (#286);|
|  ->EC |                     |        |  relayed (default `relay`)     | other bits bounded |
| DE    | kit pick            | client | game.rs::profile_input;        | COVERED (empty     |
|  ->F3 |                     |        |  play.rs::match_profile        | kits not loaded);  |
|       |                     |        |  (rules::check_play)           | exe side OPEN #286 |
| E0+   | world chunk         | client | upload.rs, game.rs::uploaded   | caps, mask, frame, |
|  ->E8 |                     |        |  (asked, window), candidates   | lone `recent`: OPEN|
|       |                     |        |                                | (#282); contents   |
|       |                     |        |                                | OPEN               |
| E4    | static info         | server | static_info.rs::check_profile, | COVERED            |
|       |                     | (accts)|  rules::move_items, make_safe  |                    |
| A2    | chat line           | client | social.rs::chat, chat_text     | COVERED; names in  |
|       |                     |        |                                | HTML OPEN (low)    |
| A3 A4 | friends, important  | client | social.rs, Writer::str         | COVERED            |
| A1 88 | squad, squad mate   | server | Writer::str, limits            | COVERED            |
| 9A    | clan info           | server | clans::name_fault, wide limit  | COVERED            |
| 91-97 | ranking, results    | server | Writer::str, limits            | COVERED            |
| names | account names       | client | cap_name (63 bytes),           | length COVERED;    |
|       |                     |        |  name_refusal                  | markup OPEN (low)  |
+-------+---------------------+--------+--------------------------------+--------------------+
```

Every other id is **N/A**: the client's D8-DB, DF (a hash vote is one of several), E0 leave,
E1 and FA-FC go to the server only; E3, E5-E7, E9, EA, EF-F2, F4-F6 and FD are server-made; ED
and EE are never sent.

## 6. The open gaps, ranked (main, 2026-10-25)

1. **A world no game can load** (OPEN; #282 would close it). A section over its buffer is a
   fatal "mutable buffer overflow" in every game that loads it; a mask bit at 20 or above
   corrupts memory; a length past the body reads past it (match.md 35, items 1 to 3). An
   unreadable upload is used as it is, and `recent` is taken from one upload nobody
   contradicted. Reach: every game started or resynced from it.
2. **Bit 26 from a hand with no weapon** (OPEN; patch 50 in #286). Every game of the match
   calls through slot 47 (match.md section 20). The server cannot tell it from a stock input.
   A patch protects a game only when every client of the match has it.
3. **The initial world's contents are one client's word** (OPEN). The two-vote check is parked:
   stale `first_finder` bytes make two battery-mode initial worlds differ, and the candidate exe
   reset at 0x3348A0 waits for the owner. What crafted contents within the caps do was not
   traced.
4. **A kit choice with no weapon in slot 7 or 10** (COVERED on the server: `check_play` loads no
   such kit; the exe guard, patch 51, is in #286).
5. **Names and clan tags as HTML in other players' chat** (OPEN, low; no crash found): markup,
   and the staff colours for a clan tag `admin`.

Ruled out in the audit: the other action bits, a rotation merge to infinity, the kit byte (the
client bounds it), relayed ticks and players (srp writes them), the 0xE8 compression (srp's, at
most 0x8000), lobby text lengths, the `#mstats` and clan-system parsers, the 0.21d chat server.
