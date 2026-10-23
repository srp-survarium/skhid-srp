# Player bots: behaviour design (0.26g)

What server-driven bots (`player_bots.md`) should do in a match, built on the data the server really has. How to build it: `player_bots_research.md`. A design, nothing of it is built yet. Tags as elsewhere: PROVEN, LIKELY, GUESS; "players' word" is what people who play the game told us.

## 1. Game facts this rests on

Players' word (2026-10-14):
- Picking up a battery takes time.
- A battery carrier is slower and cannot shoot.
- The HUD shows only marked batteries and the batteries last picked up by the player's own team (PROVEN in the client since, below).
- A battery the team picked up and did not deliver, when stolen, just disappears from the minimap: the thief is not shown.
- Spawn areas can be one-way (left by drops), and there is nothing in them to go back for.

From the code and data (`player_bots.md` §4, §5):
- **Inputs:** every player's reach the server at once: keys (walk, sprint, crouch, aim, fire, reload, weapon switch) and turning. PROVEN.
- **Worlds, every ~5 s:**
  - alive, pitch, yaw and position (PROVEN);
  - crouch, jump and sprint flags, horizontal speed and recent hits (PROVEN);
  - health (LIKELY).
- **Statistics in the worlds:** kills, deaths, streaks, battery bring (event 24) and steal (event 26) counts. PROVEN.
- **Loadouts:** each player's weapons and armour, from the match's start info.
- **Map:** spawns, battery spawners, containers, stations, resupply zones, the walker mesh and the bullet mesh (`player_bots.md` §5).

From the client code since (`player_bots.md` §2):
- **Use** is bit 21, acting on the key going down: a pickup is seen in the inputs the moment it starts. PROVEN.
- **Grenade** is bit 23, held until the throw starts. PROVEN.
- **Pickup:** taking a battery is instant in code, with a hands-up animation time-scaled to last `take_time` (PROVEN, `battery_pickup.md`): 0.6 s, 0.5 s from content-edit battery-pickup-1. Players feel it takes time; the bots have no pick-up time in code (they press use every 1.2 s until they carry).
- **Carrier speed:** ×0.95, 3.325 m/s walking (PROVEN in code); the saved worlds show carriers at 3.32–3.33 m/s, LIKELY unable to sprint.
- **What the HUD shows of the batteries** (read 2026-10-16 for the fair battery knowledge, §2.1). PROVEN in 0.21d with its symbols; the 0.26g functions matched to them have the same shape (the mask moved from +0x1B4 to +0x1A4):
  - Each battery (`victory_item_core`) holds a two-bit team mask (bit 1 team 0, bit 2 team 1). The minimap's lying batteries (`gather_victory_items_rule::fill_match_hud_state`, 0.21d 0x1FBDF4, 0.26g 0x3140C0, drawn by `update_minimap_objects` as "artifact") and the battery icon on the screen (`game_world_ui::update_icons` 0x183437) are the batteries with no carrier whose mask holds the player's team.
  - The mask is 0 when a battery is made (0x1E718E) and when it is put in a container (`deactivate` 0x1E7871; 0.26g 0x30104E). A pickup (`initialize` 0x1E7621; 0.26g 0x300BEA) and a drop, the carrier's death too (`deactivate` 0x1E78B3; 0.26g 0x3010A4), empty it and then call `set_spotted` with the carrier: the mask is the carrier's team alone. `set_spotted` (0x1E7F5F) adds the spotter's team; `victory_item_core::can_be_spotted_by` always says yes.
  - So "marked batteries" are batteries a player spotted: with the spot key (bit 28; the path from the key to `set_spotted` is traced in `client_patch.md` section 3.35). "The team's own" are the ones its player last carried and dropped. A battery that appears at a spawner or a station is on no HUD until spotted, and a stolen one leaves the victims' HUD (the thief's team only), as the players said.
  - The containers are always on the minimap (own and enemy base). The stations are on it in timed batteries only (0.26g 0x238CC9: "battery_spawn" icons when the mode is 3); the battery spawners never are.

Still to read from match recordings (`SRP_RECORD_DIR`): how long humans really stand at a battery, and how they throw grenades.

## 2. What a bot may know

The server knows everything; a bot that used it all would play like a wallhack. Every enemy fact passes one of these senses; the only exceptions are the small cheats listed below.

