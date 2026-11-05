# "Watch replay" from the lobby's History: the server side (0.26g)

**Version scope:** srp for client **0.26g** (build 2777) with patch revision 14 (the replay link starts the updater; revisions 12 and 13 have none) and srp's changed lobby movie (History's "Watch replay" button). Written 2026-10-15.

**Status: built and tested here (unit and socket tests); not run against a game, not deployed.** The game side (the exe patch, the movie, the updater) is in `client-patches/` and `client/updater/`; the file format is `replay_sur.md`.

## 1. Flow

1. A History row's "Watch replay" sends `3D 00 <REPLAY_ASK> <match id>` (`victory_screen_request_player_data`; `REPLAY_ASK = 0xFFFF_FF01`, no account id is that high). The lobby looks the replay up and keeps the answer as the asker's latest (`lobby-server/src/v026g/replay.rs`). The ask is not answered by itself.
2. The movie polls with `3B 00 <REPLAY_START> 01` (`REPLAY_START = 2_200_000_000`, above the JOIN starts) every second (30 times at most, up to 180 s while the answer is `preparing`). Each poll is answered with one `91` row (the asker's own answer, never anyone else's). A `preparing` answer is looked at again once a second at most per account (`RECHECK_EVERY`), or at once after news of its match (the writer kept, dropped or deleted its recording, or its conversion ended); polls in between, however many a datagram carries, get the same answer with the place in line made anew. The recordings' folder is listed once in 5 s at most for all the asks (and again after the writer's news), not at each look.
3. When the answer is `ready`, a `9E` goes just before that `91`: `video_link(false, "replay <id> <token> <sha256> <size>")`. The game keeps it hidden; "Download and watch" calls `video_tutorial_link_clicked`, which patch revision 14 turns into `updater_skhid.exe replay <id> <token> <sha256> <size>`.
4. The updater downloads `GET /replays/<token>` from the server's web page, checks the size and the SHA-256, and starts the game with `-match_replay`.

## 2. Wire

| message | bytes |
|---|---|
| ask (C->S) | `3D 00 <u32 0xFFFFFF01> <u32 match id>` |
| poll (C->S) | `3B 00 <u32 2200000000> <u8 count>` |
| answer (S->C) | `91 01 <str "R1;<status>;<match id 8 lower hex>;<n>"> <u32 2200000000> <u16 0> <u16 0>` (`lobby::replay_answer`); `<n>` is the file's size with `ready`, the replay's place in the conversion line with `preparing` (1 being made now, 2 with one before it, ...; 0 not known: its recording is still being written), else 0 |
| link (S->C, before a `ready` answer) | `9E 00 <wide "replay <8 hex> <32 hex> <64 hex> <size>">` (`lobby::video_link`) |

The row's name is 45 bytes at the most (63 is the client's limit). The link is 114 characters plus the size's digits (the contract said 113: "replay " is 7, the three hex fields 104, three spaces), under the client's 245.

Status words (`session::v026g::replays::Status`): `preparing` (being converted or looked up), `ready`, `none` (no ask yet; match id 0), `not_recorded` (the recording was dropped under `SRP_RECORD_MIN_PLAYERS`, or the lobby's report of the match has fewer places), `gone` (deleted by the storage cap, or not found), `not_yours` (the lobby's report or the sidecar does not have the asker), `old_game` (the asker's patch revision is below 13, or unknown), `incompatible` (the recording's games are of revisions this game does not play, or of none known, below), `busy` (a second ask within 2 s for another match, 20 conversions waiting, or 3 live tokens; asked again later, it is looked up anew and queued if there is room), `failed` (the conversion failed, or the recording is over `SRP_REPLAY_MAX_RECORDING_MB`; see §3 for when it is tried again), `off` (replays are not set up).

**Compatibility rule.** A recording made from 2026-10-15 on has each place's patch revision (a `revision` note at every connection, the highest kept). A game of revision R plays it when every known revision (0 and places that never connected are not counted) is from the oldest replayable revision to R: a newer game's match may play out otherwise in an older one, and an older game's in a newer one once what a match computes changed. The oldest replayable revision is the last row of `crates/session/src/v026g/simulation_changes.tsv` (11 now: the TDM end and the battery under the floor; so a game of 13 plays recordings of 11, 12 and 13: 12 changed nothing a match computes). Recordings made before 2026-10-15 have no revision and are refused (the owner, 2026-10-17: not guessed).

