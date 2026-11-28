# Online matches (0.69d): lobby handoff, initial world, end and results, wiring

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

PROVEN = read from the exe (address) or AS3; LIKELY = shape / analogy; CHOICE = a server rule no client evidence decides;
UNVERIFIED = not checked in a run. Addresses are 0.69d exe VAs (image base 0x200000000). Companion notes: `netcode.md` (DF / DE / hashes), `social.md` (rooms), `progression.md` (queue).

## 0. State

| Item | State |
|---|---|
| 1 lobby -> match handoff | `82` layout PROVEN and implemented (`protocol::s2c::ConnectToMatch`); online `match_options` builder (`local_match::online_options`) |
| 2 initial world | the world host builds every start world (section 2); the `.srpde` world file (`srpde-pack`) holds one for tests |
| 3 end and results | winner and statistics from the world host, else the winner from the clients' reports (`online::decide_winner`) (section 3) |
| 4a queue mode mask | `2F` mask = one bit per mode id, mapped to the data's rows; map / mode / weather picked per group (section 7) |
| 4b boosters online | charged at match creation, in the options' modifier block (section 8) |
| 4c bots online | driven by the world host: queue bot fill and room bots (section 10) |
| 4d results `97` | always sent to a paid player: names, teams, winner, rewards, the world host's numbers (section 9) |
| 4 wiring | queue group, custom room Start, squad play (through the queue) -> match server -> `82` -> reports -> rewards: `servers/lobby-server/src/online.rs`, `servers/srp/src/online.rs`. Always on (srp always runs the match server; the world host builds every start world) |
| 5 game-run checks | section 6 |

## 1. Lobby -> match handoff

### 1.1 What the exe does (PROVEN)

1. The player presses Play with the PvP queue: C->S `2F` (see `progression.md` 3). The server answers `83 2F` and `85 01 order ..`
   (searching). Nothing else is needed from the lobby until the match exists.
2. S->C **`82` connect_to_game_server** (unique class, `udp-transport` table 0x80..0x82), handler `lobby_client::on_message_received`
   case 0x82 at **0x20018E0FA** (log string 0x2009636F8 `connect_to_game_server: %s:%d match_id:%d`):
   ```
   str  host     (u8 len, FF u16 len; 0x20011DE30 reads at most 0x40 bytes into L+0x7A5294: 63 usable)
   u16  port     -> L+0x7A52D4
   u32  match_id -> L+0x7A52D8
   u8   side     -> L+0x7A52DC
   u32  order_id -> L+0x6B0D18
   ```
   Then 0x20018DF30: `L+0x6B0D14 = match_id`, `L+0x6B0D1C = side`, **queue state `L+0x6B0D08 = 2`**, and
   `network_client::connect_to_match(host, port, match_id, order_id)` through `[game+0x10FE30].vt[0x18]` (0x20018DFC4; function
   0x200124CE0, skipped when already in this match). `side` also fills the chat-context record at `L+0x6B0A60`: `+0xAC = match_id`
   (channel 5), side 0 -> `+0xB0 = match_id` (channel 6), side 1 -> `+0xB4 = match_id` (channel 7) (0x20018DFEE..0x20018E014). So `side`
   is the team index 0 / 1, and the chat contexts of a match are all `match_id` (the lobby registry does this: `set_match_context`).
   A client that is still in an old match state tears it down first (0x20018E276..0x20018E308).
3. No handshake, no key exchange: the first match datagram is **C9** `{u32 order_id, u32 session, u8 dbg, u8 0}` (0x2001F06E6). `session`
   is `[nc+0x1138]` = `login_client+0x1040`, the login session (0x20011E2A6). The 2E of the lobby carries the same login session
   (`login_client+0x1040` is the only source; PROVEN for the field, the 2E read at 0x200184F1C..0x200184F40 takes it from the caller,
   LIKELY the same value), so the lobby stores it at sign-in (`Registry::set_session`) and the match server compares it
   (`PlayerSpec::session`, `online_check_session`, default on).
4. S->C `DC` ports -> C->S `CA` probes -> S->C **`DD`** `{u8 local_player_index, u32 level_checksum, LZMA2 match_options}` -> the client
   loads the level -> C->S **`CB`** -> S->C **`DE`** (world) -> `DF` per tick. The world is frozen and DF is dropped until the first
   DE (synced flag `nc+0x7A6914`). Details and timeouts: `netcode.md`.
5. After the match the client stays on the lobby connection; queue state 2 stays until a server `85 00` (play_bots 2.4, match_end 3).

### 1.2 Local path (`5B 01`) versus online

| | local Training (`play.rs`) | online |
|---|---|---|
| trigger | `5B 01 map mode weather` | `2F` queue, room Start `62 23`, squad commander's `2F` |
| options reach the client | `83 5B 01 <size> <LZMA2>` (lobby connection) | **DD** on the match connection (same reader 0x2001250E0) |
| lobby message that starts it | `85 02` + the `83` reply | `82` (sets state 2 itself) |
| `single_player` (+0x107E) | 1: seed 1, end event value 1 | **0**: seed = `match_id`, end event `53 {5/6, value = match_id}` (0x2001168C6..0x200116981, PROVEN) |
| players | human + AI players (`ai_id != -1`) | humans, plus world-host bots in a bot-filled match (section 10) |
| world | built by the client | **sent by the server** (DE) |
| net | none (`vt[0x118] != 0` skips every sync path) | transport + DE + DF |
| results | synthetic statistics, no reward (`LOCAL_MATCH_PAYS`) | the world host's outcome and statistics, else the clients' reports (sections 3, 9) |

`local_match::online_options` writes the DD blob: header `send_period, 0, tick_ms, n, match_id, packed_inputs 0, single_player 0, b3 0, 0,
match_level 1, 0`, level project / map strings of the `game_modes` row, one player record per human (same writer as the training human:
kit, upgrades, modules, skill modifiers; `ai_id = FFFFFFFF`), three zero tables. `match_level` (+0x1081) and the +0x1082 byte are only
logged by the client (UNVERIFIED values 1 / 0). `tick_ms`, `send_period` and `match_id` are the cached world's (`WorldInfo`).

## 2. The initial world (first DE)

### 2.1 The world item (PROVEN unless marked)

`deserialize` (0x2002F1650) and the item loader (0x2002F5040) read, `save` (0x2002F4250) writes the item; the sections are produced by
four sub-functions:

| Section | Saver | Loader | Content |
|---|---|---|---|
| per player block (mask C) | 0x2002F41C0: `player_state` 0x343C40, the object at `W+0x1809D0` (0x3BC9B0), **statistics object per-player part `vt[0xD8]`** | 0x2002EF730 | one block per present player |
| G3 (`item+0x104080`) / G4 (`+0x1041A8`) | 0x2002F4070: a count / time via 0x3E8920, then every entry of the vector `W+0x17D118..0x17D120` (`vt[0x98]`, the **rule modules**: each has a type id at `+0x208`; the finders at 0x2001E4650 / 0x200228FC0 look up types 0, 8, 0xC, 0xD; LIKELY the mode rule core with tickets / scores per player) and then the **statistics object `W+0x180A38` `vt[0xE0]`** | 0x2002F4C40 (`vt[0xA0]` per module, statistics `vt[0xF0]`) | rules + statistics |
| G1 (`+0x1061D0`) / G2 (`+0x106200`) | 0x2002F4110: `u32 count = W+0x17D778`, then every node of the linked list `W+0x17D780` (`vt[0]`) | 0x2002F4D30 (`vt[1]` per node) | the objects the level registers (seven registration sites in `set_game_world_core` 0x2001F9230..0x2001F9EF6; LIKELY level objects: doors, pickups, artefact points) |

Consequences:
* **The statistics object IS part of the DE item** (G3, plus a per-player part in every player block). So a client that has the world
  has kills / deaths / score; the server gets them from its own simulation of the world (the world host, section 3.2; no
  game client runs as a server or referee, and the stock client is not patched).
* The loader diffs the player masks (loops at 0x2002F5120 / 0x2002F5201 call `vt[0x48]`, 0x2002F61B0 / 0x2002F6110 per bit), so an item
  with **no players (C = 0)** is a valid input: players enter later through the `enter_mask` of DF (`enter_match`, same call the local
  path makes for every roster player, 0x2001F47AF). So the loader accepts an empty player mask.
* The content of G1..G4 is the serialization of virtual objects whose classes depend on the level and the mode. A server cannot
  produce them without decoding every class (level object list from the client's resources + each class's `serialize`), and the
  seed / rng state inside is part of it. **Building the first world from static reading alone is not feasible**: the world
  host builds it by running the game's own world code (`servers/world-host`).

### 2.2 The world item writer

The stock client already has the writer in local mode: callback **0x20011A1B0** (guard 0x20011A1C0..0x20011A1E3: `vt[0x38]` is always 1
for `network_client`, so it runs only when the match is local, `vt[0x118] != 0`). It is called only from `new_game_state_history_item`
0x2002F0D10 when the 50-item history pool is full, and it serializes the **oldest** item, the one about to be reused (`[W+0x7DD0]`), with the
item writer 0x2002FC5A0 (full = 1, buffer 0x20CE11E90, cap 0x44800). It hands the bytes to the history ring store 0x20015CDD0 at
**0x20011A295** (arguments: the item time and `{u64 pointer to the bytes, u32 size}`). So the bytes there are exactly what a DE body
is, and the first one (about 50 ticks after the start, C = 0) is the initial item.

**Single player must be 1.** Every decodable `5B` reply runs as a local match (the `5B` handler 0x2001F4070 stores the sub as the match
mode). A local world with `single_player = 0` is created **frozen** (W ctor 0x2002EFF19..0x2002EFF32): it never moves, makes no history
item and never calls the callback. Only a DE unfreezes it (with online options it shows "Awaiting other players").

The world is seeded with `match_id` in both modes (0x2002F6F20). The item's header: time = tick (50), X = FFFFFFFF, Y = 1,
A = B = C = 0, four sections; later items have the humans in them. The level checksum is the client's log line
`collision world checksum = <hex>` (0x2001F9FF9); "Checksum on <map>" is printed only on the online path. The `.srpde` world
file (`servers/match-server/src/world.rs`, written by `srpde-pack`) stores such an item with the map, mode, match id, checksum,
tick and send period.

The weather picks the level (project / map strings, e.g. `level_01_day_tdm`), so a world belongs to one (map, mode, weather) row.
The online loader checks no option, seed or mode against the item, and it reads the sections in its own module / object order with no type
or count check (0x2002F4C40, 0x2002F4D30). So an item loads when the rule list and the level object list match, which the row fixes.

### 2.3 What a start world item holds

* Every t = 50 item splits into the four sections (G1 level objects, G2 empty, G3 rules + statistics, G4 empty).
  PvP header: (50, FFFFFFFF, 1, 0, 0, 0, 255). PvE: Y = 0, because the scripting rule has no waiting state; at t = 100 it
  is already 3, running.
* **The seed is inside the item.** The match id is stored once in G3 of every item (PvP and PvE). So the online match
  must run with match id = the world's seed. The match server refuses anything else (`relay.rs` MatchIdMismatch).
* Two items of the same row with the same seed differ in one G1 byte: offset 58, the state byte of the third level
  object, 2 vs 1. Some level object is set by something other than the seed. This is harmless for the relay design,
  because every client loads the same bytes from DE. Which object it is stays OPEN.
* Difficulty changes 14..15 bytes of a PvE item; the maze changes the object list (about 1070 bytes, different size).
* The six PvE projects share (map 15, mode 7); worlds are keyed by project (2.5a).
* Level checksum: the client's `collision world checksum = %x` is the value the DD checksum compare uses (client and server
  log the same value, e.g. 255f73e4 for level_01 TDM).

### 2.4 Per-player objects in G1

* G1 also holds per-player objects: when the stream's object count differs from the client's own list (e.g. 88 objects for
  one human against 106 for two), the client crashes in the G1 loader.
