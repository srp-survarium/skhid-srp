# Survarium Restoration Project: servers for three game versions

Server side of the old online shooter Survarium, written in Rust, for three
builds of the original game client. Each build speaks its own network
protocol, so each has a server of its own.

Built on the open source SRP project,
[srp-survarium/srp](https://github.com/srp-survarium/srp), whose 0.100b server
is where this code began. Not affiliated with or endorsed by Vostok Games;
Survarium is a trademark of its owners. The game itself is not in this
repository.

## The three versions

| Folder | Game build | What it is | State |
|---|---|---|---|
| [`srp-0100b/`](srp-0100b/) | 0.100b (early 2013) | Server-authoritative matches: the server runs combat, spawns, batteries and traps | Works locally; not hosted |
| [`srp-021d/`](srp-021d/) | 0.21d, build 2010 (April 2014) | Lockstep matches: clients simulate, the server relays inputs and compares world hashes | Hosted on skhid.link from 2026-10-10 to 2026-10-12; not hosted now |
| [`srp-026g/`](srp-026g/) | 0.26g, build 2777 (January 2015) | Still lockstep, but a new protocol at every layer (the lobby runs over UDP); items, clans, premium shop | Live on [skhid.link](https://skhid.link) (also 026g.skhid.link) since 2026-10-12; a test server on [test.skhid.link](https://test.skhid.link) |

Each folder is a complete Rust workspace that builds and tests by itself.
`srp-026g` began as a copy of `srp-021d` and still holds the 0.21d code beside
its own (its shared crates and tests use it); the 0.21d server that was hosted
was built from `srp-021d`.

## What is in each folder

| Path | What |
|---|---|
| `README.md` | What the server does, how to build, run and test it |
| `crates/`, `servers/`, `scripts/` | The code |
| `docs/` | That version's pages: the server, how matches stay in step, facts about the game client, open issues, and `docs/notes/` (the protocol research, message by message) |
| `client-patches/` | What a player's copy of the original game needs to play on this server: the patch tools for `survarium.exe`, the login certificate, and a description of every patch |
| `Dockerfile`, `docker-compose.yml`, `deploy/` | Hosting in Docker (0.21d and 0.26g); described in `docs/DEPLOY.md` |
| `certs/` | The login server's certificate and key (the game's own name, issued again; the game pins the certificate, so the key is no secret) |
| `resources/binary-config-examples/` | Game data the code embeds |

At the root:

| Path | What |
|---|---|
| [`docs/RE-TOOLKIT.md`](docs/RE-TOOLKIT.md) | How the game client was read: disassembly, symbols, game data, and the scripts in `tools/re-kit` |
| [`docs/PUBLISHING.md`](docs/PUBLISHING.md) | What must never be written into this repository (players' names, addresses, secrets) |
| `tools/re-kit/` | The reverse-engineering and log-analysis scripts |
| [`client/bugtrap/`](client/bugtrap/) | Rust crash DLL for the native Win32 client: saves zipped dumps beside the EXE, with report submission removed |
| [`client/updater/`](client/updater/) | The 0.26g launcher's updater (`updater_skhid.exe`): brings a player's files to what the server offers (the players' ZIP, the resources.db patches of `/patches`) and keeps PLAY off until they match |
| [`release/`](release/) | The scripts that release a change of the 0.26g server to players: `release/release.sh` (one run around a maintenance window), its steps, the offered-patch list `release/patches.tsv`, and `release/test.sh`; how to use them: [`.claude/skills/skhid-release/SKILL.md`](.claude/skills/skhid-release/SKILL.md) |
| `LICENSE` | GPL-3.0, as the SRP project |

## Start here

- To **run a server**: the version folder's `README.md`.
- To **host one**: `srp-021d/docs/DEPLOY.md` or `srp-026g/docs/DEPLOY.md`.
- To **release a change of the 0.26g server** to its players: `release/` and
  `.claude/skills/skhid-release/SKILL.md`.
- To **make a game copy that connects to it**: the version folder's
  `client-patches/README.md`.
- To **understand a protocol**: `docs/notes/README.md` in the version folder.

## Reading the docs

- Every page says which game version it covers. Addresses, message ids and
  constants are version-specific unless a page says otherwise.
- Findings are marked by how sure they are: PROVEN (read in the exe or seen
  in a log), LIKELY, GUESS.
- Players are written by label ("player A"), never by name.
- Some pages mention files that are not published: `WORK.md` (the
  development journal of each server), `LIVE-SERVER.md` and
  `WORKING-RULES.md` (the operator's pages). Some notes give paths on the
  author's PC (`E:\sur-res-pro\downloads\...` is an unpacked copy of the
  original game, `tools\re-kit\v026g\data\` are dumps of its data): those
  files are not here either, and `docs/RE-TOOLKIT.md` says how to make them.

## Credits and licence

The 0.100b server, the shared crates and the tooling under `scripts/` come
from [srp-survarium/srp](https://github.com/srp-survarium/srp) and its
authors. The 0.21d and 0.26g servers, the client patches for them and the
research notes were added here. Everything is under the GNU General Public
License v3.0 (`LICENSE`), the licence of the SRP project.
