# 0.26g matchmaking: levels, teams, maps

**Version scope:** client 0.26g build 2777 (no PDB; VAs are image VAs, base 0x10000) and srp's
0.26g lobby. Written 2026-10-12.

Tags: **PROVEN** = read in the exe (VA cited), the data or the lobby SWF. **LIKELY** / **GUESS**
= inference. **choice** = srp's own rule, where the client and the sources of the time say
nothing.

Code: `crates/session/src/v026g/matchmaking.rs` (level, match level, map choice, map counts),
`crates/session/src/matchmaking.rs` (`LevelRules`, `queue_leveled`, `poll_with`: the queue, the
range, the sides), `servers/lobby-server/src/v026g/play.rs` (Play, queue screen, making the
match), `crates/session/src/v026g/persist/map_plays.rs` (`g26_map_plays`, schema step 5).

Sources of the time: `progression_sources.md` §9.

---

## 1. A player's level: the client computes it itself — PROVEN

The lobby shows an "equipment level" and looks for a match around it. The client computes both
from its own account and the static dictionary, so the server must compute the same or the
search screen lies.

`0x228E60` (lobby UI) calls `0x2CA9B0(account, dictionary, u16 rating, &min, &max)`, stores the
two results at ui+0x312C / +0x3130 and calls `root.set_equipment_level(min)` (string 0xA02150).
The rating is the u16 at ui+0x3128, written by the setter at 0x2274B0 (LIKELY from the account's
own ranking row, `90`).

`0x2CA9B0`:

1. For each kit of the account (count at account+0x57, stride 0x704): skip a kit whose state is
   2 (the premium kit; srp sends that state for no kit) and a kit that fails the Play check `0x2F7DA0` (the same
   check as before Play: `rules::check_play`). Of the rest take the **highest**
   `0x2F79A0(kit)` (0x2CA9C7..0x2CAA42).
2. `match_maker_defines.elo_modifiers`: walk the rows in order while `rating >= elo_min`; the
   modifier is the `modifier_value` of the last row reached, 0 if none (0x2CAA44..0x2CAACD).
3. Row `kit level + modifier` of `match_maker_defines.items_levels_modifiers` (no bounds check):
   `min = item_level_min`, `max = item_level_max` (0x2CAACF..0x2CAB4B).

