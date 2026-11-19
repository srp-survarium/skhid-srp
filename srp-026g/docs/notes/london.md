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

Source and checks: `scripts/content-edit/london-1.json` and its README section. The server data,
bots' generated map files and website maps must be built from the same released content.
