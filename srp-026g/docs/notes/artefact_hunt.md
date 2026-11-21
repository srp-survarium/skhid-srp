# Artifact Hunt

The owner (2026-11-21): "let's add artifact hunt mode from newer game versions", then, on the
scoping (0.46c2's rules and numbers unchanged, the nine stock maps first with 0.46c2's own bases and
spawn points, the name "Artifact Hunt"): "yes, port it fully".

The donor is build 0.46c2 (`mode_name` "artefact_hunt", `st_mode_07_name`). How its executable does
it, with addresses: [artefact_hunt_046c2.md](artefact_hunt_046c2.md). 0.27, 0.28 and 0.31 do not have
the mode. This note is the specification: the rules below are 0.46c2's; where 0.26g's build differs,
"Differences" says how and why.

## Rules (0.46c2's)

- **Goal:** two teams, one artefact ("Glow of the Forest") in the world at a time. A team that
  brings it to its own base scores 1. The first to 5 (`score`) wins.
- **The artefact appears** 50 to 70 s after a round starts (`vi_spawn_time` 60, `vi_spawn_variation`
  10: uniform in [50 s, 70 s)), at one of the map's spawn points, each as likely, the same one
  possibly twice in a row.
- **Nobody takes it** for 120 s (`vi_not_picked_ttl`): it is removed, and a new 50 to 70 s wait
  starts.
- **Taking it:** the use key on it, either team, with a clear line from the eye to it. No take time.
- **The carrier:**
  - 20 % slower (`movement_speed_modifier` -0.2) and health regeneration speed +0.05;
  - cannot sprint;
  - keeps his weapons: he shoots, aims and reloads as ever;
  - glows (the artefact's effect on his body);
  - is shown to both teams: teammates and enemies see his marker, the player list marks him.
- **Dropping it:** the Drop key, the carrier's death, or his leaving. It lies at his feet (0.13 m
  up). Anyone takes it the same way. Dropped and untaken for 60 s (`vi_dropped_ttl`, the whole time
  again at each drop): removed, a new 50 to 70 s wait.
- **Delivery:** the use key on one's own team's base container while carrying. No put time. The
  team scores 1.
- **Rounds** (the owner, 2026-11-21: with one base a team, the mode is played in rounds, as
  0.46c2). A delivery ends the round:
  1. 3 s (`time_before_new_round`): play goes on, nobody respawns; the hud counts "Round restarts in".
  2. If a team has 5, the match ends here.
  3. 1 s: the screen fades; every player, alive or dead, is set to respawn.
  4. 5 s (`round_countdown_time`): everyone stands at his team's spawns, frozen.
  5. The next round: the 50 to 70 s wait again.
  The scores and the match time carry over.
- **No new round** with less than 90 s of match time left (`time_needed_to_continue_match`): the
  match ends (scores differ) or goes to sudden death (equal).
- **Respawn in a round:** in waves every 15 s (`respawn_time`) counted from the match start, never
  sooner than 7 s after the death (`time_needed_to_respawn`); the player's respawn modifier applies.
- **Match time** 900 s (`match_time`). When it runs out: the higher score wins at once; equal scores:
  sudden death.
- **Sudden death,** 180 s (`draw_additional_time`): no respawns; a team with nobody left gives the
  other team 1; a delivery wins. Neither in 180 s: a draw.
- **Waiting and countdown** before the match as in every mode (`wait_player_percent` 0.6,
  `countdown_time` 20).
- **Score events:** the deliverer `put_victory_artefact_in_container` (score 10); everyone of his
  team who carried it towards the base `bring_victory_artefact` (score 50, shared by each one's part
  of the way).
- **Messages:** "Artifact has spawned", "You picked the artifact" (the picker), "Your team have
  delivered the artifact" / "The opponent team delivered the artifact", "Artifact destroyed".
- **Announcer:** seven lines (spawned; picked up by us / by the enemy; dropped by us / by the enemy;
  brought by us / by the enemy), in the game's voice language. A removal has none.
