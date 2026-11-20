# London (level_09)

## London's tone curve (three-maps-021-lighting@9)

0.31a2's tone map (`complex_post_process_blend.ps`) is 0.26g's with the exposure key 6.0 where
0.26g has 4.5, in every program of the family; nothing else differs. Drawn with 0.26g's key,
London was 10-30 % darker than in its own build in sunlit and indirectly lit places (the headless
comparison of 2026-11-19, ten views per weather). `three-maps-021-lighting@9` adds London as a key
profile: the native tone block with key 6.0 behind London's marker, after the 0.21 maps' branches
and before the unchanged native block (resource-porter `tone_map.rs`; every other file and branch
of @8 byte-identical). The marker lives in `mie_fog_base_height`, as the 0.21 maps' do: 32909 (day,
`london-tone-1.json`), 32929 (evening) and 32919 (rain, pack london-weathers@4). Every native shader
reads that field only as `min(1, exp((base - eye_y) * falloff))`, 1 for an eye below the base, as
the authored 60/100/50 give for London's cameras (below 50 m): London's fog does not change.

In rain, 0.26g's composition darkens what the rain does not reach by a fixed 0.85; 0.31a2 reads that
factor from the environment (`rain_shadow_diffuse_multiplier`), and London's rain authors 0.7. With
0.31a2's tone curve but 0.26g's 0.85, London's rain came out brighter than 0.31a2 in every view. A
shelter profile of the same pack runs those two instructions with 0.7 behind the rain marker, in the
4 composition programs the pack converts (the rest of 0.31a2's rain composition is 0.26g's program).

Skhid's London is 0.31a2's, ported in base patch 14; Research's day_5 comes from
research-mode@3. The owner's rules:

- No anomalies, damage zones or artefacts in any mode.
- Use the owner's map-editor spawn placement: ten points per team, the same in all four modes.
  Team1 (server team 0, west) is on building_06_1's first floor, placed to match team2;
  team2 (server team 1, east) is on building_10's first floor, the owner's eight points plus
  two beside them. Preserve a point's facing when it was moved without turning it.
- These replace the earlier proposal beside the ground-floor doors. The walking distance to
  the battery container is 44–65 m for team1 and 68–74 m for team2.
- Remove the old outdoor spawn-protection zones; keep each point's own 6-second invulnerability.
- Do not include the accidental prop moves from the editor.

## London's own textures (pack london-textures)

Of the textures London's projects use, 38 differ from 0.31a2's by more than 5/255 on average
(the comparison of 2026-11-19). All but one are stock 0.26g files that every map shares (0.31a2
re-made plywood, cargo boxes, sandbags, metal and their specular maps); they stay as 0.26g has them.
The one London-only file is `textures/props/furniture/office_furniture_white.dds`, which patch 14
shipped as a cream image while 0.31a2's is wood-brown: London's cupboards with the second material
set (`office_cupboard_high[1]` and the like, used by no other project) were white. Pack
`london-textures@1` gives them 0.31a2's file.

Source and checks: `scripts/content-edit/london-1.json` and its README section. The server data,
bots' generated map files and website maps must be built from the same released content.

## Rain and evening (pack london-weathers, maps 276-281)

London's donor is 0.31a2 (`resources.03.db` of its 22 volumes holds `projects/level_09`): it ships
`day`, `evn` and `rain`, each in modes 2 to 6. No earlier build on disk has London (0.27d0, 0.28a2,
0.28d0 have levels 1-8); 0.46c2 has it again under other project names. Skhid's day is 0.31a2's
`day_2/3/4` as patch 14 converted it (not made in these repositories) with skhid's edits on top.

| Weather | Protective device | its chargers | Team Deathmatch | loading picture |
| --- | --- | --- | --- | --- |
| rain (`rain_N`, weather 2) | 276 | 277 | 278 | 28 (`Level9_rain`) |
| evening (`evening_N`, weather 3) | 279 | 280 | 281 | 27 (`Level9_evn`) |

Each weather project is the shipped day with 0.31a2's own day-to-weather difference laid over it
(resource-porter `src/weather_delta.rs`, `examples/weather-delta-pack.rs`, its manifest
`/mnt/hdd/Projects/london-weathers/london-weathers.json`):

- client objects matched by `full_name`: 149 day ambient lights give way to the weather's 163/164,
  160 probes take the weather's cubemaps (`texture` and `textures.<project>`), the lights' colours
  and ranges change; rain drops the sand and cloud particles, evening adds two cloud layers, two
  ground sand streams, one probe and one spot light; a camera `track` and the two
  `sound_environment`s have no 0.26g object and stay out;
- render visuals and collisions matched by content within 1 mm: the stand lamps become lit ones;
  `srp_lamps`' visual indices follow;