**A change of what a match computes.** A PR that makes a match play out otherwise cannot know its revision (the release sets it): it adds the row `next<TAB><what>` at the end of that list. The server takes `next` for its `SRP_MIN_PATCH_REVISION`, which the release sets to its own revision at its swap, so from that release on the recordings of older games are refused; `release/release.sh` writes the number in place of `next` in its record (`.claude/skills/skhid-release/SKILL.md`). The match server logs the oldest revision it gives at start.

**What the player sees of a refusal** (`incompatible`): no link, so nothing starts; the movie's own "This replay was recorded by a game version that yours cannot play." (English, Ukrainian, Russian), and with the first answer of each ask the game's message box (`84 49 00 <text>`, `social.md` §9.1: the box and nothing else) in the game's language (`4F`'s byte: English, Russian, Ukrainian, German, Polish): recorded with an older version of the game (the revisions), which plays matches differently, to be watched only with that release (`026g-rev<n>` of the newest of them, when that release has a tag: 12 and 15 on); for a recording with no revision, the same without the number; for a newer one, to update the game (`lobby-server/src/v026g/replay.rs`, `refusal_text`). Not seen in a game yet: a sentence (rather than a key) in that box, with Cyrillic letters.

## 3. Conversion

`match-server/src/v026g/replay.rs`, one thread (`srp-replay`), one match at a time:

- **Converted on the first ask only** (owner, 2026-10-15): a match's end converts nothing (most replays are never watched). The first player to ask for a kept recording's replay queues it (status `preparing`, with its place in line); every other asker of that match waits for the same job (never queued twice); the conversion goes on when the askers stop polling (a closed dialog), and later asks are answered from the stored `.sur` at once, each with its own token. Asks for other matches wait in line, 20 at most (one per player of a full match); past that, `busy`.
- The recording writer (`record.rs`) tells the replays (`session::v026g::replays::Replays`) of each file it closes: kept, dropped under the minimum, deleted by the cap (its `.sur` and `.json` are deleted too). At start, replays whose recording is gone and `.part` files of a stopped server are deleted.
- Checked before the recording is unpacked whole: its size against `SRP_REPLAY_MAX_RECORDING_MB`, then its header (the first record, in the first block, `record::read_header`) for the map, and so the mode's `.options` file. Only then are the notes read and the file converted.
- When a failure is tried again (`replays::Failure`): a missing or unreadable mode file (the operator may add the files) after 10 minutes; a reading or writing error of the disk after 10 minutes, then 20, 40, ... 4 hours at most; anything of the recording itself (over the size cap, damaged, no known map, the conversion's own error) not again until a restart.
- `sur_convert::convert_file` with the mode's `.options` from `SRP_GAME_MODES_DIR` (the file the map's `maps_dict` row names). Written: `SRP_REPLAY_DIR/<id:08x>.sur`, then the sidecar `<id:08x>.json` `{match_id, size, sha256, revisions (by place), account_ids (every place, late joiners too), made_at}`, each under `.part` first. A sidecar means its `.sur` is whole.
- The replays' own cap, `SRP_REPLAY_MAX_MB` (default 1000): after each conversion and at start, while the `<8 hex>.sur` and `.json` files are over it together, the oldest (by the `.sur`'s time) are deleted, the sidecar first; the one just made stays. A replay deleted so is made again from its recording when asked for.
- The thread runs at the lowest priority on Linux: nice 19 for that thread itself (`setpriority(PRIO_PROCESS, gettid, 19)`, logged once if it fails; nothing on other systems). The container has one CPU (`SRP26_CPUS`) for the lobby, the web page and every match's ticks, which must never wait for a conversion; the first viewer waits longer while matches run.

**Memory and time** (release build, this PC, the largest recording of 2026-10-14: 13.5 MB, 20 places, 15.5 minutes, 1.36 million datagrams, 4.4 million relay copies):

| reader | peak private memory | time |
|---|---|---|
| whole file into memory (`record::read` + `convert`, as before) | 431 MiB | 1.8 s |
| file read a block at a time (`convert_file`) | 230 MiB | 2.2 s |
| + one table of the copies by message instead of a list each, copies kept in 64 K pieces, both freed when read | **123 MiB** | 2.1 s |

