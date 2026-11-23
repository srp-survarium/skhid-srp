# The kill marker of a kill made in a re-simulation (patch 191)

The owner, 2026-11-22, playing rev70: "red crosses seem to not appear when person is killed
behind cover (but I am not sure)". The red cross is the kill marker on the crosshair
(`hide-hit-markers-scores.md`). Patch 167's white marker (`hit-marker-walls.md`) is not the cause.
The original game loses the kill marker of a kill that it first simulates in a re-simulation.
Exe patch 191 (`client-patches/rust/patches/src/kill_cross.rs`, `src/vostok/world_event.rs`) fixes
that.

## The game (PROVEN, the 0.26g exe)

- **The world's events:** `game_world_core::new_world_event` (0x2D43B0) adds a node to the core's
  list (+12744h): +0 the victim, +1 the killer, +18h the type, +24h the time. There are three
  types: 0 a kill (generate_killed_event, 0x2DA3A8), 1 a victory item's user (a battery picked
  up or dropped, 0x300B4A, 0x301004), 2 a hit on a player (generate_hit_event, 0x2DA054). A world
  load drops the events after its time (0x2D3700), and the re-simulation makes them again.
- **The kill marker:** game_world's tick (0x1F7FF6) gives `process_kill_cross_events`
  (0x1FB6C0) the newest ticked time as its bound (game_world+560h, the delay, is 0 and nothing
  writes it) and the last kill-cross time (game_world+564h). The walker skips the events at or
  before the last time and gives every later one to `on_kill_cross_event` (0x233770). That draws
  the marker for a kill by the drawn player (`show_enemy_killed_marker` on, 0xA70EC6) through
  0x23CF20, `root.crosshair_enemy_hit(1.0, 200, true)`. Then, for an event at or below the bound,
  it writes the event's time into the last time (0x2337FA..0x233804), whatever its type.
- **Other users of the last time:** none. Only the walker reads it (0x1F7FE6). Only 0x233770
  writes it during a match. The hud's constructor (0x233BAB) and its reset (0x23708B) set it to 0.
  The kill feed has its own walker (0x1FB660) and its own time (game_world+4D4h), 500 ms behind.
- **The loss:** the last time is not rolled back with the world. A kill that this game first
  simulates in a re-simulation gets no marker if any event at or after the kill's time had
  already been walked before the load. That includes a hit by anyone, a battery's user, and the
  same shot's hit when it was not lethal before the load. This happens when the victim's or
  another player's late input made the hit lethal, or made it hit at all. The kill feed still
  shows the kill. With patch 167, such a kill through a wall shows nothing on the crosshair. Before
  it, the original showed the white bars of the predicted hit.

## The patch

- **One lohook,** `kill_cross.walked` (0x2337FA, the sealed test before the write; `kept` at
  0x233806, past the write). It lets the write happen only for a kill (type 0). Any other event
  leaves the last time as it was.
- **Kept:** the marker's own test (the killer, the option), the walker, the bound, the write for
  a kill. Any player's kill still moves the last time, so watching another player (a replay, a
  spectator's switch) never shows his older kills.
- **A kill walked twice:**
  - A kill already shown, dropped by a world load and made again at the same time, is at the
    last time, so it is skipped: one marker, as before.
  - Made again at an earlier time, it is skipped. That kill was already shown.
  - Made at a later time: the load undid it (the late input saved the victim from that bullet)
    and a later bullet killed him. That second kill is a real kill at a new time. It shows a
    second marker, as the original does. On screen the same red cross starts again a few frames
    later and stays up its 200 ms from there. This is not changed.
- **The limit left:** a kill made at or before another player's kill that was walked before the
  load still gets no marker. It is rarer, and the original loses it too.
- **The walk:** the events after the last kill are now given to 0x233770 every frame, not only
  once. A hit or a battery's user returns at the type test. The walker already reads the whole
  list from its head each frame, so it does no more work than before.
- **Display only:** nothing a match computes changes (the last time is the hud's, game_world+564h,
  not the world's). No world format change, no `simulation_changes.tsv` row.
- **Checks:** `../../client-patches/check_hit_marker_walls_026g.py` (`tools/check
  hit-marker-walls`, Unicorn): the walker, 0x233770 with the hook and 0x23CF20 up to the invoke, in
  both exes, on event lists as a world load and its re-simulation leave them. Covered: the
  predicted hit then the kill made again at its time, another player's later hit, a battery's
  user, a kill at a new time once and not on the next frame, a shown kill made again at its time
  or later, a kill before another player's kill. In every run the registers are the original's.
  Not yet seen in a game.
