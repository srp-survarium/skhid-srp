# No white hit marker through walls (patch 167)

The owner, 2026-11-21: "white hit crosses are not seen when shot through walls (except for
leaves)", "red ones are still shown when killed"; then "Through a wall, kill shows exactly what
'Kill marker only' shows"; then "should show through glass, foliage and cloth". Exe patch 167 (`client-patches/rust/patches/src/hit_marker_walls.rs`,
`src/vostok/hit_marker.rs`).

## The rules

- **The marker:** the white bars around the crosshair on a hit on an enemy
  (`root.crosshair_enemy_hit`, `Crosshair.setDamage`; `hide-hit-markers-scores.md`).
- **Through a wall:** a hit that does not kill, by a bullet that went through a wall before the
  body, shows no marker: as "Kill marker only" treats a hit.
- **A kill through a wall** shows exactly what "Kill marker only" shows: the whole kill marker,
  white at the centre and the red ends, none of the hit's own white bars.
- **A wall:** the level's collision and a prop's, of a material whose `armor` (game.materials) is
  above 0: walls, floors, wood, metal, cars, trees' trunks, glass_hard (0.10). A material of armor 0
  is not one: in the live data glass, glass_car, foliage, cloth, plastic, paper. It is the
  piercing's own rule: try_pierce lets a bullet through armor 0 for nothing (k = 1, patch 175).
- **Not a surface:** another player's body, a trap, a lamp (what a bullet hits as an object).
- **Unchanged:** a hit with no surface in between or only surfaces of armor 0; a ricochet off a wall straight
  into the body; a gun-stock hit; grenades (the game gives them no white marker).
- **Shotgun:** the pellets of one shot that hit the same player count together: one that reached
  him clear shows the marker, with all the shot's damage. So does a clear hit not shown yet when
  a hit through a wall follows it.
- **The next hit** that is not through a wall shows the marker as before, its size all the
  damage collected on that player.
- **The hit marker list** (Settings, HUD): All is the rules above; Kill marker only and Hidden
  as they were.
- **Unchanged too:** the kill feed's wall icon, the marks of where the player is hit from, every
  sound, what other players see.
- **Replays:** the replay's hud follows the same rules.
- **Nothing a match computes changes** and nothing in the worlds: no `simulation_changes.tsv` row,
  no world parser change.

## How

- **The game** (PROVEN, the 0.26g exe): a bullet's hit on a player goes `bullet::try_pierce`
  (0x32D960) -> hit_impl (0x2D9B90) -> `player::generate_hit_event` (0x1F3C70), which adds the
  pain the victim lost over pain's maximum to the shooter's `inflicted_damage_collector` (player
  +0F534h: the victim's place, the time, the fraction, at most 1; reset for another victim, and by
  player::tick 0x246F10 some time after the last hit). It is world state, saved with the player.
  `player::st_draw` gives the drawn player's collector to `game_world_ui::draw_damage_collector`
  (0x238230, at 0x247467), which calls `root.crosshair_enemy_hit(fraction, 200, false)` when the
  victim or the fraction changed from its copy. Only bullets reach the collector (0x1F3CAA).
  The kill marker is another call, `crosshair_enemy_hit(1.0, 200, true)` (0x23CF20, the kill
  event's), which the movie draws the same with All and with Kill marker only (#2022).
- **A bullet's surfaces:** check_collision (0x32D4E0) gives try_pierce the triangle's material
  (bullet +4Ch) and the hit receiver the ray met (0 for the level); try_pierce reads the
  material's armor at its +4Ch (0x32D980). The bullet keeps only counts (+84h, +88h), so the
  patch notes what it went through.
- **The patch** (three lohooks, `sites_026g.tsv`):
  - `bullet.pierced` (0x32D88A, check_collision once try_pierce let the bullet through): a round
    (ranged ammo) through a surface whose material's armor (its +4Ch, read from the game as
    try_pierce reads it) is not 0 is noted by its key
    (its id and fire time, +4 and +5Ch, which a reloaded world gives it again), the last 128;
  - `damage_collector.hit` (0x1F3D76, generate_hit_event's store into the collector): the
    shooter's mark, by place: a clear hit pending its draw, or walled (the collector, the victim
    and time it shows) when the bullet was noted and no clear hit is pending;
  - `damage_collector.draw` (0x238272, draw_damage_collector once the collector changed and it
    took its copy): a walled mark of what the collector shows skips the marker's call; a pending
    clear hit is drawn and no longer pending. The copy is the game's, so the next change is seen
    as in the original.
- **Rollback and re-simulation:** the marks are this game's alone, kept beside the world; a
  re-simulated hit makes its mark again from the same bullet's key.
- **Limits:**
  - a hit's mark follows the bullet only from the moment this game ran the pierce: a world
    received with the bullet already through the wall (a late join, a resync in its flight) shows
    that bullet's hit;
  - the marks are not rolled back with the world: a re-simulation can leave a clear hit pending
    where it no longer is, and a walled hit re-simulated after it then shows the marker.
- **Checks:** `../../client-patches/check_hit_marker_walls_026g.py` (Unicorn: the three sites in
  both exes, the marker the hud gets in each case above, the registers the original's, the kill
  marker's and the draw's bytes but the hook's the original's), Rust tests, the hud movie's Ruffle
  harness (`../../scripts/hud-options/check.sh`, its HudHarness: a kill after a hit's white bars
  with All draws what Kill marker only draws). Not yet played.
