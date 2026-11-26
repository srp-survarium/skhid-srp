# Mode introductions

## Each account's first three matches of a mode (2026-11-26, not released)

The owner: "I feel like the Blackout message is shown to everyone always, not
only the first 3 matches." It was: since the HUD notice (below) replaced the
chat request, nothing limited it, and Freeze Tag's notice came the same way.
Now a mode's notice (Blackout's, Freeze Tag's, any later mode's) is shown in
each account's first three matches of that mode, every mode counted apart, as
the owner decided on 2026-11-03.

- **The count.** `session::v026g::mode_intro::Introductions`, kept with the
  account: for each mode with a notice (`maps::GameMode::notice`,
  `maps::ModeNotice`), the matches of it the account was put in, up to three
  (`Seen`: none, once, twice, thrice). It is migration 13's table
  `g26_mode_introductions(account_id, mode, shown)`, read and written again
  (`persist::mode_intro`); no new migration. `shown` is the count; the rows
  revision 45's HUD wrote (`blackout`, the notices it asked for) read the same
  way. An account with no row of a mode has had none. The match history was
  not used: the lobby holds accounts in memory and saves them, and the history
  is the site's, read on its own connections.
- **The decision**, when the lobby makes a match (`play::launch`, once the map
  is chosen) and when it takes a late joiner (`rooms`): each place's
  `MatchProfile::notice` is `Shown` while its account has had fewer than
  three matches of the mode, else `Skipped` (a server's bot: `Skipped`).
- **The count goes up** once the match server has the match: for every seat
  of a match made (any kind: public, private, against bots), and for a late
  joiner as it is seated (`play::seated_late`; one that left and came again
  as a new place counts once, as its map play does). A late joiner's match
  counts as any other.
- **To the game:** srp's block after the options carries the notices entry
  (tag 7, `static_info::SRPX_NOTICES`): a byte a place, 1 shown, 0 not, in every
  match of a mode with a notice and in no other. The options are the match's,
  the same for every client (so each game can see the others' bytes; nothing
  else follows from them); a late joiner's client gets the grown roster's
  options with its own place's byte. The late joiner's profile sent to the
  others (`late_join::Prepare`) has no such entry. The strict readers
  (`static_info::read_srp_block`, `srp_validate::messages::options`) know the
  entry, and Freeze Tag's `{6, 1, 1}` in the latter.
- **In the game:** exe patch 213 (`client-patches/rust/patches/src/mode_notice.rs`)
  keeps the entry at the options' reads (a match's and a replay's), and at
  `player_icons.enemy_ids` (initialize_match, after the hud's `root.reset` and
  before the match's first frame) tells the hud `root.mode_notice(shown)`:
  shown in a match when the entry marks the local place (the options'
  `is_local` profile), or when the options have no entry (an older server:
  shown, as before); never in a replay, whatever its options say.
- **In the hud:** `ModeNotice` keeps the match's word ("unknown" until told,
  then "shown" or "skipped"; the first word stays until the next `reset`). A
  notice begun (Blackout's unforce, `set_game_type(12)`) waits for the word as
  it waits for the time and the texts, and shows only on "shown". `cancel`
  ends a mode's notice and keeps the word. A movie without exe patch 213 is
  never told and shows no notice: the hud and the exe ship together.
- Ship the server, the exe and the hud movie together.

Tests (under `nix develop`): `cargo test -p session mode_intro` (the first
three of each mode, the fourth and later not, each mode apart; the counts
across a database reopen); `cargo test -p lobby-server mode_notice` (two
accounts' Blackout matches made by the lobby: the first three `Shown`, the
fourth `Skipped`; Team Deathmatch none and not counted; a late joiner's three
matches counted, its fourth `Skipped`); `cargo test -p survarium static_info`
and `-p srp-validate --test current_protocol` (the entry written, read back
and refused when malformed or on a mode without a notice); the patches crate's
`mode_notice` tests (the entry read among the others; a replay never shows);
the hud in `scripts/freeze-tag-hud/` (`freeze-names`: told no, Freeze Tag's
and Blackout's notices stay hidden; no word yet, the notice waits and shows
when told yes; a later word does not change the first). The test data has no
Freeze Tag map: the lobby test plays Freeze Tag's matches only with one.
Not played in a native match yet.

## The server side removed (2026-11-26)

The lobby no longer answers `#mode-intro:` requests, and the account's
allowances (`session::v026g::mode_intro`) and their Ruffle harness
(`scripts/mode-intro/`) are gone: no shipped HUD has sent the request since
revision 46 (the HUD notice below replaced it the same day it shipped in
revision 45). Schema migration 13 still creates `g26_mode_introductions`, so the
numbering stays (the count above uses it again). The HUD's `ModeIntroduction`
class and `GameChat.addIntroduction` remain in the movie's source, unused.
What follows is the history.

## HUD notice follow-up (2026-11-03, not released)

The owner reported that late joiners could not read the mode notice, requested
it below the timer, simpler wording and no chat copy. The old pregame text was
cleared with the native countdown. `ModeNotice` now owns a separate HUD label:
it waits for a native countdown/time update and localization, remains through
the countdown, and gets 10 seconds after play starts (10 seconds from readiness
for a late joiner; 20 until 2026-11-04). Frame ticks only advance an already started notice. Neither
chat transport nor a server reply is needed. Repeated mode callbacks do not
restart it; HUD reset/unforcing hides it. Blackout keeps the label visible over
the hidden HUD, below the greater of the pregame and match timer bounds.

The English text is “Blackout / Bring batteries to your team’s device. / HUD and
hit markers are hidden. Hold Tab to check ammo and health.” All ten dictionaries
are updated. The HUD no longer sends the old introduction request or injects a
chat line. The existing server protocol/allowances and old standalone chat
classes remain compatible with earlier clients; this notice uses neither.

The Ruffle harness covers late joins, countdown expiry, repeated callbacks,
reset, placement and every language. Native multiplayer play remains to be
confirmed.

Freeze Tag (2026-11-26) has its notice the same way: `set_game_type(12)` begins it
(`st_srp_hud_freeze_start`, all twelve dictionaries; `freeze-tag.md`, "The mode
notice"), and Blackout's unforce cancels only Blackout's (`ModeNotice.cancel`).

## Original implementation (superseded HUD behavior)

The owner approved Blackout onboarding on 2026-11-03: a localized chat explanation,
visible immediately, once in each account's first three Blackout matches. The same
flow will later serve Scarcity, with its own wording and allowance. The sender is
the already reserved **System** name.

Approved chat copy:

- English: “Blackout: bring batteries to your team’s device. HUD and hitmarkers are
  hidden for immersion. Hold Tab for ammo, health, chat and kit selection when dead.”
- Russian: “Блэкаут: несите батареи к устройству своей команды. Интерфейс и маркеры
  попаданий скрыты для погружения. Удерживайте Tab: боезапас, здоровье, чат; после
  смерти — выбор комплекта.”

The native dictionaries supply all ten supported languages: English, Ukrainian,
Russian, German, Polish, Serbian Latin, Czech, Hungarian, Finnish and Belarusian.
Teammate visibility is not described as a Tab function (the owner's correction).

The notice is a local **System:** chat line, kept in every current match tab. It
appears without the ordinary fade-in, without opening the input or moving focus.
Blackout temporarily reveals chat for 20 seconds. Expiry preserves an open chat
or held Tab; otherwise chat returns to its usual hidden state. The line remains
in chat history. Standard inline markup uses the sender style's 15 px size.
In the Ruffle check, ordinary custom chat tags made Russian text 107 px tall,
exceeding the log's 90 px limit and discarding the line. The inline formatting
measured 62.15 px. The harness checks every supported language. Other chat messages keep their
existing formatting. Incoming lines respect the log's existing history-hold
state instead of restarting its fade timer while the notice, Tab or typing holds it.
The pre-match countdown gets a mode-specific explanation, preserving the native
waiting/start text and time. This heading appears in every Blackout match,
independently of the three chat introductions.

## Ownership and protocol

- `session::v026g::mode_intro::Allowances` owns separate allowances by typed `Mode`.
  An allowance has exactly four states (three/two/one left, done). The account
  persists it in schema migration 13, `g26_mode_introductions(account_id, mode,
  shown)`. Accounts from before this feature have three available notices.
- `Running` keeps each account's first HUD request token and decision for that
  match. A repeated request returns the same decision without spending another
  allowance; another token (reconnect/recreated HUD) returns `done`. The cache
  ends with the match. No global list of match ids grows with play.
- The HUD's `survarium.modes.ModeIntroduction` owns waiting, requesting, showing,
  finished and inactive states. It waits for native chat channels, tabs and the
  `send_function` callback, then sends `#mode-intro:blackout:<hex-token>` through
  that callback on channel 10. Retries are two seconds apart and use one token.
  This is the existing chat transport, not an added native callback or ABI.
- The lobby consumes this reserved prefix before ordinary room chat delivery,
  under the existing chat rate limit, before the admin chat-mute check (muted
  accounts receive onboarding too). It checks the player's actual running
  match, mode and roster session. A valid request returns a private account-type
  chat message from id 0, name `System`, on channel 10:
  `#mode-intro:blackout:<hex-token>:show` or `:done`. Nothing is broadcast.
  The lobby's internal match id and the match host's ticket id are different;
  `Player::in_match` indexes the former.
- The HUD consumes local request echoes and replies. Only a `System` reply with
  its current token can show the notice. Duplicate/stale replies cannot restart
  it. Reset/unforcing closes the reveal and ignores pending replies. A replay
  or an account outside an eligible live match cannot consume an allowance.
- `ModeIntroText` reads `st_srp_hud_blackout_intro` and `st_srp_hud_blackout_start`
  from `LocalizationData`, populated by the native `set_localization_data` call.
  The canonical text is in `client-patches/localization/*.tsv`; the existing
  `localization-1.json` manifest row compiles the dictionaries and HUD mappings.
  Requests wait for both localization and chat readiness. No pre-match timer is
  needed, so late joiners use the same readiness path.

For Scarcity, add its `Mode` key, exhaustively handle its server eligibility,
provide dictionary keys in every supported language, and call `begin` when that
mode's HUD is activated.
Its database row is separate. Scarcity itself is not implemented by this change.
Ship the server and HUD together; an older server does not recognize this request.
No executable arena, revision number, match computation or replay gate changes.

## Validation

Implemented and tested locally on 2026-11-03; not played in a native match and
not released. No revision was changed and no live service was touched.

Under `nix develop ./srp-026g --command bash -c 'cd srp-026g && ...'`:

- `cargo test -p session mode_intro`: 2 passed (allowance transitions and SQLite reopen).
- `cargo test -p session v026g::persist`: 35 passed, including old-schema migrations.
- `cargo test -p lobby-server mode_intro`: 1 integration test passed, exercising four
  consecutive matches, ordinary-mode rejection, outsiders, malformed/unknown mode
  requests, retries, another activation token, private delivery, admin chat mute and
  account isolation.
- `cargo test -p lobby-server blackout`: 1 passed.
- `cargo test -p lobby-server room_chat`: 8 passed.
- `cargo test -p lobby-server admin_mute`: 2 passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Workspace scope
  covers the shared account model's consumers; no native arena code changed.
- `cargo clippy -p lobby-server --all-targets -- -D warnings`: passed after adding
  muted-account coverage. The workspace format check reports pre-existing differences;
  new Rust files were formatted without changing unrelated files.
- `python srp-026g/client-patches/check_localization_026g.py`: passed, ten complete
  dictionaries and all Flash references valid.

The HUD was compiled from its source and the exact `movie.json` original using
`tools/actionscript/flash_tree.py`. Verified movie:
`/tmp/blackout-main-hud.swf`, SHA256
`2f1f1f77d68f89e6789cfb199fd526f02d59c8fb540e40f5ec1d29b78fc481e6`.

Under the same Nix environment, from the repository root:

```sh
bash srp-026g/scripts/mode-intro/check.sh /tmp/blackout-main-hud.swf /tmp/blackout-main-intro-check
bash srp-026g/scripts/blackout-kits/check.sh /tmp/blackout-main-hud.swf /tmp/blackout-main-icons.swf /tmp/blackout-main-kits-check
```

Both passed on CPU Ruffle. The introduction harness checks actual rebuilt movie
classes: all ten complete System lines fit and start fully opaque; mode headings
fit and preserve the timer; readiness, retries, sender/token matching, duplicate
replies, reset, expiry during typing/Tab and cross-tab history work. It also lets
the real chat timer run for 8.5 seconds after another incoming line to prove held
history does not fade. The existing harness passes the kit key/click, body/ammo
layout, death/respawn, typing/Tab, use prompt and always-visible teammate marker
regressions. These use a stand-in root;
Ruffle does not initialize the complete native HUD without game callbacks.

The required manifest build passed on current main (`badb9de37`) with this change:
`bash release/build-content 45 --out /tmp/blackout-main-content-45` (local validation
argument only; no revision reservation). It rebuilt the content sources, compared
them with the published patches, produced a local patch and server game data, and
completed the manifest validation. Log: `/tmp/blackout-main-content.log`.
The earlier checkout's Serbian translation failure does not occur on current main.
Next acceptance is a native match with the updated server and HUD. Nothing was
uploaded, restarted or released.
