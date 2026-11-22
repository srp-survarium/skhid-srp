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
types in place of the icon; any other type is the icon's frame as before. The frag keeps the stock
glyph. An enemy's grenade still has no marker.

`SmokeMark` is the stock marker's ring and pointer around a white sketch of the RDG-M, made the way
the stock frag glyph (shape 32) is a sketch of the RGD-5 (the owner, 2026-11-22: "white sketch of the
grenade that already exists"):

- **The source**: rdg-m's own icon (`ui_desc/icon` 3, 6, icon 49: the stock hud's hudItems cell 15,
  the same render as the lobby's AmmoIcons one), measured upright (it leans 12.5 degrees): the fuse
  head, the neck, the shoulder, the body to its rim (a band a little wider than the body, 62 to 67 px
  of 79.5), the bottom cap under the rim (a dome 25 px across and 12.5 px tall; the owner: without it
  "it looks flat"), one silhouette with the body, the step in width at the rim showing it (the owner:
  "there is no need for separation"), and the lever's wire down the shoulder's left. The pin's ring is left out, as the
  frag glyph leaves out the RGD-5's.
- **The hand**: the frag glyph's. Its tilt (its fuse rod's line, 31 degrees; the RGD-5's icon leans
  about 15, so the glyph leans about 19 more, and so does the RDG-M's from its 12.5), its height
  (617 twips), its centre in the ring, its strokes (40 twips) and cuts (about 20) between the body,
  the fuse head and the lever.
- `srp-026g/scripts/grenade-icons/smoke_glyph.py` makes the polygons (shapely) from those
  measurements and prints them as `SmokeMark`'s arrays.
- **Toxic smoke** (the owner: "fully green inside"): the same sketch, its body (with the cap) green
  inside a 1 px white edge, the fuse head and the lever white. The green, 0x71AB40, is halfway from Toxic
  Smoke's own icon's (`scripts/technical-skills/make_icons.py` GREEN, 0x7BD11F; the owner: "a bit too
  bright") to the cloud's own tint in the game (the toxic-smoke pack's emissive 0.40, 0.52, 0.38,
  0x668561, too dull for a 20 px mark).

Check: `srp-026g/scripts/grenade-icons/check.sh` (Ruffle, the stock feed and the feed with kinds).

## The game's part: exe patch 194

`grenade_icons` (`rust/patches/src/grenade_icons.rs`, `vostok/grenade_icons.rs`):

- **`grenade_icons.sensed`** (0x2471A4, a lohook of 5 bytes): player::st_draw once a grenade near the
  followed player passed the team test, before it reads its position. ESI the grenade's body, whose
  user is the grenade +1Ch (`game_object.grenade`), EBP the followed player, BL the marker it takes.
  The grenade is read once into a kind: a frag; a smoke (its stack's `splash_max_damage` is 0, the
  test patch 180 uses); a toxic smoke (a smoke whose thrower's kit has Toxic Smoke 441). When the
  marker was last told another kind, `set_hud_icon_type(ui [player+F480h], 2Fh + BL, MarkerType)`
  (0x23BEA0, the game's own, its Invoke root.set_player_icon_type). The type is set in the same frame
  as the position, before the movie draws.
- **`player_icons.enemy_ids`** (order 3): initialize_match is about to make the match's markers anew,
  each a frag's (type 0): what the markers were told is forgotten.
- **A marker reused**: the six markers are filled in the order the sensor finds grenades, so a
  marker moves from grenade to grenade. Every grenade that takes a marker is compared with what that
  marker was last told, so a smoke's marker taken by a frag next is told the frag's again; a hidden
  marker keeps its type until a grenade takes it.
- **A replay, a spectated player**: the sensor runs for the player the camera follows (st_draw's
  test 0x246F5F..0x246FA5: the network client's player, `[[player+F478h]+53ACh]+4`, as the
  first-person test 0x243940 reads it; in a replay the replay client), and the team test is his.
  The hook is inside that path, so a replay's and a spectated player's markers take it too (by the
  code; not run in a replay).
- Presentation only: nothing the match computes reads it. Check: `check_grenade_icons_026g.py`
  (`tools/check grenade-icons-native`).
