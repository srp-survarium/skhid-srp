# STALKER maps (team deathmatch)

Six STALKER multiplayer maps as one content pack, `stalker-maps@2`, the owner's
pick of 2026-10-24 on the map spot planner. Team deathmatch only (mode 4).

| maps_dict id | level (`map_id`) | project / HUD key | map | source | loading id |
| ---: | ---: | --- | --- | --- | ---: |
| 210 | 14 | `cs_mp_bath` | Experimental Pool · Бассейн | Clear Sky `mp_bath` | 30 |
| 211 | 15 | `cs_mp_military_1` | Military Warehouses | Clear Sky `mp_military_1` | 31 |
| 212 | 16 | `cs_training_camp` | Training Camp | Clear Sky `mp_training_camp` + summer meadow | 32 |
| 213 | 17 | `cs_mp_rembasa` | Repair Depot | Clear Sky `mp_rembasa` | 33 |
| 214 | 18 | `soc_ug_station` | Station | community SoC map `mp_{UG}station` | 34 |
| 215 | 19 | `soc_ug_depository` | Depository | community SoC map `mp_{UG}depository` | 35 |

- Ids: 210-215 are free in the live data (stock, patches, research-mode@3's
  145-149 and 184-203, the v0.11e maps' 206-209); the earlier local package's
  197-201 are Research's live rows, so these maps do not use them.
- Levels 14-19: the lobby names them (`RoomsText.LEVELS`, `BottomPanel`) and draws
  their room tiles with their loading picture (`RoomsLook.RENDER_ONE`).
- Names: English from the map; the other languages are translations to check.
  The game's texts are Windows-1251, so a German or Polish name with letters
  outside it takes the English one in the localization (the lobby's own tables
  keep them).
- Clear Sky's versions are the most complete: mp_bath 255,461 collision faces and
  145 rpoints with TDM teams (SoC: 228,998 and 15; CoP: 198,348 and 149);
  mp_military_1 497,056 and 170 (SoC: 383,577 and 103; a CoP version exists only
  in an untrusted patch archive). Station and Depository are community SoC 1.0006
  maps (`.xdb0`, xrLC 14, DM/TDM/AH); neither the archives nor the level files
  name an author or credit.

## Spawns and ammo stashes

From the map spot planner (its `spots` list, Claude's proposals the owner has
not edited yet): 12 spawns a team, and per team one safe stash and one in each
contested middle area. A spawn is registered for its own team only; a stash is
the native level_03 team-deathmatch `resupply_zone` of that team (the zone, its
collision box, the ammo decal and the `item/quest/container_closed` container)
moved so the container stands on the planned spot. The owner edits the spots on
the planner; the pack is then built again from the new list.

Every spot stands on the converted collision (checked in the built maps against
the floor under each one) and on ground a walker reaches without a ladder. That
check moved one Repair Depot stash 0.7 m onto solid floor and dropped one
Training Camp pair (its floor under the tower is not walkable in the build); the
planner has the same.

## Grass

Experimental Pool and Military Warehouses have the grass of their Clear Sky
levels (`level.details`), converted by the porter's recipe option
`detail_objects` (resource-porter `docs/soc-map-porting.md`, "Grass: detail
objects"). It is X-Ray's own grass: its models, its `build_details` texture,
and the positions X-Ray draws, at the same density and spread for each model,
on the collision floor. It is drawn by 0.26g's grass system (16 m patches,
`r_grass_lod1/2_distance`, the 100 m cull, `r_decorations_quality`), like
Training Camp's meadow.

| map | tufts | models | triangles LOD 0 / 1 | patches (draws per pass) | biggest patch |
| --- | ---: | ---: | --- | ---: | ---: |
| Experimental Pool | 125,844 | 13 | 1.08 M / 0.61 M | 263 | 809 tufts, 14,564 vertices |
| Military Warehouses | 77,721 | 14 | 0.61 M / 0.36 M | 295 | 536 tufts, 9,233 vertices |
| Training Camp (meadow) | 50,283 | 9 | 0.50 M / 0.10 M | 120 | 1,084 tufts, 32,520 vertices |
| native level_03 (most grass) | 90,879 | | | | |

- About 23,000 and 22,000 of them are within 20 m of the spots; the rest
  cover the levels' outer hills, which the grass system culls past 100 m.
- Not carried over: X-Ray's per-slot sun, sky and colour terms (0.26g's grass
  has one colour per instance, fixed in the engine); 0.26g's own shadows
  light it.
- The other four maps take the same option. Repair Depot's level has
  349,153 tufts at the same density (thin it with `density`); Training Camp
  has its meadow already; Station and Depository need their level folders
  again.

## Ladders

0.26g cannot climb. Every ladder stays as the source has it: no ramp, lid or
blocker is added and nothing is removed, so whatever only a tall ladder reaches
stays out of reach for now (the owner decides later). There are no low ladders
(every rise is 2.9 m or more). "Needed" means the ladder is the only way to an
area; the rest are joined on foot, lead to a few square metres, or have no floor
at an end. Position is the ladder's x, z; heights are its foot and top (y).

**Experimental Pool** (9): needed: clmbl#0 (17.67, −6.57; 4.4 → 13.7 m),
clmbl#5 (−13.98, 29.92; 8.8 → 13.4), clmbl#6 (−38.52, 26.0; 4.4 → 13.3),
clmbl#8 (−4.96, −7.54; 9.0 → 13.4), each the only way up to the upper floors and
roofs (1,507 m², 15 source spawns); clmbl#4 (20.83, 36.36; 4.4 → 7.6) to a 62 m²
area. Not needed: clmbl#1, #2, #3 (joined on foot), #7 (a 15 m² spot).