- fields skhid edited on the day (five probes' radius and side width) keep the edit;
- the environment: the shipped day's settings plus native 0.26g `level_01` rain/evening's, each set
  from 0.31a2's first key frame, as patch 14 made the day's from native `level_01/day_2`'s names.
- evening's `mie_fog_base_height` is London's tone-curve marker 32929 (three-maps-021-lighting@9);
  rain's is 32919, London's marker for its probes (pack london-probes), which the tone profile does not
  use: rain keeps 0.26g's tone curve until 0.31a2's rain composition is fitted. Every native shader reads
  the field only above the eye, so the fog does not change.

The conversion rules are patch 14's own, read off the shipped day: `is_enabled` is a light's
`enabled` and is dropped elsewhere, an ambient light's colour is packed ARGB (`round(c*255)`), a
converted object takes the field set of a shipped object of its kind. Converting 0.31a2's day with
them gives the shipped day back except where skhid edited it (decal and probe placements, rotations
written in another form).

The source day is what players have (patch 51's London projects): revision 66's full content build
found every London day file unchanged against the manifest, so it is the manifest's own output.
Six new maps raise the pinned `maps_dict` counts of five edit lists by six; a PR adding maps after
this one renumbers again.

Material sets the weathers use. 0.31a2's evening places six vine bushes (`flora/vine/bush_*`) with
their second material set (visual name `[1]`, dried leaves). 0.26g ships those models without that
set, so they drew with no material, as plain grey sheets (the owner, 2026-11-19; the log says
"Incompatible model settings ... material not found"). Of every set London's projects use, only
these were missing: `london-vines-1.json` gives the six models 0.31a2's `material_settings_1`, and its
material `flora/vine_leaves_v2_desiccated` as 0.26g's `flora/vine_leaves_v2` with 0.31a2's diffuse
`flora/vine_leaves_v2_new`, the texture pack `london-vines@1` adds. Their first set is unchanged.

## The probes (pack london-probes)

London has ten box probes `environment_and_lights/probes/reflection_window(…)` along its window
facades with `affect_diffuse = false`, as 0.31a2 authored them; no 0.26g map has such a probe
(a census of every live project: only London's 5 day projects and its weathers). In 0.31a2 such a
probe adds only its reflection: its `environment_probe_lighting.ps` permutations write the specular
accumulator alone. 0.26g's write the surface's own albedo into the diffuse accumulator under the
probe's falloff (`renderer-026g.md` 5.3), which lit the walls by the windows in their own colour.

Pack `london-probes@1` (resource-porter `probe_specular`) changes the 16 `TDIFFUSE` 0 permutations
of 0.26g's shader: the diffuse output's alpha is zero, so with the probes' blend (SRC_ALPHA /
INV_SRC_ALPHA per target) the diffuse light stays as it was; the reflection is drawn as before.
One instruction per permutation (`mov o0.w, <falloff>` → `mov o0.w, l(0)`); the permutations
with `affect_diffuse = true`, the ones every other map draws, are untouched.

London's day also sets `environment_probes_specular_intensity_multiplier` 1.2 in 0.31a2; patch 14
built the day's environment from native `level_01` day's settings, which leave it out, so 0.26g drew
London's probe reflections at its default. `london-environment-1.json` adds it to the five day
projects (rain and evening carry 0.31a2's 0.6 and 1.5 from pack london-weathers).

0.31a2 also looks a box probe's reflection up box-projected (parallax-corrected) where the probe has
`use_parallax_correction` (137 of London's 160 probes: every room probe and the ten window probes;
not the 23 background-building and outdoor ones): the cubemap is read where the reflection ray leaves
the probe's box, not along the ray from the probe's centre. Patch 14 dropped the field; 0.26g's box
programs have every constant it needs and read along the ray. `london-probes@2` (resource-porter
`probe_parallax`) gives the 8 smart box permutations, before their specular sample, a branch taken only on
London's marked scenes (`mie_fog_base_height` 32909 day, 32929 evening, 32919 rain)
that replaces the lookup direction with the box-projected one. It reaches only the
probes 0.31a2 projects: `london-probe-channel-1.json` gives those 137 probes smart attenuation, which no
London probe has, and only the smart box programs carry the branch; on London's marked scenes they
also draw the plain box falloff instead of the smart one. Other maps never take either branch.

Overlapping probes draw one over another, largest radius first: both executables sort the visible
probes the same way (`std::sort` on the render probe's radius, 0.26g 0x1B9E82, 0.31a2 0x7DBC6A), so
the radii decide whose reflection lies on top. Skhid's London came from an earlier build of the map,
and seven probes differ from 0.31a2's (radii of `gerkin(1)`, `gerkin(3)`, `hall_box_1_floor(2)`; boxes
of `hall_2_floor`, `hall_box(1)`, `ladder(1)`; places of `hall_2_floor`, `gerkin(3)`; side widths of
`hall_2_floor`, `gerkin(22)`). With radius 5, `gerkin(3)` tied `gerkin_hall_centre` and could draw
under it; with 0.31a2's 4 it draws after, as there (the corridor of the owner's FFA pick 11).
`london-probe-shapes-1.json` sets the seven as 0.31a2 authors them in each project.
