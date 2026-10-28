# Bench fidelity (fx-bench, 2026-10-29)

What the real-map bench (`bots/tests/bench.rs`, the `real_bot_cost` test) now models after the 2026-10-28 analyses
(`pvb-343-check/traces-20261028/CROSS-CHECK.md` §3), the options that switch each part on, the counters that read the
outcomes, and the changes the private bench scripts need (section 4: for the coordinator, not applied).

## 1. What changed in the stand-in

| problem (CROSS-CHECK §3) | now | switch |
|---|---|---|
| 1. Phantom grenades: the stand-in blasted every logged throw, ~80 % of them with empty pockets | The stand-in blasts only a throw the thrower's kit still had a grenade for that life (`loadouts::grenades_in` of the kit in hand, one more death is a new life). The throws it refuses are counted (`grenade_carry.phantom_throws`); "grenades" still counts every logged throw. | always on (it is the game's rule) |
| 4. The stand-in camper stood within 0.7 m of one point all match | Works an area, peeks, steps off after kills and hits, leaves: `tests/camper_move.rs`, numbers from an-anticamper §8.4. | `SRP_BENCH_CAMPER_MOVE=1` (`_DWELL_S`) |
| 4. No camper by an objective, none at a spawn exit that cannot be flanked | The spot is chosen near a lying battery, his own container, or the other team's spawn (the one seen from in the narrowest sector). | `SRP_BENCH_CAMPER_AT=battery\|container\|exit` |
| 4. The camper tally cut a real camper's hold at 8 m | The stand-in camper's holds are scored within 10 m (`CAMPER_HOLD_M`); other shooters' stay at 8 m. | always |
| 3. No reckoning error: the bots' picture of a body was exact | Every world fix is moved by an error drawn from an-sight §8.1's tables by what the body was doing and the fix's age. | `SRP_BENCH_VIEW_ERROR=1` (`_SCALE`) |
| 5. Both teams place traps (`SRP_BENCH_ENEMY_HUMAN_LANES` gave both teams human enemies) | One team: in Player vs Bots the human is on team 0, so team 1's bots meet a human and team 0's meet bots. | `SRP_BENCH_ENEMY_HUMAN_LANES=team1` (`team0`; `on` is both, as before) |
| 7. Counters that mislead | "held-line entries fatal" is also given as a share (`reentries_fatal_share`). | always |

## 2. The new counters (`tallies/owner/outcomes.rs`, part `outcomes`; keys of the `owner` object of a log's `sums:` line)

| key | report | live / owner numbers it is read against |
|---|---|---|
| `hold_runs_2`, `hold_runs_3`, `hold_3rd_share` | an-anticamper §2.5: of the holds that made two kills, the share that made a third (a hold: kills within 10 m and 120 s) | live humans 25 %, the owner against bots 45 % |
| `holders_killed_after_2`, `holders_killed_flank`, `holders_flank_share` | the holder killed after two kills by a killer more than 60 degrees off every victim's bearing | 32 % / 9 % |
| `kills`, `quick_second_kills`, `quick_second_share` | kills within 3 s of the same shooter's last | live 3.5-3.9 %, the owner 18 % |
| `lane_entries_per_10s` | entries into held lines (the camper topic's) per 10 s of held time | live 0.24-0.60, bots against the owner 0.84-1.42 |
| `reentries_fatal_share` (in `campers`) | held-line entries fatal, as a share of the entries | 13.0 % OFF, 14.5 % ON in the 2026-10-27 smoke |
| `carrier_clear_s`, `carrier_clear_not_targeted_s` | an-fights §5a: bot-seconds a carrier is in an enemy's clear sight within 40 m, and is not that enemy's target | the owner's steal: 14 clear bot-seconds in 22 s, half on his escorts |
| `hurt_enemies`, `hurt_alive_15s`, `hurt_alive_15s_share` | an-fights §5b: bodies hurt to pain under 30 % with a bot of the other team within 30 m, alive 15 s later | |
| `retake_ms_hurt_near`, `retake_ms_no_hurt_near` | an-fights §5c: `[mean ms, n]` for a dropped battery to be taken or put in again, by whether a hurt enemy was within 10 m of the drop | |
| `zone_takes`, `zone_take_died_10s` | an-objective §2.5: battery pickups from a marked zone floor; the carrier dead within 10 s | |
| `escort_no_way_ends`, `collect_no_way_ends` | an-objective: `ends Escort/Collect: ... no way ...` notes | Escort 18 and 54 in the owner's two zone matches |
| `trap_placed_by_stance`, `trap_caught_by_stance` | an-gear-moves §3.2: `[standing, crouched]` placed; caught `[enemy, mate, own]` by the placer's stance (his stance at his last placement) | own 217 of 298 catches |
| `view_flip.missed_sight_share`, `view_flip.fire_into_cover_share` (+ the counts) | an-sight §8.1: of the moments a body was truly visible, the share the bots' picture says hidden; of the moments the picture says visible, the share truly hidden (a pair a second, each bot's 2 nearest enemies within 80 m) | 22 % / 20 % |
| `grenade_carry.carried_throws`, `.phantom_throws`, `.phantom_share` | an-gear-moves §3.1 | |

