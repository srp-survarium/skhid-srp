# Client 0.26g: what changed from 0.21d, and the verdict

**Version scope:** client **0.26g** (build string "0.26g0", build 2777,
compiled 2015-01-14, `resources.db` 2015-01-17) compared with **0.21d**
(build 2010, April 2014). Every address in these notes is for the build it
names. 0.26g has **no PDB**, and neither does any public build after 0.21d
(see `E:\sur-res-pro\RE-TOOLKIT.md`, "Symbols by build"). 0.26g functions are
named by their assert and `__FUNCSIG__` strings, or by matching them to 0.21d
(`tools/re-kit/v026g/functions_matched.txt`).

All findings come from reading the two exes and the two `resources.db` files
(2026-10-12). **Nothing has been tested against a running 0.26g client**
(the server stack is tested end to end with scripted clients only; see
"How to run the 0.26g stack locally" below).
Each note tags its findings PROVEN (read in the code or data at the address
given), LIKELY or GUESS.

## The notes

| Note | Covers |
|---|---|
| [login_chat.md](login_chat.md) | Command line, login over TLS, the browser server (removed), presence, chat transport and layouts |
| [lobby.md](lobby.md) | Lobby transport, every server→client message, **crash hazards and the safe push order** (§3) |
| [lobby_requests.md](lobby_requests.md) | Every client→server lobby request and the reply it expects |
| [social.md](social.md) | Chat channels, friends, squads, clans, online stats |
| [premium.md](premium.md) | Premium time, the shop's offers and bundles, items for gold, the daily bonus, the tutorial, a nickname change, autobuy and auto-repair, the level notice; their settings |
| [results.md](results.md) | Match results screen, player details, drops, daily bonus, craft tables |
| [progression_pace.md](progression_pace.md) | How long levels, faction ranks and gear tiers take, measured on the players' server, and the pay rates chosen from it |
| [items.md](items.md) | Modifications, defects, cosmetics, reroll, craft, repair: the data, the wire, the rules |
| [weapon_scopes.md](weapon_scopes.md) | Weapon scopes: how the client builds one, scope items bought once and put on any number of weapons (variants per weapon from the game data), what a release still needs |
| [match.md](match.md) | Match protocol: lockstep, handshake, message ids, snapshot and hash, time sync, input rules |
| [transport.md](transport.md) | The UDP transport that matches **and now the lobby** use: header, acks, per-id lengths, multipart |
| [static_info.md](static_info.md) | Match start (0xE4): match options, profiles with kits, byte layout |
| [matchmaking.md](matchmaking.md) | The player's level as the client computes it, the search screen, srp's queue rules (sides of 8, level range, match level, least-played map) |
| [modes_maps.md](modes_maps.md) | Maps, game modes, map ids, rule classes, match settings now in data |
| [data.md](data.md) | `resources.db`: items, skills, drops, quests, item configs |
| [client_patch.md](client_patch.md) | Porting the srp client patch, the function map |
| [replay_sur.md](replay_sur.md) | The client's own replay file (`.sur`, `-match_replay`): its layout, and `srp-recording-to-sur`, which makes one from a match recording |
| [replay_watch.md](replay_watch.md) | "Watch replay" from the lobby's History, the server side: conversion at match end, the lobby's `R1` answers and `9E` link, the download tokens, `/replays/<token>`, what is and is not reachable over HTTP |
| [password-reset.md](password-reset.md) | The admin's password reset while srp runs: the page's Password tab (confirm step), a script's one POST and its answers, the curl from the jump host, the owner's rule (2026-10-18) |
| [launcher.md](launcher.md) | The game's launcher (`survarium_launcher.exe`): how it starts, its registry values, the pipe to its updater and every message on it, PLAY, the page, the buttons, the language. What was built from it: ../../client-patches/launcher/README.md |
| [fall_damage.md](fall_damage.md) | Fall damage: the formula, the values (fatal_damage 1.0 as in 0.27, the owner, 2026-10-19), the lethal heights stock and now |
| [battery_pickup.md](battery_pickup.md) | The battery pick-up 20 % faster: take_time 0.6 -> 0.5 (the owner, 2026-10-20), why the animation plays faster and is not cut, what the release needs |
| [damage_calculator.md](damage_calculator.md) | The site's damage calculator (`/damage`): the owner's rules for it, the data it is built from, how it is served and kept up to date |
| [maps_tab.md](maps_tab.md) | The site's Maps tab (`/maps`): spawns, batteries, chargers, containers and artefacts on every map; the owner's rules, rebuilt per release |
| [spotting.md](spotting.md) | Spotting: no cooldown, a battery's body and its use box both spot it, the markers (the owner, 2026-10-19; exe patch 38) |
| [fov_lod.md](fov_lod.md) | Models at a high field of view: why they went out sooner (LOD and screen factor by size on the screen), the scale table, the fix (exe patch 40) |
| [launcher_updater.md](launcher_updater.md) | The game's own updater (`survarium_updater.exe`), which the Skhid launcher does not use: its arguments, what it said on the pipe, what it did on the network |
| [../DEPLOY.md](../DEPLOY.md) | The Docker files for the 0.26g server beside the live 0.21d one (ports table), certificates, the players' overlay. Nothing is deployed |