- **Minimap:** the two bases (one's own highlighted while he carries), the artefact while it lies in
  the world.
- **16 players** (`max_players`).

## Differences from 0.46c2

Each one a decision; the owner's are marked, the rest are the port's and open to his word.

- **The edge at 90 to 93 s left** (a delivery that equalises there ends 0.46c2's match as a draw with
  no sudden death, its two time tests being 3 s apart) is not kept: one test, at the delivery.
- **One base a team,** as 0.46c2 (its rule takes exactly two). The owner asked about several
  delivery points (2026-11-21) and chose one with rounds.
- **Dynamic anomalies** (0.46c2's rule 149, in its Team Deathmatch too) are not part of the mode: the
  maps keep 0.26g's anomalies.
- **`wait_time`** (0.46c2: minutes until an unfilled match is abandoned) is 0.26g's own waiting.
- **The 30 s end music** of 0.46c2: not ported (0.26g's matches have none).

## Data (the content pack's contract with the rule)

- **Maps:** the nine stock levels in their weathers, as 0.46c2 has them (`level_0N/<w>_7` or
  `<w>_ah`; its `_v1`.. variants are other layouts of the same maps and are left out).
- **A mode config a map** (`mode.mode` 13, `mode.rules [93, 91, 481, 94]`; "The exe" says why) with
  0.46c2's `mode.parameters` under their own names, and the map's mode data in world space. What
  reads each key:
  - the native time rule: `match_time`, `wait_time`, `countdown_time`, `wait_player_percent`, in the
    types a stock config has them in;
  - the native respawn rule and the new rule: `respawn_time`, an int (seconds);
  - the new rule, whole seconds as ints: `match_time`, `time_needed_to_continue_match`,
    `draw_additional_time`, `time_before_new_round`, `round_countdown_time`,
    `time_needed_to_respawn`, `vi_spawn_time`, `vi_spawn_variation`, `vi_not_picked_ttl`,
    `vi_dropped_ttl`; `score` an int, 1 to 255 (`Params::parse` names the key it refuses);
  - `mode.artefact_hunt.spawners[] {position, rotation}`: where the artefact appears (0.46c2's
    `victory_item_spawners[].spawned_item`); 1 to 64 a map;
  - `mode.artefact_hunt.bases[] {team, position, shapes[] {type, position, axis_x, axis_y, axis_z,
    half}}`: exactly two, one a team, at least a metre apart (0.46c2's
    `victory_artefact_containers`, the shapes its `collision_geometries`, in the form Research's
    `capture_zones` use: type 0 sphere, 1 box, 2 cylinder; at most 8 a base);
  - `mode.artefact_hunt.item {model, particle_idle, particle_spawn, particle_destroy, effect,
    movement_speed_modifier, health_regeneration_speed_modifier, usable_radius, drop_lift}`: the
    artefact's own values (0.46c2's `artefact.options`): four resource paths, the body effect's
    name (`artefact_wood_shine`), the two modifiers (-0.2, 0.05), its usable sphere's radius (0.3)
    and how far over the ground a dropped one lies (0.13, `collision_shape.position` y), floats;
  - `mode.sound_notifications {base_path, artefact_spawned, artefact_picked_up,
    artefact_picked_up_by_enemy, artefact_dropped, artefact_dropped_by_enemy, artefact_brought,
    artefact_brought_by_enemy}`: 0.46c2's own block, the announcer's seven sounds as
    `<base_path>/<voice>/<name>`. Each line is `[258, "artifact_hunt/<line>"]`: a sound collection
    (class 258, `sounds/collections/vo/<voice>/artifact_hunt/<line>.sound_collection`, seven voice
    folders), not a single sound (found while building patch 171; the pack ships them so).
- **A project a map:** 0.26g's own project of the map with 0.46c2's mode spawns
  (`server_objects.respawn_points`) and the two bases' box as a prop; nothing of 0.46c2's reworked
  geometry or anomalies. A round start respawns a whole team at once: a team with fewer free
  points than players spawns the rest a step (50 ms) later each, as the native rule does
  ("Respawns"); 8 points a team (16 players) spawn everyone in the same step.
- **Known difference: no "not for the first spawn" points.** 0.46c2 keeps some of its points out
  of a match's first spawn (`use_for_first_respawn` false: 18 on level 01, 25 on 07, 68 on London);
  0.26g's points have no such flag, so they are kept as every other point and a first spawn may
  take one of them (the owner, 2026-11-21).
- **Assets 0.26g lacks,** from 0.46c2: the artefact's model (`item/artefacts/
  artefact_shining_forest`), its particles (`fx/artifacts/wood_shain*`), its body effect
  (`artefact_wood_shine`), the base box (`item/storage_device/box_atlant8c`), the seven announcer
  lines in its six languages.
- **Texts** in the 11 languages of the client.

## The exe

A design, not code: the patch is written after its review. PROVEN = read in the 0.26g exe at the
address given (original VAs, image base 0x10000); LIKELY = inferred, the missing step named. Nothing
here was run under Unicorn or in a game. The rules are `crates/artefact-hunt-rule` (`Rule::step`, one
function from a state and an `Event` to the new state and its score events, total over the events;
the game's own decisions ask the state: `respawn_at`, `spawning`, `frozen`, `respawns`, `carrier`;
`cargo test -p artefact-hunt-rule`); the patch is a module `artefact_hunt` of
`client-patches/rust/patches` (rust_patches.md) that turns the world into events, answers the
game's decisions from the rule and shows the state: patch 171 ("The implementation" below
says what of this design is built, and what turned out otherwise).

### What is whose

| thing | owner | what the patch adds |
|---|---|---|
| waiting for players, the 20 s countdown, "in play" | the native time rule (93), unchanged | nothing: 0.46c2's states 0, 1 and 9 |
| the match clock past 900 s, rounds, the end | the new rule, and the match server | the hud's seconds; nothing in the client stops at 900 s |
| respawn points, the spawn and what it resets | the native respawn rule (91) | a death's time (waves), the places due in a round restart (held, then everyone), the blackboard's two flags in sudden death |
| the input in a round's countdown | the new rule's state | the input not applied |
| the artefact, the scores, sudden death, who is in | the new rule's state, in the rules blob | everything |
| the carrier's speed, sprint, glow, markers | derived from the rule's carrier each time they are asked | hooks that ask |
| kills and deaths | the native kd rule (94), unchanged | nothing |

### The rule's class and registration

- **`mode.rules [93, 91, 481, 94]`**, `mode.mode` 13 (below, "The mode's number"). 0.46c2's rule 97
  does the waiting, the countdown, the match time and the respawns itself; here the native time
  rule keeps the first two and the native respawn rule keeps the spawn, each asked by the new rule
  where 0.46c2 differs (below). The time rule stays because the pre-match is exactly 0.26g's (the
  same `wait_player_percent`, `countdown_time`, warmup camera and kit choice of patch 31), and
  because its state 3 is what every 0.26g "in play" test reads (core+18FE4h: the batteries, patch
  31, Research, Freeze Tag). The respawn rule stays because 0.26g's spawn (points, protection, the
  kit's load, patches 18, 20, 31, 85 and 142 on it) is that rule's tick: replacing it would mean
  every one of those again.
- **Class 481 (0x1E1).** Research's precedent (client_patch.md 3.50): a cook is a static object
  `{+0 vtable, +8 class, +10h -1, +14h thread id, +18h 8, +20h the game}` given to `register_cook`
  0x102D10 (EDI; `cooks[class] = cook` in a table of 0x206). LIKELY free: a scan of the `mov [abs],
  class` before 90 of the 122 `register_cook` calls finds no 0x1E1 (the other 32 are encoded
  otherwise and were not decoded), no tree takes it (Research 0x1E0; Freeze Tag's worktree registers
  no cook). The run-time guard of Research stays: with `cooks[481] != 0` the cook is not registered
  and the mode does not load.
- **Where:** a lohook at the start of `game::query_base_resources` 0x1EC170 (`55 8B EC 83 E4 F8`,
  6 bytes, EAX the game; PROVEN one caller, 0x1E9B4E, the game's constructor after its own cooks, and
  no branch into the span). Research's registration replaces the call before it (0x1E9B4C) and
  tail-jumps here: the two do not meet.
- **The cook:** a run-time copy of the client tdm cook's vtable 0x9FD8AC (11 slots) with slot 9
  `create(query)` `ret 4` and slot 10 the size ours (rust_patches.md `[[vtable]]`). Create allocates
  as tdm's (0x2063F0), runs `engine_match_rule_base` 0x313900 (EAX this, the type 6: free, the code
  compares it with 0 and 1 only), reads the config ("Data") into `Params` and seeds the rule with
  the match's seed, `match_options.seed` (+577B0h of `[query]`, the options (3.50); the static
  info's "match id", static_info.rs: "the seed of the match's anomalies, victory items and player
  objects", the same in every game of the match and a replay's, as 0.46c2 seeds its rule from the
  match's static data, 0x783ADE; LIKELY: that the create's `[query]` is that same object was read
  for tdm's constructor, not for the seed's offset in it). A config
  `Params::parse` refuses stops the game with the key it names (a crash report over a silently
  wrong match; the pack's own check keeps such a config from shipping).
- **The rule:** a run-time copy of tdm's vtable 0xA15DA4 (20 slots), these ours, each `ret` as tdm's
  (PROVEN in 3.50 and re-read):

  | slot | function | ret | ours |
  |---|---|---|---|
  | 7 | `on_event(type, data, time)` | 0Ch | 0: `Start`; 1 `{killer, victim}`: `Died` |
  | 8 / 9 | `entered` / `left(place, time)` | 8 | `Entered` / `Left` |
  | 11 | `set_game_world_core(core)` | 4 | keeps the core, puts the tick delegate in list 1 |
  | 15 | `serialize(writer)` | 4 | `Rule::fields`, a named write a field |
  | 16 | `deserialize(reader, dword)` | 8 | `Rule::read`, then publish |
  | 17 | `fill_match_hud_state(hud, time)` | 8 | the scores and the clock's seconds |
  | 18 | `rule_completed(time)` | 4 | `Rule::completed` |

  Slot 17's second argument is the game's time (PROVEN: 0x2D36D7 pushes it, the time rule's 0x267B22
  subtracts from it; research_mode.md had it as "?").
- **The tick:** a delegate `{object, function, previous, next, order, enabled}` at rule+120h, at the
  end of the world's list 1 (core+18850h), order from `[core+18FB0h]`, as the time rule's 0x318050
  and Research's; called `(dt, time, on_grid)` `ret 0Ch`. It sends `Tick { now: time }`, then, in
  play, sets the blackboard's two flags to `Rule::respawns()` ("Respawns").
- **The order inside one step** (PROVEN, client_patch.md 3.29 and 0x2DA9A0): list 0 the respawn
  rule's tick, then the places; list 1 the time rule, then ours; list 2 each player's node in roster
  place order, which ticks the player and applies his inputs (0x2DA9D4..0x2DAA1E). So a take, a drop
  or a death of step T comes after the rule's tick of T, and the respawn rule's tick of T comes
  before it.
- **Nothing changes the rule between the respawn rule's tick and its own** (PROVEN): the rule's
  events come from `rules_on_event` 0x2D1EA0 (three callers: the death in `hit_impl` 0x2D9EF7, list
  2; the time rule's countdown end 0x318178, list 1 before ours; its deserialize 0x31803E, a load)
  and from `on_player_entered_match` / `on_player_left_match` (0x2D2040, called by `tick_events`
  0x2D21BE, which runs apart from `atomic_tick`, 0x2D3015 / 0x2D3595 against 0x2D22E2). So the
  respawn rule's tick of T sees the rule as its own tick of T will, but for a `Start` (which only
  leaves `Idle`): what `Rule::spawning(T)` says there and what the rule's tick of T does are one
  test (the crate's `random_matches_keep_the_invariants` checks that "everyone" is exactly the
  step the countdown begins).
- **An event's time is the step before's** (PROVEN: `rules_on_event` 0x2D1EA0 pushes
  `[core+18FC0h]`, which `atomic_tick` writes at its end, 0x2D1E84). The rule's deadlines are each
  event's own time plus a duration, never "the last tick's": the crate's test
  `an_event_may_be_a_step_older_than_the_last_tick`. Steps are not only the 50 ms grid
  (`tick_events` 0x2D21E0 also steps at an enter or leave): nothing in the rule assumes 50 ms.

### The match's start, its time, its rounds and its end

- **Start.** The time rule ends its countdown at 0x31813C..0x318178: state 3, its `+12Ch := start +
  countdown`, then `rules_on_event(0, null)`. Our slot 7 sends `Start { now: [time rule+12Ch] }`
  (the time rule is the rules vector's entry of type 2 at +10Ch): the match time and the respawn
  waves are counted from the same moment as the native clock, which is saved (snapshot_rules.md
  2.2). A world's load sends event 0 too, when the loaded state is 3 and another than the one before
  (0x31802B..0x31803E), before our slot 16: `Start` outside `Idle` changes nothing, and after it
  the loaded state replaces whatever stood. The first round has no countdown of its own: 0.46c2
  goes from its match countdown (state 1) to the wait (state 2) too.
- **900 s.** PROVEN that nothing in the client changes then: `logic_tick` 0x318100 has branches for
  states 1 and 2 only, and `rule_completed` (0x318360, true in state 3 after `match_time`) is read by
  nothing in the client (modes_maps.md 11, correction). So the rule goes on by itself: sudden death
  and rounds past 900 s need no native change.
- **A round** is `Round::Hunt` (0.46c2's states 2 to 5); a delivery ends it: `Round::Brought` 3 s
  (state 6), `Round::Fade` 1 s (state 7), `Round::Countdown` 5 s (state 8), then a new hunt with the
  50 to 70 s wait. Each state's time counts from the step that entered it, as 0.46c2's state
  machine (its `initialize` keeps the start, 0x7B0BA0; one transition a step).
- **What follows a delivery** (`After`): decided once, at the delivery (the specification's one
  departure: 0.46c2 tests the time again 3 s later, 0x785CF0): a team at `score`, or the scores
  differ with less than `time_needed_to_continue_match` left: the match ends when the 3 s end
  (reason collected, or time out when the match time ran out by then: the hud's end text is
  `st_artefact_hunt_won_collected` / `_lost_collected`, `_won_time_out` / `_lost_time_out` by the
  viewer's team, `st_artefact_hunt_draw` for equal scores);
  equal scores with less left: a new round in sudden death, its 180 s counted from the delivery +
  `time_before_new_round` + `round_countdown_time` (0x78519B) and on from the fade (0x7B1004);
  otherwise a new round. It is derived from the state (the delivery is `Brought.until` less 3 s, the
  scores do not change in the 3 s), so the wire has no byte for it.
- **The match time's test** ("match over", 0x785E10) is the hunt's only, as 0.46c2's states 2 to 5:
  a round's restart that crosses 900 s ends the match or starts sudden death at the next round's
  first step (only with `time_needed_to_continue_match` under the 9 s a restart takes; 90 s ships).
- **The end is the server's**, as in every mode: for srp this mode ends when the rule's stage in an
  uploaded world is `Over` (its `at`; the result `Rule::leader`: team 0, team 1, draw), never at the
  time rule's 900 s. That is the srp worker's part; the contract is "Wire" below.
- **The native clock.** The client time rule's fill 0x267B10 writes hud+4 = whole seconds left, 0
  once over, which update_ui shows as `root.set_match_time` ("" for 0, 0x236A23). Our slot 17 runs
  after it (`fill_match_hud_state` 0x2D36D0..0x2D36E0 calls the rules in the vector's order) and
  writes hud+4 = `Hud::clock_ms / 1000`: the same number before 900 s, sudden death's own after.
  What `[core+18F54h]` vtable+1Ch, called after the rules, writes into the hud state was not read:
  LIKELY not +4, since the native time rule's +4, written before it, is what the hud shows.

### Respawns

The native respawn rule (91) keeps its points and its spawn; four things are asked of the rule.

- **Waves.** PROVEN, the respawn rule's death handler 0x316AA0: for a victim whose team's blackboard
  entry is true (0x316AFA) the delay is `D = floor(apply_modifier(kit, 60, respawn_time ms, 1.0) x
  [rule+118h] + 0.5)` (0x316B4B..0x316B66; parameter 60 is multiplicative: `respawn_time x max(0, 1
  + S)`), then `time + D` is put on the simulation's grid (`[core+18FBCh] + 50 k`, at or after it,
  0x316B6B..0x316B9D) and stored at rule+138h + 4 x victim (0x316BA3). The patch: a lohook
  `respawn.death_delay` at 0x316B6B (`8B 8F 34 01 00 00`, `mov ecx,[edi+134h]`, 6 bytes, the
  return address of the floor call; EAX = D, EDI the rule, EBX the victim's player, `[ESP+20h]` the
  time). In this mode, with `Rule::respawn_at(params, time, Shift::new(D - [edi+114h]))` =
  `Some(at)`, it sets `EAX = at - time`; the native grid and store follow. `D - respawn_time` is
  the modifier's part of a wave as 0.46c2 rounds it (0x78562C), so with `respawn_time` 15 the time
  is 0.46c2's: the first multiple of 15 s from the match's start at or after death + 7 s, moved by
  the modifier, never under the 7 s. `None` (before the start, after the end) leaves the native
  delay.
  - The tick spawns a place whose time **equals** the step's time (0x3168BA `cmp [edx],ecx / jnz`):
    a time must be a grid time, which is why the native rounding is kept and the rule's time is
    never stored directly.
  - `mode.parameters.respawn_time` is read by both rules and must be an int (the respawn rule's
    constructor: `mov ecx,[eax] / imul ecx,3E8h`, 0x316783).
- **The round's restart: held, then everyone** (0.46c2: respawns off in states 6 and 7, rule+0x220,
  0x7B0F70; 0x785C40 marks every player at the fade; state 8 turns respawns on and the tick spawns
  them, the living too, 0x785440..0x78547B). PROVEN, the respawn rule's tick 0x316820: for each place
  0..19 with a current player (`active_player_ptr` 0x2D0AB0) whose time (+138h + 4 x place) equals
  the step's (`[ebp+0Ch]`) it goes to its team's list (0x3168BA..0x3168EC); the team's points are
  chosen (slot +54h, `select_respawn_points` 0x3171D0: the free ones, its own random state); each
  chosen place's player, when in the world (+33Fh), is taken out (vtable+24h with 1, 0x2DA790:
  `remove_alive` 0x2D99C0 for a living one, which clears `m_is_alive` and counts nothing) and
  spawned (slot +5Ch) (0x31697F..0x3169BE); a place left without a point waits 50 ms more (0x3169D8:
  `add [times + 4 x place], 32h`). A time of -1 is "none": the death handler never writes it, the
  hud shows none (0x316C35), the world does not save it (snapshot_rules.md 2.3). The patch: a lohook
  at the tick's 0x316826 (`53 56 8B D9 57`, 5 bytes, after patch 31's `jmp kit_change` at 0x316820
  and its prologue; ECX the rule, `[EBP+0Ch]` the step's time) asks `Rule::spawning(time)`:
  - `Held` (the 3 s and the fade): every place whose time is the step's gets -1; a death in them is
    given its wave by the delay above and is held the same when it falls due;
  - `Everyone` (the step that ends the fade): every place 0..19 with a current player gets the
    step's time, so this very tick takes out and spawns them all, alive or dead, at their team's
    points; one without a free point the next step (the native +50 ms), inside the countdown;
  - `Native` otherwise.
  The site is `respawn.tick` (`survarium::respawn_tick_frame`), one row shared with Freeze Tag:
  its handler is order 0, this one order 1, both before the native compare of the places' times.
- **What a round start resets** is what a respawn after a death resets, for everyone, and nothing
  else (PROVEN, `base_player::initialize` 0x2DAC00, the spawn's): alive, no pending death, the
  input zero (+32Ch, +330h, 0x2DAC8D..0x2DAC99), the damage model (`damage_model::reset` 0x2E5290:
  health), the effects (`game_effect_player::stop` 0x2DE7F0), the kit's load from the profile
  (`inventory::setup_from_profile` 0x2FCB70; the weapons' spawn state, patches 18 and 20), the
  place's next kit (one picked on the death screen is played), a team point with its protection.
  Not reset: what lies in the world (thrown grenades in flight, set traps, a gun of patch 85 or an
  artefact of patch 78 on the ground), the anomalies, the kills and deaths, the statistics. 0.46c2's
  state 8 also empties a world list (0x799860) and visits every player's items (0x7B1200..0x7B1350);
  their effect was not read: LIKELY it removes thrown and placed things. The port does not: each
  kind of world object would need its own removal through its owner's code and its world record,
  out of proportion before the effect is known; proposed as a follow-up once 0.46c2's list is
  read. The server's count of a player's lives (`session::v026g::autobuy::used`: deaths and kit
  changes before play) must count a round start as a life for every player in the match (the srp
  worker's part).
- **No respawns in sudden death: the game's own flags.** The blackboard (core+188B0h) has two bools,
  "team 1 / team 2 may respawn", true from its constructor, saved as the rules blob's last two
  bytes. PROVEN: they are read only by that death handler (0x316AFA; false: no time is set) and by
  the blackboard's serialize (the getter 0x317700 has no other caller); Team Deathmatch clears one
  through `blackboard::set<bool>` 0x316630 at a team's last ticket (0x3161C8). The rule's tick sets
  both to `Rule::respawns()` in play (false exactly while sudden death is on: from the fade of a
  round in sudden death, or the hunt's step the match time ran out). So a death in sudden death
  gets no time, and a respawn scheduled before it began still comes (0.46c2's 0x785704); the round
  start of a round in sudden death spawns everyone all the same (`Everyone` writes the times
  itself). No new byte in a world: the flags are there, and a load restores them with the rule.
- **A player who enters** spawns at once in any phase (PROVEN `on_player_entered_match` 0x316BC0:
  `times[place] = time`, no flag test), as 0.46c2's 0x7842A0, but in the 3 s and the fade, where his
  time is held to the round start (0.46c2's respawns off there too); in sudden death he is in
  (`Entered`). The other hooks of the respawn rule (kit change's at 0x316820, Free for All's
  `respawn.*`) are at other spans; Free for All's act on teams of 2 and more only.

### The round's countdown: frozen

- **0.46c2** zeroes every player's input command while its state has flag 4 (states 0, 1, 8;
  0x7B0BA0 writes the player slot's +3Ah, the player's tick clears the input at 0x72C1CE).
  0.26g's own pre-match does it otherwise: the client builds no movement input in the warmup
  camera's mode 3 (`process_warmup_mode` 0x242200, client_patch.md 3.29); nothing in the simulation
  reads the time rule's state but the batteries. That is no way for a round: the players are in
  play, the camera is theirs, and a client that builds an input anyway would move.
- **The patch drops every input in the simulation** while `Rule::frozen()` (`Round::Countdown`).
  PROVEN, `base_player::apply_new_input` 0x2D86E0 (each player's node, at his input's time): an
  input the same as the stored one (+330h the mask, +32Ch the rotation's two halves) or of a dead
  player leaves at once (0x2D86F5, 0x2D8741 to the common epilogue 0x2D8A06) and nothing of the
  player changes; so an input not applied is the native "nothing new". The site is patch 78's
  `carry.input` (0x2D86E0, the entry, before its early returns), whose handler already asks this
  patch first (below, the use key): it gains the exit `held@0x2D8A0C`, the function's own `ret 8`
  (0x2D8A09 `mov esp,ebp / pop ebp` gives back the entry's stack, so the entry's stack is what that
  `ret 8` expects). Freeze Tag holds a frozen body's input at the same entry with the same exit
  (its handler order 0, patch 78's order 1), so a frozen player's Use takes no artefact.
- **Why the player stands still:** the round start's spawn zeroes the stored input (0x2DAC8D,
  0x2DAC99), and none is applied until the countdown ends: no move, no turn, no fire, no use, no
  drop. Gravity, the animations and the player's own tick go on (his tick is not stopped, unlike
  Freeze Tag's frozen body). A player who enters in the countdown spawns into it and stands too.
- **A key held at the countdown's end** acts at the client's next input change (a mouse move, a key
  pressed or let go): a client sends an input only when it differs from the last it sent (0x1F64C0:
  the mask +33Ch against +344h, or a turn +338h; recorded as sent at 0x1F793E..0x1F7944 even while
  dead or not sent). The same holds at every 0.26g respawn (the spawn zeroes the input, the client
  sent W long before), so it is left as the game's. A one-line nudge is possible (the local input
  handler's +344h made unequal to +33Ch at the hud frame the countdown ends, so that frame sends);
  not proposed unless the owner wants the held key to act at once.
- **The camera** stays the player's first person at his spawn: what switches to the warmup camera
  follows the time rule's state, which stays 3 (the client time rule's 0x27B1A0 and patch 31's
  `kit_view` at 0x26794D, client_patch.md 3.29). The screen's fade and the
  countdown's "Awaiting match start" are the hud movie's, from the update call ("HUD calls").

### The artefact in the world

- **It lies still.** 0.46c2's is a rigid body (mass 10) that may roll. In 0.26g the physics section
  of a world is empty: a dynamic body is saved by its owner (the battery writes two transforms, two
  velocities, a deactivation time and an activation state, 160 bytes, 0x301470), no patch makes a
  body, and the battery's own falls through one-sided floors (client_patch.md 3.23). A patch-made
  body would need its constructor and shape, the world's add and remove, those 160 bytes with their
  parse, and a proof that a game that loaded a world steps the same bits as one that did not
  (Bullet's contact cache is not saved). Out of proportion; proposed instead:
  - a spawned artefact is at its spawner's position and rotation;
  - a dropped one is upright, `drop_lift` (0.13 m) over the ground under the carrier: a ray straight
    down from his feet (player+43Ch) + 0.23 m, 200 m long, `engine_ray_test` 0x75E20 with filter
    24h / 2 (patch 78's `drop_carried`), the level's collision, the same in every game. With no
    ground below (out of the level) it is at his feet + 0.13 m, and the 60 s remove it.
  - What this changes for players: it never rolls off a slope or a ledge, nothing pushes it, and a
    carrier killed in the air leaves it on the ground below at once.
- **The native battery class is not reused.** `victory_item_core` and its container bring a take and
  put state machine with animations, the battery as the held object (no weapon), the gather rule's
  ownership, the statistics' battery records and the world's battery parts (patch 92's too): more to
  switch off than the artefact needs. Nor is a native artefact item (patch 78's ground artefacts):
  it is an inventory item with its world record and its skills.
- **The use key, the Drop key and the freeze: one site** (PROVEN). Use is action 35h, input bit 21
  (200000h), Drop action 36h, bit 22 (400000h; the input builder 0x2420F1..0x242118: the game's own
  `drop` key, default V, `st_drop_action` in the controls list, rebindable, the same bit as
  0.46c2's). Each player's node calls `apply_new_input` 0x2D86E0 at his input's time. Patch 78
  takes the press there before the game (`carry.input`, the function's entry: `artefact_carry::
  on_input` sees the raw new mask against the applied one, +330h) and `consume_use` keeps the
  native scan from seeing it; patch 85 rides the same handler. Artifact Hunt is asked first there,
  through one `pub fn` (`artefact_hunt::input(player, now, mask) -> Held | Used | Game`):
  - frozen: `Held`, the handler exits at `held@0x2D8A0C` (above);
  - Use pressed now (bit 21 set in the raw mask, clear in the last one patch 78's machine kept) by a living player in the world (+33Ch, +33Fh)
    whose view takes the artefact or his own base (below): `Take` or `Deliver`, then `Used`
    (`consume_use`: no take of patch 78, no pickup of patch 85, no native use of the same press);
  - Drop pressed now (bit 22, the same) by the rule's carrier: `Drop` with the ground
    under him; the input goes on (`Game`): its only native reader is `process_quick_slots`'s
    battery drop at 0x2D8E73, which needs a carried battery (`[inventory+178h]`), none in this mode;
  - else `Game`.
  So the Drop key needs no new input bit, no binding and no change to `srp-validate`'s input check
  or patch 99 (they refuse bit 30 and a bad rotation only); the item hotkeys patch's new bits are
  not needed. An edge, not the battery's level: a player who held V while taking it drops it only
  at his next press. The edges are against patch 78's last raw mask (its machine, saved with the
  player), not +330h: +330h is the filtered mask, Use cleared after a consumed press, so a held Use
  would read as pressed again at every input. A frozen countdown does not update that machine.
- **A take:** `Take { place }` when the artefact lies, his view ray (the eye matrix player+44Ch:
  position m[12..14], forward m[8..10]) enters the artefact's sphere (`usable_radius` 0.3 m about
  its position) within 2.0 m (`usable_objects_detection_distance`, 0xA710D8, the native use reach),
  and the line is clear: `engine_ray_test` from the eye to the sphere's entry point with filter
  24h / 2 meets nothing. No team test, no take time, as 0.46c2's `can_use` 0x797E00. Two presses of
  one step: the lower roster place's is applied first, and the rule makes the second nothing.
- **A delivery:** `Deliver { place }` when he is the rule's carrier, the same view ray enters a shape
  of his own team's base within 2.0 m, and no level collision is met more than 0.15 m before that
  entry (the base's box prop has collision of its own at the shape's surface; the margin is a
  proposal, to be set against the pack's props). No put time, as 0.46c2's 0x7B0B70.
  - The ray-against-shape tests (sphere, box, cylinder; single precision, no fused operations, as
    research-rule's `inside`) are pure: they are to be added to the crate with their tests when the
    patch is written. They are not in it yet.
- **Why every game computes the same:** the eye is the simulation's (`computed_head_transform`
  0x2DA4B0, recomputed at each physics assignment, at the spawn and in deserialize 0x2DB8CC), the
  artefact and the bases are the rule's and the config's, the level's collision is the project's.
  Contrast patch 85, whose gun lies where each client's ragdoll put it (so its pickup travels as a
  command in the input).
- **The prompt:** the same two tests each frame for the viewed player in `artefact_carry::on_hud`
  (the hook at `player_icons.update_start`), first in its order: `root.show_artefact_offer(
  "st_pickup_item")` or `("st_put_item")`. Both keys are the exe's own (0xA13A24, 0xA16C70); that
  every dictionary has their texts was not checked.
- **Drawing it.** The native precedent is an anomaly's artefact: a model (class 15) added with
  `scene_renderer::add_model` 0x21CD0 (`engine_add_model`) and a particle (class 61) played with
  `scene_renderer::play_particle_system` 0x22660 and removed with `remove_particle_system_instance`
  0x229D0 (the Spring client 0x25EBB0; the object particle visual's insert 0x292CC0 and remove
  0x292DC0, `engine_particle_insert` / `engine_particle_remove`, call the same two: 0x292D26,
  0x292DE7; the re-kit's inferred name of 0x22660, `particle_world::~particle_world`, is wrong). The
  patch:
  - asks four resources once a match (the model, the idle, spawn and destroy particles, paths from
    `mode.artefact_hunt.item`) by a query of its own with the run-time callback
    `resources.query_done` (patches 88 and 111 do the same);
  - keeps a view (a `GameCell`) reconciled once a frame at `game_world.after_move` (0x1F7C3C) from
    the rule's state: lying at P with the resources there: the model at P, the idle particle
    playing; otherwise both removed; the spawn and destroy particles once at the view's own changes
    (nothing to a spawner's artefact; a lying one to nothing by its time);
  - forgets the view at the cook's create and removes it at `game_world.deactivate` (0x1F9520).
  A rollback or a load only changes what the next frame reconciles to: nothing is drawn twice or
  left. While it is carried, and in a round's restart, nothing is drawn in the world.
  - New `engine_026g.tsv` rows: `play_particle_system`, `remove_particle_system_instance` (with the
    Spring's arguments: the scene renderer `[[[game+9Ch]+5360h]+200064h]`, the scene, the particle
    by value, the transform, the instance out). LIKELY, not read: that a class-61 result of a
    patch's own query is what `play_particle_system` takes.

### The carrier

All of it is asked of the rule (`Rule::carrier`) where the game asks, so a load, a rollback and a
late join need nothing, and a world gains no byte.

- **The two modifiers.** PROVEN: `apply_modifier` 0x2D5AE0 (EAX the kit's container: profile
  `[player+6B8h]` + kit `[player+115h]` x 704h + 0F4h; ESI the index) sums the kit's float and the
  index's list; index 4 is `movement_speed_modifier`, 2 `health_regeneration_speed_modifier` (the
  name table 0x928448), both multiplicative: B x max(0, 1 + S). The battery slows its carrier by
  linking a node (item+1B4h, index 4) into his kit's list (0x300C24..0x300C8B) and unlinking it; the
  lists are pointers of the match profile, in no world. Patch 109 already adds derived terms in the
  getter (`artefact.shared_modifier` 0x2D5AE6, `artefact.shared_modifiers` 0x2D5C18, filtered by
  `SPRING_LISTS`). Its two handlers call a `pub fn` of this patch: for the container of the rule's
  carrier's current kit, index 4 gets `movement_speed_modifier` (-0.2) and index 2
  `health_regeneration_speed_modifier` (+0.05); `SPRING_LISTS` gains index 2.
  - 0.26g has no `victory_item_speed_modifier` (0.46c2's parameter 105; none of the 68 names): the
    slow is the flat -0.2.
  - At each change of the carrier the patch sets that player's `m_need_to_select_animations`
    (+33Eh, a simulation byte that is saved): the speed is asked again only at a re-selection, as
    patches 94 and 109 found.
  - A rule-owned node in the native list was rejected: it would have to be unlinked and linked again
    at every load and at the rule's end: the same number held twice.
- **No sprint.** PROVEN: `base_player::can_sprint` 0x2DBF40 has no carried-item test (a battery
  carrier is refused by his held item's slot +60h); every refusal jumps to 0x2DC008. A lohook
  `artefact_hunt.can_sprint` at 0x2DBF45 (`8B 83 08 01 00 00`, 6 bytes, EBX the player) exits there
  for the rule's carrier. LIKELY: a sprinting player who takes the artefact leaves the sprint at the
  re-selection above (the mechanism the battery relies on; the logic states' tick was not traced).
- **He keeps his weapons:** nothing of his inventory is touched; the native `m_carried_item`
  (inventory +178h) stays 0.
- **The glow** as Forest Protection's (client_patch.md 3.73, `forest_glow.rs`): the pack's effect
  `artefact_wood_shine` (a class-109 resource asked once) played on the player's effects
  (`game_effect_player::add` 0x2DE090) in mode 2, the client stream: kept in the local history, so a
  rollback restores it, and not uploaded (`serializable_effects_count` does not count it). In the
  player's tick (`battery.carrier_tick` 0x2DAAD3, a chain): a living carrier who plays none gets
  one; `medical.effect_clock` ends it when he carries no more. First and third person are the
  effect's own (a composite with a `view` 0 twin, the pack's).
- **His marker for both teams** (PROVEN sites, patch 135's):
  - head marker: `player::render_name` shows an enemy only while spotted (0x1F2B50, site
    `player_names.enemy_spotted`, exit `shown`): the carrier is shown always. The icon's
    `modifier_visible` (record +388h, from `[inventory+178h] != 0` at 0x1F2D8B) is the native
    carrier mark: a lohook at 0x1F2D8B (`83 BA 78 01 00 00 00`, 7 bytes) sets it for the carrier.
    The icons movie hides enemy ids unless told: one new call, `root.srp_show_carriers(true)`, at
    `player_icons.enemy_ids` 0x235709 (a `flash/player_icons` change);
  - minimap: `update_minimap_players` 0x239420 lists an enemy only while spotted (0x239505, site
    `minimap.enemy_spotted`): the carrier always; its `is_carrying_item` (0x239521) is left alone,
    the hud movie marks the dot of the update call's carrier place;
  - the player list: 0.26g's `root.list_update_player` has no carry member; the hud movie marks the
    row of the update call's carrier place.
- **His death and leaving** are the rule's own slots. PROVEN: event 1 is sent from `hit_impl` at
  0x2D9ED3..0x2D9EF7 when the hit set `pending_death`, the victim still alive and in the world (his
  `on_player_death` runs at his next tick), so his feet are there for the drop; it is the only
  sender of type 1, and the respawn rule schedules only on it. `on_player_left_match` 0x2D2040 calls
  slot 9 while the place still holds its player. `Left` is given the ground only for the rule's
  carrier. A living player taken out by a round start is no death (`remove_alive` counts nothing)
  and carries nothing then (the delivery ended the carry). LIKELY, not read: that every lethal
  source goes through `hit_impl` (a suicide does).

### Where the port departs from 0.46c2

Beside "Differences from 0.46c2" above (the owner's and the specification's), what this design
does otherwise, each open to the owner's word:

- **The artefact lies still** and a drop goes to the ground below at once (above): no rolling, no
  pushing.
- **The take's reach** is 0.26g's own use reach, 2.0 m along the view into the artefact's 0.3 m
  sphere (0.46c2's generic use flow was not traced), and its clear line is against the level's
  collision: a player's body between does not block it.
- **A delivery** needs the view on the base's shape within 2.0 m, as 0.26g's usable objects do;
  0.46c2's container asks only the team and the artefact (its use flow, again, not traced).
- **A round start resets the players only** (a respawn's resets, above); 0.46c2 also empties a world
  list, LIKELY of thrown and placed things: in the port they stay.
- **The round start's points** are the native respawn rule's choice (its free points, its own random
  state); 0.46c2 shuffles with the rule's rng and orders by nearby enemies (0x7858C0). So the rule's
  rng here draws only the waits and the spawners: 0.46c2's sequence of draws differs once a respawn
  has drawn from it.
- **The countdown drops the inputs** where 0.46c2 zeroes them each tick: the same standing still; a
  key held at its end acts at the next input change, as at every 0.26g respawn. The camera is the
  player's own (0.46c2's was not read).
- **Sudden death's fallen** count from the round's countdown (and from the hunt's step it began in);
  deaths in the fade do not count, since everyone respawns at its end. When 0.46c2 refills its
  "players still in" (+0x380) was not read.
- **In sudden death a delivery ends the match at once.** 0.46c2's too, by its code ("match over"
  0x785E10 is its state machine's first test at the next tick, before "brought"), though its note
  says "after the 3 s".
- **A team with nobody in the match when sudden death starts** (the hunt's step, or the fade) gives
  its point then, by the same rule as a wipe; both teams empty: team 1 gets it (0.46c2's order of
  the two tests, 0x785F80).
- **A death in the step sudden death starts in** is given its wave if it came before the rule's
  tick; 0.46c2 computes the time a tick later and gives none, and still counts the player as in.
- **The way's shares** are counted on integers (millimetres, 65536ths of the way between the
  bases), a place and not a place's kit; a share can differ from 0.46c2's floats by a hundredth. A
  player who left has no share (0.46c2 keeps awarding the empty place); the others share the whole.
  0.46c2's `bring_victory_artefact_singly` (weight 0) is not counted.
- **The score events** are 0.26g's numbers 20 (put) and 6 (bring, hundredths), with this mode's
  weights 10 and 50 from srp's static info as Research's; 0.46c2's are 16 and 35.
- **The respawn modifier** is 0.26g's parameter 60 as its respawn rule rounds it (0.46c2: 59).
- **The health regeneration modifier** follows 0.26g's rule for its index 2 (x 1.05 a body part).
  0.46c2 "adds 0.05" to its parameter 1; whether that is the same relative rule was not read.
- **The slow is a flat 20 %:** 0.26g has no `victory_item_speed_modifier` to lessen it.
- **The Drop key acts on its press** (0.46c2's level or edge was not read); its default is 0.26g's V.
- **The announcer speaks the text's language** (0.26g has no voice language), english where the
  pack has no folder for it.
- **The end's text** ("Time is out" / "All artifacts collected") is fixed when the match ends;
  0.46c2 asks its clock again each time it shows it.
- **The hud's alive counts** in sudden death are of the moment; 0.46c2's are of the last death or
  leave (an entering player is not counted until then).
- **A delivered message's enemy colour** (0.46c2's third argument) needs the hud movie's own call
  if it is wanted.

### Not proven

- Class 0x1E1 against the 32 cook registrations the scan did not decode (the run-time guard covers it).
- What `[core+18F54h]` vtable+1Ch writes into the hud state after the rules' fills.
- That every lethal source sends event 1 through `hit_impl` (a suicide: yes; falls, anomalies and
  bleeding: inferred from the respawn rule's depending on it).
- Which objects besides the level a ray of filter 24h / 2 meets (character controllers' group is a
  field; grenade and trap bodies; a client's ragdoll would make games differ). Patch 78 relies on
  the same filter in the simulation and was played.
- The base props' collision against the bases' shapes (the 0.15 m margin), until the pack has them.
- A class-61 (particle) result of a patch's own query, whether the ported particles loop, and
  whether a one-shot needs a remove.
- A one-shot hud sound from patch code and stopping it; the single-sound cook on the pack's paths.
- That a re-selection ends a running sprint in the same step.
- A Number (type 5) `Invoke` other than Research's shipped sender.
- That the lohook framework takes an exit at a function's entry (`carry.input`'s `held`) with the
  entry's stack: the exits so far are mid-function; the bytes say `ret 8` there is right.
- 0.46c2: its use key's reach and flow, its Drop key's default and edge, what its `set_last_players`
  totals count, its text keys beyond the two the note names, when it refills its sudden death's
  "players still in", what its round start's world list and item visitor remove, and its
  countdown's camera.
- Everything here in a game: no part of the design was played.

### The implementation (patch 171)

The code is `client_patch.md` "Artifact Hunt (patch 171)"; checked by `check_artefact_hunt_026g.py`
(Unicorn, through the game's own callers) and the crate's tests. Not run in a game.

- **Built:** the cook and its registration, the rule and its slots 7, 8, 9, 11, 15, 16, 17, 18, the tick,
  the respawn rule's two sites, the input (freeze, take, delivery, Drop), the carrier's two modifiers,
  no sprint, his head marker and minimap dot for both teams, `root.srp_show_carriers` at every match
  start (true in a hunt, false otherwise), the hud's update call, the messages, the prompt, the
  welcome, the world check's parameters.
- **After the first local test (2026-11-21, map 309, two windows):**
  - nobody was ever in the match (the rule's `m_sides` 0 in every world): slot 8 comes before the
    place's player has a team of the two, and an unknown team was taken for a spectator's. Now slot 8
    takes a player in only with his team, and the tick takes in every place of the world's active
    places (a leaver's bit goes in the same call as slot 9) whose player's team the rule does not have
    him on. Without it no take, delivery or carrier, and sudden death found team 0 "wiped" at once:
    the test's 1:0 end. `check_artefact_hunt_026g.py` now enters team 1 with no team at slot 8;
  - a take, delivery, drop and a player taken in each write one line to the patches' log;
  - the prompt gives the hud its key (`st_pickup_item`, `st_put_item`), as patches 78 and 85 give
    theirs: the movie formats it with the Use key into the game's own context line; a translated text
    was no key of its dictionary ("#Pick up (E)");
  - an end by sudden death's wiped team is reason 2 (`m_over.reason`), its own texts
    `st_artefact_hunt_won_wiped` / `st_artefact_hunt_lost_wiped` (the owner: the end must say what
    happened);
  - the two resyncs of the test were not the rule's: at 294,800 ms the two games' worlds differ only
    in player 1's look pitch and turn and the bullets' first word, the rules sections byte-identical,
    after the server found 8 of player 1's inputs too late to relay (a 1 s stall); the first followed
    the server's move of player 2's input (147,150 -> 147,700 ms), and the rule's bytes were the same
    in both games' worlds around it (it made no decision in the match: nobody was in it).
- **Built after** (the same patch): the artefact in the world, the carrier's glow, the announcer and
  the icon's carrier mark.
  - The artefact: the item's model (class 15) and its idle, spawn and destroy particles (class 61)
    in one query of the patch's own (`query_assets`: the artefacts' cooks' own request shape,
    0x25E91A, class 3Dh at 0x25E862), asked at a hunt's first frame, each kept for the process.
    `game_world.after_move` (order 5) reconciles the scene with the rule each frame: lying, the model
    (drawn as a world model) and the idle at its point; appeared at a spawner, the spawn particle
    too; removed by its time with nobody carrying it, the destroy particle where it lay; taken, the
    model and the idle out. The particles play as the artefacts' clients do (`play_particle_system`
    0x22660, 7 stdcall arguments, the particle by value with a reference added: Lifebone's
    0x25F335..0x25F387) and stop by `remove_particle_system` 0x229D0 (0x25ECC6). `game_world.
    deactivate` (order 3) takes all of it out of the scene.
  - The glow: Forest Protection's pattern (patch 111): `artefact_wood_shine` (the item's `effect`)
    asked once at a player's tick, held from a living carrier's tick (`battery.carrier_tick` order
    6), one a place (`medical.effect_capacity` order 3), ended by its clock when he carries no more
    (`medical.effect_clock` order 3). The client stream only: not uploaded.
  - The announcer (above): the line for the viewer's team (own or enemy), none without a team or
    for a removal; a running line freed first.
  - The mark: `player_icons.carrier_mark` at 0x1F2D8B (the record's `cmp [edx+178h],0`, 7 bytes):
    for the hunt's carrier of either team EAX = place x 3 and CL = 1, out at the record's compare
    0x1F2D98; for anyone else the game's own test.
- **Otherwise than above:**
  - the rule object's layout: tdm's slot 12 (0x267820, it writes +118h) is kept, so the blackboard is
    at +118h, the tick delegate at +120h, the core at +138h, the game at +13Ch (0x140 bytes);
  - the Use and Drop edges are patch 78's (its machine's last raw mask), not +330h (above);
  - the rule's base type is 6, by which patch 87 names a mode-13 world's rule;
  - a taken class 481 logs nothing (the log is not up yet in the constructor); the mode's maps then
    fail to load their rule.
- **LIKELY items met:**
  - an exit at a function's entry (`carry.input`'s `held`): PROVEN under Unicorn (apply_new_input
    returns at once, both arguments popped);
  - the class-61 particle of a patch's query: PROVEN in the code (the artefacts' cooks ask class 3Dh
    through the same query and play `results[1]`, their callback 0x260270 storing +40Ch); under
    Unicorn the query's requests and the scene's calls (the query itself answered by the check);
    whether the ported particles loop and look right: not met;
  - the one-shot hud sound: PROVEN as the native path's calls (0x23F676..0x23F6F8: the game world's
    scene +94h, the sound world's logic user +B0h, `emit_hud_sound`, the proxy's play in mode 0) and
    run under Unicorn with those calls answered; that freeing stops it: not met;
  - a Number Invoke: not met (`invoke_numbers` writes GFx type 5 as Research's sender, not run in a
    game);
  - the seed: the create's query's options (+577B0h) as tdm's create gets them; run under Unicorn with
    made-up options, the shape of the query PROVEN only by tdm's constructor's +0Ch;
  - the modifiers' sum: PROVEN with the game's own apply_modifier (10 x 0.8 and 10 x 1.05);
  - `[core+18F54h]` vtable+1Ch after our slot 17: not read; the check runs the rules' loop only.
- **Texts** the patch shows (all eleven dictionaries): `st_on_spawn_victory_artefact`,
  `st_on_destroyed_victory_artefact` (0.46c2's keys), `st_artefact_hunt_you_picked`,
  `st_artefact_hunt_team_delivered`, `st_artefact_hunt_enemy_delivered`,
  `st_artefact_hunt_won_collected`, `st_artefact_hunt_won_time_out`, `st_artefact_hunt_lost_collected`,
  `st_artefact_hunt_lost_time_out`, `st_artefact_hunt_draw`, `st_start_match_welcome_message_13`.

### The mode's number

`mode.mode` 13. Taken: 2, 3, 4 (the game's), 5 Research, 8 Free for All, 12 Freeze Tag
(uncommitted); the lobby's 6, 7 (kept for Scarcity), 9, 10, 11. 0.46c2's own (7 in its names, 4 in
its hud) are taken. Every use of the number in the exe is a compare for equality (ffa.md, "The mode
number 8"): 13 goes every "else" way: no `set_tickets_required`, no native minimap objects
(`root.update_objects` is still sent, empty), no welcome, no `set_last_players`.

## Wire

- **The rules blob** of this mode: the time rule (13 bytes), the respawn rule, **the rule**, the kd
  stats (5 a place), the blackboard's two bools (snapshot_rules.md 2.1). The rule's bytes are
  `Rule::fields` in order, one definition for the game's serialize, `Rule::write` and `Rule::read`
  (`crates/artefact-hunt-rule/src/wire.rs`). Little-endian:

  | size | field | domain |
  |---|---|---|
  | u32 | `m_random` | any (the rng's state) |
  | u8 x2 | `m_score[team]` | 0..=`score` |
  | u32 | `m_round_starts`: the rounds started after the first (each a fade's end, everyone respawned) | see below |
  | u32 x2 | `m_sides[team]`: who is in the match | bits 0..19; no place in both |
  | u8 | `m_stage` | 0 idle (the scores 0), 1 play, 2 over |
  | | **over** | |
  | u32 | `m_over.at` | a time |
  | u8 | `m_over.reason` | 0 collected, 1 time out, 2 sudden death's wiped team (its point) |
  | | **play** | |
  | u32 | `m_start` | a time |
  | u8 | `m_round` | 2 waiting, 3 spawned, 4 carried, 5 dropped, 6 brought, 7 fade, 8 countdown (0.46c2's state ids) |
  | u8 | `m_clock` | not for 6: 0 regular, 1 sudden death |
  | u32 | `m_sudden.from` | only for 1: start..=start + horizon |
  | u32 | `m_sudden.fallen` | only for 1 and not for 7: places of the sides |
  | u8 | `m_spawner` | only for 3: below the map's spawners |
  | i32 x3 | `m_dropped` | only for 5: mm, each within 100 km |
  | u8 | `m_carrier` | only for 4: a place of the sides, not fallen |
  | u32 | `m_until` | not for 4: start..=start + horizon |
  | u32 x2 | `m_way.left[team]` | only for 3, 4, 5: 0..=65536 |
  | u32 x20 | `m_way.carried[place]` | only for 3, 4, 5: 0..=128 x 65536 |
  | u8 | `m_size` | the bytes before it |

  Across the fields: a regular clock outside 6 has both scores below `score`; 6 has at most one
  team at it; sudden death has the scores at most 1 apart and, with them equal, both teams standing
  (`sides` less `fallen` not empty: the point is given at once). 20 bytes idle, 25 over, 29 to 138
  in play (the section holds 0x800). `m_round_starts` (`Rule::round_starts()`, what the server charges
  lives by) is exact for the state: idle 0; in play the points, less one in 6 and 7 (that delivery's
  round has not started) and one more for sudden death's wiped team's point (unequal scores in
  sudden death); over, the points less one when collected (the goal, or a late lead) and when wiped
  (sudden death's point for a team with nobody left: reason 2, its own end text), the points to two
  less at a time out (a timed-out hunt, a late delivery, a delivery in sudden death, with a wiped
  point in its step at most). Added after the review, before any release: the layout is not
  released. `horizon` is the match time, two round restarts
  (`time_before_new_round` + 1 s + `round_countdown_time`), sudden death, an artefact's longest wait
  or life and a minute. `m_size` is there for the world check: the respawn rule before this one has
  two bytes a respawn point and nothing says how many, so the mode's rule is found from the
  section's end (`Rule::read_back`), as Research's.
- **What else of a world changes:**
  - the respawn rule's own bytes (snapshot_rules.md 2.3: a mask of the places that wait, a time
    each): the times the rule decides (the waves, the held -1, the round start's step time, which
    the spawn consumes) in the native fields and domains: no new byte;
  - the blackboard's two bools are 0x00 while sudden death is on in play (they were always 0xFF
    outside Team Deathmatch): no new byte;
  - the statistics: the deliverer's event 20 (+1; one byte on the wire) and each carrier's event 6
    (his share of the way in hundredths; two bytes), counted as Research counts (the place's cell of
    its current kit, then the listener through 0x2F5D40); the tail's `m_team_points` are the scores
    and `m_winner_team` the leader (0, 1, 2 a draw), written after every transition, at event 0 and
    at the end of slot 16 (the time rule's deserialize sends event 0 first). No battery records
    (they exist with a gather rule only);
  - a player's block: nothing new. A carried artefact is the rule's `m_carrier`; the modifiers, the
    sprint and the markers are derived; the glow is a client effect that is not uploaded;
    `m_need_to_select_animations` and the input's bit 22 are native fields with native domains; a
    frozen countdown leaves the stored input the spawn's zero.
- **The world check** (`srp-validate`, the same code in the match server and in patch 87):
  `ModeRule::ArtefactHunt(artefact_hunt_rule::Rule)` for mode 13, read with `Rule::read_back` over
  the bytes between the timelimit and the kd stats, then the respawn rule to where it starts; the
  refusals are `WireError`'s. It needs the map's `Params` in its `Context`: the game has its rule's;
  the server parses the mode config of its game data with `Params::parse` (nothing kept in code).
  Cross-checks to add: the statistics' `m_team_points` are the rule's scores and `m_winner_team`
  its leader; in play the blackboard's bools are both `Rule::respawns()` (an `Over` reached by a
  delivery in sudden death keeps them 0x00 until the next tick: over, either); `m_start` is the
  timelimit's start in state 3, and the stage is idle exactly while the timelimit is not in state
  3. Patch 87's
  `Match::of` names the mode by the rule's vtable. The crate is `no_std`, allocation-free and
  never panics on bytes (its tests read every state a generated match passes through).
- **Late joiners, rollbacks, replays:** the state above is everything the rule computes from
  (`random_matches_keep_the_invariants` loads a mid-match state from its bytes and replays the
  events: the same state). The patch keeps nothing of the simulation outside it; what it decides at
  the respawn rule's tick and at an input is a function of that state and the step.
- **A simulation change** for this mode only: the patch's PR adds the `next` row of
  `simulation_changes.tsv`; this crate alone changes no match.

## HUD calls

The hud and the sounds are no part of a world. Everything below runs in the hud's frame, after the
frame's steps and re-simulations, from the rule as it then is: a step played again says nothing
twice, and a game that joins late hears nothing old (`Cue::between` of the rule shown last and the
rule now; the first frame of a match only remembers). A frame can show a take that a later
re-simulation undoes: Research has the same exposure.

- **Where:** a handler in the lohook chain `player_icons.update_start` 0x23AA10 (every frame; orders
  1 to 3 taken, 4 by Freeze Tag's worktree: 5), and `hud.initialize_match` 0x234880 to forget what
  was shown. The viewed place is the hud state's first byte (ui+24h, the watched player in a
  replay; 0FFh none), his team the profile's.
- **`root.artefact_hunt_update(...)`:** 28 values, every one a GFx Number (type 5, as Research's
  sender; `vostok::ui::Movie` needs an `invoke_numbers`), from `Rule::hud` and the viewer. Sent
  when a checksum over them changes (the four ms values taken as whole seconds, rounded up) and
  every 60 frames; the movie runs its own timers between.

  | # | value | domain |
  |---|---|---|
  | 0 | own team | 0, 1; 3 a viewer without a team |
  | 1, 2 | score of team 0, of team 1 | 0..goal |
  | 3 | goal | `score` (5) |
  | 4 | phase | 0 before the start, 2 waiting for the artefact, 3 spawned, 4 carried, 5 dropped, 6 the 3 s after a delivery, 7 the fade, 8 the round's countdown, 10 finished |
  | 5 | ms left of the phase | 3, 5: the lying artefact's life; 6, 7, 8: the phase's; else 0 |
  | 6 | that phase's whole length, ms | 120000, 60000, 3000, 1000, 5000; else 0 |
  | 7 | after the 3 s | 0 not phase 6; 1 the match ends; 2 a new round |
  | 8 | clock, ms left | the match time's; in sudden death its own (whole until its start) |
  | 9 | clock, whole ms | 900000; 180000 in sudden death |
  | 10 | sudden death | 0 off, 1 it starts with the coming round (phase 6), 2 on |
  | 11..14 | own alive, own total, enemy alive, enemy total | alive: still in while sudden death is on, else 0; total: in the match; a viewer without a team has team 0 as "own" |
  | 15 | artefact | 0 nowhere, 1 lying, 2 carried |
  | 16, 17 | its x, -z | while lying; else 0 |
  | 18 | carrier's place | 0..19; 255 none |
  | 19 | carrier's team | 0, 1; 2 none |
  | 20 | the viewed player carries | 0, 1 |
  | 21 | bases | 2 |
  | 22..27 | a base: team, x, -z | team 0's first |

  The movie draws from it: the scores (its own branch for game type 13; `set_game_type(13)` is
  native), the minimap's artefact while lying and the bases (its own highlighted with #20), as
  Research's units are placed (x, -z), the carrier's row and dot, "carrying" for #20, the last
  players in sudden death, "Round restarts in" with #5 in phase 6 when #7 is 2 (0.46c2's
  `show_round_restart`, `set_round_restart_time`), a full-screen fade over phase 7's #6 (0.46c2's
  `show_fade_animation(1.0, true)`), the countdown's "Awaiting match start: 00:05" with #5 in
  phase 8 (0.46c2's pregame text: `st_final_countdown`, the exe's own key, in the movie's pregame
  box), and what #10 announces.
- **Native calls that go on by themselves:** `root.set_game_progress` (0x2364C0, from hud+90h, +92h:
  slot 17 writes the scores there, u16), `root.set_match_time` (from hud+4, above),
  `root.set_respawn_time` (0x236C30 from the respawn rule's own time, 0x316C20: a wave shows by
  itself; "" in sudden death, where no time is set, and once a held time is -1 in a restart),
  `root.update_players`, the kill feed. The native pregame calls (`show_pregame`, `set_pregame`) are
  the time rule's and are not sent for a round.
- **Messages:** `root.set_parameterized_message(text, 21h, 14h, 4000)` with the dictionary's text
  (`translate_text` 0x1FF0F0), as Free for All's; a message the movie does not take is owed to the
  next frame. By `Cue::message` for the viewer: the artefact appeared (everyone), "You picked the
  artifact" (the picker alone), "Your team have delivered" / "The opponent team delivered", "Artifact
  destroyed", and the end's won, lost or draw. A round's restart has no message (0.46c2's none: the
  hud's "Round restarts in" is its only word). 0.46c2's third argument (its enemy flag, LIKELY a
  colour) has no place in 0.26g's call; the hud movie has no `show_game_message`.
- **The welcome:** a handler in the chain `game_world_ui.welcome` 0x23B36A (orders 0 and 1 taken),
  `st_start_match_welcome_message_13`, 6000 ms, as Free for All's.
- **The announcer.** 0.26g has none (no `sound_notifications`, no `vo/` string in the exe) and no
  voice language, only the text's. The seven lines are sound collections (class 258, 0.26g's
  `sound_collection_cook`, its class 102h at 0x5F4A3) asked once with `vostok::resources::
  query_collection` (patch 88's path, class 258), `<base_path>/<voice>/<name>` from
  `mode.sound_notifications`; the voice is the text's language where the pack has its folder
  ("original" russian), else english. `Cue::line` for the viewer (none without a team, none for a removal or the
  end) is played once as a hud sound in the game world's scene (`emit_hud_sound` 0x65630, the
  proxy's `play` 0x5ABA0 in mode 0, the native path at 0x23F676..0x23F6F8), the line before it
  freed first (`free_object` 0x5B390). New in `vostok`: a `play_once` and the scene's field. LIKELY,
  not run: that freeing a one-shot voice stops it, and that a collection's emitter plays one of its
  items as a hud sound (the native hud sound at 0x23F676 is emitted the same way).
- **Nothing native to call for:** the fade, "Round restarts in" and the round's countdown text are
  the movie's own (0.26g's exe has no `show_fade_animation`, `show_round_restart`,
  `set_round_restart_time` or `show_artifact_carry`); 0.46c2's end music.
- **Texts** (all eleven `client-patches/localization/<language>.tsv`): the five messages, the three
  end texts, "Round restarts in", the welcome, the mode's name and description; `st_pickup_item`,
  `st_put_item`, `st_drop_action` and `st_final_countdown` are the game's own.

## srp

What the servers do for the mode. Nothing of it was played: no world of the mode exists yet.

- **The mode:** `maps::RuleMode::ArtefactHunt` (13, rules `[93, 91, 481, 94]`), `GameMode::ArtefactHunt`,
  lobby number 13. A config of mode 13 is played when the data has its project, as every mode's.
- **The parameters:** `maps::artefact_hunt_params` reads the config ("Data") into the crate's
  `Params` once, at the data's load (`MapMode::artefact_hunt`, set exactly for mode 13): the seconds
  and `score` as ints, the positions, axes and halves as float3s, `drop_lift` and `usable_radius` as
  floats. Another type, or what `Params::parse` refuses, stops the data with the key's name.
- **The lobby:** rooms, private rooms and the match vote offer its maps as every mode's (nothing
  there is of a mode); the lobby movie's `GameMode.ARTEFACT_HUNT` 13, after Free for All in the
  create window, named `st_srp_mode_artefact_hunt`; its loading icon the config's `ui.icon`. Player
  vs Bots does not offer it (its maps' whitelist), and the match server refuses a match of it with
  the server's bots (as Research).
- **Late joiners** are taken until 30 s before the end of play by the rule's own clock in the last
  trusted world (the match time, in sudden death its own; none once over), not the time rule's
  900 s.
- **The world check** (`srp-validate`, mode 13, `Context::artefact_hunt` the map's `Params`):
  - the rule is `Rule::read_back` over the bytes between the timelimit and the kd stats; its
    refusals are `WireError`'s (`ArtefactHuntRule`); the respawn rule ends where it starts;
  - its sides within the match's places; its stage idle exactly while the timelimit is not in play,
    in play from the timelimit's start, over only in play;
  - the blackboard's two flags, which the patch writes in play only: both true before play (the
    blackboard's own), both `Rule::respawns()` in play, equal once over (the last of play);
  - the statistics' `m_team_points` are the scores; `m_winner_team` the leader, written after every
    rule step (before play the draw, or before the rule's first step the statistics handler's own
    3 or 0xFF);
  - a world of mode 13 checked without the parameters is refused (`NoArtefactHuntParams`).
- **The end:** a trusted world whose rule is `Over` ends the match at its `at`. The time rule's 900 s
  does not: only a match in play past its start + `Params::horizon()` (no match of the parameters
  ends later) whose worlds never showed the end is stopped by time. The result is the statistics' tail: the scores and the
  leader (team 0, team 1, a draw).
- **Score:** the static info's weights of mode 13 are 0.46c2's: put (event 20) 10, bring (event 6,
  hundredths) 50. A delivery scores its team 60.
- **Pay** (the port's rule, the owner's to change), on the payment copy only, after the repeated-kill
  cap:
  - per player, puts pay at most `score` and bring at most `score` x 100 hundredths: no team
    delivers more in a match, and a delivery's shares add up to 100;
  - then puts and bring together pay at most 120 (two deliveries carried alone) + 40 a paid kill + 20
    an assist, Research's allowance's steps; above it both are cut in proportion (the puts' rounding
    given back as bring);
  - why: passing the artefact pays nothing by itself (the shares of a delivery add up to 100,
    whoever carried), so farming is feeding deliveries; the goal bounds them a match, the allowance
    ties what more they pay to fighting. A carrier who delivers three times alone without a kill is
    paid two.
- **Lives** (`autobuy::used`): every round start is a life for every player, counted by the rule
  (`Rule::round_starts` of the last trusted world, the result's `round_starts`).
- **Not covered until real worlds:** the checks above against a game's worlds (`world-check` has
  no world of mode 13; `world-drift` needs the patch's world check with the parameters), the
  tail's leader before play, the flags in a delivery's last step, a late joiner's and a resync's
  worlds.
- **Follow-ups:**
  - a late joiner's lives count the round starts before he came in too (the result has the match's
    count, not his);
  - the matches tab's time left is the time rule's, to 900 s: the rule's clock past it;
  - the balance counts a point of lead as a battery's (0.55 of a player; the goal is 5, not 10);
  - the history counts a delivery (event 20) in its batteries column.

## Parts

| part | where |
|---|---|
| the rules (a state machine, its wire form, tests) | `crates/artefact-hunt-rule` |
| the game's side (the rule's cook, the artefact, the hud calls) | an exe patch in the `.srpx` arena |
| the hud | `flash/hud` |
| the content | `scripts/artefact-hunt`, a pack in `release/content.tsv` |
| srp (the mode, rooms, results, pay, the world check) | `crates/survarium`, `srp-validate`, the servers |

## Release impact

- Exe: a new patch; the simulation changes for this mode only.
- Content: the pack and the hud movie; the server data with it makes the maps playable.
- srp: the mode in the maps, rooms, results and pay; the world check reads the rule's state.
