---
name: map-port
description: Port a map from another Survarium build (0.69d, 0.46c2, 0.27d, 0.21d, 0.11) into 0.26g for Skhid as a content pack - the resource-porter manifest and build, the pack and its row in release/content.tsv, map ids and modes, the client's memory limits, the conversion pitfalls met on Rudnya (v0.69) with their fixes, and the check against the donor build with the reference rig. Use when asked to port, rebuild, fix or check a ported map, or a mode of one.
---

# Porting a map into 0.26g

Rudnya (v0.69) is the worked example: maps 204/216/217/218, pack `rudnya-069d@3`,
resource-porter `ports/survarium-069d-rudnya-to-026g.json`. The longer notes:

- resource-porter `docs/survarium-069d-to-026.md`: every conversion choice, with numbers;
- `srp-026g/docs/notes/rudnya-069d.md`: the map's ids, modes, names and what was checked;
- `srp-026g/docs/notes/modes_maps.md`: the maps table.

## The pipeline

1. **The manifest** (resource-porter `ports/<donor>-<map>-to-026g.json`): `map` (ids,
   donor and template projects, spawn folder, `probes`, `modes`, `native_objects`,
   `environment` overrides, `published_patches`), `material_templates` (a native 0.26
   material per category), `native_materials`, `game_material_names`, `material_clusters`,
   `lod1_shares_lod0`, `namespace`. No `lod_distances`, `terrain_simplify` or
   `decimate_terrain` for a 0.69d map (see the pitfalls).
2. **The build** (donor volumes as a `.split` list, the stock db, published patches that
   templates come from; checked by sha256):
   ```sh
   vostok-port build-survarium-069d-map --manifest ports/survarium-069d-rudnya-to-026g.json \
     --donor-db donor-069d.split --target-db <stock 0.26g resources.db> \
     --published-patch ~/.cache/skhid-release/patches/resources.db.patch_30 --output work/<bundle>
   ```
   It writes `<bundle>/overlay`, `scene-summary.json` (counts, native and donor models) and
   `port-manifest.json`, whose `compatibility_notes` say what was converted, dropped or
   left out and why. Read them after every build.
3. **The pack:** `vostok-port pack from-bundle --base-db <stock db> --bundle <bundle>
   --output <packs>/<id>-<version> --id <id> --version <n>` (resource-porter-compose's
   binary); keep `.description`/`.notes` in its `pack.json`.
4. **The manifest row:** `pack <id>@<version> <sha256 of pack.json> <description>` in
   `release/content.tsv`. `pack.json` lists paths, not contents: a rebuild with the same
   file list keeps its sha, so a changed pack for a release gets a new version.
5. **The content:** `release/build-content <revision> --out <dir>` from the skhid worktree
   with the packs folder named: `release.env` sets `SKHID_PACKS_DIR` itself, so wrap it,
   ```sh
   # release-modes.env
   . <main checkout>/private/release/release.env
   SKHID_PACKS_DIR=/mnt/hdd/Projects/rudnya-069d/packs-modes   # <id>-<version> -> the pack
   ```
   and run `SKHID_RELEASE_ENV=<that file> release/build-content 39 --out <content dir>`.
   The folder holds `resources.db.patch_<rev>` and `server-game-data.db`: the coordinator or
   the owner swaps it into a local game; never assemble one by hand.
6. **Ids** (check stock, every published patch, every pack and parked branches):
   - `maps_dict` id (204; the server reads maps from the game data, not from code);
   - level, `match_making_params.map_id` (14): names, room tiles and play counts key on it;
   - loading-screen id (30), `st_level_<n>_name`, `st_srp_map_*`, the lobby tile and
     loading picture (`flash/lobby_menu`), the minimap;
   - clashes: `feat/stalker-maps` claims levels 14-19 and loading ids 30-35 too; the second
     to land renumbers. Duga's local package used 205.
