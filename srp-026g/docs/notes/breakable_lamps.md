# Breakable lamps

Shooting a configured lamp breaks it for the rest of the match: its intact
model and glow cone disappear, its lights are removed, and its broken model
appears, with glass shards and a glass-breaking sound. One bullet is enough.
A new match restores every lamp. The lamp does
not change the bullet's trajectory or the map's collision mesh.

The content list covers levels 02–08, all their day/evening/rain variants
and modes 2–5, plus the v0.11 Vostok and Chemical Plant and v0.21 Vostok and
Mamayev Kurgan day variants in modes 2 and 4: 92 projects, 1,129 fixture
placements and 699 linked lights. It covers bare bulbs, ceiling lamps, white
wall lamps, red alarm lamps, green lamps with/without wires and fluorescent
tubes. Streetlights and other fixture models are not yet configured. It
does not darken baked lights, sunlight, environment probes or ambient light.

## Content

`scripts/content-edit/breakable-lamps-1.json` is the placement source, registered
in `release/content.tsv`. `scripts/breakable-lamps/make_edits.py` authored it
from the manifest's base projects plus Research, using model bounds from
those same layers (including the ported models).
The list adds the broken visuals and a project-root `srp_lamps` table:

- `version = 2`, `break_material`: the glass material id read from
  `game_materials/game.materials` during authoring, never a runtime constant.
- `lamps`: a named table of fixtures; its binary-config order
  defines the snapshot order identically on every client.
- Each fixture has world-space `min_x/y/z`, `max_x/y/z`, a `visuals` table
  (`index` in the final render-visual array, `state`: 0 intact, 1 broken), and
  a `lights` table (exact positions, with names retained for review).
- Nearby light/cone assignments are explicit in the list, not guessed at
  runtime. Authoring rejects ambiguous assignments. Overlapping bulb and cover
  meshes, such as level_06's ceiling fixture, are one lamp.

The pack `breakable-lamps@1` supplies `props/light/srp_lamp_broken` and
`props/light/srp_plafon_lamp_broken`. `scripts/breakable-lamps/make_pack.py`
keeps the stock unlit models' socket/cap triangles above their lips, with the
original normals, UVs and material. The glass below is removed, the bounds
and vertex/triangle counts are recomputed, and model collision is disabled.
`stock.json` pins every input. White and alarm lamps use the stock
`white_broken` and `alarm_broken` models; green lamps and fluorescent tubes
use their empty fittings. Imported fixtures use their ported replacement
models, except for the generated bulb sockets and ceiling caps. Overlapping
lit/unlit covers share one replacement instead of drawing it twice.
Build this resource-porter pack into
`$SKHID_PACKS_DIR/breakable-lamps-1`, then use `release/build-content` normally.
No release patch is assembled by the authoring tools.

## Engine integration

`client-patches/lamps_026g.c` and `.asm` are the arena's `lamps.code` and
`lamps.state`. The C owner is either Empty or Loaded with one project's
typed lamps and presentation bindings. Each lamp is Intact or Broken.
Bullet workers atomically compare/exchange Intact to Broken. Only the winner
queues a break's effects. The native simulation
joins its workers before history capture/restore and frame presentation.

The bind hook runs at `simple_game_project::insert` (0x285750), before its
native `insert_static` call. Its first stack argument supplies the physics
world; its second is the game-world core, and ECX is the project.
The project owns its config (+17Ch), scene (+180h), objects (+184h/+188h),
and 48h-byte render visuals (+1E8h/+1ECh). These are 0.26g offsets; the older
engine source's layouts differ. A visual's matrix is +4, its model +44h.
A light's transform is +110h, translation +140h, props +158h, light id +250h.
The scene's +98h is its **camera director**, never the physics world.
These accesses and the functions' calling conventions were checked against
the original build 2777 executable; function/ABI findings are in the re-kit.

