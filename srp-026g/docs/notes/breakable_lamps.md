# Breakable lamps

Shooting a configured lamp breaks it for the rest of the match: its intact
model and glow cone disappear, its lights are removed, and its broken model
appears, with glass shards and a glass-breaking sound. One bullet is enough,
and so is a blast: a grenade or a booby trap breaks every lamp its splash
reaches (below). A new match restores every lamp. The lamp does
not change the bullet's trajectory or the map's collision mesh.

The content list covers levels 02–08, all their day/evening/rain variants
and modes 2–5, London (level_09) in modes 2–5, plus the v0.11 Vostok and
Chemical Plant and v0.21 Vostok and Mamayev Kurgan day variants in modes 2
and 4: 96 projects, 2,649 fixtures and 857 linked lights. It covers bare
bulbs, ceiling lamps, white wall lamps, red alarm lamps, green lamps
with/without wires, fluorescent fixtures (tubes, ceiling grids, hanging
double tubes) and lit kerosene lamps. Streetlights and other fixture models
are not yet configured.
Fixtures with no glass to break stay scenery: empty fluorescent fittings
(`fluor_lamp_base`, `lamp_big_03`, a `lamp_midle_04` without tubes) and
v0.11's `lamp_midle_0N`, older meshes than the stock broken housings. It
does not darken baked lights, sunlight or a room's ambient lights. A lit
alarm lamp's red glow, an ambient light of its own, goes out with it
(below), and so does a tube lamp's share of the room probe its light is
baked into, on London (v0.32) and London (v0.42) (Room probes, below).

## Content