`0x2F79A0` (a kit's level): for each of the 23 slots (0x18 × 23 = 0x228) that holds an item:
if the item is a stack (dictionary entry +0x22A), skip it when the slot's count or the item's
own count is 0; else value = `match_making_level` of the item (dictionary entry byte +0x385)
× `slots_weight[slot]`; keep the highest; return `floor(highest + 0.5)` (0x2F7A9E, the constant
0.5 at 0x927A44).

The data (`gameplay/db_static_dictionaries`):

- `match_making_level` equals `item_level` for all 384 items (0..=16; 16 only for two decals,
  which no slot holds).
- `slots_weight`: 23 × 1. So a kit's level is its highest item's, **ammo, grenades and drugs
  included** (patch notes 0.23g: the ammo's level counts).
- `elo_modifiers`: 100..10000 → 0, 10001..11000 → 1, … 14001..15000 → 5. A rating under 10001
  changes nothing; srp's ratings start at 100.
- `items_levels_modifiers`: row *n* for *n* ≤ 14 is `min n, max n + 1`; rows 15..20 are all
  `min 15, max 15`. So the level is 0..=15, and a player starts its search at its own level and
  the next ("Reduced starting level range in matchmaking", 0.25b, fits a start of one level).

This is what "если вы создадите профили с разным уровнем экипировки, то противники будут
искаться по максимальному уровню экипировки" (preview of 0.26) describes.

srp: `matchmaking::kit_level`, `gear_level`, `elo_modifier`, `level` / `level_of` → `Level {
min, max }`. A new account (TT-33, Mosin carbine) is level 0, range 0..=1.

## 2. The search screen — PROVEN (what it shows), GUESS (the columns' meaning)

`0x85` status 1 carries `u16 search step` and `20 × u16 search properties` (lobby.md §2.4).

- `0x22564D..0x225748`: `w = current_wait / search_step`;
  `root.set_mm_search_range(max(0, min − w), max + w)` with the `min` / `max` of §1. The SWF
  (`LobbyGameSearch.setSearchRange`) writes "search range: a–b" and lights the columns in it.
  **So in the original a waiting player's range grew by one level at each end every search
  step.** The step's value is the server's; not in the client's data.
- `root.set_mm_search_properties([{level: i, value}; 20])` (0x2254FE..0x225648; SWF
  `setSearchProperties`): 20 columns over levels 0..19, heights relative to the highest value
  (at least 30), the value shown on hover. GUESS: the players searching at each level. srp
  sends that count (a level over 19 would count as 19; there is none).
- `0x222D1F` calls `0x2CAB60(level = min)`: the three multipliers of `match_levels_dict[min]`
  (`money_k`, `experience_k`, `reputation_k`), shown in the lobby. So the client tells a player
  the multipliers of **its own** level.

## 3. Standard queue rules in srp

When a match starts: the rules below (0.21d's, the owner's "no blocking on player counts", was
replaced for 0.26g on 2026-10-13 by a minimum of 4 players, at the owner's word: "keep 4 minimum").

**The queue's rules (since 2026-10-13, 0.26g; the owner's, from the prod data of 2026-10-12):**

1. 20 players (a full match, 10 a side): it starts at once.
2. 10 to 19: it starts when nobody has pressed Play for 30 s.
3. 4 to 9: a countdown of 60 s starts when the queue gets its 4th player;
   each time somebody presses Play, 30 s are added to what is left of it.
4. No countdown runs longer than 2 minutes from its start (rules 2 and 3).
5. Fewer than 4 players, or only one side (a player alone, a squad alone):
   no match. If the queue drops under 4, the countdown is cancelled and
   starts anew with the 4th.

Squads: a squad's players count toward the 4 and toward the 20, but a squad
is **one side** (one party) and is never split: a squad of 4 alone waits for
somebody who is not of it; a squad of 3 and one player start after the
countdown, 3 against 1. `SRP_MATCH_MIN_PLAYERS` (4),
`SRP_MATCH_JOIN_EXTENSION_SECONDS` (30), `SRP_MATCH_MAX_COUNTDOWN_SECONDS`
(120), `SRP_MATCH_GATHER_SECONDS` (60, the countdown's start),
`SRP_MATCH_FILL_PLAYERS` (10), `SRP_MATCH_QUIET_SECONDS` (30),
`SRP_MATCH_TEAM_SIZE` (10) set the numbers. The levels' range widening
(§1) is unchanged and the conditions are counted over the players in range.

**Why (prod 0.26g, 16:32 to 21:32 UTC on 2026-10-12):** 75 players pressed
Play 313 times, 50 matches were made (24 of them with 2 or 3 players), the
queue never held 10. The roster of 20 is the client's (`player_profile[20]`,
PROVEN); matches above 9 players have not been run live yet, nor the 10 a
side the roster allows (the old default was 8 a side, 16).

What else is new:

| Rule | Tag |
|---|---|
| A match is two sides of at most `SRP_MATCH_TEAM_SIZE` players (10 since 2026-10-13, 8 before; 1..=10) | 10: the client's roster of 20 (PROVEN); the wiki of Feb 2015 says "up to 8" |
| A squad stays on one side; the sides are at most a player apart when the squads allow it, and among those splits they count as alike as can be (§8; until 2026-10-13: even in players, then in the sum of levels) | choice |
| **A squad is one party, and a match needs two parties**: two players alone, or a squad and at least one player who is not of it (alone, or of another squad). The gather window starts with the second party | the owner's rule, 2026-10-12 |
| A squad is looked for by its highest member | LIKELY (as the kits of one account are) |
| A match is formed around a waiting player (the longest waiting first) from the players whose level is in its range: `min − w ..= max + w`, `w = its wait / step` | the range: PROVEN as what the client shows; that the server matched by it: LIKELY |
| The step is `SRP_MATCH_SEARCH_STEP_SECONDS`, default 4 | choice |
| The start conditions above are counted over the players in range | choice |
| With more in range than a match holds, the nearest levels go, then who came first | choice |

**Nobody waits for ever.** The range grows without limit, so after `14 × step` seconds (56 s by
default) every level is in the range of whoever waits. With the default step that is inside the
default gather window: a server with a few players of any levels starts its matches exactly when
it did before levels counted. An operator of a full server raises the step (60 would show the
range growing by one a minute).

Only the range of the player the match is formed around is looked at; two others in it may be
further from each other than from it.

**A squad alone is not given a match** (since 2026-10-12). Until then the count was of players,
as in 0.21d: two squadmates alone in the queue were "two players waiting", got a match after the
60 s with both on one team and nobody against them, and left at once (live, matches 0x7f58348f
and 0x5438c90e). Now a squad counts once (`Group::parties`), so it waits as a player alone does,
for as long as it takes; one more player in its range of levels, alone or of another squad, is
enough, and the window of 60 s starts when that player joins. It does not wait for a second
squad (the owner's word: that might never come). The rule is the queue's, so
0.21d's entry in this tree (`Matchmaking::queue`, without levels) has it too, and
`srp-021d` got the same change the same day (its `WORK.md`). Tests in
`matchmaking.rs`: `a_squad_alone_waits_for_somebody_who_is_not_of_it`,
`a_squad_is_left_waiting_when_the_other_side_leaves`,
`a_squad_alone_gets_no_match_by_levels_either`,
`a_squad_is_not_matched_with_itself_while_another_is_out_of_its_levels`; the 0.21d
lobby's `a_squad_queues_when_its_last_member_plays`, which said a squad of two is
placed alone, is rewritten.

What the squad's clients see while they wait is the search screen, as for anybody: nothing in
the client tells a squad from a player there (§2).

`N` (static info's roster size) is the players really put in the match, as before: the client
starts its countdown at 80 % of it (modes_maps.md §10).

Clan matches (`seat_teams`) are not touched: their sides are the clan teams
(`SRP_CLAN_TEAM_MAX`).

## 4. The match's level — choice

`MatchSetup::match_level` (→ static info's `u16 match_level`, which the client only logs; the
rewards' row of `match_levels_dict`, results.md) was 0. It is now **the mean of the levels
(§1's `min`) of the players put in the match, rounded half up**, 0..=15 (never past the dict's
last row, 20).

What the original took for a match of mixed levels is not in the client. The mean keeps the
client's promise (§2: each player is shown the multipliers of its own level) exactly when the
players are of one level, which is what the original's narrow range gave.

## 5. The map — official source, the counting is srp's

"The history of players' matches is taken into account now. The system selects the less played
map" (patch notes 0.25a). No mode choice in 0.26 (added in 0.27a).

- A map is a level (`match_making_params.map_id` of its configs: 1 to 8 in 0.26g, 9 is London, from skhid's `resources.db.patch_14`; srp takes the numbers from the game data, any a patch adds included), whatever the mode and weather.
- Each account counts the matches it was **put in** on each map (`Account::map_plays`, table
  `g26_map_plays`), counted when the match is made at the match server. [choice: not when it
  ends; a player who leaves early has still seen the map.]
- The match's map is drawn by weight among the maps of the operator's pool (`SRP_MATCH_MODES`,
  `SRP_MATCH_MAPS`). A map's weight comes from its players' average plays on it, `a`, against the
  most-played map of the pool, `max`: `1 + SKEW * min(max - a, 5) / 5` (`matchmaking::map_weight`),
  `SKEW` being `SRP_MATCH_MAP_SKEW` (0 to 4, default 1). So a map its players played 5 or more
  times less than their most-played one is at most twice as likely (by default), and none is ever
  impossible. Then a mode at random among the pool's modes for that map, then one of that mode's
  ids (the weather). [choice: the owner's, 2026-10-16: a small lean to new maps, not a turn of
  them; until then the least-played map was always taken, and a new map came up match after
  match.]

## 6. Database

Schema step 5 of the `g26_*` tables (`g26_meta`): `g26_map_plays (account_id, map, plays)`,
primary key `(account_id, map)`. Additive; an account of before has no rows (has played
nothing). Tested from version 4.

Schema step 7: `g26_balance (account_id, rating, matches)`, the balance
rating of §8.3. Additive; an account of before has no row (the average, no
match). A build of before refuses the file once the step is made.

## 7. Not done / open

- The original's step and whether its range was symmetric in practice: unknown; the client only
  shows what §2 says.
- Whether `match_levels_dict` was taken per match or per player by the original server: unknown
  (§4).
- New-player protection in the matchmaker (the account's `protection` byte): not used.
- Nothing here was seen in a live client: the search screen's range and columns want a look.

## 8. The sides: what a player counts for — choice (2026-10-13)

Code: `crates/session/src/v026g/balance.rs` (the weights, the balance rating),
`crates/session/src/matchmaking.rs` (`seat_balanced`, `set_strengths`,
`set_player_value`), `servers/lobby-server/src/v026g/play.rs` (`strength_of`, the
log line), `crates/session/src/v026g/rewards.rs` (`balance_changes`),
`persist/balance.rs` (`g26_balance`, schema step 7).

Nothing of this is in the client: it is srp's own rule, from the players'
server's matches of 2026-10-12 and 13.

### 8.1 What was found

106 matches were made, 88 had a winner (no draw, not left by a whole side).

| | |
|---|---|
| The side with a player more | won 35 of 38 (3 players: 14 of 14; 5: 9 of 12; 7: 6 of 6; 9 and more: 6 of 6) |
| Odd counts where the smaller side had the higher gear sum by 1 to 6 | the bigger side still won 9 of 10 |
| Even counts, gear sums 1 or 2 apart | the higher side won 8, lost 6 |
| Even counts, gear sums 3 to 5 apart | won 11, lost 5 |
| A squad in the match (57 of 106) | the mean gap of the gear sums was 2.6 to 2.7; the same players with no squad could be split to 0.7 to 1.5 |

Each match's winner said from the others (logistic fit, no intercept; the
error is the mean log loss, a coin is 0.693), with what was known before it:

| Known | Right | Error |
|---|---|---|
| The players' counts | 53 of 87 | 0.522 |
| + gear level (§1) | 62 | 0.483 |
| + the account's level | 64 | 0.463 |
| + both | 66 | 0.456 |
| + both and a rating of the earlier results | 70 | 0.440 |
| The ranking's rating (replayed) instead | 66 of 88 | 0.473 |

The differences past the first two rows are within what 88 matches can tell.
The ranking's rating (`Account::rating`) says little: it moves by 4 at most,
by the same for every player of a side, whatever the sides' sizes.

### 8.2 The rule

A side **counts** the sum, over its players, of

`SRP_BALANCE_PLAYER` + gear level + `SRP_BALANCE_PLAYER_LEVEL` × account level
+ `SRP_BALANCE_RATING` × trust × balance rating

in gear levels (defaults 12, 1.5, 4). The gear level is §1's `min`; the
account's level is `Account::level` (its skill points); trust is
`matches / (matches + SRP_BALANCE_TRUST_MATCHES)` (default 5), 0 for a rating
of no match.

`seat_balanced` takes, of all splits: no side over `SRP_MATCH_TEAM_SIZE`; the
sides at most one player apart when the squads allow it; then the two sides
counting as alike as can be. So:

- An even count is split as before, by what the players count for (now with
  the level and the rating, not the gear alone).
- **An odd count: the side with a player more has `SRP_BALANCE_PLAYER` more
  to make up, so the stronger players are the side of fewer.** It is never
  more than one player apart for this. In a small match there is not that
  much to give (five players of gear 0 to 6: about 4 gear levels), so the
  bigger side stays ahead; the rule only lessens it.
- `SRP_BALANCE_PLAYER=0`, `SRP_BALANCE_PLAYER_LEVEL=0`, `SRP_BALANCE_RATING=0`
  is the rule of before.

The lobby gives the queue each waiting player's number before every poll
(`refresh_search_levels` → `Matchmaking::set_strengths`), without touching
anybody's wait. A queue that was given none counts a player for its level
(0.21d's entry, the tests).

The weights are the fit's, in gear levels: with the rating a player is 15, a
level 1.2, a rating point 3.9; without it 12 and 1.5. On the same rosters the
splits were the same for a player's value from 8 to 12. They are settings
because 88 matches are few: fit them again.

### 8.3 The balance rating

Kept per account (`Account::balance`: thousandths of a point and the matches
that moved it), shown to nobody, and not the ranking's. 0 is the average.

At the end of a match with a trusted winner (`balance::changes`):

1. `apart` = the sum of the ratings of team 0 − of team 1 + 2.2 × (players
   of team 0 − of team 1). 2.2 is what a player more was worth (the fit).
2. Team 0 was expected to win with `1 / (1 + e^(−apart))`.
3. Each player moves by `step × (result − expected)`, the result 1 for a
   win, 0 for a loss, 0.5 for a draw; `step = 0.8 × (1 + 3 / (1 + its
   matches)) × 2 / (players in the match)`.

So winning with a player more moves little and losing with one much; a new
player's rating moves four times as far as an old one's; and the step is
shared among the players, so a match of seven a side moves its sides as far
as a duel (a step for each player called a seven against seven at 99 to 1,
and it was lost: live, 2026-10-13).

Who is counted: the players who were in the match (the ranking's: one who
left has its team's result and nothing more; one whose game never got in is
not counted and not moved). No change: a match with a test bot, with no
trusted result, with one side, left by a whole side (its winner is who
stayed), or of few players (below).

**A match of few players moves none (2026-10-14, the owner's choice).** A
match has as many players as were in it for half of the longest stay or
more (`balance::players_in`), so one that began one against one and grew is
as large as it was for most of its time. With fewer than
`SRP_BALANCE_RATED_PLAYERS` (4: one against one, two against one) nobody's
rating moves, and it is nobody's match more: the trust does not grow. The
log says so: `Match 0x… moves no balance rating: N players were in it for
half of it or more, and it takes 4`. The ranking's rating, the pay and the
quests are as in any match. 1: every match moves the ratings, as before.

Why:

- Two players can be split one way only, so the match says nothing of how
  sides should be made.
- So small a match is the easiest to play by agreement (for the quests),
  and its winner then says nothing at all. The log cannot tell such a match
  from another; the rule goes by size alone.
- The shared step moves each rating the furthest there: a duel seven times
  as far as a match of fourteen. Live, on the first night of the late-join
  queue, whose matches start one against one (§9.1): all 8 matches made
  began so, and one match moved single ratings by 0.75, 0.75 and 0.81 of a
  point, against 0.09 in the first match of seven.

Four keeps two against two: a choice, not a measured number. The refit
script leaves small matches out by a count of its own (`--min-players`:
the players in for at least half of the per-minute lines), which is not
always this one.

Variants tried on the same matches (a step for each player or shared, new
players faster or not, draws counted or not, steps 0.2 to 3.2): all within
0.01 to 0.03 of each other. The shared step with faster new players was
taken for the reason above, not for its error.

### 8.4 The log

One line a match, after "made for": each side's sum, and for each player
what it counts for and why (gear level, account level, rating, matches).
With the "settled" line (the winner) it is what a later fit needs.

### 8.5 Squads

`SRP_SQUADS=off`: no invite is passed on and nobody is added
(`social.md` §9.4). Squads were in 57 of 106 matches and are what kept the
sides from being made even; with them on, a squad is still one side and
the rule above works around it.

`SRP_SQUAD_SIZE=2` (the owner, 2026-10-17): squads of two, not three. A
squad of three was what put strong players on one side; two are fine, and
nothing here changes for them (a squad of two is one side as before). The
invite that would make a third is refused (`social.md` §9.8).

### 8.6 Not done

- (Done 2026-10-13: late joins are placed by this number too, §9.)
- No draw among splits that count alike: the same players in the same order
  get the same sides until a rating moves.
- Not seen live. The numbers above are fits on the matches the old rule
  made; what the new one does to who wins is to be measured.

## Correction (2026-10-12): a seat is kept for a game that broke, not for a player who left

Where this note says a seat is kept for the whole match and Play sends a player back to it, read: for a player whose game or link broke. A player who left by its own will (0xE0 on the match link, match.md §25) has given the seat up and is queued like anybody (`Matchmaking::give_up_seat`).


## 9. The late-join queue: games of patch revision 5 or later — choice (2026-10-13)

A game with the late-join patch (`client_patch.md` §3.11) and one without
never play one match, so the server has two queues: `QueuePool::GrowingRoster`
for a session whose game signed in with patch revision 5 or later
(`late_join::spoken_by`), `Standard` for every other. Sections 3 to 8 above
are the standard queue. A squad of both kinds is refused at Play; arranged
clan matches keep their own teams and take nobody late. A server asks for
revision 7 (`SRP_MIN_PATCH_REVISION`): 5 and 6 have the late join too and
are put into the same queue, but a game of 5 can be killed when a player
joins its match (`client_patch.md` §3.13), neither lets in a newcomer who
has a grenade or a trap in a kit, and a game of 7 keeps a world's game
objects in another order than they do, so the two part when a grenade is
thrown (§3.14).

### 9.1 A new match

- It starts with `SRP_GROWING_MIN_PLAYERS` players (default 2: one against
  one), `SRP_GROWING_GATHER_SECONDS` (120) after the second one pressed
  Play; players who come meanwhile do not start that wait again. A match
  that is full starts at once. Fewer than two waiting: no countdown.
- Levels exclude nobody here: the queue is small, and who waits can join
  what is being played.
- Who plays is as many as fit with the sides at most one player apart
  (`matchmaking_policy::place`); a squad alone has no opponents and waits.
  **The sides are made as the standard queue makes them** (§8.2,
  `seat_balanced`): by what each player counts for.
- Players left over wait a new window, or join the match just made.
- The home page ("How matches start") tells these rules, and the running-match rules of §9.2, when every game that can press Play is of this queue: `SRP_MIN_PATCH_REVISION` is 5 or more (`web-server`'s `only_growing_games`). Otherwise it still tells the standard queue's (§3), whose games are then the other queue.

### 9.2 A match being played: who joins, and where

Play first looks at the matches being played (`play.rs`,
`fill_running_matches`; `matchmaking_policy::place_late`). A match takes
players while it is in play, has more than two minutes left
(`JOIN_CUTOFF`), has roster places left (twenty in all, and a place once
given is never given again) and is not busy taking somebody in.

**A side counts** what the balance rule counts (§8.2) for **the players who
are in the match now** (the match server says who has left, once a second),
and **its lead**: a battery of lead is `SRP_BALANCE_BATTERY_LEAD` (0.55) of
a player to the side that has it, a ticket `SRP_BALANCE_TICKET_LEAD` (0.2).
Of the ways to put the waiting groups on a side or leave them out, the one
that takes the most players, and of those the one after which the sides
count most alike. Allowed:

- no side over `SRP_MATCH_TEAM_SIZE` players in the match. **A joiner takes
  the place of one of the match server's own bots** (`SRP_SERVER_BOTS`)
  on its side when that side has one that gives way (`LiveView::
  giving_way`, `SRP_BOTS_GIVE_WAY` of the match server) and, with the
  joiner, would have more players than the other side
  (`matchmaking_policy::bot_gives_way`): the bot leaves, the side keeps its
  size, and the joiner's strength replaces the bot's none. Several joiners
  are taken one after another in the order of their places. The match
  server decides it by the same rule when the joiner enters, so a side
  full of bots takes humans. The joiner's roster place is a new one all
  the same: a bot leaving frees no place of the twenty;
- the sides at most one player apart. **Two, when the side with more is the
  one that trails and the lead is dramatic**: `SRP_BALANCE_DRAMATIC_LEAD`
  (2.5) players, which is five batteries or thirteen tickets. Sides that
  are further apart already (players left) take players on the smaller one;
- **a batch that would make the sides count less alike by more than half a
  player waits** until each of its players has waited
  `SRP_LATE_JOIN_PATIENCE_SECONDS` (60), unless it brings the sides nearer
  in players. So one player alone does not go into a match that is even:
  two go in together, one to each side, or it goes in after its minute.

Of several matches, the one with the highest (players with the batch)^p ×
minutes left (`SRP_GROWING_POPULATION_EXPONENT`, 1), then the one whose
sides end nearest. The batch is taken whole or not at all
(`Matchmaking::append_batch`, the match server's reservation); squads keep
their ids. The reward level of the match stays what it was at the start.

Not counted: a player who left. Its roster place stays taken (the games
have its resources), so a match that many left can run out of places before
it runs out of time.

### 9.3 Why: what the saved worlds say (2026-10-13)

The players' server keeps the worlds its matches agreed on. Of the matches
of 2026-10-12 and 13, 66 had four players or more, a winner, and worlds
through the match (51 in the battery modes, 15 Team Deathmatch; about
24,000 worlds read with `match-server/examples/standings_dir.rs`). At each
of six moments from minute 2 to 12:

| | |
|---|---|
| The side with more players in the match at that moment | won 84 to 89 % (31 to 37 matches had such a side) |
| The sides, a minute in | level in 35 of 66, one apart in 28, two in 3; at minute 12: 30, 27, 7 and two matches three apart |
| The side with more batteries home | won 77 to 89 %; by one battery 37 of 59 over the six moments, by five or more 72 of 72 from minute 6 (7 of 8 at minute 4) |
| The side that first led by five batteries, with two minutes or more left | won 28 of 30; such a lead came in 30 of 51 matches, at 6 minutes in the middle one. By four: 35 of 39. By three: 38 of 44 |
| A side that led by two to four batteries with a player fewer in the match | lost, at all 7 of the moments where that was so |
| Tickets | a lead of eleven or more was not lost (9 of 15 matches had one, at 11 minutes in the middle one) |

A fit of who won against the players apart and the lead, at five moments of
each match, the matches left out in turn: in the battery modes a player
more 0.86 and a battery of lead 0.49, so a battery is 0.57 of a player (log
loss 0.449; the players alone 0.568, the lead alone 0.457). In Team
Deathmatch ten tickets came out at 2.2 players; 15 matches are few, and the
default is lower (0.2 a ticket).

So: who is in the match now decides more than who was seated; a lead is
worth players, and a dramatic one is still the likelier winner against two
players more (five batteries count 2.75 players); and one player more in a
match that is even is the largest thing a late join can do to it, which is
why it waits.

### 9.4 The balance rating of players who were not there for all of it

`balance::changes_by_share`: each player counts for the share of the match
it was in, its time in the match against the longest anybody's was. A side
is its players by their shares (one who came at half time is half a player
and half its rating), and a player is moved by its share of a whole step.
A side that got a player late and won is not taken for a side that won
alone, a player who came into a lost match does not carry the loss, and a
side whose player left early is the short side it was. A match that nobody
joined or left is rated as before. The rating does not know the lead a
player joined at. The same shares say how many players the match had, and
with fewer than four it moves no rating (§8.3).

### 9.5 The log

- `Match 0x… sides: …` when a match is made (§8.4), in both queues.
- `Match 0x… late join: <name> <what it counts for> (gear g, level l,
  rating r of m matches) to team <n>, …; with them team 0 counts X (n in,
  lead L), team 1 counts Y (m in, lead L); the sides were Z apart`. Each
  joiner is written as the "sides" line writes a player (the parts since
  2026-10-14; before, the name and the sum alone).
- **Each minute of every match being played**, in both queues: `Match 0x…
  at <s> s: team 0 counts X (n in, lead L), team 1 counts Y (m in, lead
  L); points [a, b], kills [k0, k1]; team 0 expected at P %; out: <name>,
  …`. P is `Weights::expected` of what the sides count apart, a player
  more being the rating's nine to one. "out" names the players of the
  roster who are not in the match at that moment, and is not there when all
  are in (since 2026-10-14). The parts are there so that the numbers can
  be fitted again from these lines and the results; the line changes
  nothing.
- `scripts/balance-refit/refit.py` reads these lines and the "settled"
  one: the weights of §8 fitted on who was in each match, minute by
  minute. It does not fit the numbers of the lead.
- `<name>: JOIN match 0x… (key k): ok|full|gone|busy (…); sides …` and
  `<name>: Play with a place held in match 0x… …`: the JOIN button (§9.7).
  A JOIN that puts a player in writes the "late join" line too.

### 9.6 Not done, not known

- Not seen live: no match of this queue was played on a server.
- One failed join closes that match to later joins.
- The numbers are fits on matches the old rules made, from two days.
- A player who joins does not choose its side: it goes where the count
  says (with JOIN, §9.7, it chooses the match).
- The queue numbers a searching player is shown count both queues.

### 9.7 JOIN from the lobby's list of the matches

srp's changed lobby movie lists the matches being played with a JOIN
button (`lobby.md` §2.16.1). It is on for a player and a match only where
the rule of §9.2 puts that player alone in at once **with no patience
waited out**, and the sides then count strictly more alike than without
it: a join only ever makes a match that is uneven by value more even (the
owner's rule of 2026-10-14). A JOIN that is granted holds the place for
10 s; the player's Play then goes into that match by the steps of §9.2
(`Matchmaking::append_player`, the match server's reservation) or is
refused, never queued. While a place is held, nobody else gets one in that
match, the queue's late joins included: the match server takes one joiner
at a time. A player who waits in the queue does not see JOIN on.

### 9.8 A match against the server's bots ("Player vs Bots", 2026-10-16)

The owner's decisions of 2026-10-16. A player of equipment level 5 or
lower (`SRP_PVB_MAX_GEAR`), alone (no squad), not searching, not in a
match, chooses a size (6, 8 or 10 a side) and a map in the changed lobby
movie (`lobby.md` §2.16.2); its Play then makes the match at once:

- no queue, no rule of two parties, no gather window, no levels:
  `Matchmaking::seat_solo` seats it alone on team 0, the teams fixed;
- the lobby adds 2N − 1 of the server's bots after it
  (`play::server_bot_teams`, each to the side with fewer places: N − 1 on
  the player's side, N on the other), on the map chosen (not
  `choose_map`); the play is no map play of the player's (the rotation
  of §5 is the PvP queue's);
- the match's level is the player's own (§4, a match of one level);
- fixed teams: it takes no late joiner (§9.2's `late_admission` is
  `None`, the list says code 7), and nobody of the queue goes into it;
- at most `SRP_PVB_MAX_MATCHES` (5) run at once, counting the choices held
  for players who have not pressed Play yet: the sixth is refused (code
  7), and each map's `free` in the status says how many more may start;
- its rewards: half the silver and experience (`SRP_PVB_PAY`), no quest,
  no daily bonus, no rating and no balance rating (`results.md`;
  `session::v026g::rewards`).

The log: one line for the choice (`<name>: vs bots, 8 v 8 on map 61: ok,
held for 10 s`, or `no, code n (why)` with the numbers), one for the Play
(`<name> pressed Play vs bots …` or `refused (…); not queued`), and one
for the match (`Match 0x… vs bots: <player> with 7 of the server's bots
against 8, 8 v 8 on map 61 (level_01, mode 2); 1 bot matches of 5 at
most`).

## The growing-roster placement before 2026-10-13 (kept for the record)

Until 2026-10-13 a late join was placed by the sides' sizes with the players
who had left still counted, and by the mean gear level; the text below
describes that solver, which still chooses who plays a new match (§9.1).


This policy applies only to the patched `GrowingRoster` pool (draft PR #22's
protocol). Standard clients and arranged clan matches retain their existing
placement rules. The team capacity is the standard queue's setting
(`SRP_MATCH_TEAM_SIZE`, default 10). Departed players
remain in population counts and level sums for now;
reconnection expiry and replacement of departed slots are separate work.

For a running match, let `A,B` be its retained team populations, `S_A,S_B`
their matchmaking-level sums, and `T` its remaining play time in minutes.
For queued group `i`, `s_i` is its size and `H_i` its level sum. A solo is a
group of one. Binary `x_i,y_i` assign the whole group to A or B:

```text
A' = A + sum(s_i * x_i)       B' = B + sum(s_i * y_i)
x_i + y_i <= 1               sum(x_i + y_i) >= 1
abs(A' - B') <= 1            1 <= A', B' <= team capacity
T > 2

score = (A' + B')^p * T
level_gap = abs((S_A + sum(H_i*x_i))/A' - (S_B + sum(H_i*y_i))/B')
```

For each match, select the largest eligible batch, then the assignment with
the smallest resulting average-level gap. Across matches choose the highest
score, then the smallest level gap, then the oldest queue match id. This is a
local shared-player-minutes heuristic, not a prediction of future arrivals or
permanent departures. With `p=1`, eight resulting players with four minutes
left score 32; two with twelve minutes left score 24. At `p=0.5` the second
match wins. A match with exactly two minutes remaining is always excluded.

Examples:

- Existing 2v3 plus squads of three and two: assign three to A and two to B,
  yielding 5v5. Neither squad needs to qualify independently.
- Existing 3v3 plus a squad of three alone: wait. Add two opposing solos or
  a squad of two and 6v5 becomes eligible.
- Existing 2v4 plus a squad of three: 5v4 is eligible.
- Three players per team, average levels 10 and 20; a level-25 solo joins
  the first team, producing averages 13.75 and 20 rather than 10 and 21.25.
  Population balance takes precedence; levels are a preference, never a
  hard limit. A placement may worsen the gap when no better option exists.

The solver (`crates/session/src/matchmaking_policy.rs`) uses dynamic
programming over integer counts and level differences. For fixed final
sizes, minimizing `abs(S'_A * B' - S'_B * A')` minimizes the average gap.
Equivalent partial states share a predecessor path, avoiding enumeration of
every skip/A/B assignment. A target stops searching when it reaches a proven
integer lower bound (the nearest attainable residue modulo the level-increment
gcd); zero gap also rules out improvement by any other target. There are at
most ten slots per side and twenty
historical descriptors. Original matchmaking levels currently range 0..15.
An exhaustive small-queue oracle checks the solver independently.

The lobby commits the selected batch through `Matchmaking::append_batch`.
It revalidates every queued group while holding the queue lock, reserves
all profiles at the match host once, and only then removes the groups from
the queue. Distinct squads retain distinct nonzero ids. A stale observation
or rejected reservation leaves the entire batch queued. The existing
resource barrier and snapshot catch-up then admit the batch together.

The match server publishes an absolute admission deadline derived from its
simulation time limit, minus 120 seconds. Both selection and the registry
reservation check that deadline. Stale publications expire even if the
match thread stops polling. The cutoff applies to **new reservations**;
a player already admitted may finish loading afterwards. Score-based mode
endings may still finish a match before its scheduled time limit.

When no running match can accept a batch, a new match can start at 1v1:

- The gathering timer starts with the second queued player, defaults to
  120 seconds, and is not extended by later arrivals.
- Fewer than two queued players cancels it. A lone squad still needs
  opponents satisfying population balance.
- A full balanced match starts early. Expiry never overrides balance:
  a squad of three plus one solo keeps waiting; three plus two may start.
- Players left after forming a new match start a fresh gathering window.

`SRP_GROWING_GATHER_SECONDS` configures this window independently of the
standard queue (default 120). `SRP_GROWING_POPULATION_EXPONENT` configures
`p` (default 1; finite positive values up to 8). Invalid values fall back
to defaults. `SRP_MATCH_TEAM_SIZE` sets capacity (default ten per team, which
is also the most the experimental descriptor limit allows; set 5, 6 or 7 to
compare smaller matches). The end cutoff stays fixed at
120 seconds. Tests configure gathering and exponent directly without
changing process-global environment variables.

Queued matchmaking levels are refreshed after equipment or rating changes,
before choosing a running match or forming a new one. Refreshing a party
invalidates older batch observations without extending the gathering timer.
Team level sums use these refreshed levels (the `min` field) for newcomers
and launch-time levels for the initial roster. They stay fixed after admission
if equipment or rating later changes. The original reward `match_level` remains fixed at
creation; admissions do not change reward multipliers.
