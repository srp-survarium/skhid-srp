# Blackout (prototype)

The owner (2026-10-24): "make a game with all ui disabled and players having color bands to
distinguish teams… this is the new battery mode". A prototype to try locally; the owner's
changes of 2026-10-25 are in. Nothing here ships until the owner says so.

## The mode

- **Its own mode in the lobby:** "Protective device: Blackout" (Захисний пристрій: Блекаут, Защитное
  устройство: Блэкаут, Schutzeinrichtung: Blackout, Urządzenie ochronne: Blackout): a tick of
  the rooms' create window, a card of the vote, a row of the room and match lists.
- **Offered** while the server runs with `SRP_BLACKOUT=on` (off by default): the rooms' pool then
  holds each plain battery map of the match pool once more as Blackout.
- **Rules:** the plain battery mode (mode 2, "Protective device": batteries from the spawners,
  no chargers), on its maps; the battery rules unchanged. Not the chargers' mode (3).
- **Chosen, never given:**
  - out of the create window's defaults: a maker ticks it;
  - Play never puts anyone into a room whose pool offers it, and a room Play makes has none;
  - so only who made or joined such a room plays it; a room with Blackout and other modes plays
    whichever its vote picks.
- **Bots:** not offered (Play vs Bots lists its own maps).

## In the match

- **Nothing over the world:** no names, markers or icons over any player or object, none through
  walls, no spot marks, no Research markers (the markers' movie is given nothing to draw).
- **No interface:** what F11 hides (patch 62) for the whole match, from its first frame: ammo,
  health, score, crosshair, minimap, kill feed, chat, Tab list, the match clock, the kits and the
  death screen with its respawn timer.
- **One exception, the pre-match countdown** ("… 00:15"): shown while it runs, hidden with the
  rest when play starts.
- **Tab held:** the ammo (in the weapon and carried) and the quick-use slots with their counts;
  hidden again when Tab is let go. Never the player list or the score (the owner, 2026-10-25).
- **Kept:** the screen effects (pain edges, blood, grenade flash), the Esc menu, the kit screen's
  cursor; keys 1 to 3 pick a kit.
- **F11 does nothing**; a replay's controls still toggle with it.
- **The end:** the lobby and its results screen as always.
- **Replays and late joiners** see the match as it was (no interface, the bands).
- **Not a simulation change:** what is drawn only; no `simulation_changes.tsv` row.

## Bands

- **Look:** an armband on both upper arms of the game's own woven cloth (the Grey fabric
  camouflage: its weave normal and roughness maps, the scavengers torso's cloth material, so it
  lights as the sleeve does), tinted a worn navy (team 0, sRGB 38,64,122) or brick red (team 1,
  146,50,36) with a little grain, fading and dust; no emblem, not self-lit. The same colours for
  every viewer; red and blue stay apart in the dark (CIELAB distance 16 to 21 in a dark render,
  56 to 59 in a neutral one). Of three looks made 2026-10-25, clean colours on the cloth (A), worn
  colours on plain cloth (B) and this (C), C is in the build; the owner chooses.
- **Fit:** one band per torso model, cut from that torso's own sleeve (0.45 to 0.67 of the way
  from shoulder to elbow, 60 mm wide, 4 mm out) and skinned with the sleeve's own weights, so it
  moves with the sleeve. Measured on the model data: never more than 4 mm off the sleeve in the
  bind pose and the AK and pistol idles and run; 3 to 13 % of a band's edge runs under straps or
  plates next to the sleeve. It goes 78 to 100 % round the arm (less where a sleeve is the chest's).
- **Content:** pack `blackout-bands@5` (resource-porter `feat/blackout-bands`): 70 torsos x 2
  colours, `models/ported/blackout/<torso base_model, '/' as '_'>_<blue|red>`, surface `band`; no
  item, no dictionary row. A torso the pack does not know gets no band.
- **v1** (STALKER's multiplayer bands, one ring for all torsos) floated: one size outside every
  sleeve, low on the arm, rigid on the upper arm. Dropped.

## How

- **Lobby (srp):** Blackout's maps are named by ids of the lobby's own (`maps::blackout_pairs`:
  ids `maps_dict` does not use, from 255 down), mode 6 in the rooms' pool, cards and rows. A
  room's match on such a card is made on the battery map with `MatchSetup::blackout`.
- **Static info:** the flag is tag 2 (one byte, 1) of srp's block after the squads, on battery
  maps only; a late joiner's static info keeps it.
- **Exe, patch 65** (`blackout_026g.asm` and `.c`; `client_patch.md` 3.58):
  - patch 63's reader of srp's block keeps the flag in the options (+57870h);
  - patch 62's reset of a match's interface calls `hide_ui_force(bo_match())`: the one entry that
    forces the interface hidden (the hud's `root.force_interface_hidden`, the markers'
    `root.hide_interface`) and locks F11;
  - Research's per-frame entry of `update_player_icons` runs neither it nor the units' markers in
    a Blackout match;
  - `profile_skin_visual_cook::on_configs_loaded` adds the band of the player's torso and team.
- **The hud movie:** `force_interface_hidden` hides the root as F11 does and keeps the countdown
  (`pregameInfo`) in a layer over it, placed as the root; while forced, the game's Tab
  (`show_player_list`) moves the ammo (`ammoInd`) and the quick-use slots into that layer and back
  instead of showing the list; the reset of the next match unforces it.
- **The lobby movie:** `GameMode.BLACKOUT`, its names; the create window ticks the modes the pool
  offers; a mode's colour on the vote card from one table.
- **With the interface levels** (`feat/interface-level`, its own patch 64): the forcing call
  becomes `interface_force(0, 2)` (Disabled) once both are merged, its countdown exception kept.

## Open for the owner

- The name; relative colours (own team blue, enemy red) instead of fixed ones.
- A room's saved settings from before Blackout existed show it ticked (they cannot tell "chosen"
  from "unknown"); a schema step fixes that before a release.
- A game without patch 65 would show the interface and no bands: refuse it by revision before a
  release (`SRP_MIN_PATCH_REVISION`).
