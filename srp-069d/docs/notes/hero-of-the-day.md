# Hero of the day (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Sources: the exe, the game data and the lobby movie. Tags: PROVEN = read in the exe
(address) or in a game file; LIKELY = inferred; CHOICE = a server rule no evidence settles; OPEN = not established, not
implemented. Addresses are VAs (image base 0x200000000).

Code: `servers/lobby-server/src/hero.rs`, `crates/game-data/src/hero.rs`, `crates/accounts/src/hero.rs` (schema v13),
`crates/protocol/src/s2c/hero.rs`. Data: `data/hero_of_the_day_069d.json` (`crates/data-extract`). Tests:
`servers/lobby-server/tests/hero_actions.rs`, `crates/accounts/tests/hero.rs`.

## 1. What the player sees

* The lobby has a "Heroes of the Day" entry (`root.show_hero_of_the_day`, AS `LobbyMenu`; text `st_hero_of_the_day_description`:
  "Show the best results and get bonuses to silver, experience, and reputation."). It appears when push `A6` says so and adds a
  booster slot for the status (`addHeroBooster`). Clicking it sends the Flash event `hero_of_the_day_opened` = request
  `48 2B` (PROVEN, AS `LobbyMenu.show_hero_of_the_day`).
* The window (AS `survarium.action.DayHero`) has three nominations, in this order (0x2001CA550 loop, texts `st_score_label`,
  `st_killstreak_label`, `st_winstreak_label`): 1 Best Player (score), 2 Killstreak, 3 Winstreak. Tabs: all players,
  friends, clan. Selecting the friends / clan tab sends the Flash event `request_day_hero_data` = `48 2C` (AS
  `DayHero.handleTabSelected`, PROVEN). It lists the three places of every nomination per competitive group (name, clan
  abbreviation, score), the bonus of each place, the admitted levels (`st_hero_allowance_info`: "Admitted are characters no
  higher than ...") and a rules page.
* PROVEN rules text (client localization): "You can become a Hero of the Day regardless of your level or rank. Participate
  simply by playing ranked matches! If your results in the last 24 hours are high enough to place you in the top three players
  in one of the three nominations, congratulations - you are a Hero of the Day." / "Once obtained, your Hero of the Day status
  and bonuses received are valid for 24 hours." / "You can compete in several nominations." / "When winning in several
  nominations, you receive only one reward - the best available to you." / friends and clan lists: "Positions in this rating
  are not rewarded". The achievements `hero_of_the_day_score / killstreak / winstreak` (1..3 = third .. first place) exist
  in the data and are fed by the titles (section 6).
* The status shows as a booster slot "+N % to reputation / experience / silver" (`root.set_hero_of_the_day_level`,
  0x2001C5DD3; the text lines are in the AS of that method).
* Official (web search, 0.41 patch note quoted by a news site): players are chosen as Hero of the Day and get a unique icon,
  bonus experience, reputation and silver for 24 hours. No source gives the amounts or the ranking formula.

## 2. Game data (PROVEN)

`gameplay/event_rules/hero_of_the_day.options` (volume 00, the only volume that has it; the client reads the same file,
key `competitive_groups` at 0x2001BB5F0):

| key | value |
|---|---|
| `competitive_groups[0]` | `min_level` 3, `max_level` 5, `max_player_level` 100 |
| `bonuses` priority 1 / 2 / 3 | modifier pairs (141, 4 / 3 / 2), (142, 4 / 3 / 2), (143, 4 / 3 / 2) = `modifications` group / grade |
| `minutes_recalc_period` | 30 |
| `minutes_operating_period` | 1440 (24 h) |

The `modifications` table (`gameplay/game.options`, volume 75) resolves them: group 141 = `experience_bonus_modifier`, 142 =
`money_bonus_modifier`, 143 = `reputation_bonus_modifier`, grades 0..4 = 0.05 .. 0.25 in steps of 0.05. So place 1 = +25 %,
place 2 = +20 %, place 3 = +15 % on experience, silver and reputation. One group only.

## 3. Wire layouts (PROVEN unless marked)

Requests (C->S):

* `48 2B u32 cached id, u8 need` (builder 0x2001CA2A0): `cached id` = the client's copy of the ranking id it got in `A7`
  (kept at L+0x7A3968), `need` = 0xFF when the client has no own data yet (byte L+0x7A463C set), else 0.
* `48 2C u8 kind, u32 cached id` (builder 0x2001CA3D0): kind 1 friends / 2 clan; the id is the one the last `A9` of that kind
  carried (L+0x7A4618 / 0x7A461C).

Pushes (S->C):

* `A6` (reader 0x200188890, UI 0x2001CA550 `root.set_hero_of_the_day_data`): `u8 show`; show: `u8 n` (n <= 9), n x `{u8 min_level,
  u8 max_level, u8 max_player_level, u8 m (<= 3), m x u8 bonus percent}`. The client overflows its 0x40-byte stack buffer above
  9 groups / 3 bonuses, so the encoder refuses them. The competitive group the player belongs to is chosen by the client
  from the local account's equipment level (0x2001CAF10: the first group whose `[min, max]` holds the level, else group 0;
  the level is the highest `item_level` of the account's enabled profiles, section 11).
* `A7` (reader 0x2001889F0, UI 0x2001CA8F0 `root.set_hero_of_the_day_players`): `u8 groups` (the window loops this many times; the
  buffer holds 2), `u8 skip`; skip == 0: `u32 id`, then 2 groups x 3 nominations x 3 places x `{u32 score, str128 name, str32
  abbr}` (score 0 = empty place, the UI skips it); `u8 own`; own != 0: 2 x 3 x `u32` (the player's own value per group and
  nomination), `u8 nomination`, `u8 hero_place` (the two bytes the client copies into the chat lines of its own messages, next to
  `league_top_player`; 0x264274). Smallest: `00 01 00`.
* `A9` (inline 0x20018E8FB, UI 0x2001CAC40 `root.set_hero_of_the_day_request`): `u8 kind, u8 flag` (0 = nothing), 3 nominations x 3
  places x `{f64 public id, u32 score}`, `u32 id`. The client maps the public ids to its friends / clan list.
* `94` record kind `0x19` (jump table 0x1C5EB4 / bytes 0x1C5EC8 -> 0x2001C5930): `f3c` = nomination index 0..2 (PROVEN: 0x2001C5951 /
  0x2001C5DAE pass it as the `nomination` argument; booster icon `hero_<nomination>_<place - 1>`; `hero::notice`), `f40` = `group << 16 | grade` of the experience modifier; the client looks the percent up in
  `modifications` and calls `root.set_hero_of_the_day_level(active, percent, f3c)`. No record = no status.

## 4. Server rules

* Random and ranked queue matches (CHOICE, section 10). `hero::record` is called from `stats::record_match` for a finished,
  non-clan match whose `score` is known; the result row has the player's score, longest kill streak and win / loss. The group
  is the one of the player's OWN equipment level (`items::equipment_level`: highest worn item level over the playable
  profiles, the number the client keys its hero tab on and the repairs charge uses; CHOICE); a level
  outside every group (0..2) is not ranked. A character above `max_player_level` is not ranked.
