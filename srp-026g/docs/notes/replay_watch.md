# Watching replays from the lobby: the movie's asks, the `R1` answers, who may watch what (0.26g)

**Version scope:** srp for client **0.26g** (build 2777) and srp's changed lobby movie. Written 2026-10-15; the replay file download (History's "Watch replay" until 2026-11-08: a `3D 00 FFFFFF01` ask, a `9E` link, the updater fetching `/replays/<token>`) was removed from the movie and the server on 2026-11-08: every replay plays in the running game (a replay session, `replay-stream.md`). The exe's link patch (revision 14, `replay_link_026g.asm`) and the updater's `replay` command are Save's now (section 2.1): the server sends the link for a saved replay.

## 1. Flow

1. The movie asks to watch a match's replay in this game: `3D 00 <REPLAY_WATCH_BASE | place << 16 | seconds> <match id>` (`victory_screen_request_player_data`). The lobby answers at once with one `91` row (`lobby-server/src/v026g/replay.rs`).
2. The movie polls with `3B 00 <REPLAY_START> 01` (`REPLAY_START = 2_200_000_000`) every second (30 times at most, up to 180 s while the answer is `preparing`). Each poll is answered with one `91` row: the asker's own session's answer, never anyone else's (`none` with match 0 when it has none, `off` with replays off).
3. A watch the movie ends without an answer that ends it (the prompt's Close, or no answer in time) is withdrawn: `3D 00 <REPLAY_WITHDRAW = 0xFFFFFF05> <match id>`, not answered. The asker's session of that match is over unless its game was sent already (`Watches::withdraw`): otherwise the lobby would send the game into that replay when it is read, up to 2 minutes later, and every other Watch would get `busy` meanwhile. A `none` after the ask's own `preparing` (its session ended at the server without a word for the movie) ends the ask too: its prompt goes, and nothing is withdrawn.

### In the movie, since 2026-11-08 (watching in the running game)

The Watch redesign (`research/watch-redesign.html`): one home for watching, in the top menu, and a
Watch button wherever a match is mentioned.

- **Where.** The top menu's **Watch** tab, between Rooms and Ranking (`TopMenu.WATCH_INDEX`; the bar's
  four right tabs share Ranking's old width, `TopMenuBar.RIGHT_TABS`), with our eye on it and a red
  badge counting the live matches that can be watched now (the `L0` list's rows whose card's Watch
  would be lit: in play, with a spectator's place, open to spectators; `WatchWindow.paintBadge`):
  none for 0, a plus after the number when the list is at its most (20 rows: more may be played).
  It reads as live, not as unread (the owner's of 2026-11-10): a dark plate with a red dot, as the
  Live tab's, and the count in white; a yellow person after it when a friend plays in one of them
  (`RoomsLook.person`, the same mark before the friends' names on that match's card). With the
  arrow on the tab a small box under it says "Live matches you can watch", and on a second line "A
  friend is playing now" when one is (`TopMenu.showWatchTip`). It opens the Watch window in the rooms
  window's frame and look (`survarium/watch/`, `RoomsLook`); opening one closes the other, and the tab
  follows the window (`selectWatchTab`). The badge's list is asked every 30 s while Live is not shown,
  never during a match (`MainScene.activeMatchMaking`).
- **Context entries.** The results screen's red **Watch replay**, Save replay and the close button
  as one row centred where the close button alone is, the match's number left of the row, copied as
  a card's code is (a press selects it, Ctrl+C); the movie's own Share and Save buttons, with
  nothing behind them in this game, are hidden (`VictoryScreen.as`); each History row's **Watch** with the number under
  it; a friend's right-click **Spectate** (a friend in a match): the friend's live match watched from
  the friend's place once the live list names it (it waits through a countdown, 60 s at most); the
  player pop-up's **Matches**, and **Matches** in a player's right-click menu (a Ranking row, a
  room's member; `LobbyMenu.showPlayerInfoMenu`): Watch, Replays, that player's name searched.
- **The window.** Tabs **Live** (its count, a red dot) and **Replays**. Chips under them: Live
  Everyone (the default) or Friends (the matches with a friend in them); Replays Mine (the default:
  the asker's own `P1` rows, its private rooms' matches and those against bots among them, marked: no other list has those), Friends (`M1` with `f` 1) or Everyone (`M1`). A search box: a match's code
  (8 hex digits, `#` optional) watches it; anything else is a player's name: the game's own player
  search (`find_players`, answered in `LobbyMenu.fill_players_search`, handed by `LobbyMenu.playersFound` to the search waiting for it: one is sent at a time, a second at most, none for 3 s after one dropped unanswered after 15 s or a stray answer, so a late answer is never taken for the next search; srp answers two at once, then one a second, `social.rs` `SEARCH_BURST`)
  finds the account: srp answers the players whose name the text matches loosely, the best first
  (`session::name_search`, the item search's rules: a part of the name, its letters in order, a typo
  or two), and the window takes the name itself (any case), else the first found when three letters
  or more were typed or one player was found; `3D 00 <PLAYER_MATCHES_ASK> <account>`
  chooses it, and its `P1` rows list its matches (no chip chosen meanwhile; a chip ends the search;
  the asker's own name is Mine). After a watch asked from the window plays (the game loads a match within
  2 min of the ask) the lobby's return opens it on that view again; a refusal drops the view. It is a
  time limit because `show_match_making(false)` also comes before the load and nothing names the
  load's match. A friend's Spectate whose 60 s end, or that finds a
  match it can't watch, says so in a message box.
- **Cards** (`MatchCard`): the map's render and name by the rooms' pool (`RoomsWindow.entry`: the
  map id's level and weather; a map the pool lacks: its History row's name, else "Retired map", its
  mode's colour), mode and teams or players; the score or the player's result and K/D; the age, the
  code, selectable (a press selects it for Ctrl+C: the game's `System.setClipboard` takes nothing, run
  in a game on 2026-11-08, while a field's own Ctrl+C reaches the system's clipboard), and the friends
  in it; then one **Watch**, or why it cannot be
  watched: "Recorded before a game update" (replay 4, or an `incompatible` answer), "Replay not
  kept" (0, 3, or a `gone` answer); live: "Opens when the match starts" (made or counting down),
  "Match just ended", "No spectator places left" (full), "Not open to spectators". A History row and
  the results screen show the same reason in place of their Watch once a row or an answer said it
  (`NotificationListScrollContent.replayReason`); the asker's own matches are asked again 2 s after
  each return to the lobby (the history row is written at the server's next save, and no event says
  when), when the tab opens and once as it comes up, so History's rows know theirs.
  Cards and the toolbar are updated in place, keyed by match, so a press outlives the refreshes.
- **Watch asks** a replay session through the Watch window's one ask-and-poll loop
  (`survarium/watch/ReplayAsks.as`, the prompt `WatchPrompt.as`, `replay-stream.md`): `3D 00 <0xC0000000 | place << 16> <match id>`,
  place 0xFF (anyone) for a recent match, the player's own place for a player's match, the place of
  a friend in a live match (`F1`), else 0xFF. The live path (`live.rs`, `live_watch`) does not follow a
  place yet: the spectator starts on the server's pick. "Watch my deaths" (`0xC1000000`) is gone
  from the movie and the server: a watch ask is `0xC0000000..=0xC0FFFFFF` only
  (`REPLAY_WATCH_BASE..=REPLAY_WATCH_LAST`).
- **The rows** (`survarium/watch/WatchRows.as`; `91` rows at the request's start plus their index):
  - Live, `3B 00 <2 240 000 000> <20>`, every 5 s while shown (30 s otherwise, for the badge):
    `L0;<n>;<delay s>`, then n of
    `L1;<match 8 hex>;<map id>;<mode>;<phase>;<players a>;<players b>;<score a|->;<score b|->;<played s>;<spectators>;<watch 1 may|0 full|2 not public>`,
    then the asker's friends in them, `F1;<match 8 hex>;<place>;<team 0|1>;<name>` (their matches first);
  - Recent, `3B 00 <2 250 000 000 + f> <20>` (`f` 0 all, 1 with a friend of the asker; no bot-only
    matches), asked on showing and every 30 s: `M0;<n>;<f>` (`f` the list it answers), then n of
    `M1;<match 8 hex>;<map id>;<mode>;<score a|->;<score b|->;<players>;<length s>;<minutes since its end>;<replay>`,
    then `F1` rows as Live's (place 255: a recorded match keeps none);
  - a player's matches, `3B 00 <2 230 000 000> <10>` about the player of the asker's last `93` or
    `PLAYER_MATCHES_ASK` (`+ 1`: the asker's own, Mine): `P1;<n>;<nickname>` then n of
    `P2;<match 8 hex>;<place>;<map id>;<mode>;<result 0 defeat|1 victory|2 draw>;<kills>;<deaths>;<minutes since its end>;<replay>[;<1 private|2 with bots>]`
    (the last field on the asker's own list only);
    rows for another nickname than the one searched are left;
  - `<replay>`: 0 not recorded, 1 kept, 3 gone, 4 old version (the asker's game is below the
    replays' minimum revision). 2 (a replay file made for the download) is no longer sent.
- `srp-026g/scripts/replay-buttons/check.sh <lobby_menu.swf>` checks all of it in Ruffle, every
  language, at 1280x720 and 1920x1080; `shots.sh <lobby_menu.swf> <out>` takes the screenshots.

## 2. Wire

| message | bytes |
|---|---|
| ask (C->S) | `3D 00 <u32 0xC0000000 \| place << 16 \| seconds> <u32 match id>` (`lobby::ReplayWatchAsk`) |
| poll (C->S) | `3B 00 <u32 2200000000> <u8 count>` |
| withdraw (C->S) | `3D 00 <u32 0xFFFFFF05> <u32 match id>` (`lobby::REPLAY_WITHDRAW`), no answer |
| answer (S->C) | `91 01 <str "R1;<status>;<match id 8 lower hex>;<n>"> <u32 2200000000> <u16 0> <u16 0>` (`lobby::replay_answer`); `<n>` is the match time it starts at (ms) with `watching` (its game sent), 1 being read, 2 waiting or 0 read and its game not sent yet with `preparing`, else 0 |

The row's name is 45 bytes at the most (63 is the client's limit).

Status words (`session::v026g::replays::Status`): `preparing`, `watching`, `none` (no session; match id 0), `gone` (no kept recording), `not_yours` (`SRP_REPLAY_WATCH=match` and the asker is neither a player of the match nor a moderator), `old_game` (the asker's patch revision is below 14, or unknown), `incompatible` (the recording's games are of revisions this game does not play, or of none known, below), `busy` (the player is in a match or queued, has a session already, or `SRP_REPLAY_WATCH_MAX` sessions run), `in_room` (the player's room has a use for it now: its vote, a place in its match; also the answer to the next poll of a session its room called back before its game was sent, `Watches::recall`; `st_srp_replay_in_room`), `failed` (unreadable, over `SRP_REPLAY_MAX_RECORDING_MB`, not read within 2 minutes), `off` (no recordings).

**Compatibility rule.** A recording made from 2026-10-15 on has each place's patch revision (a `revision` note at every connection, the highest kept). A game of revision R plays it when every known revision (0 and places that never connected are not counted) is from the oldest replayable revision to R: a newer game's match may play out otherwise in an older one, and an older game's in a newer one once what a match computes changed. The oldest replayable revision is the last row of `crates/session/src/v026g/simulation_changes.tsv` (11 now: the TDM end and the battery under the floor; so a game of 13 plays recordings of 11, 12 and 13: 12 changed nothing a match computes). Recordings made before 2026-10-15 have no revision and are refused (the owner, 2026-10-17: not guessed).

**A change of what a match computes.** A PR that makes a match play out otherwise cannot know its revision (the release sets it): it adds the row `next<TAB><what>` at the end of that list. The server takes `next` for its `SRP_MIN_PATCH_REVISION`, which the release sets to its own revision at its swap, so from that release on the recordings of older games are refused; `release/release.sh` writes the number in place of `next` in its record (`.claude/skills/skhid-release/SKILL.md`). The match server logs the oldest revision it gives at start.

**What the player sees of a refusal** (`incompatible`): nothing starts; the movie's own "This replay was recorded by a game version that yours cannot play." (English, Ukrainian, Russian), and with the first answer of each ask the game's message box (`84 49 00 <text>`, `social.md` §9.1: the box and nothing else) in the game's language (`4F`'s byte: English, Russian, Ukrainian, German, Polish): recorded before a game update, and replays from older versions can't be played yet (also for a recording with no revision; no revision or release is named: nothing installs an older game, and the server knows no date of the update to name); for a newer one, its revisions and to update the game (`lobby-server/src/v026g/replay.rs`, `refusal_text`). Not seen in a game yet: a sentence (rather than a key) in that box, with Cyrillic letters.

### 2.1 Save

The save icon of the Watch window's cards and the results screen's Save replay save a match's replay to a file in the player's `<game>\replays` (`research/local-replays-design.md` section 1; the movie: `survarium/watch/ReplayAsks.as`, the same loop as a watch's).

| message | bytes |
|---|---|
| ask (C->S) | `3D 00 <u32 0xFFFFFF01> <u32 match id>` (`lobby::SAVE_ASK`), answered at once as a poll |
| poll (C->S) | `3B 00 <u32 2210000000> <u8 count>` (`lobby::SAVE_START`) |
| answer (S->C) | `91 01 <str "R2;<status>;<match id 8 lower hex>;<n>"> <u32 2210000000> <u16 0> <u16 0>` (`lobby::save_answer`); `<n>` the place in the line with `preparing` (0 not known), the file's size with `ready`, else 0 |
| link (S->C) | `9E` `video_link(false, "replay <id 8 hex> <token 32 hex> <sha256 64 hex> <size>")`, just before each `ready` row; never in the packet log |

Statuses as above, and `ready`; `none` (no ask), `gone` also for a match closed under the recordings' minimum, `busy` also for a second ask within 2 s, 20 files waiting or 3 live tokens of the account. On `ready` the movie calls `video_tutorial_link_clicked` once: patch revision 14's `replay_link` (`client_patch.md` 3.27) starts `updater_skhid.exe replay <link>`, which fetches `/replays/<token>` (a token: 10 minutes, 5 downloads, one at a time) and writes `<game>\replays\<YYYY-MM-DD>_<HHMM>_<map>_<id>.srpreplay` (`client/updater/README.md`).

The match server makes the file on a thread of its own (`match_server::v026g::replay`, nice 19, one at a time, when first asked for): `SRP_REPLAY_DIR/<id:08x>.srpreplay` (`replay_file`: a header of the map, mode, score, places' names, teams, kills and deaths, the history's timeline events, and the `.sur` a replay session plays) and its sidecar `<id:08x>.json` (size, SHA-256, revisions, accounts; never served). A file lives as long as its recording and within `SRP_REPLAY_MAX_MB`. Who may save follows `SRP_REPLAY_WATCH`, and the compatibility rule above holds for a file as for a session.

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
| `/replays/<token>` (0.26g) | GET, HEAD | `SRP_REPLAY_DIR/<id:08x>.srpreplay` of the live token's match | the token is looked up, never a path; the name is made from the match id alone, a regular file whose real path is in the folder; anything else is one and the same 404, and 30 failed lookups a minute from one address are answered 429 for a minute (`web-server/src/replay_tests.rs`) |
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

**A drawing under a press.** The window draws its cards anew on every answer, and a CLIK button clicks only when the release finds the object the press found. While the left button is down in the window `render` only notes that a drawing is owed, and the frame after the release draws it (`WatchWindow._press`; after 5 s without a release, in case it never comes). The frame after and not the release: MOUSE_UP comes before CLICK.
