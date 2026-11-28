# Config file, start command, events (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

The server takes all its settings from **one config file** (TOML). The documented example with every key, its
default and its meaning is `config/srp.example.toml`; the code is `servers/srp/src/settings.rs` (a unit test checks that every value
in the example IS the default, and that its commented example entries are valid). Every feature is always on: there are no
on/off switches (section 4).

## 1. Starting the host

```
srp setup --game <game folder> [--address IP]   # checks; writes srp.toml (the example, filled in), database, login key
srp --config srp.toml check                 # what is missing, the effective settings, the generated daemon config
srp --config srp.toml selftest              # a world host of its own builds one start world and plays a bot match: OK or the failing part
scripts/srp-host.sh start srp.toml          # background; or `run` in the foreground (WSL2: keep that call attached)
scripts/srp-host.sh stop srp.toml
```

* `srp setup` (`servers/srp/src/setup.rs`): the system (Linux / WSL 2; WSL 1 refused), the game folder (`--game`, the
  config's `world_host.exe`, or the usual Steam folders and the libraries of Steam's `libraryfolders.vdf`), the exe build
  (size 13,386,392 and SHA-256 `1e1ae04d...65448`; any other file refused, also by `check`, `run`, `world-host` and
  `selftest`), the `resources.NN.db` files readable, the world host binary and pre-warm lists, the four ports, memory
  (MemAvailable against 0.35 GB + `max_zygotes` x (0.35 GB + `max_matches` x 0.165 GB), the worst case, and the
  number of usual PvP maps that fit: guide-server.md, "Memory: how many maps and matches") and disk. It writes
  `srp.toml` only when missing (from the example; `world_host.exe`, the pre-warm folder of a source tree and the
  network keys filled in) and otherwise changes only `world_host.exe` (with `--game`) and the network keys (with `--address`), keeping every comment. Without `--address` a new
  config gets this computer's own address (the route's source address) with every bind on 0.0.0.0. Then it creates the
  database and the login key (RSA-1024 PKCS#1 PEM, mode 600) and prints the launch line. `srp run` also generates a
  missing key and logs the launch line at every start.
* `srp selftest [PROJECT]`: a daemon of its own (`<run_dir>/selftest`, `<log_dir>/selftest`, `max_zygotes` 1, no prestart)
  builds the start world of the first served PvP project (or PROJECT) for one placeholder human, as the match server asks for it,
  then plays 30 s of a bot match on that project (`world_host::bot_match`: the host's AI drives five AI players; skipped
  for the gun race, which has none).
* `srp account add|password <name> <password>`: an account as its first sign-in would make it / a new password; the stored
  password is the client digest of the password (`login.md`), so both sign-in paths work.

* `srp` (or `srp run`) starts, in this order: the world host daemon (only when none answers on `world_host.socket` and
  `world_host.exe` is set), then login, lobby and match server. A daemon srp started stops with srp (it gets SIGTERM when srp
  ends, also on a crash). A daemon that already runs is used as it is, so `srp world-host` in another terminal (or a daemon started
  any other way on the same socket) survives srp restarts.
* The daemon's config is **generated** at every start (`<run_dir>/daemon.cfg`) from the `[world_host]` settings. It names no
  project: the daemon serves the projects of the game data (`GameData::host_projects`: every PvP row whose level ships in the
  client, 111 in the 0.69d data (107 + the four Search and Destroy layouts), plus the six PvE mazes). srp asks the daemon for that list (`LIST_PROJECTS` -> `Projects`, a
  query on its own connection; `world_host::fetch_projects`), so the PvP queue offers exactly what the daemon serves; no second
  project setting exists. A daemon config with `[project]` sections (hand-written) still works and is then what srp is told.
* srp also reads the daemon's room about every 0.5 s (`LIST_STATUS` -> `Status`, `world_host::watch_room`): the loaded
  maps with their matches and room, and whether a new map can load. The queues start only matches that fit it
  (`online.md` 7.3a); without an answer they pick as before.
* Logs, all under `files.log_dir`: `srp.log` (when started by `scripts/srp-host.sh`), `world-host.log` (the daemon's output),
  `world-host/` (the daemon's `front.log`, `base/b<ms>/host.log` of each base process (the start-up stages up to the
  level), one folder per project / zygote / match; a zygote's `host.log` starts at its fork from the base).
* The daemon runs one base process besides its zygotes: it does the level-independent start-up once and forks every
  zygote from it, so the zygotes share those pages (`online.md`, "World host memory"). It is not a map: `max_zygotes`
  does not count it, and the memory rules read MemAvailable, which includes it. One that ends is started again within
  5 s, and a ready one older than 6 hours is replaced while no map is being loaded; running maps go on without it, and
  a map that needs loading meanwhile (or while the base refuses a fork for now) waits up to 30 s. Needs Linux 5.3 or
  later (pidfd).
* Linux and WSL2: the exe's Windows path (`world_host.exe_win`) is derived: an exe under `/mnt/<drive>/` has `<DRIVE>:\...`;
  an exe in any other `<game>/binaries/x64/` (native Linux) is shown as `G:\binaries\x64\survarium.exe`. The world host maps
  the Windows folder in front of the common tail of `exe` and `exe_win` to the Linux folder in front of it
  (`files::game_mapping`, logged as `game folder G: = <folder>` in host.log); any other drive `X:` stays `/mnt/x`. Tested
  through a symlinked game folder outside `/mnt` (selftest: start world built); a real ext4 copy (case-sensitive
  file names; reads fall back to a case-insensitive lookup) is to be confirmed.

### 1.1 Pre-warm lists

A zygote (one per level project) queries the shared resources its matches will ask for before it forks them, so every
match child finds them loaded and shares their pages. Without a list a PvP match child holds about 60-90 MB more and its
creation takes about 0.5-0.8 s more (live br matches 127-139 MB at creation, pre-warmed ones 37-58 MB). The lists are game
data: they are not shipped; live matches make them (`servers/world-host/src/daemon.rs`, `bp.rs`).

* **File:** `<prewarm_dir>/<project>.tsv`, one `0xCLASS<TAB>name` line per resource of the classes a zygote can load by
  name alone (configs 1F, animations 31, 2C, 3, E, F, 13, 14, D4, 8B; item, weapon-module and combined-model classes need a
  match's user data and stay with the children).
* **Which list a zygote loads** (`DaemonCfg::prewarm_pick`): a configured `prewarm` file; else the project's own list
  when it has at least as many entries as the best variant's; else the list with the most entries among its variants: the projects whose name is
  the same without a trailing `_v<digits>`, on the same level folder (`level_02_day_br_v2` takes `level_02_day_br_v3` or `_v4`; `level_01_day_tdm_v1`
  takes `level_01_day_tdm`). Other modes, other levels and the PvE mazes never stand in for each other. Else none.
* **Merge before a zygote starts** (`DaemonCfg::seed_prewarm`): the front first merges the best variant's list into the
  project's own `prewarm_dir` list (the union under the list's lock, own entries first, written only when something is
  new), so from its first zygote on a project loads its own list and never less than its most-played variant's.
* **Common seed** (`DaemonCfg::seed_common`, always on): a project with neither an own list nor a variant's list (on a
  new server every project's first visit, and most projects of a running one) starts from the entries common to every
  list of the other projects of its kind (PvP maps, or PvE missions: level folder `pve_...`), at least two lists, in the
  first list's order. They are written as its own list, which its matches then grow as any own list. On the recorded
  lists the PvP common part is about 710 entries (player and weapon configs, animations). Measured on a recorded
  two-player PvP match: 70 MB at its creation with the seed, 98 MB without any list, 39 MB with the map's own list
  (the zygote itself 61 MB without a list, 95 MB with the seed, 115 MB with the own list); ticks and captures equal
  to the golden run in all three. Logged by the front as `front: pre-warm list <file>: common seed of N PvP|PvE
  lists, K entries (E in the list, A new)` (or `... no common seed (fewer than 2 ... lists)` / `... no entry in
  common`), then `pre-warm list: common seed of N lists, K entries (file)` in its `started through the base` line and
  in the zygote's `zygote: pre-warm list: ...`. Diagnosis: `HOST_NO_COMMON_SEED=1` in the world host's environment
  leaves such projects without a list (A/B runs; the front logs `no common seed (diagnosis HOST_NO_COMMON_SEED=1)`).
* **Growth:** every match creation logs its resource requests (a hardware breakpoint at the query creation 0x2000BC810,
  armed in the match child only for its `pvp_match_resource` query and closed right after, so the kill log and the bots'
  breakpoints keep their debug registers) and adds the resources the list lacks to the project's own file: the union,
  never a replacement. The write happens under a lock (`<project>.tsv.lock`), through a temporary name and a rename, and
  only when something is new; an empty list is never written. A zygote reads its list once at start and then, on its
  idle turns, queries what the file gained since (the top-up below), so a grown list serves the running zygote's next
  matches and every zygote started after it. A project with a list set by hand in a `[project]` section (`prewarm = FILE`) uses
  that file as it is and records nothing.
* **Trace:** the front logs `front: zygote for P (...) started through the base, pid N, in T ms; pre-warm list:
  own|sibling Q|common seed of N lists|none, E entries (file)`;
  the zygote `zygote: pre-warm list: <source>, E entries (K kept, M empty) from <file>` or `none`; every match child
  `child: the zygote's pre-warm list: ...` and, after its creation, `pre-warm list recorded: <file>: E entries, A new (this
  creation R resources of Q requests)` or `pre-warm list <file>: nothing new (...)`.
* **Why a variant's list, a common seed or a grown list leaves every match byte unchanged:** the zygote queries the
  entries through the same exe query as an own list (`run::prewarm`), before the fork, with the project's own level loaded; an entry the match
  also asks for is cooked in exactly the state an own list would cook it in, and an entry it never asks for only stays in
  the resource cache. Every recorded list already carries such entries for other matches (they come from other loadouts),
  and a recorded run supports it: a recorded two-player PvP match on level_01 replayed through the daemon with its list
  grown by 17 resources that match never asked for (1442 kept against 1425) wrote ticks.tsv files of the same length (4548
  ticks) whose per-tick hashes are equal at every tick compared (start, middle and the last tick) to the run with its own list;
  the daemon runs of the common seed, the top-up and the base pre-warm set (each a list with entries the match never
  asks for) gave whole ticks files and captures equal to the golden runs. Nothing in the exe walks the resource
  cache: its name table (0x200C71270) is reached only by the find, insert and erase of one (class, name) key, and its
  entry count only by insert and erase, so an entry no match asks for can only be passed over as one more node of a bucket chain during another name's lookup. Only memory
  (the zygote holds the extra resources once) and time change.
* **Late loads:** a match child also watches the cache after its creation (`online.md`, "Late loads"): what it
  registers later is logged (`late load: ...`, one `late loads: K after the creation (...)` line per match, also when
  K is 0) and joins the project's own list through the same union at the match's end. It only reads the cache (under
  the exe's own lock, taken and given back with the calls the exe itself makes), so it changes no byte the exe reads.
  Diagnosis: `HOST_LATE_TRACE=0` turns it off.
* **Top-up:** on an idle turn (no hand-over for a second; no child forked in the last 5 s; MemAvailable at least 1.2
  GB) a zygote queries the entries its project's own list gained since it read it, in the file's order, through the
  same query as at start, at most 200 ms per turn and 48 MB in its life (`online.md`, "Pre-warm top-up"). It is exact
  by the argument above, applied at a later time: the same exe query, the project's own level loaded, before the fork
  of every match that finds the entries; growth only appends, so a zygote that loaded its own list has then queried
  the same entries in the same order as a zygote started with the grown list. Logged as `zygote: pre-warm top-up: N
  new entries (K kept, E empty) in T ms; list now L (...)`. Diagnosis: `HOST_PREWARM_TOPUP=0` turns it off.
  Measured: the second match on a zygote started from the common seed held 39 MB at its creation instead of 70 MB
  (723 entries topped up in 196 ms); ticks and captures equal to the golden runs with and without the top-up.
* **Base pre-warm set** (always on; `online.md`, "Base pre-warm set"): the entries common to the projects' own lists
  (at least two lists, and a share of the lists of at least 1.5 / `max_zygotes` when `max_zygotes` is set) are queried
  once in the base process before the cut, so every map shares them instead of holding its own copy (measured: the
  base about 47 MB more, once; each PvP and PvE zygote about 51 MB less); a zygote's pre-warm and top-up skip them.
  **Why exact:** the one thing a set queried before the level can change in the level load is that an entry it asks
  for is found in the cache instead of being cooked. The base records which cache entries the set added, and every zygote watches each entry its level load finds (a breakpoint at the
  lookup's branch for a found entry, until its own pre-warm starts); when one of the set's is found the zygote serves
  no match, writes the set entries behind it to `base-exclude.tsv` (in `prewarm_dir`, else in `out`) and starts again
  on its own without the set; the front then starts a new base without them. So every zygote that serves matches had
  a level load that met the cache as without the set; what else moves (addresses, a later clock) gave equal start
  worlds on all 118 served projects with the common entries queried right before the level. The exclusion file starts
  empty; the entries a level load asks for are found by this check the first time. Checked: ticks and captures equal
  to the golden runs with the set and with `HOST_BASE_PREWARM=0`, and no zygote's check found a set entry. Lines:
  `front: base pre-warm set: N entries (...)`, `base pre-warm: N entries (K kept, E empty) ...`, `MEM [base: after
  the base pre-warm]`, `zygote: level-load check: ...`, `zygote: pre-warm list: ... (K kept, E empty; S in the base, M loaded
  here) ...`. Diagnosis: `HOST_BASE_PREWARM=0` turns it off.

## 2. Settings

| Section | Keys (default) |
|---|---|
| `[network]` | `login` (127.0.0.1:65000), `lobby` (127.0.0.1:1235), `lobby_public` ("" = lobby), `match` (127.0.0.1:1236), `match_public` ("" = the match IP), `update` ("" = the login IP, port 65001) |
| `[files]` | `database` (srp-069d.sqlite), `login_key` (login_key.pem), `data_dir` (data: the game tables `srp setup` builds; required), `run_dir` (run), `log_dir` (logs) |
| `[accounts]` | `auto_register` (true), `client_version` ("" = any), `start_money` (absent = the real start without money) |
| `[world_host]` | `socket` (run/world-host.sock), `exe` ("" = srp starts no daemon), `exe_win` (derived), `bin` (world-host next to srp), `prewarm_dir` ("" = data_dir/prewarm; every match grows its project's list there, section 1.1), `max_matches` (4; more, up to twice it, while MemAvailable is 1.2 GB, else the match is refused), `max_zygotes` (8: a hard limit that counts a map still unloading (one still alive 10 s after unloading is killed); idle maps are unloaded for a new one, least recently used first, prestart maps last; busy maps never; with every map busy one more loads over it if MemAvailable is 1.4 GB, else the match waits 20 s and is refused; the queues read these limits from the daemon's room status (the last one is used for up to 5 s) and narrow to the loaded maps with room or wait in the queue instead; while no map can load the daemon names one busy map to drain (the longest loaded, not a prestart map, at most 20 min each), which the queues avoid so it can be unloaded for the rotation's next map, `online.md` 7.3a), `idle_secs` (600), `prestart` ([]: loaded once the base process is ready) |
| `[matches]` | `min_players` (2), `bot_wait_secs` (30, 0 = never), `bots_per_team` (6, 1..=8: players per team of a bot-filled match and a custom room's AI maximum, also capped by half the map's match size and its 8 patrol graphs per team), `report_grace_secs` (20), `equipment_use_costs` (0.55 repair prices), `match_silver_factors` (1, 1, 3.5, 7, 10, 14) |
| `[social]` | `claims_per_day`, `compliments_per_day` (0 = no daily maximum) |
| `[league]` | `tokens` ([] = 25 x league^2), `[[league.seasons]]` (none = monthly seasons) |
| `[store]` | `[[store.tabs]]`, `[[store.offers]]`, `[[store.premium]]` (none = the built-in tab, cards and premium offers) |
| `[[game_servers]]` | none = one entry "Skhid" on the match host |
| `[[login_calendar]]` | none = the built-in 7-day calendar |
| `[[events]]` | none = no event (section 3) |
| `[[faction_battles]]` | none = no faction battle |

Relative paths are relative to the config file's folder. Times are UTC dates (`2026-12-24T12:00:00Z`, `2026-12-24`) or unix
seconds. Unknown keys are refused with the key's name, so a typo cannot silently fall back to a default.

The sign-in of a client started with `-steam` (own name + password in the client's Steam Import panel) has no setting either:
it is always on and uses the `[accounts]` settings like the normal sign-in (`login.md`).

`network.update` is the TCP address of the update check that the Steam build's launcher asks before it turns its Play
button on (always on; `steam.md`). Players point their launcher at it with `scripts/steam-play/steam-play-setup.cmd`; `srp
check` prints the exact command and the file it writes. Any port works (the launcher takes `host:port`); bind it to a
reachable address and open the TCP port like the login port. The same port serves `GET /skhid/client.txt`, the player start
data (`format`, `login_host`, `login_port` as `key=value` lines; public values only, no login key: the game has no
argument for one and takes the key from the login server's `0D` at every sign-in) that
`scripts/player/skhid-setup.cmd` turns into the player's start file `Skhid.cmd`; it has no setting of its own.

## 3. Events

The 0.69d files carry no dates, so an event runs only when the config schedules it: **no entry = no event**. The example file has
a commented `[[events]]` entry to copy and date:

```toml
[[events]]
event = "new_year"                    # the only event the 0.69d data has currencies, drops and exchange rows for
start = 2026-12-24T12:00:00Z          # matches pay snowflakes / spruce trees / stars from here ...
end = 2027-01-10T12:00:00Z            # ... until here (not included)
exchange_end = 2027-01-17T12:00:00Z   # the event window and the exchange stay open until here (>= end; default = end)
header = "st_merry_christmas"         # window header, a client text key ("" = this default)
text = "st_global_event_currency2_descr"
```

* At start srp logs one line per entry: `event new_year: 2026-12-24 12:00 UTC .. 2027-01-10 12:00 UTC, exchange until ...
  (RUNNING now | starts in N h | drops over, exchange window open | over)`, or `event: no event scheduled`. `srp check` prints the
  same lines.
* Several entries are allowed (e.g. this year and next); they must not overlap (the next starts after the previous exchange end).
  The entry whose window is open at a moment is the running one (`LobbyConfig::event_at`).
* What an event does (drops, the window, the exchange, the first-win hat, ranked closed during the event): `events.md`.

## 4. Environment variables

srp reads only the config file, plus `SRP_CONFIG` (its path), `RUST_LOG` and the debug variable `SRP_RECORD_DIR`. Every
feature is always on; pockets are charged, a second sign-in of an account is refused, and the world host builds every start
world.

Test-only knobs that stay in the code, never in the config file: `LobbyConfig::{achievement_rewards, daily_quests, treasure_hunt,
hero_of_the_day, online_queue}` (tests that count messages turn the features off), `MatchConfig::{pve_handover, rejoin}` and
`HubConfig::discipline_clock` (tests drive the hub on virtual instants; srp always sets it), the world host's `HOST_*` debug
variables, and the ignored end-to-end tests' `SRP_E2E_*` / `SRP_PROBE_*` / `SRP_CLIENT94_*`.

## 5. Open points

* Native Linux on a real ext4 copy of the game (not through a link to a Windows drive): to be confirmed (section 1).
* The WSL 2 NAT address written by `srp setup` changes when Windows restarts; srp does not notice (the operator re-runs
  `srp setup --address`, or uses mirrored networking).
