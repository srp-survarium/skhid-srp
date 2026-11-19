# London (level_09)

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
