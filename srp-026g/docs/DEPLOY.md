# Hosting the 0.26g server in Docker

> **Version scope: the 0.26g server** (`srp-026g`). General steps for any
> Linux host; nothing here is about a particular machine.

## The files

| File | What it does |
|---|---|
| `Dockerfile` | Builds `srp` (the 0.26g login server, lobby, match server and web page in one process) in a Rust image and copies the binary and `certs/` into a small Debian image. Runs as a user without rights. The cargo caches are build cache mounts, so a rebuild compiles only what changed (about 1.5 minutes on one CPU; the first build about 6) |
| `docker-compose.yml` | Compose project `srp26`: the service `srp26` and, with the profile `tls`, `caddy26` (HTTPS for the web page). Ports, the data volume, the settings passed to srp, the container's limits |
| `.dockerignore` | Keeps build output, data, logs, docs and the client patches out of the build |
| `deploy/env-026g.example` | The settings to put into `.env` beside the compose file |

## What the host needs

- Linux with Docker and the compose plugin; 1 CPU and 1 GB of RAM are enough
  for a small server (the container's limit is 512 MB).
- A public **IPv4** address, or a name with an A record: the 0.26g client is
  told the lobby and the match server as IPv4 addresses (a name is looked up
  once, at start).
- These ports open to players: the login port as **TCP**, the lobby and
  match ports as **UDP**; **443/tcp** if the web page is to be public. 0.26g
  has no browser server on port 80 and no presence pings.

## The ports

The client is told the ports srp binds, so each game port must be the same
number inside and outside the container. Change `SRP26_LOGIN_PORT`,
`SRP26_LOBBY_PORT` and `SRP26_MATCH_PORT`; never map one number to another.
Voice chat is always on the match port + 1 (the games send there; srp binds
it): `SRP26_VOICE_PORT` must be `SRP26_MATCH_PORT` + 1. `release/release.sh`'s
preflight stops when it is not.

| | Default of the compose file | On a host of its own |
|---|---|---|
| Login (TCP, TLS) | 2234 | 1234 |
| Lobby (UDP) | 2235 | 1235 |
| Match server (UDP) | 2236 | 1236 |
| Voice chat (UDP, the match port + 1; `SRP_VOICE=off` for none) | 2237 | 1237 |
| Web page | `127.0.0.1:8098` | the same, or public through `caddy26` on 443 |

The defaults are a block of their own so that the stack can run beside a
0.21d server on one host without sharing a name, a volume or a port with it
(that server has 1234 tcp+udp, 1235 tcp, 1236 udp, 80 and 443). Beside it,
do not start `caddy26`: port 443 is the other server's. On a host of its own
use the game's own 1234, 1235, 1236 and 1237 for voice.

Open every game port in the host's firewall: the login port for TCP, the
lobby, match and voice ports for UDP. A voice port that is not open leaves
voice silent and nothing else: the games keep playing.

## First start

1. Copy this folder to the host.
2. Copy `deploy/env-026g.example` to `.env` beside `docker-compose.yml` and
   fill it in. `SRP_PUBLIC_HOST` is required. Keep `.env` out of any
   repository: it holds the admin page's token and path.
3. Put the players' download (the zip made as `client-patches/README.md`
   says) into `./download`, or set `SRP_DOWNLOAD_URL`. A server whose
   players had a ZIP with resource regions (before the patches) keeps
   `./download/resources/` as it is: the originals there are what the
   updater fetches (`/resources/<sha256>`) to put a player's `resources.db`
   back to stock once, when it kept no copy of its own. A new server needs
   no such folder.
4. Put the server's game data (`server-game-data.db`, made by
   `srp-game-data build` of `resources.db` and the patches the players
   mount: "The game's files" below) into `./download` too, and each patch
   at the URL it names. srp takes its game data from it, lists its patches
   at `/patches`, and does not start without it.
5. `docker compose config` (checks the file and the settings), then
   `docker compose build srp26`, then `docker compose up -d srp26`.
