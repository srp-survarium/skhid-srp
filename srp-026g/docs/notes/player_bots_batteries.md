# Player bots: batteries (0.26g)

How the server's bots take, carry and put in batteries, what they believe
about who carries one, and what changed. The write-up of #361 (the first
battery audit) stays in `player_bots_behaviour.md`. Tags as elsewhere:
PROVEN, LIKELY, GUESS.

## Bots: a put-in is judged after the put; failed put-ins move no belief to a teammate who pressed nothing

### What the Cologne match showed

In the Cologne batteries match (a recorded player match, 2026-10), bots
ended six put-ins as "could not put it in". After each one the same carrier's
battery went into the container 0.3-3.3 s later. Twice the team's belief of
who carried the battery moved, about 1 s before the put landed, to a teammate
who had never pressed use on it.

Three causes, all from #361's numbers being shorter than the game's put:

- **The put lasts 0.6 s.** The battery's put state is `states[2].put_time`
  = 0.6 s (PROVEN: the state lasts it; LIKELY: the battery goes in at its
  end, which is what the match shows). #361 judged a put-in 300 ms
  (`USE_SEEN_MS`) after the first world after the press, so a world taken
  mid-put still showed the battery carried and the bot called it failed.
- **The belief gave up at 1.5 s** (`PUT_IN_SEEN_MS`) after a put-in that
  should have landed. With 0.6 s of put and up to 2 s of the client's lag,
  that was too short.
- **The rule-out picked any teammate near the battery.** After failed
  put-ins the belief moved to the next teammate near it, even one whose own
  use never met it. After a death with the battery still carried, anyone
  near is still a fair guess, since the battery drops where he died.

The bench's stand-in made it worse: it put batteries in at the press, so
no bench run ever saw a put in progress.

### What changed

- `plays/collect.rs`: `use_seen(cx, pressed, lasts)` judges a use only
  after a world at `pressed + lasts + USE_SEEN_MS` (or after the wait).
  PutIn passes `PUT_TIME_MS` = 600; Take passes 0, as before.
- `view.rs`: `PUT_IN_SEEN_MS` 1.5 s -> 3 s (GUESS: the 0.6 s put + 2 s lag +
  slack). Each other candidate carrier keeps whether its own use met the
  battery; after failed put-ins only those can take over the belief. After
  a death, anyone near still can. The note says which: "no one else whose
  use met it" / "no one else near it".
- `tests.rs`, the stand-in's battery use model: a put-in starts at the press
  and goes in `SRP_BENCH_PUT_MS` (default 600) later, if the carrier is
  still alive and still carries that battery; 0 puts it in at the press, as
  before. The carry tally adds each carrier's arrival (first world within
  3 m of its container) and prints "put-ins from arrival N took S ms".

### Bench

`real_bot_cost` (real maps, 4v4 and 10v10, maps 6/2, 1/2, 3/3, 2/2, 4/2,
5/2, 180 s, 10 batteries, the use line on), 16 seeds, both sides on the new
stand-in (0.6 s put). "Base" is the stand-in commit alone; "after" adds the
bot change. Means per seed; * = the difference is over 2 SE.

| | base | after | diff | SE |
|---|---|---|---|---|
| collect: could not put it in | 47.00 | 0.06 | -46.94 | 1.09 * |
| collect: put it in | 43.12 | 44.81 | +1.69 | 0.87 |
| delivered | 74.75 | 75.69 | +0.94 | 1.22 |
| pickups | 98.75 | 98.06 | -0.69 | 1.80 |
| carriers killed | 17.25 | 16.50 | -0.75 | 1.18 |
| collect takes | 584.7 | 552.5 | -32.2 | 11.8 * |
| plays ended within 1 s | 482.7 | 465.9 | -16.8 | 7.8 * |
| A-B-A flips in 10 s | 154.3 | 158.5 | +4.3 | 5.6 |
| stood with a goal (ticks) | 28,662 | 28,954 | +292 | 843 |
| beliefs moved | 0.25 | 0.00 | -0.25 | 0.19 |
| belief right / wrong (world-s) | 2,355 / 0 | 2,332 / 0 | -23 | 46 |
| put-ins timed from arrival | 5.81 | 7.12 | +1.31 | 0.73 |
| arrival to put-in, mean (s) | 2.27 | 2.96 | +0.68 | 0.17 * |
| K/D team 0 / team 1 | 1.00 / 1.04 | 1.02 / 1.01 | | 0.02 / 0.03 |
| kills team 0 / team 1 | 124.8 / 127.3 | 124.6 / 123.8 | -0.2 / -3.4 | 1.8 / 2.0 |
| team kills | 0.06 | 0.12 | +0.06 | 0.06 |

- With the stand-in's real put, #361's bots called almost every put-in
  failed (47 per seed, as many as went in) and started the play again;
  now 1 in 16 seeds. The plays that end within 1 s and Collect's re-takes
  fall with it.
- Deliveries, pickups, carriers killed, kills and K/D do not move beyond
  noise. "Intercept: lost him" rose 6.8 -> 8.4 (SE 0.8), the only other
  metric past 2 SE of about 40; LIKELY noise.
- **The cost:** arrival to put-in is 0.7 s longer (LIKELY: a use that does
  not reach the box is now judged after the put time too, so each retry
  waits 0.6 s more). Only 6-7 put-ins a seed are timed (the carrier's
  first world within 3 m of its container), so the sample is small. A
  follow-up can judge a miss at once when the bot's own use provably did
  not reach the box.
- The soaks (`raids_in_whole_battery_matches` and the 7 other whole-match
  tests) pass.