The line `owner-test outcomes: ...` of a log prints them with the numbers they are read against. Another package's own counters
are its own topic file (`tallies/owner/<package>.rs`, a golden of its own, one line in `topics()` and one `mod`/`use`).

## 3. Options (all in `tests/settings.rs`)

`SRP_BENCH_CAMPER_MOVE=1`, `SRP_BENCH_CAMPER_DWELL_S`, `SRP_BENCH_CAMPER_AT`, `SRP_BENCH_VIEW_ERROR=1`,
`SRP_BENCH_VIEW_ERROR_SCALE`, `SRP_BENCH_ENEMY_HUMAN_LANES=on|both|team0|team1`. The moving camper's numbers:

| | the owner / live (an-anticamper §8.4) | the stand-in |
|---|---|---|
| area | half the time within 3 m of the hold's centre, 9/10 within 8 m | the spot and up to 3 route points of his side 3-8 m from it, those seen from the fewest points of the other side first |
| peeks | out 0.45 s / 0.40 s, every 1.2 s / 0.8 s | out 0.40-0.45 s every 0.8-1.2 s while an enemy is known within 60 m (a sideways step of 1.2 m) |
| stance | crouched 0 % / 13 % | standing, a crouch spell of 1-2 s every 10-16 s |
| after a kill / a hit | moved 2.5 / 3.6 m; 2.1 / 2.6 m | to another area point 2.5-3.5 m away; 2-2.5 m |
| leaving | 5 s / 3.5 s after the last kill; hold median 25 s | 3-5 s after a stay's 2nd kill with no new one; 20-25 s with fewer; to the next best spot 25 m or more away |

The view error: a fix's place moves by a draw from the quantile table of the body's state (still, walking, sprinting,
crouched, height change, jump) and the fix's age, in a direction drawn evenly, carried by the reckoning to the next fix.

### Checks of this PR (WSL `srp-split`, `/mnt/out/split/fx-bench-20261029/`, `SRP_BENCH_ENEMY_HUMAN_LANES=team1`)

- **Phantom grenades (1):** 9 matches (3 maps x seeds 4-6): 36 logged throws, 6 carried, 30 phantom (the old stand-in blasted all 36).
- **View error (3):** base (ping only) 9.3 % missed sight / 8.8 % fire into cover; with the error at scale 0.7 on seeds 4-6, which
  were not used to set it: **21.9 % / 20.8 %** against the live 22 % / 20 % (scale 1.0 gave 24.6 / 23.6, 0.75 23.0 / 21.7, 0.6 21.5 / 19.0
  on seeds 1-3). `view_flip.samples` reads 1620 in both arms (not 0).
- **Moving camper (2):** 6 matches (School and Vector Laboratory, seeds 1-3), hidden spot, long gun:
  `still` / `still_crouch` / `move` -> `holders_killed_flank` 0 / 2 / 5 of `holders_killed_after_2` 18 / 29 / 26, `counter_runs_flank`
  1 / 0 / 0 of 38 / 38 / 33 counter runs, `camper_runs_3` 6 / 1 / 6. The old stand-in did **not** over-credit Flank here (the
  `counter_runs_flank` of 4.3 is about zero in all three arms): what the moving camper changes is who dies to a flank, not the
  counters' count. Six matches are too few to say more; this PR only makes the case runnable.
