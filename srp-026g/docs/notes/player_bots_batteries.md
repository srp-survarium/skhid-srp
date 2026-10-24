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
| arrival to put-in, mean (s) (superseded, below) | 2.27 | 2.96 | +0.68 | 0.17 * |
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
- **Superseded:** the two "from arrival" rows timed only the put-ins that
  landed on a world's tick (the tally read the stand-in's per-step put-in
  list at worlds only): 6-7 of about 75 a seed. The next section times
  all of them: with this PR alone, arrival to put-in is 2.79 s.
- **The cost:** arrival to put-in is 0.7 s longer (LIKELY: a use that does
  not reach the box is now judged after the put time too, so each retry
  waits 0.6 s more). Only 6-7 put-ins a seed are timed (the carrier's
  first world within 3 m of its container), so the sample is small. A
  follow-up can judge a miss at once when the bot's own use provably did
  not reach the box.
- The soaks (`raids_in_whole_battery_matches` and the 7 other whole-match
  tests) pass.

## Bots: a trace of the team's battery knowledge and of why a known battery is no Collect candidate

In the Cologne match battery 2 lay under the overhang at spot B and left
every bot's candidates at 855.3 s, and the log could not say why. This PR
adds notes only; no decision changes (below).

### The notes

- **The team's knowledge** (`TeamKnowledge::change_battery`, the
  `batteries` list): a note when the team comes to know a battery,
  forgets it, sees it moved over 1 m, or it comes onto or goes off the
  HUD; a spot only put right again writes nothing. Each says why:
  - `seen by player N from [eye], near` or `..., a clear line at +h m, k
    blocked before it` (`sense_batteries`: within `battery_near_m` no ray
    is cast; else the box's top, middle and foot are tried, the first
    clear line counts);
  - `seen empty by player N from [eye], ...` the same way, for a spot
    believed to hold a battery that is not there;
  - `the HUD shows it`, `the HUD no longer shows it`, `carried by player
    N`, `put in container N` (`hud_update`).
  The HUD's notes are the team's (`team T: ...`, place `usize::MAX`), the
  sightings the bot's own.
- **A bot's Collect candidates** (`collect::take_blocks` and
  `note_take_blocks`, at each commander pass): for each battery the bot
  believes lies about, a note when it stops being a Collect candidate and
  why, once per change per bot: `no stand to take it from`, `no way to
  its stand`, `no way to its stand lately`, `the team keeps falling by
  it` (the reasons of `Ctx::can_take`, now `Ctx::take_block`), `set
  aside` or `scored out`; and `... is a Collect candidate again` when it
  comes back. A battery no longer believed to lie about leaves without a
  note: the knowledge's note says why.
- Either would have settled 855.3 s. The leading guess stays "seen empty"
  from a spot that could not see under the overhang; the next real match
  with the trace reads it.

### No decision changes

The bench's decisions file (`SRP_TRACE_DECISIONS`: every input and note
of each run) of `real_bot_cost` (the six maps, 4v4 and 10v10, 180 s, 10
batteries, the use line on), seeds 1 and 2, base 31d87767 and this PR:
with the new notes taken out, the files are the same byte for byte.

| seed | base lines | PR lines | new notes | the rest |
|---|---|---|---|---|
| 1 | 532,705 | 535,945 | 3,240 | the same |
| 2 | 537,636 | 539,673 | 2,037 | the same |

About 270 new notes a run (the runs had 200-500 notes before). Seed 1's
first 400: `no way to its stand` 119, `scored out` 91, `seen by` 65
(6 of them near), `a Collect candidate again` 59, `carried by` 39,
`no stand to take it from` 10, `no way to its stand lately` 8, `seen
empty` 5, `the HUD shows it` 4.

## Bots: a put-in's use is pressed once the view is on the box; a use that surely missed is judged at once

### Why

After #384 (above), a put-in's use was judged only after the 0.6 s put, and
arrival to put-in grew 0.7 s (on a small sample; see the correction there). The put-in pressed use as soon as it stood
near the container, while its view was still turning to the box (a take
waits for that, `settled`). Each press that missed then cost one use's spacing
plus the 0.6 s put before the next.

### What changed

- `plays/collect.rs`, `State::PutIn`: near the box (eye within
  `USE_NEAR_M`), the use is pressed once the view is on the box's middle (a
  metre over the container's use point, within `SETTLED_RAD`, as a take), or
  after `SETTLE_MS` (1.5 s) there anyway.
- At the press the bot measures how far its use ray (2 m from the eye, along
  its view) passes the box's middle. Over `PUT_MISS_M` = 1 m, the use began
  no put, and the next use need not wait for one. **GUESS:** the use box's
  size is not known; the bench's stand-in takes 0.6 m, so 1 m is well outside
  it. A bot standing close (the box 1.2 m from the eye) is judged missed only
  when its back is nearly to the box, so the guess cuts short no use that may
  have reached it.
