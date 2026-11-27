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
  (MemAvailable against `max_zygotes` x 0.56 GB + 0.5 GB) and disk. It writes `srp.toml` only when missing (from the example;
  `world_host.exe`, the pre-warm folder of a source tree and the network keys filled in) and otherwise changes only
  `world_host.exe` (with `--game`) and the network keys (with `--address`), keeping every comment. Without `--address` a new
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
* Logs, all under `files.log_dir`: `srp.log` (when started by `scripts/srp-host.sh`), `world-host.log` (the daemon's output),
  `world-host/` (the daemon's `front.log`, one folder per project / zygote / match).
* Linux and WSL2: the exe's Windows path (`world_host.exe_win`) is derived: an exe under `/mnt/<drive>/` has `<DRIVE>:\...`;
  an exe in any other `<game>/binaries/x64/` (native Linux) is shown as `G:\binaries\x64\survarium.exe`. The world host maps
  the Windows folder in front of the common tail of `exe` and `exe_win` to the Linux folder in front of it
  (`files::game_mapping`, logged as `game folder G: = <folder>` in host.log); any other drive `X:` stays `/mnt/x`. Tested
  through a symlinked game folder outside `/mnt` (selftest: start world built); a real ext4 copy (case-sensitive
  file names; reads fall back to a case-insensitive lookup) is to be confirmed.

## 2. Settings

| Section | Keys (default) |
|---|---|
| `[network]` | `login` (127.0.0.1:65000), `lobby` (127.0.0.1:1235), `lobby_public` ("" = lobby), `match` (127.0.0.1:1236), `match_public` ("" = the match IP), `update` ("" = the login IP, port 65001) |
| `[files]` | `database` (srp-069d.sqlite), `login_key` (login_key.pem), `data_dir` (data: the game tables `srp setup` builds; required), `run_dir` (run), `log_dir` (logs) |
| `[accounts]` | `auto_register` (true), `client_version` ("" = any), `start_money` (absent = the real start without money) |
| `[world_host]` | `socket` (run/world-host.sock), `exe` ("" = srp starts no daemon), `exe_win` (derived), `bin` (world-host next to srp), `prewarm_dir` ("" = data_dir/prewarm; the world host records missing lists there), `max_matches` (4), `max_zygotes` (8), `idle_secs` (600), `prestart` (["pve_01_v1_maze2"]) |
| `[matches]` | `min_players` (2), `bot_wait_secs` (30, 0 = never), `report_grace_secs` (20), `equipment_use_costs` (0.55 repair prices), `match_silver_factors` (1, 1, 3.5, 7, 10, 14) |
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