* The client builds its level-object list itself (PROVEN, exe): `on_match_ready` calls 0x2002F75B0, which walks the
  options records, their state-0 profiles (the cook skips `profile+0x29FC != 0`, 0x2002E5A3F) and inventory slots
  0..22, and calls each item's `set_world`; only then `set_game_world_core` adds the level objects. By the item's
  `data.type`: 9 (grenades) adds `max(5, 2c)` grenades, `c = (u8)(slot count low byte * stack_size)`, none for c = 0
  (0x2002D86BE, 0x200321578); 16 (medkits, LIKELY also bandages / antitoxin / painkiller) adds one `placeable_medkit`;
  2, 3, 13, 14, 15 are placeables not decoded yet; weapons, armour and ammo add nothing.
* At t = 50 a grenade saves `00 FF`, a medkit `00 00 XX 00` (XX uninitialised in the client; we write 0). Their
  owner bytes name no player, so only the number and kinds of objects matter. The level part (fixed 20-slot arrays) is the
  same for any roster.
* `local_match::roster_spec` gives each match's objects from the same profiles the options carry
  (records `|`, profiles `,`, `M` medkit, `G<n>` grenades). The `.srpde` 002 header stores the capture's own objects
  (`srpde-pack --capture-roster`, checked against G1; the capture log prints them as "roster objects"), and the hub
  swaps them for the match's (`World::with_roster`). A kit with an undecoded placeable refuses the online match.
* Every pocket kind: a set holds `c = (u8)(count * stack_size)` objects
  (cook loop 0x200379D17): grenades `max(5, 2c)`, medkits (type 16: also bandages, antitoxin, painkiller) `c` plus the
  skill bonus `floor(max(0, modifier 0x79 + 0.5)) * stack_size`, booby trap `00`, explosive trap `00 FF x6 XX`, scanner
  and totem `00 00`, crate `00 00` + 20 x `FF FF FF FF`, the sponge one grenade.

### 2.5 G3 per roster and the flow after the DE

