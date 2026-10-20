# srp-026g: the server for client 0.26g

> **Version scope: everything on this page is for client 0.26g** (build 2777,
> build string "0.26g0", January 2015, no PDB). The 0.21d server (hosted on
> skhid.link until 2026-10-12, stopped since) is in SRP-021D.md. No 0.21d message is wire-compatible with 0.26g:
> every message id changed and the lobby moved from TCP to UDP.

> **Status, 2026-10-12. Live on 026g.skhid.link**, and since that evening on
> skhid.link and www.skhid.link too (the same server, on a host of its own; a
> test server is test.skhid.link, `srp-026g/deploy/test-server/README.md`). A
> real client first signed in that morning, and matches
> between real players have run since 08:13 UTC. What they showed and what was
> fixed is in `srp-026g/WORK.md` (newest at the top) and OPEN-ISSUES.md. Until
> that day everything was from disassembly, the game's data, scripted clients
> and tests, and much of it still is: where a line below says LIKELY, or "not
> seen with a real client", it means that. A local test with one game and a
> bot: "Run locally" below.

`srp-026g` began as a copy of `srp-021d`; the 0.26g code was added beside the
0.21d code. The work order is
PLAN-026G.md; the client facts are in CLIENT-FACTS-026G.md; how a match
stays in step is in MATCH-SYNC-026G.md; the research is in
`srp-026g/docs/notes/` (start with its `README.md`).

## What it is

One process, `srp` (`cargo run -p srp`), runs the whole 0.26g stack and
shares one account store between its servers:

```
survarium_srp.exe -server=<host>:1234
      |
      1. login   TCP 1234   the sign-in in the clear, then TLS: the password as a SHA-1
      |                     digest. The answer: a session id and the lobby's IPv4:port
      2. lobby   UDP 1235   one reliable-UDP link a client: account, inventory, shop,
      |                     skills, quests, chat, friends, squads, rooms, results
      3. match   UDP 1236   lockstep: the server relays inputs and compares world
                            hashes; the world itself comes from a patched client
      web page   HTTP 8088  home, download, the launcher's page, the admin page
```

0.26g has no browser server (port 80) and no presence pings. The 0.21d
servers are still in the tree, compile and are tested, but `srp` does not
start them: the two login servers share one port.

## The parts

