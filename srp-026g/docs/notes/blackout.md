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
- **Tab held stops the player:** no walking, sprinting or jumping (a crouch stays, it does not
  move); no fire, aim, reload, held breath, grenade, melee, use, drop or quick use. The mouse
  look, the weapon keys, the fire mode, spot and the suicide key stay.
- **On Tab's release:** the moves come back at once; an act whose key was held through Tab acts
  again only after it is let go and pressed again (nothing fires on the release).
- **In step for everyone:** the local input is filtered before it is sent, as if those keys were
  not pressed; no rule of the match changes (every game, a late joiner and a replay play the
  input sent).
- **Kept:** the screen effects (pain edges, blood, grenade flash), the Esc menu, the kit screen's
  cursor; keys 1 to 3 pick a kit.
- **F11 does nothing**; a replay's controls still toggle with it.
- **Esc works as in any match** (the owner, 2026-10-25: "so people could leave"): the Esc menu is
  the main menu movie (its leave_match, settings and exit_to_os callbacks, game_options
  0x1FF5D0) and its cursor its own movie; the forced hiding touches the hud and the markers'
  movies only, and Tab's filter only the input's move and act bits, so the countdown, being
  alive, dead or holding Tab change nothing of it. After leaving, the next match is set again by
  its own static info and reset (shown and free unless it is Blackout too).
- **Points count twice** (the owner, 2026-10-25: "everything should give twice as much match
  points"): the results screen's score and every line's silver, experience and reputation of a
  Blackout match are twice a plain match's; the counts shown (kills, help) and the player's
  statistics, quests and rating are as played; the team's points and the win are the match's.
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
- **Fit:** one band per torso model, on that torso's own sleeve (its bind-pose surface between
  0.55 and 0.77 of the way from shoulder to elbow, 6 mm out), its two edges planes square to the
  upper arm bone; skinned to the upper arm's own bones only (the arm and its roll, the sleeve's
  twist share), so it turns with the arm and stays square in every pose (worst lean 2.1 degrees
  over all torsos in the idles, run and death clips). The owner saw a diagonal stripe (2026-10-25):
  pack 5 copied the sleeve's weights, which lean it up to 32 degrees in poses. Where a
  sleeve sags in a pose the band stands off it by 1 to 3 cm (heavy suits most); on the Duty and
  Freedom exoskeletons it shows in pieces between the frame bars.
- **Content:** pack `blackout-bands@6` (resource-porter `feat/blackout-bands`): 70 torsos x 2
  colours, `models/ported/blackout/<torso base_model, '/' as '_'>_<blue|red>`, surface `band`; no
  item, no dictionary row. A torso the pack does not know gets no band.
- **v1** (STALKER's multiplayer bands, one ring for all torsos) floated: one size outside every
  sleeve, low on the arm, rigid on the upper arm. Dropped.

## How

- **Lobby (srp):** Blackout's maps are named by ids of the lobby's own (`maps::blackout_pairs`:
  ids `maps_dict` does not use, from 255 down), mode 6 in the rooms' pool, cards and rows. A
  room's match on such a card is made on the battery map with `MatchSetup::blackout`.
- **Rewards (srp):** the match server's result carries `blackout`; the rewards' score and each
  line's pay are times `rewards::BLACKOUT_FACTOR` (2) before rounding; nothing in the match
  computes otherwise (the hud's score is hidden).
- **Static info:** the flag is tag 2 (one byte, 1) of srp's block after the squads, on battery
  maps only; a late joiner's static info keeps it.
- **Exe, patch 65** (`blackout_026g.asm` and `.c`; `client_patch.md` 3.58):
  - patch 63's reader of srp's block keeps the flag in the options (+57870h);
  - patch 62's reset of a match's interface calls `hide_ui_force(bo_match())`: the one entry that
    forces the interface hidden (the hud's `root.force_interface_hidden`, the markers'
    `root.hide_interface`) and locks F11;
  - Research's per-frame entry of `update_player_icons` runs neither it nor the units' markers in
    a Blackout match;
  - `profile_skin_visual_cook::on_configs_loaded` adds the band of the player's torso and team;
  - the game world's Tab (its `show_player_list` call, 0x25CD34) is kept, and the input builder's
    end (`process_first_person_mode`, 0x2421B0) takes the moves and acts out of the actions mask
    while it is held.
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