* Calculation (CHOICE of the trigger: lazily, no background job; the slot length and the window are PROVEN data): the first
  request in a 30 minute slot calculates it: the best three of each (group, nomination) over the 24 h before the slot start get
  a title that lasts 24 h from the slot start. Titles are rows in `hero_titles` (also the ranking the window shows: the newest
  slot that still has living titles). Online title holders get a 94 with the record at once.
* Ranking values (official 0.41 patch notes, section 9): Best Player = the best single match score of the window, Killstreak =
  the longest kill streak of one match, Winstreak = the longest run of consecutive wins. Ties (CHOICE): whoever reached the
  value first. A value of 0 does
  not rank. One place per player in a nomination.
* Status and bonus: the best title (lowest place) counts, a player with several gets only that one (rules text). The bonus
  is added to premium and boosters per component (`boosters::reward_percent`, LIKELY additive like the others) in local
  and online matches.
* `A7` id = the slot start (nonzero); `A7` without the ranking when the client's cached id equals it, without the own part when
  `need` is 0; `A9` id = the time of the newest result in the list (so a changed list is resent); a list with no change gets flag 0.
  The friends / clan ranking counts the player himself and uses group 0 (the request carries no group).
* Test-only knob `LobbyConfig::hero_of_the_day` (always on in srp); off = no `A6`, no records, requests ignored.