- **Scenarios (2):** `battery` / `container` / `exit`, 6 matches each (batteries on School, Cologne, Vector Laboratory, Rudnya, seeds 1-2): the
  `camper:` line has a spot in 17 of 18 logs (one `None`: no route point near the anchor), `camper_kills` 58 / 65 / 62.
- The new counters read as expected on every log (`outcomes` part present, the golden pins its shape). The activity counters above
  (`grenade_carry`, `view_flip.samples`, `hold_runs_2`, `hurt_enemies`, `carrier_clear_s`) are non-zero in these runs; `zone_takes` is
  4 and 2 in the 9 matches.

## 4. Changes the private bench scripts need (for the coordinator; not applied)

Everything here is `private/pvb-343-check/bench/...`; this PR touches none of it.

### 4.1 A138: the OFF arm on `all_off.json`, the ON arm the defaults (`phase-end-smoke/run-smoke.sh`)

After #701 `Tuning::default()` is the owner's ON set. The A128 session's note (`a128-bench-arms.md`) has the mechanics; the
edits, in `run_one` and `run`:

1. **OFF arm.** In the `[ "$arm" = off ]` case, set
   `SRP_BENCH_TUNING="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1])),separators=(",",":")))' "$SRC/servers/match-server/src/v026g/bots/tuning/all_off.json")"`
   and nothing else. Read the file from `$SRC`, the tree of the sha the binary was built from, never from a copy: the file
   changes with each PR that adds a switch (the unit test in `tuning/` keeps it complete). `null` in it means "none" for the `Option`
   fields: **never pass it through `merge_json.py`** (RFC 7386 deletes the key, which is the default, which is ON).
2. **ON arm.** `SRP_BENCH_TUNING` unset (or `{}`); `ALLON_EXTRA` stays a patch over the defaults. Drop `on_tuning()` and
   `ledger-work/allon*.json` from the run (they are no-ops over the defaults; keep the files as the record). Keep the ON env
   (`SRP_BOT_KIT=1 SRP_BOT_UPGRADES=1 SRP_BENCH_RUSHER=... SRP_BENCH_MASKS=top`): those are not tuning.
3. **Drop-one arms.** `python a128-convert-dropone.py drop-one a128-all_off.json <new dir>` writes the files that say
   "X's OFF value"; a drop arm's `SRP_BENCH_TUNING` is that file alone, not merged into the ON patch.
4. **`SRP_BENCH_*` aliases only turn things on** (`SRP_BENCH_SIGHT_REFRESH=1`, `_LIP_GUARD`, `_FLOOR_PATHS=1`, ...): over the defaults they
   are no-ops and a `=0` turns nothing off. Do not build the OFF arm from them; the patch is applied after them and wins.
5. **Check before a run:** `DRY=1 ... run` shows each arm's `SRP_BENCH_TUNING`: the OFF arm's is the `all_off.json` line, the ON arm's unset.
   The log's switch report is against `Tuning::all_off()`: the OFF log must say `changed numbers: none` and the ON log the long list.
6. **Lanes.** `SRP_BENCH_ENEMY_HUMAN_LANES=on` becomes `team1` in both arms (this PR): in Player vs Bots the human is on team 0, so
   only team 1's bots take their enemies for humans; `on` counted both teams and placed traps twice a match (an-gear-moves 3.3).
7. **View error.** Add `SRP_BENCH_VIEW_ERROR=1` to the common environment of both arms (it is a model of the game, like the heal
   and stamina models, not a feature: both arms get it) once the owner agrees to the calibration of section 3. Until then leave
   it out; the audit (4.5) lists it as a scenario setting.

### 4.2 Vector Laboratory in the batteries stage (`presets/batt.env`)

