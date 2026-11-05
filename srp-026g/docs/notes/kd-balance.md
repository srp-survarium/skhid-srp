# The sides of a match by strength

The owner's rules (2026-11-01): "k/d equalizer + squads have their kd multiplied by 1.3"; "kd
should take into account playing on the current match level"; the match level is "the highest
equipment level in the match"; a rating: "we don't need that", "simpler the better"; "we can add
small noise", so the same players do not always end up on the same side. And (2026-11-05, after
Chemical Plant match 0x73d0500e: even at its start, 15.5 apart and 118-53 after four late joiners
went onto the side of the two best players): the kills a minute count too, not points a minute;
strong players go one against another; late joiners can't skew balance; a squad that is too
strong can't start a game at all. Picks marked *(pick)* are the worker's, for the owner to change.

Code: `session::v026g::level_stats` (the record, the formula, `rivals`, `too_strong`),
`session::v026g::rooms` (`Party`, `sides`, `NOISE_MILLI`), `session::matchmaking_policy`
(`place_joiner`), `session::v026g::rewards` (`Results::raise_match_gear`, what `settle` counts),
`session::Strength`, the lobby's `rooms.rs` (`room_start`, `late_placement`, `too_strong_squad`)
and `play.rs` (`launch`, `strength_in`, `seated_late`, `Running::rivals`),
`persist/level_stats.rs` (`g26_level_stats`, schema steps 12 and 14). Back-test:
`scripts/balance-refit/refit.py --db`.

## The rule

- **Equalizer:** an open room's match splits its players into two sides whose strengths, summed,
  are as equal as they can be. Every split is tried (20 players at most); the sizes come first:
  no side over the room's size, the sides at most one player apart where the squads allow it.
