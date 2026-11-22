# Environment probe fixes (the owner, 2026-11-15)

"Lighting problems" in five screenshots, on London and Vector Laboratory. All five are environment
probes; no ambient light, light or environment setting is involved. Two of them only show once a
lamp is shot out (`breakable_lamps.md`): the lamp's own light covered them.

Predictions of the ambient term before and after each fix, beside the owner's screenshots, are in
`private/renders/map-probes/` of the main checkout (untextured collision mesh, not game captures).
None of this is played in a game yet.

## London: the window reflection probes (pack london-probes)

Screenshots 20-43-49 (a bright patch with a sharp edge beside a window), 20-43-58 (a glowing
boarded-window wall) and 20-44-40 (a halo round the wall hole, bright strips beside the boarded
windows): London's ten box probes `reflection_window`, `(1)` to `(9)`, with `affect_diffuse = false`,
for which 0.26g's shader wrote the surface's albedo into the ambient light. This list first removed
them; pack `london-probes@2` (`london.md`) now draws them as 0.31a2 does, their reflection only, so
they stay.

## London: the purple corridor (`london-probes-1.json`)

Screenshot 20-45-05. The corridor's room probe `gerkin(4)` has the fluorescent lamp's lavender light
baked into its cubemap (mean diffuse 0.091, 0.077, 0.097; its neighbour `gerkin(3)` 0.037, 0.034,
0.033). The lamp (`fluor_lamp(9)`) is shot out in the picture: its light is gone, the baked tint
stays. The list gives `gerkin(4)` its neighbour's cubemap
(`environment_probes/level_09/probe_day_default_69818708879831187022`), in rain and evening (pack
`london-weathers`) that weather's own `gerkin(3)` cubemap: all eleven London projects. The engine
reads only the `texture` key (`object_environment_probe::load`, 0x28F48D); the `textures` table is
editor residue.

With the lamp intact, the lamp's own light still tints the corridor; only the baked share goes.
Four more London probes hold a fluorescent lamp's baked light (`hall_2_floor(11)`, `gerkin(5)`,
`hall_2_floor(4)`, `hall(2)`); they are untouched until someone reports them.

The ported Londons (`london-ports-probes-1.json`) came back with it (2026-11-22, v0.32 rain_ah,
screenshot 06-02-41): 0.32a4 and 0.44a5 ship the same bake under the same names, and their lamp
(`lamp_sparks(4)`, the old `fluor_lamp(9)`) is breakable too. The same edit, each project's own
`gerkin(3)` cubemap, in every project of `v032a_level_09` and `v044a_level_09`.
On London (v0.32) the list also gives the office probe `gerkin(5)` (one of the four above; screenshot
06-30-46, rain_4's office with the glass partitions) the office's other probe `gerkin(13)`'s cubemap:
its lamp `lamp_sparks(5)` is baked into it the same way. 0.44a5 baked `gerkin(5)` anew and its
`gerkin(13)` warm, so London (v0.42)'s stays.

## Vector Laboratory: the bright band (`vector-probe-1.json`)

Screenshot 20-22-55, the room at (-18.7, 17.2, 4.0) with its fluorescent lamp shot out: a bright
band on the wall, floor to ceiling. Stock data. The room probe `indoor(9)` (a box, radius 0.4, drawn
last) ends at x = -16.4; the room's east wall is at -15.4 and `indoor(24)` starts at -14.3. In the
gap, north of z 5, only the outdoor probe `global` applies (irradiance 0.27 against the room's
0.016), on the wall, the pillar and the mouth of the passage where the lamp hangs.

The list moves the box's east face to -13.89 and keeps its west face (position x -23.997 to -22.743,
scale x 7.596 to 8.85), in the 15 projects. What it newly covers is this room, its two passages
east and solid wall; its only reach into another space is a slice 1 m deep at z -7 to -9.8, where it
replaces `indoor(24)` (0.03) with 0.016.