## 5. State (schema v13, idempotent `IF NOT EXISTS`)

`hero_results (account, at, grp, score, killstreak, won)`, `hero_titles (account, grp, nomination, place, value, since, until)`,
`hero_meta (slot)`. Results older than the window and titles that ended are deleted with every calculation.
`Profile::hero` holds the account's titles (derived, read only: a profile save never writes it).

## 6. Values, results screen, open points

* OPEN: the exact ranking formulas of the real server beyond the patch-note wording (section 9).
* In online matches all three values are real. The score is the match server's per-player score
  (`Ended::stats`), the kill streak is the world's best streak of the player (`B+0xA1E`, world host protocol 3,
  `match_data::fill_facts`), the win flag is the verdict. A match without the world's data stores killstreak 0.
* Results need online PvP matches (random, or ranked: the monthly season runs by default, league.md); without them the
  window shows empty places (the window itself works).
* `f3c` of the 94 record = nomination 0..2; the two own bytes of `A7` =
  nomination 0..2 (0xFF = none) and place 1..3 (0 = none) (reader 0x200188B5E / 0x200188B73, initial word 0x00FF; `LobbyChat` icon
  `hero_chat_<nomination>_<place - 1>` when place != 0 and nomination < 254); `hero.rs` sends both 0-based. That
  0 / 1 / 2 = best player / killstreak / winstreak is LIKELY (the window's order). Icons exist for nominations 0..2 x places 0..2.
* The hero part of the results screen (`hero_place`, `hero_exp`, `hero_money`, `hero_rep` fields of the 97 record): each 97
  player row ends with
  `team, hero_place, nomination, +0xBA, +0xBB` (`ResultPlayer.tail[0..1]`; the lobby only parses them; `hero_place` reads as 0 when
  header byte +0x119A = 3); `hero_money / hero_rep / hero_exp` are the R block index 9 ("Hero of the Day bonus" row, hidden when
  zero); the in-match scoreboard icon comes from the match options' trailing tables (`{hero_place, nomination}` at +0x4C + 2i).
  The lobby passes each seat's title (`hero::match_bytes`,
  `Seat::hero`) into the third trailing options table (`OnlinePlayer::hero`; i = record index, LIKELY), each 97 row's tail is
  `[hero_place, nomination, 0, 0]` (none `0, 255`), and the hero part of the bonus (`hero::bonus_percent` of the base reward)
  is in block 9, component 0 (CHOICE), so the screen shows the "Hero of the Day bonus" row. The hero achievements (623..625):
  section 7a.
* The second competitive group (A7 holds two) does not exist in the data.

## 7. Equipment levels and the window layout

* PROVEN (data): `competitive_groups` holds one group, equipment levels 3..5. A match whose equipment level is 1 or 2 is
  in no group and is not counted (log line "equipment level Some(1) is in no competitive group (3..5)"). `item_level` and
  `match_making_level` are equal for every item of the 0.69d dictionary, so the level key is not ambiguous.
* PROVEN (exe 0x2001CAF10): the client picks the group it shows with `root.set_hero_of_the_day_group`: the first group whose
  range holds the local account's equipment level, else group 0. That fallback only selects the tab; a level 1-2 player
  sees the tier 3-5 ranking and his own row stays 0.
* PROVEN (texts, AS `DayHero`): the row under the three places is the player's own row ("Your results",
  `st_player_score_label`): A7's own values of the selected group. It is always there; 0 = no counted match in the last
  24 h. Empty places are places with score 0 (no counted results at all).
* The group of a result is the player's own equipment level (CHOICE: the client's own key): in one
  match a level-4 player is ranked and a level-1 player is not.

