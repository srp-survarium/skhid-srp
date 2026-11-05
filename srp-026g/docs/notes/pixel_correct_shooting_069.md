# 0.69d0 "pixel correct shooting": how shots and players are networked, against 0.26g

The owner, 2026-11-05: "0.69 has completely different networking for bullets and player
character. This is something called 'pixel correct shooting' and I want us to
investigate that. If we find its improvements significant, we will backport them."

This note says what 0.69d0 does when you fire at a moving player, what it draws, what
0.26g does today, and what of it is worth bringing to 0.26g with our relay-and-vote
server. It builds on two open notes and does not repeat them:
- PR #468 `netcode_069.md` (branch `re/netcode-069`): 0.69's lockstep, per-tick
  updates, confirmations, clock steering, `ping_input`, history slots, sealed kills.
- PR #472 `netcode_port_069.md` (branch `re/tick-length`): the port plan, the tick
  length, 0.26g's bullet path and collision groups (its section 7).

**Sources.** The 0.69d0 x64 exe of `tools/re-kit/v069d/identity.json`
(`private/music-research/retail-069d.exe`, sha256 `1e1ae04d...`), read with a fresh
Ghidra 12 project (`private/re-069/pcs-gh`) and its objdump listing. The 0.26g original
exe (`tools/re-kit/v026g/identity.json`, Ghidra project `private/battery-recovery/
ghidra`). Strings of the x86 builds 0.27d0, 0.28a2, 0.28d0, 0.31a2, 0.46c2 from the
weapon survey. The direct-peer design `peer_prediction.md` (uncommitted, main checkout).

**Addresses.** 0.69 addresses here are VAs of the identity exe (base 0x200000000).
**#468 and #472 used another 0.69 x64 build** (Ghidra program `s069.exe`, MD5
`5585767e...`, .text 0x8A4C00 bytes, not the identity exe): the same code, shifted by
0x100..0x3C0 bytes. Their `0x2002F1190` is our `0x2002F1480`, `0x200306260` is
`0x200306550`, `0x2003994A0` is `0x200399860`, `0x20040FD40` is `0x200410100`,
`0x2003EBB50` is `0x2003EBF10`, `0x2002F1110` is `0x2002F1400`. The re-kit rows use
the identity exe.

Tags: **PROVEN** (read in the code at the address), **LIKELY**, **GUESS**. 0.69 names
in `code` come from its own diagnostics where the re-kit row says `observed`; the
others are ours. Diagrams are 7-bit ASCII.

Contents:
1. Short answers
2. 0.69: the shot, the hit test, the drawn players
3. 0.26g today
4. Timeline: what the shooter sees, what is hit
5. Prediction and ping differences (0.26g, 0.69, 0.26g with direct peers)
6. What it costs players
7. Fit with srp's relay and CRC agreement
8. Backport ranking
9. When it arrived
10. Re-kit rows
11. Open questions

---------------------------------------------------------------------------

## 1. Short answers

1. **Shot timing.**
   - 0.69: the shot is the fire animation's shot event (`on_shot_event` 0x200394EA0 ->
     `instant_fire` 0x2006D6240), at the event's exact ms; the origin and direction are
     the simulated weapon/view matrix (0x2006CD850) plus dispersion, never the rendered
     camera. A bullet travels; each step's segment carries its start and end ms, and the
     hit time is the exact ms the bullet crosses the target, interpolated along the
     segment (0x200399860, 0x200306550). **PROVEN.** So hits are sub-tick: that is the
     likely meaning of 0.42's "unlimited tickrate" (GUESS for the press term).
   - 0.26g: the same animation event and the same kind of matrix (player+0x44C, which is
     also the first-person camera), exact ms at the bullet's spawn; but bullets step only
     on the 50 ms grid and test the targets **as they are at that grid time**, 0..50 ms
     after the bullet reaches them (bullet_manager::tick 0x321D10 runs only on-grid).
     **PROVEN.** No lag compensation.
