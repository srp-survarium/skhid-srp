# Markers over thrown grenades

A player's suggestion the owner passed on (2026-11-22): "unique icons for an allied ordinary
grenade, a smoke one and a poison one". Renders: `private/renders/allied-grenade-icons/`.

## What the stock game draws (PROVEN in the code of the original exe)

The only place a thrown grenade is shown is a world marker of the player icons movie
(`player_icons.swf`, `MarkerRenderer`): the HUD, its minimap and the kill feed show none.

- **The markers.** `game_world_ui::initialize_match` makes six grenade markers, ids 2Fh..34h (47..52):
  `create_hud_icon(id, 1, grenade_icon_size)` (0x235809, a marker, scale 0.5) and
  `set_hud_icon_type(id, 0)` (0x235819, the icon's frame 1). Their colour row is white (0x2357C3).
  The traps' markers follow from 35h with type 1 (0x23587E, frame 2). The icon's frame 3 (an ammunition
  box) is used by no caller.
- **Which grenades.** `player::st_draw` (0x246F50), for the player the camera follows only (0x246FA5),
  asks the physics world for the objects around him (0x2470B0) and takes, up to six, every grenade
  (0x2470EF) within `grenades_sensor_radius` (7 m, 0xA74614; 0x24715B) whose thrower's team is his own:
  grenade +0Ch its stack (`grenade_set_core`), the stack's owner +250h, +174h, its player, the
  profile +6B8h's team +45ECh compared with his (0x24719C). His own grenades are his team's. An enemy's
  grenade is never sensed: it gets no marker.
- **What the movie is told.** `show_grenade_icons` (0x23C3D0, from 0x2471FC) gets the positions only:
  marker i at grenade i, the rest hidden. `place_object_icon` (0x23C750) places it on the screen with a
  scale 1..3 and an alpha 128..255 by the distance from the camera (`icon_name_min_size`/`max_size`,
  `_near_dist`/`_far_dist` 1..100 m, `_min_alpha`/`_max_alpha`): about 2.9 and 250 within 7 m. No kind
  and no side is sent, so every marker is frame 1.
- **For how long.** Every frame while the grenade's body is in the physics world and within 7 m of
  the followed player: from the throw, LIKELY until it goes off (not traced: the blast taking the body
  out; a smoke grenade's cloud is a particle effect, no object of its own, technical-skills.md).

| thrower | frag (RGD-5) | smoke (RDG-M) | toxic smoke (RDG-M, Toxic Smoke 441) |
| --- | --- | --- | --- |
| oneself | frame 1: the frag's glyph | the same frag's glyph | the same frag's glyph |
| a teammate | frame 1: the frag's glyph | the same frag's glyph | the same frag's glyph |
| an enemy | nothing | nothing | nothing |

The glyph: a white egg-shaped grenade with its lever, in a ring 43 px across (a white disk at 6 %,
its edge at 30 %) over a small white pointer, about 41 x 50 px on the screen.

The game data has two grenades (category 24): `rgd-5` (frag) and `rdg-m` (smoke). The "poison"
grenade is an RDG-M thrown by a player whose kit has Toxic Smoke (441, exe patch 180: its cloud green
and harmful; technical-skills.md).

## The movie (srp)

`survarium.constants.MarkerType` names a marker's types: `GRENADE` 0 (the stock frame 1, the frag),
`TRAP` 1, and srp's `SMOKE` 3 and `TOXIC_SMOKE` 4. `MarkerRenderer` shows `SmokeMark` for the two smoke
types in place of the icon: the stock ring and pointer, an upright canister with its fuse, lever and
pin ring, and a cloud rising from the fuse; white, the toxic smoke's cloud green (0x9BE15D). Any other
type is the icon's frame as before. The frag keeps the stock glyph. An enemy's grenade still has no
marker.

Check: `srp-026g/scripts/grenade-icons/check.sh` (Ruffle, the stock feed and the feed with kinds).

## The game's part (not written)

The stock game never tells a marker a grenade's kind. It needs an exe patch: where `player::st_draw`
takes a grenade for marker i (0x2471A4, the grenade at [ESI+0Ch], i in BL, the player in EBP), the
grenade's kind (a smoke grenade: its stack's `splash_max_damage` is 0, as technical_smoke's test; toxic:
its thrower's kit has 441), and after `show_grenade_icons` (0x2471FC) `set_hud_icon_type(ui, 2Fh + i,
kind)` (0x23BEA0) for each marker whose kind changed since the last frame. Presentation only: nothing
the match computes changes.
