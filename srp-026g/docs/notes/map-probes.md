# Environment probe fixes (the owner, 2026-11-15)

"Lighting problems" in five screenshots, on London and Vector Laboratory. All five are environment
probes; no ambient light, light or environment setting is involved. Two of them only show once a
lamp is shot out (`breakable_lamps.md`): the lamp's own light covered them.

Predictions of the ambient term before and after each fix, beside the owner's screenshots, are in
`private/renders/map-probes/` of the main checkout (untextured collision mesh, not game captures).
None of this is played in a game yet.

## London: the window reflection probes (`london-probes-1.json`)

Screenshots 20-43-49 (a bright patch with a sharp edge beside a window), 20-43-58 (a glowing
boarded-window wall) and 20-44-40 (a halo round the wall hole, bright strips beside the boarded
windows).

London has ten box probes `environment_and_lights/probes/reflection_window`, `(1)` to `(9)`: slabs
36 m long and 4.5 m deep along the window facades, with `affect_diffuse = false`. None of the 1,857
probes of the stock maps has that. 0.26g's shader for it
(`environment_probe_lighting.ps`, the `affect_diffuse` 0 permutations) has no diffuse cubemap and
writes the surface's albedo (`t_diffuse`, bound at 0x18A4AA) into the ambient diffuse buffer under
the box's falloff. So a white wall inside a slab takes its own colour as ambient light wherever no
room probe is drawn over it: probes draw largest `radius` first (0x1BA0C7), and 156 of London's 160
have radius 5, so the order between a slab and a room probe is a tie.

The list removes the ten probes from the five projects (`remove_entry` by name). What goes with
them is the window-glass reflection they were meant to add. `enabled = false` would also keep a
probe from being drawn (`stage_visibility` drops it, 0x1B9B22), but `content_edit.py` writes a bool
only through an index path.

## London: the purple corridor (`london-probes-1.json`)

Screenshot 20-45-05. The corridor's room probe `gerkin(4)` has the fluorescent lamp's lavender light
baked into its cubemap (mean diffuse 0.091, 0.077, 0.097; its neighbour `gerkin(3)` 0.037, 0.034,
0.033). The lamp (`fluor_lamp(9)`) is shot out in the picture: its light is gone, the baked tint
stays. The list gives `gerkin(4)` its neighbour's cubemap
(`environment_probes/level_09/probe_day_default_69818708879831187022`). The engine reads only the
`texture` key (`object_environment_probe::load`, 0x28F48D); the `textures` table is editor residue.

With the lamp intact, the lamp's own light still tints the corridor; only the baked share goes.
Four more London probes hold a fluorescent lamp's baked light (`hall_2_floor(11)`, `gerkin(5)`,
`hall_2_floor(4)`, `hall(2)`); they are untouched until someone reports them.

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