| Sense | Rule |
|---|---|
| Seen | Line of sight on the bullet mesh, inside a ~90° view, after a noticing delay of 0.2–0.5 s (longer far away, shorter if the enemy moves or fires) |
| Heard | Shot ~60 m, sprint ~15 m, reload or weapon switch ~10 m; the position error grows with distance |
| Told | What one bot senses, its bot teammates know ~1 s later (a callout) |
| Inferred | What the HUD and kill feed give a human: deaths and respawn timers, score, match clock, the battery markers (marked ones and the team's own) |
| Forgotten | An enemy not sensed for ~3 s becomes a last known position that fades |

**Batteries:** a fair bot knows what the HUD shows (marked batteries and its team's) and finds the others by checking the spawner positions; a battery a bot sees becomes known to its team. This is built (§2.1) and the default since 2026-10-16; the battery cheat below (every battery known) is off unless switched on.

**A little cheating** (players' word, 2026-10-14: "a bit, not too much"). The plays work better with a few things a human can't know, kept small and never in the aim:
- **Where batteries lie** (off by default since 2026-10-16, `CheatTuning::batteries`): every battery's spawner and state, not only the HUD's. Saves the search, changes no fight; in a real match it showed (bots ran straight to the batteries).
- **The thief:** when a battery of ours is stolen, his position with ~10 m of error, refreshed every few seconds until he is seen or delivers.
- **A rough radar:** the nearest enemy's position with ~15 m of error every ~5 s while none is sensed (a human's map sense). It steers routes and where to look, never a shot.
- **Senses a notch better:** hearing ranges ×1.25, last known positions kept ~5 s instead of ~3 s.

Still never:
- shooting at, aiming at or pre-tracking an enemy who isn't seen;
- reacting faster than the noticing delay;
- perfect recoil and spread.

**Difficulty:** a setting may loosen or tighten this (for example "hunter" bots with a sharper radar, or "fair" bots with none). The default is the small set above, without the battery cheat.

### 2.1 Fair battery knowledge (built 2026-10-16)

Used when `CheatTuning::batteries` is off (`bots/tuning.rs`): **the default since 2026-10-16**, the owner's choice after the numbers below. The owner found the all-knowing bots "a bit cheating" in a first real 4 v 4 (they ran straight to the batteries); this is what a human team knows instead.

What a team knows (`know.rs` `TeamKnowledge::batteries`, `spawners_seen`):
- **The HUD** (`TeamKnowledge::hud_update`, from the world's own mask of §1: each lying battery's `m_spotted_mask` comes with the world's batteries, `WorldFacts::marks`; team t's HUD shows it when bit `1 << t` is set, so 1 = team 0, 2 = team 1, as every drop in matches 0x652d…, 0x1a09…, 0x7298…, 0x225d… showed): a battery is known where it lies while our bit is set, wherever the game moves it (2026-10-16: level_01 has a floor hole at about (-43.5, 3.8); a battery dropped there falls and is put back about 2 m away with its mask kept, and the old reckoning from moves, a battery that moved more than 1 m left the HUD, lost it for 125 s and 32 s in matches 0x1a09… and 0x7298…). It leaves our HUD when anybody picks it up, when an enemy dropped it last, and when it is put in. Whatever else sets a team's bit (a player's spot, LIKELY, §1; none was seen in those four matches) reaches that team's bots too, as its HUD shows it. One our teammate carries is known by him (the HUD shows teammates); the containers' counts are on the HUD. A view without the masks (a tool or test that gives none) falls back to the old reckoning from the states. The bench (`bots/tests.rs`) gives the mask as the client does: 0 when a battery appears, the carrier's team's bit when he drops it or dies with it.
- **Sight** (`sense::sense_batteries`, each think): a battery lying in the bot's ~90° view within `battery_sight_m` (35 m, GUESS: small but lit) in a clear line on the bullet mesh, or within `battery_near_m` (3 m) at once, becomes known to the whole team where it lies. A spot the team believes holds a battery, seen so and empty, is forgotten: until then a battery taken by an enemy out of the bots' sight is still believed where it was seen (a human would not know either). A battery spawner's spot seen so is checked. At most `battery_rays` (3) rays a think, nearest first, from the poll's ray budget.
- **The spawners' spots:** in the battery mode the 10 batteries lie at spawners drawn at random at the start (`gather_victory_items_rule::select_spawners`) and never come again, so a spot once seen is done; in timed batteries a station's spot is worth a look again after `spawner_recheck_ms` (45 s), and the Station play waits there anyway.

What the plays do with it (`plays/collect.rs`, `Ctx::battery_lying`, `Ctx::battery_at`):
- Collect (and Ambush) consider only the batteries the team believes lying somewhere; a fetch gives up when the battery is seen gone.
- While batteries are missing (the mode's count less those in containers, carried by teammates or believed lying, `plays::batteries_unaccounted`), Collect offers the spawners' spots worth a look (`TargetRef::Spawner`), by no more bots than batteries are missing. A spot weighs `search_weight` (0.9) times the chance a missing battery lies in sight of it: missing batteries × spots worth a look within `cluster_m` (30 m) of it ÷ all spots worth a look, at least `search_min` (0.25); so a known battery comes first and a yard of several spots before a lone one. A new look is not taken within `spread_m` (20 m) of a spot a teammate is going to; a bot keeps its own spot (without that, bots near each other traded spots every commander tick: tried, 62,000 looks taken in 54 runs and bots standing 19 % of the time). A look ends once any bot of the team has seen the spot.
- **An honest search** (2026-10-16, after matches 0x7db2c16a/0x15aa5b99, where three batteries lay in level_01's swamp all match and team 0 searched 12.4 bot-minutes after nothing findable was left, 14 kills for 30 deaths): the chance above counts each spot not yet seen by how likely it still holds a missing battery (`collect::search_prior`). A spot no way leads to all match (`Ctx::shut_all_match`, the swamp) surely keeps what lies there (1) and is never offered; any other spot's likelihood falls by up to `clear_share` (0.8), the nearer the enemy's container (from the map's middle on) and the longer the match has run (fully after `clear_s`, 120 s): the enemy clears his side. The numerator counts only the reachable spots within `cluster_m`. The spot's worth is then multiplied by its risk: each death of the team within `danger_m` (25 m) of it, or of a bot sent to look at it, in the last `LOSS_KEEP_MS` (120 s) takes `danger_cut` (0.4) off, fading (the kill feed, `TeamKnowledge::losses`); and a spot in the enemy's third loses up to `deep_cut` (0.5), the deeper the more. A spot whose chance × risk is under `search_cut` (0.1) is not looked at.
- **A lost battery** (match 0x15aa5b99: a thief the bots killed dropped our battery in a nook 3.6 m from our container; the game shows a dropped battery on the HUD of the dropping side only; the battery sight missed the nook in 157 of 161 looks; 126 s unknown): when an enemy dies whom the team knew to carry a battery (a theft of ours, `Theft::thief`, or a bot saw him carrying it, `TeamKnowledge::carriers_seen`), the team gets a look spot (`LookSpot`) at his last place its own senses gave: the killer's sight if a bot of the team killed him, else the newest own sight, hearing or callout of him (`Track::own`; never the thief cheat's place), at most 30 s old. Collect offers it (`TargetRef::Point`) at `look_weight` (1.0) times its score, above any spawner's spot, one bot a spot; the look ends once a bot of the team stood within `battery_near_m` of it (what lies there it then sees), when the battery is known, home or carried by a teammate, or after 120 s.
- The battery sight tries a lying battery, and a spot the team believes holds one, at three heights (`sense::BATTERY_HEIGHTS`: 0.25, 0.12, 0.04 m; the client's battery collides as a box of half sizes 0.3 x 0.12 x 0.16 m set 0.12 m up, `!victory_item_collision!` in `gameplay/victory_items/default.options`, model `item/quest/electrobox`); a spawner's spot at the top only. A spot believed to hold a battery and seen empty at any height is forgotten, so a take's stand (its line is to the middle) does not keep trying a battery gone from a crate. The nook of 0x15aa5b99, probed with the bots' own sight code (`aimpoint::sight_line` on level_01's bullet mesh, team 0's bots from 462.2 to 588.3 s with their feet 0.9 m / 0.55 m under the worlds' capsule centre): of 104 worlds with the battery in view within 35 m, 0 clear lines at +0.25, +0.12 or +0.5 m, 0-1 at +0.04 m, 0-2 at +1.0 m (battery y as the world gave it and 0.12-0.24 m lower). Heights do not find it; the look spot does.
- With the cheat on nothing of this runs: the decisions digests of the soaks and of the real-map runs are the same as before (15 of 15, `private/mapdata-digests/decisions-cheat-switch-on.txt`, made with `SRP_BENCH_CHEAT=1`, against `decisions-straight.txt`). The default's digests from here on: `decisions-fair-default.txt` (Team Deathmatch's unchanged).

**Measured** (2026-10-16, `real_bot_cost` with `SRP_BENCH_BATTERIES=10`: 10 batteries at spawners drawn at random, as the game places them, instead of one at every spawner; the cheat then the default and `SRP_BENCH_FAIR=1` for it off, now off by default and `SRP_BENCH_CHEAT=1` for it on; scripts `private/mapdata-digests/fair-runs.sh`, `fair_summary.py`, logs `fair-*.log`). Batteries delivered (both sides), kills, and the share of bot ticks standing with a goal and no route:

| Run | Cheat on | Cheat off |
|---|---|---|
| 180 s, 6 battery maps (levels 1-6, level 3 timed), 4 v 4 and 10 v 10, 8 seeds (96 runs) | 566 delivered, 1,449 kills, stood 7.2 % | 501 (−11 %), 1,447, 6.7 % |
| the same, 4 v 4 only (48 runs) | 293, 318, 5.5 % | 250 (−15 %), 279, 5.4 % |
| the same, 10 v 10 only (48 runs) | 273, 1,131, 7.8 % | 251 (−8 %), 1,168, 7.2 % |
| 600 s, the 6 maps, 4 v 4, 8 seeds (48 runs) | 409, 1,216, 7.1 % | 376 (−8 %), 1,236, 6.5 % |

- The loss is the opening search: over 600 s the fair bots bring 8 % fewer, over the first 180 s 11 %; kills are about the same (fewer in the short 4 v 4 runs, more in 10 v 10 and over 600 s), and they stand less.
- Rays: 73.1 a bot a second with the cheat off against 72.1 (the battery looks). The bots' time is not comparable here (the runs ran side by side).
- Tried and left out: a looking bot looking at its spot from up to 15 or 25 m (249 and 252 delivered against 258 without, 4 seeds); spots within 45 m counted as one yard instead of 30 m (263 against 258) and a battery seen from 50 m instead of 35 m (262 against 258), both within the noise; a first search without the cluster weight brought 469 in the 96 runs (−17 %).
- The owner's rule (2026-10-14) is that bots never get dumber; the fair bots bring fewer batteries, the cheat is the "a bit, not too much" of §2. The owner chose the fair bots as the default (2026-10-16), knowing these numbers.

### 2.2 Taking a battery (built 2026-10-16)

In two real matches (0x7db2c16a, 0x15aa5b99) bots ended a take "could not take it" 35 times at or on a battery; one lay 98 s at its team's feet. The take (`plays/collect.rs`, `take`) now:
- uses where it stopped only if a use reaches from there: the crouched eye within 1.6 m, a look down of at most 0.98 rad (the view stops at 1 rad), and a clear line on the bullet mesh from the eye to the battery;
- else, or after two uses there, goes to another stand on a ring round the battery (eight ways at 0.6, 0.9 and 1.2 m, on floors a walk may get to, crouched or standing, the eye within 1.7 m, a look down of at most 0.95 rad, the line clear), the nearest, the other stance than the last, on a side not tried; up to four stands a play, two uses each, the second aimed 0.1 m lower;
- presses once the view is on the battery (its own estimate), or after 1.5 s there;
- right over the battery, walks off to a stand in a line; wedged (no stand off it in a line), steps back the way it came (else behind its back), straight, with no line looked at;
- keeps the stands it gave up at (per bot and battery, 90 s), so the next take of that battery stands elsewhere;
- walks the last metres to a stand straight by keys (`Intent::straight`, `Motor::step_to`): the route's arrival (0.6 m) is too loose for a stand.

**What the matches showed** (worlds, the bots' traces, the bots' bullet mesh of level_01):
- PROVEN: the worlds' player position is the body's capsule centre, not the feet (`Body::feet`: 0.9 m standing, 0.55 m crouched below it). A bot's own trace has its feet 0.55 m under the world's position crouched and 0.90 m standing. Taken as feet, the bot's look down seemed 0.27 rad short; from its feet, the pitch it asked matches its geometry to 0.004 rad at all three stands of the case, and the world's pitch is what it asked. So no pitch-unit error shows here.
- PROVEN on the bots' mesh: from the two stands where a bot failed 9 times (1.3 m east of the battery), the line from the crouched eye to the battery is blocked 0.77-0.80 m along a 1.45 m ray; from the stand where it took it (0.6 m, another side) the line is clear. A floor 1.3-1.4 m over the battery at 0.25-0.97 m off it (an overhang): the other bot stood under it, right over the battery, 25 s.
- LIKELY: the client's use ray stops at the level's geometry as the bots' mesh does (the case above; not read in the client's code).
- GUESS still: the crouched eye 0.95 m over the feet (`view::EYE_CROUCHED`) and the crouched capsule centre 0.55 m; the one take that worked bounds the eye at 0.75-1.0 m over the feet.

**Measured** (`real_bot_cost`, 180 s, the 6 maps, 4 v 4 and 10 v 10, budget 10,000; `private/mapdata-digests/take-bench.sh`, `take_summary.py`; ± is the seed-to-seed standard error of the difference a seed):

| Run | Before (19b4931) | After | Difference a seed |
|---|---|---|---|
| 20 seeds (240 runs): delivered | 2,434 | 2,479 | +2.3 ± 3.3 |
| kills | 3,262 | 3,253 | −0.5 ± 3.1 |
| stood with a goal and no route | 5.97 % | 5.89 % | −0.08 ± 0.12 |
| failed searches | 33,821 | 34,732 | +46 ± 77 |
| 10 seeds: "could not take it" ends | 634 | 96 | −54 ± 7 |
| 10 seeds, the use stopped by the bullet mesh (`SRP_BENCH_USE_LINE=1`): delivered | 1,182 | 1,244 | +6.2 ± 3.8 |
| kills | 1,596 | 1,582 | −1.4 ± 4.1 |
| "could not take it" ends | 1,175 | 94 | −108 ± 7 |

- The stand-in counts a step walked straight as no standing (it has no route).
- A first version pressed at stands it had passed by up to 0.5 m (standing uses 2.1 m long) and over looks down past 1 rad: kills −7.2 ± 2.9 a seed; fixed before the numbers above.
- Left: a battery the team believes lying where it is not (it lies on a crate and the look at it from below is blocked) is still tried at each stand (seen in the soaks); the team's look at it is `sense::sense_batteries`.

### 2.3 A take waits for the fight unless covered (built 2026-10-23)

A take is a use: the bot looks at the battery and its gun is down for the ~0.6 s animation, and the carrier cannot shoot after it. In the battery soaks the takers had an enemy in sight 91 % of their time next to a battery, so "never take in sight of an enemy" would bring back the batteries that were never taken. The owner's rule (2026-10-23): the bot does not start the take while a known enemy close by can see it, unless that enemy is reloading or down, or a teammate covers it.

Within `hold_near_m` (3 m) of the battery, `take` (`plays/collect.rs`) stays and fights (the play's legs stop; the fight layer keeps aiming and firing) while `Ctx::exposed_to` (`plays/mod.rs`) names a threat:
- a threat is an enemy, alive and present, seen by the bot within `SEEN_NOW_MS` (700 ms), within `exposed_m` (25 m: "close by"), with a clear line from his eye to the bot's, and no Reload sound of his within `reload_cover_s` (1.5 s);
- a cover is a teammate of the bot, alive, present and not carrying, within `cover_m` (40 m) of the threat with a clear line from his eye to the threat's;
- each threat needs a cover of its own: with two enemies in sight, one teammate who sees both covers one of them only (`plays::cover_each`, a matching by augmenting paths), and the bot waits for the other.

The numbers are GUESSES; only `exposed_m` was tried at two values (below). `exposed_m` 0 turns the rule off.

**Measured** (2026-10-23). The flat floor: `a_take_waits_for_the_fight_unless_covered` in `bots/tests.rs` (the battery soak, 4 a side, the first to bring 3 wins, at most 900 s, seeds 1-16, the cheat off and on: 32 matches a side). Real maps: `real_bot_cost`, 180 s, the 6 battery maps (levels 1-6, level 3 timed), 4 v 4 and 10 v 10, `SRP_BENCH_BATTERIES=10`, `SRP_BENCH_USE_LINE=1`, seeds 1-16 (192 runs a side), off with `SRP_BENCH_COLLECT=exposed_m=0`; ± is the seed-to-seed standard error of the difference a seed:

| Run | Rule off | `exposed_m` 25 m (kept) | Difference a seed | `exposed_m` 40 m | Difference a seed |
|---|---|---|---|---|---|
| flat floor: takes | 214 | 188 | | 157 | |
| carriers killed within 3 s of a take | 49 (23 % of takes) | 32 (17 %) | | 22 (14 %) | |
| batteries brought | 95 | 95 | | 86 | |
| matches ended level | 1 | 1 | | | |
| real maps: delivered | 620 | 616 | −0.2 ± 0.3 | 616 | −0.2 ± 0.5 |
| carriers killed carrying | 177 | 156 | −1.3 ± 0.7 | 160 | −1.1 ± 0.7 |
| picked up | 1,015 | 994 | −1.3 ± 0.7 | 994 | −1.3 ± 1.0 |
| kills | 2,138 | 2,107 | −1.9 ± 1.5 | 2,103 | −2.2 ± 2.0 |

- The flat floor has no cover: a bot by a battery is nearly always in an enemy's sight there, so the rule bites hardest. At 40 m it halved the takers' deaths but brought 9 % fewer batteries; at 25 m a third of those deaths go and the batteries brought stay.
- On the maps the batteries delivered are the same within the noise at either distance, and an eighth fewer carriers are killed carrying; the rest is within the noise.
- `with_the_battery_cheat_whole_matches_are_won_by_batteries` moved from seed 1 to seed 2: seed 1 ended level, 1-1, with the rule (a level end came in 1 of the 32 soak matches with the rule and 1 without it, at other seeds).

### 2.4 Who carries a battery, and the put-in (built 2026-10-23)

The worlds say a battery is carried, not by whom (§1): the view (`bots/view.rs`, `Tracker::carriers_seen`) guesses the carrier. In the owner's matches of 2026-10-22 (the audit of `private/pvb-jumps-local/analysis/AUDIT.md`, #2) a bot at its own container was believed to carry a battery a teammate beside it had picked up: the old rule gave it to whoever had pressed use last within 4 m; the bot tried 9 put-ins and the real carrier walked off, not knowing he carried it. Now:
- **Who took it.** Each player's last 4 use presses are kept with his eye and view (`UsePress`; the client's use ray is 2 m, PROVEN). A battery newly carried goes to the player, alive, not of a thief's team for a battery taken from a container, and not believed to carry another, whose press's ray passed nearest the battery (its middle, 0.12 m up; a container's battery 1 m over it), plus a quarter of his feet's distance, plus 1 if the press came more than 1 s before the last world (it was judged already); a press within 6 s and 4 m. Without such a press, the nearest player within 6 m at a cost of 3 + his distance. The others in reach are kept, cheapest first.
- **The belief moves.** When its carrier's 3 uses at his own container (feet within 2.5 m) put nothing in, each judged by a world 1.5 s after it, or he died and the battery stayed carried 2.5 s, the belief goes to the next of the others still alive and carrying nothing; with none it stays.
- **The log** (`Note` place `usize::MAX`): `battery 3 picked up by 4: its use's ray 0.01 m off it, 0.6 m away; the others near: 5 (7.9)`; `player 4 no longer believed to carry battery 3 (3 uses at his container put nothing in); now player 5`; a battery dropped more than 4 m from its believed carrier, `battery 3 fell 6.2 m from player 4, believed its carrier`.
- **The put-in and the take wait for the world** (`plays/collect.rs`, `use_seen`): the world showing a use comes a second or two after the press (worlds come every second, behind the clock), so a second press "failed" while the first had put the battery in, and the bot took a new play that ended "put it in" within a second (291 of 442 put-in plays on the bench, 8 seeds). A use is judged by a world `USE_SEEN_MS` (300 ms, GUESS) after it, or after 3 s without one; then the next press, or the failure. A carry that ends with the bot not carrying says how (`carry_ended`): "put it in"/"brought home", "someone else carries it", or "dropped it".
- **A failed take's log** gives the last use's ray: `took no battery 0 from a stand 0.9 m off it (standing): its use's ray 1.68 m to it, 42 deg down, the view 0.1 deg off it`.

**Measured** (2026-10-23; `real_bot_cost` as in §2.3, `SRP_BENCH_BATTERIES=10`, `SRP_BENCH_USE_LINE=1`, seeds 1-16, 192 runs a side; the battery soaks `raids_in_whole_battery_matches`, 4 a side to 3, 900 s, seeds 1-8 cheat off and on; ± the standard error of the difference a seed, * over 2 of them). A seed's sums:

| Run | Before (55f1042) | After | Difference a seed |
|---|---|---|---|
| real maps: put-ins failed | 36.9 | 0.3 | −36.6 ± 0.8 * |
| Collect plays a pickup | 5.82 | 5.21 | −0.62 ± 0.15 * |
| plays ended within 1 s | 367 | 345 | −22 ± 7 * |
| batteries picked up | 61.5 | 63.8 | +2.3 ± 1.0 * |
| batteries put in by the play's carrier | 18.8 | 22.2 | +3.4 ± 1.3 * |
| delivered | 38.4 | 39.6 | +1.3 ± 0.9 |
| carriers killed carrying | 9.8 | 11.8 | +2.0 ± 0.8 * |
| intercepts ended "he no longer carries it" | 9.6 | 11.3 | +1.7 ± 0.8 * |
| carrier believed wrong (world-s) | 1.7 | 1.2 | −0.5 ± 0.4 |
| kills, all | 132.9 | 136.7 | +3.8 ± 2.2 |
| K/D team 0 / team 1 | 0.92 / 1.13 | 0.90 / 1.18 | −0.02 ± 0.02 / +0.05 ± 0.03 |
| deaths team 0 / team 1 | 75.8 / 67.9 | 79.0 / 69.2 | +3.3 ± 1.6 * / +1.3 ± 1.6 |
| A→B→A flips in 10 s | 90.1 | 90.8 | +0.8 ± 2.8 |
| stood with a goal, ticks | 20,093 | 19,964 | −129 ± 281 |
| soaks: put-ins failed | 2.31 | 0 | −2.31 ± 0.20 * |
| soaks: delivered | 3 | 3 | 0 |
| soaks: kills team 0 / team 1 | 109.9 / 105.7 | 107.9 / 107.0 | −2.0 ± 1.7 / +1.3 ± 1.5 |

- The whole-match tests pass (`four_bots_a_side…`, `with_the_battery_cheat…`, `timed_batteries…`, `a_take_waits…`, `bots_hold_fire…`, the Team Deathmatch).
- Both teams run this code: more batteries move, and the interceptors chase the right carrier, so more carriers die and both sides fight more (team 0's deaths up 4 %); neither team's K/D moves beyond the noise.

**Tried and left out** (no clear benefit by the owner's rule):
- *A battery two bots of the team failed to take, left by the whole team for 45 s* (audit #7). With the first two batteries of each run made untakeable (`SRP_BENCH_UNTAKEABLE=2`, 8 seeds): failed takes −9.0 ± 2.6 and failed stands −23.5 ± 5.8, but delivered −0.4 ± 0.6, team 0's deaths +2.0 ± 0.8 and standing with a goal +497 ± 248 ticks; on the plain bench it never fired.
- *A carrier camping deep in his ground* (audit #6: kept within 3 m of a spot 20 s, in the far third; a quarter of his score, no alarm, a run from beyond its band ended). On the bench it fired 0.25 times in 8 seeds' runs, nothing changed beyond the noise; a scenario (a carrier set still at the far end of the soak's floor, 8 seeds, cheat off and on) came out the same with and without it to the kill.
- *The cause of the failed takes* (audit #7) is still open: on the bench, 10 of 12 failed stands were standing ~1 m off with the eye 1.4-2.0 m from the battery (median 1.67 m, near the 2 m ray's end) and the view on it; the bench's take is the stand-in's own rule (`uses_battery`), so the real client's reason needs the new log lines from real matches.

### 2.5 Target choice and the aim-in (built 2026-10-23)

Audit issues 4 (a close enemy ignored, a far target kept) and 8 (bots hit 5.5-9.9 % of their shots in the owner's matches against his 20.5-23.2 %).

**Where the misses came from.** The bench (`real_bot_cost`) had no bullet spread: every shot left along the view. The 0.26g spread is now read from the executable and the configs (`player_bots_weapons.md` §9) and the bench applies it with `SRP_BENCH_SPREAD=1` (`bots/bench_spread.rs`; the shots' tally in `bots/fight_tally.rs`). With it the base bots hit 14.8 / 16.6 / 17.5 % at tiers 4-5 / 9-10 / 13-15. 35-40 % of their shots left before their sights were fully up (`aim_time` after AIM is pressed), and those hit 9-11 % against 18-24 % once aimed in: from the hip or during the aim-in, the stance and movement add 5-20 degrees to the cone (10.3 degrees strafing with an Uzi), while fully aimed-in the cone is the weapon's base alone (0.3 degrees), strafing or not. The miss was the habit of firing as the sights came up, not the aim. The bots keep strafing as they fire.

**What changed:**
- **Aim-in first** (`CombatTuning::aim_in_first`, `reflex::hip_cone_fits`): no shot until the aim has been pressed for the weapon's `aim_time` (from its config) and one tick, unless the hip cone of the bot's stance and movement (from the weapon's `dispersion` block and `default.player`) keeps 58 % of the shots within a half torso (0.25 m) at the target's distance; then it fires from the hip without the aim. The aim is pressed whenever the hip cone does not fit, near or not. The skill tiers keep their aim error, reaction and settle (`signals.rs`): the change is only when to press the trigger.
- **Steps heard** (`SenseTuning::step_m` 8 m): an enemy walking upright (faster than 1.5 m/s, not crouched) is heard within 8 m. Before, a walking enemy made no sound (in match 0x4da3b51d a bot stood 1-4 m from an enemy for 8 s).
- **Wider switch** (`switch_share` 0.65 from 0.5, `switch_near_m` 25 m from 15 m): a seen enemy nearer than 0.65 of the target's distance, or nearer than 25 m and 5 m nearer, takes the target.
- **Switch between thinks** (`switch_between_thinks`): an enemy noticed between two thinks who takes the target by those rules is taken at that tick, not at the next think.

**Tried and left off** (bench, tiers 9-10, the change without that part, 8 seeds; ± is the standard error of the difference a seed):
- A firing gate scaled by the body's width at the distance (`fire_body_share` 3): +1.6 points of hit rate alone, but with the aim-in −11.5 kills a seed.
- A hold of 300 ms on a target before the near rules apply (`switch_hold_ms`), against flicking: no measure moved by 2 standard errors (flips +1.5 ± 7.6 without it).
- A nearer enemy facing the bot (within 20 degrees) taking the target from one not facing it (`switch_facing_deg`): nothing better with it; without it enemies within reach engaged within 1 s +3.1 ± 2.2 points.
- A glance at any sound heard within 6 m while holding a target (`glance_close_m`): no measure moved (kills +0.75 ± 0.41 without it, the rest within 1 point); the steps heard do that work.
- The wider switch alone cost kills (−6.4 a seed, significant); with the steps heard and the switch between thinks it brings farther targets down (below).

**Measured** (2026-10-23). `real_bot_cost`, 180 s, the 6 battery maps (levels 1-6, level 3 timed), 4 v 4 and 10 v 10 (12 runs a seed), `SRP_BENCH_BATTERIES=10`, `SRP_BENCH_USE_LINE=1`, `SRP_BENCH_SPREAD=1`, seeds 1-8, the tier with `SRP_BENCH_TIER`; the base is `4a1ef11b`. Means a seed (12 runs); * marks a difference over 2 standard errors:

| Measure | 4-5 before | after | 9-10 before | after | 13-15 before | after |
|---|---|---|---|---|---|---|
| hit rate | 14.8 % | 20.8 %* | 16.6 % | 23.6 %* | 17.5 % | 26.3 %* |
| shots | 5,155 | 3,619* | 5,101 | 3,504* | 4,853 | 3,312* |
| kills, team 0 / 1 | 59.1 / 68.4 | 60.3 / 66.1 | 72.1 / 74.1 | 66.3* / 73.0 | 68.6 / 75.3 | 73.6* / 76.8 |
| deaths, team 0 / 1 | 79.4 / 60.8 | 74.4 / 60.8 | 82.9 / 72.5 | 80.4 / 67.4* | 86.1 / 69.3 | 84.0 / 74.5* |
| K/D, team 0 / 1 | 0.74 / 1.13 | 0.81 / 1.09 | 0.87 / 1.02 | 0.82 / 1.08 | 0.80 / 1.09 | 0.88 / 1.03 |
| batteries delivered | 38.3 | 37.8 | 36.9 | 38.6 | 36.3 | 36.1 |
| carriers killed carrying | 10.9 | 12.1 | 12.0 | 14.5 | 11.0 | 12.9 |
| contacts a farther enemy aimed at | 12.4 % | 10.2 %* | 12.2 % | 10.8 %* | 11.8 % | 10.5 % |
| enemies within reach engaged within 1 s | 68.8 % | 76.9 %* | 71.9 % | 79.3 %* | 71.2 % | 79.7 % |
| ... never engaged | 24.8 % | 19.7 %* | 24.5 % | 18.5 %* | 25.0 % | 17.9 % |
| plays ended within 1 s | 374 | 356 | 366 | 344 | 338 | 363 |
| target flips A→B→A | 95.0 | 83.5 | 94.4 | 82.5* | 82.6 | 88.5 |
| standing with a goal and no route | 4.0 % | 3.6 % | 4.0 % | 3.9 % | 3.8 % | 3.7 % |

- The hit rate rises at every tier and keeps the tiers' order; a third fewer shots land as many hits, and the total kills are the same within the noise (−1.1 ± 3.8, −7.0 ± 4.4, +6.5 ± 3.9).
- The per-team shifts at tiers 9-10 and 13-15 go opposite ways (team 0 kills fewer at 9-10, more at 13-15): both teams are the same bots, so it is the sides of the maps, and team 0's K/D is better at two tiers of three.
- **On top of the play-choice change** (#359, merged into the base meanwhile; tiers 9-10, the same binary with this change off by the bench's settings and on): hit rate 16.5 → 23.4 %*, farther targets 13.0 → 11.3 %*, near enemies engaged within 1 s 73.1 → 77.5 % (+4.5 ± 2.4), kills 141.6 → 141.9, delivered 38.5 → 37.9 (−0.6 ± 1.0), flips 80.5 → 69.3.
- **Without the bench's spread** (the old bench, where every shot flies along the view, tiers 9-10): hit rate 19.7 → 23.9 %*, kills 154.5 → 137.5 (−17.0 ± 4.6*): with no cone to beat, waiting for the sights only delays the shot. The game has the cone (PROVEN), so the numbers with it are the ones that count.
- **Whole matches** (`--ignored`, the flat floor, no spread): all 7 pass before and after. The take soak brought 95 → 96 batteries a side with no level end (one before); with friendly fire off the bots killed 181 → 158 teammates; the raids 449 / 469 → 423 / 409.
- The log notes of seeds 3 and 6 show the same plays taken; one 4 v 4 on level 3 took Retreat 85 times against 25, but over all runs Retreat is not more frequent (−13.6 ± 25.7 a seed at tiers 9-10).

**Tests:** `reflex::tests::a_close_enemy_beside_a_bot_with_a_far_target_is_taken_at_once`, `a_nearer_enemy_facing_the_bot_takes_the_target_from_one_who_does_not`, `steps_heard_within_six_metres_are_glanced_at_while_holding_a_target` (these two with the parts left off set on), `it_fires_only_once_the_sights_are_up_unless_the_hip_cone_fits`; `sense::tests::a_walking_enemy_near_behind_is_heard_by_his_steps`; `weapons::tests::the_dispersion_is_read_from_the_configs`; `bench_spread::tests`. `lost_battery_run` keeps the shots before the sights are up (its thief must take the battery before he dies), and the near-hip part of `it_aims_down_the_sights_and_steps_side_to_side_at_a_seen_target` turns the aim-in off.

**Not verified:** no real match has been played with this. Whether the game drops the aim during a reload while AIM stays held is not traced (the aim-in counts from the bot's own press). The bench does not model recoil.

## 3. Plays

Each play has a score from the situation, a precondition and steps. A commander per team hands them out every ~0.5 s: never two bots on one ambush, and no bot parked at its own container. In the saved worlds lives that stay in their own third are the worst in every mode (K/D 0.3–0.6, ~0.03 batteries a life), so a steal is answered by the 1–2 nearest bots (the thief cheat and the alarms in §4); the only guard is play 18.

1. **Battery ambush.** An enemy is heading to a battery no friend covers.
   1. Hide 10–25 m from it, with a line of sight to the pickup point and out of sight of his approach. The bot goes only if it gets there first; otherwise it intercepts (play 2). Saved worlds: the killers of players who died within 6 m of a battery spawner stood 10 / 17 / 27 m from it (quartiles), 45 % of them 10–25 m away.
   2. Crouch, stand still, aim at the pickup point.
   3. Let him finish the pickup: he then cannot shoot back and is slow. Kill him, and take the battery he drops.
   4. Shoot during the pickup only if his teammates are close.
   5. Give up after 30–40 s.
2. **Carrier interception.** An enemy carries a battery: slow, unable to shoot, bound for his container. Meet him at a choke he reaches 1–2 s after the bot; otherwise chase him.
3. **Escort.** Our carrier, bot or human, cannot shoot. One or two bots go along, one ahead and one behind looking back.
4. **Sneak into their base.** Do it when several enemies are dead (respawn timers), last known far away, or heard fighting elsewhere.
   1. Take a route around their last known positions and the routes humans use most.
   2. Crouch-walk the last 20 m.
   3. Steal, then leave by another route.
   4. Abort on contact.
5. **Punish a reload.** An enemy heard reloading or switching weapons in a duel: push or peek in that window.
6. **Turn to a sound.** A shot heard: turn toward the estimated spot, with the error of its distance.
7. **Break sight and re-peek.** After taking hits: leave the line of sight, wait 1–2 s, come back from another angle, never the same one twice in a row.
8. **Read a held angle.** An enemy crouched, aiming and still is holding an angle: flank him or wait.
9. **Crossfire.** Two bots cover one choke from angles at least 60° apart.
10. **Health.** Push a wounded enemy. A wounded bot retreats to resupply by a covered route.
11. **Weapon matchup.** From the loadouts: keep a shotgun at range; close in on a sniper through cover.
12. **Cover a human.** Follow a human teammate at 5–10 m and look where he isn't looking.
13. **The top player.** Normal bots don't duel the best enemy alone; they pair up. An optional hunter squad goes after top players on purpose.
14. **Personalities.**
    - camper: ambushes, holds;
    - rusher: intercepts, pushes;
    - flanker: sneaks, wide routes;
    - support: escorts, crossfires, covering humans.

    The mix follows how humans play (saved worlds, per life): about 55 % move steadily, 30 % mostly sprint, 10–15 % hold still. So a team of bots is mostly flankers and support, a third rushers, few campers. The losing team grows more aggressive as the gap grows.
15. **Hold the exits.** Hold forward chokes on the routes out of the enemy's base, as humans do: in the saved worlds the best kill spots of each side are mostly 30–60 m from the enemy base, and the enemy third of the map gives the best K/D (×1.3 overall, ×1.6 in Team Deathmatch). Owner's rule (2026-10-14): near the exits yes, **never inside a spawn area**.
    - **Team Deathmatch only, as a long hold.** In the battery modes the lives that reach the enemy third and keep moving bring the most (0.41 batteries a life, 0.60 in timed batteries), holding there brings 0.26 (0.00 in timed batteries): there a bot holds only briefly on its way.
    - **Never in the middle:** holding in the middle is poor in every mode (K/D 0.5–0.66).
    - **High ground, mid range:** among the allowed chokes, prefer spots above the exit with sight lines of ~15–30 m (the killer stood >2 m higher in 23 % of kills, lower in 14 %; median kill distance 19 m), not the longest open view.
    - **Move on:** after 1–2 kills or ~40 s, go to another allowed spot, never straight back to the last one; humans learn a spot fast.

16. **Late denial (battery modes).** Players' word: before the match ends, take batteries out of the enemy's container and just drop them there (no need to carry them home): the enemy's count falls (the statistics take one off the other team when a battery is taken out, `snapshot.rs` `StatisticsTail`), so a narrow lead turns. A bot does it in the last minutes when the score is close or behind and the enemy's container is reachable: steal (use, bit 21), then drop at once (bit 22), then steal the next one or fight off the defenders. Note: dropped batteries can be taken back, so it pays only near the end.
17. **Routes from humans.** For each map, mode and side, the paths humans took (saved worlds: each life's sideways offset at checkpoints along the base-to-base line, grouped) and what they gave (kills, assists, batteries found and put in). On most sides one or two routes clearly pay more (level_01 batteries, one side: K/D 2.9 on one route, 0.8 on another). Bots pick routes weighted by that reward, not always the best one, so they stay hard to read. What a bot knows then shifts the weights, and is checked again on the way (each ~0.5 s with the commander):
   - an enemy sensed, on the radar or last known on a route: that route weighs less, or more for a bot sent to fight (rusher, hunter);
   - a known battery, a thief's position or a carrier to escort: the route toward it weighs more, whatever humans did there;
   - a teammate killed on a route in the last ~20 s: it weighs less;
   - teammates already on a route: it weighs less, so the team spreads (one or two per route).
   A bot whose route turns bad on the way picks again from where it stands.

   **The best players' routes, by the bot's task (built 2026-10-23).** The table (format 2) keeps a route's kills and assists a life (`fight`) and its batteries found and put in a life (`battery`) apart. The 174 routes of all players are kept. 57 come from the top players of human matches of 10+ players, Oct 12-23, with bot matches left out. A top player is one of the side's first 2 by score (3 in matches of 12+), and only if present 60 %+ of the match with K/D 0.75-8; an unreliable one is not replaced by the next. Only lives a bot could follow count: a life that walked 2 m+ on the floors of a damage zone that is on all match (bots keep off them; 13.6 % of lives, most on levels 6, 2 and 3) is left out of the routes and of the side's averages. Such lives' deaths went to no enemy (an anomaly, a fall) 3.9 % of the time, against 1.4 % for the others. A group of their lives becomes a route when all of these hold:
   - trusted: 10+ lives from 4+ matches and 4+ players;
   - it beats the side's average life by 2+ standard errors and the top players' own average by 15 %+, in fight or in batteries, with K/D no worse than the side's;
   - it lies a bar away from every other route: 20 % of the side's travelled width, 8-15 m (RMS of the sideways offsets);
   - a bot can walk it today (the walk check in `routes.rs`): no leg without a way, under 2 jumps, under 1.5 times the straight way, half its points usable (not in a hazard or a spawn area).

   Up to 6 a side, best first. Fewer but better: past the 1.15 bar the extra groups were mostly the same way again. 124 of the 612 groups that paid failed the walk check (most for a detour round a wall or a hazard, or 2+ jumps). A new route's numbers are put on the all-player scale (times the side's average over its top players' average), so it weighs by what the way gave, not by who walked it.
   - **By task (Roam):** a route weighs `fight + 2 x battery x out`, where `out` is the share of batteries neither home nor carried by a teammate. A fighter (`fighter_push`) counts batteries at `fighter_battery` (half). In Team Deathmatch, and once every battery is in, fight alone counts.
   - **A carrier's way home (Collect):** while known danger lies within `home_danger_m` (20 m) of the straight way home, a carrier takes the rest of one of its side's routes toward home. Known danger is an enemy seen, heard or on the radar, or a teammate killed in the last 20 s. The route must pass within `home_route_m` (20 m), carry less danger, and be at most `home_detour` (1.4) times as long. It walks with its fire held and drops a point once it is reached or a teammate dies by it.

   **Measured** (2026-10-23): `real_bot_cost`, all 16 Player vs Bots maps (levels 1-8, batteries and Team Deathmatch), 4 v 4 and 10 v 10, 180 s, `SRP_BENCH_BATTERIES=10`, `SRP_BENCH_USE_LINE=1`. Base: the code before, the old table; seeds 1-32 unless stated. Each figure is the difference a seed ± standard error; * is beyond 2 standard errors.

   | | new table + code (32 seeds) | code, old table (32) | without the carrier's way (16) | without the cut merge (16) | without the task (32) |
   |---|---|---|---|---|---|
   | kills, both teams | −21.1 ± 3.1* (of 432) | −1.3 ± 1.9 | −2.4 ± 1.2 | −12.6 ± 4.3* | −1.0 ± 0.6 |
   | deaths a bot-minute | −0.017 ± 0.002* | −0.001 ± 0.002 | −0.002 ± 0.001 | −0.010 ± 0.003* | −0.001 ± 0.000 |
   | K/D team 0 / team 1 | +0.013 ± 0.013 / −0.014 ± 0.015 | +0.013 ± 0.010 / −0.011 ± 0.009 | | | +0.008 ± 0.004 / −0.008 ± 0.004* |
   | delivered (of 59) | +0.0 ± 0.6 | +0.3 ± 0.4 | −0.6 ± 0.5 | +0.3 ± 0.5 | +0.1 ± 0.2 |
   | carriers killed (of 16) | −1.3 ± 0.7 | −0.7 ± 0.4* | +1.1 ± 0.4* | +0.5 ± 0.9 | +0.4 ± 0.2 |
   | plays ended within 1 s, a bot-minute | −0.080 ± 0.012* (of 0.284) | +0.013 ± 0.010 | −0.005 ± 0.002* | −0.004 ± 0.012 | −0.001 ± 0.001 |
   | A->B->A flips, a bot-minute | −0.006 ± 0.006 | +0.004 ± 0.004 | −0.006 ± 0.003 | −0.019 ± 0.008* | −0.001 ± 0.003 |
   | idle 5 s+, % alive | +0.29 ± 0.22 (of 25.0) | −0.13 ± 0.14 | −0.03 ± 0.04 | +0.55 ± 0.26* | +0.03 ± 0.03 |
   | standing with a goal and no route, % | −0.23 ± 0.07* (of 5.9) | −0.00 ± 0.04 | +0.04 ± 0.03 | +0.10 ± 0.07 | +0.02 ± 0.02 |
   | held angles (HoldExit, Crossfire, Cut, Flank), s a bot-minute | +1.23 ± 0.07* | −0.35 ± 0.05* | −0.05 ± 0.03 | +1.33 ± 0.13* | |
   | Roam ended "the route turned bad", a bot-minute | −0.084 ± 0.013* (of 0.214) | | | | |

   The ablation columns are each against the full change, not against the base, and were run with an earlier draw of 80 new routes (before the zone walkers were left out).
   - **Which team the routes help:** new routes for one side only (old routes for the other), against the same seed with the old routes on both sides, 32 seeds each way (64). The new routes' team took +0.3 ± 0.3 points of the kills (Team Deathmatch +0.4 ± 0.4) and delivered +0.1 ± 0.2. So bots gain nothing clear from them against bots. In Team Deathmatch both teams killed less (the routes' team −4.9 ± 1.2, the other −6.8 ± 1.3).
   - With the new routes on both sides bots spread more: deaths with no teammate within 20 m +2.6 ± 0.6 points, deaths traded within 5 s −0.9 ± 0.3. The fewer kills are mostly Team Deathmatch on levels 1, 2 and 4 (level_02 10 v 10: −7.9 ± 0.9 of 31 a run); level_03's had more (+2.4 ± 0.8).
   - They ship anyway (the owner's call, 2026-10-23): the routes paid against humans, and the bench's enemies are bots. They are to be checked in live Player vs Bots matches, bots' K/D against the humans and deliveries before and after.
   - Kept for their own numbers: the carrier's way home (carriers killed −0.7 ± 0.4 with the old table, +1.1 ± 0.4 without it) and the cut merge (without it, kills −12.6 ± 4.3 and Cut deaths +3.9 ± 1.4).
   - The task weighting shows nothing clear either way: Roam is about 6 s of a bot-minute, and most battery routes' batteries are small next to their fights. It is kept as the owner's requirement (routes by the bot's task); no guard is worse.
   - Left out: dropping the old routes that fail the walk check (44 of 174). It changed kills by +0.8 ± 3.9 and plays ended within 1 s by +0.035 ± 0.009* (16 seeds).
   - **Anomalies** (`real_routes_and_the_damage_zones`, the zones as a match has them): of the 174 old routes, 43 have a point on a zone's floors (32 of an always-on zone) and 61 a leg crossing one. Roam already leaves out a point in a zone that is on when the route is drawn, and the walk goes round a zone or waits for it to switch off. Left out: moving such points at load to the nearest floor clear of every zone within 4 m (else dropping them). Roam's no-headway ends fell (−0.005 ± 0.002 a bot-minute), but idle time rose (+0.31 ± 0.12 points), failed searches rose (+0.9 ± 0.2 points) and Roam time fell (32 seeds; the same with the old table).
   - Leaving the zone walkers out (57 routes against the earlier 80, same code): idle −0.50 ± 0.17 points and held angles −0.34 ± 0.05 s a bot-minute, but Roam no-headway ends +0.015 ± 0.002 a bot-minute and standing with no route +0.11 ± 0.05 points. It is kept because a route should be scored by the lives a bot can follow.
   - **Weapon fit, left out** (does a route pay one weapon class more than another, beyond what it pays every class?). Each life's weapon 1 comes exactly from the match recordings' kits (24,011 of the 53,740 lives, 161 of the 331 matches; the rest have no recording, or one with no kits; the server logs do not say which kit a human played with). Classes as the bots' (`WeaponKind::of_item`). The mix: rifles 46 %, SMGs 26 %, sniper rifles 23 %, shotguns 4 %, pistols 1 % (top players 53 / 26 / 16 / 4 / 0). Each life out of base is put on its side's nearest route; per route and class, the class's mean over its side mean, against the route's own. Shuffling the classes among a side's lives: p 0.06 over the 32 sides. Split in two halves of matches, the routes' class fits correlate +0.02 to +0.17 (about 90 route-class pairs, three splits): a weight from them would be mostly noise, so the table has none and Roam and Cut do not weigh routes by the held weapon.
   - The whole-match tests and the real-map route and cut tests pass with the new table.

   **Sprinting on a route:** sprint out of spawn and behind the team's own lines; walk once within ~30 m of a known danger (an enemy sensed, last known or on the radar, a danger-map spot, a teammate's death). Lives that were mostly sprinting did far worse than steady movers reaching as far (K/D 0.2–1.0 against 0.6–2.3), but that is LIKELY skewed by short lives being mostly the run out of spawn, and sprinting right before a death was no commoner than before a kill. A cheap rule of thumb, not a finding.
18. **Guard the lead (battery modes).** The other side of play 16: in the last ~2 minutes, the leading side puts 1–2 bots 12–20 m from its own container, covering it, to stop a late denial. The only time bots guard their base. Not on top of it: in the saved worlds players within 12 m of their own container had K/D 0.74–0.81 of the mode's (0.46 within 6 m in timed batteries), 12–20 m off about even.
19. **Spot for the humans.** A bot that sees an enemy presses spot (bit 28, key down, 3 s cooldown), so its human teammates get what it sees: honest teamwork that humans notice. What a spot shows in 0.26g is to be checked in stage 0 before this is relied on.
20. **Grenade before the peek.** Against a held angle (play 8) or a camping spot from the danger map, throw a grenade (bit 23, held until the throw starts) and then peek or push. Rarely, so it stays a surprise, and never at an enemy no sense gave.
21. **Trade partner.** Bots leave spawn in pairs about 10 m apart. When one dies, the other gets ~3 s to trade on that spot (it was seen or heard), then breaks off (play 7). Not measured yet: the assists in the statistics can test how often humans trade.
22. **Heal behind cover.** A wounded bot out of every line of sight uses a medkit from a quick slot (bits 14–19), then goes back (play 10). Needs a kit that carries one: the lowest kit a bot spawns in is to be checked.
23. **Self-balancing difficulty.** Bots should make matches fun, not win them. The commander keeps the bots' K/D against humans: above ~1.5 over the last few minutes, their aim error and noticing delay grow a step; when humans stomp them, they shrink a step, inside set limits. It changes only aim and reaction, never what the bots know (§2).
24. **Raid.** In the battery modes, take a battery out of the enemy's container and bring it home (a swing of two) whenever their container is weakly held: some of them dead, none known by it, not in the first two minutes. Winners of 6+ player matches took batteries out about 2.6 times as often as losers, mostly in the middle and last thirds, not only at the end (play 16 drops them there instead). Once taken, a raid holds while the raider is healthy and fewer than three enemies are known within 25 m of their container (`raid::raid_candidate`): held only while the window it was taken in held, it lasted a median of 4.5 s live (2026-10-20..23; 43 of 11,387 stole), short of the 15-60 s walk there. Held, on the real-map bench (8 maps, 4 v 4 and 7 v 7, 600 s, 5 seeds) raid steals went 10 -> 30 (battery cheat on: 8 -> 23), all steals 50 -> 65, deliveries 495 -> 511, kills 2441 -> 2366.
25. **Keep with the partner.** Play 21's pairs move together: the follower keeps beside its leader (9 m, running back past 18 m) instead of roaming alone, and the leader waits up to 6 s for a follower left 25-90 m behind. Deaths with a teammate within 20 m were traded (the killer killed within 5 s) 20 % of the time, without one 3 %; winners died near a teammate more often.
26. **Cut the enemy's best way.** From the enemy side's human routes (play 17's table), weighed by lives times what a life gave: hold a side angle (off their line by 35 degrees or more, not on their side) 12-28 m from where the route comes into our half (60 % of the way from their base, out of every spawn area). Long holds only in Team Deathmatch; moved on after 1-2 kills. One bot per route. Ways through one place are one cut: of the cuts within `merge_m` (10 m) of each other, only the most used is kept, and only the `most_cuts` (4) most used are cut (built 2026-10-23, with the table's new routes many ways share a mouth).

What the paths also say (87 matches of 4+ players; `srp-worlds` tables):
- Lives that never get 30 % of the way out are the worst in every mode (K/D 0.3–0.6, almost no batteries): bots don't idle at base.
- Lives that reach the enemy third and then hold still are the longest (3–4 min) with K/D ~2: that is play 15.
- Sprinting is not what kills: players were sprinting about as often right before a death (21–24 %) as right before a kill (18–23 %).

**Never:**
- go into an enemy spawn area, or shoot into one from outside to camp it;
- aim perfectly from the first frame;
- react to anything no sense gave.

Bots never path back into their own spawn area.

What decides the winner (52 matches of 6+ players with a result, 10 of them recorded at every tick; a private study, 2026-10-14):
- With sides of equal size, being forward, crouching, sprinting, accuracy and height did not separate winners from losers; the bigger side is forward because it is bigger.
- Winners survived more and died alone less; their deaths were answered more often. At the tick level a trade needs a teammate within 20 m (play 25).
- The battery modes are won by the batteries: winners picked up more (20 of 25 matches), lost fewer on the way home (put in per pickup 1.00 against 0.67) and raided the enemy's container (play 24); more kills went with the win in only 64 % of them.
- Winners mostly won the gunfight itself (the first aimed shot won 85 % against 71 %): skill or gear, not a team pattern. Focus fire, crossfire and who saw first did not separate them. For both sides, firing back within 0.5 s and breaking the line of sight when losing paid.
- The commander keeps a slot (a battery, a post) with the bot that holds it unless another bot's claim beats it by the margin: two bots no longer swap one battery every half second.
- **The commander's bands** (alarm > objective > fight > roam; `team::Bands`): an objective first, but only a real one. An objective scoring under 0.10 (its curves all but rule it out) competes in the roam band by its score; the bot's running play keeps its band down to half that. A second bot on a container a teammate already has (a second raider, a second guard of the lead) needs 0.20. In the owner's matches of 2026-10-16, 192 of 226 raids were taken at under 0.05, over a roam at 1.00, and gave no steal; the two that scored were taken at 0.21 and 0.24. In the fight band instead (tried), a raid at 0.02 still beat every roam. Bench, 600 s, 40 seeds, 6 maps, 4 v 4 and 10 v 10: raids taken 7,271 -> 510 (6,137 of them under 0.05 before), steals by raiders 8 -> 7, all steals 33 -> 30, deliveries 9,572 -> 9,543 (±37) and kills 22,353 -> 22,371 (±212), both within the noise. Alarms are never demoted. **Behind on points** (the HUD's score gap under 0) an attack on the enemy's container (raid, sneak, late denial; `team::attack`) has no floor and no second-bot bar (`Bands::behind_floor`, `Bands::behind_second`, both 0): attacks score as products of many parts and most sit under 0.10, so with the floor the team behind in the owner's 7 v 7 matches 0x652d4e6e, 0x1a0961e2 and 0x7298fb57 (2-5 down, every battery home) took none of them and held, grouped and roamed instead; before the floor it took 25-43 % of them and came back from -3 to 0 in one. Level or ahead the floor stands. Asymmetric bench (`SRP_BENCH_START=3,7` and `7,3`, 10 batteries all home, 7 v 7, 600 s, 4 battery maps, 20 seeds): points won back by the side behind 51 -> 110 (+59 ±18), its steals 81 -> 120, its deaths on an attack 0.67 -> 0.41 a bot-minute, 44 -> 79 % of its attack time with a teammate attacking too; a floor of 0.05 (+37) or the second bar kept (+38) won back less. With batteries lying (`0,2`, `2,0`) deliveries hold (+7 ±12). The normal bench (180 s, 20 seeds) holds: delivered 2457 -> 2452, kills 3204 -> 3208.
### 3.1 Fewer plays dropped at once, supports that do something (built 2026-10-23)

The audit of three real 7 v 7 matches (2026-10-22) found plays taken and ended within the second over and over (the churn), followers standing beside their leader doing nothing, and few trades. What changed:
- **A spot play is offered only where it has a spot.** HoldExit, Crossfire and Cut looked for their spot after they were taken; with none they ended within the second, sat out the set-aside, and were taken again. Now each offers a target only where a spot is fine for the bot right now (`fine_spots`), unless it is already running there. Their spot lists are looked for once per side and point and kept `spots_keep_s` (15 s; `Tactics::spots_kept`, keyed by `Tactics::spot_key`), so the sight rays are cast once every 15 s per exit, choke or cut. A hazard on a spot is still checked at each poll.
- **Roam ends when it makes no headway.** A bot that has not got `headway_m` (3 m) further in `headway_s` (10 s) on its way ends the run, and the commander picks again. On level_06's Team Deathmatch whole teams stood at a point by their base, with an open way to the goal and no stuck note (it pushed nowhere). Only the spot plays failing at once had restarted them. Without that churn, kills there fell from 10.3 to 1.1 a run (10 v 10) until this end was added, then rose to 13.7.
- **PairUp keeps its teammate.** It is kept while he lives and stays within `mate_max_m`, instead of asking each poll for a teammate nearer the top player. Asking again ended it as soon as the bot stood a step ahead of him.
- **A Buddy near his leader covers him.** It watches where the leader does not: a known enemy off his facing (more than `front_deg`, within `threat_m`), else behind him or a side, drawn and turned now and then. It matches his stance.

Left out, with numbers (battery bench below unless stated, 16 seeds, difference a seed ± standard error):
- **A floor for fight plays** (a running fight play keeps its band): quick ends fell, but A->B->A flips rose +0.087 (beyond the noise), idle Buddy rose +0.37 % and the thief battery test broke.
- **A Buddy that steps out of his leader's line of fire**: it covered more (watching off his facing +22 points) but its moves churned the plays (flips +0.038 ± 0.012).
- **Trades (audit issue 9) are not improved.** Two tries made more Trade plays without more trades:
  - Trade taken by every mate within 25 m and kept up to 10 s by the killer: Trade plays +180, flips +0.088 (both beyond the noise), deaths traded within 5 s +1.3 ± 1.2, within 10 s +1.5 ± 1.8.
  - The partner alone, kept by the killer: Trade kills +1.9 ± 0.6, but deaths traded within 5 s −0.3 ± 0.9 and within 10 s +0.5 ± 1.2.
- **Skip a human route whose next point turned bad** (a teammate killed by it in the last 10 s). Such a route is drawn and then ends on its first step: 7,178 quick ends in 32 seeds of Team Deathmatch with the routes, now the most common. Skipping it cut those, but battery kills with the routes fell −3.6 ± 1.6 (beyond the noise); walking toward a fresh death is how bots trade.

**Measured** (2026-10-23): `real_bot_cost`, 180 s, 4 v 4 and 10 v 10.
- Battery: the 6 battery maps (levels 1-6, level 3 timed), `SRP_BENCH_BATTERIES=10`, `SRP_BENCH_USE_LINE=1`.
- Team Deathmatch: levels 1, 2, 5, 6, 7 and 8.
- "Routes" runs set `SRP_BOT_ROUTES` (the human routes, as live); the others run without them. Cut works only with routes.
- Each cell is base -> with the change; the bracket is the difference a seed ± standard error; * is beyond 2 standard errors.
- Quick ends are plays the bot ended itself within 1 s. The flips and quick-end rows are per bot-minute. Idle Buddy is the share of its alive time with a goal and no movement. "Watching off his facing" is the share of a Buddy's time within 1.4 x `close_m` of its leader spent looking more than 60 degrees off his facing.

| | Battery, 16 seeds | Battery, routes, 96 seeds | Team DM, 16 seeds | Team DM, routes, 32 seeds |
|---|---|---|---|---|
| quick ends | 0.257 -> 0.106 (−0.151 ± 0.013*) | 0.246 -> 0.117 (−0.129 ± 0.005*) | 1.486 -> 0.107 (−1.379 ± 0.011*) | 2.260 -> 0.547 (−1.713 ± 0.034*) |
| A->B->A flips | 0.472 -> 0.449 (−0.023 ± 0.016) | 0.441 -> 0.435 (−0.006 ± 0.006) | 1.470 -> 0.531 (−0.939 ± 0.022*) | 1.295 -> 0.456 (−0.838 ± 0.013*) |
| idle Buddy, % | 0.34 -> 0.18 (−0.16 ± 0.05*) | 0.18 -> 0.11 (−0.07 ± 0.01*) | 9.19 -> 8.50 (−0.69 ± 0.27*) | 7.72 -> 6.56 (−1.16 ± 0.20*) |
| idle Buddy/PairUp/CoverHuman, % | 0.37 -> 0.21 (−0.17 ± 0.06*) | 0.22 -> 0.15 (−0.07 ± 0.02*) | 9.86 -> 9.71 (−0.16 ± 0.31) | 8.32 -> 7.31 (−1.01 ± 0.21*) |
| Buddy watching off his facing, % | 36.0 -> 56.1 (+20.0 ± 3.7*) | 40.1 -> 65.0 (+24.9 ± 1.7*) | 39.0 -> 62.7 (+23.8 ± 0.7*) | 43.9 -> 66.6 (+22.7 ± 0.6*) |
| PairUp ended itself | 5.3 -> 3.8 (−1.6 ± 0.9) | 5.2 -> 3.6 (−1.5 ± 0.4*) | 77.6 -> 77.2 (−0.4 ± 5.6) | 66.5 -> 51.9 (−14.5 ± 3.6*) |
| deaths traded within 5 s, % | 11.97 -> 11.29 (−0.68 ± 1.28) | 10.89 -> 11.04 (+0.15 ± 0.41) | 18.09 -> 17.00 (−1.10 ± 1.15) | 14.72 -> 14.58 (−0.15 ± 0.54) |
| kills, team 0 | 66.4 -> 64.4 (−1.9 ± 1.9) | 63.8 -> 64.2 (+0.4 ± 0.7) | 132.3 -> 138.8 (+6.4 ± 2.7*) | 105.5 -> 108.8 (+3.3 ± 1.8) |
| kills, team 1 | 66.5 -> 69.5 (+3.0 ± 2.7) | 69.2 -> 68.3 (−1.0 ± 0.9) | 134.3 -> 141.0 (+6.8 ± 2.2*) | 105.3 -> 108.6 (+3.2 ± 1.7) |
| deaths a bot-minute | 0.272 -> 0.274 (+0.002 ± 0.008) | 0.272 -> 0.271 (−0.001 ± 0.003) | 0.562 -> 0.591 (+0.030 ± 0.009*) | 0.438 -> 0.452 (+0.014 ± 0.006*) |
| team kills | 10.8 -> 9.9 (−0.9 ± 1.3) | 10.6 -> 10.5 (−0.1 ± 0.4) | 12.6 -> 14.8 (+2.3 ± 1.5) | 4.9 -> 4.8 (−0.1 ± 0.6) |
| delivered | 38.4 -> 37.5 (−0.9 ± 0.5) | 38.2 -> 37.6 (−0.6 ± 0.3) | | |
| carriers killed | 9.8 -> 10.6 (+0.8 ± 1.1) | 11.7 -> 12.3 (+0.7 ± 0.4) | | |

- Both teams are bots, so a team's K/D stays about 1. In Team Deathmatch both sides kill more because more bots are out holding instead of restarting, and deaths per bot-minute rise with it.
- Delivered with the routes: seeds 1-16 +0.3 ± 0.6, 17-32 −2.5 ± 0.7, 33-48 −0.6 ± 0.9, 49-96 −0.2 ± 0.5. On seeds 17-32, 49-80 the spot plays alone gave −0.8 ± 0.4 and the Buddy alone −0.5 ± 0.3. It is the one guard that leans the wrong way, about 1.4 %.
- The 7 whole-match tests pass. In `a_take_waits_for_the_fight_unless_covered`, carriers killed within 3 s of a take went 52 -> 43 (cheat off) and 40 -> 34 (cheat on). In `bots_hold_fire_while_a_teammate_is_in_the_line_of_fire` (one run, friends on), team kills went 10 -> 19.
- Run notes (level_06 10 v 10, seed 3): before, HoldExit and Crossfire ended with "no spot" about every 25 s. After, none did. Some Crossfire <-> Buddy switches remain; that is what the fight floor was for.

## 4. Event reactions

| Event | How bots learn it | Reaction |
|---|---|---|
| A battery the team picked up disappears from the minimap (stolen) | HUD marker gone | An enemy has it near where it last was: that spot is the thief's last known position. The 1–2 nearest bots cut off his way to his container (play 2); his exact place is known only once seen or heard |
| An enemy steals from our container | The steal count (event 26) in a world, up to ~5 s late | Same alarm; the rest keep their posts |
| An enemy picking up at a spawner, seen | Sight; he stands still while picking up | Ambush or interrupt (play 1) |
| Our carrier killed | Kill feed and the battery's marker | The nearest bot goes for the dropped battery, others cover it |
| We killed an enemy carrier | Kill feed and sight | The nearest bot picks it up, one escorts |
| A battery delivered, either side | The bring count (event 24) | All bots re-plan |
| Several enemies dead at once | Kill feed | Push or sneak window (play 4) |
| A teammate killed at a spot | Kill feed | That spot is dangerous for ~20 s: avoid it, or go in to trade |
| An enemy on a kill streak | Statistics | Pair up against him |
| Little time left | Match clock | Losing team all in, winning team defends |

## 5. What match recordings give

- **Danger map per map:** where humans kill and die. Bots pre-aim known camping spots as they pass.
- **Real routes:** where humans approach batteries from (play 1), and which routes are busy (play 4).
- **Real timings:** spawn to each battery, pickup time, carrier speed.
- **Human numbers:** reaction time (an enemy appears, then the first turn toward him and the first shot), burst lengths, how often humans re-peek. These set the bots' parameters.
- **Grenades and pickups in use:** when humans throw (bit 23) and how long they stand at a battery (bit 21).

**How the best players fight** (10 recorded matches of 6–10 players, 2026-10-14; every input of every player at its 50 ms tick, positions replayed between the worlds, sight on the bullet mesh as the bots' own; private research, aggregate numbers only). The three best players by kills a minute, K/D and kills+assists a life (players I, B and E; 20 matches between them, 1.4 / 1.4 / 0.8 kills a minute) against the 17 other humans; quartiles, n in brackets. The bots' defaults (`tuning.rs`, `reflex.rs`) are set to the top three's numbers where a bot can do the same:

| | top three | everyone else | bots |
|---|---|---|---|
| sight to first shot | 400 / 650 / 1050 ms (262) | 450 / 750 / 1300 ms (508) | 550 ms median at skill 1, 750 at 0.5, 950 at 0 (unchanged) |
| yaw off the chest while firing | 1.5° median (7,810 ticks), ~1.0° without the replay's error | 1.8° (12,551) | 0.9° at skill 1, 1.5° at 0.5, 2.0° at 0 (a slow wander added; was 0.3–0.8°) |
| automatic bursts | 500 / 750 / 1100 ms (658) | 150 / 350 / 800 ms (1,514) | 500–1000 ms (was 250–600) |
| pauses between bursts | 200 / 400 / 750 ms (139) | 100 / 200 / 450 ms (658) | 200–650 ms (was 300–700) |
| aim below the chest during a burst (the summed turns) | 1.4° in the first 250 ms, 3.5° by 500 ms, 5.8° by 1 s (median) | 1.1 / 2.4 / 4.4° | each tick's turn takes back the kick its weapon's model predicts for that tick (`recoil.rs`, any weapon): the view stays on the aim (new; was a pull of 7° a second up to 5.5°) |
| down the sights while firing | 86 % within 10 m, 99 % beyond | 80 % / 98 % | always beyond 10 m, nearer with 85 % a target (new) |
| a side key held while firing (automatic) | 78 % (7,810) | 64 % (12,551) | side steps of 250–700 ms, a still moment of 150–450 ms after half of them (new) |
| one way of the side step, in a fight | 300 / 450 / 750 ms (920) | 250 / 450 / 750 ms (1,720) | as above; a hit turns it at once (73 % of the top three turn within 1 s) |
| standing still while shot at | 3 % (1,742 ticks) | 16 % (4,358) | side steps (bolt action excepted) |
| crouched while firing | automatic 12 %, bolt action 32 % (112) | 19 % / 28 % | a bolt action crouches with 30 % a target (new) |
| one-round weapons | press again at the weapon's cycle (SKS 200 ms at 200, TT-33 250 at 200, all players) | | at the cycle + 50 ms (was + 100), no burst pause (was one) |
| reload | 90 % before empty, half the magazine left (363) | 93 %, two thirds left (670) | tactical after 1 s with nothing seen (was 3 s) |
| sprint, share of moving time | 70 % with every enemy > 60 m, 49 % at 30–60 m, 30 % within 30 m unseen, 74 % of the first 10 s of a life | 57 / 49 / 36 %, 61 % | unchanged: run unless a known danger is within 30 m (play 17) |

**The kick, from the client's code** (2026-10-14; `bots/recoil.rs` keeps the addresses). PROVEN: a shot lifts the view itself, camera and bullet together: the view's pitch is `clamp(m_look_pitch + recoil.y, ±1)` (0x2E0F60), and the camera and the bullet take one matrix (0x1F3480 → 0x2422B0; `instant_fire` 0x2E23A0); there is no lift of the shot over a still view. A downward turn is first taken off the kick (0x2E0FB0: `c = min(−d, y)`, `y −= c`, the look pitch moves by `d + c`), so a turn moves the view by exactly `d`, and a world's look pitch is the sum of the inputs' turns only with no fire: in three recorded matches 373 of 381 firing windows have it above the sum, never by more than the downward turns (an earlier reading here, "the look pitch is the sum of the turns to 1e-5 rad over 1,275 pairs", holds for windows without fire only). Each shot adds `T (1 − max(y, 0)^p)` to the kick, reached linearly over the shot interval `60 / rpm`, `T` uniform in the weapon's config range (0x331C50); after the interval a PID controller (Kp 5, Ki 10 or 15, Kd 0) brings it back, undershooting by 19-25 %, and because the weapon ticks before the tick's shot it also runs inside a burst (0x331A50, 0x3321E0); no fixed cap; aiming down the sights and crouching change nothing. With the ranges' middles and no pull, an AK-74M burst of 12 shots lifts the view 1.8° at 0.1 s, 3.2 at 0.2, 4.1 at 0.3, 4.4 at 0.5, 5.7 at 0.7, 7.5 at 0.9 s and 7.3 at 1.1 s (the config's degrees), 1.5 at 0.2 s after the release, then 0.9-1.4 below; a bolt action 3° over its shot interval, below 0 some 0.25 s later. The bots keep that model of their own weapon (`recoil.rs`, the five bot weapons from their configs, the AK-74M as the default), aim from the view (base pitch and kick) and turn so it stays on the aim: each tick's turn takes back the rise the kick will give the view that tick (`recoil_pull` 1), the skill's error and wander on top; the wall test is on the view. The soaks' stand-in kicks the same way, each shot's kick drawn in the config's range. LIKELY: the shot cadence (60/rpm from the tick the trigger goes down). Not traced: where the input's turn falls against the tick's weapon update and shots. UNKNOWN: the pitch's unit (LIKELY 60-64° a unit; the bots take it for a radian); how far the side kick turns the view; and the recorded hits above are not reconciled with the code: the view under the chest is "the summed turns under the chest" = the kick applied less what the controller took back, which matches the no-pull curve in shape and size although the top players must pull (weapon mix, single fire, hit attribution open). The hits: 1,540 recorded hits (each victim's `m_recent_hits`, given to the enemy firing at him at that tick with him nearest the view) were landed with the attacker's summed turns under the victim's chest by more the longer the trigger was held, 1.6° after 0.1-0.2 s, 2.7° at 0.3-0.4 s, 4.2° at 0.4-0.6 s, 5.1° at 0.8-1 s, 7.9° at 1-1.4 s (medians over the one-round weapons' hits, which sit at -0.4°; 1,427 automatic hits), while their yaw stays at 1.1-1.4°; and a round of 0.5-1 s into a burst still hits 85 % as often as one of the first 0.25 s (0.34 against 0.40 hits a round). Estimated: the replay's place error (~0.3 m) adds about 0.9° at 20 m to every angle above; reaction times are on the match clock from the first tick the enemy is in the view and in sight (the client's own display delay included, as it is for what a bot shows).

The worlds the server already saves (one every ~5 s, `SRP_SNAPSHOT_DIR`) give a first danger map now: `srp-worlds` tables them (positions, statistics, batteries), and a kill is placed where the killer stood in the world after his kill count grew, if he moved under 10 m in those 5 s. Per 8 m cell: time present, kills, kills per minute present (shrunk toward the level's rate), K/D against the level's average. Matches of fewer than 4 players are left out. The tables and maps stay private (game data of real matches).

## 6. Technical needs

- **Event delay:** events reach the server with the worlds, every ~5 s. In matches with bots: a world every 1 s (~8 KB each).
- **Positions between worlds:** carried forward from keys and turning (drift on collisions). A bot's own position is carried forward the same way and corrected at each world.
- **Path distances and visibility:** from a walk grid built from the walker mesh, and a ray tree on the bullet mesh (`player_bots.md` §5). Distance maps per target (enemy base, batteries, resupply) are cheaper than repeated path searches.
