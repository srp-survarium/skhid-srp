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
RESULTS_PLACEHOLDER

## 4. Changes the private bench scripts need (for the coordinator; not applied)

BENCH_SCRIPTS_PLACEHOLDER
