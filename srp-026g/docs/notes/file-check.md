# File check (patch 218)

The match server records which files differ from the local reference build in
`logs/resources.log`, with the player, account, match and asset path. Read a
local test log with `release/moderate resources --log-file logs/resources.log`.
The local-clients launcher writes the same events to `runtime/server.log`
in its setup directory. Pass that file to `--log-file` for a launcher run.
Add `--all` to include clean replies.

Every match checks its vegetation files, the archived `startup.cfg` and the
file table of the base archive and each expected patch. An `@table/` entry
covers the archive index, including every entry’s path, size, offset and
CRC. Repacking an outfit with a new colour ordinarily changes that index.

The server also asks for the actual bytes of a sample of human outfit
materials, character colour textures and decal-composition pixel shaders.
Each connection samples up to eight per class, spread across the ordered
asset list, with a 12 MiB limit per class. The game hashes the effective
file after patch overrides. A payload edit that preserves the index is
visible when its path is checked. A file discrepancy can also come from
damage or a different build. Effects applied outside the archives are
outside this check.

## The parts

| Part | Where |
|---|---|
| What a match checks | `crates/resources-db/src/checked.rs`: the server's game data names, for each project, its `client_project` and the files its vegetation layers name (their `.veg` files, models, the models' materials and the materials' textures with their `.options`), and `startup.cfg` for every match; each file once with its size and sha256 (`srp/checked`, built with the data by `srp-game-data build`) |
| The formats | `crates/srp-validate/src/checks.rs`: the request (an 0xE4, `ff 'R' 'C' 01`, up to 512 paths front coded in ascending order, 16 KiB at most) and client report kind 4 (`srp_validate::report`) |
| The game | patch 218, `client-patches/rust/patches/src/file_check*` and `vostok/file_check.rs` |
| The server | `servers/match-server/src/v026g/resource_checks.rs`, its log target `resources` (`servers/srp/src/logger.rs`) |
| Reading it | `release/moderate resources [--account A] [--all]` |

## The exchange

1. When a match connection's first world is in (`world_load.loaded`), the
   game offers a check: a report of kind 4 whose part 0 names the
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

With the local revision 74 archives, the data includes 55 patches plus the
base, 278 projects, 307 character materials, 3,343 candidate textures and
140 bake shader variants. Even including 24 appearance files, all projects
fit the request limits (at most 382 entries and an upper bound of 9,321
encoded bytes).

Earlier vegetation-only measurements follow.

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
codec has no version): the server that has kind 4 goes live with or before
the game that sends it, as a release does. Server data built before this
has no `srp/checked`: a match then checks no file, and logs the mounts
alone.
