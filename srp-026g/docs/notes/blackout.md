# Blackout

The owner (2026-10-24): "make a game with all ui disabled and players having color bands to
distinguish teams… this is the new battery mode". The owner's changes of 2026-10-25 and
2026-10-26 are in. Since 2026-10-28 it is a normal mode, with no server feature toggle.

- **No bands for now** (the owner, 2026-10-26: "bands are fine, but we will do release without
  them, instead player markers should still be visible"): the painted team bands are parked on
  branch `feat/blackout-bands` (the per-torso table `scripts/blackout-bands/bands.tsv`, its
  tools, the exe's part and srp's band in the profile), ready for later.

## The mode

- **Its own mode in the lobby:** "Protective device: blackout" (Захисний пристрій: блекаут, Защитное
  устройство: блэкаут, Schutzeinrichtung: Blackout, Urządzenie ochronne: blackout: as each language
  writes "Protective device: chargers"): a tick of the rooms' create window, a card of the vote, a
  row of the room and match lists. Listed with the protective device's kinds: plain, chargers,
  blackout, then team deathmatch and Research (the owner, 2026-10-26).
- **Always offered:** the rooms' pool
  holds each plain battery map of the match pool once more as Blackout.
- **Rules:** the plain battery mode (mode 2, "Protective device": batteries from the spawners,
  no chargers), on its maps; the battery rules unchanged. Not the chargers' mode (3).
- **Part of the standard pool** (the owner, 2026-10-25): in the create window's defaults, in every
  room Play makes; Play puts players into rooms that offer it; a room's maker may untick it.
- **Always the vote's third card** (the owner, 2026-11-02: "offers blackout in voting always as a
  third choice"; before, each Blackout map was one more entry the three cards were drawn from
  alike, so about half of the votes had a Blackout card, anywhere): every vote, public or
  private, of a room whose pool has Blackout.
  - Cards 1 and 2: drawn from the rest of the pool, as before; never Blackout.
  - Card 3: one of the pool's Blackout maps at random, on a map cards 1 and 2 are not while
    there is one; won, its match is Blackout (as any Blackout card's).
  - No switch: Blackout is a mode like the others. A pool without it (a private room's maker
    unticked it, or ticked no battery map) or of Blackout alone: its three cards drawn as
    before. A public room's pool always has it (the live pool has 25 battery maps).
- **Bots:** not offered (Play vs Bots lists its own maps).

## In the match

- **The teammates' markers, always** (the owner, 2026-10-31, release 40: "in hardcore mode let's
  show markers of teammates"; before, 2026-10-26, only while Tab was held): the marker over each
  teammate's head as a plain battery match shows it, the whole match, alive or dead, Tab or not.
  - **Enemies:** never a marker (the game's own rule, as in a plain match).
  - **Names:** part of the same marker, so as a plain match shows them: an Alt key held or the
    crosshair on the teammate; and, as before, every eligible teammate's while Tab is held. An
    enemy's name never.
  - **Still hidden:** the minimap (its teammates' dots too: the minimap is the hud's, not a
    marker), the match's objects' markers (the batteries and the like), Research markers, and
    everything else below.
- **No interface:** the rest of what F11 hides (patch 62) for the whole match, from its first
  frame: ammo, health, score, crosshair, minimap, kill feed, chat (but while typed in and on Tab), Tab list, the match clock (but on Tab), the
  kits and the death screen with its respawn timer.
- **One exception, the pre-match countdown** ("… 00:15"): shown while it runs, hidden with the
  rest when play starts.
- **Tab held:** the chat's history, the ammo (in the weapon and carried), the quick-use slots with their counts, the
  body's state (the doll: the body parts, bleeding, radiation, toxins; the owner, 2026-10-26), all
  teammates' names over their markers and the time left until the match ends (the hud's own match timer, in its normal place at the top of the
  screen, the rest of the bar hidden; the owner, 2026-10-26); hidden again when Tab is let go.
  Never the player list or the score (the owner, 2026-10-25).
- **The chat** (the owner, 2026-10-26: "should be seen when typing and tab should show history"):
  while the player types (the chat key): the chat window with its recent lines and the input, as
  in a plain match, hidden again when the message is sent or cancelled; while Tab is held (alive or
  dead): the chat window with its history held on (its last lines, as the chat keeps them);
  otherwise hidden, and a new message does not pop up.
- **Tab held while dead** (the owner, 2026-10-26: "pressing tab when dying should show kits"): the
  kits, as a plain match shows them on the death screen and before the start, to pick by a click
  or 1 to 3 (shown chosen as in a plain match), the teammates' names (the markers are always
  shown) and the time left; hidden again when Tab is let go.
  Keys 1 to 3 pick a kit with or without Tab. A death or a respawn while Tab is held switches
  between the kits and the alive player's view immediately.
- **Keys while Tab is held** go on as with Tab up (a plain match's Tab list takes every key while it
  is up; Blackout has no list): the kits' 1 to 3, the replay's keys; acts are filtered, and
  movement is filtered after the 200 ms grace.