2. **Remote player presentation.**
   - 0.69 **draws every other player in the past, interpolated between two world
     captures**, at `render time - remote_player_delay(view player)`, every frame
     (0x2002374D0 -> 0x200236710 -> 0x20022F390). The view player is the one the camera
     controller follows; the render time is `now - view delay` (0x200238480,
     0x2001E6600). **PROVEN** for the per-frame pass and its arithmetic; **LIKELY** that
     in live play the view place is the local player, so others are drawn at about
     `now - (own delay + tick)`, and that the renderer draws from this pose (followed to
     player+0x1F2E0 and the weapon's presentation, not to the draw call).
   - Bullets are tested against the target posed at `crossing ms - remote_player_delay
     (shooter)`. Both use the same world-state number (the shooter's `ping_input` plus a
     tick), so **what you see and what is hit agree to within one tick** (20 ms LIKELY):
     PROVEN for both formulas; the one-tick difference is not explained (open question 1).
   - `interpolate_player_matrices_data` is **not** a rendering type: it is the job bundle
     of the hit-pose rebuild `recompute_damage_collision_bones_for_time_ctr` 0x200307450
     (its boost::bind is used only there). **PROVEN.**
   - 0.26g draws everyone at the predicted present: the render move simulates to the
     clock with the last known inputs (extrapolation by the simulation); a correction
     after a rollback is a snap. **PROVEN** for the draw time, **LIKELY** for no smoothing.
3. **Hitbox poses.** 0.69's 16 history bodies (21 inserted, 16 usable) are coarse root-only
   bounding bodies for the broadphase (as #472 says). The body actually tested is the
   **full bone pose**, rebuilt for the exact ms by interpolating the local bones of two
   world-capture animation states (0x200307450: slerp of rotations, lerp of translations).
   **PROVEN.** Sub-tick time: interpolation, not nearest slot.
4. **Local player.** Both builds simulate and draw your own player at the present
   (predicted, no input delay). 0.69 exempts it from the delayed presentation by a
   per-player construction flag (player+0x19E0; LIKELY the local one). No correction
   smoothing found in either: a snap. 0.69's `interpolating_camera` (vtable 0x200970030)
   is a Gaussian-eased camera transition (kill camera / menus), not network smoothing.
5. **`allow_killer_claim`, `st_claim_added`: no hit claim.** It is the kill camera's
   "complain about your killer" button: `comment_about_user` sends a user comment to the
   lobby (0x2001E8D60) and then calls `root.allow_killer_claim(false)` (0x20019D540);
   the notice shows `st_claim_added` or `st_compliment_added` (0x200192060). **PROVEN.**
   Nothing in 0.69 lets a shooter claim a hit; every client and the server compute hits
   from the same world.
6. **When.** Sub-tick hit poses (`recompute_damage_collision_bones_for_time`,
   `interpolate_player_matrices_data`) and `ping_input` first appear in **0.46c2**; they
   are absent from 0.31a2. That fits 0.42 (March 2017). The per-tick protocol and clock
   steering are 0.69 only (#468). The kill camera and `interpolating_camera` came in
   0.31a2. Section 9.
7. **Backport, ranked** (section 8):
   1. **Sub-tick hit time** (S): test the target at the bullet's crossing ms inside the
      grid step, its root lerped between the previous and the current grid. Removes a
      0..50 ms bias (mean 12 cm at 5 m/s). No protocol, no server. Worth it.
   2. **Delayed drawing plus a declared, capped rewind** (M-L): draw remote players from
      confirmed states at `now - D`, and let the shot input declare D, clamped by the
      simulation, so the hit matches what was drawn and is never undone. Costs players
      more deaths behind cover (about +D) and older targets on screen. An owner choice;
      only with a small cap (100..150 ms), after 1.
   3. **Direct peers** (existing WIP): fresher remote movement on screen; with 2, a
      smaller D for a direct pair. Separate.
   4. **0.69's per-tick protocol and clock steering** (#472 phases 2..3): your view age
      becomes your own round trip, whatever the others' ping. Separable; not needed for
      1 or 2.
   - Not worth it: the 20 ms tick (#472), full-bone rewind, compensated explosions.
   - 1 and 2 change what a match computes: each needs its `simulation_changes.tsv` row.

---------------------------------------------------------------------------

## 2. 0.69: the shot, the hit test, the drawn players

### 2.1 The shot

- `weapon_core_fire_state_base::on_shot_event` 0x200394EA0 (assert
  `m_bullets_in_queue > 0`) runs on the fire animation's shot event and calls
  `weapon_core::instant_fire` 0x2006D6240 with the event. PROVEN.
- instant_fire (assert `get_user()->is_alive()`) takes the fire matrix from 0x2006CD850,
  adds dispersion (0x2006CBC50, 0x2005A9650), and calls the bullet manager's virtual
  +0x58 once a pellet with the event's time `[event+0x20]`. PROVEN for the call; that
  0x2006CD850 is the simulated weapon/view transform is LIKELY (it reads weapon state
  only, nothing from the renderer).
- So the shot's time is the simulation time of the animation event, its aim the simulated
  player. The click itself is an input with a precise ms (#468 2.3: a byte back from the
  tick), as 0.26g's precise inputs.

### 2.2 The bullet and the hit time

```
 bullet_manager::tick 0x2003BCFA0 (core delegate)      every step, time T
   for each bullet: bullet::tick 0x2003983E0(T)        flight from +0x58 to T
     segment [t0, t1] = test_segment 0x200399860
       bullet+0x7C = t0, +0x80 = t1, +0x70 origin, +0x84 length
       slot  = ((t0 - remote_player_delay(shooter)) / tick) % min(max/tick + 1, 16)
       ray test group 0x4010, mask (0x8000 << slot) | 0x4008   (statics + that slot)
         a slot body is touched -> needsCollision -> get_damage_collision_for_query
           0x200306550(target, query, point):
             t_cross = t0 + floor(min(1, |point - origin| / length) * (t1 - t0))
             T_hit   = t_cross - remote_player_delay(shooter)    (one tick if none)
             refuse if T_hit < target spawn (+0x704) + tick
             T_hit   = min(T_hit, newest ticked time)            core+0x180A9C
             recompute_damage_collision_bones_for_time_ctr 0x200307450(target, T_hit)
             -> the full hit model, posed at T_hit, is tested against the ray
   apply collected hits; a hit on a dead target counts if its time <= death time (+0x708)
```

PROVEN for every line (decompiles of the four functions); the uncommitted
controller-animation research in the main checkout reads the same body of 0x200306550
with an executable oracle. Two bypasses use the current pose: player+0x5F8 transition
weight at 1 (0x2003456D0), and a world state 4 or 5 at `time - own delay`
(0x2002F6EA0); their labels are open.

**The pose at T_hit** (0x200307450, PROVEN): the two grid captures around T_hit are
found in the world history; each player's captured animation tree is restored without
ticking (0x200306810), the collision bones' local transforms are sampled from both and
interpolated at `(T_hit - tA)/tick` (0x2003069E0, slerp/lerp 0x2005A9B60), the weapon
hierarchy likewise (0x200306D90), then the hierarchy is rebuilt and pushed to the
compound shape (0x200599F90). `interpolate_player_matrices_data` is this job's bundle.

**The slot bodies** (0x200305F90 from the per-tick 0x2003024C0, PROVEN): body
`(T/tick) % N` is moved to the player's root at grid T, read from the same captures. Root
only: a broadphase filter. They are derived, not serialized (#472 7.2).

### 2.3 The delay

- `remote_player_delay_in_ms(place)` 0x2002F1480: place record (core+0x17C2A0, stride
  0xB8) mode +0x4B: 0 = base + tick, 1 = base + tick x a factor from the place's weapon
  (0x2002FF700), 2 = base; base = +0x5C; clamped to `[tick, max remote delay]`
  (core+0x180ACC, a u16 of the static match info). PROVEN.
- +0x5C is the place's `ping_input` (LIKELY: its readers treat it as that place's
  latency, 0x2002F1400 and 0x2001AEF70; the applier was not found).
- The ping a client puts in the world (0x2001AEF70, on every 0xDF): the age of each
  server update on arrival, in a 257-sample ring; mean + k x spread, capped at 500; it
  may fall by at most 50 per change and is sent at most every 200 ms, as
  `change_input<ping_input>` at the next tick. PROVEN. With 0.69's clock steering this
  age is about the client's own round trip (#468 2.6): the age of the newest closed
  tick on its screen.

### 2.4 What is drawn

```
 game_world::tick 0x200238480, every frame, now = the game clock
   camera controller = game object type 8 (0x2001E3E70), view player at +0x2C0
   offset = 0                       replay (game_world+0xF4648) or no controller
          = tick                    viewing a live player (+0x2C0 set)
          = min(view_delay, now - tick - origin) otherwise
              view_delay 0x2001E6600 = remote_player_delay(game_world+0x7B62) + tick
   render = now - offset
   render_move 0x200236FF0: move to grid(render - tick), then to now
                            (the world is simulated to the present)
   job pose_players_for_view 0x2002374D0, unconditional:
     T = render - remote_player_delay(view player)     (0 if none)
     interpolate_players_at 0x200236710(T):
       items at floor(T) and floor(T)+tick             0x200236AC0
       for each active player: interpolate_presentation 0x20022F390
         roots at both item times from the world captures   0x20022E610
         slerp + lerp at (T - tA)/tick -> player+0x1F2E0
         (current root instead when player+0x19E0 is set)
```

- PROVEN: the per-frame job is not behind the kill camera; it runs every frame and its
  time is the render time minus the view player's delay (0x2002374D0..0x200237585 in
  the listing: no branch around the call).
- game_world+0x7B62 is written only by `select_view_place` 0x2001E5FD0, which also sets
  the core's owner place to the same place. LIKELY: in live play it is the local place,
  so the presentation offset is `remote_player_delay(local) + tick`; while spectating it
  is the watched player, so the spectator sees the world as the watched player was shown
  it. Not settled statically: which branch live play takes, and whether +0x19E0 marks
  the local player (open questions 2, 3).
- The kill camera additionally overrides the killer's matrices from a record of the kill
  (0x2002374D0 after the call); `allow_interpolation` is a key read with the kill
  camera's settings (beside `killer_death_duration`, `preview_camera`), not a netcode
  switch (LIKELY: no code reference to it was resolved).

---------------------------------------------------------------------------

## 3. 0.26g today

All PROVEN unless marked (0.26g VAs).

- **The shot.** `on_shot_event` 0x30EBF0 -> `instant_fire` 0x2E23A0 at once; the shot
  carries `[event+0x10]` to `bullet_manager::emit_bullet` 0x322830 and the bullet stores
  it as its spawn ms (bullet+0x5C). Origin and direction: player+0x44C (bone "Item" x
  root, written by 0x2DA4B0 every step) through `get_dispersed_bullet_direction`
  0x2E1D50. The first-person camera is set from the same player+0x44C (`update_camera`
  0x1F3480 -> 0x2422B0), so crosshair and bullet differ only by dispersion.
- **The bullet.** `atomic_tick` 0x2D1E20 passes on_grid = `time % 50 == 0` to every
  delegate; `bullet_manager::tick` 0x321D10 does nothing off the grid. At the grid G,
  `bullet::tick` 0x32C180 integrates from the last tick to G and casts one ray along the
  whole path (`check_collision` 0x32D4E0, group 0x10, mask 0x48). Bullets are the last
  delegate list, so the targets are already at G; their bones are posed lazily at G
  (`recompute_damage_collision_bones` 0x2DA5B0, #472 7.1). Bullets travel (speed
  ammo+0x2A4, drop by 0x32CDD0).
- **So a target is tested 0..50 ms after the bullet reaches it** (mean 25 ms): at 5 m/s,
  0..25 cm, mean 12 cm, in the direction it moves. Off-grid render steps cannot test
  bullets: they differ between clients (each frame ends at its own ms), the grid is what
  every client steps alike.
- **What is drawn.** `game_world::tick` 0x1F77E0: local input, render move to the clock
  (sync_core.md 1.3), then `player::mt_draw` 0x2474A0 for each player at its time.
  Remote players are the predicted present: simulated forward with their last input.
  A rollback that moves someone (or you) is shown as a snap (LIKELY: no blending code
  found). The only smoothing is the 200 ms "camera smooth" view (0x1F79D9..0x1F7C67,
  0x1F76B0, #472) for a spectated or replayed player.
- **Lockstep decides the hit later.** The shooter's world tests the target as predicted;
  when the target's real inputs arrive, the world is re-simulated and the hit is decided
  again against where the target really was at G. A hit marker can show for a hit that
  the re-simulation removes (#468 3.2).

---------------------------------------------------------------------------

## 4. Timeline: what the shooter sees, what is hit

A at 30 ms round trip shoots B at 150 ms, B running and stopping into cover at x.
0.26g at 50 ms ticks; 0.69 at 20 ms (LIKELY). Times are A's.

```
 0.26g                                      0.69
 -----                                      ----
 B's moves reach A ~115 ms late             B's moves reach A ~32 ms late
 (75 + 15 one-way + 25 grid snap)           (A's own round trip; B runs ahead)

 A's screen at t: B predicted to t          A's screen at t: B interpolated at
   from inputs <= t-115 (extrapolated)        ~t-72 from confirmed captures

 t   A fires; bullet spawns                 t   A fires; bullet spawns
 |                                          |
 G   next 50 ms grid: ray vs B at G         t_c bullet crosses B (same step)
 |   (B as predicted in A's world)          |   ray vs B posed at t_c - 52
 |                                          |   (confirmed: never changes)
 t+115 B's real inputs arrive: re-sim,      |
 |   ray vs B where B really was at G       |
 |   -> hit may vanish (marker stays)       |

 drawn B   : predicted present              drawn B   : t - 72 (D + tick)
 tested B  : true B at G (0..50 ms late)    tested B  : t_c - 52 (D)
 mismatch  : B's moves in the last 115 ms   mismatch  : about one tick (10 cm)
             + grid lag 0..25 cm                        (open question 1)
```

---------------------------------------------------------------------------

## 5. Prediction and ping differences

Assumptions: A round trip 30 ms (15 each way), B 150 ms (75), C 30 ms; 5 m/s; server
time 0; inputs reach the server one-way late (0.26g) or just in time (0.69, steering).
0.69 D = ping_input + tick (mode 0). "Direct" = `peer_prediction.md`, a direct route of
20 ms one-way between A and B.

### 5.1 Side by side

| | 0.26g (relay) | 0.69d0 | 0.26g + direct peers |
|---|---|---|---|
| (a) Own player | simulated at once, no input delay; correction = snap; clock set by 0xE5, corrected by 0xE7 (patch 15): about the server's clock | the same, and the client **runs ahead of the server by its one-way latency + 2 ms** (#468 2.6), so its inputs reach the server just before the tick closes | as 0.26g |
| (b) Remote players drawn | predicted present: last known input carried forward | **interpolated past**, now - (D_own + tick) (2.4) | predicted present; the direct hints feed a temporary projection each frame, removed before every confirmed move (`peer_prediction.md`) |
| Age of B's movement on A's screen | one-way B + one-way A + grid snap: 75 + 15 + 25 = **115 ms** | A's round trip: **~32 ms** (confirmed; drawn at ~72) | direct one-way + snap: 20 + 25 = **45 ms** (hint; confirmation still 115) |
| Age of A on B's screen | **115 ms** | B's round trip: **~152 ms** (drawn at ~192) | **45 ms** |
| Prediction error after B stops / reverses | up to 0.58 / 1.15 m on A's screen, for 115 ms | none (drawn from confirmed states) | up to 0.23 / 0.45 m for 45 ms |
| Size of a rollback from B's late input | 115 ms of re-simulation on A | none from B for drawing; the world still re-simulates on late inputs, bounded by the own round trip (#472 (c)) | unchanged (hints never enter the confirmed world) |
| A's hit on B tested at | true B at grid G, 0..50 ms after the crossing | B at t_c - 52, interpolated | as 0.26g |

### 5.2 Worked example: who dies behind cover

"Window" = how long after the victim reached cover on his own screen a shot can still kill
him. In lockstep the shot is simulated at the shooter's time, against the victim's state
at that time (or that time minus the rewind), and the victim learns of it one route later.

| | 0.26g, 50 ms | 0.69, 20 ms | 0.69 design, 50 ms | proposed 2 (section 8), 50 ms |
|---|---|---|---|---|
| A (30) shoots B (150): rewind | 0 | D_A = 52 | D_A = 82 | declared 140 (B's age + tick), cap 150 |
| B's window | A->B route: **90 ms** | D_A + B's round trip: **~204 ms** | **~234 ms** | 140 + 90 = **~230 ms** |
| B (150) shoots A (30): rewind | 0 | D_B = 172 | D_B = 202 | 140 |
| A's window | **90 ms** | D_B + A's round trip: **~204 ms** | **~234 ms** | **~230 ms** |
| A shoots C (30): window | 30 | ~84 | ~114 | 80 + 30 = ~110 |

- In 0.26g the window is the shooter-to-victim route, the same both ways: the low-ping
  player dies behind cover as late as the high-ping one, and his view of the high-ping
  player is as stale (115 ms) as the other way round.
- 0.69 roughly doubles the window (it adds the rewind to the route) but makes it a sum of
  the two round trips; a high-ping shooter is compensated by his whole delay (172 ms),
  the low-ping one by 52. Who dies behind cover more often: everyone about equally; the
  window is large whenever one of the two is far.
- Peeker's advantage: in 0.69 the one who steps out is seen late by the other's view age
  (A sees B late by ~72, B sees A late by ~192): the high-ping player peeking a low-ping
  one gains ~72 ms, the low-ping peeker gains ~192 against him. In 0.26g both see each
  other 115 ms late, but predicted (no delay when nobody changes direction).

### 5.3 (d) What the per-tick protocol, confirmations and steering change

- They make **your** view age your own round trip, whatever the others' ping (115 -> 32
  for A above), and bound your rollbacks by your own round trip. That is the gain of
  #472's phases 2..3, for prediction and rollback size.
- They are **not needed for compensation**: 0.69's compensation reads only world state
  (the shooter's `ping_input`) and world captures. A 0.26g compensation can take its
  delay from the shot input (section 7). Separable, both ways.

### 5.4 Direct peers and compensation

- **(a) How much earlier.** Through the server: one-way(sender) + one-way(receiver);
  direct: the direct route. For two Brazilians on the Polish server: ~200+ ms -> ~20 ms
  for movement. Shots (precise inputs) stay on the server path. The confirmed world, the
  rollbacks and the CRC are unchanged: hints only reduce what is mispredicted on screen.
- **(b) With compensation.** The rewind must equal the age at which the target **was
  drawn**: a direct-route target is fresh (~45 ms), a server-route one old (~140+). One
  delay per shooter (0.69's model) is wrong for a mixed screen: with the server-route
  rewind a direct target is hit ~100..180 ms in his past, behind cover. So the shot must
  declare the age it drew, **per target** (2.4's single number generalised): the
  simulation clamps it to the cap and poses that target at `t - age(target)` from
  **confirmed** inputs only. Everyone computes the same hit once the confirmed inputs are
  in; a wrong hint only makes the drawn pose differ from the tested one, never the votes.
  Which delay with direct peers on: the declared view age (it is the peer-route age for a
  hinted target, the server-route age otherwise).
- **(c) Package.** For a relay-and-vote server, "sub-tick hit time" alone, then direct
  peers, then a small declared-age rewind, is cheaper than 0.69's per-tick protocol (no
  server tick closing, no new server role) and aligns hits as well; it does not shorten
  rollbacks for players without a direct route, which only the per-tick protocol does.
  Direct routes are a later feature (owner, 2026-11-05): the first compensation design
  only has to take an age per target from the start (section 8, piece 2).

---------------------------------------------------------------------------

## 6. What it costs players

Each item: 0.69 / 0.26g today.

1. **Dying behind cover.**
   - 0.69 rewinds by `remote_player_delay(shooter)`, clamped to `[tick, max remote
     delay]` (0x2002F1480, PROVEN); the max is a server value (static info u16, not in
     the client). The broadphase has 16 slots (0x200399860: `min(max/tick + 1, 16)`,
     PROVEN): at 20 ms, 300 ms back at most before slots alias. The precise pose is
     clamped to the newest ticked time and refused before the target's spawn + a tick
     (0x200306550, PROVEN). Further limits: none by the victim's state beyond the two
     bypasses of 2.2 and "hits on a dead target count up to his death time" (0x2003BCFA0,
     PROVEN); **no line-of-sight recheck at the present**: the one ray tests static
     geometry and the past pose together (PROVEN, no second test in the bullet path).
   - 0.26g: no rewind; a victim dies behind cover for the shooter-to-victim route (90 ms
     in 5.2), from rollback alone.
2. **Peeker's advantage.** 0.69 draws others at ~`now - (D_own + tick)` (2.4, LIKELY for
   live play): 72 ms at 30 ms ping, 192 at 150. 0.26g: predicted present, but the inputs
   behind it are one-way sum + snap old (115 ms in 5.1).
3. **Ping above the cap.** 0.69 clamps the delay to the max (0x2002F1480): the drawn
   time follows the same clamped number (2.4), so drawing and testing stay together, but
   the drawn states are then partly unconfirmed, and the excess behaves as in 0.26g (hits
   tested against a predicted pose, decided again on re-simulation). LIKELY. The ping
   input itself is capped at 500 (0x2001AEF70, PROVEN). 0.26g: no cap needed.
4. **Inflated ping.** The client measures and sends its own `ping_input`. A cheat can
   raise it to the max remote delay and no further (the clamp), and the client code caps
   the sample at 500 and lowers it by at most 50 a change (PROVEN). Whether the 0.69
   server checked it against its own measurement is not visible. Our design (section 7)
   has the same bound: the cap.
5. **Rollback interplay in 0.26g.** Today a late shooter's input re-simulates the past,
   so a victim already dies after reaching cover (5.2). Compensation adds its rewind to
   that: **worse for the victim**, about +D; **better for the shooter**, whose hit on what
   he drew is no longer undone by the target's late input. It does not shorten rollbacks.
6. **Explosions and grenades.** 0.69 compensates them too: the explosion query takes the
   thrower's delayed slot and re-poses each player through 0x200306550 (0x200410100;
   sensor grenade 0x2003EBF10, PROVEN); the occlusion rays use the present static
   geometry (#472, LIKELY). 0.26g: present positions, occlusion rays from the centre to
   each bone (0x339630, 0x3393F0). Not worth compensating in ours: a fuse is seconds, the
   thrower's aim does not need it, and it would add deaths behind cover for nothing.
7. **Cost.** 0.69: 21 extra bounding bodies a player (16 used), a full bone re-pose per
   (target, time) on a ray that touches a slot (cached by time and query key), 47 world
   captures on a client (920 ms at 20 ms) that also keep each player's animation tree
   (0xB800-byte arena, 0x2002EF390). Its slot bodies are derived from the captures, not
   serialized; a client right after a load has fewer captures (its 0.69 server simulated
   and resynced on a mismatch, #472). For 0.26g: section 7 (c).

---------------------------------------------------------------------------

## 7. Fit with srp's relay and CRC agreement

srp simulates nothing: it relays inputs, compares the clients' CRCs (0xDF) by majority,
and takes worlds from clients (0xE8) for late joins and resyncs (`MATCH-SYNC-026G.md`).

**(a) What must agree bit for bit.** Every input of the hit test: the shot's ms and
matrix (already world state), the rewind of each target, the past pose it is tested
against, and the arithmetic (crossing ms, lerp). The hit changes health and deaths, so
all of it ends in the CRC.

**(b) Where the delay comes from.**

| Option | Carrier | Drift | Verdict |
|---|---|---|---|
| A match constant (static info) | none (0xE4 trailer) | does not fit any one ping | simplest, too coarse |
| Client `ping_input`, as 0.69 | a new input kind | none in 0.69: its renderer reads the same world number (2.4) | needs a new input kind |
| Server-measured, injected as a relayed input | the client accepts relays for any place and does not filter them (`process_player_input_change` 0x27C280 -> change_input 0x1FAB50, place from the payload; PROVEN) | **drifts**: the server's measure is not what the client drew | possible, worst match |
| **Declared view age in the shot**, clamped by the simulation | a profile (kit) input byte: 0xDE {tick, value} relayed as 0xF3, `change_input<profile_input>` 0x1FB730; stock kit code ignores values >= 3 (0x321790, match.md 20, LIKELY) | **cannot drift**: it is the age the client drew | **recommended** |

The declared age is what 0.69's `ping_input` is in effect (a client-declared number the
renderer obeys), without a new input kind. It is relayed, rolled back, replayed and sent
to late joiners like every input. Spare action bits are not proven free (the direct-peer
filter's mask leaves bits 10, 11, 26, 29, 30 out; no use proved either way): do not use.

**(c) The history.**
- 0.26g's 11 world items (50 ms each) hold everything a past pose needs, but the root is
  not stored as such: base_player::deserialize 0x2DB490 rebuilds it from the controller
  and the item matrix, and the animation state is a retained in-memory tree (0x395AF0 /
  0x395B50), probably outside the CRC bytes (GUESS). Posing a past body from them means a
  world load: too expensive per shot. PROVEN for the contents, LIKELY for the cost.
- So a small new ring: each player's root (position + yaw, 16 B) at the last N grid
  points, written once a step after the move at 0x2D94A9 (#472 7.3), **serialized with
  the player** (u16-length-prefixed block): in the CRC and in the uploaded snapshot, so
  a late joiner or a resynced game holds the same ring from its first tick. Unserialized,
  a fresh game would rewind against slots it does not have and compute different hits
  for N ticks: a resync loop.
- Size at N = 4 (150 ms at 50 ms ticks): 64 B a player, 1.3 KB for 20 places.

**(d) One client computes a different hit.** Its world differs from then on (health,
death, score). Its 0xDF CRC at the horizon item disagrees with the majority; after
`divergence_confirm_ms` (1000) the server resyncs it from a trusted world. The player
sees a resync (a hitch, a kill that disappears or appears). If the shooter is the odd one,
the shooter is resynced. A systematic difference (one CPU, one build) resyncs that client
again and again; a split without a majority stalls the vote.

**(e) Determinism of the new code.** Arena C or Rust, SSE2, no FMA, no `-ffast-math`, no
x87 in the new code, default MXCSR, no transcendental functions (lerp the position, lerp
yaw with one wrap; no slerp), integer ms with wrap defined once, the engine's values
copied bitwise. One supported client version: each piece that changes the simulation adds
`next<TAB><what>` to `srp-026g/crates/session/src/v026g/simulation_changes.tsv`, and
replays recorded before it are refused.

---------------------------------------------------------------------------

## 8. Backport ranking

Each piece ships alone, smallest first.

**1. Sub-tick hit time for bullets.** Do it.
- What: at the grid step G, the bullet ray already spans [last tick, G]. For each target
  the ray may touch, place the body at the root lerped between its previous grid root
  and G at the crossing ms, test, restore bitwise (#472's swap, 7.3 (c), with the
  "previous root" as the only past slot). The bones stay G's pose relative to the root.
- Gain: removes the 0..50 ms grid lag (mean 12 cm, up to 25 cm at 5 m/s): shots land
  where the crosshair was on a fast strafer. No change to what is drawn, no protocol, no
  server. Behind cover: a target that hid between the crossing and G is now hit (the
  bullet reached him first): at most the rest of one tick (0..50 ms) more, never a rewind
  before the shot.
- Cost: S. One CALLEE around the ray test at 0x32D5E8 (the piercing loops pass through
  it), one root slot a player serialized (16 B), ~0.3 KB of code. Risk: low; determinism
  as 7 (e). A `simulation_changes.tsv` row.

**2. Delayed drawing plus a declared, capped rewind.** Owner's choice; not before 1.
- What: (i) draw each remote player at `now - age(place)` from a client-only ring of his
  confirmed grid roots (interpolated; the renderer may use any data, it is not world
  state); (ii) the shot sends the drawn ages with it (v1: one byte, the same age for every
  place; later a per-place list); (iii) the simulation clamps each age to a cap (100 or
  150 ms, a match option in the static info) and tests that target against the serialized
  root ring of 7 (c) at `t_cross - age`.
- Growth to direct peers without a second path: the simulation always calls
  `age(place)`; v1's declaration decodes to the same value for every place, a later
  declaration carries `(place, age)` pairs. One field, one function, no protocol break
  (the carrier is the same profile input; a version byte in it is enough).
- Gain: what you see is what you hit, and the hit is final at once (no re-simulation can
  undo it, when the age is at least the target's confirmed age). This is 0.69's "pixel
  correct" property.
- Cost to players: about +age more deaths behind cover (5.2: 90 -> ~230 ms in the 30/150
  example); targets drawn older, so more peeker's advantage. Code: M-L (render ring and
  presentation hook, shot declaration, ring, swap, clamp; ~1..1.5 KB). Risk: medium (a
  ring or clamp difference is a desync for everyone; tests in Unicorn with scripted
  shots). A `simulation_changes.tsv` row.
- If the owner rates dying behind cover above hit reliability, stop after 1.

**3. Direct peers** (`peer_prediction.md`, WIP). Fresher remote movement on screen
(115 -> 45 ms in 5.1) for connected pairs; no simulation change. With 2, a hinted target's
age is small, so its rewind and its window shrink. Separate.

**4. 0.69's per-tick protocol and clock steering** (#472 phases 2..3). Your view age
becomes your own round trip; rollbacks are bounded by it; it helps everyone, with or
without a direct route. L, protocol and server work. Not needed for 1 or 2.

**Not worth it:** the 20 ms tick (#472 6); a full-bone rewind as 0.69's 0x200307450 (the
animation tree restore is the heavy part and our history does not keep it cheaply);
compensated explosions (6.6); 0.69's 16-slot collision groups (0.26g has 4 free bits,
#472 7.2; the swap needs none).

**Already equivalent in 0.26g:** shot origin and direction from the simulated player,
exact shot ms, travelling bullets, hits decided by the whole lockstep (no shooter claim).

---------------------------------------------------------------------------

## 9. When it arrived

Strings of each exe (x86 builds from the weapon survey, `out/game/binaries/x86/
survarium.exe`; 0.46c2 and older have fewer diagnostics, so "-" may be a stripped
string). Columns: 026g 027d0 028a2 028d0 031a2 046c2 069d0.

```
recompute_damage_collision_bones_for_time   -  -  -  -  -  Y  Y
interpolate_player_matrices_data            -  -  -  -  -  Y  Y
ping_input                                  -  -  -  -  -  Y  Y
allow_killer_claim                          -  -  -  -  -  Y  Y
st_claim_added                              -  -  -  -  Y  Y  Y
kill_cam / show_kill_cam                    -  -  -  -  Y  Y  Y
interpolating_camera, allow_interpolation   -  -  -  -  Y  Y  Y
remote_player_delay_in_ms (assert)          -  -  -  -  -  -  Y
inserting active player (21 bodies)         -  -  -  -  -  -  Y
netsync_send_window_in_ms                   -  -  -  -  -  -  Y
on_shot_event, weapon_core::instant_fire    Y  Y  Y  Y  Y  Y  Y
```

- 0.31a2: kill camera, its interpolating camera and the complaint notice.
- 0.46c2: sub-tick hit poses, ping as world state, the complaint button on the kill
  camera; that is "unlimited tickrate / pixel perfect hit detection" of 0.42 (LIKELY; no
  0.42 exe on disk). Whether 0.46c2 already drew remote players delayed is not known
  (its x86 code was not read; GUESS: yes, the two belong together).
- 0.69d0: the per-tick protocol, clock steering and client-asked resyncs (#468 4).
- 0.52 ("less input lag between pressing fire and a shot"): no exe on disk; LIKELY
  shorter fire animations to the shot event, since the shot is that event in every build.

---------------------------------------------------------------------------

## 10. Re-kit rows

- `tools/re-kit/v069d/functions.tsv` (29 rows), `abi.tsv` (6), `abi_inputs.tsv` (13):
  the functions of sections 2 and 6, identity-exe VAs. Rows that the uncommitted
  controller-animation research also has (0x2002F1480, 0x200306550, 0x200307450) use its
  names.
- `tools/re-kit/v026g/inferred.tsv`: 0x00395AF0
  `vostok::animation::animation_player::capture_state` (retained tree clone in a 0x7000
  arena, called from base_player::serialize at 0x2DB226; its pair 0x395B50). The other
  0.26g functions named here are already in the kit; 0x1F76B0 is added by #472.
- Data (not rows): 0.69 game_world+0x7B62 view place, +0xF4578 presentation ring,
  player+0x1F2E0 presentation matrix, +0x19E0 presentation exemption, +0x704 spawn time,
  +0x708 death time; type-8 object +0x2C0 view player, +0x314 camera mode, +0x318 kill
  camera phase. 0.26g bullet +0x5C spawn ms, ammo +0x2A4 speed, +0x2AC pellets; camera
  smooth flag 0xA711BB.

---------------------------------------------------------------------------

## 11. Open questions

1. The one tick between the drawn time (`now - (D + tick)`) and the tested time
   (`t_cross - D`): an artifact of when captures are taken (a capture at G holding the
   state after the step to G), or a real 20 ms offset. A run of 0x2002374D0 and
   0x200306550 in Unicorn on one capture pair would settle it.
2. Which branch of 0x200238480 live play takes: whether the camera controller follows the
   local player (view place = local) while alive. Reading the match-start path of
   0x2001E5FD0's callers (0x2001E6660, 0x2002AAB50, 0x2002ADBE0) would settle it.
3. What player+0x19E0 (a constructor argument +0x331, 0x2002FEB18) marks; LIKELY the
   local, controlled player.
4. The retail tick, max remote delay and place mode (+0x4B): server values (#468 7.1).
5. Where `ping_input` is applied to place+0x5C (the readers are proven, the writer not).
6. Whether 0.46c2 already had the delayed presentation (read its x86 0x2002374D0
   counterpart).
