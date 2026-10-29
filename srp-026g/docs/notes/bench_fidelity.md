# Bench fidelity (fx-bench, 2026-10-29)

What the real-map bench (`bots/tests/bench.rs`, the `real_bot_cost` test) now models after the 2026-10-28 analyses
(`pvb-343-check/traces-20261028/CROSS-CHECK.md` §3), the options that switch each part on, the counters that read the
outcomes, and the changes the private bench scripts need (section 4: for the coordinator, not applied).

## 1. What changed in the stand-in

| problem (CROSS-CHECK §3) | now | switch |
|---|---|---|
| 1. Phantom grenades: the stand-in blasted every logged throw, ~80 % of them with empty pockets | Each stand-in body keeps its frag grenades as the client does (`standin_grenade::Stock`, PROVEN `loadouts.rs`: a spawn fills the hand with `min(pocket units, the item's store)` from the kit's store, what is left goes back at the next spawn). A bot's store is its count today, so a grenade thrown is gone for the match; A144's store of 100+ gives the count every life with no change here. A throw with nothing in the hand blasts nowhere and is counted (`grenade_carry.phantom_throws`); "grenades" still counts every logged throw. The rule moves to `KitPockets::of_profile` when fx-gear's store PR lands. The bots' own list of live grenades (`tracker.launch_grenades`) still lists a phantom throw, so a mate may dodge a blast the stand-in no longer makes, until that reader exists (`set_pockets` for bot places would refill per life, the bug this fixes). | always on (it is the game's rule) |
| 4. The stand-in camper stood within 0.7 m of one point all match | Works an area, peeks, steps off after kills and hits, leaves to the next spot of his scenario and spot rule, and after a death goes back to the same spot 13-20 % of the time (median 34 s) and takes another otherwise: `tests/camper_move.rs`, numbers from an-anticamper §8.4. He plays by a fixed tuning (the build's defaults), the same opponent in both runs of a head-to-head. | `SRP_BENCH_CAMPER_MOVE=1` (`_DWELL_S`) |
| 4. No camper by an objective, none at a spawn exit that cannot be flanked | The spot is chosen near a lying battery, his own container, or the other team's spawn (the one seen from in the narrowest sector). | `SRP_BENCH_CAMPER_AT=battery\|container\|exit` |
| 4. The camper tally cut a real camper's hold at 8 m | The stand-in camper's holds are scored within 10 m (`CAMPER_HOLD_M`); other shooters' stay at 8 m (the camper topic's K1 definition; the outcomes' holds are §2.5's 10 m and 60 degrees, a different measure). | always |
| 3. No reckoning error: the bots' picture of a body was exact | Every world fix is moved by an error drawn from an-sight §8.1's tables by what the body was doing and the fix's age. | `SRP_BENCH_VIEW_ERROR=1` (`_SCALE`) |
| 5. Both teams place traps (`SRP_BENCH_ENEMY_HUMAN_LANES` gave both teams human enemies) | One team: in Player vs Bots the human is on team 0, so team 1's bots meet a human and team 0's meet bots. | `SRP_BENCH_ENEMY_HUMAN_LANES=team1` (`team0`; `on` is both, as before) |
| 7. Counters that mislead | "held-line entries fatal" is also given as a share (`reentries_fatal_share`). | always |

## 2. The new counters (`tallies/owner/outcomes.rs`, part `outcomes`; keys of the `owner` object of a log's `sums:` line)

