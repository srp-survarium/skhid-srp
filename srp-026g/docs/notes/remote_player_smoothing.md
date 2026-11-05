# Other players' corrections drawn smoothly (patch 104)

The owner, 2026-11-05: build "smarter prediction" for other players. This is
its first part: corrections are blended, not snapped. Parts 2 and 3 (what the
prediction carries, fading very late players) are findings only, below.

## What players notice

- Another player whose input reaches you late no longer jumps when the game
  corrects him: he slides to the corrected place over 80 ms.
- A correction of 1 m or more still jumps, as today.
- Nothing else changes: hits, deaths, what the server and the other players
  compute.

## Rules

- Display only. The offset is added to what is drawn; no byte of the world,
  a player, the history or a hash input is written.
- Blend: 80 ms of the world's time, linear, from `old place - new place` to
  nothing. A second correction during a blend adds to what is left and
  starts the 80 ms again.
- Snap (drawn as the game does, at once): a correction of 1 m or more
  (with what is left of a blend), death, a new life (respawn: the life is
  the spawn time +328h), the local player, the player the camera looks from
  (the input handler's, +F470h: the local player, or the one a spectator
  watches), a player not inserted. A correction under 1 mm is none.
- 0.26g has no teleport but the respawn, and no vehicle or ladder state
  (LIKELY: no such code was found); a teleport would show as a correction
  of a metre or more, or a new life, and snap.
- On for everyone; no setting.

## How it works

- Measured: `history.grid_saved` (game_world_core::tick_events 0x2D230F,
  just after serialize saved a grid point) records each place's player
  place (+43Ch) at that time. A rollback steps the history again over the
  same times; where a time saved before is saved again, `old - new` is that
  player's correction. The newest such time in a frame is the one used.
  Re-simulating with the same inputs gives the same place: no correction.
- Decided: once a frame at `game_world.after_move` (after the render move,
  before the draws), one state a place: `Steady`, or `Blending { life,
  from, started }`, and one transition `step(state, now, frame)`.
- Drawn: `player_draw.bones` (mt_draw 0x247716, after the ragdoll's
  capture) adds the offset, in the model's space, to every bone of mt_draw's
  own array; `name_tag.eye` (render_name 0x1F2B98) gives the tag a copy of
  the eye moved by the offset.
- Corrections smaller than one tick: an input whose time falls after the
  newest grid point re-simulates no saved point, so its correction (at most
  50 ms of movement, about 25 cm) is still drawn at once.

## What is drawn with the offset

| Thing | How | Covered |
|---|---|---|
| The body | mt_draw's bone array (model space) | yes |
| Its weapon or held item | mt_draw poses it from the Weapon bone of the same array (0x247763) | yes |
| Muzzle flash | weapon::draw computes the muzzle locator from that transform (+620h, 0x244E61) and moves the weapon's effects there (update_pfx_transform 0x244570); a shot starts its effect at the locator +660h (0x2443EE) | yes (PROVEN in the code; not run) |
| Shadows | the same render instance and bones | yes |
| Name tag | render_name's eye (+44Ch) copy | the tag's place yes; its second point (the feet, +43Ch, 0x1F2DF3) no |
| Kill / death camera | your own corpse's eyes (patch 46): the local player, never smoothed | yes, by the rule |
| Spectator camera | the watched player holds the input handler, so he is never smoothed: camera and body agree | yes, by the rule |
| Ragdoll start pose | the ragdoll captures the bones before the offset, and a death snaps | yes |
| Tracer start | the bullet's simulated origin (bullet manager; the sub-tick hits work's area) | no: up to the offset, for at most 80 ms |
| 3D sounds (steps, shots, voice) | placed by the simulation at its time | no: up to the offset, for at most 80 ms |
| Hit model, hits | the simulation's | untouched, by design |

## Why it is presentation only

- The grid save, the frame and the name tag hooks read the game and write
  only the patch's own state (`patches.state`, `patches.data`).
- The bones hook writes mt_draw's stack array alone, which only the draw
  reads (update_skeleton copies it; the held item's transform is made from
  it; the hit model is posed from the core skeleton, +408h, by
  recompute_damage_collision_bones, not from it).
- The name tag hook points ECX at a copy; the player's eye is not written.
- The world CRC (0xDF) is the CRC-32 of the serialized event-horizon item
  (sync_core.md 1.2); nothing of patch 104 is serialized or written there.
- `check_remote_smoothing_026g.py`: the whole made-up memory (world core,
  players, saved item) bitwise the same across every hook, but mt_draw's
  own array at the bones.

## Parts 2 and 3: what the prediction carries, fading late players

Not built. The condition (speculative state only) does not hold on main:

- A remote player's last input is world state. apply_new_input (0x2D86E0)
  copies an input node into the player (+32Ch rotation, +330h actions) once,
  at the node's time; every tick after uses that until the next node. The
  ticks are saved into the history items as they are stepped.
- One-off actions are edge-triggered where the code compares with the input
  before: use (bit 21) only when it was clear before and is set now
  (0x2D88C3..0x2D88DF). The held bits (movement, fire, aim, sprint, crouch)
  act every tick while set (LIKELY per bit: their consumers were not all
  read); bit 28 acts while set with a 3 s cooldown (+34Ch, 0x2D8787..0x2D87C1).
- Inputs are sent on change. The client cannot tell "no news" from "late":
  there is no per-place "confirmed up to" time in 0.26g.
- The CRC is the event-horizon item's (core +18FB4h, 500 ms behind the
  newest time; try_send_all 0x27CFB0 sends its 0xDF at 0x27CD7A). That
  item is re-simulated only when an input older than it arrives. An item
  stepped with a predicted input that
  no later input corrects becomes the horizon item as it is. So a predictor
  that changes the input the world steps reaches the confirmed world and the
  vote.
- So parts 2 and 3 need the projection of the P2P work
  (`peer_prediction.md`, `client-runtime::projection`): insert the predicted
  inputs before the render move and remove them before every confirmed move,
  send and hash. That is the Rust client transition's (PLAN-RUST-CLIENT.md).

What a predictor plugged into that projection would take and return:

- Takes, per remote place: its received inputs (the history's input nodes,
  core +2FCCh + place x 480h, match.md 5.2: each the 8-byte input, yaw and
  pitch as half floats and the action mask, and its time), so the last
  inputs and their times, how long each bit has been held, the look; the
  time now (core +18FC0h), so the time since the last input; what the
  simulation keeps of the player (+330h the actions applied, the stance by
  the crouch bit, alive +33Ch, the life +328h).
- Returns: the input to step with at a grid time after the last node (the
  8 bytes), the hold-last baseline returning the last node's input.
