# Mode introductions

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
