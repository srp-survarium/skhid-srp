# Top-down map images for the 0.26g damage calculator

One orthographic top-down render per map (1536 x 1536 px, webp, 0.12-0.18 m/px).
Each image covers exactly the square frame the in-game HUD minimap uses.
`maps.json` gives the pixel-to-world transform, the maps_dict ids and a 64 x 64
elevation grid for every map.

| key | name | m/px | maps_dict ids |
|---|---|---|---|
| level_01 | Rudnya | 0.1693 | 61-66, 117-119 |
| level_02 | School | 0.1432 | 67-72, 120-122 |
| level_03 | Vostok Radar Station | 0.1302 | 73-78, 123-125 |
| level_04 | Chemical Plant | 0.1497 | 79-84, 126-128 |
| level_05 | Tarakanovsky Fort | 0.1172 | 85-90, 129-131 |
| level_06 | Mamayev Kurgan | 0.1302 | 91-96, 132-134 |
| level_07 | Vector Laboratory | 0.1302 | 97-102, 135-137 |
| level_08 | Cologne | 0.1198 | 103-106, 109, 110, 138-140 |
| v021d_level_03 | Vostok Radar Station (v0.21) | 0.1302 | 166, 152 (ported, live since revision 20) |
| v021d_level_06 | Mamayev Kurgan (v0.21) | 0.1302 | 175, 153 (ported, live since revision 20) |
| level_09 | London | 0.1823 | 181-183 (skhid patch 14) |
| v011e_level_03 | Vostok Radar Station (v0.11) | 0.1302 | 208, 209 (ported, patch 22) |
| v011e_level_04 | Chemical Plant (v0.11) | 0.1823 | 206, 207 (ported, patch 22) |
| v069d_level_01 | Rudnya (v0.69) | 0.1693 | 204, 216, 217 (ported, patch 40) |

All weathers and modes of a level share one image. The mode projects differ only
in mode props such as battery racks, containers and charging stations; the
layout is the same.

## Using the transform

Pixel `(px, py)`: (0,0) is the top-left corner of the image, and pixel centres are at +0.5.

    x = posX - size + px * mpp        z = posY + size - py * mpp        mpp = 2*size / 1536

`maps.json` stores this as `pixel_to_world_affine` `[a,b,c,d,e,f]`, with
`x = a*px + b*py + c` and `z = d*px + e*py + f`. Image right is +x and image up
is +z. Distance in metres is `hypot(dx, dz)`. To get the slant distance, add
`dy` from `elevation.heights`: the rows run north (z_max) to south and the
columns run west to east. The grid holds terrain heights, so it does not give
floor heights inside buildings. If the image is displayed scaled, convert the
click position back to the 1536 px image first.

## How the transform was derived

- hud.swf (retail build 2777) defines the frames. The class
  `survarium.controls.Minimap` has a `_minimapProps[project_name] = {size, posX, posY}`
  entry per map, and `setProps` and `updatePlayerPos` place the player at
  `((x - posX + size), (posY - z + size)) * bitmap_width / (2*size)`. The
  engine passes the player position as (x, -z).
- The `levelmap_min/max` values in the level_01/02/04/05 client_projects describe
  the same frames.
- The images are rendered straight into those frames. Each one is a top-down
  z-buffer render of every LOD0 render model in `projects/<level>/day_2/client_project`,
  taken from the pristine `resources.db`. Textures use mip-mapped bilinear
  sampling with vertex-blend layers and alpha test. Shading is Lambert lighting
  from the height map plus cast shadows from a north-west sun.
- Because the render rasterizes world coordinates directly, the scale is exact
  by construction.
- On the native maps, the native minimap's play-area outline is drawn in yellow,
  and ground outside it is dimmed (`minimap_area.py`). The 0.26g bitmaps are
  clear outside the play area (alpha > 0.15); London's fades out in a vignette
  around a more opaque play area (alpha > 0.4); the 0.11e bitmaps are opaque
  with a blurred edge, and their play area is what their drawn light outline
  encloses.
- London (patch 14) and the 0.11e ports (patch 22) have their own minimaps and
  frames in the patch-22 hud.swf (`_minimapProps` / `_levels` keyed by their
  project_name), and their renders use those frames. Their geometry is read
  with the released patches mounted over the 0.26g db (`extract_level.py --patch`).
- The ported 0.21d maps use project_name `level_03` / `level_06`, so the game
  shows them in the native level_03 / level_06 HUD frames. Their renders use the
  same frames. They have no outline, because the native bitmaps show the 0.26g
  layout. Their geometry comes from the content pack
  `/mnt/hdd/Projects/release-20/packs/maps-021d-7/files`, with the 0.26g db
  supplying the rest.

## Verification

Results are stored in `maps.json -> maps.<key>.verification` and in `check_<key>.json`.

1. **Placed objects (`check_<key>.png`).** The check images plot every object
   from the day_2, day_3 and day_4 projects: respawn points (team 0 cyan,
   team 1 red), battery containers (yellow squares), battery spawners (green)
   and charging stations (magenta). There are 1,869 points across the 13 maps.
   - None of them falls outside its image. They land inside barracks, hangars,
     the bridge and the fort, where you would expect them.
   - Height test: an object must not stand more than 0.5 m above all rendered
     geometry at its pixel.
     - Real transform: 0-2 objects fail per map. Level_04 has 10, and all 10 are
       mode-3 spawners and charging stations or a container standing on mode
       props, which are not in the day_2 render.
     - Deliberately wrong transform, as a control: 4-106 fail per map with z
       mirrored, and 4-44 with a 5 m shift. (London, v011e_level_03 and
       v011e_level_04: 0 fail; controls 52/7/12 mirrored, 9/1/13 shifted.)