`SRP_BENCH_MAPS=6/2,1/2,3/3,2/2,4/2,5/2` runs levels 1-6 only. Add `,7/2` (Vector Laboratory batteries; the smoke and offered stages
have it, the batteries stage, which is read for objectives, does not; its middle zone and spawner 26 are an-objective's case). Cologne
(`8/2`) is also missing. `3/3` (timed batteries, Vostok) is not offered in Player vs Bots (`pvb.rs`), so its numbers never meet a player: say
so in the sheet or replace it by `3/2`. Cost: one map more of seven in the stage (+17 % per seed).

### 4.3 Camper stages

A stage `camper` (preset `presets/camper.env`: source `batt.env`, then):
```
export SRP_BENCH_CAMPER=1 SRP_BENCH_CAMPER_SPOT=hidden SRP_BENCH_CAMPER_GUN=long SRP_BENCH_CAMPER_MOVE=1
```
and cells for the three scenarios, each a value of `SRP_BENCH_CAMPER_AT` (one cell per value, or arms of the stage):

| scenario | `SRP_BENCH_CAMPER_AT` | maps | what is read |
|---|---|---|---|
| a camper by a lying battery | `battery` | `2/2,8/2,7/2,1/2` | the `camper:` line has a spot (none: no route point near a battery), the `stand_in_camper` object, Collects into his lane, deaths by the battery |
| a camper by his container | `container` | the same | the same, the raiders' way to a container (an-anticamper RC3) |
| a camper at a spawn exit that cannot be flanked | `exit` | the same | what the other team does when every sight on him comes from one way: `holders_flank_share`, `lane_entries_per_10s`, `hold_3rd_share`, the `threat` line and `Flank` / `Crossfire` takes on him |

Arms: the stage's ON and OFF as in 4.1, plus (once) `still` / `still_crouch` against `move` to see what a moving camper changes
(measured 2026-10-29: `counter_runs_flank` is about zero in all three arms, so read `holders_killed_flank` and `camper_runs_3` instead, see "Checks of this PR"). 10 a side, tier 13-15, seeds 1-4, 180 s. The camper tally already scores his holds within 10 m.

### 4.4 The new counters in the reports

`smoke-compare.py` `METRICS` (name: reader, worse direction; `+1` higher is worse, `-1` lower, `0` either; `_owner(key)` reads the `owner` object):

| name | reader | worse |
|---|---|---|
| `hold 3rd-kill share` | `_owner("hold_3rd_share")` | +1 (live 25 %) |
| `holders killed by a flank share` | `_owner("holders_flank_share")` | -1 (live 32 %) |
| `quick second kill share` | `_owner("quick_second_share")` | +1 (live 3.5-3.9 %) |
| `lane entries / 10 s` | `_owner("lane_entries_per_10s")` | +1 (live 0.24-0.60) |
| `held-line entries fatal share` | `_owner("reentries_fatal_share")` | +1 (read this, not the count `reentries_fatal` alone) |
| `carrier clear, not targeted s` | `_owner("carrier_clear_not_targeted_s")` | +1 |
| `hurt alive 15 s share` | `_owner("hurt_alive_15s_share")` | +1 |
| `battery retake ms (hurt near)` | `_owner("retake_ms_hurt_near.0")` | +1 |
| `zone take deaths` | `_owner("zone_take_died_10s")` | +1 |
| `escort no-way ends` | `_owner("escort_no_way_ends")` | +1 (A129: 18 and 54) |
| `own trap catches, crouched placer` | `_owner("trap_caught_by_stance.1.2")` | +1 (an-gear-moves 3.2: 217 of 298) |
| `missed sight share` | `_owner("view_flip.missed_sight_share")` | 0 (a model check: live 22 %) |
| `fire into cover share` | `_owner("view_flip.fire_into_cover_share")` | 0 (live 20 %) |
| `phantom throws` | `_owner("grenade_carry.phantom_throws")` | 0 (volume the bench no longer blasts) |

`_owner` reads a list item by index (`retake_ms_hurt_near.0` is the mean ms, `.1` the number; `trap_caught_by_stance.1.2` the crouched row's own catches). Shares are per match, so the paired z of a share is the share's, not the counts'. Retire as "worse is lower" the lines of
an-fights §5b that read "died unturned" and "turned on shooter" as failures (they are one event and higher is more human), and
"drops >= 4 m" (planned drops): keep them as information.

`features-report.py`: one block per fix, the activity counter first. The flag the owner asked for (2026-10-28) is that **every
report flags a counter that reads 0 in the ON arm where its switch should fire** (`ZERO in ON`, which the report prints for the old
blocks; each new block must name an activity counter or print `no dedicated counter`). For this PR's own options:
`grenade_carry.carried_throws + phantom_throws` reads 0 when no bot threw (then `grenades` also reads 0: a finding), and
`view_flip.samples` reads 0 only when no bot was in the match.

### 4.5 `switch-audit.py` after #701

Its premise (every boolean false by default must be in the ON set or `KEPT_OFF`) is now: **every boolean false in the new defaults
must be in `KEPT_OFF`**. Change `on = {}` (drop the `allon` / `pending` reads), keep `KEPT_OFF` (it matches the 9 switches the defaults
leave off; the golden of the switch report pins them), and for the "key the tip does not know" check read the keys of
`all_off.json` against the defaults dump (`Tuning::default()` JSON at the tip). For the model audit: `MODEL_SCENARIO` gets
`SRP_BENCH_CAMPER_MOVE`, `SRP_BENCH_CAMPER_AT` and `SRP_BENCH_CAMPER_DWELL_S` (scenarios). `SRP_BENCH_VIEW_ERROR` and `_SCALE` are
**model** settings: export them from `run-smoke.sh` (4.1 item 7) or list them as `MODEL_SCENARIO: 'off until the owner agrees'`, or the
audit fails on them. `SRP_BENCH_ENEMY_HUMAN_LANES=team1` is exported by the smoke as before (the audit's `n + '='` finds it).

### 4.6 Head-to-head arms (owner rule 2026-10-29) and the by-team counters

Every counter this PR adds is also in `owner.by_team.{"0","1"}` of a log's `sums:` line (`outcomes` part: all of its keys but
`lane_entries_per_10s`, which is the camper topic's; `campers`: `by_team.{0,1}.reentries / reentries_fatal / reentries_fatal_share`,
by the entering bot's team; `grenade_carry.by_team`). Shares are per team (`hold_3rd_share`, `holders_flank_share` of a team's
shooters / holders), `retake_ms_*` by the team that dropped the battery, `carrier_clear_*` by the carrier's team, `view_flip` by the
looking bot's team. The totals are unchanged. The stand-in and the tallies read **no** tuning: the camper, the view error and the
lanes are `SRP_BENCH_*` model settings that are the same for both teams, so nothing here needs `tuning_of(team)`.

`run-smoke.sh`: `ARMS=h2h` runs two runs of the same binary per cell, `SRP_BENCH_ONE_SIDE=1`:

| run | team 0 | team 1 |
|---|---|---|
| A | ON (`SRP_BENCH_TUNING` unset) | OFF (`SRP_BENCH_SIDE1_TUNING` = the `all_off.json` line of 4.1 item 1) |
| B | OFF (`SRP_BENCH_TUNING` = that line) | ON (`SRP_BENCH_SIDE1_TUNING` unset) |

(A drop-one arm is the same with the drop file in place of `all_off.json`.) The by-side report sums ON and OFF over both runs, so
a side advantage of the map cancels: in run A the ON numbers are `by_team["0"]`, in run B `by_team["1"]`:

```python
import json, re, sys, glob
from collections import defaultdict
# by-side.py DIR_A DIR_B KEY...   (KEY: an outcomes key, e.g. holders_flank_share is recomputed from its counts)
def sums(path):
    text = re.sub(r'test [^ ]+ has been running for over [0-9]+ seconds[^a-z]?', '', open(path, encoding='utf-8', errors='replace').read())
    for m in re.finditer(r'sums: (\{.*\})', text):
        yield json.loads(m.group(1)).get('owner', {})
on, off = defaultdict(float), defaultdict(float)
for run, (dirname, on_team) in enumerate(((sys.argv[1], '0'), (sys.argv[2], '1'))):
    for path in glob.glob(dirname + '/**/*.log', recursive=True):
        for owner in sums(path):
            for key in sys.argv[3:]:
                for team, acc in (('0', on if on_team == '0' else off), ('1', on if on_team == '1' else off)):
                    value = owner.get('by_team', {}).get(team, {}).get(key)
                    if isinstance(value, (int, float)):
                        acc[key] += value
for key in sys.argv[3:]:
    print(f'{key:32} ON {on[key]:10.1f}  OFF {off[key]:10.1f}')
```

Counts (not shares) are summed; a share is recomputed from its counts (`holders_killed_flank / holders_killed_after_2`). A counter that
reads 0 on both sides is flagged as in 4.4.
