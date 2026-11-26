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

* DF `code` values; late join (not needed). E0 and the resync answers to a CC run only with the world host
  (`netcode.md`: the host's hashes and a DE of the running world), and only while resync is on; without the host there
  is no E0 and a CC is answered with the cached DE, which a running client ignores (no repair).
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
  (`local_match::online_options_row`); the world host builds that project.

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
  [0, 0]", PROVEN; level_03_day_sad_v2 LIKELY the same), a bot without a graph is undefined in the exe and the host refuses the
  match. The client offers no S&D bots training either. So S&D gets no bot fill and no room bots (`online::NO_BOT_MODES`): an
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