**Military Warehouses** (14): needed: clmbl#0 (71.68, −58.67; −8.9 → −0.5) and
clmbl#1 (13.19, −58.58; −8.9 → −0.6), the only ways up to a 675 m² area with 2
source spawns; clmbl#2 (−77.12, 39.04; −11.3 → −7.8) to a 503 m² area. Not needed:
#4-#9 (joined on foot or a 0 m² landing), #3, #10-#13 (no floor at an end).

**Training Camp** (1): clmbl#0 (21.75, 3.22; 16.6 → 20.2) to a 22 m² walkway with
one source spawn. The earlier package's slab over its rungs is not kept.

**Repair Depot** (2): clmbl#0 (−4.76, −33.8; 0.5 → 15.9) to an 18 m² ledge and
clmbl#1 (42.07, 18.87; 4.4 → 7.6) to a 15 m² ledge, no spawns: neither matters.
The earlier package's slabs are not kept.

**Station** (6): needed: clmbl#0 (131.36, −6.76; −12.2 → −8.5) 65 m²,
clmbl#1 (157.52, −8.09; −12.0 → −8.3) 49 m², clmbl#2 (118.0, −38.71; −12.4 → −8.0)
97 m², clmbl#5 (231.72, 23.85; −12.5 → −8.5) 314 m² with 2 source spawns. Not
needed: #3 (a 4 m² spot), #4 (26 m²).

**Depository** (6): needed: clmbl#1 (22.92, −16.14; 13.4 → 16.9) 35 m²,
clmbl#2 and #3 (4.63, 43.75 / −8.09, 33.39; 13.2 → 19.7) 173 m², clmbl#4
(15.6, −1.9; 13.5 → 17.9) 251 m² with 4 source spawns, clmbl#5 (17.48, 12.7;
18.0 → 21.4) 170 m². Not needed: #0 (no floor at its top).

## How it is built (resource-porter branch `stalker-maps-026`)

```
scripts/stalker-tdm/port-map.sh MAP [LEVEL_FOLDER [TEXTURES_FOLDER]]   # one map's v0.100b bundle, then the pack
scripts/stalker-tdm/build-pack.sh [--version N]                       # every map: donor, spec, 0.26g, pack, minimaps
```

- `port-map.sh` converts a level with `build-soc-map` (recipes
  `ports/cs-mp_bath-to-0100b.json`, `cs-mp_military_1-…`, `cs-training_camp-…`
  with the meadow addon, `cs-mp_rembasa-…`, `soc-ug-mp_station-…`,
  `soc-ug-mp_depository-…`), from the installed game or from a level folder read
  over the game's archives (a `base` + `overlay` source the porter opens as one),
  and writes every source file's sha256 (`sources/MAP.sha256`) so a re-export can
  be compared.
- `build-pack.sh` composes the six bundles into one v0.100b donor, matches every
  converted material to its template, writes the spec from
  `ports/stalker-tdm/maps.json` (ids, names, loading ids, probes) and
  `ports/stalker-tdm/spots.json` (the planner's spots), imports the maps into
  0.26g (`build-survarium-maps-026`, which now takes spawns by team, ammo stashes,
  the level number, a loading id and names by language) and makes the pack with
  `pack from-bundle`, checked over every live patch. One pack: the maps share
  converted textures and materials.
- Nothing in the steps is edited by hand: a level the owner changes in the SDK
  goes through the same commands.

### Re-exporting a map from the SDK

| map | SDK | files the porter reads back |
| --- | --- | --- |
| Experimental Pool, Military Warehouses, Training Camp, Repair Depot | the Clear Sky SDK (the CS level format) | `level`, `level.geom`, `level.cform`, `level.game`, `level.spawn`, and the level's own `terrain\*.dds` |
| Station, Depository | Shadow of Chernobyl SDK 0.4 (xrLC 14, SoC 1.0006) | the same, and the map's own `textures\` (terrain, intro and map pictures) |

The rest (shaders, materials, shared textures) comes from the installed game.
The spawns do not come from the level: they are the planner's.

## In this branch

- `srp-026g/flash/lobby_menu`: levels 14-19 in the rooms' and matches' name
  tables, their room tiles (`RENDER_ONE`), their loading pictures
  (`LobbyMenu._load_images` 30-35, classes `Level_<project>`).
- `srp-026g/flash/hud`: their minimaps (`Minimap._levels`, `_minimapProps`,
  classes `minimap_<project>`).
- `tools/actionscript`: a tree's `movie.json` may add bitmaps the original movie
  lacks (`"images"`: a DefineBitsJPEG3 per class, premultiplied JPEG and zlib
  alpha, bound by a new SymbolClass); `srp-026g/scripts/stalker-maps/ui_images.py`
  writes them from the porter's minimap render and a loading picture.
- Loading pictures: Training Camp and Repair Depot use the earlier package's
  in-game shots; the other four have a placeholder cut from the top-down render
  until an in-game shot is taken.
- `release/content.tsv`: the pack row `stalker-maps@2` (@1 without grass).

## Still to do before a release

- An in-game test of each map (local game): spawns, stashes, minimap frame,
  loading picture, names.
- The site: a top-down render per project in the calculator's maps
  (`scripts/damage-calc/assets/maps`, README) before `release/build-site`, or the
  Maps tab build stops.
- Bots: `mapdata-dump` for the new maps (`docs/notes/player_bots_mapdata.md`).
- The live pools: `SRP_MATCH_MAPS` / `SRP_ROOMS_EXTRA_MAPS` say whether the maps
  are in the match pool or rooms only.