| Crate / module | Does |
|---|---|
| `crates/udp-transport` | The 0.26g reliable UDP transport, shared by the lobby and the match server: the per-message delivery table, split messages, ordered and unique channels, the packet log (`SRP_PACKET_LOG`) |
| `crates/survarium/src/v026g/` | Game data and wire formats. Data from the game's own configs, read at start from the server's game data of the release the players run (`SRP_GAME_DATA`; `game_data`, `tables`): `dictionary`, `item_params`, `prices`, `skills`, `maps`, `quests`, `drops`, `loadouts`, `modifications`, `shop_offers`, `awards`, `lzma2`. `lobby/`: writers for every server message, `requests::parse` for every client message, and the strict `client_model` (reads as the client does; what a client would drop or crash on is a fault). `match_msgs/`: the match messages, the snapshot reader and writer, its client model |
| `crates/session` | Shared with 0.21d: accounts by user name, argon2id passwords, sessions, the matches' seats and 0.21d's queue (`matchmaking.rs`), squads, bans, limits, `srp.db` |
| `crates/session/src/v026g/` | The 0.26g account (`account`: money, gold, premium time, experience and level, skills, reputations, three kits, inventory, quests), the lobby's actions (`rules`), `quests`, the account as messages (`wire`), match rewards (`rewards`), and its tables in the same `srp.db` (`persist`: `g26_*`, versioned on their own in `g26_meta`). `GameAccounts` is the store. `clans`, `extras`, `premium`, `bonus`, `items_rules`, `pay` (the match pay rule), `matchmaking` (a player's level, the map choice) |
| `servers/login-server/src/v026g.rs` | The TLS sign-in. First sign-in of a name registers it. Bans, limits, closed registration, the game's own status codes |
| `servers/lobby-server/src/v026g/` | The lobby: `core` (links, sign-in, the tick), `dispatch` (every request answered or refused), `play` (Play, a match made through `MatchHost`), `rooms` (the players' "servers": every PvP match), `social` (chat, friends, squads), `Inbox` (what other servers tell it: a match is over, an account changed, a push, the operator's text) |
| `servers/match-server/src/v026g/` | The match server: `registry` (the lobby makes a match here), `game` (sync, relay, hashes, resync, the end), `server` (socket and loop) |
| `servers/srp/src/v026g.rs` | Joins them: `Stack` (the three servers and the session collector, each on a supervised thread), `RegistryHost` (lobby → match server), `Settler` (match result → rewards → database → lobby), `WebLink` (web page → lobby) |
| `servers/web-server` | The web page. `run_026g` is the 0.26g page (`src/v026g.rs`); `run` is the 0.21d page, kept and tested, not started |
| `scripts/client-patch-026g/` | `patch_026g.py` (the client patch), `selftest_026g.py`, `make_client_026g.py` (the players' overlay) |
| `crates/resources-db` | Reads `resources.db` and its patches (`resources.db.patch_<n>`), lays them over one another as the game mounts them, and `srp-game-data` builds the server's game data from them: the `gameplay/` files, the English and Ukrainian localizations and each project's `server_objects`, with what it was made of (`resources.db`, each patch with its URL). `srp` reads it at start (`SRP_GAME_DATA`). `crates/resources-db/README.md` |
| `resources/game-026g/` | The server's game data of tests and tools: 0.26g's `resources.db` with the test patches 14 to 16 |

## How a player gets into a match

1. Login: the user name and build in the clear, then TLS, then the password
   message (a SHA-1 digest as 40 hex characters; with a typed password the
   password too). srp keeps only the argon2id hash of the digest. The answer
   is the session and the lobby's address.
2. Lobby sign-in (`2E` with the session). `0x80` goes alone; once it is
   acked the rest of the account follows, unasked.
3. Play (since 2026-10-17 every PvP match is a room's: the owner's; the
   queue, the "matches now" list and its JOIN went): Play puts the player
   into a room, the players' "servers" (`servers/lobby-server/src/v026g/
   rooms.rs`, `lobby.md` §2.16.3): of the open rooms that let it in the one
   nearest its level, however far (then the fullest), else a new one (8 v 8, any max level, 30 s). Once a room has its players their vote runs for 15 s between
   three maps of the room's pool, then the room's own wait; its members
   free in the lobby are put on two sides made to count alike (a player's
   gear, level and balance rating; with an odd count the stronger players
   are the side of fewer: `matchmaking.md` §8), squads together. Who comes
   while the match is played sees the room first and goes into it 15 s
   later (or at once with Join match) until 30 s are left, on the side
   with fewer players; of two alike in that, the side it played on before
   in that match; then the one that counts for less (the lead counts
   too). After the match everyone is in the room again for the next vote; a player leaves by
   itself, or when its game is gone. A player's level is the client's own
   "equipment level" (its highest kit, moved up by its rating); the
   match's level (the reward multipliers) is the mean of its players'.
   Only a game of patch revision 5 or later plays in rooms. A clan team's
   match is drawn uniformly from `SRP_MATCH_MODES` / `SRP_MATCH_MAPS`.
4. The lobby makes the match at the match server and sends its address
   (`0x82`) once "match found" (`0x85`) is acked.
5. The match: MATCH-SYNC-026G.md. **Only a client with the snapshot-upload
   patch can start one**: the world stays frozen until a client uploads it.
6. The end: the match server's result is settled
   (`session::v026g::rewards`), written, and the lobby shows the results
   screen (`0x97`) when the client is back.

## Settings (`SRP_*` environment variables)

Every setting has a default; none is needed on a dev PC. Taken from the code
on 2026-10-12 (grep `SRP_` in `crates` and `servers`).

### Addresses (`crates/vostok/src/config.rs`)

| Variable | Default | What |
|---|---|---|
| `SRP_BIND_HOST` | `127.0.0.1` | The interface every server listens on (`0.0.0.0`: all) |
| `SRP_PUBLIC_HOST` | `127.0.0.1` | The host clients are told. For 0.26g an IPv4 address, or a name with one (looked up once, at start) |
| `SRP_LOGIN_BIND`, `SRP_LOBBY_BIND`, `SRP_MATCH_BIND` | `SRP_BIND_HOST` | One server's interface |
| `SRP_LOGIN_PORT` | `1234` | Login, TCP. What `-server=` names |
| `SRP_LOBBY_PORT` | `1235` | Lobby, UDP. Bound and told to the client: the same number |
| `SRP_MATCH_PORT` | `1236` | Match server, UDP. Bound and told: the same number |
| `SRP_LOGIN_PUBLIC_HOST`, `SRP_LOBBY_PUBLIC_HOST`, `SRP_MATCH_PUBLIC_HOST` | `SRP_PUBLIC_HOST` | One server's public host |
| `SRP_LOCAL_PEERS` | on | A client on this machine or its network is told the address it reached srp at. `off`: everyone is told the public host (a container) |
| `SRP_BROWSER_PORT`, `SRP_LOGIN_UDP_BIND`, `SRP_LOGIN_UDP_PORT` | | Still read by the shared config; the 0.26g stack starts neither a browser server nor presence pings |

A port that is not a number stops srp at start.

### Game data

srp takes every table of game data from the archives the players' game
mounts, once, before anything else (`crates/survarium/src/v026g/game_data.rs`):
the stock `resources.db` and every `resources.db.patch_<n>` over it, n
ascending, a later file over an earlier one (client patch revision 12). It
reads them from the server's game data (`server-game-data.db`), which
`srp-game-data build` makes of exactly those archives (`crates/resources-db`,
`server`): the files srp reads (every `gameplay/` file, the English and
Ukrainian localizations, and each project's `client_project` cut to its
`server_objects`), each checked against its archive's file table, and
`srp/provenance`, what it was made of: `resources.db`, and each patch with
its sha256 and the URL the players download it from. From its `gameplay/`
files srp derives the dictionary, every item's parameters and the slots it
covers, what it may be modified with, the game modes and every map: which
ids can be played (a mode srp plays, its config and its project: no level is
known to the code, so a map a patch adds is played on the data that has it),
each project's respawn points and whose its battery containers are, and the
names the web page gives maps and modes (the localizations), the player's carry
limits, and the modification, craft, reroll, drop, quest, skill, award and
test-profile files (`tables.rs` checks them against each other). So it
cannot know an item, a map, or a value of one, the players' game does not.
What the game's files do not say stays in code: the shop's exceptions
(`prices.rs`), the bundles, and the maps offered against bots (`pvb.rs`
`MAPS`, the maps the bots' map files exist for).

| Variable | Default | What |
|---|---|---|
| `SRP_GAME_DATA` | unset (the compose file: `/srv/srp/download/server-game-data.db`) | The server's game data. **Required**: without it, or with a file that is missing, damaged or not made by `srp-game-data build`, srp does not start and says why; nor when it lacks an item, modification or quest an account of `srp.db` (or a kept results screen) names, or an item the code names (`session::v026g::known`, `survarium::v026g::tables::named_items`): srp never throws an item away for that, the data was made without a patch |

The web page lists the data's patches for the players' updater at `GET
/patches` (`servers/web-server/src/patches.rs`), in the game's order:

```json
{"patch_14":{"sha256":"<64 hex>","urls":["http://203.0.113.5/patches/resources.db.patch_14","https://mirror.example/p14"]}, ...}
```

each patch with its URLs, 1 to 8, in the order the updater tries them (the
patches server, then mirrors; `release/patches.tsv`). The updater reads that
form from 0.3.0 on (and the one-`url` form of the updaters before it); a
release that brings 0.3.0 has the older updaters install it before their
patch step, so no player's updater reads a form it does not know.
`{}` when the data was made of `resources.db` alone. The updater keeps
exactly those files in the game folder (`client/updater/README.md`,
"Patches"), so the players run the archives the server runs on. The page does
not serve the patch files: they are at their URLs. The data is read once: a
release replaces `server-game-data.db` and restarts srp (`DEPLOY.md`, "The
game's files"). Tests and tools read `resources/game-026g/server-game-data.db`
(0.26g's archive with the test patches 14 to 16; its README says how it is
made).

An item whose `data.additional_model_slots` names other armour slots is
worn alone in them (`crates/session/src/v026g/rules.rs`): putting it on
takes their items off, putting anything into one of them takes it off. The
Duty exoskeleton, worn in the back slot, covers the six others; 0.26g's
helmets with a built-in mask cover the mask's slot. A kit saved against
that rule is mended when the accounts load, and never plays.

### Data and logs

| Variable | Default | What |
|---|---|---|
| `SRP_DATA_DIR` | `./data` | `srp.db` (accounts, sessions, and the 0.26g `g26_*` tables; since 2026-10-12 also the finished matches' results screens not shown yet and each account's play period, `g26_results`), `backups/` (a copy at every start) |
| `SRP_DB` | on | `off`: everything in memory only |
| `SRP_LOG_FILES` | on | `off`: no `logs/<server>.log`, stdout only |
| `SRP_LOG_DIR` | unset | A folder: every line srp prints, in one file per UTC day (`srp-YYYY-MM-DD.log`), added to across restarts. The compose file sets `/srv/srp/data/logs` (the data volume): a container's own output is gone at every update |
| `SRP_LOG_KEEP_DAYS` | 14 | Files of `SRP_LOG_DIR` older than this are deleted (at least 1) |
| `RUST_LOG` | `info` | The log level |
| `SRP_SNAPSHOT_DIR` | `logs/snapshots` | Where the match server keeps accepted worlds; `off`: nowhere |
| `SRP_RECORD_DIR` | unset (off; the compose file sets `/srv/srp/data/recordings`) | Match recordings: every datagram of a match in and out (no addresses), one LZMA2-packed file per match (`servers/match-server/src/v026g/record.rs` has the format); read with `srp-recording` |
| `SRP_RECORD_MIN_PLAYERS` | `6` | A recording is kept once this many players (test bots not counted) have entered its match, late joiners included; the others are deleted when their match ends |
| `SRP_RECORD_MAX_MB` | `5000` | Kept recordings over this together: the oldest are deleted |
| `SRP_REPLAY_DIR` | unset (off; the compose file sets `/srv/srp/data/replays`) | "Watch replay" (`docs/notes/replay_watch.md`): a kept recording is made into the game's replay file here (`<match id>.sur` and its sidecar `.json`) when a player first asks for it (never at a match's end; one at a time, 20 waiting at most, at nice 19 on Linux), kept for later asks, served only at `/replays/<token>`. Must be a folder of its own: not `SRP_RECORD_DIR`, inside it or around it (then replays are off, said in the log). Off without `SRP_RECORD_DIR` or `SRP_GAME_MODES_DIR` (the lobby answers `off`) |
| `SRP_GAME_MODES_DIR` | unset (off; the compose file sets `/srv/srp/data/game_modes`) | The game's `gameplay/game_modes/*.options` files, every patch's included (game data: never in the repository): copy them into the data volume's folder. `off` or empty in `.env`: replays off |
| `SRP_REPLAY_MAX_RECORDING_MB` | `24` | A larger recording is not converted (status `failed`, not tried again until a restart): the conversion's memory grows with it (123 MiB for 13.5 MB, `replay_watch.md`) |
| `SRP_REPLAY_MAX_MB` | `1000` | Replays (`.sur` and `.json` in `SRP_REPLAY_DIR`, outside `SRP_RECORD_MAX_MB`) over this together: the oldest are deleted after each conversion and at start (the one just made stays); a deleted one is made again when asked for |
| `SRP_PACKET_LOG` | unset | A folder: every datagram of the lobby and the match server, one file a connection (the lobby sign-in's session id left out). **Set it for every live test** |

### Sign-in

| Variable | Default | What |
|---|---|---|
| `SRP_REGISTRATION` | open | `closed`: new names are refused; the admin page can switch it while srp runs |
| `SRP_SIGN_INS_PER_MINUTE` | 20 | Password checks an address may ask for a minute |
| `SRP_REGISTRATIONS_PER_DAY` | 3 | New accounts an address may make a day |

### Lobby

| Variable | Default | What |
|---|---|---|
| `SRP_BALANCE_PLAYER` | 12 | What a player counts for by being there when the sides are made, in gear levels: with an odd count the side with a player more has this much more, so the stronger players are the side of fewer. 0: only the strengths are made even (`matchmaking.md` §8) |
| `SRP_BALANCE_PLAYER_LEVEL` | 1.5 | What a level of the account counts for, in gear levels |
| `SRP_BALANCE_RATING` | 4 | What a point of the balance rating counts for, fully trusted, in gear levels |
| `SRP_BALANCE_TRUST_MATCHES` | 5 | The matches after which a balance rating counts for half; one of no match counts for nothing |
| `SRP_BALANCE_RATED_PLAYERS` | 4 | The fewest players a match must have had in it to move the balance rating, 1 to 20: the players who were in it for half of the longest stay or more. With 4, one against one and two against one move none; 1: every match does (`matchmaking.md` §8.3). The ranking's rating and the pay do not depend on it |
| `SRP_BALANCE_BATTERY_LEAD` | 0.55 | In a match being played (modes 2, 3): what a battery of lead counts for, in players, for the side that leads. Used where a player who comes into a room while its match is played goes, and in the log's prediction (`matchmaking.md` §9) |
| `SRP_BALANCE_TICKET_LEAD` | 0.2 | The same for a ticket of lead in a Team Deathmatch |
| `SRP_SQUADS` | on | `off`: nobody is invited to a squad or added to one; the one who invites is told in a message box. Clan teams are not squads |
| `SRP_SQUADS_OFF_TEXT` | `Squads are switched off on this server.` | What that message box shows: a text key of the game or a sentence of up to 255 bytes, shown as it is (the game has no text of its own for this) |
| `SRP_MIN_PATCH_REVISION` | none | A game whose exe says a patch revision under this one is refused at its sign-in with the game's own "You're using an outdated version of the game." (login answer 0x22), and a lobby sign-in with a session it had before the restart is refused and the session ended, so a game that reconnects goes to its login screen within about half a minute and is refused there. It never gets the lobby's sign-in, which names items (the price list) its game may lack (`patch_check.rs`). The test bots are never refused. Left out or 0: every game is let in; not a number from 0 to 255 stops srp at start, and so does a number under the newest patch of the server's game data (`resources.db.patch_<n>` is released with revision n: an older game lacks its items), with a line that says what to set |
| `SRP_MATCH_MODES` | `2,3,4` | The modes played |
| `SRP_MATCH_MAPS` | every playable map of those modes | `maps_dict` ids, e.g. `117` (Team Deathmatch on level_01) for a first test: a room's pool by default, and what a clan team's match is drawn from |
| `SRP_ROOMS_MAX` | 50 | The most rooms at once, 1 to 1000 |
| `SRP_ROOMS_EXTRA_MAPS` | none | Playable `maps_dict` ids a room's maker may add to its pool besides `SRP_MATCH_MAPS` (the experimental maps) |
| `SRP_LOBBY_LINK_TIMEOUT_SECONDS` | 30 | A client silent this long loses its lobby link: offline for its friends (its room keeps its place while its session waits) |
| `SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS` | 120 | The same for a player in a match that runs (its level may be loading); never less than the one above |
| `SRP_LOBBY_SESSION_GRACE_SECONDS` | 60 | How long the session of a link that went silent waits for its client to connect again (the client tries by itself, every 5 s); for a player in a match, counted from the match's end. 0: the session ends with the link |

### Player vs Bots (`servers/lobby-server/src/v026g/pvb.rs`)

One player against the server's own bots, chosen in srp's changed lobby movie (`docs/notes/lobby.md` §2.16.2, `matchmaking.md` §9.8, `player_bots.md` §8).

| Variable | Default | What |
|---|---|---|
| `SRP_PVB` | off (the compose file sets `on`) | `on`: the mode is offered |
| `SRP_PVB_MAX_GEAR` | 5 | The highest equipment level let in (the client's own, of the kits that can be played), 0 to 15 |
| `SRP_PVB_MAX_MATCHES` | 5 | The most matches against bots at once, 1 to 20 (choices held for a Play count too). With `SRP_PVB=on` the match server keeps up to this many bots' maps in memory (or `SRP_MAPDATA_AT_ONCE` if more): about 35-70 MB each |
| `SRP_PVB_PAY` | 0.5 | The share of a match's silver and experience such a match pays, 0 to 1. It counts for no quest and no daily bonus, and moves no rating |
| `SRP_PVB_STALL_S` | 60 | A match against bots whose player has been asked for its world this long and uploaded none while it plays is over (as deserted), 10 to 3600 |

### Match server (`servers/match-server/src/v026g/mod.rs`)

Each is an escape hatch for one thing not proven against a live client.

| Variable | Default | What |
|---|---|---|
| `SRP_LATE_INPUTS` | `relay` | `relay`: a late input goes to the others at its tick while every other game can still take it, else as `discard`; `discard`: tell the sender to drop it (0xF1/0xF2); `move`: move it (0xEF/0xF0) |
| `SRP_CLOCK_CORRECTION` | on | `off`: no 0xE7 (the 0xDB clock field is LIKELY) |
| `SRP_CHECK_INITIAL_WORLD` | on | `off`: take a first world that already has statistics in it |
| `SRP_MATCH_CLIENT_TIMEOUT_SECONDS` | 10 | A playing client silent this long is gone. A client that loads its level has 60 s, or this if it is longer (since 2026-10-12) |
| `SRP_RESYNC_WORLD_ONLY` | off | `on`: a playing client is resynced with 0xE8 alone |
| `SRP_RESTORE_LIMIT` | 0 | Resyncs in a row without agreement after which a client is disconnected; 0: no limit |
| `SRP_COMPRESS_STATE` | on | `off`: the world (0xE8) goes to the clients as it is, not as LZMA2 (since 2026-10-13; `docs/notes/match.md` section 27) |
| `SRP_MAPDATA_DIR` | unset (off; the compose file sets `/srv/srp/data/mapdata`) | The bots' maps: the `<project>.mapdata` files (`docs/notes/player_bots_mapdata.md` section 6; game data, copied into the data volume by hand). A folder that is not there, or a project with no file: bots without maps (a warning) |
| `SRP_MAPDATA_BUILT` | `<SRP_MAPDATA_DIR>/built` | Where the server writes the grids and trees it builds of the maps and reads them the next time; `off`: built at each load |
| `SRP_MAPDATA_KEEP` | 1 | Built maps kept that no match holds |
| `SRP_MAPDATA_AT_ONCE` | 1 | The most bots' maps in memory at once (with `SRP_PVB=on`, at least `SRP_PVB_MAX_MATCHES`) |
| `SRP_ENTRY_STATE_SECONDS` | 4 | How long the first world of a game that has none yet is kept on its way before its sync starts over; 2 to 5, and less for an old snapshot (the ticks on the wire reach 6.35 s from the clock the game waits at). A game that plays is resynced within 1.75 s |
| `SRP_RELAY_PACK_MS` | 15 | How long a relay or a drop order may wait for others to share its datagram (at most 50; 0: each goes at once). Anything else queued, a full datagram or an ack due sends them at once (since 2026-10-14; `docs/notes/transport.md` section 11.1) |

### Test bots (testing only; `crates/session/src/v026g/bots.rs`)

| Variable | Default | What |
|---|---|---|
| `SRP_BOT_ACCOUNTS` | not set: nothing about bots is on | The names of the accounts that are test bots, comma-separated, e.g. `bot-1,bot-2`. **For a test alone on one PC; never on a public server** |

A match needs two players. `srp-bot` (`servers/srp/src/bin/srp-bot.rs`,
`srp::bot`) is a scripted client that signs in, presses Play and sits in the
match so that one real client gets one: "Run locally" below. It cannot
simulate the game, so it has no world to hash or upload, and it makes none
up. With its account named here the server knows that instead:

- **Match server** (`game.rs`, "Bots"): a bot's place is a player (roster,
  0xE9, relays, pings, the end) and never a witness of the world. Its hash
  reports and uploads are dropped, it is never asked for a world, never
  counted among the clients that must agree, and never resynced for its
  hashes. It is started only once a real client is in. The trust rule
  counts the real clients only: with two or more of them nothing changes
  (a world needs two vouchers); with **one** real client it is the rule a
  client alone in a match always had: its world is trusted on its own word
  (`trusted_by` 1), within the plausibility limits. A match with bots and no
  real client connected for 45 s is over.
- **Rewards**: a bot's account is settled as any account. A match with a bot
  in it moves nobody's rating; a bot gets no daily bonus and starts no run
  of bonus days; bots are not in the ranking.

Without the setting a bot is a player like any other, and a match of one
real client and one bot never gets a trusted world after the first (two
must agree, the bot says nothing): no end by the mode's rule, empty results.

Why it is not for a public server: the one real client could claim any
plausible result, and whoever holds a named account is never checked. The
name belongs to whoever registers it first, as any name does.

### Rewards and features

| Variable | Default | What |
|---|---|---|
| `SRP_EXPERIENCE_RATE` | 2 | What a match's experience is multiplied by (1 to 100). 1 is the original's pace; 2 is the owner's choice of 2026-10-14 (it was 1 from 2026-10-12 and 3 before). Until 2026-10-14 it multiplied the reputation too, which now has `SRP_REPUTATION_RATE` |
| `SRP_MONEY_RATE` | unset: by match level | The percent of the original's silver a match's three rows pay; premium's half follows. Unset, it goes by the match's level (the owner's choice of 2026-10-14): 150 at levels 0 to 5, ten more each level, 220 from level 12. Set (1 to 1000), it is that percent at every level (it was 120 from 2026-10-13, 80 before) |
| `SRP_REPUTATION_RATE` | 120 | The percent of the original's reputation a match's rows and the quests completed in it pay (1 to 1000). 120 is the owner's choice of 2026-10-14 |
| `SRP_ITEM_WEAR` | off | `on`: a match wears the weapons and armour of the kits played, as the game's data says, and a worn item may get a defect. Off is the owner's choice (2026-10-13): an item stays as it is. The first start of a database with it off makes every worn weapon and armour piece whole, once |
| `SRP_DROP_PLAYTIME_BONUS` | off | `on`: the first four games of an hour of play raise the chance of a drop by 100, 75, 50 and 25 % (the numbers are the game's data; how they were applied is a guess). Off is the owner's choice (2026-10-13): with it on, 73 % of the players who finished a match found something |
| `SRP_DROP_CONDITION` | 40 | The condition, in percent, a weapon or armour piece found after a match arrives in (1 to 100; the owner's choice) |
| `SRP_DROP_DEFECT` | on | `off`: such an item arrives without a defect |
| `SRP_DAILY_BONUS` | on | `off`: no doubled victories, no seven-day reward, and no daily bonus message to the client |
| `SRP_DAILY_BONUS_RESET_HOUR` | 0 | The hour of UTC (0 to 23) a daily bonus day starts at |
| `SRP_PREMIUM_PRICES` | `100,200,400,1000,2500,4500,8000` | Gold for premium time, shortest first: a day, 3 days, a week, a month, 90 days, 180 days, a year. 90 days = 2,500 is from a January 2015 sale notice; the rest is the owner's choice (2026-10-12). |
| `SRP_CLAN_COST` | 50000 | Silver to register a clan (owner's choice) |
| `SRP_CLAN_MAX_MEMBERS` | 30 | Most members of a clan (owner's choice) |
| `SRP_CLAN_PLAYERS_REQUIRED` | 1 | Players who must accept before a clan is registered. The game's text says 5: the example `.env` sets 5 for the live server |
| `SRP_CLAN_TEAM_MIN`, `SRP_CLAN_TEAM_MAX` | 1, 10 | Players a clan team presses Play with |
| `SRP_PREMIUM_DISCOUNT` | 0 | Percent off premium time for a player without premium (0 to 90); 0 offers none |
| `SRP_PREMIUM_OFFER_SECONDS` | 3600 | How long the premium offer after a match stands (the client counts it down) |
| `SRP_RENAME_COST` | the rename voucher's price in the game's dictionary | Gold for a nickname change |
| `SRP_AMMO_USE` | on | `off`: a match takes no ammo off the account |
| `SRP_ITEM_DEFECTS` | on | `off`: items get no defects |

### Web page (`servers/web-server`)

| Variable | Default | What |
|---|---|---|
| `SRP_WEB_BIND` | `127.0.0.1` | Never the shared bind host: the page carries passwords over plain HTTP |
| `SRP_WEB_PORT` | `8088` | |
| `SRP_ADMIN_TOKEN` | unset | The admin page is off without it … |
| `SRP_ADMIN_PATH` | unset | … and without this: it is served at `/<SRP_ADMIN_PATH>` (a long random name) |
| `SRP_ADMIN_IPS` | `127.0.0.1,::1` | Addresses or ranges that may open it; anyone else gets the plain 404 |
| `SRP_WEB_TRUST_PROXY` | off | `on`: the client is the last `X-Forwarded-For` entry (a reverse proxy in front) |
| `SRP_DOWNLOAD_FILE` | unset | A file handed out at `/download` (5 at a time, resumable) |
| `SRP_DOWNLOAD_URL` | unset | Or a `https://` link for the download button |
| `SRP_DOWNLOAD_SHA256` | unset | Its checksum, shown on the home page |
| `SRP_OTHER_SERVER_URL` | unset | The page of another server of ours (plain `https://`). Every page's header links to it, named as the server for the other game version; on a test server (`SRP_TEST_SERVER=on`) it is named in the notice as the players' server instead. Not set on the players' server since 2026-10-12 (the 0.21d server it named is stopped) |
| `SRP_TEST_SERVER` | unset | `on`: every page says this is the test server: a badge beside the name, the title, and a notice that the server can restart at any moment and that accounts and progress there can be reset |
| `SRP_GAME_URL` | unset | Where the original game is had (plain `https://`): the home page's first step. The compose file's default is the web archive's copy of build 2777 |

Tests only: `SRP_TEST_LOG` (the servers' log on stderr). `SRP_WEB_DOMAIN` is
read by the 0.21d browser server and by the compose file's caddy, not by the
0.26g stack. The compose file's own variables (`SRP26_*`) are in
`srp-026g/docs/DEPLOY.md`.

## The web page and the admin

`http://127.0.0.1:8088` by default. English and Ukrainian.

- `/`: what the server is; step one, where the original game is had
  (`SRP_GAME_URL`); step two, the download of the few files that go over it;
  the launch line (`-localization=… -server=<host>:<port> -mt_allocator=768
  -minimum_resources_memory_size=768`); players online; how matches start.
- `/exchange` (`GET`): a short page saying that silver and gold are
  exchanged in the game now (the lobby's exchange window,
  `client-patches/README.md`, "Lobby exchange"), in the language of its
  `?lang=` (`en`, `ua`, `ru`, `de`, `pl`, as the launcher sends them; else
  the page's own). The SHOP button of the launchers players already have
  opens it (the launcher has no SHOP button since). The page exchanges
  nothing: a `POST` there is "not found" like any address it does not have.
- `/resources/<sha256>` (64 lower-case hex digits; `GET` and `HEAD`): the
  file of that name in the folder `resources` beside `SRP_DOWNLOAD_FILE`, as
  `/download` hands out its file (a `Range` too, five at a time in slots of
  their own), "not found" when there is none. The blobs of the updater's
  resource step (`../../client/updater/README.md`, "Resources"). Any other
  name under `/resources/` is the page's "not found"; the 0.21d page has no
  such route.
- `/patches` (`GET`, public): the patches of `resources.db` the players'
  updater keeps in the game folder, as JSON (`{"patch_<n>":{"sha256":…,"urls":[…]},…}`,
  `{}` for none): those the server's game data was made of ("Game data"
  above). The 0.21d page has no such route.
- `/api/client-files` and `/launcher/…` (since 2026-10-13, for the patched game
  launcher and its updater: `../client-patches/launcher/README.md`). The first answers with the
  SHA-256 and size of the file `/download` hands out (`SRP_DOWNLOAD_FILE`), as
  three lines of text; the file is read again when its size or time changes,
  so a new ZIP put in its place is seen without a restart. The second is the
  page the launcher shows in its window (899 by 335, made for Internet
  Explorer's engine, one script of its own, nothing from another host): the ten best
  players by rating among those who were in a match (name, clan tag, level,
  rating; test bots left out), players online, searching, matches running,
  accounts, battles fought (how often an account was put in a match, all
  accounts together), clans, a planned maintenance, the legal notice.
  Ukrainian for `?lang=ua`, else as every page. Its numbers are counted again
  at most every 15 seconds. Not on the players' server yet.
  The left side has two tabs, "Top 10 players" and "Matches now"; a click
  on a label in the heading shows that tab. The second lists the matches
  made and not over, the oldest first, and names no player (the page is
  open to everyone): the map and mode by the game's own names, each team's
  players who have not left ("4 v 3"), how long ago play began ("6 min
  ago"; "waiting" before the countdown, "starting" during it), the time to
  the time limit in minutes rounded up ("≤ 9 min": the mode's own rule or
  everyone leaving ends a match sooner), and the score with the leader's
  number in amber (tickets left in a Team Deathmatch, batteries brought
  otherwise). The score is the one of the newest world the match server
  trusts (`servers/match-server`: the match loop tells the registry how
  each match stands once a second, `MatchRegistry::live_matches`); it is
  shown only for a match in play, and not when that world is more than
  60 seconds old (the clients disagree), where a dash stands. Ten lines
  fit: with more matches the last line counts the rest.
  While the window is open the page's script asks
  `/launcher/places/<uk|en>` every 10 seconds: the board, the matches, the
  numbers of this moment with the maintenance line, and the totals, each
  behind the `id` of its place, all with `<hr>` between them. It puts in
  place only a piece that changed, so the page is not loaded again and
  nothing else is drawn: the tab that was chosen stays (which one shows is
  the class of the left side, outside every place). A request that fails
  leaves the page as it is (the launcher does not go to its offline page).
  A place the page does not have is passed over, so a page that stays open
  over a later change of the server keeps what it knows.
  `/launcher/fresh/<uk|en>` still answers the three places of the page
  from before the tabs, in its form, for a launcher left open over that
  deploy. These addresses alone are sent with a `Content-Security-Policy`
  that lets that one script run, by its SHA-256, and ask this server.
- The admin page (token, path and address as in the table). Tabs:
  money (silver and gold, `+500` / `-100`); account (premium hours, set the
  level, experience, reputation with a faction 1 to 4); item (an item of the
  0.26g dictionary by id or exact name: up to 8 of an item, up to 100 000
  units of a stack; an id the dictionary lacks is refused, since the client
  crashes on one); text (two lines, 63 bytes each, shown to everyone in the
  lobby and to who signs in, until cleared: `0xA5` / `0xA6`); ban, unban,
  bans; maintenance; signups; password (since 2026-10-18).
- Password reset (`notes/password-reset.md`, the owner's decision of
  2026-10-18): by the user name the player signs in with, in any case, never
  the character name. The page shows the account it matched first, then
  resets; a script resets with one POST (`tab=reset_password`, `login=`,
  the token) and gets one line back (`ok <login> <id>`, `no such login`,
  …). The password is cleared and written at once, the account's sessions
  end (its game is signed out), and its next sign-in, with any password,
  sets the new one. srp keeps running; no SQL by hand.
- Every change is written and the player's open game is told
  (`Inbox::account_changed`: inventory, account, money, level and skills,
  reputations, quests; then the services message for the premium time).
- A ban ends the account's session; the 0.26g lobby signs its link out
  (`AA`, then the link is closed) within a second, and the next sign-in gets
  the game's own ban status. Maintenance minutes reach the clients in the
  online counts (`0x9F`). Closed registration refuses new names at login.
  These are the store's, the same code 0.21d uses.
- Not there: the 0.21d-era console in the srp terminal (`status`, `start`,
  `end`) does not exist in this tree; nothing replaced it.

The page is of one game version by how it is started (`web_server::run` for
0.21d, `run_026g` for 0.26g), not by a setting: `srp` here starts the 0.26g
servers only, so a switch would have nothing to switch.

## What is implemented, and what is refused

Implemented (coded and tested; what live play has shown so far is in
OPEN-ISSUES.md, "0.26g"):

| Area | State |
|---|---|
| Login | Sign-in, registration on first sign-in, wrong password, bans with the game's ban reasons, limits, closed registration, wrong build |
| Lobby account | The sign-in push in the order the client needs; inventory with its three "spent" lists; kits (three, every one usable without premium), move, autobuy flag, active kit, kit names |
| Shop | Buy, sell, repair, gold for silver and (srp's lobby exchange) silver for gold (`47 04`); sellers by reputation level |
| Skills | Set, reset for silver or gold |
| Quests | Choosing a faction, daily quests, progress and pay after a match |
| Queue and matches | Play alone or as a squad, leave, rejoin after a new sign-in (`MatchRegistry::reseat`) |
| Results | Rewards (experience, silver, reputation, levels, rating, drops, item wear), the results screen, player details, the drop card |
| Social | Chat (one general channel for every language: the owner's rule, as on 0.21d; private, squad), friends, ignore list, squads, important messages, online counts, the ranking. A name that is no player gets the game's "Player not found." (`social.md` §9) |
| Operator | The web page above; the operator's text |

Coded on 2026-10-12 and tested; most of it not looked at with a real client
yet (`WORK.md` has each one's entry; what each message gets is in
`srp-026g/docs/notes/coverage.md`):

| Area | Notes |
|---|---|
| Item modifications, upgrades, reroll, craft, using an item, defects and repair | `items.md` |
| Premium shop, buying premium (also from the results screen), bundles, bonuses, offers, nickname change | `premium.md` |
| Clans (create, invite, ranks, clan teams, clan play) | `social.md` |
| Match pay by the results screen's rows, drops, the seven-day bonus | `results.md` §10, `progression_sources.md` |
| Matchmaking by level, 8 a side, least-played map | `matchmaking.md` |
| Silent lobby clients and their return | `transport.md` §6.1 |
| Match server: sync order, paired uploads, hash phases | MATCH-SYNC-026G.md |

Refused or absent, and nobody is on it:

- Comments about a user (`49 05`): refused.
- A browser server and presence pings: 0.26g has none.
- The mode 3 battery patch of 0.21d: not needed (0.26g serializes the
  next-battery time).
- Public HTTPS for the page beside the live server (DEPLOY.md).

srp's own choices (the client does not reveal a rule): the level table is
0.21d's, 50 levels; the reward rules are 0.21d's; one queue; a second sign-in
signs the first out; chat limit 10 lines then 1 a second (the client's own
spam filter is stricter and prints the game's notice itself); a match's
report is kept while a results screen of it may be open (two hours); silver for gold on
the page at the game's gold-for-silver rate; the admin's limits above.

## Build and test on Windows

```bash
export OPENSSL_DIR="C:/Program Files/OpenSSL-Win64-3" OPENSSL_STATIC=true OPENSSL_LIB_DIR="C:/Program Files/OpenSSL-Win64-3/lib/VC/x64/MD"
cd srp-026g && cargo test --workspace --offline
```

- `servers/srp/tests/v026g_end_to_end.rs`: the real servers on free ports
  with a database file, two scripted clients from the TLS sign-in through a
  purchase, a room, a whole match, the rewards and the results screen, to
  a second sign-in.
- `servers/srp/tests/v026g_web.rs`: the same servers with the web page: the
  admin's changes reaching an open game, the old exchange form changing
  nothing, the operator's text, a ban ending the lobby link.
- Every server message has a byte-exact test, and every push in the tests
  goes through the strict client model.

These prove the servers agree with each other and with the notes; they do
not prove the notes agree with the client. Format only the files you touched
(`rustfmt --edition 2024 <file>`).

## Run locally

```
cd srp-026g        (the certificates are read from .\certs)
cargo run --release -p resources-db --bin srp-game-data -- build --dir <the game folder>\game --patch-url http://127.0.0.1/patches --out game-data.db
set SRP_GAME_DATA=game-data.db
set SRP_MIN_PATCH_REVISION=<the newest patch's n; leave it out without patches>
cargo run -p srp
survarium_srp.exe -localization=ukrainian -splash_screen=0 -server=127.0.0.1:1234 -mt_allocator=768 -minimum_resources_memory_size=768
```

`--dir` takes `resources.db` and every `resources.db.patch_<n>` of that
folder, as the game mounts them; a stock game folder gives the data of
`resources.db` alone (and `/patches` answers `{}`). `SRP_GAME_DATA` may also
name the release's `server-game-data.db`.

The two memory keys are different arenas (read in the exe 2026-10-12):
`-mt_allocator=<MiB>` is the global multithreaded allocator (option object
0xA80150, read at 0xE0FF5: without the key the arena is 16 + 360 = 376 MiB,
with it `value << 20`), and `-minimum_resources_memory_size=<MiB>` is the
floor of the resources arenas (option object 0xA83080, a float read at
0xE9966, default 0x150 = 336 MiB). Neither covers the other. Upstream
(`origin/feature/server-026g`, live under Wine) ran out of the 376 MiB arena
while loading a map and fixed it with `-mt_allocator=768`
(`upstream_compare.md` §10). Both keys together are what the players' start
file and the shortcuts use, and maps load with them (live since 2026-10-12).

Start the game from its `game\binaries\x86` folder (the client finds its
login certificate by a path relative to the working directory). The game
folder needs the patched exe and srp's certificate
(`client-patches/README.md`). What to watch in a first session, step by
step: `srp-026g/docs/notes/README.md`. For a quick first match:
`SRP_MATCH_GATHER_SECONDS=5 SRP_MATCH_MAPS=117 SRP_PACKET_LOG=logs/packets`.

### A match alone: the test bot (added 2026-10-12)

One PC, one game, and a match needs two. Start srp with
`SRP_BOT_ACCOUNTS=bot-1,bot-2,bot-3`, then the bot:

```
cd srp-026g
cargo build -p srp --offline                    (builds srp.exe and srp-bot.exe)
target\debug\srp-bot.exe --server 127.0.0.1:1234
```

`srp-bot [--server HOST:PORT] [--name NAME] [--password PASSWORD] [--count N]
[--once] [--kit ID]`; also `SRP_BOT_SERVER`, `SRP_BOT_NAME`,
`SRP_BOT_PASSWORD`. The default name is `bot-1` (a new name registers
itself), `--count 3` runs `bot-1`..`bot-3`, the password is never printed
(without one a built-in password for local tests is used). It prints one
line a step: signed in, in the lobby, pressed Play, in a room, match
found, static info, the world, in the match, the match's end, the results;
then it presses Play again (`--once`: it stops). When srp is restarted it
signs in again by itself.

Then press Play in the game: you go into the bot's room (it is near a new
account's level), and the match is made after the room's vote and wait.
The bot is on the other team. It sends no
inputs: its player is put on a respawn point by the game's own respawn rule
(no input or kit choice is needed for that: PROVEN in the exe, match.md
§19) and stands there. With `--count N` the first bot waits in its room
and the others join once somebody else is in it.

With the setting it is proven by the scripted clients only
(`v026g_end_to_end.rs`, the match server's `tests/bots.rs`). Seen with a real
client once without it (2026-10-12 07:50 UTC, on the public host before bots
were ruled out there): the match ended by the clock with no trusted world, as
the paragraph above says.

## The client patch and the players' download

`scripts/client-patch-026g/patch_026g.py <survarium.exe> <survarium_srp.exe>`
builds the patched client from the original exe (its sha256 is checked; each
site is checked against the original bytes). It patches the code-signing
certificate check (without it every stock 0.26g client stops with "Resources
archive is corrupted": the certificate ran out in 2015), the single-instance
check, the default hash interval (100 → 50 ms), two instructions of the
battery rule of modes 2 and 3 (so that a team's last battery can be put in;
patch revision 3, in the players' download since 2026-10-13 12:22 UTC), two
sites that let a game start whose Documents folder it may not write to
(patch revision 4, in the players' download since 15:36 UTC of that day),
and adds the snapshot
upload in a new PE section. `--cert-only` and `--cave=hash-mismatch` are the
other builds. Detail: MATCH-SYNC-026G.md and `client_patch.md`.

`client-patches/make_client_026g.py <game folder> <output>
--server=<host>:<port>` makes the players' overlay without touching the game
folder: `game/binaries/x86/survarium_skhid.exe` (the patched exe),
`bugtrap.dll` (the Rust local crash reporter), `play_skhid.bat` and the Linux
launcher `play_skhid.sh` beside them, srp's login certificate, a player
`README.txt` and `LINUX.txt`; and, unless `--no-launcher` is given, the game's
launcher made to work again (`client-patches/launcher/README.md`):
`launcher_skhid.exe`, `updater_skhid.exe` with `updater_skhid.url`, `7zr.exe`
(the extractor the updater unpacks the original game with for a player who
has none), the launcher's ten local pages, and `Skhid.bat` and `Skhid.sh`
beside `game`, the one file a player starts. Build the Rust programs first with
`../client/bugtrap/build.sh` and `../client/updater/build.sh`; the overlay
builder includes them by default.
`--pack-zip` creates the player archive directly instead of a folder, with exactly
those files (23; seven with `--no-launcher`) and no source, symbols or tests.
`--check` writes nothing. The same input gives the same archive byte for byte:
the updater installs whenever the archive the site offers has another checksum.
The sites hand out this overlay (`Survarium-0.26g-srp-files.zip`), never the
whole game.

## Deploying

Live on a host of its own since 2026-10-12 (026g.skhid.link; `DEPLOY.md`
has the layout and how it is updated). The Dockerfile and
`docker-compose.yml` in `srp-026g` build and run the 0.26g stack as compose
project `srp26` (services `srp26` and, with the profile `tls`, `caddy26`),
with the game's own ports there: login 1234/tcp, lobby 1235/udp, match
1236/udp, the web page behind caddy on 443. `deploy/env-026g.example` is the
`.env` to fill in.

The compose file's defaults are another block of ports (2234/tcp, 2235/udp,
2236/udp, the page on 127.0.0.1:8098), made so that the stack can also run
beside a 0.21d server on one host without sharing a name, a volume or a port
with it; that way was not taken. Both ways, and what each needs, are in
`srp-026g/docs/DEPLOY.md`.

## The cross-check: upstream's 0.26g branch

Upstream has its own 0.26g work: branch `origin/feature/server-026g` of the
`srp` repo (`srp`, commit `8204c88b7`). It is a **dev match
server with a fixed roster**: no lobby service, no matchmaking, no accounts.
But it was **live-tested with real 0.26g clients, under Wine** (Linux; never
on Windows), before ours had met a real client.

`srp-026g/docs/notes/upstream_compare.md` sets their findings
against our notes and code, topic by topic, with the strength of each piece
of evidence (live, oracle, static) and a list of recommended changes. The
points it raised before our first live session:

- the launch line needs `-mt_allocator=768` (done in `make_client_026g.py`,
  the web page, the docs and the shortcuts);
- the client joins messages into one datagram by default (our transport
  note said it does not);
- after a resync, a client's own inputs must not be replayed to it;
- an unfocused client can stall for close to a minute (Wine evidence), and
  our match server drops a silent client after 10 s. The lobby link is
  ticked by the client in every state, focus or not (transport.md §6.1);
  the lobby drops a silent link after 30 s (120 s in a match) and keeps
  the session a minute more for the client's own reconnect
  (`SRP_LOBBY_LINK_TIMEOUT_SECONDS`, `SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS`,
  `SRP_LOBBY_SESSION_GRACE_SECONDS`);
- under Wine only, the client dies in `qwave.dll` when it connects to the
  lobby.

The match server points were applied on 2026-10-12 (`WORK.md`).