| key | report | live / owner numbers it is read against |
|---|---|---|
| `hold_runs_2`, `hold_runs_3`, `hold_3rd_share` | an-anticamper §2.5: of the holds that made two kills, the share that made a third (a hold: kills within 10 m and 120 s) | live humans 25 %, the owner against bots 45 % |
| `holders_killed_after_2`, `holders_killed_flank`, `holders_flank_share` | the holder killed after two kills by a killer more than 60 degrees off every victim's bearing | 32 % / 9 % |
| `kills`, `quick_second_kills`, `quick_second_share` | kills within 3 s of the same shooter's last | live 3.5-3.9 %, the owner 18 % |
| `lane_entries_per_10s` | entries into the killed lanes (the camper topic's cone of 15 degrees) per 10 s of holds of two kills or more (the report's cone is 20 degrees) | live 0.24-0.60, bots against the owner 0.84-1.42 |
| `reentries_fatal_share` (in `campers`) | held-line entries fatal, as a share of the entries | 13.0 % OFF, 14.5 % ON in the 2026-10-27 smoke |
| `carrier_clear_s`, `carrier_clear_not_targeted_s` | an-fights §5a: bot-seconds a carrier is in an enemy's clear sight within 40 m, and is not that enemy's target | the owner's steal: 14 clear bot-seconds in 22 s, half on his escorts |
| `hurt_enemies`, `hurt_alive_15s`, `hurt_alive_15s_share` | an-fights §5b: bodies hurt to pain under 30 % with a bot of the other team within 30 m, alive 15 s later | |
| `retake_ms_hurt_near`, `retake_ms_no_hurt_near` | an-fights §5c: `[mean ms, n]` for a dropped battery to be taken or put in again, by whether a hurt enemy was within 10 m of the drop | |
| `zone_takes`, `zone_take_died_10s` | an-objective §2.5: battery pickups from a marked zone floor; the carrier dead within 10 s | |
| `escort_no_way_ends`, `collect_no_way_ends` | an-objective: `ends Escort/Collect: ... no way ...` notes | Escort 18 and 54 in the owner's two zone matches |
| `trap_placed_by_stance`, `trap_caught_by_stance` | an-gear-moves §3.2: `[standing, crouched]` placed; caught `[enemy, mate, own]` by the placer's stance (his stance at his last placement) | own 217 of 298 catches |
| `view_flip.missed_sight_share`, `view_flip.fire_into_cover_share` (+ the counts, `pairs` and `samples`) | an-sight §8.1: of the moments a body was truly visible, the share the bots' picture says hidden; of the moments the picture says visible, the share truly hidden (a pair a second, each bot's 2 nearest enemies within 80 m) | 22 % / 20 % |
| `grenade_carry.carried_throws`, `.phantom_throws`, `.phantom_share` | an-gear-moves §3.1 | |