The second-largest (10 MB, 10 places) peaks at 57 MiB (233 MiB before), in 1.3 s. The 20 recordings made byte-identical `.sur` files before and after the change. 431 MiB would not have fitted the 512 MB container beside the rest of srp; 123 MiB does. The bound for larger files is `SRP_REPLAY_MAX_RECORDING_MB` (default 24, about 220 MiB by the same slope; a 20-place match of 15 minutes is the largest the game has).

## 4. Tokens

128 bits from the OS's generator (`OsRng`), 32 lower-case hex digits, bound to (account, match), 10 minutes, 5 downloads (a `GET` takes one, a `HEAD` none; a broken download resumes with the next), one download (`GET` or `HEAD`) of a token under way at a time, 3 live tokens per account (an ask for a match it has a live token of gives that one again; a token past its time still counts while its download runs). So one account holds 3 of the page's 5 replay download slots at most. Many tokens of one replay download the one file at once, a slot each. In memory only: a restart ends them. A log line shows the first 6 hex digits; the lobby's packet log (`SRP_PACKET_LOG`) redacts every `9E`.

## 5. `GET|HEAD /replays/<token>`

`web-server/src/lib.rs` (`replay_route`, `replay_token`, `replay_file`, `Web::replay`):

- Every request whose path is `/replays` or under `/replays/` is answered here, never by a page.
- Only a `GET` or `HEAD` whose target, **exactly as sent**, is `/replays/` + 32 lower-case hex digits is looked up (no query, no fragment, no escapes, no suffix). The token gives a match id; the file is `SRP_REPLAY_DIR/<id:08x>.sur`, its name built from that u32 alone, served only if it is a regular file (not a link, not a folder) whose real path is in the replay folder. No part of the request is ever joined to a folder.
- Everything else (malformed, unknown, expired or used-up token, a token of another server, replays off, the file gone) gets the same 64 bytes: `HTTP/1.0 404 Not Found`, `Content-Length: 0`, no body.
- Failed lookups are counted per address: 30 in a minute, then `429` for a minute. Behind the proxy, `SRP_WEB_TRUST_PROXY=on` is needed for the address to be the player's.
- Own slots (5 at once, as `/download` and `/resources/` have); `Range` as `/download`. A second download of a token while its first runs gets `503 Service Unavailable` with `Retry-After: 10` (not a failed lookup, none of its downloads taken).
- The log line of a replay handed out shows the `Range` escaped (`{:?}`): the request's header lines are split at `\r\n` alone, so a bare `\n` or another control character in it cannot start a forged log line. The other values in it are checked first (the method, the token's 6 hex digits, the parsed address); the other routes log request values (user names, the admin text) with `{:?}` too.
- `SRP_REPLAY_DIR` must be a folder of its own: equal to `SRP_RECORD_DIR`, inside it or around it, replays are off (logged).

## 6. Routes of the web page, and why nothing private is reachable

| route | method | what it can read | why nothing private |
|---|---|---|---|
| `/replays/<token>` (0.26g) | GET, HEAD | `SRP_REPLAY_DIR/<id:08x>.sur` of a live token's match | the name is built from the token's u32; the `.json`, recordings, `.part` files and any other name are never opened; regular files in the folder only |
| `/download` | GET, HEAD | the one file `SRP_DOWNLOAD_FILE` (read-only mount) | exact path match; the file is the operator's setting, no part of the request names it |
| `/resources/<64 hex>` (0.26g) | GET, HEAD | `<folder of SRP_DOWNLOAD_FILE>/resources/<64 hex>` | only 64 lower-case hex digits name a file there (`resource_file`); that folder holds the update blobs only |
| `/api/client-files` (0.26g) | GET | the SHA-256 and size of `SRP_DOWNLOAD_FILE` | the archive's checksum, nothing else |
| `/patches` (0.26g) | GET | the list of `resources.db`'s patches the server's game data was made of (name, SHA-256, address), built in memory at start-up (`srp::v026g::offered_patches`) | no file; exact path match; the patch files themselves are served by another host |
| `/api/stats` (0.26g) | GET | counts from the store and the registry | no file |
| `/launcher…`, `/launcher/places/<lang>`, `/launcher/fresh/<lang>` (0.26g) | GET | the board and counts; the skin is compiled in (`include_bytes!`) | no file; the language is read from the path, nothing else |
| `/`, `/exchange` | GET | the store's counts (`/exchange` only says the exchange is in the game now) | pages; no file |
| `/<SRP_ADMIN_PATH>` | GET, POST | accounts, bans, maintenance (with `SRP_ADMIN_TOKEN`, from `SRP_ADMIN_IPS` only) | no file; any other address gets the plain "not found" |
| anything else | any | nothing | "not found" page |