Only configured objects receive replacement vtables. Their native RTTI and
other slots are retained. The insert/remove slots have the native calling
conventions, declared in `INSTALLED_VTABLES`. One transition function manages
Detached, Hidden and Shown, emitting only the required renderer add/remove.
The engine library calls the original visual methods (0x285FA0/0x285FE0) and
light methods (0x291790/0x2917D0); no renderer container or model refcount is
edited directly. Project removal (0x285614) restores the original vtables.
The destructor hook (0x285330) also detaches before resource members die.

The bullet hook at 0x32D631 reads the native ray's origin, unit direction,
length and closest hit, and intersects the segment with each intact lamp's
authored box. A hit stops the tested segment: lamps behind walls or players
do not break. The original bullet, result, registers, flags and FP state are
preserved. The hook is immediately after ragdoll's separate 0x32D627 hook.
The frame hook at 0x1F7C41 applies the settled simulation's states to the
renderer, after ragdoll's separate 0x1F7C3C hook, preserving both original
branches. Rendering never runs on a bullet worker.

The first hit queues the bullet/glass material pair's particle and sound
through `engine_play_particle` (0x3223E0) and `engine_play_sound` (0x3222F0).
The native bullet manager owns their resource references, accepts worker
calls and drains its effects queue after joining. The effect is placed at
the fixture's bounds centre, independently of its original collision
material. Native bullet impacts remain unchanged. Repeated hits on a broken
fixture queue nothing; quiet bullets change state without effects, and
loading a historical lamp state never replays its break sound or shards.

## History and compatibility

The epilogues of `bullet_manager::serialize` (0x321C3B) and `deserialize`
(0x321D01) append/read the lamp state in the existing bullet-history blob:
`u32 0x31504D4C` (LMP1), `u32 fixture_count`, then `ceil(count/32)` u32 words,
low bit first, 1 meaning Broken. The limit is 256 lamps and 1,024 presentation
bindings per project. The suffix is at most 40 bytes. Maps without metadata
keep the original bullet format. The bound physics world selects the owner.

The native snapshot envelope, hashing and server forwarding already preserve
the bullet blob. This keeps lamp state through rollback, late joins, resyncs
and replay snapshots without a new server packet or server-side map table.
The reader's `{begin, cursor, byte_count}` is bounded before reading the suffix;
wrong magic/count or truncated data is rejected before changing any lamp.
Invalid project metadata is also rejected rather than partially activated.

`simulation_changes.tsv` has a `next` row: older recordings must be refused by
the release that ships this. Release revision and minimum-client gating remain
the release workflow's responsibility; this feature does not change `REVISION`.

## Verification

`check_lamps_026g.py <original exe>` builds the client and executes its native
config lookup, project iteration/vtable dispatch, visual/light methods,
empty-bullet history functions and installed hooks in Unicorn. It checks
hits, misses, occlusion, repeated hits, glow removal, rollback/resimulation,
late-join/replay snapshot loading, malformed suffixes, removal, destructor
fallback and next-match reset. Every authored map variant also runs through
native binding, insertion and removal. The bullet test enters through the
adjacent ragdoll hook; both frame continuations preserve registers and flags.
Renderer/effect queue endpoints, the writer's byte sink and `GetCurrentThreadId`
are stubbed. The native material-pair lookup executes normally. The check
also asserts one shard/sound event per break, no miss/repeated-hit effects,
and silent quiet bullets and snapshot loads.

`python -m unittest discover -s scripts/breakable-lamps` checks transformed
bounds, mesh cropping and every source binding's ownership/limits.
`content_edit.py ... --check` checks all 92 projects against the authoring
base, and `release/build-content` validates composition through the manifest.

The local game tests confirmed model replacement, light removal and breaking
on Chemical Plant and Vostok v0.11. The initial owner-identity regression
now enters through the native insertion ABI with different camera and
physics objects: scene +98h is a camera director, while insertion supplies
the physics world used by bullets and history. No release was run.

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
