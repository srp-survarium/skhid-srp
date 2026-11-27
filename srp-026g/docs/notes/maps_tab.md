# Maps (`/wiki/maps`)

The 0.26g wiki's section "Maps" / "Мапи" (until 2026-10-29 the site's tab `/maps`, which now sends
to it with a `301`). Built by `srp-026g/scripts/maps-tab/` (its README),
served by `servers/web-server/src/maps.rs`.

## Owner's rules

- Every playable map, the ported ones too. The points shown:
  - player spawns (by team, turned the way the player faces);
  - battery spawns;
  - chargers (they spawn batteries);
  - battery containers;
  - ammo stashes (the teams' resupply zones);
  - artefact spawns.
- Research (mode 5) shows its three units, A to C, where its config puts them (`parameters.capture_zones`), as the
  HUD draws one nobody owns (a white letter on grey; its icon is drawn in code, not one of `_icons`).
- A mode's second layout (School's Research, `research_mode.md` "Layouts") is a choice of its own beside the mode,
  "Research · 2", with its own units; its address is `#level_02/5.2`.
- Artifact Hunt (mode 13; the owner, 2026-11-23: "i am talking about artifact spawns in artifact hunt") shows where
  its artefact appears (`artefact_hunt.spawners`), drawn as the HUD's minimap draws it lying (the orb), and the two
  teams' bases (`artefact_hunt.bases`, the HUD's `Base` icon by team), with its own project's spawns. Freeze Tag
  (mode 12) is a mode of every map that has it, with its spawns.
- Free for All (mode 8; the owner, 2026-11-13: "add to the wiki new mode with its spawns and a map") is a mode of
  every map that has it, and the maps played in it alone (the STALKER ports) are on the page with that one mode.
  It shows the spawns its players get: the team-2 points of its own project (`ffa.md`), never the two teams' points
  that project keeps. One kind of spawn: one swatch, no team in the tooltip. Yellow is not the owner's pick: the
  grey of a point of no team did not read on the minimaps.
- The markers are the game's own HUD minimap icons (hud.swf `Minimap._icons`), painted in one blue and one red alike
  (players, containers, ammo stashes: no discs). The battery is the Protective device's mode icon (the owner,
  2026-11-27: "for batteries let's use 'protective device' mode icon on wiki"; "it is the original one everyone
  knows"): the lobby's loading-screen icon (`SrvGameModeIcons`, a 61x55 bitmap), the very file the modes page shows
  (`/wiki/icons/modes/`), in its own colours with the outline, 24 px on the map and 18 in the legend (the file is
  64 px: sharp at twice the density). It replaced the kill feed's battery without its burst (2026-11-15).
  Artefacts are their lobby icons alone, no circle.
- The site's header: the calculator's look (paper, ink rule, PT Sans Narrow), separators between Home | Wiki,
  the page shown in the accent colour (Home too). The calculator and the maps are sections of the wiki (the owner,
  2026-10-29): listed on its index, each page under a "Wiki / <page>" line; the calculator's page says
  "Damage calculator".
- Containers and stashes stand where their collision zone is, never at the object's pivot.
- Each kind is toggled on its own; one toggle per artefact.
- The map underneath is the game's minimap, or the top-down render ("From above") on the player's pick. Nothing else.
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

## Anomalies

The owner (2026-11-26): "one thing we miss on our wiki with artifacts is rules, how many can spawn of which kind
and when and how"; "we should show anomalies there and for each one some kind of floating window"; on the maps.

- A toggle "Anomalies" draws each anomaly that gives artefacts (`generic_anomaly_core` with `artefacts_enabled`
  and containers; Keeper's hidden pool has none) as its zones from above, tinted by what it holds (each kind its
  glow's colour, `artefacts.md` "The stock glow"; two kinds in stripes of both). A zone that hurts is filled, a
  harmless one (sound and sight) dashed.
- Twins: two anomalies on the very same zones, one hurting and the other only seen and heard (Rudnya's Lifebone
  and Spring, v0.69 Rudnya's too; Chemical Plant's Spring and Onyx, v0.11's too). Drawn apart, the harmless one lay
  on top and its card said "no damage" where the ground hurts. The build marks the later one (`twin`, the index
  of the first); the page draws the place once, in both kinds' stripes, and its card lists each anomaly's
  artefacts and timing, then the place's damage.
- Pointing at an anomaly or at one of its artefacts (or tapping it, which keeps it until a tap elsewhere) dims the
  others and shows its card: what it holds (each kind and its count), the first artefact's window and each next
  one's, the chance where it is below 100 %, how many may lie uncollected, its containers and their search time,
  how often its zones switch (Vostok's electric pairs), what a pickup starts (the next window, the container's
  cooldown), and "its own zone does no damage" for an idle-only one.
- A line under the map gives the rule the card's numbers follow (the 0.26g exe, `dynamic_anomalies.md` section 2,
  `bots/anomalies.rs`): a due anomaly draws its chance (a miss tries again a window later), then puts a random free
  artefact of its pool into a random free container of its own (empty, cooldown over); it tries again a window later
  until `max_artefacts_count` lie in its containers; a pickup makes it due again a window later. A free artefact is
  one nobody has: the pool caps how many of its artefacts are out at once.
- Each zone that hurts is a row of the card: its damage type (the equipment pages' names), its hit as a share of each
  part's health before armour (a continuous zone's per second, `max_hit` x 1000 / `interval_in_msec`; "deadly" from
  1 a second; another's per hit, at most once an interval), and its parts as the site names them, head to feet
  ("whole body" for all eight). Zones of one type, parts and timing are one row with the range of their hits
  (School's fruit cloud: five levels).
- The v0.11 and v0.21 ports' `electric_hit_damage_zone` and v0.21 School's `fruit_toxic_cloud_explosion` list
  `body` and `head`, their builds' part names, which 0.26g's body has not: the card leaves them out and the build
  says so. What 0.26g does with an unknown name is not read (LIKELY: no part, no damage there): a port to check.
- The containers' search time and cooldown are the same for every anomaly of each map (6 s and 80 s; 5 s on the
  v0.21 ports): said once in the line under the map; a map whose anomalies differed would have them on each card.
- Not on the page until checked in a game: the short spawning state of an artefact in a container before it can be
  taken (`m_time_left_to_spawn`), and whether an artefact a full-slots pickup replaces goes back to its pool.
- Every number is the release's data: nothing typed in. The modes of a map have the same anomalies (the build stops
  otherwise).
