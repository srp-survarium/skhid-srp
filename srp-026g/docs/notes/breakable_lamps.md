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
and 4: 96 projects, 2,487 fixtures and 857 linked lights. It covers bare
bulbs, ceiling lamps, white wall lamps, red alarm lamps, green lamps
with/without wires and fluorescent fixtures (tubes, ceiling grids, hanging
double tubes). Streetlights and other fixture models are not yet configured.
Fixtures with no glass to break stay scenery: empty fluorescent fittings
(`fluor_lamp_base`, `lamp_big_03`, a `lamp_midle_04` without tubes) and
v0.11's `lamp_midle_0N`, older meshes than the stock broken housings. It
does not darken baked lights, sunlight, environment probes or ambient light.

## Content

`scripts/content-edit/breakable-lamps-1.json` is the placement source, registered
in `release/content.tsv`. `scripts/breakable-lamps/make_edits.py` authored it
from the manifest's base projects plus Research, using model bounds from
those same layers (including the ported models).
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
map's visuals or of another kind, a light position no light or two lights
have, one object bound twice) leaves that project without lamps, with the
game's own vtables, and logs the reason to `srp_patches.log`: every client
loads the same content, so all agree. Malformed metadata still aborts.
History errors are reported after releasing the registry lock.

`simulation_changes.tsv` has a `next` row: older recordings must be refused by
the release that ships this. Release revision and minimum-client gating remain
the release workflow's responsibility; this feature does not change `REVISION`.

## Verification

`check_lamps_026g.py <original exe>` runs native config lookup, project
iteration/vtable dispatch, visual/light methods, bullet history and installed
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