6. For HTTPS on the web page, on a host whose port 443 is free: point a
   domain at the host, set `SRP_WEB_DOMAIN` (several names: commas between
   them, the whole in quotes) and `SRP_WEB_TRUST_PROXY=on`,
   then `docker compose --profile tls up -d`.
7. Check: `docker compose ps` shows the container healthy (the check opens
   the web page's port; the game ports are UDP), and the log's first lines
   say which servers started on which ports, and where the game data was
   read from (`game data: ...`).

Players start the game with `-server=<SRP_PUBLIC_HOST>:<login port>`
(`client-patches/README.md`).

## Settings

`deploy/env-026g.example` lists them with comments. The compose file's own:

| Setting | What |
|---|---|
| `SRP_PUBLIC_HOST` | The address players reach the host at. Required |
| `SRP26_LOGIN_PORT`, `SRP26_LOBBY_PORT`, `SRP26_MATCH_PORT`, `SRP26_VOICE_PORT` | The game ports (above; the voice port is the match port + 1) |
| `SRP26_WEB_PORT` | The web page's port on the host's 127.0.0.1 (default 8098) |
| `SRP26_HTTPS_PORT` | caddy's port (default 443) |
| `SRP26_DOWNLOAD_DIR` | The folder with the download (default `./download`) |
| `SRP_GAME_DATA` | The server's game data (default `/srv/srp/download/server-game-data.db`: `./download/server-game-data.db` on the host) |
| `SRP26_CPUS`, `SRP26_MEMORY` | The container's limits (1 CPU, 512 MB) |
| `SRP_ADMIN_TOKEN`, `SRP_ADMIN_PATH`, `SRP_ADMIN_IPS`, `SRP_WEB_TRUST_PROXY`, `SRP_WEB_DOMAIN` | The admin page and the web page, as for 0.21d: the admin page is on only with a token and a path, for the listed addresses only |
| `SRP_DOWNLOAD_FILE`, `SRP_DOWNLOAD_URL`, `SRP_DOWNLOAD_SHA256`, `SRP_GAME_URL`, `SRP_OTHER_SERVER_URL`, `SRP_TEST_SERVER` | The home page's download, where the original game is had, the link to another server of ours, and `on` for a test server: every page then says that it is one and names the other server as the players' |

**Only the settings named under `environment:` in the compose file reach the
server.** A game setting of `SRP-026G.md` that is not in that list keeps its
default even when `.env` sets it: add its name to the list first. Every
setting of `deploy/env-026g.example` is in it, except `SRP_BOT_ACCOUNTS`.

**A changed `.env` takes effect only in a new container:**

    docker compose up -d --no-deps --force-recreate srp26

`docker compose restart srp26` starts the same container again, with the
settings it was made with: a new value in `.env` is not seen.

Never set `SRP_BOT_ACCOUNTS` on a public server (`SRP-026G.md`, "Test
bots"); the compose file does not pass it.

A download that every game must have (new content in `resources.db`, a
match rule older games compute otherwise) goes with the server refusing
older games (`.env`):

    SRP_MIN_PATCH_REVISION=<the download's revision>

An older game is then refused at its sign-in with the game's own "You're
using an outdated version of the game.", and a game that reconnects after
the restart with a session from before is refused in the lobby: it goes to
its login screen within about half a minute, and is refused there. It never
gets the lobby's sign-in, whose price list names every item sold.
Left out (or 0), every game is let in; a value that is not a number from 0
to 255 stops srp at start.

It must be at least the number of the newest patch of the server's game
data (`resources.db.patch_<n>` is released with revision n, and a game of
an older revision does not have it): with less, srp does not start, and its
log says what to set:

    srp: SRP_MIN_PATCH_REVISION is 15, but the server's game data (...) has resources.db.patch_16: ... Set SRP_MIN_PATCH_REVISION=16 in .env and recreate the container (...)

## The game's files

srp takes every table of game data from the server's game data
(`SRP-026G.md`, "Game data"): one file of about 3 MB,
`server-game-data.db`, the files srp reads of the stock `resources.db` and
the `resources.db.patch_<n>` archives the players' game mounts over it, with
what it was made of. `srp-game-data build` makes it where the game's
archives are (the release machine; `crates/resources-db/README.md`):

    srp-game-data build --dir <the game folder> --patch-url <where the players download the patches> --out server-game-data.db

The host keeps it in the download folder (mounted already):

    /opt/srp-026g/download/server-game-data.db    (SRP_GAME_DATA=/srv/srp/download/server-game-data.db)

It names each patch with its sha256 and the URL given to `build`, and the
web page hands exactly that list to the players' updater at `GET /patches`
(public, no token; `{}` when there are no patches). So the players fetch
the patches the server runs on and no others. srp does not serve the patch
files themselves: each must be downloadable at its URL (any plain file
server; the updater checks it against the list's sha256). The host needs
neither the 6.3 GB `resources.db` nor the patches to run srp.

srp does not start, and its log (a line `srp: ...`) says why, when:

- `SRP_GAME_DATA` is unset, or names a file that is missing, damaged (a
  file unlike its table) or not made by `srp-game-data build`: the line
  says how to make it;
- the data lacks an item the code names (the starter kit, the shop's
  tables, the bundles, the vouchers, the starting ammo and the tutorial's awards):
  `... lacks what srp's code names: loadouts::STARTER tt_33, ...`;
- `SRP_MIN_PATCH_REVISION` is under its newest patch (above);
- `srp.db` names an item, a modification or a quest the data lacks: `srp:
  cannot load the 0.26g accounts: the kept accounts name what the server's
  game data (...) lacks; item dict ids 812 (account 3, account 7), ...`.
  The data was made without a patch the players' items come from: build it
  of every patch released and start again. Nothing of the accounts is
  changed: srp never throws an item away because the data does not know it.

Each is a release's mistake, never a player's: put the old
`server-game-data.db` (and the old `SRP_MIN_PATCH_REVISION`) back, or fix
the new one, and recreate the container.

- **It is read once, at start.** A release that adds or changes a patch
  goes out in this order: the patch file at its URL, then, with srp26
  stopped, the new `server-game-data.db` and the players' ZIP of that
  revision over the old ones in `download/` and `SRP_MIN_PATCH_REVISION` in
  `.env` set to the release's revision, then a new container. The ZIP
  goes in with them, never after the start: the site serves whatever ZIP is
  in `download/` at once, and the old ZIP beside the new `/patches` and
  revision lets a launcher enable PLAY for a game the login server refuses
  (`release/lib.sh`, `swap_server`; the release script does this). The
  container is made anew (`docker compose up -d --no-deps --force-recreate
  srp26`; `restart` would keep the old setting). The log's
  `game data: ...` line names what it was made of; `curl
  https://<the page>/patches` shows the list the updaters now get, and a
  player's launcher fetches the new patch at its next start (an open
  launcher starts itself anew within seconds and does it). Every patch
  goes with `SRP_MIN_PATCH_REVISION` (above): srp does not start without.
- **Before the new container:** `srp-game-data info server-game-data.db`
  checks the file and prints its patches and URLs; `srp-game-data check
  server-game-data.db --dir <the game folder> --patch-url <base>` says
  whether it was made of exactly those archives.
- **A rollback** is the old `server-game-data.db`, with the
  `SRP_MIN_PATCH_REVISION` of the release it goes with, and a new container:
  the updaters then delete the patch the list no longer names. Once a
  player was given an item of the patch (a drop, the shop), the old data
  lacks it and srp does not start on it (above): go forward with a fixed
  patch instead.

## Data, logs, backups

- The volume `srp26-data` holds `srp.db` (SQLite: accounts, sessions, the
  0.26g tables), `backups/` (a copy of the database at every start),
  `snapshots/` (worlds the match server accepted, kept to compare) and
  `logs/`.
- `logs/srp-YYYY-MM-DD.log`: every line srp prints, one file per UTC day,
  added to across restarts, deleted after `SRP_LOG_KEEP_DAYS` (14). A line
  has the time of day only; the date is the file's name. About 0.3 MB for a
  day of 14 matches at `RUST_LOG=info`.
- `docker compose logs srp26` is the running container's output only; an
  update makes the container anew.
- The logs and the database are in one volume: a full disk stops both.
  Nothing copies them off the host. Do that yourself.
- Passwords and their digests are in no log line.

## Updating

1. Run the tests (`cargo test --workspace`).
2. Copy the changed source to the host.
3. When no match runs: `docker compose build srp26`. On a host with one CPU
   a build takes from a running match.
4. Still no match: `docker compose up -d srp26`. A match runs while the log
   has more `Match 0x.. made` lines than `Match 0x.. is over` and `no client
   came; given up` lines together.
5. Check the log: no error, the schema line if the database changed, players
   signing in again.

A restart ends every match (no results for it) and drops every lobby link.
Clients sign in again by themselves within seconds; the queue and squads are
lost; results screens not shown yet are kept.

## What the container is allowed

Read-only root file system, every capability dropped, no new privileges, the
memory and CPU limits above, 512 processes. The web page inside is plain
HTTP and is published on the host's 127.0.0.1 only: reach it through caddy
or an SSH tunnel, never publish that port (it carries passwords).

## Public stats and deploying the endpoint

`GET /api/stats` on the existing 0.26g web listener returns JSON without
login or an admin token, with `Content-Type: application/json` and
`Cache-Control: no-store`. For example (illustrative counts):

```json
{"online":12,"registered":45,"matches":3,"rooms":2,"in_rooms":9,"queued":1,"last_match_ends_in":512}
```

- `online`: distinct accounts with an active lobby/chat connection or a
  session waiting for matchmaking or seated in a match.
- `registered`: registered accounts.
- `matches`: matches created in the match server and not yet ended. This
  includes matches still loading or waiting for their clients to arrive.
- `rooms`: rooms that exist (the lobby's Rooms; every PvP match is a room's).
- `in_rooms`: players in a room.
- `queued`: players waiting in a full room's queue.
- `last_match_ends_in`: seconds, rounded up, until the last running match
  ends at the latest: the longest time left to the time limit of any match
  (the launcher page's "Time left"); a match still loading, before its
  countdown, counts its map's whole wait, countdown and match time (925 s);
  0 when none runs. A match can end sooner. `release/release.sh` plans the
  maintenance 30 seconds after it (rounded up to whole minutes).

All seven fields are nonnegative integers. Counts are read from live state;
separate locks mean they are not a single atomic snapshot. Reading the endpoint
never forms a match. No names, identifiers, addresses or credentials are
returned. It is independent of the page language. Poll about once a minute.
The endpoint is only added to `srp-026g`; other version folders are unchanged.

After the endpoint PR is merged, on the host with the existing checkout:

```bash
cd /path/to/skhid-srp
git switch main
git pull --ff-only
cd srp-026g
docker compose build srp26
# When no matches are running; restarting drops lobby links and the queue.
docker compose up -d --no-deps srp26
docker compose ps srp26
docker compose logs --tail=50 srp26
curl --fail --show-error https://026g.skhid.link/api/stats
```

Use the existing deployment directory, `.env`, Compose project and data volume.
If the host receives copied sources instead of using Git, copy the updated
`srp-026g` source into that same deployment directory while preserving `.env`,
then run the Compose commands there. Do not remove volumes. No database
migration, client patch, new port or bot deployment is required. The existing
reverse proxy must forward `/api/stats` to the same web listener as `/`.
For a local check, use the configured `SRP26_WEB_PORT` (default 8098):
`curl --fail http://127.0.0.1:8098/api/stats`.

## The test server

A second 0.26g server, on a host of its own, takes `main` when a maintainer
starts the workflow "Deploy the test server" by hand. How it is set up, and
why a job on its runner can do nothing but deploy `main`:
[`deploy/test-server/README.md`](../deploy/test-server/README.md).