7. **Modes** are map ids of their own (216 Protective device, 217 with charging stations,
   218 Research), each `map.modes[]`: the ported scene plus what native Rudnya's project of
   that mode has over its TDM project (props by model and position, client objects by
   `full_name`), native's server objects (spawns, chargers, containers, stations) and its
   options. Positions come from 0.26 ("we know positions from 0.26 version"); each is
   checked against the donor's walker mesh, and what does not fit is in `drop`,
   `drop_objects` or `moves` with its `why`. `map.native_objects` copies native objects
   into every mode (the swamp anomaly and its artefact spawners).
8. **Also for a release:** the site's top-down render (`release/build-site` stops
   without it), bots (`.mapdata`), localization rows in every language.

## Limits

- The exe is 32-bit, large-address-aware; its arenas reserve about 1835 MiB. Map loading
  ran out of the 376 MiB multithreaded arena before `-mt_allocator=768`;
  the "ai" arena story is `client_patch.md` §3.34.
- Static render geometry is the hard budget: the vertex pool holds 88 MB; beyond it the
  client logs "No enough memory in vertices pool" and draws holes. Native maps load
  43-48 MiB; Rudnya (v0.69) is 35.6 MiB (TDM), 40.9 MiB (charging stations).
- Measure every build: `python3 -I <rig>/geometry.py <bundle>
  projects/<ns>_<level>/<day>/client_project` (overlay files first, then the stock db);
  resource-porter `examples/geometry_bytes` reads one db.
- Textures: Rudnya keeps every imported texture at full size: 0.69d's never exceed what
  0.26g's stock maps use for the same kind (2048; terrain normal and gloss maps 1024). Duga
  capped at 1024. `reuse_native_textures` draws 0.26g's own file where 0.69d kept the same
  image (it re-encoded most of what it kept from 0.26): 238 of Rudnya's 463, 345 MiB.

## Pitfalls met on Rudnya, and their fixes

