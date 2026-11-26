# Protocol audit

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Every message was compared with the client's own code: each server->client encoder with the client's reader AND the
consumer of each stored field (some field-order errors are only visible at the consumer, see the `B1` notice in
social.md), each client->server decode with the client's builder and the Flash call that feeds it (what each value MEANS).
Our decoders mirror our encoders, so round-trip tests alone cannot catch such errors; the rules below come from the exe.

Coverage: 83..9F 146 rows, A0..BF + login + match 62 rows, client requests 93 rows; the UNVERIFIED rows are in "Open".

## Rules checked against the exe

| Message | Rule (proof) |
|---|---|
| 97 header | u8 team scores at +0x1196 / +0x1197 (0x20013F177 'team1_score'), u32 params 0, the byte after the count = match level (0x20013F9C8 'match_level') |
| 97 own block | quest triple in the order money, rep, exp (0x200182515.. -> quests_money / quests_rep / quests_exp) |
| 97 / 99 rows | the score is a signed i16 (0x200141457) |
| 99 header | 0, success, found, total, n (0x200180D67..0x200180DB7) |
| 93 | 3 awards + 11 x 9 score components after the id (0x200182387 -> 0x200181C20) |
| 85 state 1 | place = 1-based queue position, total = waiting (0x2001C16F0) |
| 85 state 2 | `02 u32 order, u32 match id, u8 side`; local matches `-1, -1, 3` (0x200186308) |
| 88 | the trailing u32 is the profile VERSION (rebuild test 0x200277E80) |
| 87 / 88 / 93 bodies | at most 189 items (worn first), the encoder refuses more (fixed area, 0x2001256E3) |
| 9A members | 2nd byte = the member's LEVEL (0x20018C033 -> 0x20027FD88 'level'); default_rank comes from the rank table |
| 94 calendar / 95 tab name | at most 64 days (assert) / 0x3F bytes (NUL over the tab id) |
| login 0C 02/03 | a permanent ban is sent as 71 582 787 minutes at most; the client adds +1 min itself (0x2001298F1) |
| DF confirmation | b = the match clock when the CD arrived (servo 0x2001AF50D); b = a lets the client clock drift until DFs are dropped |
| options +0x107A | max remote delay 500 ms (<= 15 ticks; CHOICE value); 0 online shows the high-ping icon always and turns lag compensation off |
| player record +0x2D8 / +0x2F6 / +0x2EF | games played (1 once the first match is claimed) / face / premium |
| item records in options | slot name <= 31 bytes, <= 8 upgrades, no dict-0 items |
| 48 0F | clan window opened: no change, state pushed |
| 48 0E | role decides: no clan -> cancel reservation, commander -> disband, member -> leave (CHOICE: same bytes) |
| 48 14 | the confirmed kick |
| 48 13 / 48 12 01 | 48 13 to the commander asks (`83 48 13 01`), 48 12 member 01 = the confirmed hand-over |
| 48 0D u32 | the deposit amount (see the registration form below) |
| 62 21 | signed deltas, gold first (0x200282980); no increase once the room's tokens are complete |
| s2c class table | the client's table |

## More findings from the exe

* Partial clan deposits: the field is the 9A sub 0 REGISTRATION form (the byte after the clan tag != 1): `u16 own index,
  u8 commander, u8 P, u8 Q, u32 collected silver, u8 S` after the member list; P / Q / collected only grow until a
  `NoClan` 9A; `83 48 0D` / `83 48 08` have no reader. The server sends the registration form after 48 07 and at sign-in
  and keeps a running total of partial deposits. 8D slot 4 = the silver registration cost (the deposit target).
* `48 0C u64`: "invitation sent" request; answer `83 48 0C u64, str128 name` -> "Player <name> successfully invited to
  clan."
* `83 48 07` flag: one of the three inputs of the "Register" button (`set_clan_creation_enabled`), shown with the next
  registration form (we send it).
* `83 46 07` tile order: tile i = AA page entry i, 0-based; the client SKIPS entries whose dict it lacks (later tiles
  shift): pages only hold dictionary ids.
* `8E` record +4: no reader. `9E` id: the 57 bit and the 56 entries use the 9E id byte (ours = list index; keep < 32).
* `97` R +0x14 = the match's total silver: the in-match victory splash shows `silverReward = +0x14 - sum of the three tail
  lists` (0x2001ACCFC). +0x18 has no reader; +0x2E = per-profile match level for `match_levels_factors` (undrawn values
  only; left 0).
* `98` min / max players: PER TEAM, the allowed characters of each team's ONE-digit field (`restrict "min-max"`). The
  server sends min 1, max = half the match size clamped to 1..9 (the same cap for bots and for 62 PlayerCount / room
  creation).
* `5A 03` Refresh (delta 0): never gated by `can_change_drop_level` in the client; only +-1 check it.
* `94` autobuy: cannot show with 0.69d data (only service records read the mask and they resolve to 1088 only).
* `AE`: profile id = the mate's 88 profile id, version = cache key, items level -> PvE equipment level; state bits 0..5
  read, 6..7 no reader. `B9`: popup i gets `current_state = i` for i in [lo, hi) (only the name is drawn). `AC` = minutes
  to a server maintenance (< 60 warning, < 30 Play blocked; 60 = none announced).
* `62 19`: no caller in the exe or any of the 37 movies (unreachable). `45 02` two-argument form: dead code. `49 05` 2nd
  byte: 0xFF only from the lock-slot button (bits 4 / 5), 0 otherwise (our bit checks cover it).
* `46 00` last byte: 1 = buy a faction-locked silver item now for gold at the 8A `c` price (`st_force_unlock_for_gold`);
  our 8A has `c = 0`, so the client never sends 1. Offering it needs a gold price per item (no source gives one). The
  locked tile still opens the dialog at the silver price; the server refuses it with an `84 46` text (items.md, "Buy a
  locked faction item").
* `46 02` also buys BUNDLES (95 elements with +9 == 1, same id space as premium offers); we send none.

## Open

Member status values of the registration form beyond 0 = ready; the original required supporter count (text: at least 5;
we require 0, founder alone); refunds to a leaving supporter (texts: lost); the original 48 0C side effects; a gold unlock
price; whether the 0.69d faction shop shows bundle cards and the bundle purchase answer; R +0x18; AE bits 6 / 7; B9 lo / hi
convention (invisible); `46 05` (Steam, not ported).
