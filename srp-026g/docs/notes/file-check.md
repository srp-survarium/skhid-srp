# File check (patch 215)

The game's files against the release's, as the updater checks its patch
files: a game whose `resources.db` or patches are not the release's (a
damaged or an older file, a `resources.db.patch_<n>` of no release) shows in
the match server's log, `logs/resources.log`. Nothing a player sees changes,
and nothing is written to the db.

## The parts

| Part | Where |
|---|---|
| What a match checks | `crates/resources-db/src/checked.rs`: the server's game data names, for each project, its `client_project` and the files its vegetation layers name (their `.veg` files, models, the models' materials and the materials' textures with their `.options`), and `startup.cfg` for every match; each file once with its size and sha256 (`srp/checked`, built with the data by `srp-game-data build`) |
| The formats | `crates/srp-validate/src/checks.rs`: the request (an 0xE4, `ff 'R' 'C' 01`, up to 512 paths front coded in ascending order, 16 KiB at most) and client report kind 3 (`srp_validate::report`) |
| The game | patch 215, `client-patches/rust/patches/src/file_check*` and `vostok/file_check.rs` |
| The server | `servers/match-server/src/v026g/resource_checks.rs`, its log target `resources` (`servers/srp/src/logger.rs`) |
| Reading it | `release/moderate resources [--account A] [--all]` |

## The exchange

1. When a match connection's first world is in (`world_load.loaded`), the
   game offers a check: a report of kind 3 whose part 0 names the
   `resources.db.patch_<n>` of its folder (as the mount found them at the
   game's start) and no file. Only a game that offered is ever sent a
   request, so an older game, which has no reader for it, never gets one.
2. The server logs the patches against the release's (the data's
   provenance), and sends the request for the match's project's files,
   resent until acked.
3. The game takes the request before the native options handler (patch
   114's message check, as patch 125's reply), and starts a run in pages of
   its own (`memory_profile::reserve`): each path looked up in the archives
   it mounted, the last one first, as the mount lays them (a patch's file is
   over the lower ones'); each table read through a cache of 32 pages of 4
   KiB, only the nodes on a lookup's way; then each file found hashed, 32
   lookups and 64 KiB of files a frame. The whole answer (its size and its
   sha256's first 8 bytes, or missing, or unreadable, for each file, in the
   request's order) goes as one report; the run's pages go back then, or at
   the match's close.
4. The server logs it once whole: as the release, or each file that is not.

## The log

```text
<name> (account <id>), match <id>, mounts 12 14 ... 73: as the release
<name> (account <id>), match <id>, mounts 12 14 ... 200: not as the release: also 200; not 73
<name> (account <id>), match <id>, level_01/day_2: 162 files as the release
<name> (account <id>), match <id>, level_01/day_2: 2 of 162 files not as the release: <path> size <n> sha256 <16 hex>, the release's size <n> sha256 <16 hex> | <path> missing
```

A game that leaves before its answer is whole tells none.

## Measured

- The release's archives (stock `resources.db` and the 54 listed patches,
  2026-11-26): 3,854 files checked in all; a project's 7.7 MB at the
  median, 26.6 MB at most (v069d_level_01), 301 files at most (a request of
  6.5 KB).
- The game's logic over those archives (`file_check::run`'s opt-in test
  `the_players_archives_hash_as_the_servers_data_says`): every project's
  request answers as the server's data says, 278 projects, 48,769 files,
  2.86 GB.

## Deploying

The server reads a report kind only when it knows it (`srp_validate`'s
codec has no version): the server that has kind 3 goes live with or before
the game that sends it, as a release does. Server data built before this
has no `srp/checked`: a match then checks no file, and logs the mounts
alone.