- **Strong players go one against another** (the owner's of 2026-11-05: "the game should never
  allow the second strongest player to join the first strongest one"): of the splits that fit
  the sizes, only those that put no rivals on one side (`level_stats::rivals`), then the most
  even of them, noise included. Rivals are
  - **the two strongest players**, unless they are in one squad (the squad is the players' own
    choice *(pick)*), or the second is tied with a third (then no two are the strongest);
  - **a squad with a strong player and every other party with one** (the owner's: "every other
    strong player in the match then goes against that squad"; the simplest version of it: any
    squad with a strong player *(pick)*).
  Where the sizes leave no split without rivals together, the fewest of them.
- **Strong** *(pick)*: a strength of 2.5 or more (`STRONG`, 2.5 times an average player; the data
  below). Logged at start.
- **A squad too strong to play** *(pick: two strong members or more, each by its own strength at
  the level they would play at, the highest gear level of theirs)*: Play, Join (a private room's
  too) and a queue refuse it (`RoomCode::SquadTooStrong`, 18), the lobby's Play is red and says
  "Split up" with the reason "Squad too strong: split up" in its tip (the presets' list's bit 64),
  and an open room's match start or late join takes its members out of the room, told why.
  Logged when refused. Its members play once they split up.
- **A side counts** its players' strengths, and 1 more for each player being there
  (`SRP_BALANCE_PLAYER_KD`, default 1: an average player). So with an odd count the side with a
  player more must be the weaker in strength *(pick)*.
- **Squads:** each member of a squad that plays together counts 1.3 times (`SQUAD_FACTOR`). A
  squad is one party: its members are always on one side. Who is strong and who the two strongest
  are is by each player's own strength, without the 1.3.
- **Bots:** the server's own bots count 1.0 (`BOT_STRENGTH`) and are nobody's rival.
- **No mid-match reshuffling.** The sides are made once, at the start.

## A player's strength

At the match's equipment level `L`:

```
overall K/D  = (all kills + 10)                / (all deaths + 10)
K/D          = (kills at L + 10 * overall K/D) / (deaths at L + 10)
overall kpm  = (timed kills * 60 + 900 * 0.83)        / (seconds + 900)
kpm          = (timed kills at L * 60 + 900 * overall) / (seconds at L + 900)
strength     = K/D * min(kpm, 1.245) / 0.83
```

- Never infinite or NaN; a new player is 1.0. A level of few deaths leans on the player's
  overall K/D, a player of few deaths overall on 1.0 *(pick: 10 deaths, `PRIOR_DEATHS`)*.
  Examples: 3 kills and no death 1.3 overall; 30 kills and 10 deaths 2.0.
- **The kills a minute** (the owner's of 2026-11-05: "a player who goes 6/1 sitting on a base
  counts for less than one who goes 60/15"): of the player's rated matches at `L`, its kills and
  its time in them (`PlayerResult::time_in_match_ms`, what the balance rating's shares use,
  `matchmaking.md` §9.4), against the server's average (`AVERAGE_KILL_RATE`, 0.83 a minute). A
  level of few minutes leans on the player's overall rate, a player of few minutes on the average
  *(pick: 15 minutes, `PRIOR_SECONDS`, one match)*. 6 kills in a match of 15 minutes are 0.62 a
  minute, 0.74 of the average; 60 kills are 2.4.
- **At most 1.5 times the average** (`KILL_RATE_MOST`, 1.245 a minute) *(pick)*: a fast killer
  counts up to half as much more, a slow one down toward nothing. Unbounded, a few players counted
  for ten and more and the back-test's sides came out worse than by the K/D alone.
- **Only kills counted with their time** make the rate (`TimedKills`, schema step 14): the kills
  and deaths of before it stay in the K/D (no reset), but have no time, so everyone's rate starts
  at the average at the release and becomes its own over its next matches (half its own after one
  match at a level).
- Whole thousandths (`Strength`, `KillRate`), so every sum and comparison is exact.

## What the data says (back-test of 2026-11-05)

`scripts/balance-refit/refit.py <logs> --db <copies of srp.db>`: the 2,080 match reports against
players kept in the 48 database backups of 2026-10-16 to 11-05 and the live one, and the 497
matches against players logged since the K/D rule (2026-11-01 22:37 to 11-05 21:00).

- **The average:** 77,190 kills in 93,319 minutes, **0.827 a minute**. Of the 272 players of 30
  minutes or more, the median is 0.56, the 90th percentile 1.22, the 99th 3.49.
- **The prior:** the players' rates are far apart against the noise of a minute (a Poisson floor
  of 3 minutes); the winners' fit is flat from 5 to 60 minutes (log loss 0.6762 to 0.6768), so
  one match's worth.
- **K/D x kpm predicts the winners better than the K/D alone**, each player by its other matches,
  by its share of each: 724 decided matches of 4 players or more, log loss **0.6859 -> 0.6763**
  (-0.0096, 90 % range -0.0150 .. -0.0056). On the 326 logged matches since the K/D rule, by who
  was in each minute, the K/D at the level as logged **0.5583**, times the kpm factor **0.5235**
  (-0.0348, range -0.0555 .. -0.0131). Uncapped the factor made the logged matches worse (0.5660);
  the cap at 1.5 was the best of 1, 1.5, 2, 2.5, 3 there (2 on the reports, 0.6747 against
  0.6763).
- **Strong at 2.5:** 11 % of the players at the starts, 8 % of the 194 players (15) strong in a
  match or more; at 2.0 it would be 16 % and 13 %, at 3.0 7 % and 6 %. Every player the owner
  named is strong at its levels: Anathema 7.4 (level 10) and 4.7 (15), СтрелокВонючийПенис 6.5 and
  3.8, Strelok_NK 4.0 and 3.3, Dr1t 3.0 and 3.4, Топотунчик 4.5 and 3.1, Fasojib 3.7 and 2.6, Alex
  Stan 4.2 (15); their medians over their matches 3.1 to 5.7, the next players' 2.2 to 2.8.
- **The sides it would have changed:** 60 of the 497 starts (the two strongest together 56, a
  strong player beside a squad with one 4). Squads too strong: 9, in 61 matches.
- **Late joiners** (1,907 joins): 1,679 in at once (565 of them on the other side than they
  went), 228 would have waited, 124 of them let in later (median 1.4 min, longest 12.9 min) and
  104 to the match's end (a room of 8 a side assumed).
- 0x73d0500e: Anathema (7.3) came 26 s in, with ŚąnguiśExtręmiś (3.3) on team 0, the weaker side
  then: he would have waited until team 1 was the weaker.

## Noise

- Before the split, each party's strength is multiplied by a factor drawn from 0.95 to 1.05
  (`NOISE_MILLI` = 50, the owner's pick); then the most even split of those strengths is taken.
  No history of past teams, no other state.
- Drawn by a seed the lobby takes from its random generator at each start: the same parties and
  seed always make the same sides (tests fix the seed). Squads are never split by it, nor are
  rivals put together.
- Checked on rooms of realistic K/Ds (0.45 to 1.9; 6 v 6, 8 v 8 and 10 v 10, with squads of 2
  and 3), 1000 starts each: the sides change at 97 to 100 % of consecutive starts, about 2 to 3
  players a side moving; the true imbalance is 0.06 to 0.13 of a K/D at the median, 0.4 to 0.6
  at worst (a side of 8 to 10 counts 17 to 22). At 0.9 to 1.1 the sides changed as often, at
  twice the imbalance (worst 1.1).

## The match's equipment level

- The highest gear level (`matchmaking::gear_level`) of the players placed at the start, worked
  out before the split: everyone counts at that level.
- **Late joiners** (the owner's of 2026-11-05: "late joiners can't skew balance"): the level
  stays, unless a joiner of higher gear raises it; the joiner (and its mate going in with it)
  counts at the level then. Players already in keep the strength they were seated with *(pick)*.
  A joiner goes **only to the weaker side** (what the side's players in the match now count for,
  and its lead: `matchmaking.md` §9), within the room's size counting the places kept; never
  where it would be on one side with a rival (the two strongest of the players in and the
  joiner, a squad with a strong player and another strong one); one whose mate is in the match
  only when its mate's side is the weaker one *(pick: the squad stays on one side, and the
  joiner waits rather than make the stronger side stronger)*. Otherwise it waits in the room,
  told the match is full, until the sides let it in or the next match. "Fewer players first"
  and "the side played on before" are no rules any more.
- The kills, deaths and time of a match are counted at its level when it ends (the highest of
  everyone who was in it, late joiners too).

## What a match counts

- **Kills, deaths and time are counted in exactly the matches that move the balance rating**
  *(pick)*: a PvP match of an open room or Play with a trusted winner (or a draw), both sides in
  it, at least 4 players in it for half of it or more (`SRP_BALANCE_RATED_PLAYERS`). A player who
  left early counts its kills and deaths in full, and its time in the match.
- **Not counted:** Player vs Bots, any match with a bot, a private room's (its owner arranges the
  sides; nothing is balanced there), one with no trusted result, one a whole side left, one
  called off, and one the server does not know the level of (made before a restart).
- The overall balance rating (`Account::balance`) is still moved and written in the log's "sides"
  lines, for `scripts/balance-refit`; it no longer counts for the sides.

## The log

- At start: the rule and its numbers (the average rate, its most, `STRONG`).
- `Match 0x… sides (equipment level L): team 0 counts X: <player>, … | team 1 …`, each player
  `<name> <counts> (gear g, level l, rating r of m matches, at L: kd K of k/d, kpm R of k/m min[,
  squad][, strong])`; the "late join" line writes each joiner so. `scripts/balance-refit` reads
  them (the parts after the rating since 2026-11-01; kpm, squad and strong since 2026-11-05).
- `<name>: refused, its squad is too strong to play (<member> <strength>, …; strong from 2.50)`
  at Play, Join and a queue; `Room <key>: squad … too strong to play at its match's start (or a
  late join) …: out of the room`.

## Kept

- `g26_level_stats` (schema step 12): one row per account and equipment level, its kills and
  deaths; step 14 adds `timed_kills` and `seconds` (0 for the rows of before). An account of
  before step 12 has no rows: everyone started at 1.0 then *(pick: the kept results have no gear
  levels, so there is nothing cheap to backfill from)*.

## Release

- Server and lobby movie (the red Play and its texts in the dictionaries). The sides are decided
  before the match and recordings keep each place's team and squad, so replays of older games
  play out as they did: no `simulation_changes.tsv` row.
- `SRP_BALANCE_PLAYER`, `_PLAYER_LEVEL`, `_RATING`, `_TRUST_MATCHES` (gear levels) are no longer
  read. Unset them on the live host.