The draft patcher is in `srp-026g/client-patches/`. The data dumps
are in `tools/re-kit/v026g/data/`.

## How to run the 0.26g stack locally

`srp` (`servers/srp`) starts the 0.26g servers in one process
(`srp::v026g::Stack`): login, lobby, match, the session collector, and the
web page. The 0.21d servers are in the tree but not started.

```
cd E:\sur-res-pro\srp-026g        (the certificates are read from .\certs)
cargo run -p srp
survarium_srp.exe -localization=ukrainian -splash_screen=0 -server=127.0.0.1:1234 -mt_allocator=768 -minimum_resources_memory_size=768
```

**Corrected 2026-10-12: `-mt_allocator=768` added.** The two memory keys are different arenas (read in the exe 2026-10-12):
`-mt_allocator=<MiB>` is the global multithreaded allocator (option object
0xA80150, read at 0xE0FF5: without the key the arena is 16 + 360 = 376 MiB,
with it `value << 20`), and `-minimum_resources_memory_size=<MiB>` is the
floor of the resources arenas (option object 0xA83080, a float read at
0xE9966, default 0x150 = 336 MiB). Neither covers the other. Upstream
(`origin/feature/server-026g`, live under Wine) ran out of the 376 MiB arena
while loading a map and fixed it with `-mt_allocator=768`
(`upstream_compare.md` §10). Both keys together are what the players' start
file uses, and maps load with them (live since 2026-10-12).
The shortcut in the game folder still has the old line (it is the
owner's to change). For test runs upstream also used `-net_log
-log_to_stdout -write_errors_to_stderr`.

The second line is what the shortcut `Survarium 0.26g (srp, Ukrainian).lnk`
runs; its port is srp's default login port.

| Server | Default | Setting |
|---|---|---|
| Login (TLS sign-in) | TCP 127.0.0.1:1234 | `SRP_LOGIN_PORT`, `SRP_LOGIN_BIND` |
| Lobby | UDP 127.0.0.1:1235 | `SRP_LOBBY_PORT`, `SRP_LOBBY_BIND` |
| Match | UDP 127.0.0.1:1236 | `SRP_MATCH_PORT`, `SRP_MATCH_BIND` |
| Web page | HTTP 127.0.0.1:8088 | `SRP_WEB_PORT`, `SRP_WEB_BIND` |

Every setting has a default; none is needed on a dev PC.

- `SRP_BIND_HOST`, `SRP_PUBLIC_HOST`: for all servers at once (other PCs:
  `SRP_BIND_HOST=0.0.0.0` and the machine's address as the public host; the
  0.26g client is told the lobby and the match server as IPv4 addresses, so
  a name is looked up at start).
- `SRP_DATA_DIR` (default `./data`): `srp.db`, both the accounts and the
  0.26g tables (`g26_*`). `SRP_DB=off`: memory only.
- `SRP_LOG_FILES=off`: no `logs/<server>.log`, stdout only.
- `SRP_LOG_DIR=<dir>` (unset: none): everything stdout gets, in one file per
  UTC day, `<dir>/srp-YYYY-MM-DD.log`, added to across restarts.
  `SRP_LOG_KEEP_DAYS` (default 14): older files are deleted. The compose
  file sets it to `/srv/srp/data/logs`, in the data volume (since
  2026-10-12: a container's own output is gone at every update).
- Matches: every PvP match is a room's (the players' "servers", since
  2026-10-17; `lobby.md` §2.16.3): Play puts a player into one, its vote
  and wait start its match, and who comes while it is played goes into it.
  `SRP_ROOMS_MAX` (50), `SRP_ROOMS_EXTRA_MAPS` (none).
- `SRP_LOBBY_LINK_TIMEOUT_SECONDS` (default 30),
  `SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS` (120),
  `SRP_LOBBY_SESSION_GRACE_SECONDS` (60): a silent client's lobby link
  goes after the first (the second for a player in a match), and its
  session waits the third for the client to connect again (transport.md
  §6.1).
- `SRP_MIN_PATCH_REVISION` (default none; not a number from 0 to 255 stops
  srp, and so does one under the newest patch of the game data): a game
  whose exe says an older patch revision is refused at its sign-in (0x22)
  and its sessions from before a restart in the lobby (client_patch.md §6).
- `SRP_MATCH_MODES` (default `2,3,4`), `SRP_MATCH_MAPS` (default: every
  playable map, 72 of them): a room's pool by default, and what a clan
  team's match is drawn from, so for a first test name one, e.g.
  `SRP_MATCH_MAPS=117` (Team Deathmatch on level_01).
- `SRP_BALANCE_PLAYER` (default 12), `SRP_BALANCE_PLAYER_LEVEL` (1.5),
  `SRP_BALANCE_RATING` (4), `SRP_BALANCE_TRUST_MATCHES` (5): what a player
  counts for when the sides of a match are made (`matchmaking.md` §8).
  `SRP_BALANCE_RATED_PLAYERS` (4): the fewest players a match must have
  had in it to move the balance rating (§8.3).
  `SRP_BALANCE_BATTERY_LEAD` (0.55), `SRP_BALANCE_TICKET_LEAD` (0.2):
  what a lead counts for when a player comes into a match being played
  (`matchmaking.md` §9).
  `SRP_SQUADS` (default on; `off`: no squads), `SRP_SQUADS_OFF_TEXT`
  (`social.md` §9.4).
- `SRP_EXPERIENCE_RATE` (default 2), `SRP_MONEY_RATE` (percent; unset:
  150 to 220 by the match's level), `SRP_REPUTATION_RATE` (percent,
  default 120): the rewards (`results.md` §10, §13; how long progression
  takes: `progression_pace.md`). `SRP_ITEM_WEAR` (off): whether a
  match wears the items played with. `SRP_DROP_CONDITION` (40),
  `SRP_DROP_DEFECT` (on): what a found weapon or armour arrives as.
  `SRP_DROP_PLAYTIME_BONUS` (off): the play time bonus to the drop chance.
  `SRP_DAILY_BONUS` (on), `SRP_DAILY_BONUS_RESET_HOUR` (0): the daily bonus.
- `SRP_SNAPSHOT_DIR` (default `logs/snapshots`; `off`): uploaded worlds.
- `SRP_RECORD_DIR=<dir>` (off by default), `SRP_RECORD_MIN_PLAYERS` (6),
  `SRP_RECORD_MAX_MB` (5000): match recordings, every datagram of a match
  in and out, kept for matches that 6 or more players entered (late
  joiners counted). Format: `match-server/src/v026g/record.rs`; reader:
  `cargo run -p srp --bin srp-recording -- <file> [--dump]`.
- `SRP_REPLAY_DIR=<dir>`, `SRP_GAME_MODES_DIR=<dir>` (both off by default),
  `SRP_REPLAY_MAX_RECORDING_MB` (24), `SRP_REPLAY_MAX_MB` (1000): the
  lobby History's "Watch replay": kept recordings made into `.sur` files
  when first asked for, a token from the lobby, the file at
  `/replays/<token>` (`replay_watch.md`).
- `SRP_PACKET_LOG=<dir>`: every datagram of the lobby and the match
  server, one file a connection (the lobby sign-in's session id left out).
  **Set it for every live test**: it is the packet capture.
- Escape hatches of the match server, each for one thing that is not tried
  against a live client:
  - `SRP_CLOCK_CORRECTION=off`: no 0xE7 (what the client does with it is
    read from its code, match.md §17; not seen live);
  - `SRP_CHECK_INITIAL_WORLD=off`: take a first world that already has
    statistics in it;
  - `SRP_LATE_INPUTS=move`: move a late input to an open tick
    (0xEF/0xF0; match.md §16) instead of telling its sender to drop it
    (0xF1/0xF2, the default, what 0.21d does);
  - `SRP_RESYNC_WORLD_ONLY=on`: resync a playing client with 0xE8 alone
    (match.md §18);
  - `SRP_RESTORE_LIMIT=<n>`: disconnect a client after n restores in a row
    that did not help (default 0: never);
  - `SRP_COMPRESS_STATE=off`: send the world as it is, not as LZMA2
    (match.md §27);
  - `SRP_ENTRY_STATE_SECONDS` (default 4, 2 to 5): how long a game's first
    world is kept on its way;
  - `SRP_RELAY_PACK_MS` (default 15, at most 50, 0 = off): how long relays
    and drop orders wait to share a datagram (transport.md §11.1);
  - `SRP_MATCH_CLIENT_TIMEOUT_SECONDS` (default 10): raise it (60–120)
    when two clients run on one PC, a window out of focus can stall.

**Only a client with the snapshot-upload patch can start a match**: the
world stays frozen until a client uploads it (0xE0 chunks), and a stock
client never does. **The full patched exe is installed since 2026-10-12**
(corrected the same day; this note said "NOT installed yet"): with the
owner's yes, `survarium_srp.exe` in the local test game folder is the
section build of `patch_026g.py` (certificate check, single-instance check,
50 ms hash interval, snapshot upload); the exe it replaced, which had only
the certificate patch (`--cert-only`), is beside it as
`survarium_srp.cert-only.bak.exe`. The upload code runs in live matches
since 2026-10-12 (`WORK.md`; `client_patch.md` was written before that).
Since the evening of that day `patch_026g.py` also patches the battery rule
(`client_patch.md` §3.9; patch revision 3), lets a game start whose Documents folder it may not write to (§3.10, written 2026-10-13; patch revision 4) and skips the minimum video-memory check (§3.7): the exe installed that day has none of them; the players' download
has all three since 2026-10-13 15:36 UTC (the battery rule since 12:22 UTC).

### A match alone: the test bot (added 2026-10-12)

A match needs two players; one person with one PC has one game. `srp-bot`
is a scripted client that is the second player. **For testing only.**

1. Start srp with the bots' accounts named:
   `SRP_BOT_ACCOUNTS=bot-1,bot-2,bot-3` (`srp-026g-local-test.bat` has it).
   Without it the bot still signs in and takes its place, but the match
   never gets a trusted world after the first (two clients must agree, and
   the bot reports nothing): it runs to the time limit with empty results.
2. Start the bot: `E:\sur-res-pro\srp-026g-bot.bat`, or
   `target-live\debug\srp-bot.exe --server 127.0.0.1:1234` (built by
   `cargo build -p srp`). Options: `--name` (default `bot-1`; a new name
   registers itself), `--password` or `SRP_BOT_PASSWORD` (never printed; a
   built-in one for local tests otherwise), `--count N` (`bot-1`..`bot-N`),
   `--once` (stop after one match), `--kit ID`.
3. Sign in with the game and press Play. The bot waits in the queue
   ("in the queue: place 1 of 1"); with the player there are two, and the
   match is made `SRP_MATCH_GATHER_SECONDS` later.

What the bot prints, a line a step: signed in at the login server; in the
lobby; pressed Play; its place in the queue; a match is found; goes to the
match (0xD8); took the static info and says it has loaded (0xDA); waits for
the time and the world (the real client's upload starts the match); took
the world (0xE8); is in the match (0xE9); every player who enters or
leaves; every 30 s that it is still there; the match is over (0xEA with
its own place); the results screen; Play again. A line with `WARNING` is a
message the strict client model would not take: a server fault to look at.
When srp is restarted the bot signs in again by itself (every 5 s).

What the bot is in the match (`match.md` §19): a stock client that
simulates nothing. It acks, answers the time and the pings, sends its
keep-alives, and sends no input, no hash and no world. The real client
spawns the bot's player by its own respawn rule and the player stands
there. In Team Deathmatch (`SRP_MATCH_MAPS=117`) killing it takes its
team's tickets (100), which ends the match by the mode's rule; otherwise
the match runs its 900 s.

What the server does for a bot (only with `SRP_BOT_ACCOUNTS`): its place is
a player and never a witness; the trust rule counts real clients only (one
real client: its world is trusted on its own word, as for a client alone in
a match); no rating moves in a match with a bot; a bot gets no daily bonus
and is not in the ranking; a match left to its bots for 45 s is over.

With `--drive` (added 2026-10-14) the bot sends inputs: a fixed script
(walk, turn a quarter, walk, sprint, crouch, fire, reload, weapon 2, turn
back), again and again, and prints each input ("input for T ms (tick, its
clock): keys, turn, yaw sum: what it starts"). A server that dropped or
moved one of its inputs is a `WARNING` line. What to check:
`player_bots.md` §6.

In the log (`logs/match.log`): "… N of them test bots", "player P (a test
bot) waits for a real client to be in", then the usual lines; "only test
bots for 45 s; it is over". In `logs/srp.log` a warning at start that the
setting is on.

### What is tested, and how

`servers/srp/tests/v026g_end_to_end.rs` starts the same `Stack` on free
ports over a database file and drives two scripted clients from the TLS
sign-in through a purchase, the queue, a whole match (static info, upload,
sync, inputs both ways, hash reports, the end by the mode's rule), the
rewards and results screens, a sign-out and a second sign-in, and checks
the database file. Every message is read by the strict client models. This
proves the servers agree with each other and with the notes; it does not
prove the notes agree with the client.

The same file runs the test bot as the `srp-bot` binary runs it
(`srp::bot::run`) beside one such scripted client: with `SRP_BOT_ACCOUNTS`
a whole match to both results screens, and without it the check that
nothing about bots is on (the bot's account is ranked, its place is a
witness's, and one client's claim alone ends nothing). The match server's
own `tests/bots.rs` has the rules one by one.

### What to check in the first live session

In order; each step's failure points at one note.

1. **Login.** `logs/login.log`: "Sign-in: account=… session=…, sent to the
   lobby at UDP …". A TLS failure means the certificate in the game folder
   is not srp's (`login_chat.md` §3).
2. **Lobby sign-in.** `logs/lobby.log`: "… signed in to the lobby from …",
   and the client shows its lobby with the gear. A crash or a hang here is
   the push order or one message's layout (`lobby.md` §3 and the hazards
   H1..H8); the packet log has the last message sent. Watch for "Lobby
   message 0x.. was not sent" and "A lobby message was not read".
3. **0x89's tail.** The inventory now ends with three empty lists
   (`00 00 00`, `lobby.md` §2.9). If the lobby breaks right after the
   inventory, look there first.
4. **Shop, kits, skills.** Buy and move an item; refusals must show the
   game's own text.
5. **Play, two clients.** "Match 0x… made for 2 players on map …", then in
   the client "match found" and the level loading. `logs/match.log`:
   "player N (session …) connected", "has loaded".
6. **With the upload patch only:** "gave the initial world", "starts from
   the snapshot", "is in sync", "the countdown starts", "play begins".
   Lines to watch: "no agreed hash for player N's snapshot … not used"
   (rare with the full patch: a world is trusted only when two clients
   reported its tick), resyncs, "too late to relay and discarded" counts,
   clock corrections.
   **Hash interval.** The clients must report their hash every 50 ms: the
   full patch sets that (`client_patch.md` §3.6); an exe without it needs
   `-hash_check_interval_in_ms=50` on its command line (LIKELY syntax).
   If `logs/match.log` says "on opposite hash phases", a client runs with
   the stock 100 ms. The server then asks two clients for the world of one
   tick every 3 s and compares those (match.md §15): the end and the
   results still come, but a client out of step is found later, and each
   client uploads its world every 3 s.
7. **The end.** "the mode's rule has its end", "over at … (ModeRule)", then
   in `logs/srp.log` "Match 0x… settled (…)", in the lobby log "… is shown
   the results of match …", and the results screen in the client with the
   experience, the money and the drop. Then sign in again: the account must
   be as the screen said.

Known gaps a live session will meet:
- The web page (`servers/web-server`) is the 0.26g one since 2026-10-12
  (corrected; this note said it still worked on the 0.21d balances): its
  admin acts on the 0.26g accounts and tells the lobby
  (`Inbox::account_changed`), and it tells players `-server=`. Its silver
  and gold exchange is gone since 2026-10-16 (it is in the lobby's exchange
  window; `/exchange` only says so). Never seen
  with a real client: whether an account pushed again outside a sign-in,
  and the operator's text (0xA5), show as expected is open.
- Premium, the shop's bundles, the daily bonus, a nickname change,
  autobuy and auto-repair are there (`premium.md`). **There is no
  tutorial since 2026-10-19** (`premium.md` §5): a new account gets the
  daily bonus window at its sign-in, and starts with 10 000 silver and
  500 gold.
- **The lobby on a screen wider than 16:9** (seen 2026-10-12 at 3440x1440:
  the top bar's parts overlap). The lobby SWF is drawn for 1920x1080 and
  has two paths split at aspect 1.85 (`LobbyMenu.onStageResize`: scale by
  width below it, by height above; `StatusPanel.draw` resets the top
  bar's scale only below it). Client-side only; the server has no part.
  The exe has no command-line key for the resolution or for another
  config file (its keys were listed from the key table in `.data`:
  `-all_resolutions`, `-splash_screen`, `-server` and so on); the window
  size is the console variable `r_resolution` of
  `Documents\survarium\user.cfg` (read at 0x1EA355; the file is shared
  with 0.21d). Use a 16:9 size for 0.26g, e.g. `r_resolution 2560x1440`
  with `r_fullscreen off`. Not tried live yet.
- The drop card's request (`46 03`) has its two arguments guessed
  (`results.md` §3); srp takes them in either order.
- A restart of srp ends every match (no results for it) and forgets the
  queue and squads; the results screens not shown yet are kept since
  2026-10-12 (`results.md` §11).

## Verdict

**0.26g can be served, but it is a new protocol at every layer.** No 0.21d
message is wire-compatible with 0.26g. Every message id changed, the lobby
moved from TCP to the match's UDP transport, and that transport was itself
redesigned. What carries over from srp-021d is the server design and logic,
not the wire code:
- the lockstep relay with hash voting, resyncs and majority-decided results;
- matchmaking, squads, sessions, accounts, bans, limits and persistence;
- the reward rules and the web page.

**The match is still lockstep**, so the plan that works for 0.21d still works.
Clients simulate, the server relays inputs, and the world stays frozen until
the first snapshot. Only a patched client can supply that snapshot, so the
**snapshot-upload client patch is still required**, and it is the largest
risk.

| Layer | Change from 0.21d | Work for the server |
|---|---|---|
| Login | First message and the 1-byte reply unchanged; the client takes `-server=host:port`. The password arrives as a SHA-1 hex digest (plus plain text when typed). The reply is 11 bytes pointing at the lobby. Error codes renumbered; presence ping and browser server removed | Small. Replace the bundled certificate |
| Lobby transport | **TCP → UDP**, on the match transport. Sign-in is the connect payload | Shared with the match transport (one implementation) |
| Lobby messages | Every id renumbered, the `0x36 xx` and `0x21 xx` groups flattened into their own ids. One account message with up to 3 kits and a shared item table replaces profiles and contents. Prices and shop LZMA2-compressed. The client never asks for its account or money, so the server must push them after each change | Large: rewrite `lobby.rs` writers and the request parsing |
| Chat, friends, squads | Chat inside the lobby connection; clan tags added; friend lists pushed by the server; squad bodies unchanged | Medium |
| Clans, crafting, upgrades, drops, daily bonus | All new | Optional at first: deny or send empty |
| Match transport | 3-byte header, 8-bit sequence numbers, 128-bit acks, fixed length per id from a table in the exe, no ordered channel, unique-message dedup | Rewrite `transport.rs` |
| Match protocol | New ids; 1-byte tick times; half-float inputs; sync generations on every resync; snapshot has 6 sections instead of 4; new relays (multi-player, moved inputs) | Rewrite `game.rs` encoders and decoders; resync logic carries over |
| Match start | LZMA2-wrapped; a single map id selects a data config that sets mode, timings and rules; always 20 profiles with kits and 68 modifiers | Rewrite the writer (stored LZMA2 chunks are enough, no encoder needed) |
| Game data | Same binary config format (srp's parser reads all of it). 384 items instead of 83, a new skills tree, item wear and mods, new drops, renumbered quest types, 8 maps, mode 4 Team Deathmatch | Rewrite the dictionary, skills and drops parsers; regenerate the quest and item tables |
| Results | Same shape plus team values, game mode, clan tags; the full player result redesigned | Medium |
| Client patch | Mutex patch ported. Snapshot upload: hook sites and every function it calls found. Batteries patch no longer needed (0.26g fixed it; confirm live) | Finish and test the snapshot patch |

## Biggest risks

1. **The snapshot-upload patch.** Without it no match can start. The hook
   sites are proven, but the code cave is too small (0x5D bytes free, ~369
   needed). The draft overwrites a debug-only function
   (`game_world_core::on_hash_mismatch`), which costs only the patched
   client's `-debug_hash_mismatch` option. Adding a new PE section would be
   cleaner. Nothing is tested yet.
2. **The client checks almost nothing.** It does no bounds checks, has no
   range check on lobby ids (any id outside 0x80..0xAA, or 0x81/0x86/0xA0, is
   a jump to a bad address), and has fixed-length message tables. One wrong
   byte either crashes the client or silently drops a whole datagram. See
   `lobby.md` §3 for the full hazard list and the push order. Examples:
   inventory 0x89 must come before account 0x87; the 0x85 search step must be
   non-zero.
3. **Lobby and match share one new transport.** A bug there breaks
   everything at once, but it also means one implementation serves both.
4. **No symbols.** Game code is only ~30% matched automatically (network
   code ~18%), because 0.26g was compiled differently. New questions mean
   more manual reading.
5. **1-byte tick times** wrap every 256 ticks. Any time more than about 6 s
   off the client's clock is read as the wrong tick with no error.

## Suggested order of work

The detailed version, with the code, tests and done criteria of each step,
is `E:\sur-res-pro\PLAN-026G.md`.


1. **Transport** (`transport.md`), with unit tests built from its worked
   bytes. Both the lobby and matches depend on it.
2. **Login** (`login_chat.md` §6): new password message, 11-byte reply,
   renumbered codes, no browser server.
3. **Lobby minimum**: sign-in and the push order from `lobby.md` §3, with
   empty clans, drops, bundles and quests. Goal: the stock client reaches the
   lobby and shows its gear.
4. **0.26g data**: dictionary, items, skills, maps (`data.md`,
   `modes_maps.md`).
5. **Client patch**: finish `patch_026g.py` and test the snapshot upload on
   one PC with two clients (the mutex patch is ready).
6. **Match**: static info, relay, hash votes, resyncs with sync generations
   (`match.md`, `static_info.md`). Map choice by map id; modes 2, 3 and 4.
7. **Results and rewards** (`results.md`), then chat and squads
   (`social.md`).
8. Optional later: clans, crafting, upgrades, drops, daily bonus.

## Open questions that matter most

- Whether the lobby push order in `lobby.md` §3 is enough for a stock
  client to reach the lobby (only a live test can tell).
- Whether a standard raw LZMA2 stream with stored chunks decodes in the
  client (0x8A, 0x95, 0xE4).
- What `32 00 <flag>` after sign-in asks for, and what the flag in 0x80
  means.
- What the server must put in a kit's modifier block so every client
  computes the same values (the 68 names and the bit order are known:
  static_info.md §3.1).
- The lobby's ordered channel: how the client handles a multipart ordered
  message (transport.md §4.1). Inventory and account pushes depend on it.
- Whether the original server moved late inputs (0xEF) or discarded them
  (0xF1). What the client does with each is known (match.md §16); srp
  discards them, `SRP_LATE_INPUTS=move` is the other way.
- The `wait_player_percent` 0.8 start rule: does a match with fewer players
  than the roster wait forever?

Each note ends with its own full list of open questions.
