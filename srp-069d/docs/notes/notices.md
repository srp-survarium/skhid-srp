# Lobby notices: level-up, friend level, faction rank-up, contract reward (B1 types 0xA, 9, 4, 0x16)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read in the 0.69d exe or the lobby movie's ActionScript (address / class given); LIKELY =
inferred; CHOICE = a server rule with no client evidence; OPEN = no evidence found. Static analysis only: nothing was run
against the real client. Addresses are 0.69d exe VAs (image base 0x200000000). Companion note for the client's follow-up
requests: requests-34-39-3a.md.

## The carrier: `B1` important message (PROVEN)

`B1 u8 type, u64 sender id, str128 name, str32 abbr, u8 flag, str256 text, u32 message id` (reader 0x200265550; also
used by squad, clan and quest messages). The client keeps a list de-duplicated by message id and hands each entry to
`important_message_arrived` (0x2001C2140) with the type, the sender id, the message id and `text` / `name` / `abbr`. The wire `type` is not the Flash type: the function maps it. The types below are the ones this note
covers; the others (2 squad, 7/8 invites, 0x0E quests, 0x17 PvE squad) are described with their features.

| wire type | Flash type (`survarium.constants.ImportantMessage`) | meaning | handler |
|---|---|---|---|
| 0x0A | 8 `LEVEL_UP` | the receiver's own level-up with its reward | 0x1C370F .. 0x1C3B05 |
| 9 | 6 (no constant; `LobbyMenu` branch `type == 6`) | "`<nickname>` / reached level N" | 0x1C354C .. 0x1C3700 |
| 4 | 2 `FACTION_LEVEL_UP` | faction level-up (rank window, unlocked items, reputation gained) | 0x1C3054 .. 0x1C3488 |

The `text` of these messages is a list of **bracketed integers** read one by one by 0x1C4750 (find `[`, find `]`, `atoi`
between, drop the consumed part); a missing bracket reads 0. The quest messages (`qg[uid][dict][param]`) use the same
reader.

## Type 0x0A: character level-up (PROVEN layout, CHOICE trigger and values)

Text, nine integers in this order (reads at 0x1C3779 .. 0x1C37FA):

| # | name | meaning (derivation) | server value |
|---|---|---|---|
| 1 | gained | number of levels gained: the bar animates `levelOffset = array length - 2` levels (ActionScript `LobbyMenu`, `LEVEL_UP` branch), the window shows "+N abilities" (`MessagePlayerRank`) | new level - old level |
| 2 | first | old level + 1: the client builds `leveling_array` for the levels `first - 1 .. first + gained` (0x1C3949 loop, each entry `{level, exp = character_leveling[level - 1].experience_points}`), `skill_level = first - 1 + gained` (0x1C388C) is the number shown as "level achieved" | old level + 1 |
| 3 | xp before | `delta = player_experience - this` (0x1C3A5A) | experience before the change |
| 4 | xp now | `player_experience` (0x1C3A31) | experience after |
| 5 | reward_type | passed as `reward_type`; the Flash window does not read it (it draws one money row and one ability-point row, `LobbyMenu` `LEVEL_UP`) | 2 (`Reward.MONEY`), CHOICE |
| 6 | reward_id | the `param` of the money row: the `Payment` enum, 0 = silver (progression-audit.md section on `RewardRenderer`) | 0 |
| 7 | reward_amount | the `amount` of the money row | sum of `character_leveling.reward_amount` (silver) of every level reached (`Levels::level_up_silver`, the silver the server already pays in `progress::add_xp`) |
| 8, 9 | premium proposal | handed to 0x1C1F90 as (current, previous): when current > previous and an offer of the premium shop (95) fits, the window shows `st_propose_premium_on_levelup` (`show_premium`, `premium_message`) | 0, 0 = no proposal. PROVEN: no proposal when int8 <= int9; else the client needs a `95` offer whose sub-record u32 >= 24 x int9 (smallest such), and shows "With a premium account you will reach new level %d days earlier!" with int8 - int9 (LIKELY days without / with premium, the u32 hours). The `95` carries premium offers (3 d / Week / Month / 3 months) and the numbers are sent: `notices::premium_days` = (days to the next level without premium, with premium) from the player's own pace since the account was created and the +50 % premium bonus (CHOICE: the original formula is in no source); 0, 0 while premium runs or without a pace. The #ru faction pair uses the same rule on that faction's reputation. |

