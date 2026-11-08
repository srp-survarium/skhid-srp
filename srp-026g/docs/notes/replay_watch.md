# Watching replays from the lobby: the movie's asks, the `R1` answers, who may watch what (0.26g)

**Version scope:** srp for client **0.26g** (build 2777) and srp's changed lobby movie. Written 2026-10-15; the replay file download (History's "Watch replay" until 2026-11-08: a `3D 00 FFFFFF01` ask, a `9E` link, the updater fetching `/replays/<token>`) was removed from the movie and the server on 2026-11-08: every replay plays in the running game (a replay session, `replay-stream.md`). The exe's link patch (revision 14, `replay_link_026g.asm`) and the updater's `replay` command stay until a release drops them; the server no longer sends them a link.

## 1. Flow

1. The movie asks to watch a match's replay in this game: `3D 00 <REPLAY_WATCH_BASE | place << 16 | seconds> <match id>` (`victory_screen_request_player_data`), or `0xC1000000` for the asker's deaths. The lobby answers at once with one `91` row (`lobby-server/src/v026g/replay.rs`).
2. The movie polls with `3B 00 <REPLAY_START> 01` (`REPLAY_START = 2_200_000_000`) every second (30 times at most, up to 180 s while the answer is `preparing`). Each poll is answered with one `91` row: the asker's own session's answer, never anyone else's (`none` with match 0 when it has none, `off` with replays off).

### In the movie, since 2026-11-08 (watching in the running game)

- A History row has **Watch**; the results screen has **Watch replay** and **Watch my deaths** on its
  bottom bar, left of the close button (`VictoryScreen.as`).
- Watch and Watch replay ask for a replay session (`replay-stream.md`): `3D 00 <0xC0FF0000> <match id>`
  (`REPLAY_WATCH_BASE | 0xFF << 16 | 0`: from the start, anyone in view). Watch my deaths asks with
  `0xC1000000` (`REPLAY_WATCH_MY_DEATHS`, no fields: the server finds the asker's place and deaths,
  the movie knows neither).
- All of them go through the History's prompt (`NotificationListScrollContent.replayBegin(id, ask)`):
  the same polls and `R1` answers. A session's `preparing` (1 reading, 2 waiting) says the match
  server is opening it; `watching` closes it with "The replay opens in this game window".
- Each answer sets the match's chip on its History rows (under Watch) and its Watch window cards:
  `watching` Ready, `preparing` Preparing, `incompatible` Old version, `gone` Expired; no chip
  before an answer, nor for one about the asker or the server
  (`busy`, `old_game`, ...). Each History row shows its match's number (`#` and the 8 hex digits of
  the `R1` rows and the logs) left of its chip, and the results screen after its buttons.