Every key is also in `by_team.{"0","1"}`, filed under the team whose decision it measures (the table at the top of `tallies/owner/outcomes.rs`:
holds and quick second kills under the team the shooter fought, holders killed under the killer's team, the carrier's sight under the enemy's,
hurt enemies under the near bot's team, a retake under the team that took the battery, the rest under the acting team).
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

## 4. Changes the private bench scripts need

The write-up (the A138 OFF arm on `all_off.json`, Vector Laboratory in the batteries stage, the camper stages, the new counters in `smoke-compare.py` and `features-report.py`, `switch-audit.py` after #701, the head-to-head arms `ARMS=h2h` with the by-side script) edits private files, so it lives with the shared notes: `private/pvb-343-check/traces-20261028/fx-bench-private-scripts.md`. This PR touches none of those files.

Two facts about the model that a reader of that file needs: the trap counters exist for team 1 only under `SRP_BENCH_ENEMY_HUMAN_LANES=team1`, so a trap fix's by-side table compares team 1 in run A with team 1 in run B; and the stand-in camper plays by a fixed tuning (the build's defaults), the same opponent in both runs.

## 5. Bench metrics (A148): what a bench run now reports for a decision

`BENCH-DECISIONS.md` (shared notes) is the rule; this is what the bench prints for it.

- **The scoreboard** (`tallies/owner/scoreboard.rs`, golden `golden/scoreboard.txt`): one line per player at the match's end,
  `scoreboard: <project> NvN seed S | place P team T kind bot|camper|human | kills K deaths D assists A streak B | events {id: count} |
  objective {delivered, carrier_kill} | score X`, and the same objects as `sums.owner.scoreboard`. The score is the client's own sum
  (`snapshot::score` over `static_info::score_weights(mode)`; the table is not copied); events are the game's ids: 0 kill, 1 assist,
  2 death, 6 brought (100 hundredths: the stand-in's carrier delivers alone), 20 put in, 10 enemy carrier killed. Headshot (15), steal
  (17) and found (19) are not counted (no part in the stand-in / not in its log as counts). **Assists are a GUESS** (a non-killer teammate who hit the
  victim within 10 s); the line `owner-test scoreboard:` prints their share of the kills next to the live 0.19 a kill, and the
  window is the thing to move. The owner's bar (BENCH-DECISIONS §5) is read from these lines: the rank an average player's live score
  per minute would take in each team.
- **One hold definition.** The camper topic and the outcomes use the same hold: kills from one place within 10 m of the first, 120 s
  apart, a killer from a flank at 60 degrees or more off every victim's bearing (an-anticamper §2.5: the live holds and the owner's the
  report counted). The camper topic's old 8 m / 45 degrees (K1) and the stand-in camper's special 10 m are gone; every decision
  compares arms of one binary, so the old benches' numbers are not comparable and need not be. The camper topic still counts a holder
  killed after one kill, the outcomes after two.
- **The decision metrics** (`tallies/owner/metrics.rs`, goldens `fairness`, `reaction`, `trades`, `damage`, `work`), all by team:
  - *enemy position reads* (`bots/fairness.rs`, a helper a package calls where a decision reads an enemy's position from a `Track`;
    wired in `Ctx::known_place`, `enemies_near` and `threats`): fresh (own senses within 15 s), stale, hud, cheat. This counts
    **reads, not choices**: a play's scoring reads many positions. New code should add no stale, hud or cheat read; a package that
    adds its own read site calls `fairness::read(team, Site, &track, now)`. An empty function in a live build.
  - *reaction*: ms from the first tick a bot has a live sight of an enemy (none in the second before) to its first fire key down;
    n, median and p10 by the shooter's team, and the engagements that ended with no shot.
  - *trades*: a death is traded when a teammate kills the killer within 5 s; deaths and traded by the dead's team. Team-wide; the
    Trade play's own count is one play's.
  - *damage*: health lost per tick by the team that lost it (taken), shared among the enemy hits on the body in that tick for the
    team of the shooters (dealt); the rest is `other` (grenade, fall, zone, bleeding, a mate). In lives (1.0 = a body's full health).
  - *work* (`bots/bench_work.rs`, test builds only): floors expanded, rays cast, spot queries, plays scored, a match-second, by the
    team of the bot whose think or tick did them; `shared` is the commanders and the shared search slices. No time is read, so two
    arms compare on any machine. The bench's own `timed_poll` copy of `Bots::poll` sets the same marks.
- **The feel counters** (`tallies/owner/feel.rs`, golden `feel`): the live census's definitions (`an-top-player.md` §5, `an-gang-up.md`),
  for each team's top bot (the bots' own rule, `plays::pair_up`: kills + 2 x streak at least 8 and 3 over the team's next), the stand-in
  human (`SRP_BENCH_HUMAN`) and the other bots: deaths a minute alive (live top 1.19, others 0.81), the share of deaths with 3 or more
  distinct enemies within 30 m in the 10 s before (live top 41 to 45 %), enemies within 30 m at the death, seconds from a life's
  start to the first enemy within 30 m (live median 14 to 15 s for a top's lives), and a `crowded death:` note for a top's or the
  human's crowded death: the enemies near with the play each ran and how each one's track of the dead came to it
  (Seen, Heard, Told, Hud, Cheat(Radar), Cheat(Thief), none). Distances are straight lines.
- **Assists** are counted for several windows at once (`owner-test scoreboard:` prints the assists a kill for 1, 2, 3, 5, 10 and 30 s):
  the window that gives the live 0.19 a kill is the one to use.
