# STALKER maps: handoff (2026-10-27)

This picks up the work on the STALKER team-deathmatch maps (`stalker-maps.md`)
where it stopped. You don't need the earlier conversations: what was done,
where it is, what is open and how to build the next pack are all below.
Experimental Pool is `cs-mp_bath`, maps_dict 210. Military Warehouses is
`cs-mp_military_1`, maps_dict 211.

## Ground relief checkpoint (later on 2026-10-27)

**The current feature pack is @6.** This supersedes the earlier @4 checkpoint
below. The owner's first reported surface was Military Warehouses outdoor
ground. Its installed Clear Sky terrain shaders use strengthened normal maps,
not terrain parallax. The port missed the THM detail scale (57, previously the
fallback 142), weakened terrain normals and retained unrelated template height
maps. These are fixed in resource-porter commit `d92d7cc`, branch
`fix/stalker-ground-relief`, based on `stalker-maps-026`:
[draft resource-porter PR #14](https://github.com/srp-survarium/resource-porter/pull/14).

- Persistent source worktree: `/mnt/hdd/Projects/stalker-maps-026/porter-ground`.
  This filesystem has executable-mode noise; use `git -c core.fileMode=false`
  for status/diff/staging. Binary: `target-ground/release/vostok-port`.
- Candidate artifacts: `out-ground-v6/`; corrected donor SHA-256:
  `7877c0616a665dd3dd61453f5aa05f6a45980ada224a0f72b5bef2dabf68c96b`.
- Local private pack: `stalker-maps-6`; pack.json SHA-256:
  `ceb6ae661dda93e24f62d5c59a5cfba68cc9d4e3cd2e34d2eba1b52d99bd17cb`.
- Validated content: `content-rev32-ground-v6/resources.db.patch_32`, SHA-256
  `f2b089f9885e2deaedb12cc9c7d98d6f944d946602ea352617e8013ddfaea624`.
  Use the matching `server-game-data.db` from that same directory.
- Evidence, logs and matching headless road/grass screenshots:
  `analysis-bath/ground-relief/{before,after}/`. Captures completed successfully
  and show stronger asphalt and exposed-grass relief. The wrapper `capture.py`
  uses its adjacent `poses.json`; set `STALKER_GAME_DATA` to the matching DB.
  It imports the existing preview worker's script listed below.
- All 12,981 content manifest files passed archive readback. Porter tests passed
  (346 library + one tool; final X-Ray subset 153), resources-db tests 3 passed,
  and library Clippy passed. Workspace Clippy and rustfmt have existing unrelated
  failures. See `porter-ground/docs/soc-deathmatch-026.md` for shader details.

Only Military Warehouses was rebuilt. The other five maps and all layouts are
from @4; **the Experimental Pool cave-in is still absent**. The target normal
blend is an approximation of Clear Sky's blend, not identical source lighting.
@5 in `out-ground/` failed native material-type validation and must not be used.
Do not rebuild @6 in place. Use version **7 or newer** for subsequent changes.
The cave-in recipe below now uses the corrected donor, so it preserves this fix.
Neither these artifacts nor the maps have been merged or deployed.

## Rules that apply to every step

- **One GPU, one game.** Start a game, or capture with `previews.py`, only while
  the owner's own game is closed. Check both of these first:
  - `private/local-clients/runtime/state.json`, for the owner's clients;
  - `pgrep -af 'Survarium|srp_exe|wine'`.

  `previews.py capture` refuses on its own while another game runs; `--wait`
  makes it wait instead. Never show anything on the owner's screen. Captures
  run on an Xorg dummy display.
- **`pack.json` doesn't pin contents.** It lists the pack's operations, not
  hashes, so a pack rebuilt with different spots or props keeps the same
  sha256. @3 and its rebuild with other spots both had `b3d24075…`.
  `release/content.tsv` therefore can't tell two builds of one version apart.
  Give every changed build a **new version**, and check the built files
  themselves: `projects/v010b_cs_mp_bath/day_4/client_project` in the pack's
  `files/`.
- Use the release env without printing it:
  `set -a; . private/release/release.env; set +a`.
- Never `git stash` (worktrees share the stash). Never `find /`.
- Add no AI attribution to commits or pull requests. Merge nothing without the
  owner's word for that change.

## Where everything is

### Repositories

**resource-porter**, branch `stalker-maps-026`. The main worktree is
`/mnt/hdd/Projects/stalker-maps-026/porter`.

| commit | what |
| --- | --- |
| f02bb4e | grass from X-Ray's detail objects (recipe option `detail_objects`) |
| 76cf31e | the grass's X-Ray detail normals |
| f4b6a78 | Experimental Pool's spots (`ports/stalker-tdm/spots.json`) |
| 823b2fe | the cave-in, team 1's way to the roof (`ports/stalker-tdm/maps.json`); `make_spec.py` passing `walker_ramps`, `walker_boxes`, `props` and `cuts` from `maps.json`; `Prop.rotation` in `src/survarium_026.rs` |

Two more worktrees sit in the same folder:
- `porter-grass` is the grass worker's, detached at f4b6a78.
- `porter-packs` is detached at 422fccd. Its binary is the `PACK_VP` below;
  it has `pack from-bundle` / `pack check`.

**skhid-srp**, branch `feat/stalker-maps`, worktree
`.claude/worktrees/feat-stalker-maps`. Its commits:

| commit | what |
| --- | --- |
| 153d4ad9 | @1 |
| e1fc53b9 | @2, grass |
| 953896d1 | @3, the spots |
| 3522366a | @4, grass normals and the spots |
| 35a1f32f | notes on the cave-in |
| this file's commit | this handoff |

Two files there are the preview worker's and are untracked:
- `srp-026g/scripts/stalker-maps/previews.py`, the headless in-game preview
  capture (setup / capture / sheet / apply; its docstring explains each);
- `previews.json`, the poses and picks.

Commit them together with the loading pictures once the owner has approved
the picks.

### vostok-port binaries

All are under `/mnt/hdd/Projects/stalker-maps-026/`.

| path | built from | use |
| --- | --- | --- |
| `target/release/vostok-port` | the grass commits, 2026-10-26 20:28 | `VP` in `env.sh`. It **can't** read 823b2fe's `maps.json`: the spec rejects unknown fields and fails on `rotation`. |
| `target-ground/release/vostok-port` | d92d7cc, 2026-10-27 | terrain fix plus the cave-in importer |
| `target-roof/release/vostok-port` | 823b2fe, 2026-10-27 | historical cave-in-only binary |
| `target-packs/release/vostok-port` | `porter-packs` | `PACK_VP` (`pack from-bundle`, `pack check`) |

To rebuild, from the porter worktree:
`CARGO_TARGET_DIR=<dir> nix develop -c cargo build --release --bin vostok-port`.
Plain `cargo` has no linker here. `nix develop -c cargo test --release` gives
342 passes at 823b2fe.

### Inputs and work folders

`/mnt/hdd/Projects/stalker-maps-026/env.sh` exports these:
- `TARGET010`: retail v0.100b resources.db.
- `TARGET026`, `STOCK_DB`: the 0.26g resources.db.
- `PATCHES`: the live patches cache.
- `VP`, `PACK_VP`: the binaries above.
- `PYTHON`: a python3 with numpy, scipy and Pillow. It points at a session
  scratchpad wrapper that may be gone. If so, use
  `nix shell --impure --expr 'with import <nixpkgs> {}; python3.withPackages (p: [p.numpy p.scipy p.pillow])'`
  and set `PYTHON` to that `python3`.

The work folders (`OUT`) are siblings of `porter/`:

| folder | what |
| --- | --- |
| `out/` | @1: bundles, donor, minimaps, the first top-down renders |
| `out-grass/` | the grass worker's: `bundles/` with grass and normals (rebuilt 2026-10-26 ~23:00), `donor/stalker-maps.db` (sha256 `c5e7deb0…`), `specs/`, packs @2 and @4 |
| `out-spots/` | an abandoned @3 build over @1's bundles (no grass); ignore it |
| `out-spots2/` | @3: grass bundles as of 18:46 plus the spots |
| `out-roof/` | 823b2fe's import of all six maps over `out-grass`'s donor (`bundles026/`), made only to check the cave-in; it is not packed |
| `analysis-bath/` | the analysis scripts, renders, crops and in-game shots named below |

Packs live in `$SKHID_PACKS_DIR` (release.env) as `stalker-maps-1` … `-4`.
Content patches (`release/build-content 32`) are in folders next to
`porter/`:

| folder | built from |
| --- | --- |
| `content-rev32` | @1 |
| `content-rev32-grass` | @2 |
| `content-rev32-spots` | @3 |
| `content-rev32-grass-spots` | @4, current |

`previews.py` reads `server-game-data.db` from `content-rev32-grass-spots`.

The preview runtime (`--runtime`) was the session scratchpad's
`previews/runtime`, holding `game/`, `prefix/`, `run/`, `shots/` and the
`sheet-*.jpg` files. It may be gone. Its shots are copied to
`analysis-bath/ingame/previews/<project>/day/` (PNG plus the pose JSON), and
the grass worker's shots to `analysis-bath/ingame/grass/`. Rebuild the runtime
with `previews.py setup` (its docstring lists the arguments).

## Pack state

- **@4 was the earlier pack** (the former `release/content.tsv` row: `stalker-maps@4`,
  `ade4843d…`). It has the grass with the detail normals on Experimental Pool
  and Military Warehouses, and Experimental Pool's new spots. Its content patch
  is `content-rev32-grass-spots`. Military Warehouses was subsequently captured
  in game as the ground-fix baseline.
- **The cave-in (823b2fe) is in no pack.** It has only been imported once, in
  `out-roof/`, and checked on that built map:
  - the five cuts removed the roof slab, and the importer logs which surfaces
    and meshes each one hit;
  - both doors of the hatch room reach the roof with a 0.6 m step, 50° slope
    and 1.8 m headroom;
  - nothing in the room or on its roof is a dead end;
  - all 12 props have collision and walker collision.
- The cave-in's design renders are in `analysis-bath/renders/`:
  `sheet_current.png` (before), `sheet_A.png` (the first try, a clean cut) and
  `sheet_A2.png` (the cave-in, the owner's pick).

## How to build @7 (@6 plus the cave-in)

The cave-in changes only the 0.26g import step. Reuse the corrected donor
from @6 (`out-ground-v6/`) and the mixed bundles shown below. You only need to
run `port-map.sh` if
a recipe or the level conversion changes, for example a fix for the concrete
corners below.

1. Build the binary, if `target-ground` isn't from the commit you build:

   ```
   cd /mnt/hdd/Projects/stalker-maps-026/porter-ground
   git log --oneline -1          # d92d7cc or later
   CARGO_TARGET_DIR=/mnt/hdd/Projects/stalker-maps-026/target-ground nix develop -c cargo build --release --bin vostok-port
   ```

2. Only if the 0.10b conversion changed: run the two grass maps' bundles, then
   the pack. Both maps come from the installed Clear Sky archives, so they
   take no level folder.

   ```
   . ../env.sh
   export OUT=/mnt/hdd/Projects/stalker-maps-026/out-7 VP=/mnt/hdd/Projects/stalker-maps-026/target-ground/release/vostok-port
   bash scripts/stalker-tdm/port-map.sh cs-mp_bath --no-pack
   bash scripts/stalker-tdm/port-map.sh cs-mp_military_1 --no-pack
   ```

   The other four maps' bundles have to be in `$OUT/bundles` too. Link them
   from `out-grass/bundles/` (`ln -s`). Then run step 3 **without**
   `--keep-donor`, because the donor must be composed again.

3. The pack. Without step 2, reuse @4's bundles, @6's Military Warehouses
   bundle and @6's donor through symlinks:

   ```
   . ../env.sh
   O=/mnt/hdd/Projects/stalker-maps-026/out-7; mkdir -p $O/bundles $O/donor
   for b in ../out-grass/bundles/*; do ln -sfn "$(realpath $b)" $O/bundles/$(basename $b); done
   ln -sfn "$(realpath ../out-ground-v6/bundles/cs-mp_military_1)" $O/bundles/cs-mp_military_1
   ln -sfn "$(realpath ../out-ground-v6/donor/stalker-maps.db)" $O/donor/stalker-maps.db
   export OUT=$O VP=/mnt/hdd/Projects/stalker-maps-026/target-ground/release/vostok-port
   bash scripts/stalker-tdm/build-pack.sh --version 7 --keep-donor > $O/build-pack.log 2>&1
   tail -3 $O/build-pack.log    # "map 210 cs_mp_bath: 12+12 spawns, 2 stashes; 1 walker_ramps, 12 props, 5 cuts" and the pack's pack.json sha256
   ```

4. Copy the pack to the packs folder:
   `cp -r $O/packs/stalker-maps-7 "$SKHID_PACKS_DIR/"`.
5. Change the row in `release/content.tsv` (on `feat/stalker-maps`): the name
   becomes `stalker-maps@7` and the sha becomes the new `pack.json` sha256.
   Add the cave-in to the row's description.
6. Build the content patch. In this branch `release/patches.tsv` ends at 31, so
   the revision is 32.

   ```
   cd .claude/worktrees/feat-stalker-maps
   set -a; . /home/sheep/Projects/survarium/skhid-srp/private/release/release.env; set +a
   ./release/build-content 32 --out /mnt/hdd/Projects/stalker-maps-026/content-rev32-cavein
   ```

   It should end with "with resources.db.patch_32 over players' files, all …
   files the manifest makes are as it makes them".
7. Check the built map: run `analysis-bath/built_check.py <OUT>/bundles026/stalker-maps/overlay`.
   It needs `PYTHONPATH=<porter>/tools/playable-map-pack`, and it has to run
   from `analysis-bath/` because it reads `./native/`. It reports the props in
   the project, the slab columns under the cave-in, and whether the doors reach
   the roof. Extract `./native/` first with `analysis-bath/extract_native.py
   <models…>`; it uses the dbgrab binary at
   `tools/re-kit/common/dbgrab/target/release/dbgrab` on the 0.26g
   resources.db.
8. Commit `content.tsv` and the notes on `feat/stalker-maps`. Merge nothing.

## The concrete corners (open)

The owner: "the cement itself, its corners are broken". This is the map's
own concrete as ported, not the cave-in: the roof slabs, the walls' edges and
their trims. Nothing is fixed yet.

### What was checked

- **Alpha:** no concrete or trim material uses alpha.
  - The textures:
    - the converted textures of `crete/crete_border_02`,
      `wall/wall_border_01`, `crete/crete_debris_01` and
      `crete/crete_walls_02` are DXT1 with no alpha;
    - the Clear Sky sources are DXT1 too. They are in
      `analysis-bath/src_cs/` if that folder is kept; if not, extract them
      again with `vostok-port soc-extract`.
  - The materials: of the 163, 21 are alpha-tested, all fences, grates,
    beds, girders and trees. All concrete, brick, wall and trim materials are
    `g_stage_default`, one-sided, without alpha test (`analysis-bath/mats2.txt`).
- **Shaders:** `vostok-port soc-audit-shaders --level mp_bath`
  (`analysis-bath/shaders_bath.txt`) shows `default` (lightmapped, LM) on 102
  surfaces, `def_vertex` on 34, `def_aref` on 3, `def_trans` on 2,
  `def_aref_v` on 3, `def_trans_v` on 1, plus trees and terrain.
- **Normals:** the concrete's vertex normals follow its faces
  (`analysis-bath/normals.py`, `ndev.txt`). None point against their face, and
  at most 2 % deviate more than 30° on most concrete materials. The exceptions:

  | material | share deviating more than 30° |
  | --- | --- |
  | `crete_debris_01` | 41 % |
  | `crete_walls_01` | 22 % |
  | `mtl_walls9a` | 19 % |
  | `mtl_stena_ch_04` | 13 % |

  So the normals aren't smoothed across corners.
- **Geometry:** offline renders without culling show no holes, gaps or cracks
  at the roof rims, wall corners, the bridge's slab edge, the north gallery or
  the pool rim (`analysis-bath/renders/close_*.png`). The faces are there and
  their normals agree with their winding.
- **In game:** the preview capture `poolside-se`
  (`analysis-bath/ingame/previews/cs_mp_bath/day/poolside-se.png`; crops in
  `analysis-bath/corners/ingame_*.png`) shows nothing broken at this distance.
  The balcony slab and the pool rim look solid. The breach in the west block's
  wall is modelled that way in Clear Sky.

### What is left (the suspects)

The offline renderer ignores normal maps, culling, lighting and alpha, so the
fault is most likely in one of those.

1. **Normal maps or tangent frames on the concrete.** The porter binds X-Ray's
   `<diffuse>_bump` companions (`src/soc/build.rs` ~800–860, ~1322). The
   importer already rebuilds degenerate tangent frames (`early.rs`
   ~1290–1350). A wrong tangent sign or swizzle where UV islands meet along an
   edge would light the corners and trims dark or bright, so they read as
   chipped.
   - Check: render one concrete surface's tangent and binormal (bytes 16–24 of
     the 0.26 vertex; the layout is in `analysis-bath/normals.py`).
   - Check: capture the same corner with the bump unbound (the template's
     normal flag off).
2. **Lightmaps.** 102 of the surfaces are X-Ray LM: their light and corner
   shadows were baked into lightmaps that 0.26g doesn't use. The edges then
   get only 0.26g's dynamic sun, ambient and SSAO. Thin trims along the edges
   (the `*_border_*` textures, which picture chipped concrete) may read
   "broken" under that light even though their geometry is right.
3. **Back-face culling of thin edge faces.** Ruled out where the normals agree
   with the winding. Still check any surface whose source shader is two-sided
   in X-Ray; the shaders.xr audit reports none for concrete.

Next steps:
- Ask the owner for one in-game screenshot of a corner he means. The newest
  file in `~/screenshots` is his last one, and none of those so far shows
  concrete close up.
- Or, while his game is closed, add 2–3 corner poses to `previews.json` and
  run `previews.py capture --runtime <dir> cs_mp_bath`: the south block's roof
  rim at about (-10, 15.1, -3), the south-west block's corner at about
  (-37, 6, -14), and the bridge's slab edge.
- Compare with Clear Sky's own picture of the same spot (the CS SDK, or the
  porter's `soc-probe-level`).
- Fix it as a general porter step with a test, since the same fault would be
  on every ported map. Then run `port-map.sh` for every map that changes, and
  build @7 as above, without `--keep-donor`.

## Open owner decisions

- **Loading pictures and room tiles.** The proposed picks are in
  `previews.json`: `poolside-se` for Experimental Pool and `road-tower` for
  Military Warehouses. The contact sheets were in the preview runtime
  (`sheet-cs_mp_bath-day.jpg`, `sheet-cs_mp_military_1-day.jpg`); the single
  frames are in `analysis-bath/ingame/previews/`. Once the owner picks,
  `previews.py apply --runtime <dir> <project>` writes them with
  `ui_images.py`. The other four maps keep their placeholders.
- **A north stair for team 2's stash.** Team 2's stash is on the north grass
  at (-22.88, 4.18, 48.88), 81 m on average from its spawns; team 1's is
  32 m. The north blocks have no way out to the north except through the
  sunken passage. A stair or ramp from the north gallery (8.6 m,
  z 29–31) down to the yard would even this out. It is not built and the
  owner hasn't asked for it; offer it as a one-line option.
- **Indoor lighting.** Experimental Pool's `skylight_probe.interiors` in
  `maps.json` is empty, so the interiors get the outdoor ambient. Training
  Camp has one interior (`room_00`, made with
  `tools/playable-map-pack/interior_probes.py`). The owner decides whether the
  pool's blocks get darker interior probes. If he does, measure them with
  `interior_probes.py`.
- Also there:
  - roof sight lines: the south block's north windows face the north roofs, so
    9 of team 1's spawns can be seen from about 320 m² of roof (`stalker-maps.md`);
  - the HUD minimap framing of Experimental Pool is 0.1 m off the pack's own
    (0.3 px). It was left as it is.

## The other four maps

They are as @1 left them, with the planner's spots. Each still needs an
in-game test.

- **Repair Depot** (`rembasa-cs-dm`): its level has 349,153 grass tufts at the
  level's density. Thin it with the `detail_objects` option's `density` before
  turning grass on.
- **Training Camp** (`training-camp-cs-dm`): it has its summer meadow (the
  `meadow` addon), so it takes no `detail_objects` grass.
- **Station and Depository** (`dl-mp_station`, `dl-mp_depository`): community
  SoC maps found in no installed archive. `port-map.sh` needs their level
  folders again, which the earlier session downloaded and unpacked. The
  sources' sha256 lists are in `ports/stalker-tdm/sources/`.
- Every map: an in-game test of spawns, stashes, minimap, loading picture and
  names. Before a release:
  - a top-down render per map for the site's Maps tab
    (`scripts/damage-calc/assets/maps`);
  - the bots' `mapdata-dump`;
  - the `SRP_MATCH_MAPS` / `SRP_ROOMS_EXTRA_MAPS` pools (see "Still to do
    before a release" in `stalker-maps.md`).

## Analysis tools (`/mnt/hdd/Projects/stalker-maps-026/analysis-bath/`)

These are scripts, not part of either repository. Run them from that folder
with a python3 that has numpy, scipy, Pillow and matplotlib.

| script | what it does |
| --- | --- |
| `load.py` | the built map's collision triangles |
| `pa.py` | `render_minimap.play_area` with a set slope, headroom and a fine grid |
| `geom.py` | walk graph, ray casts, clearance |
| `walk2.py` | 0.1 m walk graph that bridges one riser cell (crate steps) |
| `traps.py` | directed dead-end check |
| `plan_spawns.py`, `plan_stash.py`, `write_spots.py` | the spot planning |
| `roofvis2.py`, `vox.py` | voxel sight lines from the roofs |
| `designs.py` | the roof-access designs as spec entries; `design_a2` is the built one |
| `scene.py`, `views.py`, `render_designs.py`, `render_spots.py`, `sheet.py` | software renders: perspective, plan, section and contact sheets |
| `normals.py` | vertex normals against face normals |
| `built_check.py` | the built-map check above |