After showing the window the client removes the message from its list, answers `51 u32 id, u8 0` (read, builder 0x2647F0) and
sends `34 00` (0x1C3B05). Nothing in the client waits for an answer to either (requests-34-39-3a.md): the server logs and
sends nothing. OPEN: whether the real server paid the level-up silver only when the `34` arrived; here it is paid at the
level change.

## Type 9: a player reached a level (PROVEN layout, CHOICE trigger)

Text `[level][reward][reward_type]` (0x1C3575 .. 0x1C35AB); the name of the message is the player's nickname. The Flash branch
builds the line `nickname` + newline + `reach_level` text + the level (green) and shows it as a 15 s notification; it ignores
`reward` and `reward_type`. After it the client sends `51` and `34`, as for type 0xA.

CHOICE: the server sends it to every **online friend** of a player who gained a level, with the final level reached
(`[level][0][0]`, sender = the player). The files give no trigger: the message has a `nickname` and no recipient rule, a
friend notification is the only reading that fits a nickname-only message, and the friends list is real stored state. A level
change that crosses several levels sends one message with the last level.

## Type 4: faction level-up, text `#ru[<faction>:<level>][a][b]` (PROVEN layout, CHOICE trigger)

Parsing (0x1C3054 .. 0x1C3188): the text must start with `#ru` (string 0x20096A388); `[` .. `:` is the faction id, `:` ..
`]` the level; the rest is two bracketed integers read like the others (a number without brackets reads 0). The two integers are the same
(current, previous) premium-proposal pair as in type 0xA, computed on that faction's reputation.

Then the client: sends `3A 00` only if bit `faction - 1` of its faction mask (the last `96`) is clear, sends `35 00` always,
builds the window from its own data: `level`, `faction_id`, `faction_current` = the points of its latest `8E` for that
faction (0x18ADE0), `faction_reputation_delta` = those points minus the snapshot it took at lobby load (`[L+0xEBC14+4*faction]`,
written at 0x11B67B), the faction's `unlock_message` from `factions_dict.faction_<id>.levels[level]`, and the unlocked items.
The reputation gain is shown in this window; there is no separate "reputation gained" message in the exe (the list of wire
types in `important_message_arrived` was read completely: 0, 2, 4, 5, 7, 8, 9, 0xA, 0xD, 0xE, 0x14, 0x15, 0x16, 0x17, 0x19).

CHOICE (trigger): one message per faction level reached, each with `level` = `game_data::levels::faction_level(points)` (the
index of the last reached threshold of `reputation_leveling.options`, the number the rest of the lobby uses), preceded by a
fresh `8E` so `faction_current` is up to date. A reputation gain that crosses no threshold sends nothing, which matches
the window being a *level-up* window.

## Type 0x16: the Winner's Contract daily reward (PROVEN layout)

Handler 0x2001C3B28: the text is two bracketed integers read by 0x1C4750 (0x1C3B67, 0x1C3C0E) into the Flash keys `amount`
(0x95B638) and `param` (0x95B8E4); Flash type 9 (`ImportantMessage.REWARD`), label from the key at 0x96A3C8 (`st_received`).
The client then answers the read itself (`51`, 0x2002647F0) and calls `root.show_contracts_message(true)` (0x1C3CD7, opens the
contract window with its reward view). The Flash REWARD branch (`LobbyMenu.add_important_message`, message without an icon)
draws a money notification of type `(param == 0 ? 1 : 2) - 1`, so `param` 0 = silver. Server (`contract::on_won_match`): text
`[20000][0]` after the first won online PvP match of a contract day, sent after the 8B and the `94` with the updated contract
(the window reads the latest contract data). Not stored for an offline player (the payer is the player who just finished the
match).

## When they are sent (the implementation)

`accounts::Store::update_profile` calls a hook after a committed change of the experience or of a faction's reputation
(`ProgressMark` before / after). `lobby-server/src/notices.rs` queues it and `notices::flush` (after every handler and on
every poll, `server.rs`) builds the messages, so every source is covered: match rewards (`play::reward_messages`), daily
quests, chests, tutorial awards, achievements, faction battles. The handler's own pushes (8B, 8C, 8E) go out first.
The message id is `0x1000_0000 | (boot time * 4096 + counter)` (the client only needs ids that differ).

A receiver who is not signed in gets nothing: notices are **not stored** (CHOICE; the original may have kept them until the
`51` read ack, OPEN). The state itself (money, level, reputation) is correct at the next sign-in through 8B / 8C / 8E.

## Follow-up requests (consistent handling)

* `51 u32 id, u8` read ack: logged, no answer (nothing is stored, so nothing to mark).
* `34 00` after a type 9 / 0xA message: logged, no answer (requests-34-39-3a.md).
* `3A 00`: the `96` faction mask (all four factions open, mask 0x0F, so the client never asks after a `#ru`).
* `35 00`: the `8E` with the battle status when a faction battle schedule is configured.