- **The Watch window** (`survarium/watch/`, `research/watch-hub-proposal.md`): the bottom bar's
  Watch tab opens it in the rooms window's frame and look (`RoomsLook`); opening one closes the other.
  Tabs Live, Recent, My matches, and a player's matches (the player pop-up's Matches button closes
  the pop-up and opens it); at the toolbar's right a box takes a match number as the cards show it
  (any case, `#` optional; Watch or Enter; anything else opens the prompt saying what a number is).
  Each match is a card (`MatchCard`): its map's render and name, mode, state or number and age,
  score or the player's result and K/D, the friends in it, the replay's chip, Watch, and Players
  (live: the roster, each player a button that watches from its place) or My deaths (My matches).
  A friend's right-click menu has **Spectate** for a friend in a match: Live, that match's roster
  with the friend marked. After a watch asked from the window plays (`watching`, then the game
  loads a match within 2 min), the lobby's return (`show_match_making(false)`) opens it on that tab
  again. **The lobby server does not answer its rows yet** (tracks A and D build them); the rows the
  movie takes (`survarium/watch/WatchRows.as`; `91` rows at the request's start plus their index):
  - Live, `3B 00 <2 240 000 000> <20>`, polled every 5 s: `L0;<n>;<delay s>`, then n of
    `L1;<match 8 hex>;<map id>;<mode>;<phase>;<players a>;<players b>;<score a|->;<score b|->;<played s>;<spectators>;<watch 1 may|0 full|2 not public>`,
    then the asker's friends in them, `F1;<match 8 hex>;<place>;<team 0|1>;<name>` (their matches first);
  - its roster: `3D 00 <0xFFFF_FF06> <match>` chooses the match, then `3B 00 <2 241 000 000> <24>`:
    rows `L2;<match 8 hex>;<place>;<team 0|1>;<kills>;<deaths>;<name>` (others' rows are left);
  - Recent, `3B 00 <2 250 000 000 + f> <20>` (`f` 0 all, 1 with a friend of the asker; no bot-only
    matches), asked on opening and every 30 s: `M0;<n>`, then n of
    `M1;<match 8 hex>;<map id>;<mode>;<score a|->;<score b|->;<players>;<length s>;<minutes since its end>;<replay>`,
    then `F1` rows as Live's;
  - a player's matches, `3B 00 <2 230 000 000> <10>` about the player of the asker's last `93`
    (`+ 1`: the asker's own, My matches): `P1;<n>;<nickname>` then n of
    `P2;<match 8 hex>;<place>;<map id>;<mode>;<result 0 defeat|1 victory|2 draw>;<kills>;<deaths>;<minutes since its end>;<replay>`;
    rows for another nickname than the tab's are left;
  - `<replay>`: 0 not recorded, 1 kept, 3 gone, 4 old version (the asker's game is below the
    replays' minimum revision): the card's chip before any answer, and Watch only for 1 (2, a replay
    file made for the download, is no longer sent; the movie still reads it as Ready). A card's Watch asks with `0xC0FF0000` (a player's matches: from that place,
    `0xC0000000 | place << 16`).
- `srp-026g/scripts/replay-buttons/check.sh <lobby_menu.swf>` checks them in Ruffle, every language;
  `shots.sh <lobby_menu.swf> <out>` takes their screenshots (1280x720, 1920x1080).

## 2. Wire

| message | bytes |
|---|---|
| ask (C->S) | `3D 00 <u32 0xC0000000 \| place << 16 \| seconds> <u32 match id>`, or `<u32 0xC1000000>` (`lobby::ReplayWatchAsk`) |
| poll (C->S) | `3B 00 <u32 2200000000> <u8 count>` |
| answer (S->C) | `91 01 <str "R1;<status>;<match id 8 lower hex>;<n>"> <u32 2200000000> <u16 0> <u16 0>` (`lobby::replay_answer`); `<n>` is the match time it starts at (ms) with `watching`, 1 being read or 2 waiting with `preparing`, else 0 |

The row's name is 45 bytes at the most (63 is the client's limit).

Status words (`session::v026g::replays::Status`): `preparing`, `watching`, `none` (no session; match id 0), `gone` (no kept recording), `not_yours` (`SRP_REPLAY_WATCH=match` and the asker is neither a player of the match nor a moderator), `old_game` (the asker's patch revision is below 14, or unknown), `incompatible` (the recording's games are of revisions this game does not play, or of none known, below), `busy` (the player is in a match or queued, has a session already, or `SRP_REPLAY_WATCH_MAX` sessions run), `failed` (unreadable, over `SRP_REPLAY_MAX_RECORDING_MB`, not read within 2 minutes), `off` (no recordings).

**Compatibility rule.** A recording made from 2026-10-15 on has each place's patch revision (a `revision` note at every connection, the highest kept). A game of revision R plays it when every known revision (0 and places that never connected are not counted) is from the oldest replayable revision to R: a newer game's match may play out otherwise in an older one, and an older game's in a newer one once what a match computes changed. The oldest replayable revision is the last row of `crates/session/src/v026g/simulation_changes.tsv` (11 now: the TDM end and the battery under the floor; so a game of 13 plays recordings of 11, 12 and 13: 12 changed nothing a match computes). Recordings made before 2026-10-15 have no revision and are refused (the owner, 2026-10-17: not guessed).

**A change of what a match computes.** A PR that makes a match play out otherwise cannot know its revision (the release sets it): it adds the row `next<TAB><what>` at the end of that list. The server takes `next` for its `SRP_MIN_PATCH_REVISION`, which the release sets to its own revision at its swap, so from that release on the recordings of older games are refused; `release/release.sh` writes the number in place of `next` in its record (`.claude/skills/skhid-release/SKILL.md`). The match server logs the oldest revision it gives at start.

**What the player sees of a refusal** (`incompatible`): nothing starts; the movie's own "This replay was recorded by a game version that yours cannot play." (English, Ukrainian, Russian), and with the first answer of each ask the game's message box (`84 49 00 <text>`, `social.md` §9.1: the box and nothing else) in the game's language (`4F`'s byte: English, Russian, Ukrainian, German, Polish): recorded with an older version of the game (the revisions), which plays matches differently, to be watched only with that release (`026g-rev<n>` of the newest of them, when that release has a tag: 12 and 15 on); for a recording with no revision, the same without the number; for a newer one, to update the game (`lobby-server/src/v026g/replay.rs`, `refusal_text`). Not seen in a game yet: a sentence (rather than a key) in that box, with Cyrillic letters.

## 3. What a replay holds

A replay session plays what `sur_convert` makes of the recording, in memory (`replay-stream.md`); `srp-recording-to-sur` writes the same as a `.sur` file for the game's own player (`-match_replay=`, `replay_sur.md`). It holds what every player of the match was sent (`sur_convert`'s chunks): chunk 0 the engine version; chunk 1 the 0xE4 options of the largest roster (names, clan tags, kits, items, the map, the match's static id, the squad numbers); chunk 2 the relayed inputs, roster changes and kit choices; chunk 3 worlds the clients uploaded; chunk 4 the mode's config, which every game has. No address (the recording has none), no session id (the clients' 0xD8, which the recording holds, is not read into it), no token, no server path. `match_server::v026g::replay::tests::a_recordings_places_read_and_its_sur_holds_nothing_private` plants a session id, a token, an address and the server's folders in a recording and finds none in the file or in any chunk unpacked.

## 4. Routes of the web page, and why nothing private is reachable

| route | method | what it can read | why nothing private |
|---|---|---|---|
| `/download` | GET, HEAD | the one file `SRP_DOWNLOAD_FILE` (read-only mount) | exact path match; the file is the operator's setting, no part of the request names it |
| `/resources/<64 hex>` (0.26g) | GET, HEAD | `<folder of SRP_DOWNLOAD_FILE>/resources/<64 hex>` | only 64 lower-case hex digits name a file there (`resource_file`); that folder holds the update blobs only |
| `/api/client-files` (0.26g) | GET | the SHA-256 and size of `SRP_DOWNLOAD_FILE` | the archive's checksum, nothing else |
| `/patches` (0.26g) | GET | the list of `resources.db`'s patches the server's game data was made of (name, SHA-256, address), built in memory at start-up (`srp::v026g::offered_patches`) | no file; exact path match; the patch files themselves are served by another host |
| `/api/stats` (0.26g) | GET | counts from the store and the registry | no file |
| `/launcher…`, `/launcher/places/<lang>`, `/launcher/fresh/<lang>` (0.26g) | GET | the board and counts; the skin is compiled in (`include_bytes!`) | no file; the language is read from the path, nothing else |
| `/`, `/exchange` | GET | the store's counts (`/exchange` only says the exchange is in the game now) | pages; no file |
| `/<SRP_ADMIN_PATH>` | GET, POST | accounts, bans, maintenance (with `SRP_ADMIN_TOKEN`, from `SRP_ADMIN_IPS` only) | no file; any other address gets the plain "not found" |
| anything else | any | nothing | "not found" page |

Recordings, `srp.db`, logs, `.env` and certificates are opened by no route.

## 5. Not proven

- The compatibility rule above is a choice: which revisions really play each other's recordings is not known.

## Solo matches against bots (2026-10-28)

When recording is enabled, PvE keeps its recording after one human enters, even when
`SRP_RECORD_MIN_PLAYERS` is higher. PvP still uses that configured threshold. No human entered:
the recording is discarded as before. The actual threshold is written in each recording's header.
A kept recording can be watched by its sole human participant.

Lowering retention alone would lose the human's inputs: they are not relayed back to their owner,
and server bots have no receiving client. `Game::relay_to_others` therefore records accepted human
actions and kit choices in PvE after validation, duplicate suppression and late-input placement.
The recorder writes full match times (not wrapping tick bytes), rotation/actions or the kit.
`sur_convert` combines those with the bots' network relays and deduplicates any human relay copies
in multi-human PvE. Raw inbound packets are not treated as accepted actions. Existing recording
files remain readable.

Tests cover solo retention versus empty/PvP matches, accepted and rejected actions, kit choices,
deduplication and conversion from a simulated solo PvE match. In-game playback of the new recording
path still needs a playtest; no client was launched for these checks.