2. **Native minimap alignment (`align_<key>`).** The developers' own minimap
   bitmaps were compared with the render's building mask while searching scale
   0.95-1.05 and shift ±8 m.
   - The best fit is scale 1.000 ± 0.005 with a shift of 1 m or less on level_01,
     02, 03, 04, 06 and 07.
   - On level_05 (fort) and level_08 (Cologne bridge) the mask metric is weak
     (trench lines, bridge steel), so the best fit landed at the edge of the
     search range. A visual overlay of the native bitmap on the render matches
     the outlines exactly.
   - London and the 0.11e maps: best scale 1.015, 0.995 and 1.005 with shifts of
     0.5, 1.1 and 1.5 m. Their bitmaps are painted rather than block masks, so
     the metric is weaker; a side-by-side of bitmap and render matches.
3. **Measured distance.** The long south building of level_03 measures:
   - 50.05 m from the model vertices,
   - 49.2 m on the render,
   - 49.2 m on the native minimap (0.39 m/px).

## Regenerating

Scripts are in `tools/`. Their working copies are in `../maps-work/`. Run them
with a Python that has pillow, numpy and scipy, for example
`nix build --impure --expr 'with import <nixpkgs>{}; python3.withPackages(p:[p.pillow p.numpy p.scipy])'`.
Always use `python3 -I`.

    DB=/home/sheep/games/Survarium-0.26g-original/Survarium/game/resources.db
    DBGRAB=<skhid-srp>/tools/re-kit/common/dbgrab/target/release/dbgrab
    W=<scratch>/maps-work                       # tools copied here; outputs in $W/out
    # 1. native minimap bitmaps (FFDec export of flash_movies/hud.swf images) -> <scratch>/ex/hud/images
    # 2. raw projects for the check overlays
    $DBGRAB $DB get projects/level_0 $W/db
    # 3. render inputs per level (LOD0 models, materials, diffuse textures as <=256 px PNG; ~60-80 MB each)
    for l in 01 02 03 04 05 06 07 08; do python3 -I extract_level.py $DBGRAB $DB level_$l/day_2 $W/lv/level_$l; done
    PACK=/mnt/hdd/Projects/release-20/packs/maps-021d-7/files
    for l in 03 06; do python3 -I extract_level.py $DBGRAB $DB v021d_level_$l/day_2 $W/lv/v021d_level_$l $PACK; done
    # the maps a released patch added (London: patch 14; the 0.11e ports: patch 22) are read with the
    # released patches mounted over the db, as the game does (the highest patch with a file wins)
    PATCHES=$(for n in 12 14 17 19 20 21 22; do printf ' --patch %s' <patches>/resources.db.patch_$n; done)
    for k in level_09 v011e_level_03 v011e_level_04; do python3 -I extract_level.py $DBGRAB $DB $k/day_2 $W/lv/$k $PATCHES; done
    # their raw client_projects (day_2/3/4 as the patches give them) go to $W/db like the stock ones
    # Rudnya (v0.69), from its pack (rudnya-069d) over the db; native Rudnya's frame, minimap and play area
    python3 -I extract_level.py $DBGRAB $DB v069d_level_01/day_2 $W/lv/v069d_level_01 <packs>/rudnya-069d-3/files
    # (extract_level.py before 2026-10-19 lost every surface whose name ends in "_", e.g. level_08's moss
    # and ground cover, level_03's terrain_all; all 13 maps were re-rendered with the fix)
    # 4. render + shade + check + webp (~1 min per map)
    python3 -I build_all.py $W <scratch>/maps --hud-images <scratch>/ex/hud/images --rerender
    # 5. checks against the native minimaps, then maps.json
    python3 -I align_check.py $W/out level_03 <hud-images>/976_minimap_base_minimap_base.png 100 $W/out/align_level_03.json   # per native map
    python3 -I measure_building.py level_03 100 14 -42.196 <hud-images>/976_minimap_base_minimap_base.png -28 -73 > $W/out/measure_level_03.json
    python3 -I make_json.py $W <scratch>/maps <skhid-srp>/srp-026g/crates/survarium/src/v026g/maps/table.rs
    # or, to add or redo some maps in an existing maps.json (the patch-added ones need no table.rs):
    python3 -I make_json.py $W <dir with maps.json> - --only level_09 v011e_level_03 v011e_level_04
    python3 -I play_area.py <dir with maps.json>/maps.json <patch-22 hud images>   # the play-area crops

The rasterizer was copied from the playable-map-pack's `render_minimap.py`.

## Limits

- Roofs are rendered as roofs, as in a satellite photo, and trees hide some ground.
  Indoor-heavy maps such as level_07 (Vector Laboratory) show the lab's roofs,
  much like the native minimap's dark blocks.
- The elevation grid holds terrain heights only, so floors inside or on top of
  buildings are not in it.
- The ported maps that are not live (194-201) are not included.
