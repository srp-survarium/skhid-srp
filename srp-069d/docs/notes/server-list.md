# Game-server list: `9E`, `57`, `56` (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Labels: PROVEN = read from the exe (address given), LIKELY = inferred, CHOICE = our decision, OPEN = unknown. Nothing here was run
in the real client yet.
Code: `servers/lobby-server/src/servers.rs`, `crates/protocol/src/c2s.rs` (`Retry56`, `ServerList`), setting `[[game_servers]]` of the config file.
Test: `servers/lobby-server/tests/gaps.rs`.

## 1. What the client does (PROVEN)

* `9E` (S->C, reader 0x200189A50) fills the client's server vector (inline capacity 8; a ninth entry is an assert + `int 3`,
  so the server never sends more than 8). Layout: three `str(max 0x400)` (the first two are opened as web pages by the
  handlers 0x1E8CA0 / 0x1E8D00, the third is the base of the `BA` client-auth URL), `u8 n`, then n entries of
  `str(max 0x40) host`, `str(max 0x40) label`, `u16 x`, `u16 y`, `u8 id`. The entry offsets are +0x00 host, +0x40 label,
  +0x84 x, +0x86 y, +0x88 id; +0x80 (ping) is set to 0, +0x82 (`wait_count`) is never written by the reader.
  * With `n = 0` the client's saved server mask (client global 0xAA2134) is replaced by all ones (0x189B1A); with
    `n > 0` the saved mask is kept.
  * The Flash call `root.set_game_servers_startup_info` gets, per entry, `{id, label, x, y}` (0x189DA1..0x189F4A): `x` / `y`
    are the position of the server's marker on the server window's map, `label` is the text shown (not a localisation key).
  * `root.update_game_server_data {id, ping, selected, wait_count}` (0x1C9DF0) refreshes one row from the entry's ping,
    the mask bit of the id and `wait_count`.
* Ping (PROVEN structure): the lobby tick (0x174AA0) calls 0x195460, which starts a pinger (0x2005C1650) on the first entry
  with the entry's host string; its completion callback (0x195150) stores the measured value (u16 ms) in entry +0x80 and starts
  the next entry; after the last one it sends `56`.
* `56` (C->S, builder 0x195150 at 0x19528C): `u8 n`, n x {`u8 id` (entry +0x88), `u16 ping ms` (entry +0x80)}. No reply exists.
* `57` (C->S, builder 0x195560): `u32 mask` (global 0xAA2134: bit `id` = the server the player ticked), `u32 flags`
  (global 0xAA2120, 0x10 when its bit 4 is set). Sent by `server_list_updated` (0x1E9FF0), which writes the new mask into the client's
  settings (`user.cfg` key) when the player ticks a server.

## 2. What the server does

* Setting (CHOICE: the data has no server list): `[[game_servers]]` entries `host`, `label`, `x`, `y` (host "" = the match host; up to 8
  entries, host and label at most 64 bytes). Unset = ONE entry for this server: label "Skhid" (a server name, not a text key),
  host = the match server's public host (`network.match_public`), else the lobby's public address, marker at (238, 47)
  (`servers::GameServer::default_list`). An empty list left both server selectors empty: the PvP search's server choice and the
  custom match window's region drop-down are both filled from this list (`LobbyMenu.set_game_servers_startup_info` ->
  `ServerSelect.setServersList` and `CustomMatchWindow.setServersList`, PROVEN; the PvP selector is enabled only with
  `length > 0`). The list is read once at start.
* Marker position (CHOICE): the selector's `worldMap` clip is 310 x 162 px and Flash places a marker at `x * 0.76`, `y * 0.76`
  (`ServerSelect.setServersList`; SrvServerSelector in the lobby movie): (238, 47) is about 30 E, 50 N on a plate carree map.
* The selection: the client's mask is a `user.cfg` variable whose default is all ones (-1); the `80` sign-in answer's last `u32`
  replaces it only while it is still the default (0 there means -1, PROVEN 0x20018594C..0x200185965; the server sends 0). So a
  fresh client has the one server ticked; a `57` stores the player's choice on the peer. A custom room stores the chosen entry id
  as its `region` (`62 1E`, refused when it names no entry).
* `9E` at sign-in carries the list (ids = positions). Empty list = four zero bytes.
* `57`: the mask is stored on the peer (`Peer::server_mask`, bits outside the list dropped) and the list is sent again
  (LIKELY; the client never waits for it, and `9E` with `n > 0` keeps the saved mask,
  so the re-send is harmless). With an empty list nothing is sent.
* `56`: the results are stored (`Peer::server_pings`); no reply.
* What the mask would select: [`servers::chosen`] returns the listed servers a mask selects (all when it selects none). Today
  there is one match server, so nothing consumes it; when several match servers exist the queue picks among the chosen ones
  by the stored pings (OPEN: no 0.69d evidence for the placement rule).

## 3. Open

* The two web-page strings and the `BA` base are sent empty (no web site; `52` links are unanswered for the same reason).
* `wait_count` is written only by `AC` (PROVEN, handler 0x20018AEC0): `u32 value, u32 mode_mask, u8 n, n x {u8 server
  id, u16 wait_count}`; the row with that id gets +0x82 (0x20018B088), then 0x1C9DF0 refreshes the rows. It shows only in
  the server tick-box tooltip `ping: N wait: M` (lobby movie `ServerCheckBox`), a plain number. The server sends n = 0
  (no queue per game server). The count byte is always sent (without it the client reads the byte after the message).
* `AC` `mode_mask` -> global 0x200AA2124: the PvP mode list (0x200177ED0) shows mode i as playable only with bit i set,
  else `st_locked_by_server` "The mode is unavailable". Sent (login.rs `play_gate`, online matches on): every check-box mode
  (bits 0..6) with a row in the unrated queue pool that the match server serves (the world host's config projects); never
  bits 7..9 (online.md 7.3); with the test-only override `online_queue` only its mode. Local bots matches do not read it (they play with 0).
* Web strings (LIKELY for string 2): string 1 is the store link (`go_to_store`, 0x2001E8CA0), string 3 the base of the
  `/client-auth/%I64u/%s` link, string 2 LIKELY the knowledge base; a survarium.com / skygate.info link makes the client
  send `52`.
* The real server list of the original (names, map positions) is not in any file: all entries are the operator's.