## Tests

`servers/lobby-server/tests/notices.rs`: the level-up text and values (levels, xp, silver), no notice without a new level, one
`#ru` per faction level after a fresh `8E`, online friends see type 9 (an offline friend gets nothing), the follow-up requests.
`notices.rs` unit tests pin the three text layouts.

## To check in the game

* Earn a level (finish a Training match with enough experience): the level-up window with the animated bar, "+N abilities"
  and the silver reward; then a friend's client shows "name / reached level N".
* Earn a faction level (reputation over a threshold): the rank window of the faction with the unlock message and the
  reputation gained.
* Watch the server log for `51` / `34` after each window (they must not produce an error or a reply).
* Questions the game can answer: is the bar start right (`first` = old level + 1), does the premium proposal show sensible
  day counts, do the faction unlock texts match the level.

## Offline notices (web sources)

Offline notices: no web rule (NOT FOUND); a moderator post (2017-12) says a pending AFK message is delivered after the next
match; 0.69d added alerts for incoming messages. Keeping notices unstored stays a CHOICE.

## Localized texts

Rule: every player-visible text is the client's own localization key or a typed message the client
formats itself; no server-written English. How the client localizes (PROVEN):
* `localize` 0x20013E4B0 looks the WHOLE text up as a key (any key, no `st_` rule); a missing key shows the text as sent.
  No `%d` step and no nesting (only `{USE}` / `{SPOT}` expand): a key never carries a number.
* 11 language files (chinese, english, french, german, original, polish, portuguese, russian, spanish, turkish, ukrainian);
  the client loads one (LIKELY no fallback: a key missing in that language shows raw). Check a key in all 11 before use.
* `84` display 0x200176400 localizes the text first. 0x2F: text -> system message; numeric code 0 `a` 0 / 1 ->
  `st_cant_play_leave` / `st_cant_play_afk` + a live mm:ss countdown of `b + 1` seconds (the queue-block notice).
  0x46: text or numeric `b` 0..3. 0x48 / 0x49 / 0x4C / 0x62: localized while the lobby flag `[menu+0x265]` is set, else
  the RAW text goes to the in-match chat (all senders answer lobby requests). Other ids: an assert, nothing shown.
* Localized too: the `94` record `ui` body (exe converter 0x200294190), `A1` (custom match aborted, 0x20018EE6D).
* NOT localized: the store `BC` blob `label` / `description` (0x20028E8A0), `95` tab names, `AF` chat lines (text and
  name), player names. The request `5C 00` carries a language index (1 russian .. 10 turkish, 0x2000915E0): the original
  server sent per-language store texts.

Texts the server sends: the leaver / queue-block texts -> `84 2F` numeric code 0 `a` 0 `b` secs - 1 (a fine without a block
sends no text; 8B shows it; the fine amount has no key); ranked closed during an event -> `st_locked_by_server` ("The
mode is unavailable"; no exact key exists); custom rooms -> the `st_custom_match_error_*` / `st_cant_*_custom_match_error`
/ `st_dont_have_permission` / `st_not_enough_money` / `st_no_space_in_custom_match` keys, `A1` ->
`st_custom_match_aborted_by_commander`; clan refusals -> `st_no_clan_info`, `st_player_is_already_in_clan`,
`st_clan_name_and_abbr_are_not_valid`, `st_string_pattern_error`, `st_cant_create_clan`, `st_dont_have_permission`,
`st_clan_max_members_count_error`, `st_player_not_found`, `st_cant_add_clan_member`, `st_cant_buy_clan_registration`,
`st_cant_oust_commander`, `st_cant_change_rank_clan_member`, `st_cant_change_permission`, `st_cant_create_clan_rank`,
`st_cant_delete_default_rank`, `st_error_during_storing_user_comment`; event window defaults -> `st_merry_christmas` /
`st_global_event_currency2_descr` (`st_whats_new_051_ny_event` "Holiday season event" is missing in 3 languages).
Every key above is present in all 11 languages.
OPEN: the store card and tab texts; default clan rank names (`Commander` / `Officer` / `Member` are stored
English; the client has `st_clan_rank_commander` / `st_clan_rank_assistant` / `st_clan_rank_soldier`, whether the clan
window localizes rank names is not traced); a missing-key fallback; the `[menu+0x265]` flag.

## B1 field order, invite answers, friend requests

See social.md (the `B1` field order, invite answers and friend requests).
