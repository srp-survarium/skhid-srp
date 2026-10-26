# Blackout (prototype)

The owner (2026-10-24): "make a game with all ui disabled and players having color bands to
distinguish teams… this is the new battery mode". A prototype to try locally; the owner's
changes of 2026-10-25 and 2026-10-26 are in. Nothing here ships until the owner says so.

- **No bands for now** (the owner, 2026-10-26: "bands are fine, but we will do release without
  them, instead player markers should still be visible"): the painted team bands are parked on
  branch `feat/blackout-bands` (the per-torso table `scripts/blackout-bands/bands.tsv`, its
  tools, the exe's part and srp's band in the profile), ready for later.

## The mode

- **Its own mode in the lobby:** "Protective device: Blackout" (Захисний пристрій: Блекаут, Защитное
  устройство: Блэкаут, Schutzeinrichtung: Blackout, Urządzenie ochronne: Blackout): a tick of
  the rooms' create window, a card of the vote, a row of the room and match lists.
- **Offered** while the server runs with `SRP_BLACKOUT=on` (off by default): the rooms' pool then
  holds each plain battery map of the match pool once more as Blackout.
- **Rules:** the plain battery mode (mode 2, "Protective device": batteries from the spawners,
  no chargers), on its maps; the battery rules unchanged. Not the chargers' mode (3).
- **Part of the standard pool** (the owner, 2026-10-25): in the create window's defaults, in every
  room Play makes; Play puts players into rooms that offer it; a room's maker may untick it.
- **How often:** each Blackout map is one more entry of the pool, as any map; the vote's three cards
  are drawn from the pool's entries alike. With the live pool (100 entries, 25 of them plain
  battery maps) Blackout adds 25 of 125: a card is Blackout 1 time in 5, and about half of the
  votes (49 %) offer at least one Blackout card; the vote picks.
- **Bots:** not offered (Play vs Bots lists its own maps).

## In the match

- **The players' markers stay:** the markers and names over players as in a plain battery match
  (teammates' as the game shows them; an enemy's name never, the game's own rule). The match's
  objects' markers (the batteries and the like) are hidden; no Research markers.
- **No interface:** the rest of what F11 hides (patch 62) for the whole match, from its first
  frame: ammo, health, score, crosshair, minimap, kill feed, chat, Tab list, the match clock, the
  kits and the death screen with its respawn timer.
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
- **Replays and late joiners** see the match as it was (no interface but the players' markers).
- **Not a simulation change:** what is drawn only; no `simulation_changes.tsv` row.

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
    forces the interface hidden (the hud's `root.force_interface_hidden`; the player icons shown
    as at every reset) and locks F11;
  - the player icons' movie is told the forced state from its feed (Research's per-frame entry of
    `update_player_icons` calls patch 62's `hide_ui_icons` first): its
    `root.force_interface_hidden` at a change of the movie (the game world's own) or of the state;
  - the game world's Tab (its `show_player_list` call, 0x25CD34) is kept, and the input builder's
    end (`process_first_person_mode`, 0x2421B0) takes the moves and acts out of the actions mask
    while it is held.
- **The hud movie:** `force_interface_hidden` hides the root as F11 does and keeps the countdown
  (`pregameInfo`) in a layer over it, placed as the root (and still placed by the hud's own
  layout there, `Layout.manage`: centred as in any match, at every resolution and interface
  scale, also when kept before the layout's first pass at the match's start); while forced, the
  game's Tab (`show_player_list`) moves the ammo (`ammoInd`) and the quick-use slots into that layer and back
  instead of showing the list; the reset of the next match unforces it.
- **The player icons' movie:** `force_interface_hidden` hides the match's objects' icons (ids from
  37, `HUDIconLayer.FIRST_OBJECT`: the game gives a player's icon his index and its objects' from
  25h) and keeps the players'; told again after the next match's reset; F11's `hide_interface`
  is apart.
- **The lobby movie:** `GameMode.BLACKOUT`, its names; the create window ticks the modes the pool
  offers; a mode's colour on the vote card from one table.
- **With the interface levels** (`feat/interface-level`, its own patch 64): the forcing call
  becomes `interface_force(0, 2)` (Disabled) once both are merged, its countdown exception kept.

## Open for the owner

- The name; the bands (parked, `feat/blackout-bands`).
- Saved room settings: a mode is in unless its maker left it out, so settings kept before Blackout
  have it ticked, as the defaults do; no schema step is needed.
- A game without patch 65 would show the interface: refuse it by revision before a
  release (`SRP_MIN_PATCH_REVISION`).
