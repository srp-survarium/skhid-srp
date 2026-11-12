# Predicting other players' inputs, fitted on recorded matches

The owner, 2026-11-05: "we do have packets, right? can we use them to fit our prediction, and use
the best model we find (if there is such a thing)".

0.26g shows another player by carrying his last input forward until the next one arrives
(`pixel_correct_shooting_069.md` sections 3 and 5, PR #1274). This note measures, on the live
server's recorded matches, how wrong that is and whether any model of the next inputs does better.
The tools are `scripts/prediction-fit/` (README there); the chosen model's table is
`prediction_fit_table.tsv` beside this note. Only aggregate numbers are here: the recordings hold
account names and ids and stay in `private/`.

Status: **fitted and scored offline**. Nothing is implemented in the client; the in-game side
(PR #1279, the remote-player projection of the Rust client transition) takes the table later.

Contents:
1. Short answers
2. The data
3. Delays
4. The harness
5. The movement proxy and its own error
6. Results
7. Lookup tables, one feature at a time
8. A per-map flow table
9. The choice
10. What the in-game side needs
11. Limits

---------------------------------------------------------------------------

## 1. Short answers

- **Movement: nothing beats hold-last.** Fading to a stop, release-hazard tables, Markov chains of
  one and two states, the owner's lookup tables grown to six features (3.9 MB) and gradient
  boosting all lose or tie, even when told that nothing changed up to t - d (more than a client
  knows). The best hold length any of them finds is "never let go within the horizon". The
  game's 0.3 s speed blend is why: a key change moves the body little in its first ticks, and a
  guessed change that did not come costs as much as a missed one.
- **The position error of hold-last is already small at the delays players have:** 1.42 cm mean,
  19 cm p99 at the realistic delay (median 84 ms); 0.3 / 1.3 / 3.1 / 9.0 cm mean at 50 / 100 /
  150 / 250 ms (p99 4 / 14 / 28 / 65 cm). Centimetres; most of the tail is turning while moving,
  not keys.
- **Look: carrying a turn on helps, a little.** A turning player's last turn repeated once, scaled
  by a gain from a 7-cell table (14 bytes), cuts the look error from 3.23 to 2.67 degrees mean
  (-17 %) at the realistic delay, 8.5 to 7.8 at 250 ms, and the position error from 1.42 to 1.37
  cm (the heading). That is 3 % behind the best a client can run (2.60 degrees, an 18 KB table)
  and 13 % behind gradient boosting told the delay (2.37 degrees).
- **Recommendation:** keep hold-last for every key and the mask (it is world state anyway); add
  the look projection only if the in-game side builds the projection for its own sake. Nothing
  here beats hold-last by a margin that matters in position: the gain is millimetres on average.
  Correction smoothing of the tail (PR #1279) is worth more than any prediction here.
- **A per-map flow table does not pay** (section 8): it makes the look slightly worse at about
  2.7 KB a map.
- **Delays** (section 3): the server gets an input a median 31 ms after its game time (p90 64,
  p99 159); a remote player's input reaches another client a median 84 ms after its time (p90 141,
  p99 327), not the 115 ms the earlier note assumed.

---------------------------------------------------------------------------

## 2. The data

From the live server's recordings (`SRP_RECORD_DIR`), copied on 2026-11-05 at 2 MB/s (all 959
kept files, 4.9 GB, recorded 2026-11-02 13:53 to 2026-11-05 04:05 UTC), decoded with
`prediction-decode`: every recording read to its end; 2,016 client datagrams the transport's
parser refuses, 2,879 inputs whose later copy differs from the first (the first is kept).

| What | Value |
|---|---|
| Recordings / with human inputs | 959 / 906 |
| Matches by humans | 1: 660 (against bots), 2-5: 86, 6-10: 111, 11+: 49 |
| Matches by mode id | 4: 518, 2: 300, 5: 46, 3: 42 |
| Player-hours in a match / alive / combat | 489 / 404 / 119 |
| Client revisions seen (players) | 42 to 48 |
| Inputs (first copy of each place and game ms) / ticks with one | 27.4 million / 20.1 million |
| Alive time moving / turning | 75 % / 61 % of ticks |
| Runs of the held keys (movement, aim, crouch, sprint) | median 5 ticks (250 ms), p90 21, p99 164 |
| Split | 622 training matches (473,088 samples), 283 test matches (237,000 samples a delay set) |

- An input is sent only when something changes (keys or a turn); 2.1 % are precise (0xDD). A
  turning player sends one a tick.
- Alive time: between two uploaded worlds of the same life (same spawn time), at most 6 s apart.
  Patched clients upload about one world a second.

---------------------------------------------------------------------------

## 3. Delays

- **Server lateness:** the match clock when the server got an input, less its game time. The
  clients stamp their present, so this is about the uplink: median 31 ms, p90 64, p99 159 (20.6
  million inputs, mean 40).
- **Round trip per player** (the server's datagrams to their first ack): median 81 ms, p90 145,
  p99 322 (2,480 player-matches).
- **End to end, the delay d of the harness:** the sender's lateness at that moment (its nearest
  input) + half a random other human's round trip in the same match (the downlink; a match with
  one human uses his own) + up to a frame (16.7 ms). Receivers run about the server's clock (0xE5,
  0xE7). Over all samples:

