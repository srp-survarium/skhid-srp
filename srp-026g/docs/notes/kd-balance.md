# The sides of a match by K/D

The owner's rules (2026-11-01): "k/d equalizer + squads have their kd multiplied by 1.3"; "kd
should take into account playing on the current match level"; the match level is "the highest
equipment level in the match"; a rating: "we don't need that", "simpler the better"; "we can add
small noise", so the same players do not always end up on the same side. Picks marked *(pick)*
are the worker's, for the owner to change.

Code: `session::v026g::level_stats` (the record, the formula), `session::v026g::rooms` (`Party`,
`sides`, `NOISE_MILLI`), `session::v026g::rewards` (`Results::raise_match_gear`, what `settle`
counts), `session::Strength`, the lobby's `rooms.rs` (`room_start`, `late_placement`) and
`play.rs` (`launch`, `strength_in`, `seated_late`), `persist/level_stats.rs` (`g26_level_stats`,
schema step 12).

## The rule

- **Equalizer:** an open room's match splits its players into two sides whose strengths, summed,
  are as equal as they can be. Every split is tried (20 players at most); the sizes come first:
  no side over the room's size, the sides at most one player apart where the squads allow it.
- **A side counts** its players' strengths, and 1 more for each player being there
  (`SRP_BALANCE_PLAYER_KD`, default 1: an average player). So with an odd count the side with a
  player more must be the weaker in strength *(pick)*.
- **Squads:** each member of a squad that plays together counts 1.3 times (`SQUAD_FACTOR`). A
  squad is one party: its members are always on one side.
- **Bots:** the server's own bots count 1.0 (`BOT_STRENGTH`).
- **No mid-match reshuffling.** The sides are made once, at the start.

## A player's strength

At the match's equipment level `L`:

```
overall  = (all kills + 10)           / (all deaths + 10)
strength = (kills at L + 10 * overall) / (deaths at L + 10)
```

- Never infinite or NaN; a new player is 1.0. A level of few deaths leans on the player's
  overall K/D, a player of few deaths overall on 1.0 *(pick: 10 deaths, `PRIOR_DEATHS`)*.
  Examples: 3 kills and no death 1.3 overall; 30 kills and 10 deaths 2.0.
- Whole thousandths (`Strength`), so every sum and comparison is exact.

## Noise

- Before the split, each party's strength is multiplied by a factor drawn from 0.9 to 1.1
  (`NOISE_MILLI` = 100); then the most even split of those strengths is taken. No history of
  past teams, no other state.
- Drawn by a seed the lobby takes from its random generator at each start: the same parties and
  seed always make the same sides (tests fix the seed). Squads are never split by it.
- Checked on rooms of realistic K/Ds (0.45 to 1.9; 6 v 6, 8 v 8 and 10 v 10, with squads of 2
  and 3), 1000 starts each: the sides change at 98 to 100 % of consecutive starts, about 2 to 3
  players a side moving; the true imbalance is 0.2 to 0.3 of a K/D at the median, 1.1 at worst
  (a side of 8 to 10 counts 17 to 22). At 0.95 to 1.05 the sides change as often, at half the
  imbalance (worst 0.6).

## The match's equipment level

- The highest gear level (`matchmaking::gear_level`) of the players placed at the start, worked
  out before the split: everyone counts at that level.
- **Late joiners:** the level stays, unless a joiner of higher gear raises it; the joiner (and
  its mate going in with it) counts at the level then. Players already in keep the strength they
  were seated with *(pick)*. They go where they went before (`matchmaking.md` §9): the side with
  fewer players, the side played on before, then the side that counts for less.
- The kills and deaths of a match are counted at its level when it ends (the highest of everyone
  who was in it, late joiners too).

## What a match counts

- **Kills and deaths are counted in exactly the matches that move the balance rating** *(pick)*:
  a PvP match of an open room or Play with a trusted winner (or a draw), both sides in it, at
  least 4 players in it for half of it or more (`SRP_BALANCE_RATED_PLAYERS`). A player who left
  early counts its kills and deaths in full.
- **Not counted:** Player vs Bots, any match with a bot, a private room's (its owner arranges the
  sides; nothing is balanced there), one with no trusted result, one a whole side left, one
  called off, and one the server does not know the level of (made before a restart).
- The overall balance rating (`Account::balance`) is still moved and written in the log's "sides"
  lines, for `scripts/balance-refit`; it no longer counts for the sides.

## Kept

- `g26_level_stats` (schema step 12): one row per account and equipment level, its kills and
  deaths. An account of before has no rows: everyone starts at 1.0 at the release *(pick: the
  kept results have no gear levels, so there is nothing cheap to backfill from)*.

## Release

- Server only; no client change. The sides are decided before the match and recordings keep each
  place's team and squad, so replays of older games play out as they did: no
  `simulation_changes.tsv` row.
- `SRP_BALANCE_PLAYER`, `_PLAYER_LEVEL`, `_RATING`, `_TRUST_MATCHES` (gear levels) are no longer
  read. Unset them on the live host.