- **LODs.** Authored LOD2 simplification cut up to 94% of a surface (the tractor
  station's windows) and 30 m type-0 switching showed it early: no `lod_distances`, models
  keep the donor's switching and merge only with models alike; `lod1_shares_lod0`.
- **Terrain blend overlays (`*_vc`).** Their vertex alpha gates the alpha test, so they
  stay full density; simplified base tiles under them (`terrain_simplify`,
  `decimate_terrain`) left overlay vertices up to 2 m off: dark seams and holes in
  trenches. Keep the whole terrain unsimplified (1 075 vertices > 25 cm off -> 94).
- **Probes** (`MapProbes::Donor`, `src/survarium_026/lighting.rs`):
  - scale each probe's diffuse by its `multiplier` (interiors x1.5-10; ignored, rooms
    were 2-10x too dark and split by hard edges);
  - keep volumes' full rotation, not y only (tilted church lights became blue columns);
  - 0.26 has no cylinder: a fill cylinder becomes the box of its footprint area, a
    darkening one is left out (its box cut black squares);
  - darkening lights leave at least x0.25 (0.69d keeps the reflections under them; 0.26
    drew the tractor station's wall tops black);
  - lights 0.69d fades out (`fade_out_distance`) farther than every spawn is from them are
    left out (`ambient_light_chaes` lit the power-plant backdrop into the haze);
  - interior probes darker than 0.03 are raised to it, at most x8
    (`lighting::adaptation_gain`): 0.69d's exposure shows its rooms alike.
- **Materials.** Tell a vertex-blend material before an alpha-tested one (the blended
  opaque layer closes the cut-outs: house_6's wall was gone) and keep `two_sided` on
  vertex-colour categories (`vertex_colour_two_sided`, `vertex_blend_two_sided`,
  `vertex_blend_alpha_test[_two_sided]` templates).
- **Native models** are reused only when collision is identical and render bounds agree
  within 5 cm (`native_equivalent`); a reused one takes the donor's placement.
- **What 0.26 cannot draw** (metalness, parallax, colour grading, height fog over the
  map): left out with a note, never faked per spot. The pond's `volume_fog` is the same
  object and shader in both builds and needs no change.

## Checking against the donor

- **The rig** (`<rig>`: its tools, kept outside the repository with the map's work; a
  Python with pillow and numpy, `python3 -I`):
  - `ref069.py batch POSES --map <key> --out DIR`: 0.69d itself, the project loaded as the
    login scene (`--level level_01/day_tdm`, `level_01/day_br` for the battery modes);
  - `ref026.py start|shot|stop --runtime DIR`: the port, in a runtime of its own;
  - `compare.py --dirs REF PORT -o OUT`: side-by-sides with mae, ssim, luma, sharpness.
- **The final check is the sweep** (owner, 2026-10-31): about 100+ generated locations,
  flagged by region, not a handful of chosen views.
  - `sweep_poses.py --work <bundle> --content <content dir>` writes
    `<rig>/../poses-sweep.json`: interiors from the probe and light volumes,
    roofs, the fog, the horizon (the backdrop), terrain overlay edges, every mode object,
    spawns; cameras inside geometry or buried are skipped (`geoq` ray-casts the collision).
  - `sweep.py run --build <name> --content <content dir> [--fresh]`, then
    `<rig>/../report-<name>/index.html`, worst first, with black / dark /
    hole / cast / gap flags on an 8 x 6 tile grid. `sweep.py compare --build <name>` redoes
    the report from the shots taken.
  - **0.69d reference shots are taken once and kept.** Each lands in
    `ref-sweep/ref-<project>/` and is filed in `sweep-ref/<project>/<pose key>.png` as it
    is shot; a later run only shoots poses it has no shot for. Only the port is re-shot
    each iteration: `--fresh` drops the port's shots alone. Never delete or re-take the
    reference shots.
  - **Rudnya's reference is closed** (owner, 2026-10-31): 0.69d never starts again for
    it. The check is `<rig>/../poses-kept.json`, 112 poses: the 47 of
    `poses-rudnya.json` (`shots/`), the 37 fix poses (`shots-fixes/`), the 24 battery-mode
    poses cached from `level_01/day_br`, each with its kept shot as `ref`, and 7 with none
    (the Research units, spawns). Run it port-only:
    `sweep.py run --no-ref --poses ../poses-kept.json --build <name> --content <dir> --fresh`;
    the poses with no 0.69d shot get `port_only.py <port shots> ../poses-kept.json`
    (near-black tiles, sky where a wall or the ground should be). A spot the kept poses
    miss is checked from the port alone.
  - Fix what it flags at its cause, rebuild, rerun, until nothing serious is flagged.

## The machine

- No capture while the owner's game runs: wait for
  `! pgrep -f '[s]urvarium_skhid.*\.exe'` (the bracket keeps the loop from matching
  itself; a plain `pgrep -f survarium_skhid` never ends). The rig waits by itself and stops
  a session the moment the owner starts a game; after such a stop it starts again only
  after 10 minutes with no game ("waiting for 10 quiet minutes": test games come in
  bursts, and each one killed a session that takes minutes to load). When the coordinator says no captures,
  start none at all.
- At least 2.5 GB of VRAM free (`nvidia-smi --query-gpu=memory.free --format=csv`).
- Never touch `private/rudnya-preview` or `private/rudnya-local`: the owner's test setup.
  The coordinator swaps builds in.
- Headless only: Xorg with the dummy driver at 60 Hz plus openbox (Xvfb's 0 Hz crashes
  DXVK), unset `WAYLAND_DISPLAY`; nothing opens on the owner's display.
- No whole-disk `find`, no `git stash`, no `pkill -f` with a pattern your own shell matches.
- Commit with `git -c core.fileMode=false commit` on `/mnt/hdd` worktrees, no AI trailer.