- `tests.rs`, the stand-in: each use a carrier presses within 3 m of its
  container is recorded with whether it began a put and how far its ray
  passed the box. The carry tally prints `uses at home N, missed M, far F
  (began a put B), far then in K; first uses from arrival A took S ms`. "Far"
  are the uses the bot judges missed at once (ray over 1 m off); "began a
  put" counts those the stand-in took all the same (a put the bot would
  cut short); "far then in" those whose carrier's next use, from the same
  stand (within 0.3 m) and within 3 s, did begin a put.
- `tests.rs`, the tally: every put-in is timed from the carrier's arrival
  (a log of the run's put-ins). #384's tally timed only those that landed on
  a world's tick (6-7 of about 75 a seed).

### Bench

`real_bot_cost`, the `batt` preset (the six maps, 4v4 and 10v10, 180 s, 10
batteries, the use line on, spread on), no tier, 16 seeds. Base is
b222a338 (#384 and #387) with this PR's tally; "after" adds the bot change.
Means per seed; * = the difference is over 2 SE. Seeds 1-8 left K/D team 1
at -0.06 (2 SE), past early stopping's guard, so all 16 ran.

| | base | after | diff | SE |
|---|---|---|---|---|
| uses at home (within 3 m) | 147.6 | 75.6 | -72.0 | 1.5 * |
| of them missed the box (no put began) | 72.6 | 0.25 | -72.4 | 1.1 * |
| ray over 1 m off (judged missed at once) | 53.8 | 0.12 | -53.6 | 0.7 * |
| of these, began a put all the same | 0 | 0 | 0 | 0 |
| of these, the next use from the same stand went in | 40.9 | 0.06 | -40.8 | 0.9 * |
| arrival to first use, mean (s) | 0.50 | 1.01 | +0.51 | 0.01 * |
| put-ins timed from arrival | 75.8 | 75.9 | +0.1 | 0.8 |
| arrival to put-in, mean (s) | 2.79 | 1.68 | -1.10 | 0.01 * |
| collect: could not put it in | 0.06 | 0 | -0.06 | 0.06 |
| collect: put it in | 43.8 | 42.2 | -1.6 | 1.5 |
| delivered | 75.8 | 75.9 | +0.1 | 0.7 |
| pickups | 101.8 | 100.3 | -1.6 | 1.5 |
| carriers killed | 18.4 | 18.5 | +0.1 | 1.5 |
| plays ended within 1 s | 379.3 | 392.2 | +12.9 | 12.1 |
| A-B-A flips in 10 s | 102.5 | 103.8 | +1.3 | 8.3 |
| stood with a goal (ticks) | 26,608 | 27,179 | +571 | 580 |
| K/D team 0 / team 1 | 1.00 / 1.05 | 1.03 / 1.01 | +0.03 / -0.03 | 0.03 / 0.03 |
| kills (all) | 229.5 | 233.8 | +4.3 | 3.1 |
| team kills | 0.12 | 0.12 | 0 | 0.13 |

- Half the uses at the box missed it: the bot pressed while still turning.
  Now the press waits for the aim (+0.5 s to the first use), and arrival
  to put-in is 1.1 s shorter: the 0.7 s #384 cost is more than won back.
- **The 1 m guess cut short no put:** of the uses the bot judged missed
  at once, none began a put in the stand-in (0 in 16 seeds). In base, 41 a
  seed were followed by a use from the same stand that went in; now
  nearly no use is judged so, because nearly none misses.
- Deliveries and every guard stay within noise.
- **The "arrival to first use" row is a partial sample.** It times 67 base
  and 48 after carries a seed, of 76. The tally reads the game at worlds,
  once a second. When a carry's first use and its put-in, 0.6 s later,
  fell between the same two worlds, the put-in cleared the arrival before
  the use was timed. That happens more often now that the first use goes
  in, so the row leaves out more of after's quick carries. The PR fixes the
  order (uses first); this bench ran before that fix. Arrival to put-in
  times all 76.

## Bots: a battery whose stand is in a pocket no carrier leaves is not taken (Cologne CG-11)

### Why

In the Cologne match a battery lay on the floor of pit A, a pocket: no
walk leads back out. Bots went in for it, took it and could not bring it
out. A carrier cannot sprint (`victory_item_core::can_sprint`), so a pocket
that only a sprint jump leaves keeps a carrier in it. #396 (unstuck's
escapes) marks the pockets (`WalkGrid::pocket_of`) and the team's ways out
of them.

### What changed

- `plays/collect.rs`, `Ctx::take_block`: after the way checks, a battery
  whose take stand is in a pocket (`Ctx::in_pocket`) with no way out for a
  carrier (`Ctx::escape_known(stand, true)`: a standing jump, or one a
  teammate got out by this match) is blocked: "its stand is in a pocket a
  carrier cannot leave" (`POCKET_BLOCK`).
  - Every takeable check goes through it: Collect's candidates and its
    re-checks, Ambush (`can_take`), and #387's take-block notes, which now
    name this reason.
  - No expiry: the pocket's ways out come from the grid, and a teammate's
    escape counts once it is made.
  - It reads the stand, not the battery, since the taker stands there.
- `plays/escort.rs`: no escort for a carrier who gave up getting out of his
  pocket (`Ctx::teammate_trapped(..).gave_up`). An escort would only walk in
  to him.
- `tests.rs`, the carry tally: from the take-block notes, the batteries
  left out for their pocket and the seconds one bot or more left each out
  (to the run's last note when no later note says otherwise), and the
  Ambush plays taken.

### Where it acts

An ignored probe (`collect::tests::pocket_stands_on_the_real_maps`) looks at
every battery spawner on levels 1-8, days 2 and 3: no spawner's take stand
is in a pocket on any map. So the rule acts only on a battery that came
into a pocket later, carried or dropped there, as pit A's battery in
Cologne (a carrier died in it). The unit test
`a_battery_in_a_pocket_no_carrier_leaves_is_not_taken` is that case: a
battery lying in a 3 m pit is no candidate, and says why; one in a pit with
a 0.9 m standing rim (pit A's) still is.

### Bench

`real_bot_cost`, the `batt` preset, tiers none, 4-5, 9-10 and 13-15, seeds
1-8. Base is 54ececcf (#396) with this PR's tally; "after" adds the change.
- None, 4-5 and 13-15: base and after are the same in every count.
- 9-10: one seed parts slightly. Flips -0.25 (SE 0.25), carriers killed
  -0.12 (0.12), delivered +0. No battery was left out for its pocket there
  either, so it is the escort filter: carriers who gave up their pocket
  number 1.6 a seed.
- Left out for the pocket: 0 batteries and 0 s in every tier. The stand-in
  has 0.1-0.6 deaths in pockets a seed, too few for a battery to lie there
  in 8 seeds.
- Ambush takes are unchanged (12.5, 12.5, 15.75, 14.9 a seed).
- Bot ms/s, summed over a seed's 12 runs, after minus base: -0.85 (SE
  0.97), -0.94 (0.93), +0.52 (0.94), +0.66 (0.65). No cost.
- Delivered, flips, quick ends, pocket bot-s and pocket deaths do not move.

## Bots: a goal is reached at its floor's height; a standing take stand beside a high box (Cologne CG-17)

### Why

In the Cologne match a bot stood on the overhang over battery 2's stand at
spot B and counted as there. Every "arrived" test was flat (xz): the steer's
end of a walk in `drive_tick`, Collect's Fetch→Take, and the motor's end of
a route, which only refused an end above the feet (a ledge to jump up to).
So a bot over its goal stopped, and the take went nowhere.

### What changed

- **One rule for "arrived":** within reach across, and the feet at the
  goal floor's height, not over it by more than `motor::ARRIVE_ABOVE_M`
  (0.6 m, GUESS: over a step's or a stair's rise, under a storey) nor under
  it by more than `motor::ARRIVE_BELOW_M` (0.5 m, as before). Two places
  use it:
  - `motor::arrived`, a route's end (#396's landing rule for a last leg
    that is a jump stays);
  - Collect's Fetch→Take (`at_goal_height`, the stand's floor from
    `path::goal_floor`). A bot near across but off that height (over the
    stand on an overhang or a box top, or under it) asks for the stand
    itself (`within(0.0)`, as Take already did from under its stand): a
    goal within reach across is no goal in `drive_tick`, so only the
    route's end decides.
- **Not in `drive_tick`:** the first version also ended `drive_tick`'s
  goal by the height rule. On the tdm bench (8 seeds, against
  base-939b2cca) it raised pocket tries on tier 9-10 by +7.75 (2.3 SE) and
  pocket seconds by +88 (3.0 SE). Limited to goals within 1.2 m, it still
  gave +8.0 tries (3.9 SE) and +58 s (3.9 SE): every intent without its own
  reach asks within 1 m, and so do the reflex's steps, so bots in fights
  walked on to points they stood by, into pits. With only that rule off,
  tdm 9-10 played out as base's, game for game; with only the motor's rule
  off, the tries stayed up (+9.4, 3.6 SE). So `drive_tick` is as it was.
- **A side effect, by design:** a route that ends past a drop is arrived at
  only once the bot has dropped, not at the drop's top.
- `collect::take_stand`: `Tactics::stand_to_take`'s stand, else, for a
  battery on a box top over the crouched eye, the floor beside it whose
  **standing** eye sees it within the use's reach. Collect's five stand
  look-ups and the search's `shut_all_match` use it.
- **The probe** (`pocket_stands_on_the_real_maps`): of about 500 spawners
  on levels 1-8, days 2 and 3, five have no crouched stand. The standing
  eye adds one, level_02 day 2 spawner 39 (stand 1.34 m under the battery);
  the other four are seen from no floor within 1.8 m. Grid algo 5. The
  shared finder's rewrite and marginal-jump stands wait for the grid's
  erosion fix (algo 6), and come back only if the probe's numbers change
  there.
