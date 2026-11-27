# Ranked leagues (rating, seasons, tokens, league shop)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

PROVEN = read from the exe (address, image base 0x200000000) or the game data / AS3; LIKELY = inferred; CHOICE = server
rule with no client or data evidence. Code: `game_data::league` (tables, rating rule, schedule), `accounts::league`
(state, ladder queries, schema v10), `lobby-server/src/league.rs` (9F, `4B`, season end, rating update),
`crates/data-extract` (data), `tests/league.rs` (wire tests).

## 1. What the client and the data prove

**Data** (`gameplay/event_rules/leagues.options`, newest volume 22; volume 00 has `min_equip_level` 2 on leagues 1..3):
`leagues_parameters` = 10 leagues (league 1 = lowest). Per league `points_for_levelup` (12, 12, 12, 16, 20, 20, 28, 40,
60, 60), `possible_level_down` (false for 1..6, true for 7..10), `min_equip_level` (3), and per place of the player in
his team (index 0 = best, 8 entries) `winner_team_points` (`4 4 4 4 3 2 1 0`, every league) and `looser_team_points`
(league 1-2: all 0; 3-5: `0 0 -1 -1 -2 -2 -2 -2`; 6-10: `0 -1 -2 -3 -4 -4 -4 -4`). The client copies these names into
the `root.set_league_levels` objects (0x20027D5C0..0x20027D9A0: `points_for_levelup`, `min_equip_level` as
`equipment_level`, `possible_level_down`, `winner_team_points`, `looser_team_points`, `reward_group_id`,
`league_end_reward_group_id`, `rank`), so the rating rule above is the client's own table. `game.options`:
`rating_match_bonus_percent` = 0. No file has a season schedule, token amounts, reward groups, or shop prices.
Text: `st_rating_match_descr` ("ranked matches are available at equipment tiers 3-5, not for squads ... when the season
ends you receive tokens based on the maximum rank you reached"), `st_season_N` (1..15 names), `st_league_currency_info`.

**9F / 83 4B layouts** are in `protocol::s2c::league`. Decodes (exe, PROVEN):
* `set_league_progress` (0x20027EA40): `id` (u32) = minutes left of the season (Flash key `season_minutes_left`), `w` (u16)
  x 0.25 (constant 0x2009E5E30 = 0.25) = stars, i.e. `w` = points (4 points = 1 star; the level object shows
  `points_for_levelup >> 2` stars), `rank` = K - lv + 1, `max_reached_rank` = K - lv2 + 1 (K = number of levels, lv 1 =
  lowest league, rank 1 = best). A zero lv / lv2 counts as 1.
* Level table slot (12 bytes, 0x20027D640..): `u32 reward_group_id`, `u32 league_end_reward_group_id`, `u8 league (1..K,
  the index into leagues_parameters)`. Reward group item slot (0x20027E830..): `u16 dict_id, u32 amount, u8
  unknown_mod_count`.
* 9F byte `x` = lowest equipment tier for a ranked match: 0x2001E8350 compares the player's tier with it and else shows
  `st_low_item_level_for_rating_match`; the same function refuses squads (`st_squads_not_allowed_for_rating_match`) and
  checks `min_equip_level` of the current league (`st_low_item_level_for_league_degree`).
* 9F type 0 fields `A`, `B`, `val` = season-final notice (0x200189279..0x2001893F7, message type 18 `LEAGUE_FINAL`):
  A = league (rank = K - A + 1), B = league whose `league_end_reward_group_id` item (first slot: dict + amount) is shown,
  `val` != 0 shows the inventory item with that instance id instead; A = 0xFF = none.
* 9F type 3 (0x200188F25..0x2001890D4): `state` = the NEW league (nothing shown when <= 1); `league_up` 1 = "new league"
  message (its rewards = the reach group of that rank, `getRewardsData`), 0 = "league down" message; `n` entries of
  `{dict_id u16 @0, stack u32 @4, pc u8 @0xA, u32 (unused), chest_id u16 @8, pc x {u16, u16} modifiers}`; then the client
  always sends `4B 02`.
* `4B 04 u64`: the 8 bytes are the player's PUBLIC ID (printed with `%llu`, 0x20027EBE8), not a season
  (`League4B::History { public_id }`). The reply echoes them; elements are `{u8 month, u16 year, u8 league}` (keys `month`,
  `year`, `rank` = K - c + 1).
* Top players (0x20027EDE0): `{u16 points, str128 name, str32 clan_abbr}` x n, then the local player's record and `u32
  place`. Friends / clan list elements (0x20027F090): `{u16 stars_progress (x 0.25), u8 rank, u8 max_rank, u64 id}`.
* Shop: 8A kind 7 = league item; the league currency is the `league` field of 8B and the item "Tokens" (dict
  1356, category 29).

## 2. What is implemented

* **State** (`Profile::league`, tables `league`, `league_history`, schema v10): season, league (1..10, 0 = no ranked
  match yet), points, highest league of the season, matches, token balance, finished seasons (final league, highest league,
  tokens paid, end time, "final notice sent").
* **Rating** (`game_data::league::apply`, called by `league::record` from `stats::record_match`): a result counts only
  when `MatchFacts.match_type == Some(2)` and `team_place` is known (the place in his team by score). Points =
  `winner_team_points[place]` / `looser_team_points[place]` of the league before the match (PROVEN table, LIKELY "league
  before"). CHOICES: surplus points carry into the next league; the top league keeps counting past its `points_for_levelup`
  (OFFICIAL: the stars count on above rank 1, the top 100 is ordered by them); below 0
  the player drops one league only if the league has `possible_level_down` and lands at `points_for_levelup - |deficit|`
  of the league below, else stops at 0; a place above 7 uses entry 7; a leaver (finished = false) counts as a loss in the
  last place; a new season starts in league 1 with 0 points (no qualification phase: `st_league_qualified_info` is from
  an older rating system). A league change pushes 9F type 3.
* **Seasons**: by default a season runs every calendar month, UTC, from the 1st 00:00 to the
  next 1st (OFFICIAL rule: monthly seasons from the 1st; `game_data::league::Season::month`). Its number is
  `(year - 2000) * 12 + month` (CHOICE: the client builds no `st_season_<n>` key from the 9F `S`, which only has to be
  nonzero; every month keeps a unique number, November 2026 = 323). `[[league.seasons]]` entries of the config file (number, start, end; UTC dates or unix
  seconds) replaces the monthly seasons; between configured seasons ranked is off and 9F says S = 0.
* Season end is lazy per account (`league::settle`: at sign-in, at every `4B` request and with the next
  ranked result): history record, tokens credited, state cleared, the final notice (9F A/B) sent once with the next
  9F. Tokens = `schedule.tokens_for(highest league)`: 10 35 80 140 220 315 430 560 710 1000 (top =
  1000 OFFICIAL, sum 3500 FAN, the rest CHOICE in proportion to league^2) unless `league.tokens` of the config file lists
  ten values. The season-end reward group is `100 + league` with the item (1356, tokens), the reach group is `league`
  (empty: no data names a reach reward).
* **9F / `4B`**: 9F type 0 with the level table and reward groups from the data (also when ranked is off: K levels, S = 0),
  `x` = 3 (lowest `min_equip_level`), the player's state (minutes left, points, league, highest league, history, friends
  and clan members who play the season); `4B 01` answers the same state, `4B 02` answers 9F, `4B 04` = the stored history of the
  asked player, `4B 05` = the real ladder (league, then points, then older account; top 100; points shown = all points
  climbed, CHOICE) with the asker's place.
* **Tokens and the league shop**: 8B `league` = the balance; 16 league items (the `league`/`legend` weapons and armour of
  the dictionary without the `_premium` gold variants) are price kind 7 in 8A and bought with tokens (`Currency::League`).
  Price: Saiga-12K "Legend" 2250 PROVEN (official post 2020-06-03); every other item CHOICE 450 x item level (see
  `data/league_069d.json` `source`). A league item sells for 0 silver.

## 3. Open (exactly why)

* The ranked MATCH itself: `team_place`, `match_type` come from an online match observer. An online rated match whose
  match server delivered real scores (`Ended::stats`) fills `team_place` (teammates with a higher score, `online::pay`);
  without statistics the rating never changes: nothing is faked.
* Season schedule, token amounts per rank, reach rewards, other shop prices, loss-of-points edge rules: no data (see the
  CHOICES above).
* 9F `flag` (L+0x7A394C): sent 1 while a season runs. The client reads it only at 0x200264290 and copies it into a
  chat-record byte that is never sent or read: no effect (PROVEN). The 5 x `{u8, u8}` pairs at the end of the 9F state go
  into a vector at UI+0x3B8 that nothing reads (PROVEN): sent as zeros.
* Live season end while the player is online: nothing pushes it; it is applied at the next `4B` request or sign-in.
* The `league_*` items' `min_equip_level` check and the ranked queue entry are client-side; the server's ranked queue has to
  check the same (equipment tier >= 3, no squad, `st_*` texts above).

## 4. To check in the game

Without any setting the monthly season runs: the league window shows "Ур 1" / league 1 with an empty star bar and the
minutes left until the next 1st; the shop gets a league tab whose items cost tokens (give tokens by setting `tokens` in the
`league` table of the database). The rank-up and season-final dialogs need a ranked result (online match).

## 5. The "Performance History" window

* PROVEN (panel_league.gfx, AS `LeagueHistoryWindow`): tabs player / friends / clan. The player tab calls `ask_league_history`
  with the viewer's own `PlayerInfo.publicAccountId`; the friends / clan tabs list the friends / clan members of the 9F state
  (players of the running season) and ask for the selected one. The builder 0x20027D330 reads the argument with
  `flash_value::GetUInt64` and sends `4B 04 u64`; the reply kind 4 echoes the u64 (0x20027EB90 prints it with `%llu` into
  `public_account_id`).
* PROVEN: the window's text field starts with `st_season_league_history_not_found` ("Cannot load the history.") and keeps it
  when the reply's list is empty and the id is the viewer's own; for another player an empty list shows
  `st_season_league_player_history_not_found` ("This player did not participate in ranking matches."). So a player without
  a finished season sees "Cannot load the history." on the player tab: the client's normal text, not a failed request.
* The list holds finished seasons only (`league_history`, one row per season that ended, `{month, year, highest league}`;
  month / year = those of the season's last second, so a monthly season shows its own month, not the next 1st):
  a month the player played ranked appears after the next 1st (settled at the next sign-in or league request). The
  server logs every `4B 04` (asked id, own or not, number of seasons).

## 6. Web sources

* Seasons: calendar months starting on the 1st (or after an update / event), everyone back to rank 10, tier 3+ only, no
  squads / bots / killcam / hit markers, no drop at rank 5 or below (agrees with the data), ranked off during holiday events,
  modes every 10 minutes from 00:00 UTC Battery -> TDM -> Research -> S&D (OFFICIAL 0.47d + 2019-2021 posts).
* Tokens: the best players "each received 1000 tokens" (OFFICIAL 2017-10-04); a per-rank table summing to 3500 with 1000 at
  the top (FAN forum clue). The server's table (section 2) follows it.
* Top 100 of rank 1 by stars, icon for 30 days (OFFICIAL 2021); stars keep counting above rank 1 (OFFICIAL Q&A 2019-06-27,
  2017 result posts). The server applies no cap.
* League shop: Vz.58 P "Legend" 1250, Saiga-12K "Legend" 2250 tokens (OFFICIAL); P90 / Glock Legend, Delta sets, backpacks,
  Mauser, medical masks without prices. 2022-01: players had bought everything and had tokens left.
* The 2016-17 ranked bonus +50 % is superseded by the 0.69d data (0).
* Ranked is closed while an event runs (OFFICIAL 0.69b): a rated `2F` during an event's active window
  gets `84 2F` "Ranked matches are unavailable during the event" and `85 00` (`queue::join`).
* No 0.6x token table found (a 2020 season page only says "the higher the rank ... the more tokens"); the 1000 for the best
  players is from 2017, and the server's table rests on that number.