```
 percentile    5    10    25    50    75    90    95    99   mean
 d (ms)       48    54    67    84   107   141   178   327     95
```

  With 50 ms ticks, d = 84 ms means the newest one or two ticks of a remote player are a guess.

---------------------------------------------------------------------------

## 4. The harness

- **A sample** is a moment t of a living player (between two uploaded worlds of the same life,
  at most 6 s apart) and a delay d. The model sees the player's inputs up to t - d (the ticks up
  to K0 = floor((t - d) / 50)) and gives inputs for the ticks K0 + 1 .. floor(t / 50). Samples
  are every 2 s of a player's alive time (`--every 40` ticks), at a random part of the tick.
- **Delay sets:** the realistic one (section 3) and fixed 50, 100, 150, 250 ms.
- **Split by match** (a hash of the recording's name: 30 % test), never by sample. Every model
  is fitted on the training matches at the realistic delay and scored on the test matches.
- **Scores at t:**
  - position error: the true inputs and the predicted ones moved on by the movement proxy
    (section 5) from the same body state at K0 (its 0.3 s speed blend warmed up over the 12 ticks
    before), the tick under way counted by its part gone;
  - look error: the angle between the yaw/pitch the true and the predicted turns add up to;
  - action error: the share of predicted ticks whose action bits (everything but the movement and
    stance keys) differ from the truth.
- **Subsets:** all alive time; combat (the player's own fire key within 3 s); PvP (matches with
  two humans or more; 73 % of the matches are one human against bots, section 2).
- **Two kinds of model.** Inputs are change events (PR #1279's finding): carrying the last one
  forward is what the player did until a new one comes, and it is world state. So a better guess
  can only be a temporary projection, and the client cannot know whether a change is on its way:
  it knows the newest node and the time since it.
  - "Delay-aware" models (sections 6 and 7: fade, hazard, Markov, the tables, damped look, the
    ceiling) are told that nothing changed up to t - d. That is more than a client knows, so
    their scores are an upper bound of what a projection of that kind could gain. A model that
    guesses a change that did not come is scored for it.
  - "Projected" models (`projected-look[...]`) are what a client can run: one input after the
    newest node, from that node and the time since it only, scored over everything from the
    node on (the ticks with no input before t - d included).

---------------------------------------------------------------------------

## 5. The movement proxy and its own error

The proxy is the match server bots' model (`servers/match-server/src/v026g/bots/kin.rs`, read in
the client and checked on recordings there): each tick the body goes at the desired speed of its
keys (walk 3.5 / 2.5 / 3.0 m/s forward / side / back, aimed 3.0 / 2.0 / 2.5, crouched 2.5 / 2.0 /
2.0, crouched aimed 2.0 / 1.5 / 1.5, sprint 5.0), reached by a linear blend over 6 ticks in the
body's frame; a tick's turn is made before its move. Walls, slopes, jumps, hits and the kit's
speed level (x0.95..1.05) are not known.

Checked against the uploaded worlds: from one world of a life to the next, the body moved on by
the proxy with the true inputs, against where the later world has it (jumps left out):

```
 gap between worlds   pairs    proxy error m     moved m        yaw error p99
                               p50     p90       p50    p90     rad
 50-99 ms            153436   0.025   0.084     0.16   0.28     0.002
 100-199 ms            9748   0.052   0.162     0.34   0.59     0.081
 200-299 ms            2846   0.093   0.291     0.65   1.25     0.287
 300-499 ms            5610   0.152   0.435     1.11   2.07     0.120
 500-999 ms           29890   0.209   0.667     1.89   4.09     0.033
 1000-2000 ms        315337   0.205   0.802     2.25   5.33     0.006
```

The world's horizontal speed against the proxy's at the same tick: median difference 0.24 m/s
(the kit's speed level, x0.95..1.05, is not modelled).

The proxy's own error is about a sixth of the distance moved over 50-300 ms. The yaw summed from
the inputs equals the worlds' yaw (p99 error under 0.3 rad over all gaps; median 0). The scores
of section 6 compare two runs of the same proxy, so its bias largely cancels; walls would make
real errors smaller still (a body stopped by a wall needs no prediction).

---------------------------------------------------------------------------

## 6. Results

Models (all give inputs; the game moves the body from them):
- `hold-last`: the game's own. `hold-movement`: only the movement and stance keys carried, the
  actions dropped.
- `fade-to-stop`: the movement keys let go a fixed number of ticks past K0 (fitted: 10, past the
  horizon of nearly every sample: never).
- `hazard-table`: per motion class (5 directions x walk, sprint, aim, crouch) and hold age (22
  buckets), the number of ticks the movement is still held before it is let go, fitted to the
  least position error (the release hazard folded into the input it calls for; 440 bytes).
- `markov-1` / `markov-2`: per held state and hold age (and the state before), the state each
  predicted tick brings, the one whose velocity is nearest on average to what followed.
- `hold-last + damped-look`: the keys held; the look turns on at the last tick's turn x 0.6^s
  (yaw), 0.7^s (pitch), fitted over windows 1-6 and dampings 0-1 (delay-aware).
- `look-table[turn]`: the same with a damping per turn bucket (delay-aware, 14 bytes).
- `projected-look[turn]`, `[turn,elapsed]`: one projected input, what a client can run (section
  4); 14 and 140 bytes.
- `learned-ceiling`: LightGBM on 2 million (sample, tick) rows, 300 trees each for the yaw, the
  pitch and the movement keys (16 classes, the class chosen by least expected velocity error),
  features: 8 ticks of turns and inputs, the keys, the keys before, hold age, time since input,
  the body's velocity, combat. Delay-aware.

Position error at t, mean cm (all alive time):

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          1.42   0.32    1.31    3.06    8.99
hold-movement                      1.42   0.32    1.31    3.06    8.99
fade-to-stop                       1.42   0.32    1.31    3.06    8.99
hazard-table                       1.42   0.32    1.31    3.06    8.97
markov-1                           1.42   0.32    1.31    3.06    8.98
markov-2                           1.42   0.32    1.31    3.05    8.98
hold-last + damped-look            1.21   0.25    1.07    2.64    8.21
look-table[turn]                   1.20   0.24    1.06    2.61    8.12
projected-look[turn]               1.37   0.39    1.24    2.81    8.42
projected-look[turn,elapsed]       1.27   0.30    1.12    2.66    8.74
learned-ceiling (boosting)         1.22   0.25    1.08    2.64    8.21
```

Position error p99 cm (the snaps players see):

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          19.1    4.1    13.5    27.7    65.1
hold-movement                      19.1    4.1    13.5    27.7    65.1
fade-to-stop                       19.0    4.1    13.5    27.7    65.1
hazard-table                       19.1    4.1    13.5    27.7    64.9
markov-1                           19.1    4.1    13.5    27.7    65.1
markov-2                           19.0    4.1    13.5    27.7    65.1
hold-last + damped-look            16.3    3.1    10.9    23.4    58.7
look-table[turn]                   16.3    3.1    10.9    23.2    58.3
projected-look[turn]               17.6    5.0    12.2    24.2    59.5
projected-look[turn,elapsed]       16.8    3.7    11.1    23.5    62.2
learned-ceiling (boosting)         16.4    3.1    10.6    22.7    57.7
```

Look error at t, mean degrees (all alive time):

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          3.23   1.78    3.50    5.21    8.54
hold-movement                      3.23   1.78    3.50    5.21    8.54
fade-to-stop                       3.23   1.78    3.50    5.21    8.54
hazard-table                       3.23   1.78    3.50    5.21    8.54
markov-1                           3.23   1.78    3.50    5.21    8.54
markov-2                           3.23   1.78    3.50    5.21    8.54
hold-last + damped-look            2.55   1.20    2.75    4.37    7.58
look-table[turn]                   2.52   1.19    2.72    4.33    7.49
projected-look[turn]               2.67   1.31    2.88    4.53    7.79
projected-look[turn,elapsed]       2.61   1.27    2.77    4.43    8.23
learned-ceiling (boosting)         2.37   1.09    2.54    4.12    7.29
```

Combat: position error, mean cm:

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          1.47   0.32    1.32    3.17    9.54
hold-movement                      1.47   0.32    1.32    3.17    9.55
fade-to-stop                       1.47   0.32    1.32    3.17    9.55
hazard-table                       1.47   0.32    1.32    3.17    9.51
markov-1                           1.47   0.32    1.32    3.16    9.53
markov-2                           1.47   0.32    1.32    3.16    9.53
hold-last + damped-look            1.31   0.26    1.13    2.81    8.95
look-table[turn]                   1.31   0.26    1.12    2.79    8.88
projected-look[turn]               1.39   0.34    1.21    2.90    9.05
projected-look[turn,elapsed]       1.36   0.31    1.17    2.83    9.33
learned-ceiling (boosting)         1.31   0.27    1.13    2.80    8.85
```

Combat: look error, mean degrees:

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          3.16   1.76    3.43    5.13    8.28
hold-movement                      3.16   1.76    3.43    5.13    8.28
fade-to-stop                       3.16   1.76    3.43    5.13    8.28
hazard-table                       3.16   1.76    3.43    5.13    8.28
markov-1                           3.16   1.76    3.43    5.13    8.28
markov-2                           3.16   1.76    3.43    5.13    8.28
hold-last + damped-look            2.53   1.24    2.75    4.34    7.40
look-table[turn]                   2.52   1.23    2.73    4.32    7.31
projected-look[turn]               2.63   1.33    2.84    4.45    7.56
projected-look[turn,elapsed]       2.59   1.31    2.77    4.38    8.00
learned-ceiling (boosting)         2.38   1.13    2.54    4.09    7.06
```

PvP matches: position error, mean cm:

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          1.33   0.34    1.38    3.23    9.43
hold-movement                      1.33   0.34    1.38    3.23    9.43
fade-to-stop                       1.33   0.34    1.38    3.23    9.43
hazard-table                       1.33   0.34    1.38    3.23    9.41
markov-1                           1.33   0.34    1.38    3.23    9.42
markov-2                           1.33   0.34    1.38    3.23    9.42
hold-last + damped-look            1.12   0.26    1.14    2.80    8.63
look-table[turn]                   1.11   0.26    1.13    2.77    8.54
projected-look[turn]               1.30   0.42    1.32    2.98    8.84
projected-look[turn,elapsed]       1.18   0.33    1.20    2.83    9.16
learned-ceiling (boosting)         1.12   0.26    1.13    2.78    8.56
```

PvP matches: look error, mean degrees:

```
model                         realistic  50 ms  100 ms  150 ms  250 ms
----------------------------  ---------  -----  ------  ------  ------
hold-last                          3.29   1.85    3.63    5.40    8.83
hold-movement                      3.29   1.85    3.63    5.40    8.83
fade-to-stop                       3.29   1.85    3.63    5.40    8.83
hazard-table                       3.29   1.85    3.63    5.40    8.83
markov-1                           3.29   1.85    3.63    5.40    8.83
markov-2                           3.29   1.85    3.63    5.40    8.83
hold-last + damped-look            2.61   1.27    2.90    4.59    7.91
look-table[turn]                   2.59   1.26    2.87    4.55    7.81
projected-look[turn]               2.74   1.39    3.02    4.73    8.08
projected-look[turn,elapsed]       2.67   1.35    2.92    4.63    8.51
learned-ceiling (boosting)         2.39   1.13    2.64    4.28    7.54
```

Action error (share of predicted ticks with other action bits than the truth): hold-last 2.3 %
at the realistic delay (3.9 % at 250 ms); dropping the actions (`hold-movement`) 8.7 %. Held
actions (fire, aim) last longer than the delay, so carrying them is right; the edge-triggered ones
(use, bit 21; the quick uses) do not act again while held, so carrying them costs nothing.

Reading the tables:
- every movement model is hold-last to within 0.03 cm; at 250 ms the hazard table gains 0.02 cm;
- the look models gain 11-33 % of the look error (more at short delays), in combat and in PvP
  alike, and with it 9-22 % of the position error (a turning body's direction);
- `projected-look[turn]` costs 0.07 cm at 50 ms (it also turns ticks already known to have no
  turn) and gains from 100 ms on;
- `projected-look[turn,elapsed]` is better at the realistic delay but worse than `[turn]` at a
  fixed 250 ms: its elapsed buckets learn the realistic delay mix (a long elapsed time usually
  means the player stopped turning), which a high-ping receiver does not have.

---------------------------------------------------------------------------

## 7. Lookup tables, one feature at a time

The owner's idea (2026-11-05): a context key maps to the most likely next input, or to the chance
a held key is let go; the key grows one feature at a time; a rare cell (under 200 training
samples) falls back to the coarser key. Features: the movement keys held (16), the stance
(crouch, aim, sprint: 8), how long the held keys have lasted (9 buckets), the time since the
player's last known input (8), the body's speed (6), the look turn of the last tick (7). Sizes
are of the dense table: every cell of the key (a byte per cell and predicted tick for the move
tables, 13 ticks; 2 bytes per cell for the look tables).

"vs hold" is the gain over hold-last in position (move tables) or look (look tables); "step"
the gain of the feature added; "filled" the cells with 200 samples or more.

Move tables (context -> the held keys of each predicted tick; delay-aware):

```
key                                           pos cm  look deg  pos 250  look 250  vs hold    step   cells  filled    bytes
--------------------------------------------  ------  --------  -------  --------  -------  ------  ------  ------  -------
moves                                          1.975     3.234    11.63      8.54   -39.3%       -      16      12      208
moves,stance                                   1.422     3.234     8.99      8.54    -0.3%  +39.0%     128      48     1664
moves,stance,hold_age                          1.422     3.234     8.99      8.54    -0.3%   +0.0%    1024     196    13312
moves,stance,hold_age,since_input              1.422     3.234     8.99      8.54    -0.3%   -0.0%    7168     393    93184
moves,stance,hold_age,since_input,speed        1.422     3.234     8.99      8.54    -0.3%   -0.0%   43008     454   559104
moves,stance,hold_age,since_input,speed,turn   1.422     3.234     8.99      8.54    -0.3%   +0.0%  301056     529  3913728
```

Look tables (context -> damping per axis; delay-aware):

```
key                                           pos cm  look deg  pos 250  look 250  vs hold   step   cells  filled   bytes
--------------------------------------------  ------  --------  -------  --------  -------  -----  ------  ------  ------
turn                                           1.199     2.523     8.12      7.49   +22.0%      -       7       7      14
turn,since_input                               1.199     2.523     8.12      7.49   +22.0%  +0.0%      49      13      98
turn,since_input,moves                         1.197     2.518     8.09      7.45   +22.1%  +0.2%     784     108    1568
turn,since_input,moves,stance                  1.197     2.516     8.08      7.44   +22.2%  +0.0%    6272     271   12544
turn,since_input,moves,stance,hold_age         1.197     2.512     8.08      7.43   +22.3%  +0.1%   50176     659  100352
turn,since_input,moves,stance,hold_age,speed   1.197     2.511     8.08      7.43   +22.3%  +0.0%  301056     529  602112
```

Projected look (context -> gain per axis, one input after the newest node; what a client can
run):

```
key                        pos cm  look deg  pos 250  look 250  vs hold   step  cells  filled  bytes
-------------------------  ------  --------  -------  --------  -------  -----  -----  ------  -----
turn                        1.373     2.674     8.42      7.79   +17.3%      -      7       7     14
turn,elapsed                1.271     2.606     8.74      8.23   +19.4%  +2.1%     70      67    140
turn,elapsed,stance         1.271     2.603     8.74      8.22   +19.5%  +0.1%    560     231   1120
turn,elapsed,stance,moves   1.271     2.598     8.74      8.22   +19.7%  +0.2%   8960     457  17920
```

- The move tables stop paying at once: the movement keys alone are worse than holding (they
  forget the stance); with the stance they are hold-last, and no further feature finds a cell
  where letting go or switching beats holding.
- The look tables pay with the turn alone (7 cells); every further feature adds 0.0-0.2 %.
- The projected look gains 2 % more with the time since the input (140 bytes) and nothing after.

---------------------------------------------------------------------------

## 8. A per-map flow table

Per map (80 maps in the training matches), 4 m cells of the reconstructed positions (each tick's
position carried on from the last world by the proxy, at most 2 s) hold the mean direction of
travel of the players moving there (cells with 20 samples or more; strength = the length of the
mean unit vector). A moving player whose cell's flow is strong (>= 0.6) gets an extra turn toward
it, gain x (flow yaw - heading) per tick, the gain fitted per tick by least squares on training.

```
 model                              pos cm   look deg   (realistic delay)
 hold-last + damped-look             1.21     2.548
 hold-last + damped-look + flow      1.21     2.556
 look-table[turn]                    1.20     2.523
 look-table[turn] + flow             1.20     2.531
```

The fitted gain is under 0.008 a tick: over 100-250 ms the flow says nothing a player's own last
turn does not, and it makes the look slightly worse. Its size would be about 2.7 KB a map (median,
2 bytes a cell over the map's box; 200 KB for all 80) and ship per map as content. **Not
recommended.**

---------------------------------------------------------------------------

## 9. The choice

**Chosen: hold-last for the keys and the mask, plus `projected-look[turn]`**, the simplest model
a client can run that is within 10 % of the best one it can run (look 2.67 against 2.60 degrees
for the 17,920-byte table; position 1.37 against 1.27 cm), and better than hold-last at every
delay from 100 ms on, in combat and in PvP. It is 7 cells x 2 bytes: **14 bytes**. It is robust to
the receiver's delay (no bucket of it learns the delay mix), where `[turn,elapsed]` is worse at
250 ms.

Stated plainly: **nothing beats hold-last by a margin that matters.** At typical delays the gain
in position is 0.5 mm on average (1.42 -> 1.37 cm) and 1.5 cm at p99; the look is 0.56 degrees
better on average. If the projection is built anyway (for direct peers or smoothing), the table is
nearly free to add; it is not worth building for itself.

The table: `prediction_fit_table.tsv` (regenerate with `python -m predfit.export results.json
'projected-look[turn]'`):

```
 turn = |yaw turn| + |pitch turn| of the newest node, radians
 turn from   to        yaw gain   pitch gain   (/ 64)
 0           0.0001    0          0
 0.0001      0.003     0          0
 0.003       0.008     32         51
 0.008       0.02      48         61
 0.02        0.05      54         54
 0.05        0.12      58         54
 0.12        -         48         48
```

A gain of 54/64 = 0.84: the projected input repeats 84 % of the node's turn once. Turns under
0.003 rad a tick (mouse jitter) are not carried on.

---------------------------------------------------------------------------

## 10. What the in-game side needs

What the projection (PR #1279, the Rust client transition) needs for the chosen model, per remote
place, each frame before the render move:

1. The place's newest received input node: its tick tau (grid ms), its 8 bytes (yaw and pitch
   turns as halves, the u32 mask), and that the place is alive. Nothing else: no delay estimate,
   no hold durations, no history.
2. If the node is at the tick under way or later (now < tau + 50), or 24 ticks (1.2 s) old or
   more, no projection (the table was fitted so: an older node's turn counts as none).
3. Else: `turn = |yaw| + |pitch|` of the node (radians, from the halves); the row of the table
   whose `[from, to)` holds it gives `yaw_gain`, `pitch_gain` (bytes, / 64).
4. Project one input at tau + 50 ms: the node's own mask **unchanged** (so no key edge is made:
   use, bit 21, and the quick uses act on edges; spot, bit 28, has its cooldown), yaw turn =
   node yaw x yaw_gain / 64, pitch turn = node pitch x pitch_gain / 64, written as halves. Both
   gains 0: no projected input at all (the world as hold-last has it).
5. Take it out before every confirmed move, as every projection.

Costs: one table read and two multiplies per remote place per frame; 14 bytes of table
(`[[u8; 2]; 7]`) and 6 bucket edges. In fixed point (Q16 radians: the halves read once into it)
the edges are 7, 197, 524, 1311, 3277, 7864; the projected turn is `turn * gain >> 6`, written
back as a half. No allocation, no float needed.

Optional, if the client keeps the time since the node anyway: `projected-look[turn,elapsed]` (70
cells, 140 bytes) is 2 % better at typical delays but worse for a receiver whose delay is far
above the median; not recommended unless its elapsed buckets are fitted per receiver delay.

---------------------------------------------------------------------------

## 11. Limits

- The position errors are of the proxy (no walls, slopes, jumps, kit speed level); its own error
  is under 3 cm over 50 ms (section 5). Walls would make real errors smaller.
- 73 % of the matches are one human against bots: those players' delays use their own round trip
  for the downlink. The PvP rows (2+ humans) agree with the totals.
- The delay-aware models are told more than a client knows (that nothing changed up to t - d);
  their scores are upper bounds, and they still do not beat holding the keys.
- Receivers are taken to run the server's clock; an offset of a client's clock would add to d.
- Positions for the flow table are carried on from worlds without the blend (good to a few
  decimetres for 4 m cells).
- Three days of matches (revisions 42-48); the recordings are deleted oldest first over the
  size cap, so a refit later sees other matches.
