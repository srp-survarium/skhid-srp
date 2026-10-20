# Maps tab (`/maps`)

The 0.26g site's tab "Maps" / "Мапи". Built by `srp-026g/scripts/maps-tab/` (its README),
served by `servers/web-server/src/maps.rs`.

## Owner's rules

- Every playable map, the ported ones too. The points shown:
  - player spawns (by team, turned the way the player faces);
  - battery spawns;
  - chargers (they spawn batteries);
  - battery containers;
  - ammo stashes (the teams' resupply zones);
  - artefact spawns.
- The markers are the game's own HUD minimap icons (hud.swf `Minimap._icons`), painted in one blue and one red alike
  (players, containers, ammo stashes: no discs). The battery is the game's electrobox drawn flat (the game has no
  flat icon of it). Artefacts are their lobby icons alone, no circle.
- The site's header: the calculator's look (paper, ink rule, PT Sans Narrow), separators between Home | Calculator |
  Maps, the page shown in the accent colour (Home too). The calculator's tab is "Calculator"; its page says
  "Damage calculator".
- Containers and stashes stand where their collision zone is, never at the object's pivot.
- Each kind is toggled on its own; one toggle per artefact.
- The map underneath is either the top-down render ("From above") or the game's minimap. Nothing else.
- Data-driven, per release, never stale:
  - everything is read from the released patches;
  - a live release rebuilds it from the patches it offers (`release/build-site`), never on a "don't forget"
    basis; its record commits it.
- The calculator's look; English and Ukrainian; names from the game's localization. Little text.
- A mode shows only what its rules use. Chargers appear in the timed battery mode only, with "N active"
  from the mode's `spawn_points_count`; that mode shows no battery spawns (its batteries come out of the chargers).
- No zoom or drag: the map shows its whole play area. The chosen map and mode are in the address (`#level_01/3`).
- Maps in one frame (a port and its original, such as the three Vostoks) show the same view: switching between
  them, nothing jumps.
- The view and toggles are remembered in the browser.