`scripts/content-edit/breakable-lamps-1.json` is the placement source, registered
in `release/content.tsv`. `scripts/breakable-lamps/make_edits.py` authored it
from the manifest's base projects plus Research, using model bounds from
those same layers (including the ported models).
Its input is every project as the manifest's rows above the list leave it
(the base, the packs, and the earlier edit lists: the 0.21 weathers' own
geometry, `v021d-weather-geometry-1.json`, comes just before it). A pack that
copies a lamp day into another weather (London's rain and evening, the 0.21
maps' rain and evening) carries the day's broken models and table: the
generator authors such a project over them and replaces its table
(`remove_key`, then the new value) only where the rule now binds otherwise
(London's weathers, Fort's and Chemical Plant's v0.21 weathers: their
kerosene lamps, and London's battery modes' extra hanging lamp).
The list adds the broken visuals and a project-root `srp_lamps` table:

- `version = 3`, `break_material`: the glass material id read from
  `game_materials/game.materials` during authoring, never a runtime constant;
  `effect_material`: the `bullet` material's id, read the same way. Every
  break plays the pair `effect_material` x `break_material` (bullet x glass:
  `fx/material_hits/glass`, 50+-15 shards and a crack sprite, and
  `glass_hits`), whatever part of the fixture was hit and whatever broke it.
- `lamps`: a named table of fixtures; its binary-config order
  defines the snapshot order identically on every client.
- Each fixture has world-space `min_x/y/z`, `max_x/y/z`, a `visuals` table
  (`index` in the final render-visual array, `state`: 0 intact, 1 broken), and
  a `lights` table (exact positions, with names retained for review), and
  its glass line `glass_from_x/y/z`, `glass_to_x/y/z`: the longest local axis
  of its longest glass part, through that part's centre, transformed with its
  placement as the light tips are (parallel tubes of that length averaged; a
  tube model's at its tubes' height, `TUBE_Y` in `make_edits.py`). A turned
  fixture's glass follows its tubes, not a world axis of its box.
- Nearby light/cone assignments are explicit in the list, not guessed at
  runtime. Authoring rejects ambiguous assignments. Overlapping bulb and cover
  meshes, such as level_06's ceiling fixture, are one lamp.
- A lit kerosene lamp (London 11, Tarakanovsky Fort 9, level_04 1, and the
  0.21 ports' copies of the models: Fort v0.21 9, Chemical Plant v0.21 1; the
  owner's `2026-11-15-20-44-11.png`) is three things: a frame
  (`stand_group_batlamp`, standing or on a rope, or `wall_group_nailed_batlamp`),
  its lit glass, a model of its own (`..._glass_emit`), and its flame, a
  particle object (`fx/lights/oil_lamp`) inside the glass, which lights the
  place too: there is no light object. The glass is an intact visual with no
  broken one and the frame a housing, as the lit tubes above; the flame
  within 0.4 m of the fixture is listed in a `particles` table, as `lights`
  (`LANTERN_GLASS`, `FLAME`, `FLAME_REACH`). A break hides the glass, puts the
  flame out and bursts along the glass (0.27 m: two bursts). A frame without
  the lit glass is an unlit lamp, scenery. A survey of every map project
  (2026-11-20) found three more flame fixtures, all scenery: the Fort's (and
  Fort v0.21's) one hanging chain lamp (`ceiling_group_chain_lamp`), whose
  flame burns in a model with its glass in its own mesh, so there is no glass
  to hide and no broken model; `stand_group_oil_lamp` (level_02, the Fort,
  School v0.21), never lit (no flame); and the Fort's wall torches
  (`wall_group_torch_emit`, six on each map), an open flame with no glass.
- A lit alarm lamp (`alarm_red_light`) has no light object: it glows through
  a red ambient light beside it. Its row lists it in an `ambient_lights`
  table, as `lights` (exact positions, names for review); the other rows have
  no such table. Authoring gives a lit alarm fixture every red ambient light
  within 2.5 m of it (`LIT_ALARMS`, `ALARM_GLOW_REACH`, `is_red`): Vector's
  one (0.6 m away) and level_05's six at five lamps (up to 2.4 m), in all
  their variants. The rooms' grey and coloured fills near other lamps are
  not a lamp's and stay. The owner's `2026-11-15-20-23-12.png` (Vector, the
  alarm lamp at 10.68, 15.37, -6.34 broken, its red light left) was this.
- Lit tubes that are models of their own (`fx/lamp2` and its two flicker
  variants, three in a tubeless `lamp_midle_04` housing on Vector and London)
  are intact visuals with no broken one: breaking hides them. The housing is
  part of the fixture's bounds, so a shot into it breaks the tubes, but is
  not bound: it stays. A housing without such tubes is not a lamp.

A broken model is its intact model without its glass, nothing else: the
other triangles keep their vertices, normals, UVs and material, on every LOD.
The pack `breakable-lamps@2` (`scripts/breakable-lamps/make_pack.py`) makes
them from stock models, recomputes bounds and vertex/triangle counts and
disables model collision; `stock.json` pins every input.

- `srp_lamp_broken`: the bulb's socket, the triangles above its last ring
  (local y -0.0578); the glass below goes.
- `srp_plafon_lamp_broken`: the stock unlit `plafon_lamp` whole. It is metal
  only (cap, rings, cage): its glass dome is `plafon_lamp_on`, a model of its
  own, and v0.11's `plafon_lamp` is this model plus that dome. Version 1 cut
  it at the cap's lip, which dropped the cage and, by 0.3 mm of rounding, the
  cap's down-facing faces: seen from below, the broken lamp drew nothing.
- `srp_lamp_midle_01/02/03_broken`, `srp_lamp_big_01_broken`: the fluorescent
  housings without their tubes and broken tube ends. Each tube is a connected
  part of its own, textured from a tube region of the `lamp_school` atlas.

White and alarm lamps use the stock
`white_broken` and `alarm_broken` models; green lamps and fluorescent tubes
use their empty fittings. Imported fixtures use their ported replacement
models, except for the generated bulb sockets and ceiling caps. Overlapping
lit/unlit covers share one replacement instead of drawing it twice.
Build this resource-porter pack into
`$SKHID_PACKS_DIR/breakable-lamps-2`, then use `release/build-content` normally.
No release patch is assembled by the authoring tools.

## Bottles

The owner (2026-11-26, `2026-11-26-03-48-25.png`: School's round table at
17.0, 19.5, 33.7): "there are bottles in school which can be breakable". The
three `props/kitchen/bottle_vodka` there (two lying, one standing, within
0.3 m of one another) are fixtures of the project's lamp table, as a lit
tube is: the intact visual alone, no broken model, no lights. Their
collision is `glass` (the model's `coll_GLASS_s`, material 29), the
project's `break_material`, so a break plays School's glass pair (bullet x
glass) along the bottle (0.3 m: two bursts, its longest local axis).

That spray reads as a break on a lamp up by the ceiling, not on a bottle at
arm's length (the owner, 2026-11-26: "it looks like it disappears instead of
shattering", "there should be proper shards and destruction of the
bottle"). A bottle breaks into eight shards, its row's loose pieces of kind
`shard` ("Loose pieces"): pack `bottle-shards@1`
(`scripts/bottle-shards/make_pack.py`) cuts the bottle's surface (49
vertices, 58 triangles, flat sides 17 cm tall) to triangles of at most 2 cm,
each vertex's position, UV and three attribute vectors interpolated, and
groups them into the neck and seven body pieces (the nearest of two rings
of points, jittered a triangle so the edges are jagged), each doubled with
its winding reversed and its vectors negated so it shows from either side,
its origin its centre. The list places the shards after the project's last
visual (no index an earlier list bound moves), as the bottle but at each
one's centre.

The bottles' collision objects (`collision`, `walker_collision`) are taken
out of the projects ("after that there should be nothing on the table and
bullet holes should disappear"): a hidden bottle's would stay, the glass
hits playing in the air and a player bumping into nothing. A shot breaks a
fixture its ray crosses, clipped at the first collision it hits, which is
now the table's; no glass mark is ever drawn on a bottle.

`scripts/content-edit/breakable-bottles-1.json`, made by
`scripts/breakable-lamps/make_bottles.py`, adds the rows `bottle0..2` (in the
order of the project's visuals) to `srp_lamps/lamps` of School's 21
projects (day, evening and rain: modes 2 to 5, 5b, Free for All's 8 and
Artefact Hunt's `_ah`) and School v0.21's 10 (`ported/v021d/` copies of the
model, their own bounds); the lobby's two bottles stay (no lamp table). A
row's box holds the model's origin too, its base 5 mm under the mesh, as
`check_built` reads a fixture's placement inside its box. Bottles stay out
of `make_edits.py`'s lamp rule (`PARTS`), which merges touching models into
one fixture: one shot would break all three. `BOTTLES` lists the models for
`check_built`. The list is the manifest's last edit row: its indices are
those of the final `render_visuals` (authored over a quick build's patch on
players' files), and build-content's lamp check refuses a build where a
later list moves them.

The table is named, so its fixtures' order in the history is its keys' CRC
order: the bottles take places among the lamps, and these maps' worlds save
three lamps more (`simulation_changes.tsv`). Their collision gone, a bullet
through a bottle is no longer stopped or slowed by its glass.

## Rows of their own

A row may say its own `break_material`, `effect_material` and `leftover`
over the table's (each left out: the table's): a skeleton's cap among
Vostok's lamps marks as stone and leaves its body, a tin can among School's
plays its metal hit, a wash-stand tank among Rudnya's figures says glass
(`lamps.rs`'s `Look`, one a row). The client's marks rule is a row's: a mark
of a row's material inside its bounds is skipped while its leftover says so.
`check_built` reads a row's leftover against its fixture (a figure's body,
every other glass). The later lists below add rows to tables an earlier list
made, their new visuals at the end of `render_visuals` (`content_edit.py`'s
`add_entry` with `"at_end": true`: Rudnya's Research projects end in two
transmitter slots at one place, which `"after"` would match twice), by
`scripts/breakable-lamps/fixtures.py`; entries alike across a level's
projects are one, its files `projects/<level>/*/client_project`.

## Skeleton caps on every map

The owner (2026-11-27): the cap on every map, not Rudnya's alone.
`skeleton-cap-2.json` (`scripts/skeleton-cap/make_other_edits.py`) makes
every other sitting skeleton a row of its project's lamp table,
`skeleton_cap<k>`, as Rudnya's (below, "Loose pieces"), its look stone and
leftover body of its own: Vostok (1), Chemical Plant (2), the Fort (1), the
three Londons (4 each), v0.11's Vostok and Chemical Plant (2 each), v0.21's
Vostok, Chemical Plant and Fort; 132 projects. Pack `skeleton-cap@2` adds
the v0.11, v0.32 and v0.42 copies of the skeleton (the same cap; v0.11's two
LODs), version 1's models the same bytes.

## Kitchen ware

The owner (2026-11-27, "shoot the can"): `kitchen-ware-1.json`
(`scripts/kitchen-ware`) makes the maps' small kitchen things rows of their
projects' tables, their collision objects out (as the bottles'), 137
projects:

- whole things, `ware<k>`: School's tin cans (3, and v0.21's), the teapots of
  Mamayev Kurgan (stock, v0.21) and the three Londons, Rudnya's wash-stand
  tanks (2: stock, v0.21, v0.69), v0.11 Vostok's can, teapot and pan. The
  intact visual is the map's; the loose piece pack `kitchen-ware@1`'s
  `srp_<name>_loose`, the whole model (every LOD) about its LOD0 bounds'
  centre, of kind `ware` with its capsule in the row (`half`, `radius`,
  `axis`: along the bounds' longest side, its radius the mean of the other
  two half sides; a can's 6 + 10 cm along y). The client turns the body so
  its capsule (the body's X) lies along that axis (`WareAxis`) and draws the
  visual back from it (`PieceKind::drawn`). `LoneBody::ware_shot` throws it
  2.2 m/s up and 2.6 m/s along the shot's level direction with 10 rad/s of
  tumble (some 1.5 m from a table), `ware_kicked` 1.9 and 2.3 after; a shot
  within its capsule's half length knocks it on. Its break is the bullet's
  hit of its own collision material (17, thin metal; the pan's 18).
- ceramic, `ceramic<k>`: Rudnya's three clay jugs and its pot (stock, v0.21,
  the v0.69 port's jug), the bottle's surface and material: eight shards
  each (`bottle-shards`' cut: 2 cm triangles, two-sided, its seeds spread
  over the surface by `far_seeds`), kind `shard`, its break bullet x glass.
  Every jug is placed at scale 1.225; a loose piece's seat is rigid, so its
  shards are cut from the mesh at that scale (`SCALE`) and placed unscaled.

Rudnya's projects have 35 pieces (a cap, 32 shards, 2 tanks), v0.21 Rudnya's
43.

## Vector's capsules

`lab-capsule-glass-1.json` (`scripts/lab-capsule`): Vector Laboratory's four
glass capsules (`props/chemicalflaskcentre/lab_flask`, 3.7 m tall, the lower
floor) are glass tubes of two halves, the capsule's front and its door, a
model of its own beside it (`lab_flaskdoor`; the owner, 2026-11-27: "only
one wall, even if I shoot another"). Each half is a row of its project's
table (18), `capsule<k>` and `door<k>`. Its bounds are the glass's
(`capsuleglass_s`, a cylinder's segment of radius 0.70 m, 0.61 to 3.07 m up
in the capsule: a shot at the frame below is no hit); the intact visual the
model, the broken one pack `lab-capsule-glass@1`'s `srp_<model>_broken`
(every LOD's glass surface out of its settings) placed as it; the glass's 16
shards its loose pieces, cut to 20 cm triangles jagged by up to 6 cm, some
40 cm a piece, a few hundred triangles each. The table's look: bullet x
glass along the glass's height (ten bursts), leftover glass: the glass
collision stays, as a lamp's, unmarked.

A shard is of kind `pane_shard` (`PieceKind::PaneShard`): its row gives its
glass's level outward normal there (`out_x/y/z`, from the cylinder's axis,
fitted to the glass: `make_pack.py`'s `axis_of`) and its long side
(`along_x/y/z`, its points' principal axis), both in its model's frame. Its
body (`ShardSize::Pane`: a capsule 50 cm long of radius 2 cm, 1.5 kg;
glass's grip) lies along the shard (`pane_frame`: the body's X along it, its
Z out) and `LoneBody::pane_shattered` drops it out of the frame: 1.3 m/s
out, 0.4 up, 0.3 along the shot, toppling outward at 2 rad/s. The first
build threw them as a bottle's, the body's X the model's whatever the shard
and 1 m/s along the shot (the owner: "they fall too slowly and not all glass
breaks"): into the tube, its glass's collision kept them up. Vector's
projects have 128 pieces (the most) and bind 473 objects.

## Vector's jars

`lab-jars-1.json` (`scripts/lab-jars`; the owner, 2026-11-27, of the racks
of jars on Vector's lower floor: "I expected those one"): Vector's racks
(`props/lab/chemical_flask`, five a project) hold ten glass jars each, 36 cm
across and 1.06 m tall, on rods, one surface (`windows_broken_s`) of ten
closed parts. The list sets each rack visual's names to pack
`lab-jars@1`'s `srp_chemical_flask_frame` (the rack without its glass; its
collision object, the rack's, keeps the jars' glass) and makes each jar a
row, `jar<r>_<k>` (rack r in the project's order, jar k by its centre, x
then z): its bounds 1 cm larger, its glass line its axis, the jar alone
(`srp_chemical_flask_jar<k>`) drawn while intact and its stump
(`_broken`: its bottom under a jagged rim 6 to 22 cm high) while broken,
both at the jar's centre. Its loose pieces are the glass over the rim as five
shards (`_glass<j>`: strips around the jar, its seeds around its wall at one
height; the owner: "jar racks don't have shards"), of kind `pane_shard`, out
from the jar's axis. The racks' collision objects take the pack's
`srp_chemical_flask_collision`: the rack's without its jars' ten glass
capsules (the owner: "glass bullet holes are left", "even though glass was
destroyed": a jar's capsule reached 18 cm over the jar, past its bounds, and
stayed once it broke). A bullet breaks a jar on its way through, to what is
behind. Vector has 157 rows and 378 pieces a project.

A shard knocked again (a shot within its reach, a blast) plays its
fixture's hit once at it, its look's (`Piece::look`: glass on a shard, metal
on a can, stone on a cap): at most four a shot or a blast (the owner:
"hitting shards doesn't make glass sound").

A pane shard bursts out of its glass (`pane_shattered`: 5 cm out at once,
clear of the glass's collision, 1.6 to 2.8 m/s out, a throw of its own from
its place's `scatter`) and lands with its own grip (`PANE_GRIP`: held near
the ground under 1 m/s, some 30 cm of skid); a loose body steps up to 0.1 s
a frame (`LONE_SUBSTEPS`), real time down to 10 frames a second (the
owner: "they drop slowly and just lie on ground").

A project keeps 384 pieces (`MAX_PIECES`, `MAX_LOOSE`), each a pose of some
100 bytes (`Perch`); only a piece in the air has a body (`Bodies`, 128 of
some 290 bytes: `MAX_FLYING`). A piece that would take off while every body
flies waits where it is (struck, on its seat) for one to land. The first
build gave every piece a body, and 128 pieces were the most the registry's
plan (built on the main thread's 1 MB stack) could hold.

## Vector's windows

`vector-windows-1.json` (`scripts/vector-windows`; the owner, 2026-11-27, of
the boiler room's metal-framed glass: break it as the capsules): Vector's
glazed partitions are no model of their own but the surface `glass_s`
(`tiled/window/glass`) of its building `level/07/base_a`, a render visual at
the origin of each of its 18 projects (no other map has it); their glass's
collision is the building's collision object's (`coll_glass`). The
building's vertical glass stands in 23 bays: the boiler room's partitions
(z -27.5, x 68.1), the back wall's high windows (z -31.5), two more; each
part two-sided (a quad and its twin 2 cm off), a bay the vertical parts of a
plane meeting along it (`make_pack.py`'s `bays`). Its flat glass (tabletops,
the floor's) stays as it is. The list sets the building visual's names to
pack `vector-windows@1`'s `srp_base_a_frame` (the building, its `glass_s`
the flat glass alone, every other surface as it is: 5.6 MB) and makes each
bay a row, `pane<k>`: its bounds 3 cm off its plane and 1 cm along it, its
glass line its height through its middle, the bay alone
(`srp_base_a_pane<k>`) drawn while intact and what its frame keeps
(`_broken`: each part's glass 1 to 7 cm in from its edges, jagged, a part
and its twin alike) while broken, both at the bay's centre. No pieces
(Vector's projects have the most); the game's glass burst is its break, the
glass's collision stays (leftover glass). Vector has 180 rows a project.

## Bells

School's wall bells (`props/electric/bell_school`, four a project, School's
21 and v0.21's 10) ring when shot (the owner, 2026-11-27: "should be
directional and just once or every n seconds"). `school-bells-1.json`
(`scripts/school-bells/make_edits.py`) adds rows `bell<k>` of `"reaction":
"ring"` and `"ring_every": 4`: the bell's bounds (2 cm larger) and its middle
(the glass line), no visual, no piece, no light. A ring row is no fixture of
the history: `LampCount` counts the breaking rows (a table of bells alone is
refused). Its `model::Bell` is `Silent`, `Due` (a shot or a blast reached
its bounds since the last frame) or `Rang { at }`; a strike within
`ring_every` of its ring is not heard (a burst's next bullets, a bullet
resimulated after a rollback). The frame (`game_world.after_move`, on the
game's thread, outside the registry's lock) plays each due bell's ring at its
middle, a point sound of the world's scene (`GameWorld::play_sound_at`, as
the corpse shot's): heard from where the bell is, by everyone near (each
client its own bullets' and blasts', as it draws them). The bells have no
collision: the ray reaches their bounds on its way to the wall.

The ring is one of ten takes, each the collection
`sounds/collections/props/school_bell_srp/ring_<k>` (`RING_TAKES`, asked at
the first frame of a project with bells; a take missing is silent and the
log says so), pack `school-bell@1` (`scripts/school-bells`): a wrecked
Soviet electric school bell, synthesized (`synth.py`, the owner: "give me
many versions, this should sound post-apocalypse like"; no build of the game
has a bell): a dying battery, a rusted dome, bad contacts, a cracked dome, a
bent dome, far down the corridor, a cold start, a drowned one, an old one
still working, a dent mid-ring (the owner kept all of synth.py's but its
single clang and stuck hammer). Which take is the shot's (the owner,
2026-11-27: "can be w/some randomness, so everyone hears the same one"):
`model::shot_seed`, a bullet's start (a blast's centre) to the centimetre,
mixed with the bell's row (`bell_seed`: each bell of a blast its own), the
same in every game that simulates the shot and in a rollback's; the first
strike of a frame is the one heard. Vorbis mono, the stock metal hit's
options (3D, the `material_hit` preset: heard to 35 m).

## Room probes

In a ported map most of a room's light is its environment probe's, whose
cubemap the donor's editor baked with the lamps on. Nothing in the donors
ties a probe or an ambient light to a lamp: 0.44a5's and 0.32a4's lamp group
holds its point light and its sparks alone, their ambient lights are each
weather's fill set (`ambient_light_day`, `ambient_evening_light`: sky-blue
and grey, none in a tube lamp's reach), and a probe has no owner field. The
owner's `2026-11-22-05-55-19.png` (London (v0.42), the tower's corridor,
lamp at 1.5, 16.8, 0.4 broken) was this: no ambient light within 9 m, the
corridor drawn by `gerkin(4)`, whose light is some 60 % its lavender tube's.

`scripts/breakable-lamps/probe_light.py` splits a probe's light by colour.
Its light is the mean of its irradiance cubemap (`_diffuse.dds`) as 0.26g's
shader reads it (RGBM); it is fitted as a mix of the lamp's colour (its
light object's) and the room's light without lamps (the mean colour of the
project's probes that hold no lamp's light). A diffuse probe whose volume
holds the light of one lamp alone, and whose light is at least 40 % that
lamp's (`LAMP_SHARE`), is that lamp's: its row lists it in a `probes` table
(exact position, name, `keeps`: the share of its light not the lamp's, at
least 0.15, `KEEPS_FLOOR`). The two colours must differ by 8 degrees
(`SPLIT_ANGLE`): a warm bulb or a kerosene flame (no light object) is the
daylight's colour and splits nothing. London (v0.42)'s day projects bind 5
probes (`gerkin(4)` keeps 0.43, `gerkin(5)`, `hall(2)`, `hall_2_floor(3)`,
`(11)`), London (v0.32)'s day, rain and evening 5 to 7.

London (v0.42)'s rain light is as blue as the tubes: no split. The lamp and
the probes (their names, places and boxes) are the same in every weather
(checked on both Londons), so where a weather's own split cannot be made the
day's lamp light in the probe of that name and place is taken out of the
weather's, where some of every channel is left: London (v0.42)'s rain binds 5
probes so. What is left need not have the room's colour: the rain is as blue
as the lamp (which is why its own split cannot be made), and the neighbours'
floor below keeps the room from going darker than the rooms beside it. (Until
the owner's 2026-11-22-07-52-13.png the rest had to be within `SPLIT_ANGLE`
of the room's colour, which left out `hall(2)`, the stone building's opened
first-floor room, 14 degrees off: it keeps 0.41 now.) Its evening bake
carries no lamp (taking the day's lamp out leaves negative light) and its
own split binds nothing.

That room stays brighter than the rain outside with its lamp lit or broken:
both Londons' rain light it with `ambient_light(14)`, a fill of radius 9 and
intensity 2 (adding its whole warm-white colour, 255, 223, 225), the
strongest rain fill there. It is not a lamp's: its centre is outside the
room's probe, in the facade's opening, 5.7 m from the tube, of another colour
than the tube's, and the rain sets have no fill tied to a lamp. Whether 0.26g
adds it as 0.44a5 and 0.32a4 do is not checked yet (the reference rig, in
rain, at that spot).

The fill lights inside a lamp's room probe (ambient lights, additive in
London: intensity above 1) are its room's baked light too and go out with
it, in the row's `ambient_lights` table (a fill in two lamps' probes is the
nearest lamp's): London (v0.32)'s rain 3 (`ambient_light(125)` in `gerkin(5)`
was one of the two pools the owner saw in rain_4's office,
2026-11-22-06-30-46.png), its evening 1, London (v0.42)'s day and rain 1. A
fill outside any lamp's probe (the other pool there, `ambient_light(124)`, in
`gerkin(13)`, which holds no lamp) stays: the weathers' fill sets light rooms
with no lamp alike. The rule binds 4 probes on Vostok (v0.11) too, whose list
(`breakable-lamps-1.json`) is not regenerated yet.

A broken lamp's probe stays drawn, with its `diffuse_multiplier` and
`specular_multiplier` (+3F0h, +3F4h, read by `insert`) times `keeps`: the
registry takes it out and puts it back with the other multipliers
(`drive_probe`), and gives it back its own when the lamp is whole again or
the project goes. One multiplier dims the probe's light, not its colour: the
lamp's tint stays in what is left. A probe in the boxes of two lamps (a
room's or a building's) is left as it is.

A dimmed probe is never darker than what is drawn beside it, where its room
goes on into the next (`neighbour_light`): the probes that overlap it
without holding all of it (not one under it, a building's), at most twice
its radius, laid over one another as the renderer does (largest first, each
by its smoothstep alpha), just outside its box at its middle height. Its
`keeps` is at least their light over its own. The split alone took London
(v0.32) evening's corridor probe `gerkin(4)` to 0.26 of 0.33 (0.085) between
`gerkin(1)`, `(2)`, `(3)` and `gerkin_hall_centre` at 0.11 to 0.13: its box
showed as a dark rectangle, and the corridor beyond it as a lit patch (the
owner's 2026-11-22-07-16-06.png). It keeps 0.38 now, their 0.12. A probe the
lamp-free rooms beside are as bright as (London (v0.42)'s
`hall_2_floor(3)`, darker than them with its lamp on) keeps all of it.

The renderer draws the visible probes largest sort key first. A render
probe's key is its radius plus a number in [0, 0.1) its constructor makes from
its heap address (+380h, 0x1C2F30..0x1C2FA6, added by `set_properties`
0x1C370F), so probes of one radius come in an order of the run. A neighbour of the dimmed probe's radius that overlaps it
(London (v0.32)'s `bkg_building_all_floor_low_32(7)` over the top half metre
of `hall_2_floor(4)`'s room, 0.124 against the dimmed 0.049: the owner's
bright strip under the ceiling, 2026-11-22-06-31-49.png) shows through where
it happens to be drawn later. While dimmed, the probe's radius (+3ECh) is
0.1 less (`DRAWN_OVER`): its key is below every key of its radius, whatever
the numbers, so it is drawn over its equals. (#2400's 1 mm did that in about
half the runs.) A box probe's volume and falloff are its transform and side
width, not its radius; London binds box probes only.

## Rust implementation and ownership

`client-patches/rust/patches/src/lamps.rs` and `lamps/model.rs` implement the
feature in the shared `patches` crate, with `forbid(unsafe_code)`. The hook
declarations are `lamps.toml`; entries, native continuations, convention
shims and runtime vtable copies are generated. There is no lamp C/NASM
implementation or separate lamp arena block.

The patch operates on `LampId`, `LampCount`, `MaterialId`, `VisualIndex`,
finite `WorldPoint`, `RayDirection`, `Distance`, `Bounds` and a clipped
`BulletSegment`. Configuration records are decoded into `ConfigValue`
once by `vostok::config`; authored rows are then resolved into a complete
project plan. `Snapshot` bounds the fixture count and reads the complete
suffix before committing. Renderer membership has one total transition
function over `Detached`, `Hidden` and `Shown`. Lamp transitions distinguish
first hits from repeated hits and restoration, which never emits effects.

`vostok::project::ProjectRegistry` is the sole owner of the registration:
`Empty` or `Bound` with project identity, physics identity, scene, patch data
and presentation bindings. An uninstalled `ProjectPlan` borrows the insertion
hook. Every visual index, exact light position, native type, unique owner
and capacity is checked before committing any vtable change. Scenery without
metadata requires none of the lamp-specific scene or physics inputs.

The registry binds five native kinds, each by its own table: render visuals
(0xA029B4), `object_light` (0xA0ADEC), `object_ambient_light` (0xA0AC4C),
`object_particle_visual` (0xA0AFAC, the table of the object at +0) and
`object_environment_probe` (0xA0ABEC: insert 0x28FFD0, remove 0x2904A0 at
slots 8 and 9, `this` in ECX, the translation at +140h; insert hands its
properties to `scene_renderer::update_environment_probe` under its render id
+3E4h, remove takes that id out).
The ambient light has `object_light`'s layout where the patch reads it: ten
methods with insert and remove at slots 8 and 9 (0x290930, 0x290AE0: `this`
in ECX, nothing on the stack), the translation at +140h. Its insert hands
its transform, colour, intensity and radius to
`scene_renderer::update_ambient_light` under its render id (+154h); its
remove takes that id out. The particle object is the same again (insert
0x292CC0, remove 0x292DC0): its insert plays its system (+158h) at its
transform through `scene_renderer::play_particle_system` and, when it has a
track (+160h), registers its tick with the scene; its remove takes the
instance out and unregisters the tick. The registry calls each kind's own
two methods, and the renderer directly only to move a loose visual's model
(Loose pieces).

The native insertion hook is `simple_game_project::insert` (0x285750): ECX
is the project, its first stack argument the physics world. Scene +98h is
the camera director. Project config, arrays, scene and light fields are
named in `fields_026g.tsv`; wrappers contain no literal game addresses or
field offsets. The engine functions and their ABIs remain in the engine
library and re-kit. Native handles stay behind the registry's interface;
workers can reach only the patch's simulation data.

Bullet and frame handlers share the existing `bullet.ray_traced` (0x32D627)
and `game_world.after_move` (0x1F7C3C) sites with ragdolls, in explicit order.
One generated entry saves/restores the machine and replays the native span.
The ray is clipped at the nearest native hit before testing fixtures; the
original bullet and collision result remain unchanged.

## Break effects

Version 2 played the pair of the bullet's own material and the glass once, at
the centre of the fixture's bounds: one bullet-hit puff of shards, as small as
an ordinary glass hit and inside the housing's box, which on a 1.2 to 1.6 m
fluorescent tube or the plafon's cage reads as no shards at all (the owner's
test of release 39). Four bursts spread over the box's longest world axis
still read as nothing on a tube, from a bullet or a grenade (round 4 of
release 39): `fx/material_hits/glass` is some 70 shards of 3.5 cm flying for
1 to 1.5 s, which covers a bulb or an alarm lamp's glass close to the eye, but
not 1.5 m of tube at the ceiling. The stock game has no bigger glass break:
its only other glass effect, `glass_melee` (the melee x glass pair), is one
crack sprite and no shards.

A break now plays the pair's particles along the fixture's glass line, one
burst per 15 cm of it rounded (at least one, at most ten), each in the middle
of its share: a bulb's or an alarm's one, Mamayev Kurgan's 0.52 m plafon
three, London's 0.83 to 0.95 m tubes six, School's and Vector's 1.5 m
fixtures ten, some 700 shards. The sound plays once, at the line's middle. A
bullet's and a blast's break play the same bursts; a bullet's shards fly
along the bullet, a blast's away from it.

**Why release 39's bursts showed nothing (release 40).** Every burst of
release 39 was drawn nowhere, one or ten. The shards the owner saw on a shot
lamp were the game's own glass hit, at the bullet's hit point with the real
surface's normal; a grenade has no such hit, so its breaks showed none. The
burst was queued and played (the manager's +30h stack has no cap: its tick
runs every functor queued), but with the normal set to the reversed ray. The
game orients an effect in `game_world::play_particle` (0x25B9B0, the
`bullet_manager_engine` slot 4 the queued functor calls): the glass pair's
`particle_orientation` (+74h) is 0, so `create_rotation(direction, normal)`
(0x25CB10), rows normalize(normal x direction), the normal, and their cross
product (its second transform, about the direction reflected in the normal,
likewise). A normal along the direction has a zero cross product: 1/0 times 0,
two NaN rows, and every shard's velocity (the pair's `RandomTargetVeloc`
box, 3.2 m/s down the effect's y) and the crack sprite with them. Running
0x25B9B0 on release 39's arguments gives those NaN rows.

`SurfaceBreakEffects` now reads the pair's orientation once
(`ParticleOrientation`) and gives play_particle a frame that is never
degenerate: for the glass pair's (0) the normal back along the ray and, as
direction, the ray's `perpendicular()` (its cross product with the world axis
it is least along, never shorter than sqrt(2/3) of it); for a pair oriented to
the direction, the ray and that perpendicular. The effect's y is still the
reversed ray, so the shards fly as designed; the direction only sets its roll.
`check_lamps_026g.py` runs 0x25B9B0 on every queued burst (vertical, oblique
and turned rays, blasts, both orientations) and requires two finite
orthonormal transforms with that axis.

## Blasts

Every blast of the simulation goes through `bullet_manager::explode`
(0x339630; 0.21d's name, 0x2085C5): a grenade's (`grenade_set_core` 0x30A4A0,
from `grenade_core`'s and `grenade`'s explode) and a booby trap's (0x3056D0).
ESI is the splash, a copy of the set's config with the thrower's modifiers in:
`splash_radius` +28h, `splash_min_damage` +2Ch, `splash_max_damage` +30h,
`splash_pierce` +34h. Its stack: the core world's bullet manager (+18F58h)
and the centre. It damages each object within `splash_radius` of the centre,
scaled by (R - d) / R and by what the game's trace 0x3393F0 leaves of
`splash_pierce` along the ray from the centre (every front-facing hit of a
material with `physic/armor` v > 0 multiplies it by 1 - (1 - c)^2, c =
clamp((s - v) / v, 0, 1); armor 0, glass, passes), when that is above 0.

The lohook `bullet_manager.explode` at its entry (`sub esp,0BCh`, replayed)
breaks every intact fixture whose bounds' nearest point to the centre is
within `splash_radius` and that the same trace reaches with something left,
through `engine_explosion_reach` (the game's own function, not a copy of its
rule): a wall stops it as it stops the damage, a window does not. A blast
inside a fixture's bounds needs no trace. Each break goes through the same
transition as a bullet's, under the same lock, into the same bullet history:
rollback, resimulation and late joins agree, and only the first break of a
fixture plays its shards and sound (none on a manager without a renderer).

## Loose pieces

A fixture can have a piece that comes off when it breaks: Rudnya's sitting
skeleton's cap (the owner, 2026-11-26: "shooting this helmet should make it
fly off", "similar to caps flying off dead ppl"). A row's optional `loose`
table names its visuals, `{ "cap0": { "index": n } }`. The rest of the row is
a fixture's: its bounds are the cap's (a shot at the skeleton's legs is no
hit), its intact visual the skeleton, its broken one the skeleton without the
cap, both placed alike. The piece is a model of its own whose origin is its
centre, placed as the skeleton but at that centre: its seat. Content:
`scripts/skeleton-cap` (pack `skeleton-cap@1`: the cap is the part of the
skeleton's cloth textured from its atlas' top right block, in every LOD;
edit list `skeleton-cap-1.json`: Rudnya and its v0.21 and v0.69 ports, 33
projects, a fixture for each sitting skeleton, two on v0.69's; the skeleton's
collision is stone, so its pair is bullet x stone). `check_built` checks a
figure's bare model and its piece (`FIGURES`).

A figure's table says `"leftover": "body"` (a lamp's leaves it out, `"glass"`):
the collision left after the break is the figure's own, which its broken model
still covers, not glass hanging in the air. Its break material's marks inside
a fixture's bounds are skipped only while the fixture is intact (they would
be on the cap, and stay in the air once it flies) and drawn once it is broken:
the skull's collision box (17 x 16 x 22 cm) lies almost wholly inside the
cap's bounds, so a rule without the state left the bare skull unmarked but
for its face (the owner, 2026-11-26). This rule reads the condition; a mark a
rollback to intact leaves stays, under the cap, and late joins carry none.

The piece is drawn while its fixture is broken. Its flight is
`lamps::model::Flight`, one machine per piece: `Seated`, `Struck` (broken by a
push this client saw: a bullet's ray, or the direction from a blast's centre),
`Kicked` (off its seat, struck again where it was), `Flying` and `Lying`;
how it flies is its `kind` (`PieceKind`): `headwear` (the default, the cap),
`shard` (a bottle's: `ragdoll::LoneBody::shattered`, a capsule 9.4 cm long
of 50 g thrown 1 m/s up, 1.2 m/s out from the glass's middle and 1.5 m/s
along the shot, `shard_kicked` after; a shot within 6 cm knocks it on),
`pane_shard` (Vector's capsule glass, "Vector's capsules") or `ware` (a
whole thing, "Kitchen ware"). A project has at most 384 pieces (Vector's
378), 128 of them in the air at once, and 96 level rays a frame for them. A
break strikes the fixture's pieces under the bullets' lock; a later bullet
whose segment passes within 14 cm of a flying or lying piece's centre, or a
blast whose splash reaches its centre, kicks it on (the owner, 2026-11-26:
the cap is "a thing that flies around the map"): it has no collision, so the
ray goes on to what is behind it, and the next frame launches it from where it
is as `ragdoll::LoneBody::kicked` (2.5 m/s up and along the push, 8 rad/s of
tumble: about 2.4 m along the ground); each frame (`game_world.after_move`, the workers joined) steps them by
the real time (`vostok::clock::FrameClock`) against the level the ragdolls
use (`ragdoll::GameLevel`, 32 rays a frame), and the condition the history
holds wins: an intact fixture's piece is seated again (a rollback), a broken
one's that no push of this client's struck (a late join, a load) drops off
its seat. The flying body is a corpse's headwear's (`ragdoll::LoneBody::
shot_off`): the same capsule (half 0.04 m, radius 0.10 m, 1 kg), 0.8 m/s up,
0.5 m/s along the push's level direction, a tumble of 3 rad/s about up x
away, the same contacts, friction and rest. Only the condition is in the
world; the flight is each client's own drawing.

The registry draws a loose visual (`ProjectPlan::loose_visual`,
`Presentation::placement`) by writing its place into the visual's own
transform (+4h, the 16 floats the native insert gives
`scene_renderer::add_model`), so an add, the native one included, draws it
there. One drawn already is moved by `scene_renderer::update_model` with the
visual's model reference (`[+44h]+108h`) and its scene's (`game_scene+4`, the
renderer `[[+9Ch]+5360h]+200064h`), as the visual's insert names them
(0x285FA0). Before its first add its model is made movable (+115h, as
`weapon::on_show` makes a gun's): the renderer treats a model without it as
static (0x172C4B). Detach gives the visual back its own transform.

## Locking

`Lock` serializes worker transitions: a fixture breaks once, and only that
transition emits feedback. The parsed material-pair capability queues native
particles and sound, with neither available to a quiet bullet. The game joins
workers before presentation/history operations. Renderer methods run only
from the scene lifecycle/frame callbacks, through the original engine calls;
the patch never edits a renderer container or model reference count.

Generated copies of the visual and light vtables retain RTTI and untouched
slots. Their typed `NativeVtable<T>` handles cannot be interchanged. Both
native removal (0x285614) and destructor fallback (0x285330) remove only
shown objects and restore every original table before native members die.
The same owner coordinates these paths; a later match starts intact.
The framework's lock requires `Send` data and releases ownership through a
guard, including host-test unwinding. The registry's private native handles
are transferred only as stored identities; scene callbacks alone use them.

## Bullet marks

A bullet's mark is not the hit model's: the hit's processing queues
`bullet_manager::add_decal` (0x321FA0), and the game world places a
world-space decal box, the next id of a ring of 64, at the hit's point. Hiding
the intact model leaves it there, in the air or on the broken model behind
the glass. The collision of every stock fixture (and the ported alarm, white
and plafon ones) is all `glass`, and the collision stays after a break, so
later shots at a broken lamp hit the same glass.

Four lohooks sit at the material pair's decal slot of each hit kind
(`bullet.decal_front`/`back`/`reflect`/`collide`: entering, leaving,
ricocheting off and stopping in a surface). A hit of the metadata's
`break_material` inside any fixture's bounds (1 cm margin) skips the
`add_decal` call; its sound and particles stay. The shot that breaks a lamp
is such a hit, so no glass mark is ever placed on a fixture, intact or
broken, and none has to be removed at the swap. The rule reads no lamp state,
so rollback, resimulation and late joins (which carry no decals) agree.
Other materials (the ported green lamps' `metall_thin` shades, which stay in
their broken models) and other places keep their marks. The ported v0.21
`lamp_on`, `plafon_lamp` and `plafon_lamp_on` have no game material
(0FFFFh: the manager's default), so their marks are not glass and stay.

## History and compatibility

The epilogues of `bullet_manager::serialize` (0x321C3B) and `deserialize`
(0x321D01) append/read the lamp state in the existing bullet-history blob:
`u32 0x31504D4C` (LMP1), `u32 fixture_count`, then `ceil(count/32)` u32 words,
low bit first, 1 meaning Broken. The limit is 256 lamps and 1,024 presentation
bindings per project. The suffix is at most 40 bytes. Maps without metadata
keep the original bullet format. The typed physics identity selects the owner. Bits beyond the fixture count
are ignored and serialized as zero, matching the C reference.

The native snapshot envelope, hashing and server forwarding already preserve
the bullet blob. This keeps lamp state through rollback, late joins, resyncs
and replay snapshots without a new server packet or server-side map table.
The reader's `{begin, cursor, byte_count}` is bounded before reading the suffix;
wrong magic/count or truncated data is rejected before changing any lamp.
Invalid project metadata is rejected before any native vtable changes.
Metadata naming objects the map does not have (a visual index past the
map's visuals or of another kind, a light or ambient light position none or
two of the map have, one object bound twice) leaves that project without lamps, with the
game's own vtables, and logs the reason to `srp_patches.log`: every client
loads the same content, so all agree. Malformed metadata still aborts.
History errors are reported after releasing the registry lock.

`simulation_changes.tsv` has a `next` row: older recordings must be refused by
the release that ships this. Release revision and minimum-client gating remain
the release workflow's responsibility; this feature does not change `REVISION`.

## Verification

`check_lamps_026g.py <original exe>` runs native config lookup, project
iteration/vtable dispatch, visual, light and ambient light methods, bullet history and installed
hooks in Unicorn. It covers all 96 authored variants, hits, occlusion, the shard bursts on
the glass lines of Mamayev Kurgan's plafon (3), London's tubes (6, one fixture
turned 54 degrees) and School's and Vector's fluorescent fixtures (10, from a
bullet and from a blast) with one sound each, each burst oriented by the game's
own play_particle into a finite frame along its ray, blasts (two lamps within the
radius both break with one burst and sound each and the history a bullet
leaves; one outside it or with the trace stopped breaks none; rollback,
resimulation, a late join, a manager without a renderer, a blast inside a
fixture),
repeated hits, quiet bullets, effects, history loading, malformed suffixes,
rollback/resimulation, ordinary removal, destructor fallback and reset.
Additional cases verify that a wrong binding leaves the map unbound and a
malformed one refuses, both before any vtable changes, and unconfigured scenery.
The four decal sites run natively up to add_decal: no glass mark inside a
fixture (or its margin) for an intact lamp, a broken one, after rollback and
in a late joiner; marks of another material, outside the bounds and on maps
without metadata are queued. Renderer/effect queue endpoints, add_decal, the splash trace 0x3393F0, the
writer's byte sink and OS services are stubbed; the native material-pair lookup runs normally.

The Rust port was checked once against the C reference (commit `a4f05203`,
never shipped; its exe SHA-256
`571b6864669cdc6e69312a5e2583036cb6db9e6d0197571348171f91d766f5a5`) by a
stateful gate: 1,692 comparisons over 400 seeded shots, all 92 variants,
restoration, teardown and malformed history, with matching renderer/effect
call traces, history bytes, native object fields and vtable ownership. The
C code and that port-only gate were then removed.

Rust tests cover geometry, total state transitions, every supported snapshot
capacity, truncation, padding, restoration, owned bounded storage and
multithreaded lock behavior. Generator tests cover ordered chains, early
exits, native slot checks, RTTI and unmodified slots, and invalid/ambiguous
vtable declarations. They are included in the full self-test.

`python -m unittest discover -s scripts/breakable-lamps` checks transformed
bounds, mesh cropping and every source binding's ownership/limits.
`release/build-content` validates composition through the full manifest and
checks every made `client_project` against its own bindings
(`make_edits.py` `check_built`): each visual index names its role's model
(intact inside its bounds, broken placed as its intact model, a cone) and
each light position is exactly one light of the map, after every later edit
list or map-editor save.

The local game tests of the C reference confirmed model replacement, light removal and breaking
on Chemical Plant and Vostok v0.11. The initial owner-identity regression
now enters through the native insertion ABI with different camera and
physics objects: scene +98h is a camera director, while insertion supplies
the physics world used by bullets and history. The Rust port has native
emulation coverage; it has not yet been played in the client. No release
was run.

## Imported cone alignment

Vostok v0.11 uses `ported/v011e/editor/cone`, independently of the actual
light and fixture model. Ten hanging-lamp cones in each of `day_2` and
`day_4` belong to their fixture's Intact presentation. The two outdoor cones
remain scenery, with positions aligned to their nearby native light emitters.

Stock, v0.21 and v0.11 cones share the mesh CRC `d3f2a927`, but their
materials differ. The imported `fx/cone_volume_material` sets
`constant_transparency` to 1.0, versus 0.05 in stock. It also uses an
imported dust texture and one-sided rendering instead of stock's two-sided
setting. Copying the stock placement offset therefore puts the brighter
legacy cone visibly above the lamp.

The legacy placement aligns the centre of the mesh's first ring (local
(0, -0.1327432245016098, 0)) to the transformed bulb tip. Aligning the apex
(local y -0.009467163123190403) instead left the visible beam too low in
the owner's screenshot: its pointed tip fades out. First-ring alignment
raises the cone 0.143247 m at its placed y scale 1.162. Brightness and
break behavior are unchanged. This latest height adjustment has an
authoring regression; visual confirmation in the client is still pending.

An audit of all 785 v0.11 model/material configs found exactly one
volumetric-cone material and one model using it, `ported/v011e/editor/cone`.
Vostok has 12 instances per mode; Chemical Plant has none. Chemical Plant's
eight green fixtures each own their nearby native light in both modes.
There are 24 checked `set_entry` edits across the two Vostok projects.
Stock and v0.21 placements remain unchanged.

Seven authoring regressions cover the first-ring height, outdoor alignment
without a false lamp owner, unchanged stock/v0.21 placements, unrelated
scenery, transformed bounds, mesh cropping and binding ownership/limits.
`check_lamps_026g.py --patched <exe>` can also check an existing executable
without relinking the arena.
