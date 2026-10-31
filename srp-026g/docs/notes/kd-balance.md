# The sides of a match by K/D and rating

The owner's rules (2026-11-01): "we need to change how matches are balanced: k/d equalizer +
squads have their kd multiplied by 1.3"; "kd should take into account playing on the current
match level"; the match level is "the highest equipment level in the match"; "we need also some
rating system + a bit of diffusion so players are matched randomly a bit too". Picks marked
*(pick)* are the worker's, for the owner to change.

Code: `session::v026g::level_stats` (the record, the formula, the constants),
`session::v026g::rooms` (`Party`, `sides`, `DIFFUSION_PERCENT`), `session::v026g::rewards`
(`Results::raise_match_gear`, what `settle` counts), `session::Strength`, the lobby's `rooms.rs`
(`room_start`, `late_placement`) and `play.rs` (`launch`, `strength_in`, `seated_late`),
`persist/level_stats.rs` (`g26_level_stats`, schema step 12). Tests: the session's `level_stats`,
`rooms` (`the_sides_are_as_even_as_they_can_be`, `a_squad_counts_one_and_three_tenths`,
`the_seed_draws_among_the_ways_near_the_most_even`, `squads_are_never_split_...`), `rewards`
(`a_rated_match_counts_...`), the lobby's `balance_tests` and
`rooms_tests::a_late_joiner_of_higher_gear_raises_the_matchs_equipment_level`.

## The rule

- **Equalizer:** an open room's match splits its players into two sides whose strengths, summed,
  are as equal as they can be. Every split is tried (20 players at most); the sizes come first:
  no side over the room's size, the sides at most one player apart where the squads allow it.
- **A side counts** its players' strengths, and 1 more for each player being there
  (`SRP_BALANCE_PLAYER_KD`, default 1: an average player). So with an odd count the side with a
  player more must be the weaker in strength *(pick, the rule of before in K/D units)*.
- **No mid-match reshuffling.** The sides are made once, at the start.

## A player's strength

At the match's equipment level `L`:

```
strength = max(0, kd + RATING_KD * trust * rating)

overall  = (all kills + 10 * 1.0)     / (all deaths + 10)
kd       = (kills at L + 10 * overall) / (deaths at L + 10)
trust    = matches at L / (matches at L + 5)
rating   = the rating at L, in points (0 is the average)
RATING_KD = 1 / 2.2 = 0.45 K/D a point
```

- **K/D carries the fighting, the rating the winning** (batteries, points, playing for the team),
  which kills alone do not show *(pick)*.
- **Damped, never infinite or NaN** *(pick)*: 10 deaths more at the next K/D up
  (`PRIOR_DEATHS`). A level of few deaths leans on the player's overall K/D, a player of few
  deaths overall on 1.0. Examples: a new player 1.0; 3 kills and no death 1.3 overall; 30 kills
  and 10 deaths 2.0. Whole thousandths (`Strength`), so every sum and comparison is exact.
- **The rating** is Elo-like (`balance::changes_by_share`, the one the balance rating used since
  2026-10-13): side 0's expected chance is `1 / (1 + e^-(rating sum 0 - rating sum 1 + 2.2 *
  players apart))`; each player moves by `0.8 * (1 + 3 / (1 + matches at L)) * 2 / players *
  share * (result - expected)`, the result 1, 0.5 or 0. So a player's first matches at a level
  move it four times as far (the K-factor), and its rating counts for nothing before its first
  rated match there, half after 5 (`TRUST_MATCHES`: the provisional period) *(picks)*.
- **0.45 K/D a point** follows from the rating's own odds: a player more is 2.2 points, and a
  player is 1 K/D *(pick)*.
- **Squads:** each member of a squad that plays together counts 1.3 times its strength
  (`SQUAD_FACTOR` 13/10). A squad is one party: its members are always on one side.
- **Bots:** the server's own bots count 1.0 (`BOT_STRENGTH`), for where they are put (Player vs
  Bots) and the log *(pick)*.

## The match's equipment level

- The highest gear level (`matchmaking::gear_level`, what a room's max level compares) of the
  players placed at the start, worked out before the split: everyone counts for its strength at
  that level.
- **Late joiners and backfill:** the level stays what the match had at its start, unless a joiner
  of higher gear raises it; the joiner (and its mate going in with it) counts at the level then.
  Players already in keep the strength they were seated with *(pick: no recount mid-match)*.
- The kills, deaths and rating changes of a match are counted at its level when it ends (the
  highest of everyone who was in it, late joiners too).

## Diffusion

- Of the splits that fit the sizes and are fewest players apart, one is drawn at random among
  those whose strengths are apart by no more than the most even split's plus 1% of what both
  sides count for together (`DIFFUSION_PERCENT` = 1) *(pick)*. 8 v 8 of average players count
  for 32: 0.32 of a K/D.
- The draw is by a seed the lobby takes from its random generator at each start; the same parties
  and seed always make the same sides (tests fix the seed). Squads are never split by it.

## Late joiners

- As before (`matchmaking.md` §9, the owner's of 2026-10-19): the side with fewer players; of two
  alike, the side it played on before; then the side that counts for less (players, strengths
  and the lead); a squad's second goes beside its mate. Only the strengths are now K/D and rating.

## Rooms where sides are picked

- A private room (a PIN) plays its owner's arrangement as it is: nothing is balanced there, and
  its matches count nothing (they are unrated). Only open rooms (Play and Join) are balanced.

## What a match counts

- **Kills, deaths and the rating at the level move together, in exactly the matches that move the
  balance rating** *(pick: one rule)*: a PvP match with a trusted winner (or a draw), both sides
  in it, at least 4 players in it for half of it or more (`SRP_BALANCE_RATED_PLAYERS`).
- **Not counted:** Player vs Bots, any match with a bot (the server's or a test bot), a private
  room's, a match with no trusted result, one a whole side left (Deserted), one called off, and
  a match the server does not know the level of (made before a restart).
- **A player who left early** counts: its kills and deaths in full, its rating moved by its share
  of the match (time in it against the longest stay), its team's result. One whose game never
  came in counts nothing.
- The ranking's rating and the pay do not depend on any of this.

## Kept

- `g26_level_stats` (schema step 12): one row per account and equipment level, kills, deaths,
  rating (thousandths of a point) and rated matches. An account of before has no rows.
- **Nothing of before is counted:** at the release everyone starts at 1.0 at every level and
  the counts start from zero. A backfill was looked at: the kept results (`g26_results`) hold
  only the last 64 matches' screens, with kills and deaths but no gear levels; the lobby log has
  each player's level at the start but only each side's kills; the match recordings
  (`SRP_RECORD_DIR`, size-capped) have the rosters and kits, and the kills would have to be
  read back from their snapshots by a new tool. Not cheap for what it gives.
- The overall balance rating (`Account::balance`) is still moved and written in the log's
  "sides" lines, for `scripts/balance-refit`; it no longer counts.

## Release

- Server only; no client change. Team assignment is decided before the match and recordings keep
  each place's team and squad, so replays of older games play out as they did: no
  `simulation_changes.tsv` row.
- `SRP_BALANCE_PLAYER`, `_PLAYER_LEVEL`, `_RATING`, `_TRUST_MATCHES` (gear levels) are no longer
  read; one left set is an error in the log. Unset them on the live host.