* G3 is NOT the same for any roster (a wrong one crashes the clients when they save their next item): `kd_stats_rule` saves 5 bytes per options record (u16 kills, u16 deaths, u8 flag; saver 0x20030EDF0)
  and takes the count from the client's own options (`opts+0x1080`, 0x20011C9A4), so a one-record world shifts
  every later G3 byte for two records. The timelimit rule also saves the team counts `records >> 1` (+0x273 / +0x274,
  0x200365A25). `.srpde` 004 records the kd block (G3+42 in TDM / CTP / slaughter) and the
  team counts (16 bytes before G3's end); `World::with_roster` sizes the block and writes the counts. Battle royale,
  artefact hunt and PvE are not split yet: their worlds serve only a one-record roster.
* Enter: a client drops every DF that arrives before its DE is processed, and only the DF enter mask enters a player.
  The relay holds every enter bit and start profile until each entered client answered a DF with a CD (it does so
  before its own enter, 0x2001B0088 -> 0x2001B1100), then enters everybody in one DF and repeats the bits 10 DFs;
  after 20 s without every CD it enters everybody anyway (logged).
* DF lists are capped at the client's buffers: late 60, inputs 20, profiles 20, pings 20 (more asserts).
* Match end: the client's own end sends nothing to the match server. The lobby ends the match there once every seat
  reported (53 events 5 / 6) or 30 s after the first report; the relay's end then pays.

### 2.5a PvE worlds

* The six PvE projects `pve_01_v{1..3}_maze{1,2}` share map 15 / mode 7 but have their own level checksum (a mismatch
  is fatal, 0x2001F1071) and seed. `.srpde` 005 adds the level project (64 bytes) and a flags word; the store and the
  hub key worlds by (map, mode, project) (`WorldStore::get_project`, `Hub::can_create`, `MatchSpec::project`);
  `SRPDE004` files still load (project "").
* The PvE G3 holds, besides kd_stats, four scripting-rule blocks whose length follows the record count (savers
  0x2003FD5EE / 0x2003FDB87 / 0x2003FCC90 / 0x2003F9D40); their G3 offsets are not located. So a stored PvE world holds
  the match's full roster (the human + the 17 script bots = 18 records) and is fixed-roster
  (flag bit 0, set by `srpde-pack --project pve_*`): `World::with_roster` swaps the G1 objects of the records (the
  human's kit may differ) but refuses another record count with a clear error. OPEN: matches of 2 or 3 humans need
  worlds with 19 / 20 records or a G3 splitter that replays the scene saver.

### 2.6 Relay timing

* Three relay rules keep the clients from discarding their own inputs ("update for 1150 still not received by server") and
  hitting the change_input assert:
  * The client matches a pending tick only against the confirmations of the DF it is handling, and moves its own input
    to that DF's time (0x2001B02F4, 0x2001B2650). The relay puts each CD's confirmation, inputs, profile and ping
    entries in DF `a` when that DF has not gone out, else in the next DF (as normal inputs there).
  * A player never gets his own inputs, late, profile or ping entries back (the assert: an input of the local player
    arriving where its own unmoved input is). The server's start profiles still go to everybody.
  * Pre-enter CDs are stamped at DF times; confirming them with the server receive clock would push each client clock ahead
    (jump gate nc+0x7A6915, 0x2001AF6D0). They are confirmed as on time (b = a - 2).
* `enter_match` (0x2002F2890) rejects an enter older than present minus f, f = max(100, the player's ping rounded to a
  tick) up to 500 (0x2002F1400); the original server's world moved a late enter to the next tick (W+0x180AD9). The relay
  delay is 50 ms (below 100) and the enter goes out in one DF.

## 3. Match end and results

### 3.1 What the clients send and do (PROVEN)

* The end is inside the simulation: rule state `W+0x180AE0` = 4 or 5 in the newest history item makes `game::tick` call
  `close_current_match(3)` (0x2001168A1) and push the end event (0x2001165F1..0x2001169A0).
* The only report to the server: the lobby `53` flush with **event 5 (my team won) or 6 (lost; a draw counts 6 for both teams), `value =
  match_id`** (0x20011695E..0x200116988; winner byte `[[core+0x180C68]+0xEBC0]` compared with my team `[game+0x7B63]`). B8 enables ids 5 / 6
  (login push). No client message carries kills, deaths or score (C9..D4 enumerated).
* The client has no `97` request. After the match it waits for the lobby: `85 00` (queue state back to 0) is **required** or Play stays
  dead. Whether the results screen needs a `97` is not known.

### 3.2 What the server does (implemented: `online.rs`)

* The reports are collected per player (value must equal the match id). `decide_winner`: a team wins when a majority of its reports say 5
  and the other team's majority does not; a majority of 6 on both teams is a draw; anything else is unknown. Unknown pays nobody.
* The match server (relay) reports the end (all players left, timeout, lobby request). The lobby pays when every entered player reported or
  after `online_report_grace` (20 s, CHOICE).
* Per player that entered and reported: rewards from `GameData::match_reward` (progression-audit.md section 4; the premium and booster
  percentages of `boosters::reward_percent` apply), the drops (`drops.roll`), and the facts
  `stats::record_match(account, &MatchFacts{ won, finished, duration_secs, match_type (1 random / 2 rated), in_squad, map, mode, faction,
  reputation })`; kills / deaths / score are `None` unless statistics exist. A player who left early or reported nothing gets nothing, only `85 00`.
* `97`, `8B`, `8C`, `8E` and `85 00` go to each player (online or not; the profile is saved either way). The `97` is sent to every paid player; its
  numbers come from `MatchEnd::stats` when the world host delivered them and are 0 otherwise (section 9).
* Not served, named: the kill list (`KillFact`)
  for the achievements that need weapon / victim details, `best_match_member` nominations.
* With the world host (required): every PvP match is fed to the host; an end
  the relay does not mirror (tickets, deliveries, points, rounds) comes from the host's Outcome (`EndReason::HostEnd`: every client closed
  at once); after a mirrored end the `MatchEnd` waits up to 1.5 s for the host's `Final`. The winner is the world's byte `S+0xEBC0`
  (0 / 1 team, 2 draw, 3 none = the reports decide); `MatchEnd::stats` holds the world's kills (event row 0), deaths (row 2), score
  (`+0xA1C`) per player and the team scores (`S+0xEBDC` / `+0xEBDE`). They flow into the `97`, the team place, the league and
  `record_match`. Medals / nominations: OPEN (badge slots `B+0x534` exist; their `97` mapping is not proven).
* Payment with the match server's winner: no report is needed, but a player who left before the end in
  any way (menu, disconnect, silence) is paid nothing; only the menu leave is fined (a disconnect can be a crash).

## 4. Wiring and switches

* Settings (config file, docs/notes/config.md): `matches.report_grace_secs` (`online_report_grace`), `matches.min_players`
  (`online_min_players`, default 2, CHOICE: a queue group of two players starts a match; the mode row's own minimum is higher),
  `network.match` (127.0.0.1:1236), `network.match_public` (host text for the clients, at most 63 bytes). Online matches are always on and the
  C9 session is always checked; `online_queue` is a test-only override.
* `online_queue` only PINS the queue to one row (a test-only override: it ignores the masks); unset (srp), the queue chooses among the rows
  the world host serves (the daemon's project list, section 7).
  With no world or no match server hook every start is refused (`85 00`, "not available" text for a room). `match_level` and
  +0x1082 of the options are unverified.
* Paths: queue group (`queue::start_if_full` -> `online::queue_start`, teams by `balance`: CHOICE, whole tickets, largest first, even teams),
  custom room (`rooms::start` -> `online::start_room` on the room's row: rules and bots in `social.md` section 4; the room closes), squad play (the
  commander's `2F` queues one ticket, same path). Accounts already in a match cannot queue. One busy rule (`servers/lobby-server/src/busy.rs`, CHOICE) covers every way in: an account is in at most one of a custom room, a PvP search, a PvE search, an online match and a local match. `2F`, `5F` Play, `62` Create / Invite / Answer and local `5B` Play ask it first, each answering in its own channel (rooms: the game's text "Player is already in a match or looking for a match"; PvE: `deny::GENERIC` + `85 00`; PvP and local Play: `85 00`). A new local Play replaces a stale local-match record (the client can leave a match without the end flush); the other entries refuse while a local match is recorded. A member who joins a squad (`49 02`) while the squad searches ends that search (`85 00` to every member): the ticket names the members it was made for, no source says the original did otherwise (CHOICE).
* `servers/match-server/src/bin/srpde_pack.rs` writes worlds; `MatchServer::start_with_hub` lets `srp` keep a hub handle.
* Tests: `servers/lobby-server/tests/online.rs` (recording hook: queue group, reports, grace time, split vote, refusals, room Start),
  `servers/srp/tests/online.rs` (the real hub with two scripted match clients behind the lobby), unit tests of the verdict, the team balance, the
  `82` layout and the options header.

## 5. Open

* DF `code` values; late join (not needed). E0 runs only while a world host's link is live (`netcode.md`: the host's
  hashes). A CC is then answered with a DE of the running world while resync is on for that client, else logged and
  not answered. Without a live host there is no E0 and a CC is answered with the cached DE, which a running client
  ignores (no repair).
* The mask's write path from the check boxes (7.1); the original rotation and map pool (server settings, in no client file); `match_level`.

## 6. Game-run checks (two game copies), in order

1. **One client, one world**: a one-player room is refused (both teams needed), so use two copies. Check the 82: the client logs
   `connect_to_game_server: host:port match_id:N`, then C9 reaches the match server (`C9 from ... unknown order id` or a refused session in the log
   = the session assumption is wrong).
2. **Handshake**: DC accepted, DD accepted (no "Checksum mismatch", the log shows Match id / Map / Players), level loads, CB arrives, the first DE
   gives "THE VERY FIRST SYNCHRONIZATION" and no "connection forbidden".
3. **Two clients in the match**: both enter (enter_mask), see each other move; no "large network lag", no "RESYNCHRONIZATION"; the hash
   ring `nc+0x7A75C0` compared between the copies stays equal for 5 minutes.
4. **Chat**: channels 5 / 6 / 7 reach the right players (match context = match id).
5. **End**: let the timer run out. Both clients return to the lobby; `85 00` arrives, Play works; the log of the server shows the two reports and the
   verdict; rewards (8B / 8C / 8E) and the results screen appear.
6. **Leave early** with one client: it gets `85 00` and no reward; the other still ends normally.
7. Then: squad play and the PvP queue, the mask mapping, bots.

## 7. The `2F` game-mode mask and the queue's pick of map, mode and weather

### 7.1 What the client sends (exe)
* `play_button_clicked` (0x200185E60) writes `2F u32 server, u8 type, u32 mask, u8 flag`. The mask is the saved config value `match_modes_mask`
  (static dword 0x200AA2120, default `0xFFFFFFFF`) and is sent as **`0x10` alone when bit 4 is set, else unchanged** (0x200185F37..0x200185F49). PROVEN. The event check box (bit 4) is exclusive.
* The lobby's refresh function (0x200177ED0) loops `i = 0..9` over `1 << i`, masks the saved value with `0x3FF` (0x2001782F5) and calls
  `root.set_game_mode_enabled(i, ..)` / `set_game_mode_selected(i, ..)`. PROVEN: the mask has ten bits, bit `i` = mode `i`.
* The movie's `MatchMakingSettings._modeButtons` holds **seven** check boxes, indexed by that same `i`: 0 `battery_charge`, 1 `team_match`, 2
  `capture_points`, 3 `gun_race` (the `checkBoxDomination` slot), 4 `paintball` (the event box), 5 `search_destroy`, 6 `slaughter`
  (`set_game_mode_selected` indexes `_modeButtons[i]` and returns when there is none). PROVEN. These match the data's mode ids: 0 storm /
  battery charge, 1 tdm, 2 ctp, 5 sad, 4 paintball, 6 slr; the data also has mode 9 (artefact hunt, `ah`). LIKELY: bit = the options'
  `mode` id (six of seven names agree; nothing computes the mapping in the exe).
* Bits 7, 8, 9 have **no check box**, so the exe never clears them and every client's mask carries them. LIKELY explanation of the `0x380` seen in
  a real log: no check box ticked (all locked or off), the three untouched bits remain. The lock rule: a mode (except bit 4) is disabled and
  unselected while the account's match counter (`9D` `G`, +1 per `97`) is below `9D` `H`; the server sends `H = 0`, so nothing is locked.
* OPEN: the code path that stores the Flash selection (`getSelectedModesList` -> `[selected && unlocked, unlocked]` per box) into the config value.

### 7.2 Queue groups
* `queue::modes_of_mask`: bit `i` -> mode id `i` (mask 0 = any mode). `PvpQueue::group_for`: every waiting ticket can seed (oldest first); a ticket
  joins when the mask it shares with the group is non-empty (a 0 mask adapts) and still has a playable mode; the first seed whose group holds
  `online_min_players` players wins. The group's shared mask is the match's mask. The original's map pool and rotation are
  server settings; the client data has none (only `singleplayer_match_maps.options` for the bots tab). The data's `wait_player_percent`
  / `min_players` are in-match script values and are not used for the queue size (`online_min_players`, CHOICE).
* No served row at all -> the full group is refused; rows exist but nobody shares a mode with the waiting players ->
  they keep searching (`85 01`). A mask that selects only modes without rows (the event bit: no paintball row) waits. With a pinned
  `online_queue` row, fewer than `online_min_players` searchers, or tickets that do not split into two teams, also wait: the refusal at the
  smallest row minimum (`queue::players_needed`) is only the fallback when no match server can start the queue.

### 7.3 Mode and map selection
* The test-only `online_queue` (e.g. `(1, 1, 0)`) pins every match to one row and makes `AC` offer that row's bit only; the
  other modes then show "The mode is unavailable".
* What the client sends (PROVEN, 7.1): only the mode mask (and the game-server mask); no map, variation or weather preference exists
  in the exe (the only saved matchmaking key is `match_modes_mask`). The map is the server's choice.
* `update_game_modes` (0x200177ED0, PROVEN): mode i is enabled iff its `AC` bit is set (battles lock aside); selected = saved bit and
  enabled; the number of selected modes, **counting mode 9 but not 7 / 8** (0x200178256), goes to `root.set_selected_game_modes_block`
  (count <= 1 locks the last box). The saved mask always carries bit 9, so `AC` must never set bit 9: the player could untick every
  visible box and search for ah alone, which has no check box.
* Rows the match server serves: `OnlineMatches::serves(data, row)`; srp: a world host serves the projects of its
  project list (the daemon answers srp's project-list query; it serves the game data's projects), variation and weather included (each is its own project); a stored world (tests) only its capture's
  row (variation 0, day). `online::playable_rows`.
* `AC` (login.rs `play_gate`): the check-box modes (bits 0..6) that have a served row in the unrated pool. The test daemon's projects
  (level_01..05 day tdm, level_05 day br / ctp / slr_vn / ah) give 0x47: battery, TDM, research (ctp), slaughter.
* Pool (`queue::Pick::in_pool`): every served row except Rudnya battery (1, 0) and Cologne Bridge research (8, 2), "disabled in all
  game modes, including ranked" (OFFICIAL 2021-11-17). The same post's School v2 research and London v3 battery / research are OPEN
  (London has no fourth variation in the data, so the post's numbering is not the `_vN` suffix).
* Unrated (`PvpQueue::choose` / `pick`): the mode among the shared ticked bits 0..6 (no-button bits never: ah was custom-only in 0.69d,
  OFFICIAL), the one whose last queue match is the oldest (CHOICE); the map on it the one played longest ago (CHOICE: NOT FOUND how
  the original picked it; the rotation spreads the maps). Modes or maps that are equally old (every one after a server start: the
  rotation is kept in memory) are drawn at random among them (`pick_random`, CHOICE); then a uniform draw among that map's served rows, i.e. the variation
  ("one of the variations is selected" per match, OFFICIAL 0.46, random per a FAN summary) and the weather with it (CHOICE: the
  data has one row per weather and no weights). Players with no ticked mode in common, or only modes nobody serves, keep searching.
* Ranked: the mode is the 10-minute slot from 00:00 UTC, battery -> TDM -> research -> S&D (OFFICIAL 2020-07-05; `queue::ranked_mode`),
  whatever the players ticked; maps from the ranked list at their default variation (`queue::RANKED_MAPS`, OFFICIAL season posts
  2020-03 / 2021-09; `queue::default_variation`: 0, S&D see 7.4). CHOICE (`queue::ranked_order`): a slot whose mode cannot form the
  match (no ranked row served, or the group does not split into that row's two teams, e.g. S&D's 5 + 5 seats under a larger
  `online_min_players`) falls through to the next mode of the rotation that can (S&D slot: battery, then TDM, then research);
  the searches never wait silently for the next slot. With every ranked mode served the slot's own mode always plays.
  Ranked tickets group regardless of their masks; still humans only, no squads (client), off during an event.
* Bot fill (`matches.bot_wait_secs`) uses the same unrated pick on the served rows, without the modes the AI cannot play
  (`online::NO_BOT_MODES`, 7.4). The match options carry the picked row's project and map name
  (`local_match::online_options_row`); the world host builds that project. Players per team (`online::bot_team_cap`):
  the smallest of `matches.bots_per_team` (default 6, 1..=8), the hard cap `online::BOT_TEAM_MAX` 8, half the row's
  match size and the patrol graphs per team (8, 7.4); the bot-fill log line names the limit that set it.

### 7.3a Loaded maps: the world host's room
The world host loads at most `max_zygotes` maps (level projects) and runs at most `max_matches` matches per map (up to
twice it while 1.2 GB is free); a match it has no room for waits 20 s in its front and is then refused. The lobby
therefore chooses with the host's room (`lobby_server::host_room`):
* srp asks the daemon for its room about every 0.5 s (`ListStatus` 0x06 -> `Status` 0x8C on a connection of its own,
  `world_host::watch_room`, `match_server::query_status`). The front answers from the same rules a match meets
  (`daemon::room_status`): per loaded map its running and starting matches (handed over and not taken, or Creates the
  front has read and not handed over), whether it takes one more (`match_room`), its rank in the eviction order when idle
  (no match running or on its way; the front's own placement counts an idle map with a Create on its way as busy too)
  and its age; the maps that are not loaded and can load now, in order, each with the idle maps evicted for it (`plan`
  for a new map again and again: under the limit, idle maps to evict, or one over the limit with 1.4 GB free, each map
  listed before it counting 0.42 GB of that memory; none while the base process is not ready); and, only while no new
  map can load and the base is ready, the map to drain
  (`daemon::DrainTurn`: the busy map loaded the longest that is not a `prestart` map; kept while busy, for at most
  20 min; once idle or gone the next one; one still busy after its 20 min is passed over until a drain succeeds; the
  front cannot tell a custom room's or clan battle's map, so such a map is passed over that way). srp keeps the latest status
  (`srp::host_room::RoomBook`); the lobby reads it without waiting (`OnlineMatches::host_room`). Every match srp creates
  is counted against it (a reservation, `with_reserved`) until a status asked 1 s or more later arrives (the daemon
  counts the Create from its first frame, sent at the match's creation): a start on a loaded map counts as starting
  there (and makes an idle map busy); a start on a map that is not loaded takes the next new map, and the idle maps the
  front evicts for it are closed to the queues; the new maps left are those whose evictions are still possible. So two
  groups never both take a map's last slot or the same idle map. A failed read keeps the last status; one older than
  5 s, or no world host, = no status: the queue picks as before. Custom rooms and clan battles do not check the room.
* The choice (`host_room::scope`, `PvpQueue::choose_in` / `pick_in`), at every start, with no state kept between starts:
  a new map can load -> the rotation of 7.3 over every served row (without loaded maps that take no more match);
  no new map can load -> only the served rows whose map is loaded and takes one more, the rotation's order among them
  (the mode, then the map whose last queue match is the oldest, a never started one first; so a freshly loaded map is
  picked once, then the others take their turn), the variant the loaded one started the longest ago (ties drawn).
  Avoided rows (`host_room::avoided`, taken only when no other row fits the group: a group is never held for them):
  while no map can load, the idle maps and the map the front drains (`host_room::drain_map`), so busy maps with room
  go first and the drained map turns idle (over the limit a second map can idle too, and two evictions make room for a
  load); while a new map can load, the idle maps that load evicts (the first new map's evictions, the lowest ranks), so
  the rotation loads a map that is not loaded and the idle map goes, instead of the rotation picking the idle map again
  (its last start is old, since it was drained). Ranked takes its slot mode from every
  served row in both scopes (it falls through to the next slot's mode only when the slot's mode has no served row at all)
  and then only the scope's rows of that mode: every map of it full or not loaded -> held. Nothing fits -> the group is
  held (`PvpQueue::group_in`): it stays queued, a line is logged once per group (its mode mask) and kind of hold (maps
  full, no room, base not ready; `queue: held, the group (key 0x...) stays queued: ...`; again after a start) and the
  queue is tried again every second from the tick until it starts. As soon as a map can load, the next start is the
  rotation's again. The bot fill (same hold lines) and the ranked fall-through after a failed team split use the same
  scope. The reservations err on the safe side (an over-limit slot is used up by one start; a match the next status
  already counts is counted twice until the reservation ends): a group can be held for up to about 1.5 s more than
  needed, which the 1 s retry recovers.
* PvE (`pve::start_full`, `host_room::pve_maze`): while a new map can load, the lobby's maze draw when that maze is not
  loaded or takes one more match, else the other maze under the same test, else held; while no map can load, a loaded
  maze of the group's difficulty with room (both: the one not drained, else the one started the longest ago); neither:
  the group is held before anything is charged (one line per difficulty and kind), tried again every second; the groups
  behind it may still start (`next_group_without`).
* Log lines: `world host room status read (every 500 ms): ...` / `... not read (...)` (srp, at each change of the link),
  `queue: no new map can load: narrowed to the N loaded row(s) with room (...; world host status M ms old: ...): ...
  candidates ...: picked P (...)` (with `draining D (loaded N s, the longest): ...`), `PvE queue: the drawn maze K ... is
  loaded and takes no more match: maze J instead`, `PvE queue: no new map can load: maze K ... is loaded with room
  (...)`, and the held lines.
* Checked in runs without game clients (a daemon, srp reading its room, matches created by the test tools): the
  status and its 500 ms reads, maps over the limit and full ones, the drain pick (the busy map loaded the longest),
  the drained map going idle and evicted first, a dropped link logged once and read again, no refused match. The
  queue's holds, reservations and rotation over the loaded maps need game clients in the queue and are not checked in
  a run yet (unit tests only).

### 7.4 Search and Destroy (mode 5)
* Rows: besides the training modes (0, 1, 2, 6, 9) the extractor
  (`crates/data-extract`) reads the mode files' own `match_making_params` for mode 5, limited to the
  layouts the client's custom-match list offers (`singleplayer_match_maps.options` `custom_match_data`) and to shipped projects:
  School `level_02_day_sad_v3` (variation 7), Vostok `level_03_day_sad` (5) and `level_03_day_sad_v2` (6), Abandoned base
  `sad_abandoned_base` (map 12, variation 5). 10 players (5 v 5), weather 1. School variations 5 / 6 ship but the client does
  not list them: not served. PROVEN (mode files, client list, resource list).
* Level path: `sad_abandoned_base` keeps the mode file's folder-less name; the exe loads `projects/sad_abandoned_base/default`
  from it (world host probe: start world built, its requests name `.../sad_abandoned_base/default/...`). PROVEN.
* All four projects build their start world in the world host (probe: 368..691-byte start worlds). PROVEN.
* `AC`: with the rows served, bit 5 is set and the client unlocks the S&D box. Unrated: the box plays S&D like any ticked mode.
* Ranked layouts (`queue::default_variation`): the client labels variation id 5 `st_variation_0_name` on Vostok and Abandoned
  base (PROVEN), so 5 there; School lists only id 7: CHOICE, School's ranked S&D layout is 7 (School is on the official ranked
  S&D list). Vostok's second layout (6) is unrated only.
* Rounds (world host, ten idle players, level_03_day_sad): start at enter + 10 s; each round ends on time after 120 s plus the
  pause between rounds (137.1 s apart), the defending side winning; sides swap after `rounds_count` 3; the match ends about
  7 s after a team's 5th round (`rounds_to_win`), here after 8 rounds at start + 1086.75 s, winner team 1, team values 3 - 5
  (S+0xEBDC / S+0xEBDE = rounds won), event 49 `sad_round_won` counted per player of the round's winners. PROVEN (host run).
  So an S&D world plays past its `match_time` 900: the S&D rule stores it (ctor 0x2003AC0C0, rule +0x308) but no reader was
  found, and the mode lists no timelimit rule (102). The relay therefore gets no time-limit mirror for S&D
  (`srp::online` `end_rule`: None): the world host's Outcome ends the match (relay `host_poll`), `max_duration` (40 min) is the
  backstop.
* Achievements 627 / 628 (mode mask bit 5) count from an S&D win; 627 needs the 5 - 0 lead (team values = rounds).
* Bots: the exe's AI cannot play S&D. A PvP bot claims a team patrol graph from the level project's `server_objects`; the S&D
  projects have none (world host runs on level_02_day_sad_v3, level_03_day_sad and sad_abandoned_base: "patrol graphs per team
  [0, 0]", PROVEN; level_03_day_sad_v2 has none in its project data either), a bot without a graph is undefined in the
  exe and the host refuses the match. The graphs are exclusive: a bot claims a FREE graph of its team (`team_id`) at its
  start and at each respawn and frees it at death; with none free the pick reads an uninitialised index. Every project
  of a bot-playable row has 8 graphs per team (one has 9; project data, PROVEN; the world host logs [8, 8]), so a team
  gets at most 8 bots (`online::PATROL_GRAPHS_PER_TEAM`). The exe holds 20 player records and 20 AI controllers per
  match, so 8 + 8 fit. The client offers no S&D bots training either. So S&D gets no bot fill and no room bots (`online::NO_BOT_MODES`): an
  S&D-only search waits for ten players; a search that also ticks another mode gets its bot fill there. The AI's objective
  code (plant / defuse / carry) was therefore not reachable and is not claimed.
* Facts of an online match with real statistics (`Ended::stats`): `team_place` = teammates with a higher score (0 = best, equal scores share a
  place), so a rated match moves the league rating (`league::record`).
* The reward level is PER PLAYER (`online::reward_level`): the player's own equipment level (`items::equipment_level`, the
  client's 0x2003330C0: highest worn item level over his playable profiles; pockets and empty stacks do not count), clamped to 1..5. It indexes
  `match_levels_factors`, the `drop_after_combat` roll (silver, spare parts, chests) and the badge awards, and it is the `97` header level and
  R +0x2E of his profile; the repairs charge uses the same number. OFFICIAL: Guns Race notes 0.61b / 0.65d / 0.69b "The list of rewards does
  not depend on your maximum equipment tier" (so the other modes do), developers 2022-01-27 "the rewards are a stimulus to purchase higher tier
  equipment. So if you are after rewards, you should play with a higher equipment tier", 0.56b "Silver penalty depends only on the maximum
  equipment tier of your character"; the `97` keeps a level per profile.
* The events' silver factor is the server setting `LobbyConfig::match_silver_factors` (index = reward level), default
  `MATCH_SILVER_FACTORS` = 1 / 1 / 3.5 / 7 / 10 / 14 for levels 0..5, in place of the 0.69d data's `match_levels_factors.silver_factor`
  1 / 1 / 2 / 3 / 3.5 / 4 (empty = the data). Reason: with the 0.55 repair prices the data's factors leave average players behind from
  level 4 and everyone at level 5; with the default a strong player (15 kills / 5 assists in 15 min of TDM) nets about +1200 .. +5900 per
  match at levels 1..5, an average one (8 / 3) breaks even at level 5, a weak one (4 / 1) loses at levels 4-5 (CHOICE).
  Experience, reputation, the drop roll and the badge awards (computed by the client with its own data factors) keep the data's values.

### 7.5 Arms Race (gun race, mode 3)

Details: [gun-race.md](gun-race.md). The row `(13, 3, 0, 1, arena_01, arena_01/default, 8, 8)` comes from its own mode file
(`data/gunrace_069d.json`); the world host serves `arena_01`. All versus all: every seat is its own team (its record index),
each record carries the nine race kits of the match's equipment level (the humans' highest) as profiles 0..8
(`local_match::gunrace`); race level 7 carries the icicle kits (CHOICE: the data's snowball kits fault
the exe, their `grenade_set` file is not shipped). arena.lua ends the match in the world host (race winner = `player_id`,
9 at the 10-minute end), the relay passes that byte on (`match_server::mode_winner`), the lobby pays the seat whose team
it is, places by score over the whole table, the `gunrace` event rows with no level factor, reward level 1, no
after-match charge. Humans only (`NO_BOT_MODES`), never ranked, squads may queue. Offered in `AC` (bit 3) and the unrated
pool (`queue::GUNRACE_OPEN`).

## 8. Match boosters online

* The per-match boosters (339..344) are charged **at match creation** (`boosters::charge_profile`: stock first, else
  the silver of `booster_use_costs[booster level][nth]`, an unaffordable one is switched off). The rule itself is LIKELY (store.md section 4: nothing
  in the exe says when the real server charged). The row is the client's own (`online::booster_level`): the client prices
  the boosters it shows (`booster_participation_costs` 0x2001BF460, fed by 0x2001BFBC0) with row = max(the active profile's level 0x2003330C0, the
  lobby bytes +0x39C / +0x3A0, every squad member's level), the level itself as the row index (0x2001BF65C). The two lobby bytes are written only
  by `root.set_equipment_level` (0x2001C75CF: the highest level over the player's playable profiles; `root.set_mm_search_range` shows them) and
  the squad members' levels are the AE `items_level`, which the lobby fills with `items::equipment_level`. So the row = the highest
  `items::equipment_level` of the player and his squad mates (solo: his own; level 1 = row 1). `Vouchers::match_cost` reads row = level. Online PvP and custom rooms (`start_room`) charge this way; PvE matches never charge boosters.
* Order: the charge is run on a copy of each seat's profile first, so the options carry exactly the boosters that will be paid; the real charge is
  saved after the match server accepted the match (a refused match costs nothing) and every member gets the used stock (`83 45 03`), `8B` and the
  `94` records.
* The in-match effect is the per-player modifier block of the options (store.md section 4). `local_match::online_options` writes
  `HumanLoadout::extra_modifiers` like the training builder (test `online_booster_modifiers_fill_only_their_players_block`).
* All kinds except PvE are charged (rated, random, custom training, clan). OPEN: whether 0.69d charged boosters in a custom match, and whether PvE
  allowed them at all (a PvE seat still carries its enabled boosters without being charged: `pve.rs` owns that match).
* A local Training match with bots (`5B 01`) charges no booster: it pays nothing
  (`LOCAL_MATCH_PAYS`), so its boosters are not used up; they stay selected for the next online match. Test
  `a_local_training_match_charges_no_boosters`.

## 9. The `97` results screen of an online match

* PROVEN: in the lobby the results screen exists only after a `97` (`fill_victory_screen` builds the "MATCH"
  notification); nothing else triggers it. The client has no `97` request and sends no statistics (match-stats.md): the numbers can only come from
  a server that observed the world.
* Sent to every paid player (and to the players of a custom training match, with a zero reward): names, public ids and teams of the roster,
  the winner byte (section 3.2; draw = 2), the paid experience / silver / reputation in the reward block (`ResultFull::rewards`). Kills / deaths / score and the team scores come from `Ended::stats` when a source delivered them, else 0 (a
  player the source did not list gets 0). Not sent when the verdict is unknown or the player left / reported nothing: nothing is paid then either.
* CHOICE: zero rows show "0 / 0 / 0" in the table. The alternatives (hide the table, or synthetic numbers as the local path does) are worse:
  the first is not a layout the client was proven to render, the second is invented data.
* A bot-filled match lists its AI players after the humans (ids 1.., the host's counters, the
  name of their two name indexes in the language the client sends in `32` after the 80 / in `5C 00`; the window shows names raw). The personal
  tab's three reward rows (R +0x38) carry the base reward per `events.options` `ui_category` with the client keys `st_statistics_score_kills` /
  `_help` / `_game_events` (the real server's sample in the lobby movie's `DebugData`), the bonus rows the score blocks 1 equipment, 2 premium,
  3 boosters, 8 decals, 9 hero; block 10 `game_mode` is never drawn by the window and stays 0. The blocks go on the wire in the order 0, 1, 4, 2, 3, 5..10 (league before premium and boosters; reader 0x200181D0A,
  `SCORE_WIRE_ORDER`); the numbers here are the categories. The window's "total" = the rows + every block
  but block 2 + the client's badge row; "total premium" = "total" + block 2 (`VictoryScreenPlayerMatchDetails.setData`; the row is drawn in
  `PREMIUM_COLOR` when the player's own row has its `premium` flag, else in `PREMIUM_COLOR_SECONDARY`, and the totals swap their highlight).
  So block 2 is the premium share a premium player was paid ("total premium" = paid), and for anyone else what the premium account would have
  added, +50 % of the base experience / silver / reputation ("total" = paid).
* The after-match charge (`charge.rs`, the `97` tail lists, taken after the reward with an `8B`; inactive players pay nothing):
  * "Match costs" = the equipment use silver of the player's equipment level (`items::equipment_level`, the client's
    0x2003330C0: his highest worn item level over the playable profiles), one entry of dict 0xFFFF when it is above 0. The
    prices are the setting `LobbyConfig::equipment_use_costs`, default `EQUIPMENT_USE_COSTS_055` = the OFFICIAL patch 0.55
    repair prices (Steam 2018-10-31: level 1 0, 2 1000, 3 2000, 4 5000, 5 8000 silver; "the price for repairs is now
    untied from the item of equipment or weapons ... depends on the level of equipment"; 0.56b: "silver penalty depends
    only on the maximum equipment tier of your character"). The 0.69d data's own `equipment_use_costs`
    (booster_parameters.options) is 0 at every level; an empty setting uses it. The entry is the client's own "repairs"
    group of the pre-match cost (0x200333210 -> 0x2001BFBC0 type 2; the details window titles a repair entry of dict 65535
    `btn_faction_equipment`). The pre-match screen computes its number from the client's own data table (zeros) and no
    message carries a table, so it shows 0 while the server charges the 0.55 price. The per-item
    `match_participation_cost` of game.options is not charged (the exe never reads it).
  * "Special items" = the pocket units the world host's `Used` row (protocol 6) reports, at the 8A shop price (silver items only).
  * "Ammunition" stays empty (rounds are free); boosters are paid at the match start (`booster_use_costs`, boosters.rs). Every per-profile value is at the current equipment profile (R +0x1C play time = the match length).
  * Premium ammo boxes (slots 11 / 12, categories 37 / 38): an online PvP match takes ONE box per box slot per player (the
    primary weapon's and the pistol's), whatever profiles he swapped (CHOICE, `charge::match_boxes`: per slot the first
    box the world host's `Used` row lists). A player who got in and left early (or got no verdict) loses his boxes too when his `Used` row shows
    one. The stack loses one unit: `83 45 03 <uid>` (the client lowers the stack by one), then `87` when the stack ran
    out. No money, nothing in the `97` lists. The custom training and the gun race take no box; a PvE mission takes one
    per spawned profile and box slot. Log lines: "spawned with premium ammo boxes ... one per box slot, takes ...",
    "used N premium ammo box(es)", and "no Used row from the world host" when the host sent none (then nothing is taken).
* The window opens only from the lobby notification (`LobbyMenu.fill_victory_screen` always adds it). The original's automatic screen is the
  in-match one: the `97` handler fills the HUD too when the 97 arrives while its match is loaded (0x20018B2F1..0x20018B473), and the ui rule's
  end hook (vtable slot 17 0x20025EA90) shows the splash and then the statistics; that needs the `97` before a code-2 close. NOT built for PvP
  (relay/host order: badges come after the host's Close).
* The advantage notice ("We are winning / losing, the match will end in 30 seconds", `st_match_advantage_our_team` /
  `_enemy_team`) is the client timelimit rule's own listener 0x2001E7980, run every game-world frame (job 0x20023A980,
  bucket 4, scheduled at 0x200238AA1). It posts `root.show_game_message(text, 4000)` again on every run once the time cut
  happened (`rule+0x271`, never cleared) and world variable slot 9 (advantage team: tdm kill 0x200366868..0x2003668E5,
  leave 0x200365D8C / 0x200365E10; never reset) holds a team; its "last shown" byte `rule+0x2AC` is written only by the
  constructor. So it stays through the last 30 s, comes back after the splash's own `removeMessage` and lasts until the
  world is unloaded. Every gate is client world state computed from the relayed inputs (the DF carries no world
  variable); the only server-set gate, the options' single-player byte (+0x107E), also switches the leave cut off, so it
  is not used. No server change.

### 9.1 What the relay can observe
* The relay sees, per tick: the entered / left masks, every player's `player_input` (two rotation u16 and a 56-bit action mask: movement, fire, ...),
  profile changes and pings (`netcode.md`), and the clients' `CC` hash complaints. It never sees positions, hit results, health or the world.
* A kill needs the world: the shooter's aim against the other players' positions and colliders, weapon ballistics, armour, health. The clients'
  inputs are applied by each client's own deterministic simulation; the DE the server holds is the starting world only (the clients never upload one).
  So kills / deaths / score are NOT derivable from the relayed traffic; the "shots fired" count (fire bit set) is derivable but is not a statistic the
  screen shows. The statistics object lives in the client world (per-player 0xA24 blocks), so a source needs a simulation
  of the same world on the server: the world host (section 3.2). No headless game client is used.

## 10. Bots in online matches

* PROVEN (call sites): the AI controllers (`ai_player_controllers` start, 0x2002D6B50) have exactly one caller, the **local** `on_match_ready`
  (0x2001F4B2F in 0x2001F4240); the online `on_match_ready` (0x2001F0A20) never starts them, and the patrol graph request is also local only. The options reader accepts `ai_id != -1` players with their AI block in any match (same reader 0x200125460), but without a
  controller such a player would stand still: **the clients do not simulate online bots from the options**.
* So an online bot is a player whose `player_input` a SERVER produces and relays (DF input lists carry any player index; the `ai_player_input
  {u16 type, u16 param}` list is the script command channel, `netcode.md` 7.4). That server needs the AI code on a live copy of the world
  (vision / hearing sensors, patrol graphs from the level project, fire queues) and an input feed per bot into the relay, created
  tick by tick inside the lockstep (the 500 ms rule of `netcode.md` 2). The world host does this: the queue's bot fill (7.3) and
  room bots (`social.md` section 4).
* The original 0.69d server did this for PvE (pve.md section 10: the proof, the roster, what the server sends). `ai_player_input`
  type 1 is a scripted animation (the simulation plays `param`), not only an AI pause; type 2 is a voice phrase. Bot inputs come from our server only (no patched client).

## Web evidence

OFFICIAL unless marked:
* Queue modes 0.69d: Team Deathmatch, Research, Battery Retrieval, Search and Destroy, Slaughter; Guns Race in Paintball's
  place since 0.69c ("we just removed Paintball", Feb 2022); Artifact Hunt only in custom matches (since 2019). Players pick
  modes when queuing; no per-map exclusion.
* Pool: non-ranked PvP has "all balanced maps" (no team wins over 55 %); disabled since Nov 2021: Rudnya Battery, School v2
  Research, Cologne Bridge Research, London v3 Battery, London v3 Research. The variation is picked per match (0.46).
* Rotation: ranked only, a 10-minute mode loop from 00:00 UTC: Battery, TDM, Research, Search and Destroy, on the ranked map
  list. General PvP: no rotation; map weighting NOT FOUND.
* Team size up to 8 v 8; ranked about 5 v 5, no squads, no bots. Matchmaking: equipment tier first, widening with the wait;
  since 0.69 Elo team balance incl. the top-2 / top-4 players.

## Web round 2

Leave / AFK penalty [0.5x 0.56b 2019-01-10 gid 2417771014048749680, in force through 0.6x, OFFICIAL]:
leaving costs silver that "depends only on the maximum equipment tier" and blocks the queue for a time that grows with "how
many matches you left in a day"; "You will receive the same penalties for not participating in a match (for being AFK)";
nothing if only bots remain; none in Team Missions, custom or local matches; leavers get no match reward (2022-01-27), no
Elo change (2021-08). AFK = fewer than 5 points in 5 minutes, reset by dying while moving (FAN moderator 2019). Player reports
(3 min first wait, ~2000 / ~5000 silver by tier, 20 min after repeats) fit the client's `inactivity_fine.options` (money
1000, ban ladder 0 / 180 / 180 / 300 / 300 / 480 / 480 / 600 / 600 / 900 s). The exact fine per tier: NOT FOUND.
Server: a player who got into a random / ranked match and reported no result while the others did pays
1000 x his equipment tier silver (`game_data::fines::leaving_fine`; the tier product is a CHOICE fitting the reports; data
`leaving_match_fine.options`) and cannot search for the data's `ban_time` of his n-th leave of the UTC day (schema v19
`leave_penalty`); he is told with 8B + an `84 2F` text, a blocked `2F` gets the time left. None in custom, clan, PvE or
local matches, none when no result was reported at all (verdict unknown).

## What the relay alone tells the match server

* Kills, deaths, assists, score, killstreak, place, objective counts: NOT derivable from relayed traffic (PROVEN). In-match C->S
  messages are C9, CA, CB, CC, CD, CE, CF, D0, D2, D3, D4 (D1 unused); none carries kills, damage, hits, own death / killer,
  score, objectives or the match end. The original server's E0 carried per-player world hashes no client uploads, so it LIKELY
  simulated the world itself.
* Respawn requests: `profile_input` byte 1 = 1 (the `request_respawn` key, or the PvE paid-respawn confirm). NOT a death count:
  tdm (1) and slaughter (6) respawn without a request, `set_player_autorespawn` removes it everywhere, first spawns / repeated
  presses add extras (PROVEN).
* Inactivity (client rule, PROVEN): per-player stats core + i x 0xA24 + 0xA10 time / + 0xA18 flag (= block +0x9E8 / +0x9F0,
  match-stats.md round D), stamped only by simulated events (damage dealt,
  hit while having moved in the last 30 s, score gain, search-and-destroy zone checks, entering / match start); flag after 300 s
  (+ time between S&D rounds) when the options match-type byte is 1 or 3; HUD warning at 180 s, fine text at 300 s; nothing is
  sent. `inactivity_fine.options`: only `give_fine_time_seconds` / `first_warning_time_seconds` are read by the exe; money, ban
  ladder, multiplier, divider are the server's. A relay proxy is possible (movement bits = `actions & 0xF` of the 11-byte input;
  count an input when the mask changes or a rotation word is non-zero): LIKELY a subset of the client's flagged players.
* Voluntary leave: CE is sent only by the menu "leave match"; the normal end sends none. `relay.rs` tells CE
  (`LeaveReason::Menu`) from a disconnect (`LeaveReason::Disconnected`).
* OPEN: score-add callers; idle input repeats; transport after a normal end; action-mask bits; respawn key alive gating; the
  auto-respawn default; round-mode respawns; stats event 0x28 name.
* PROVEN: an idle client sends NO inputs (the producer uses
  the simulation's change filter, 0x2001FD745; a dead player none at all; CD still every tick), so the relay's input count is
  a change count (inactivity source B = "no input for 300 s"); the normal end sends no CE and no FA (no FA sender on the match
  connection; `Event::Disconnected` never comes from a stock client); action bits: 0 forward, 1 back, 2 left, 3 right, 4 jump,
  5 fire, 6 reload, 23 use, quick command n = one-shot bit 31 + n; `request_respawn` has no alive check (a press while alive is
  sent); `set_player_autorespawn` defaults to off; stats event 0x28 = `victory_artefact_carry_kill` (the victim carried the
  victory artefact; NOT an inactivity flag); the in-match profile choice goes
  in CD `n_profile` (b0 = profile, b1 = 0), entry slot = profile index (0x2002E5AD3). Still OPEN: keepalives after the final
  DF (a two-client run); how a dead player returns at a new S&D / round-mode round (no wire message either way).

## Inactivity fine and leavers: server rules

* Inactivity fine: not built. What it needs (section above):
  1. source A (exact, needs the server-side world simulation): per player the stamp events of the client rule (damage dealt
     to an enemy, hit by an enemy after moving in the last 30 s, score gain, search-and-destroy zone checks, entering / match
     start); flag at 300 s since the last stamp (+ time between S&D rounds), only when the options match-type byte is 1 or 3;
  2. source B (relay proxy, buildable today): last input change per player from CD (`actions & 0xF` movement bits of the
     11-byte input, or a non-zero rotation word); "no change for 300 s" = inactive. A subset of the client's flagged players
     (someone moving without fighting escapes it);
  3. the fine itself: `inactivity_fine.options` money 1000, ban ladder, multiplier / divider (the exe reads only the two
     timers; the rest is the server's), applied like the leaver penalty (`online::penalize_leaver`, `game_data::fines`).
* Leavers: the relay tells the menu "leave match" (CE, `LeaveReason::Menu`) from a disconnect
  (`LeaveReason::Disconnected`; the normal end sends no CE). A menu leaver of a random or ranked match is fined and unpaid even
  when his client still reports a result (`EndedPlayer::left_by_menu`).
* The 97 place byte: `protocol::s2c::set_places` on online results; local results keep 1 (the reference
  generator's value; the screen takes bot rows from the client).

## Protocol audit

See `protocol-audit.md`: the DF confirmation carries the match clock at arrival, options +0x107A = 500 ms online, player record games played / face / premium, 85 states.

## World host memory

Each map is a zygote process (its matches are fork children that share its pages), and every zygote is itself a fork
of one **base process**, so the start-up that does not depend on the level is done once and shared by every map:

* **The base process (`daemon::base_loop`, `world-host --base-fd 3`, started and restarted by the front).** It runs
  steps 1-2 (CRT, static ctors, engine init, the 76 volumes, the config query) and step 3 up to the cut
  (`run::step3_base`: the world thread's FPU set-up, the render worlds and the null render init, the game object
  0x200107CA0, the cook registration 0x200118090, the null cooks, the items dictionary, startup.cfg). Nothing in it
  reads the level, and every query in it has finished when it returns. The cut is before the line that names the
  level: the Bullet hooks and the localization holder 0x20013DFA0 stay after it (that holder's load is still running
  when its call returns, so it must not be in flight at a fork). Per map the base fork()s (`run::fork_world`, kind
  Zygote); the child gets the command line a zygote started on its own would get (read through `cmdline`), its own
  output folder and log, restarts `:hdd` and `:resources manager` as a match child does (reporting the base's thread
  ids), loads the level, pre-warms and serves its matches. The overlay and private drive stay the base's (nothing is
  written there after the cut). Every map after the first therefore shares the base's start-up pages instead of
  holding its own copy (measured: the base holds about 124 MB at the cut, about 100 MB of which every zygote shares,
  and about 168 MB with the base pre-warm set below; what a zygote still holds alone shows as Private_Dirty in its
  `MEM [zygote ready]` line, the part shared with the base as Shared_Dirty). Measured map sizes (Private_Dirty at
  `zygote ready` without the base pre-warm set: the level and the pre-warm list): a usual PvP map 0.11-0.15 GB (its
  level alone 31-98 MB), a PvE mission 0.25-0.26 GB (185-192 MB), the largest, `level_11`'s three modes, 0.32-0.34 GB
  (281-283 MB; from one-shot runs of its level and list, not measured as a zygote). With the set a zygote holds about
  51 MB less (PvP 112 -> 61 MB, PvE 252 -> 200 MB). A match child: a recorded two-player PvP match 37 MB at its
  creation and 41-43 MB at its end, a PvE mission 92 -> 100 MB; a 12-slot PvP match with 11 of the host's AI players
  on a map started from the common seed 126-131 MB at creation and at most 137-140.5 MB, a PvE mission with the host's
  AI driving its 17 AI players at most 122 MB, an 18-minute PvE mission at most 106 MB (docs/guide-server.md, "Memory:
  how many maps and matches").
* **The front and the base.** The front asks the base for a zygote with a Load message (`base.rs`: the project and the
  zygote's command line, its control socket attached); the base answers with the zygote's pid and a pidfd. The front
  watches (`poll`) and kills (`pidfd_send_signal`) zygotes through their pidfds, so a reaped and reused pid is never
  hit; the base reaps them. A map that needs loading while no base is ready waits like a match waiting for room.
* **Checked.** Every served project (118: each PvP map, mode and variant and each PvE mission) loaded after a fork at
  the cut gives a byte-equal start world (`start_item.bin`) and the same static geometry as a cold load
  (`HOST_FORK_AT_CUT=1`); the regression recordings through the daemon give ticks and captures equal to the golden
  runs; the daemon's cases (zygote restarts and lists, eviction, a map over the limit, a third map that waits and is
  refused, idle eviction, a burst over `max_matches`, a stopped zygote killed after eviction, a killed base replaced
  while the old zygotes keep their matches, `prestart` through the base) behave as described.

Four host-side steps keep pages
that only hold zeros, or that nothing uses, out of the zygote's private memory. The first three change no byte the exe
reads; the fourth changes the render command pool's capacity and free list (render data only, never run by the host):

* **Zero pages of the image (`mem::drop_startup_zero_pages`, end of step 1).** The static ctor 0x200075B70 fills 27
  per-thread job-slot arrays (0x20411B0F0 + i x 0x400008, 108 MiB of BSS) with zeros (0x2000AA770, 0x2000AF590). At the
  end of step 1 only the world thread runs exe code (the first CreateThread is in step 2; the drop checks that no exe
  thread exists), so every resident page of the writable image that is all zero is given back with `MADV_DONTNEED`
  (the image is one private anonymous mapping; the drop checks `/proc/self/maps` and refuses anything else). A read
  gives zeros again, a write allocates the page again. Saving per zygote: up to 108 MiB, minus the arrays the threads
  fill later (a PvE zygote's step 1 gave back 117 MB of all-zero pages in all; the part its threads fill again is not
  measured on its own).
* **No zeroing of fresh heap memory (`mem::heap_alloc`).** A zeroed request (HEAP_ZERO_MEMORY, LMEM/GMEM_ZEROINIT, a
  zeroing realloc's tail) writes zeros only into a block reused from a free list; a new mapping or unused chunk space
  holds zeros already. Saving: about 24 MB per zygote (`prepare_process`: 8 MB network client stand-in + 16 MB
  player-record buffer).
* **Null-cook stand-ins in a pool, sized per class (`mem::pool_alloc`, `nullcook::null_res_size`).** An empty
  resource takes the size of its class's real object where the exe's own cook allocates a fixed size (texture 0x488,
  particle system 0x600, sounds 0x238 ...: allocation sites in the code), rounded up to 16, instead of a 5120-byte heap
  block. The classes with a variable-length tail get their fixed part: static render model instances (0x13, one per
  render visual of the level, the most frequent class) 0x3F0 (0x3E8 + 0x1A8 per surface in the exe), composite
  sounds (0x101, 0x238 + 0x40 per sound item) and sound collections (0x102, 0x240 + 0x20 per entry) 0x240, grass
  (0x79) 0x350. A real object of these classes can be larger (0x13 up to 0x8370 in the data); the tails are filled
  only by the real cooks, which never run in the host, and reached only through the object's own pointers and counts,
  which are 0 on a stand-in (PROVEN for the cooks; for every other reader by the guard-page run,
  `HOST_NULL_RES_GUARD=right` below, at these sizes or smaller: no access past them on any served project's level
  load and match creation or through the regression recordings). Saving for 0x13: about 3 KiB per render visual,
  about 22 MB in a PvE mission's zygote (7 190 visuals) and 3-6 MB in a PvP one; for 0x101 / 0x102 about 0.5-0.8 MB
  a map.
  The skeleton model instance (0x14) is 0x5D58 bytes in the exe, and exe render code writes past +0x1000 of it in a
  render job the host never runs: it gets its full size in whole pages (untouched pages cost no memory). The size the
  exe's memory statistics are told stays 0x1000. Saving: at least 1 KiB per null-cook answer (5 862 to 12 878 answers
  per zygote and match), about 3.5 KiB per particle system. Measured in a PvE zygote: 11 243 stand-ins in 14.5 MB of
  pool, 43 MB less than as heap blocks.
* **A smaller render command pool for the matches (`world::compact_render_pool`, in a loaded zygote).** The scene's
  command pool (`single_size_buffer_allocator<0x628>` at 0x2034C3F40, 0x2000 records = 12.3 MiB, every page written by
  the scene init 0x2005FD980) feeds the render thread, which the host does not have: its commands are retired unrun
  (the zygote's retire after its level load and pre-warm, a drain thread during each match creation, a retire after
  every step). The level load and the pre-warm keep the exe's capacity (their peak, about 7200 on PvE, stays under
  it): at a lower capacity the exe's full callback 0x2005FEC90 would free commands there, and its free 0x2000FA0D0
  takes no lock. Once the zygote has loaded and retired everything, every record is free: the free list is rebuilt over
  records 0..1087 in ascending order, the capacity goes down to 1024 and the pages after record 1087 are given back.
  The allocate 0x2005FEC30 checks the count, pops, then counts, so threads that allocate at the same instant each take
  at most one record past the capacity, and the list is last in, first out, so the highest record handed out is the
  most records out at once: 64 spare records cover every thread the exe has. A match's creation then copies at most
  1.6 MiB of the pool instead of up to 11 MiB (PvE), and the zygote gives back the pool pages its level load wrote
  (up to about 10.7 MiB; 10.9 MB in a measured PvE zygote). A producer at the capacity waits in the exe's full
  callback; its yield (SwitchToThread at 0x2005FECC0, Sleep(0) at 0x2005FECCC) retires the queued commands itself, so
  even a step that queues more than the capacity goes on.

Trace: every `MEM [...]` line is followed by `MEM [...] savings` (totals of the process; a fork child counts its
zygote's part too), and step 1 logs the pages given back (`zero-page drop ...`) and `MEM [after the zero-page drop (step 1)]`.
The pool logs `render pool: free list rebuilt over records 0..1087 (...); capacity 1024 of 8192; <n> pages after it
given back`, the creation drains `pool peak <n> of 1024` (before: of 8192, values up to 7190; not comparable), and a
producer that retired commands at the capacity `render pool: a producer at the capacity ... (first time)`.

In a match, the **history pool keeps 22 items instead of 47** (`world::cap_history_pool`, right after
`pvp_match_resource`, before the first item). The match core's history pool ([W+0x7DE8]) is sized in client mode for 47
items of 0x106268 bytes; `new_game_state_history_item` 0x2002F0D10 recycles the oldest item once `used` (+0x38) reaches
the capacity (+0x40), the only reader of that field (0x2002F0D4C). 22 is the exe's own server-mode count (0x2002E61AF),
and server mode (22) and client mode (47) gave equal ticks and captures on the recorded matches without late inputs.
What reads old items at a 50 ms tick: the host's resync DE answers `WantDe` with the newest final tick T = the newest
stepped DF - 300 ms (session.rs:173-174, `final_lag_ms` session.rs:90) and `run::de_with_tail` writes the items of
exact times from T - 500 ms (options +0x107A, run.rs:1807-1813) up to T: 17 items back from the newest (a missing one
would be written as a zero byte instead of its animation block); the exe's late-input restart, its 500 ms horizon, the
load backfill and lag compensation stay within 12 items. 22 keeps 5 to spare; for another tick, horizon or final lag
the capacity is the DE's items + 5 when that is more (`world::history_capacity`), never above the exe's. Items 23..47
are then never touched (only their free-list link page): about 25 x 0.17-0.29 MB = 4-7 MB less per match. Trace:
`history pool: capacity 22 items (the exe's 47); the resync DE reads 17 (tick 50 ms, final lag 300 ms, horizon 500 ms)` at the
creation, `history pool: U of C items in use at the end, the oldest of time T0, the present T` at the end (`child: ...`
in a match child). Diagnosis: `HOST_HISTORY_POOL=exe` keeps the exe's 47 (A/B runs). Checked through the daemon at
22 items: the equality suite (the six regression recordings, ticks and captures equal to the golden runs); the
late-input copy of a recorded match (`HOST_CAPTURE_DELAY=300`) with its captures 248 / 245 / 248 equal as before; a
resync DE (`HOST_DE_AT`) byte-equal to the one with `HOST_HISTORY_POOL=exe`; at the end 22 of 22 items in use,
spanning 1050 ms of world time.

**Late loads (`registry.rs`).** What a match cooks for itself is what its creation finds missing in its zygote; the
matches load nothing more after the creation: the trace below logged 0 late loads in every match it watched (the
regression recordings; 12-slot PvP with 11 of the host's AI players; a PvE mission with the host's AI driving its 17
AI players; an 18-minute PvE mission), and the one later
query the host makes itself, at the AI's start, finds its entry cached. Every match child keeps checking it, without
a breakpoint:

* **The exe's name cache.** One hash table of 0x8000 buckets at 0x200C71270 (bucket i's first node at +8 + i x 8),
  its entry count (u32) right after the last bucket at 0x200CB1278. A node holds the class (+0), its owner (+8), a
  pointer to the zero-terminated name (+0x10) and the next node of its bucket (+0x18). A resource of a class shared
  through the cache enters it where a lookup does not find its name (0x2000CB019, before it is cooked) and, once
  finished, as its own node with a copy of its name (0x2000CAC2D); every insert adds 1 to the count, every erase takes
  1 off. The exe holds the critical section 0x200CB1280 around every find, insert and erase (taken at 0x2000A7A90,
  0x2000C1EA0, 0x2000CABC0, 0x2000CAFC0), and nothing in the exe walks the table or reads the count: the table's address
  occurs only in those finds, inserts and erases, and the count only in insert and erase.
* **The trace.** The child walks the table once at the end of its creation query (the creation's entries), then reads
  the count (one u32, without the lock) before Ready, after the host's AI started and after every stepped DF. When the
  count differs from the last walk's, it walks the table under the exe's own lock and names every entry it has not
  seen; in play at most one walk per second of world time, and one more walk at the end whatever the count did. The
  lock is taken with the host's TryEnterCriticalSection and given back with its LeaveCriticalSection (the calls an exe
  thread makes; a free section's bytes are the same after them); a lock not free within 50 ms skips that walk (counted).
  A walk stops after 2^20 nodes and reads only node and name pointers in the image or the host's low area.
* **The list.** At the end the late entries of the classes a zygote can query by name alone join the project's own
  pre-warm list through the creation's union (`daemon::grow_prewarm_list`: appended, never shrinking, written only
  when something is new), so the project's next zygote, or its running zygote's top-up, holds them for every later
  match. Names are read whole (the creation's recorder cuts at 119 bytes).
* **Exactness.** Reads only: no exe byte is written, no breakpoint, no debug register, no signal. The lock is the only
  shared state the trace touches, through the same calls the exe makes, and it is free again after each walk; an exe
  thread that asks for it during a walk waits that long (time only). The trace's allocations are the host's own.
* **What it can miss:** an entry registered and erased again between two checks, and in play an insert and an erase
  between two walks that leave the count unchanged (the walk at the end finds the insert if it is still there).
* **Lines (match child):** `late loads: watching the name cache (N entries at the end of the creation query, K nodes;
  walk T us)`; per entry `late load: world time T, before Ready|at the bots' start|in play: class 0xC NAME` (the
  first 32, then `late loads: M more not listed`); at the end `pre-warm list FILE: late loads: E entries, A new (list
  now L)` when the list grew, and always `late loads: K after the creation (B before Ready, S at the bots' start, P in
  play); cache entries C0 at Ready, C1 at the end, N changes of the count, W walks; list: A new` (`not recorded (no
  own list)` for a project without a `prewarm_dir` list). A zygote logs `zygote: name cache [zygote ready]: N entries
  (K nodes), names S kB; walk T us`.

**Pre-warm top-up (`daemon::TopUp`).** A zygote reads its pre-warm list once, at start; its matches then grow the
project's list (what their creations missed, and late loads). A zygote that started from the common seed or from no
list therefore had every match cook those resources privately until the map was loaded again (measured on a
two-player PvP match: 70 MB at creation instead of 39 MB with the list that match itself had just recorded). Now the
zygote takes in what its list file gained:

* **When.** On an idle turn of its hand-over loop (no hand-over for a second, so no fork and no other exe code runs),
  when the list file's size or time changed since it read it, or the last top-up left entries over; not while a child
  forked in the last 5 s may still be in its creation (it may grow the list once more); not while MemAvailable is under
  1.2 GB (the rule for a match over `max_matches`; logged once per wait); at most one top-up per turn, of at most
  200 ms of queries (the rest on the next idle turn); at most 48 MB of the zygote's private memory in all its top-ups,
  then no more (logged once). Nothing for a project with a `prewarm` file set by hand (it records nothing).
* **What.** The entries of the project's own list file that this zygote has not queried yet (at start or by a top-up),
  in the file's order, each through the start's own query (`run::prewarm_entries`: the exe query and callback of
  `run::prewarm`, one reference kept each), with the fork gate's idle parking off meanwhile as during the start's
  pre-warm; then render commands are retired as after the start's pre-warm (none are expected), parking goes on again,
  and every later fork runs the gate and the census as every fork does.
* **Exactness** (the argument of `config.md` 1.1, at a later time): the entries are queried through the same exe query,
  with the project's own level loaded, before the fork of every match that finds them; an entry a match also asks for
  is cooked in the state an own list would cook it in, and one it never asks for only stays in the cache, which nothing
  in the exe walks ("Late loads" above). The order is the file's, and growth only appends: a zygote that loaded its own
  list (or the common seed written as its own list) has queried a prefix of the file, so after a top-up it has queried
  the same entries in the same order as a zygote started with the grown list (a zygote that loaded a variant's list
  queries the own file's other entries after it). Children forked before a top-up are other processes; between
  top-ups the zygote runs no exe code. A later clock is nothing new (every creation's cooks run at the child's later
  clock). An entry that faults when queried by name alone would end the zygote, but it is in the list file already:
  without the top-up it ends the project's next zygote at its start instead. What this does not prove is the premise
  of every pre-warm list: that a cache hit and a match's own cook give the match the same bytes (the equality runs).
* **Checked.** The equality suite with the top-up and with `HOST_PREWARM_TOPUP=0`: ticks and captures equal to the
  golden runs. A recorded two-player PvP match replayed twice on one zygote started from the common seed: the zygote
  topped up 723 entries in 196 ms between them, and the second match held 39 MB at its creation instead of 70 MB.
* **Lines (zygote):** `zygote: pre-warm top-up: follows FILE (N entries queried at start)` (or `none (no recorded list
  for this project)`); per top-up `zygote: pre-warm top-up: N new entries (K kept, E empty) in T ms; list now L (top-up
  n; the file has F[; R left for the next idle turn]); render commands retired C; Private_Dirty A -> B kB`, then
  `zygote: name cache [after top-up n]: ...`; `zygote: pre-warm top-up waits: MemAvailable M MB, under 1200 MB`,
  `zygote: pre-warm top-up stopped: the top-ups added X kB (TOPUP_MAX_MB 48)`. A match child's `child: the zygote's
  pre-warm list: ...` ends with `; topped up by N entries (K kept, E empty) in n top-up(s)` once its zygote topped up.

**Base pre-warm set (`baseset.rs`).** The entries common to the maps' pre-warm lists (player and weapon configs,
animations: about 710 entries are on every PvP list) are queried once in the base process, before the cut, so every
zygote shares their pages instead of holding its own copy; a zygote's own pre-warm skips them. Always on. Measured
with a set of 1413 entries (two lists): the base 121.5 -> 168.5 MB (once), a PvP zygote 112.2 -> 60.8 MB, a PvE
zygote 252.2 -> 200.3 MB, the match children unchanged (36.8 / 92.4 MB at creation).

* **The set (front, at every base start).** An entry joins when at least two of the served projects' own lists hold it
  (each list file once; a common seed is an own list) and, with `max_zygotes` set, its share of the lists is at least
  1.5 / `max_zygotes`: the set pays once about two loaded maps use an entry (15 lists: 12 needed at `max_zygotes` 2, 5
  at 5, 2 at 12 or more or without a cap). The entries of `base-exclude.tsv` (in `prewarm_dir`, else in `out`) never
  join. Written to the base's folder as `base-prewarm.tsv` and passed as `--base-prewarm FILE --base-exclude FILE`.
  No list, no common entry or `HOST_BASE_PREWARM=0`: no set, nothing changes.
* **The base.** At the end of `run::step3_base` (after startup.cfg, before the cut) it queries the set entry by entry
  through the zygote pre-warm's own query (`run::query_kept`, one reference kept each), and reads the exe's name cache
  ("Late loads" above) before the set, after every entry that changed the cache's count, and after the set: the cache
  entries the set added (their nodes) and the entries whose queries added each one. The set's cooks also put in what
  they ask for themselves (an animation's two baked forms, for instance), which no list names; the cache read sees
  them.
* **The check (zygote).** Moving the set before the level changes one thing the level load can see in the cache: an
  entry the level load asks for may already be there. Every found entry passes one place in the exe: the lookup
  0x2000CAF70 branches to 0x2000CB03D only when its find 0x2000C2A10 returned a node, and from there either attaches the
  request to that node's request in flight or hands out its finished resource; the find's other two callers (before an
  erase, and the completion path) serve no request. From the start of the level load until its own pre-warm starts
  (`prepare_process` included) the zygote holds an execution breakpoint there on every thread; the handler reads the
  found node and looks it up among the nodes the set added (host memory only). None found: the level load met exactly
  the cache state it meets without the set (a set node is either found or not, and none was), and the zygote goes on.
  One found, a breakpoint missing on a thread, or a base whose cache read failed: the zygote serves no match. It adds
  the set entries behind the found node to `base-exclude.tsv` (the union under its lock, as the lists) and runs its own
  command line again as a new program (same process and pid, the control socket kept, output in `<zygote folder>/own`),
  so a hand-over already queued is served by a zygote that loaded everything itself, as before the set. The front
  replaces the base once the exclusion file changed (as at BASE_MAX_AGE: no request in flight; old zygotes go on), so
  the set shrinks by the entries a level load asks for; an empty set is the behaviour without it. The exclusion file
  starts empty: on the recorded data two config entries of the lists are asked for by level loads (one by the level
  loads of 4 projects, one by 116; the second joins the set only from `max_zygotes` 5, by the share rule), and the
  first zygote that finds one excludes it this way, at the cost of one more level load.
* **Exactness.** The check covers the cache (a found set entry is never served to a level load that is kept). The rest
  of what moves when the set is loaded before the level (addresses in the resource arenas, the order of their free
  blocks, a later clock) was compared on every served project with a one-shot run that queries 709 common entries right
  before the level project: the start world (`start_item.bin`) and the static geometry count are equal on 118 of 118
  projects. The level part's Private_Dirty is 18-24 MB smaller there; a diagnosis run with and without the entries
  first showed the level load's requests identical, no name shared with the set and differences only in entries named
  by address, so that drop is the level load reusing memory the set's cooks freed, not cache hits (the end of each run,
  holding more resources, is 5-19 MB smaller too). In a zygote those freed blocks are the base's pages, which the level
  load copies as it writes them, so the saving per zygote is the set's own cost, not more (measured above: about 51 MB
  a zygote). A base that ends before Ready with a set (an entry that cannot be queried before the level) makes the
  front start every later base without one.
* **Lists.** A match on such a zygote finds the set's entries cached, so its creation never shows what their cooks ask
  for; the recorder adds the cache entries the set added through the entries the creation asked for, of the list
  classes (`baseset::with_closure`), as a cold creation records them, so the lists and the set do not shrink.
* **Lines.** Front: `front: base pre-warm set: N entries (in at least K of L lists (max_zygotes Z)[, X left out by
  FILE]) -> FILE` or `front: base pre-warm set: none (...)`; on a change `front: base pid P: the base pre-warm set's
  exclusion file FILE changed, ...: replaced`. Base: `base pre-warm: N entries (K kept, E empty) in T ms from FILE; the
  set added A name-cache entries to the B before it (W walks)`, `base pre-warm: empty entries (first n): ...` when E >
  0, `MEM [base: after the base pre-warm]`. Zygote: `zygote: level-load check: F lookups found a cached entry, none
  of the A the base set added (T ms, n threads)`, or `zygote: level-load check: H of F lookups found entries the base
  set added`, one `zygote: level-load check: found CLASS NAME (added by ...)` per node, `zygote: base set exclusion
  FILE: ...` and `zygote: ...: it serves no match and starts again on its own, without the base set (log
  .../own/host.log)`; `prewarm list FILE: K resources kept, E empty, in T ms (S more in the base pre-warm set, not
  queried here)` and `zygote: pre-warm list: own, N entries (K kept, E empty; S in the base, M loaded here) from FILE;
  base pre-warm set B entries`. Match child: `pre-warm list FILE: N entries the base set's cooks asked for join this
  creation's M`.
* **Checked.** The equality suite through the daemon (the six regression recordings, ticks and captures) with the set
  and with `HOST_BASE_PREWARM=0`: equal to the golden runs in both. Every zygote's level-load check found none of the
  set's entries (3888 found lookups on a PvP level load, 9648 on a PvE one). Memory: the numbers above.

Diagnosis switches for the memory work (environment variables of the world host, inherited by the base process,
its zygotes and every match; off by default, and without them nothing is recorded and nothing changes):

* `HOST_NO_COMMON_SEED=1`: projects without a list start without the common seed (`config.md` 1.1), for the A/B proof
  of the seed's ticks and memory.

* `HOST_LATE_TRACE=0`: the name cache is never read (no late-load trace, no `zygote: name cache` line; a match child
  logs `late loads: not watched (diagnosis HOST_LATE_TRACE=0)`), for the A/B proof that the trace changes no tick.
* `HOST_PREWARM_TOPUP=0`: zygotes never top up their pre-warm list (`zygote: pre-warm top-up off (diagnosis
  HOST_PREWARM_TOPUP=0)`), for the A/B proof of the top-up's ticks and memory.
* `HOST_BASE_PREWARM=0`: no base pre-warm set (`front: base pre-warm set: none (diagnosis HOST_BASE_PREWARM=0)`; a
  base given one anyway logs `base pre-warm: off (...)`): every zygote queries its whole list itself, for the A/B proof
  of the set's ticks and memory.
* `HOST_PRELEVEL_LIST=FILE`: the pre-warm list FILE (`0xCLASS<TAB>name` lines) is queried right before the level
  project query (`run::step3_level`) instead of only after the level, in a one-shot run or a zygote. It answers whether
  a level load that finds resources already in the cache gives the same start world (`start_item.bin`, the static
  geometry line) and how much less it holds: the question a shared base set or a per-level layer depends on. Lines:
  `diagnosis: HOST_PRELEVEL_LIST=FILE: ...`, `prewarm list FILE: K resources kept, E empty, in T ms`, `MEM [after the
  prewarm list]`, then the level as usual (`MEM [after project query (step 3)]`). Run on every served project with
  three lists (the 709 entries common to the PvP lists, the 3033 in two or more lists, and per level the entries its
  level loads ask for): the start world and the static geometry equal to the cold load on 118 of 118 projects each.
* `HOST_PAGEMAP=1`: `PAGEMAP [label] ...` lines at the base's cut, at `zygote ready`, at a match child's Ready and at
  its end (and the one-shot run's level load and fork children): where the process's private pages are, read from
  `/proc/self/pagemap`. A page is private when it is present, anonymous and exclusively mapped (bit 56): a page a match
  still shares with its zygote, the shared zero page and a page merged by KSM do not count, a page it copied on write
  or allocated does. Linux shows bit 56 to an unprivileged process for its own pages (only the frame numbers need
  CAP_SYS_ADMIN); every report checks it on a page of its own first and, when it does not show, counts every present
  anonymous page and says so. Regions: each per-thread job-slot array and its job deque by thread index, the render
  command pool, the match core buffer, the CRT allocator arena, the rest of .data/BSS in 1 MiB buckets, the null-cook
  pool by class, the host heap, the host's low stacks, the exe's VirtualAlloc block split by the engine's arena table
  (the offsets of its start-up lines `memory stats for arena [...]`), the host binary, libraries and anonymous memory,
  then the 64 KiB ranges with the most private pages.
* `HOST_NULL_RES_GUARD=right[,CLASS=SIZE...]` / `scan`: checks that no exe code reaches an empty resource past a size.
  `right` puts every null-cook stand-in at the end of a slot of its own with an inaccessible page right after it and a
  0xA5 fill before it, sized 0x3F0 for class 0x13, one page for 0x14, 0x310 for 0x79, 0x240 for 0x101 and 0x102, its
  normal slot for every other class, or the size the switch names (hex, at least 0x220). An access past the size
  faults at once, and the fault line `null guard fault: class C stand-in A (size S) accessed at +O, from exe address X`
  names it before the usual fault report; at the points above `null guard [label]: right: ...` counts the guarded
  stand-ins per class and the fill bytes that changed. Two mappings per stand-in: a PvE zygote and its match stay
  under the kernel's default limit of 65530 mappings. `scan` keeps the normal layout and logs per class the highest
  non-zero byte of its stand-ins (writes only).

Test tools for these runs: `world-host projects` (with `HOST_DATA_DIR`) lists every project a daemon serves from the
game data with the one-shot arguments that load it (`--match MAP,MODE,VARIATION` for PvP, `HOST_PVE=D:M` for PvE; a
one-shot `--match` takes the variation as a third field), and `world-host room-status --socket S [--every-ms N]
[--count K]` prints the front's room status (7.3a) as the lobby reads it.

Base and zygote trace: the base's own `host.log` (`<out>/base/b<ms>/`) holds steps 1-2 and the null render init with
their `MEM` lines, then `MEM [base: at the cut]`, `base: ready, pid B; cut after apply_startup_cfg at T s` and per map
`base: zygote for P requested`, `fork census: k of n timer thread(s) in their idle wait after U us`, `base: zygote P
-> pid Z (pidfd sent) after M ms` (or `failed: ...`) and `base: zygote P pid Z exited, status S`. A zygote's `host.log`
starts at its fork: `child pid Z after fork; reporting world thread id W`, `fork gate: kept on in the zygote; N stale
poller record(s) of the parent cleared`, the two `restart: exe thread ...` lines, `zygote L: forked from base pid B (log
...) at its uptime U s`, `zygote: arguments: ...`, `MEM [zygote: right after fork]`, then the level lines as before and
`zygote: level L loaded after the fork in N ms (static geometry G, pre-warm ..., render commands retired R)`. The front
logs `front: base started / ready after T s / exited`, `front: zygote for P (...) started through the base, pid Z, in N
ms` and `front: zygote P (pid Z) exited`.

Fork safety: before every fork two waits run, each for at most 250 ms: the fork gate waits until every worker poller
seen so far (a thread that has called the worker loop's Sleep at least once) is parked in it, and the fork census waits
until every exe timer-loop thread seen so far (`:hdd`, `:resources manager`: a thread that has entered its idle wait,
WaitForMultipleObjects returning to 0x2000AEC42, at least once) is inside that wait. The handle table is then held across
fork(); a timer thread needs it to leave its wait. The host allocator's page pool, heap and region locks are held
across fork() as well. What this guarantees:

* **A zygote** (forked by the base) is forked only when both waits completed: no thread seen by the gate or the census
  runs exe code at the fork instant. After a TIMEOUT of either the base does not fork; it answers `Busy` (`base: zygote
  P refused for now: ...`), and the front lets the match wait and asks again (up to the base wait, 30 s).
* **A match child** is forked also after a TIMEOUT (logged `TIMEOUT`): that fork may copy a thread in the middle of exe
  code, and the one match can then hang (its backstop ends it).
* Not covered either way: an exe thread that has never reached the gate's Sleep or its idle wait (none besides the
  three named ones exists in the recorded logs), and exe work on such a thread.

Clock: a zygote inherits the base's monotonic clock, so it loads its level at the base's uptime instead of a few seconds
after start. The front therefore replaces a ready base older than 6 hours when no request is in flight (`front: base pid
B is ... h old (BASE_MAX_AGE 21600 s) ...: replaced`); its zygotes go on, the old base ends on its closed control
socket, and new maps fork from the new base. `HOST_BASE_MAX_AGE_S=<s>` (diagnosis, at least 60) sets that age for a
run, so a replacement can be seen without waiting 6 hours. `HOST_CLOCK_OFFSET_S=<s>` (diagnosis) starts a
one-shot run's clock that far ahead, to compare a level load and a match at a large clock with a normal run.
`HOST_FORK_AT_CUT=1` (diagnosis) makes a one-shot run fork once at the cut and load the level in the child, to compare
with a cold load without a daemon.

## World host CPU

* **Start-up pump.** Step 2 delivers the queued callbacks (among them the volume mount) by pumping the dispatch
  0x2000BCF60 until every `resources.<n>.db` of the game folder has had its FAT signature checked, then 250 ms more,
  at most `--pump-ms` (3000): about 1.45 s less than the fixed 3 s in the recorded starts. If the config check after it
  fails, it pumps the rest of the 3 s and checks again (logged).
* **Map load.** The base process runs steps 1-2 and step 3 up to the cut once; a map's zygote only forks and loads its
  level and pre-warm list (about 3 s less per map than a zygote that starts on its own). Measured on a test PC: the
  base is ready about 2.8 s after its start, a zygote's fork takes 1-2 ms, and its level with its pre-warm list is
  loaded about 1.0 s (PvP) or 3.1 s (PvE) after the fork (`zygote: level ... loaded after the fork in N ms`).
* **Idle zygote.** The engine's worker loop dispatches before every Sleep(1) once its dispatch byte is set, which in
  0.69d is for good (0x20009117B): about 935 dispatches a second while a zygote waits. Between hand-overs a poller with
  no job and no lock parks in the fork gate for 25 ms per Sleep (a deadline: other pollers' notifications do not end
  it), so its loop still dispatches, at most 25 ms late, about 40 times a second. Fork children poll as before. That
  poller is `:sound`, which only the base process has: a zygote restarts `:hdd` and `:resources manager` after its
  fork, not `:sound` (it never gets a job in a zygote), so a zygote has no worker poller. Measured: an idle zygote
  uses about 0.1 % of a core, the idle base about 0.2 %.
* **No transparent huge pages** (`mem::disable_huge_pages`, at process start, inherited by every fork): the image, the
  exe's 3 GB reserved arena and the low mappings are touched sparsely, and under the THP mode "always" a touched 4 KB
  page could be backed by a whole 2 MB page. Logged as `transparent huge pages: off for this process (system mode:
  ...)`; under "madvise" (Ubuntu's default) nothing changes.
* **Same-page merging (KSM)** (`mem::enable_memory_merge`, at process start): `PR_SET_MEMORY_MERGE` (kernel 6.4+)
  marks every anonymous mapping of the process and its forks; on an older kernel the exe's own mappings get
  `MADV_MERGEABLE` (the image, VirtualAlloc regions, low mappings; a decommitted range again). The kernel's ksmd then
  merges identical pages of the base, the zygotes and the matches into one copy and copies it again on the first
  write, so no byte any process reads changes. It merges only while the operator has `/sys/kernel/mm/ksm/run` at 1
  (guide-server.md), and costs ksmd's CPU. Logged as `memory merging (KSM): ...; system switch ... = N`. Huge pages are
  off (above), which KSM needs: it does not merge a 2 MB page. Measured with KSM on (`pages_sharing`): about 27 MB
  for the base and two idle zygotes, about 37 MB with one PvP match running, about 185 MB with five PvE matches of one
  mission (about 30-40 MB a match). ksmd's CPU is not measured.
* **Import calls.** Each import thunk decides once per slot whether its import is in the `--escape-at` / `--end-at`
  lists (before: two formatted strings and two copies of the thread name per call), and the thread id the
  critical-section and thread-id stand-ins ask for is read once per thread (the fork child's thread reads its own).