Recordings, sidecars, `srp.db`, logs, `.env`, certificates and mode files are opened by no route. The test `web_server::replay_tests::no_other_route_reaches_a_private_file` sends every route with 17 hostile tails (`..`, `%2e%2e`, `%2f`, `%5c`, `\`, `//`, NUL, `?file=`, the token, the sidecar's and the recording's names) by GET, HEAD and POST over a socket, with such files planted beside the replay, and finds none of them in any answer.

## 7. The `.sur`'s content

It holds what every player of the match was sent (`sur_convert`'s chunks): chunk 0 the engine version; chunk 1 the 0xE4 options of the largest roster (names, clan tags, kits, items, the map, the match's static id, the squad numbers); chunk 2 the relayed inputs, roster changes and kit choices; chunk 3 worlds the clients uploaded; chunk 4 the mode's config, which every game has. No address (the recording has none), no session id (the clients' 0xD8, which the recording holds, is not read into it), no token, no server path. `match_server::v026g::replay::tests::a_kept_recording_becomes_a_replay_and_its_sidecar` plants a session id, a token, an address and the server's folders in a recording and finds none in the file or in any chunk unpacked.

## 8. Settings

`SRP_REPLAY_DIR`, `SRP_GAME_MODES_DIR` (both needed, with `SRP_RECORD_DIR`; else `off`), `SRP_REPLAY_MAX_RECORDING_MB` (24), `SRP_REPLAY_MAX_MB` (1000: the replays' `.sur` and `.json` files together, outside `SRP_RECORD_MAX_MB`; the oldest go first). The compose file sets `SRP_REPLAY_DIR=/srv/srp/data/replays` and leaves `SRP_GAME_MODES_DIR` empty: replays are off until the operator copies the 79 `.options` files into the data volume and names that folder. The mode files are game data and are never put in the repository.

## 9. Not proven

- Nothing here was played against a game: the movie, the exe patch and the updater are the first live test (the owner runs it).
- A `.sur` made by srp has not been played by a game yet (`replay_sur.md` §6).
- The compatibility rule above is a choice: which revisions really play each other's recordings is not known.
- The nice 19 call was type-checked for Linux here, not run: the log says so at the thread's start if it fails.
- One download of a token at a time: when a connection dies with no reset, the server's write waits up to 5 s (`IO_TIMEOUT`) before the slot is free; the updater tries again after 3 s, and a `503` then ends its try with "busy" (pressing "Download and watch" again works while the token lives).
- Between the checks of `replay_file` and the open, the replay folder is the server's own (nothing else writes there); a link put there in that moment would be followed. The standard library has no "do not follow" open.

## Solo matches against bots (2026-10-28)

When recording is enabled, PvE keeps its recording after one human enters, even when
`SRP_RECORD_MIN_PLAYERS` is higher. PvP still uses that configured threshold. No human entered:
the recording is discarded as before. The actual threshold is written in each recording's header.
A kept recording is considered before the replay service's missing-file/minimum-player fallback,
so History can convert and serve it for its sole human participant.

Lowering retention alone would lose the human's inputs: they are not relayed back to their owner,
and server bots have no receiving client. `Game::relay_to_others` therefore records accepted human
actions and kit choices in PvE after validation, duplicate suppression and late-input placement.
The recorder writes full match times (not wrapping tick bytes), rotation/actions or the kit.
`sur_convert` combines those with the bots' network relays and deduplicates any human relay copies
in multi-human PvE. Raw inbound packets are not treated as accepted actions. Existing recording
files remain readable. No recording starts a replay conversion until someone asks to watch it.

Tests cover solo retention versus empty/PvP matches, accepted and rejected actions, kit choices,
deduplication and conversion from a simulated solo PvE match. In-game playback of the new recording
path still needs a playtest; no client was launched for these checks.