- **Tab held stops the player after 200 ms:** a quick tap keeps walking and sprinting;
  holding longer stops them. Actions, including jumping, are blocked immediately (a crouch stays, it does not
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
- **Replays and late joiners** see the match as it was (no interface but the teammates' markers).
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
- **Exe, patch 65:** the Rust module `patches::blackout` (`client-patches/rust/patches/src/
  blackout.rs`, its hooks `blackout.toml`; `client_patch.md` 3.58, `rust_patches.md`), six
  `lohook`s at sites of its own (`sites_026g.tsv`), no other patch touched:
  - `static_info.options_read` (0x27B92D) and `replay.options_read` (0x1F0971), after the two
    calls of `match_options::deserialize` (the 0xE4's and a replay's chunk 1): srp's block is read
    from the reader's cursor (where the options end), the options keep the mark (+57870h) and the
    patch the match's flag;
  - `player_icons.update_start` (0x23AA10, each frame): the hud movie told
    `root.force_interface_hidden(flag)` and the player icons `root.force_interface_hidden(flag)`,
    each at a change of the movie (the game world's own) or of the state, and again at the next
    frame while a movie does not take it (Tab's state is not told: the markers do not depend on it);
  - `keyboard.tab_state` (0x25CD2C): Tab's state; in a Blackout match the key goes on as with Tab
    up (bl cleared: the game's Tab test at 0x25CD39 no longer sends it to the function's end, so
    patch 58's kit keys at 0x25CD41 get it);
  - `player_names.visibility` (0x1F2F10): while Tab is held, show all eligible teammate
    names as with Alt. Keep the native enemy/spectator decisions and earlier visibility checks;
    releasing Tab restores the native Alt/crosshair decision.
  - `input_builder.end` (0x2421B0): acts out while Tab is held; moves out after a 200 ms grace, reset by release or a new match.
  - **Beside the other patches:** patch 63 (team damage, #370) reads srp's block at
    `match_options::deserialize`'s end (0x1F0F30), Research's markers (#367) wrap update_ui's call
    of update_player_icons (0x23B207), patch 62 (F11) keeps its reset (0x234AE8) and key: each owns
    another site, so they land beside Blackout unchanged (each site has one owner; none is a
    chain). The forced interface's F11 lock and its reset are the movies' (below).
- **The hud movie:** `force_interface_hidden` hides the root as F11 does; what it keeps is
  `ForcedView`'s (`survarium/controls/ForcedView.as`: the parts by the forced state, Tab held and the
  chat typed in, `focus_chat`; the chat kept while typed in or on Tab, its history held on for Tab):
  the countdown
  (`pregameInfo`) in a layer over it (`KeptLayer`: the layer takes no mouse, its parts do, so the
  kits are clicked there; the first build made the whole layer mouse-blind, the owner's
  2026-10-26 "I cannot switch between profiles"), placed as the root (and still placed by the hud's own
  layout there, `Layout.manage`: centred as in any match, at every resolution and interface
  scale, also when kept before the layout's first pass at the match's start); while forced, the
  game's Tab (`show_player_list`) moves the match's time left (the label `set_match_time` writes
  for the mode, `matchTime`) and, alive, the ammo (`ammoInd`) and the quick-use slots or, while
  the game shows the kits (`show_profiles_selector`: the death screen, before the start), the kits
  (`_profilePanel`) into that layer and back instead of showing the list (`keepOnTab`: what it
  kept is given back as it was). Forced, `hide_interface` (F11, patch 62; a match's reset) does
  nothing: the interface stays forced until the exe tells the movie otherwise (a match without
  Blackout: `force_interface_hidden(false)` at its first frame, which puts every kept part back).
- **The player icons' movie:** forced (`force_interface_hidden`), it hides the match's objects'
  icons (ids from 37, `HUDIconLayer.FIRST_OBJECT`: the game gives a player's icon his index and its
  objects' from 25h) and keeps the players' as in a plain match (the teammates'; an enemy's never,
  `HUDIconLayer.canShow` with the game's `set_enemy_icon_ids`); forced, `hide_interface` (F11, a
  match's reset) does nothing.
- **Checks in Ruffle:** `scripts/blackout-kits/check.sh <hud.swf> <player_icons.swf>`: the hud's kit
  panel and body doll in its `KeptLayer` (a kit key, a real click), the player icons movie told each
  state: the teammate's marker shown, forced or not, the enemy's never, the object's not while
  forced (and F11 doing nothing while forced).
- **The lobby movie:** `GameMode.BLACKOUT`, its names; the create window ticks the modes the pool
  offers; a mode's colour on the vote card from one table.
- **With the interface levels** (`feat/interface-level`, its own patch 64): Blackout's telling the
  hud becomes the Disabled level once both are merged, its countdown and Tab view kept.

## Open for the owner

- The name; the bands (parked, `feat/blackout-bands`).
- Saved room settings: a mode is in unless its maker left it out, so settings kept before Blackout
  have it ticked, as the defaults do; no schema step is needed.
- A game without patch 65 would show the interface: refuse it by revision before a
  release (`SRP_MIN_PATCH_REVISION`).