* Inactive players (the world's flag `B+0x9F0`, `MatchFacts::inactive`) store no result (rule: inactive =
  no reward, no fine; `stats::record_match` skips the hero row like achievements and daily quests).

## 7a. Hero achievements

* PROVEN (data): the only hero achievements are 623 / 624 / 625 (`type` 30, `apply` 0, precondition `nomination` 1 / 2 / 3),
  states `[condition 1, score 10]`, `[2, 20]`, `[3, 30]` with no reward item. Texts: state 1 = third place, 2 = second,
  3 = first; names "Hero of the Day: Best Player / Killstreak / Winstreak". The client only loads them (`nomination` read
  for type 30 at 0x20034D3D1) and shows what the server sends.
* Server: every title of a newly calculated slot (every 30 minutes) moves the achievement of its nomination to the best place
  reached so far (third 1, second 2, first 3; never back), `hero::title_achievements`. Titles come only from stored results,
  so the achievements follow the same filter (random / ranked, equipment 3..5, character level <= 100, active players).
  Changes go out as `B9` with the reached states (popups) to an online holder; the stored progress is in the login `B9` and
  in `83 55 02` (window), the state scores in `83 55 03` category 6.
* The title's bonus (+25 / +20 / +15 % experience, silver, reputation by place) is paid in the match rewards and shown in the
  97's "Hero of the Day bonus" row (block 9); tests `hero_achievements_follow_the_ranking_filter_and_reach_the_window`,
  `a_hero_title_shows_in_the_results_rows_and_its_bonus_in_its_own_row`.

## 8. To check in the game

Sign in: the lobby shows the Heroes of the Day entry; opening it shows three empty nominations (levels 3-5, up to character level 100,
+25 / +20 / +15 %), the own row "Your results" with 0 and the rules page; the friends and clan tabs show empty lists. After random
or ranked online matches with equipment of level 3 or higher (log "hero: account N result stored"), the next 30 minute slot lists the
best three; a listed player sees the booster slot with +25 / +20 / +15 % and the match results
carry that bonus on top of premium.

## 9. Web sources

OFFICIAL 0.41 patch notes (2016-04-05; 0.42 added the friends / clan tabs):
* Best Player = "more points in a single match"; Killstreak = "the longest killstreak in a single match"; Winstreak = "the
  highest consecutive number of wins"; places 1-3 get a bonus to experience, reputation and silver "for the next 24 hours" and an
  icon; "only the last day's results are taken into account". This matches the rules of section 4.
* CONFLICT: 0.41 "by simply playing the game" vs the 0.69d client text "Participate simply by playing ranked matches!" (newer,
  kept). Ranked was off during New Year 2021/22.
* The data's single group (equipment 3..5) equals the OFFICIAL ranked entry rule (tier 3+, tiers 3-5 matched together).
* Bonus percentages, the 30 min recalculation, a minimum match count: NOT FOUND on the web (client data only).

## 10. Which matches count

Hero of the Day counts ALL PvP queue matches, random and ranked (CHOICE; OFFICIAL 0.41 "by simply playing the game";
the 0.69d client text names ranked matches, but ranked is closed during events). The client cannot show a ranking per match
type: its categories are the data's equipment-tier groups (one group, tiers 3..5; the A7 window draws at most two) and the
three nominations. Clan battles, custom training, local and PvE matches stay out (`hero::record`).

## 11. Limits and values in the exe

* No minimum match count exists (data, client strings, texts, the DayHero movie: NOT FOUND). Client-side rules only:
  `max_player_level` 100 ("Admitted are characters no higher than 100 level.") and the equipment-tier group [3, 5].
* +25 / +20 / +15 % (PROVEN): `hero_of_the_day.options` bonuses priority 1 / 2 / 3 -> modifier groups 141 / 142 / 143 at
  grades 4 / 3 / 2; `game.options` `modifications` grade g = 0.05 x (g + 1).
* 30 min / 24 h = `minutes_recalc_period` 30 / `minutes_operating_period` 1440 in the same file; the exe never reads them
  (server-only values).
* The client takes the group from the LOCAL account's enabled profiles (max `item_level`, profiles.md), not from all
  players' items.
